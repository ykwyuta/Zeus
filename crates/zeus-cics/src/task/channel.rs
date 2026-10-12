//! Channels and containers, and the asynchronous API (RUN TRANSID, FETCH
//! CHILD / ANY, FREE CHILD). A child task runs to its end when it is
//! started (there is no concurrency in a region), on a copy of the channel
//! it was given; FETCH returns that channel as the child left it.

use std::collections::BTreeMap;

use super::*;

/// The containers of a channel, by name.
pub type Channel = BTreeMap<String, Vec<u8>>;

/// A child task that ended and was not fetched yet.
pub struct Child {
    token: Vec<u8>,
    channel: Option<(String, Channel)>,
    /// The abend code (None: it ended normally).
    abend: Option<String>,
}

/// The name in a data area or literal (16 characters: case kept,
/// trailing blanks removed).
fn name_of(m: &Machine, a: &ExecArg) -> String {
    let b = Task::bytes(m, a);
    ebcdic::decode_latin1(&b)
        .iter()
        .map(|&c| c as char)
        .collect::<String>()
        .trim_end()
        .to_string()
}

fn padded(name: &str, len: usize) -> Vec<u8> {
    let mut b = ebcdic::encode(name);
    b.resize(len, ebcdic::SPACE);
    b
}

impl Task {
    fn opt_name(m: &Machine, call: &ExecCall, opt: &str) -> Option<String> {
        call.arg(opt).map(|a| name_of(m, a))
    }

    /// The channel of a command: CHANNEL, else the program's current one.
    fn channel_name(&self, m: &Machine, call: &ExecCall) -> Option<String> {
        Self::opt_name(m, call, "CHANNEL")
            .or_else(|| self.st.borrow().channel_stack.last().cloned().flatten())
    }

    /// PUT / GET / DELETE / MOVE CONTAINER.
    pub(super) fn container(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        let name = Self::opt_name(m, call, "CONTAINER").unwrap_or_default();
        let Some(chan) = self.channel_name(m, call) else {
            // no channel named and no current channel
            return self.finish(m, call, "INVREQ", 4);
        };
        match call.command.as_str() {
            "PUT" => {
                let data = Self::from_data(m, call);
                let mut st = self.st.borrow_mut();
                let c = st.channels.entry(chan).or_default();
                if call.has("APPEND") {
                    c.entry(name).or_default().extend(data);
                } else {
                    c.insert(name, data);
                }
                drop(st);
                self.finish(m, call, "NORMAL", 0)
            }
            "GET" => {
                let data = {
                    let st = self.st.borrow();
                    let Some(c) = st.channels.get(&chan) else {
                        drop(st);
                        return self.finish(m, call, "CHANNELERR", 2);
                    };
                    match c.get(&name) {
                        Some(d) => d.clone(),
                        None => {
                            drop(st);
                            return self.finish(m, call, "CONTAINERERR", 10);
                        }
                    }
                };
                if call.has("NODATA") {
                    if let Some(l) = Self::area(call, "FLENGTH") {
                        Self::store_num(m, &l, data.len() as i64);
                    }
                    return self.finish(m, call, "NORMAL", 0);
                }
                if let Some(p) = Self::area(call, "SET") {
                    let addr = m.getmain(data.len().max(1), 0);
                    m.storage_mut()[addr..addr + data.len()].copy_from_slice(&data);
                    Self::store_num(m, &p, addr as i64);
                    if let Some(l) = Self::area(call, "FLENGTH") {
                        Self::store_num(m, &l, data.len() as i64);
                    }
                    return self.finish(m, call, "NORMAL", 0);
                }
                let max = match Self::opt_num(m, call, "FLENGTH") {
                    Some(l) => l.max(0) as usize,
                    None => Self::area(call, "INTO").map_or(0, |a| a.len),
                };
                if let Some(into) = Self::area(call, "INTO") {
                    let n = data.len().min(max);
                    m.storage_mut()[into.addr..into.addr + n].copy_from_slice(&data[..n]);
                }
                if let Some(l) = Self::area(call, "FLENGTH") {
                    Self::store_num(m, &l, data.len() as i64);
                }
                if data.len() > max {
                    self.finish(m, call, "LENGERR", 11)
                } else {
                    self.finish(m, call, "NORMAL", 0)
                }
            }
            "DELETE" => {
                let gone = self
                    .st
                    .borrow_mut()
                    .channels
                    .get_mut(&chan)
                    .and_then(|c| c.remove(&name))
                    .is_some();
                self.finish(m, call, if gone { "NORMAL" } else { "CONTAINERERR" }, 10)
            }
            _ => {
                // MOVE CONTAINER(a) AS(b) [CHANNEL(x)] [TOCHANNEL(y)]
                let to_name = Self::opt_name(m, call, "AS").unwrap_or_else(|| name.clone());
                let to_chan = Self::opt_name(m, call, "TOCHANNEL").unwrap_or_else(|| chan.clone());
                let mut st = self.st.borrow_mut();
                let Some(d) = st.channels.get_mut(&chan).and_then(|c| c.remove(&name)) else {
                    drop(st);
                    return self.finish(m, call, "CONTAINERERR", 10);
                };
                st.channels.entry(to_chan).or_default().insert(to_name, d);
                drop(st);
                self.finish(m, call, "NORMAL", 0)
            }
        }
    }

    /// STARTBROWSE / GETNEXT / ENDBROWSE CONTAINER.
    pub(super) fn container_browse(
        &self,
        m: &mut Machine,
        call: &ExecCall,
    ) -> RResult<ExecOutcome> {
        match call.command.as_str() {
            "STARTBROWSE" => {
                let Some(chan) = self.channel_name(m, call) else {
                    return self.finish(m, call, "INVREQ", 4);
                };
                let names: Vec<String> = {
                    let st = self.st.borrow();
                    match st.channels.get(&chan) {
                        Some(c) => c.keys().cloned().collect(),
                        None => {
                            drop(st);
                            return self.finish(m, call, "CHANNELERR", 2);
                        }
                    }
                };
                let token = {
                    let mut st = self.st.borrow_mut();
                    st.container_browses.push(Some((names, 0)));
                    st.container_browses.len() as i64
                };
                if let Some(a) = Self::area(call, "BROWSETOKEN") {
                    Self::store_num(m, &a, token);
                }
                self.finish(m, call, "NORMAL", 0)
            }
            "GETNEXT" => {
                let token = Self::opt_num(m, call, "BROWSETOKEN").unwrap_or(0);
                let next = {
                    let mut st = self.st.borrow_mut();
                    match st
                        .container_browses
                        .get_mut((token - 1).max(0) as usize)
                        .and_then(|b| b.as_mut())
                    {
                        Some((names, i)) => {
                            let n = names.get(*i).cloned();
                            *i += 1;
                            Some(n)
                        }
                        None => None,
                    }
                };
                match next {
                    None => self.finish(m, call, "TOKENERR", 3),
                    Some(None) => self.finish(m, call, "END", 2),
                    Some(Some(n)) => {
                        if let Some(a) = Self::area(call, "CONTAINER") {
                            Self::store_bytes(m, &a, &padded(&n, a.len));
                        }
                        self.finish(m, call, "NORMAL", 0)
                    }
                }
            }
            _ => {
                let token = Self::opt_num(m, call, "BROWSETOKEN").unwrap_or(0);
                if let Some(b) = self
                    .st
                    .borrow_mut()
                    .container_browses
                    .get_mut((token - 1).max(0) as usize)
                {
                    *b = None;
                }
                self.finish(m, call, "NORMAL", 0)
            }
        }
    }

    /// RUN TRANSID [CHANNEL] CHILD: the child task runs now, on a copy of
    /// the channel.
    pub(super) fn run_child(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        let transid = Self::opt_text(m, call, "TRANSID").unwrap_or_default();
        if !self.region.borrow().csd.transactions.contains_key(&transid) {
            return self.finish(m, call, "TRANSIDERR", 1);
        }
        let channel = match Self::opt_name(m, call, "CHANNEL") {
            Some(n) => {
                let c = self.st.borrow().channels.get(&n).cloned();
                match c {
                    Some(c) => Some((n, c)),
                    None => return self.finish(m, call, "CHANNELERR", 1),
                }
            }
            None => None,
        };
        let names = channel.as_ref().map(|c| c.0.clone());
        let term = Rc::new(std::cell::RefCell::new(Terminal::new("")));
        let (r, back) = run_task(&self.region, &term, &transid, 0, None, Some(None), channel);
        let token = {
            let mut st = self.st.borrow_mut();
            st.child_count += 1;
            let token = ebcdic::encode(&format!("ZEUSCHILD{:07}", st.child_count));
            st.children.push(Child {
                token: token.clone(),
                channel: names.map(|n| (n, back.unwrap_or_default())),
                abend: r.err(),
            });
            token
        };
        if let Some(a) = Self::area(call, "CHILD") {
            Self::store_bytes(m, &a, &token);
        }
        self.finish(m, call, "NORMAL", 0)
    }

    /// FETCH CHILD(token) | ANY(token): the channel a child returned, its
    /// completion status and abend code.
    pub(super) fn fetch_child(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        let child = {
            let mut st = self.st.borrow_mut();
            if call.has("ANY") {
                if st.children.is_empty() {
                    None
                } else {
                    Some(st.children.remove(0))
                }
            } else {
                let want = call.arg("CHILD").map(|a| Self::bytes(m, a));
                st.children
                    .iter()
                    .position(|c| Some(&c.token) == want.as_ref())
                    .map(|i| st.children.remove(i))
            }
        };
        let Some(child) = child else {
            return if call.has("ANY") {
                self.finish(m, call, "NOTFND", 1)
            } else {
                self.finish(m, call, "INVREQ", 1)
            };
        };
        if let Some(a) = Self::area(call, "ANY") {
            Self::store_bytes(m, &a, &child.token);
        }
        if let Some((name, chan)) = child.channel {
            if let Some(a) = Self::area(call, "CHANNEL") {
                Self::store_bytes(m, &a, &padded(&name, a.len));
            }
            self.st.borrow_mut().channels.insert(name, chan);
        }
        if let Some(a) = Self::area(call, "COMPSTATUS") {
            let v = if child.abend.is_some() {
                "ABEND"
            } else {
                "NORMAL"
            };
            let v = zeus_cobol::front::cics::cvda_value(v).unwrap_or(0);
            Self::store_num(m, &a, v as i64);
        }
        if let Some(a) = Self::area(call, "ABCODE") {
            let code = child.abend.unwrap_or_default();
            Self::store_bytes(m, &a, &padded(&code, a.len));
        }
        self.finish(m, call, "NORMAL", 0)
    }

    pub(super) fn free_child(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        let want = call.arg("CHILD").map(|a| Self::bytes(m, a));
        let found = {
            let mut st = self.st.borrow_mut();
            let before = st.children.len();
            st.children.retain(|c| Some(&c.token) != want.as_ref());
            st.children.len() < before
        };
        self.finish(m, call, if found { "NORMAL" } else { "INVREQ" }, 1)
    }

    /// INQUIRE ASSOCIATION(task): where the task's work came from (this
    /// region and terminal).
    pub(super) fn inquire_association(
        &self,
        m: &mut Machine,
        call: &ExecCall,
    ) -> RResult<ExecOutcome> {
        let applid = self.region.borrow().applid.clone();
        let (termid, userid) = {
            let t = self.term.borrow();
            (t.id.clone(), t.userid.clone())
        };
        let (transid, taskn) = {
            let st = self.st.borrow();
            (st.transid.clone(), st.taskn)
        };
        for (name, arg) in &call.options {
            let Some(ExecArg::Area(a)) = arg else {
                continue;
            };
            let text = match name.as_str() {
                "ODAPPLID" | "APPLID" => applid.clone(),
                "ODUSERID" | "USERID" => userid.clone(),
                "ODFACILNAME" | "FACILNAME" | "ODLUNAME" => termid.clone(),
                "ODNETWORKID" | "NETWORKID" => "ZEUSNET".into(),
                "ODTRANSID" | "TRANSACTION" => transid.clone(),
                "ODFACILTYPE" | "FACILTYPE" => {
                    let v = if termid.is_empty() {
                        "TASK"
                    } else {
                        "TERMINAL"
                    };
                    let v = zeus_cobol::front::cics::cvda_value(v).unwrap_or(0);
                    Self::store_num(m, a, v as i64);
                    continue;
                }
                "ODTASKID" => {
                    Self::store_num(m, a, i64::from(taskn));
                    continue;
                }
                _ => continue,
            };
            Self::store_bytes(m, a, &padded(&text, a.len));
        }
        self.finish(m, call, "NORMAL", 0)
    }

    /// INQUIRE / SET TERMINAL(id) UCTRANST: the terminal's upper case
    /// translation.
    pub(super) fn terminal_setting(
        &self,
        m: &mut Machine,
        call: &ExecCall,
    ) -> RResult<ExecOutcome> {
        let id = Self::opt_text(m, call, "TERMINAL")
            .or_else(|| Self::opt_text(m, call, "NETNAME"))
            .unwrap_or_default();
        let term = if id == self.term.borrow().id.trim() {
            Some(self.term.clone())
        } else {
            self.region
                .borrow()
                .terminals
                .get(&id)
                .and_then(|w| w.upgrade())
        };
        let Some(term) = term else {
            return self.finish(m, call, "TERMIDERR", 1);
        };
        let cvda = |n: &str| i64::from(zeus_cobol::front::cics::cvda_value(n).unwrap_or(0));
        if call.command == "SET" {
            if let Some(v) = Self::opt_num(m, call, "UCTRANST") {
                term.borrow_mut().uctran = v == cvda("UCTRAN");
            }
        } else {
            let uctran = term.borrow().uctran;
            if let Some(a) = Self::area(call, "UCTRANST") {
                Self::store_num(
                    m,
                    &a,
                    if uctran {
                        cvda("UCTRAN")
                    } else {
                        cvda("NOUCTRAN")
                    },
                );
            }
            if let Some(a) = Self::area(call, "USERID") {
                let u = term.borrow().userid.clone();
                Self::store_bytes(m, &a, &padded(&u, a.len));
            }
        }
        self.finish(m, call, "NORMAL", 0)
    }
}
