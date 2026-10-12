//! Files and stream-oriented transmission (GET / PUT).

use std::io::{Read, Write};

use super::value::{self, SPACE, Value};
use super::*;

const TABS: [usize; 5] = [25, 49, 73, 97, 121];

/// Where stream output goes.
pub enum Sink {
    /// Host text lines (SYSOUT, the terminal).
    Text(Box<dyn Write>),
    /// Records of a data set (written at close).
    Records(PathBuf, zeus_ds::format::RecordFormat, Vec<Vec<u8>>),
    /// PUT STRING: the characters written.
    Buffer(Vec<u8>),
}

pub struct FileState {
    pub decl: FileDecl,
    pub open: bool,
    pub input: bool,
    pub output: bool,
    pub update: bool,
    pub print: bool,
    pub stream: bool,
    pub keyed: bool,
    pub title: Option<String>,
    // stream output
    pub sink: Option<Sink>,
    pub line: Vec<u8>,
    /// Something was written on the current line, or it was started by
    /// positioning.
    pub fresh: bool,
    pub lineno: usize,
    pub pageno: usize,
    pub linesize: usize,
    pub pagesize: usize,
    pub endpage_done: bool,
    pub asa: u8,
    // stream input
    pub inrecs: Vec<Vec<u8>>,
    pub rec: usize,
    pub pos: usize,
    // record I/O
    pub recs: Vec<Vec<u8>>,
    pub rpos: usize,
    pub path: Option<PathBuf>,
    pub format: Option<zeus_ds::format::RecordFormat>,
    pub keyed_ds: Option<zeus_ds::keyed::KeyedDataset>,
    pub keys: Option<(u32, u32)>,
    pub last_key: Vec<u8>,
    pub last_read: Option<usize>,
    pub dirty: bool,
    /// The current line was written out early (at the return of a call
    /// from another module); the next line end writes nothing.
    pub held: bool,
}

impl FileState {
    pub fn new(decl: &FileDecl) -> FileState {
        FileState {
            decl: decl.clone(),
            open: false,
            input: false,
            output: false,
            update: false,
            print: decl.print,
            stream: decl.stream,
            keyed: decl.keyed,
            title: None,
            sink: None,
            line: Vec::new(),
            fresh: true,
            held: false,
            lineno: 1,
            pageno: 1,
            linesize: 120,
            pagesize: 60,
            endpage_done: false,
            asa: b' ',
            inrecs: Vec::new(),
            rec: 0,
            pos: 0,
            recs: Vec::new(),
            rpos: 0,
            path: None,
            format: None,
            keyed_ds: None,
            keys: None,
            last_key: Vec::new(),
            last_read: None,
            dirty: false,
        }
    }

    fn ddname(&self) -> String {
        self.title
            .clone()
            .unwrap_or_else(|| self.decl.ddname.clone())
    }
}

fn host(b: &[u8]) -> String {
    zeus_cobol::ebcdic::decode(b)
}

fn ebcdic(s: &str) -> Vec<u8> {
    zeus_cobol::ebcdic::encode(s)
}

impl Machine {
    fn file_id(&mut self, f: &Option<Expr>, default: &str, act: usize) -> R<FileId> {
        match f {
            Some(e) => match self.eval(e, act)? {
                Value::File(id) => Ok(id),
                _ => err("FILE option is not a file"),
            },
            None => {
                if let Some(i) = self.module.files.iter().position(|x| x.name == default) {
                    return Ok(i as FileId);
                }
                // SYSIN / SYSPRINT used implicitly
                let decl = FileDecl {
                    name: default.to_string(),
                    ddname: default.to_string(),
                    stream: true,
                    print: default == "SYSPRINT",
                    input: default == "SYSIN",
                    output: default == "SYSPRINT",
                    update: false,
                    keyed: false,
                    direct: false,
                };
                Rc::make_mut(&mut self.module).files.push(decl.clone());
                self.files.push(FileState::new(&decl));
                Ok((self.files.len() - 1) as FileId)
            }
        }
    }

    /// Opens a file (explicitly or implicitly).
    pub(crate) fn open_file(
        &mut self,
        f: FileId,
        output: bool,
        update: bool,
        record: bool,
        act: usize,
    ) -> R<bool> {
        if self.files[f as usize].open {
            return Ok(true);
        }
        let st = &mut self.files[f as usize];
        st.output = output || st.decl.output;
        st.update = update || st.decl.update;
        st.input = !st.output && !st.update;
        st.stream = !record && st.decl.stream;
        if st.output && st.stream && st.decl.name == "SYSPRINT" {
            st.print = true;
        }
        let dd = st.ddname();
        let info = self.env.dds.get(&dd).cloned();
        let st = &mut self.files[f as usize];
        match info {
            Some(d) => {
                st.path = Some(d.path.clone());
                st.format = d.format;
                st.keys = d.keys;
                if st.stream && st.output {
                    st.sink = Some(match (d.sysout, d.format) {
                        (false, Some(fmt)) => Sink::Records(d.path.clone(), fmt, Vec::new()),
                        _ => {
                            let file = std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(&d.path);
                            match file {
                                Ok(f) => Sink::Text(Box::new(f)),
                                Err(_) => return self.undefined_file(f, act),
                            }
                        }
                    });
                } else if st.stream {
                    // stream input: records of the data set (or text)
                    let data = std::fs::read(&d.path).unwrap_or_default();
                    st.inrecs = match d.format {
                        Some(fmt) if !d.sysout && !data.is_empty() => {
                            zeus_ds::format::decode(fmt, &data).unwrap_or_default()
                        }
                        _ => String::from_utf8_lossy(&data).lines().map(ebcdic).collect(),
                    };
                    st.rec = 0;
                    st.pos = 0;
                } else {
                    self.open_records(f, act)?;
                }
            }
            None if st.decl.name == "SYSPRINT" || (dd == "SYSPRINT") => {
                st.sink = None; // the run's standard output
            }
            None if st.decl.name == "SYSIN" || dd == "SYSIN" => {
                if self.env.stdin.is_none()
                    && let Some(mut r) = self.env.stdin_reader.take()
                {
                    let mut text = String::new();
                    let _ = r.read_to_string(&mut text);
                    self.env.stdin = Some(text.lines().map(str::to_string).collect());
                }
                let st = &mut self.files[f as usize];
                st.inrecs = self
                    .env
                    .stdin
                    .clone()
                    .unwrap_or_default()
                    .iter()
                    .map(|l| ebcdic(l))
                    .collect();
                st.rec = 0;
                st.pos = 0;
            }
            None => return self.undefined_file(f, act),
        }
        let st = &mut self.files[f as usize];
        st.open = true;
        st.fresh = true;
        st.lineno = 1;
        st.line.clear();
        st.endpage_done = false;
        Ok(true)
    }

    pub(crate) fn undefined_file(&mut self, f: FileId, act: usize) -> R<bool> {
        self.raise(Cond::UndefinedFile(f), 80, act)?;
        Ok(self.files[f as usize].open)
    }

    pub(crate) fn open(&mut self, s: &OpenSpec, act: usize) -> R<()> {
        let Value::File(f) = self.eval(&s.file, act)? else {
            return err("OPEN of a non-file");
        };
        if self.files[f as usize].open {
            return Ok(());
        }
        if let Some(t) = &s.title {
            let Value::Char(t) = self.eval(t, act)? else {
                return err("TITLE");
            };
            self.files[f as usize].title = Some(host(&t).trim().to_string());
        }
        if let Some(n) = &s.linesize {
            self.files[f as usize].linesize = self.eval(n, act)?.as_i64().max(1) as usize;
        }
        if let Some(n) = &s.pagesize {
            self.files[f as usize].pagesize = self.eval(n, act)?.as_i64().max(1) as usize;
        }
        if s.print == Some(true) {
            self.files[f as usize].print = true;
        }
        if s.keyed {
            self.files[f as usize].keyed = true;
        }
        let record = !self.files[f as usize].decl.stream || s.keyed || s.update;
        self.open_file(f, s.output, s.update, record, act)?;
        Ok(())
    }

    pub(crate) fn close(&mut self, f: FileId) -> R<()> {
        if !self.files[f as usize].open {
            return Ok(());
        }
        if self.files[f as usize].stream {
            self.flush_line(f, true)?;
            let st = &mut self.files[f as usize];
            if let Some(Sink::Records(path, fmt, recs)) = st.sink.take() {
                let mut data = Vec::new();
                for r in &recs {
                    zeus_ds::format::encode_into(fmt, r, SPACE, &mut data);
                }
                let _ = zeus_ds::format::write_file(&path, &data);
            } else if let Some(Sink::Text(mut w)) = st.sink.take() {
                let _ = w.flush();
            }
        } else {
            self.close_records(f)?;
        }
        let st = &mut self.files[f as usize];
        st.open = false;
        st.title = None;
        Ok(())
    }

    /// Writes out the current lines of the stream output files (their
    /// output then precedes that of the caller of another module).
    pub(crate) fn complete_lines(&mut self) {
        for f in 0..self.files.len() {
            let st = &self.files[f];
            if st.open && st.stream && !st.line.is_empty() {
                let _ = self.flush_line(f as FileId, false);
                self.files[f].held = true;
            }
        }
    }

    pub(crate) fn close_all(&mut self) {
        for f in 0..self.files.len() {
            let _ = self.close(f as FileId);
        }
    }

    // ---- stream output -----------------------------------------------------

    /// Writes out the current line.
    fn flush_line(&mut self, f: FileId, at_close: bool) -> R<()> {
        let st = &mut self.files[f as usize];
        if std::mem::take(&mut st.held) && st.line.is_empty() {
            return Ok(());
        }
        if at_close && st.line.is_empty() {
            return Ok(());
        }
        let line = std::mem::take(&mut st.line);
        let asa = std::mem::replace(&mut st.asa, b' ');
        let print = st.print;
        match &mut st.sink {
            None => {
                let mut text = zeus_cobol::ebcdic::to_host(&line);
                if asa == b'1' && print {
                    text.insert(0, b'\x0c');
                }
                let _ = self.env.stdout.write_all(&text);
                let _ = self.env.stdout.write_all(b"\n");
            }
            Some(Sink::Text(w)) => {
                let mut text = zeus_cobol::ebcdic::to_host(&line);
                if asa == b'1' && print {
                    text.insert(0, b'\x0c');
                }
                let _ = w.write_all(&text);
                let _ = w.write_all(b"\n");
            }
            Some(Sink::Records(_, _, recs)) => {
                let mut r = Vec::new();
                if print {
                    r.push(ebcdic(&(asa as char).to_string())[0]);
                }
                r.extend(line);
                recs.push(r);
            }
            Some(Sink::Buffer(b)) => b.extend(line),
        }
        st.fresh = false;
        Ok(())
    }

    /// SKIP(n): to the start of the n-th next line.
    fn skip_lines(&mut self, f: FileId, n: i64, act: usize) -> R<()> {
        let st = &self.files[f as usize];
        if st.print && n <= 0 {
            // overprint: back to column 1
            self.flush_line(f, false)?;
            self.files[f as usize].asa = b'+';
            return Ok(());
        }
        let n = n.max(1) as usize;
        if st.fresh && st.line.is_empty() && st.lineno == 1 && st.pageno == 1 {
            // first positioning on a new file: line n
            for _ in 1..n {
                self.flush_line(f, false)?;
            }
            self.files[f as usize].lineno = n;
            self.files[f as usize].fresh = false;
            return Ok(());
        }
        self.flush_line(f, false)?;
        for _ in 1..n {
            self.flush_line(f, false)?;
        }
        let st = &mut self.files[f as usize];
        st.lineno += n;
        if st.print && st.lineno > st.pagesize && !st.endpage_done {
            st.endpage_done = true;
            self.raise(Cond::EndPage(f), 90, act)?;
        }
        Ok(())
    }

    pub(crate) fn new_page(&mut self, f: FileId) -> R<()> {
        let st = &mut self.files[f as usize];
        if !st.line.is_empty() || !st.fresh {
            self.flush_line(f, false)?;
        }
        let st = &mut self.files[f as usize];
        st.asa = b'1';
        st.lineno = 1;
        st.pageno += 1;
        st.endpage_done = false;
        st.fresh = true;
        Ok(())
    }

    fn line_to(&mut self, f: FileId, n: i64, act: usize) -> R<()> {
        let n = n.max(1) as usize;
        let cur = self.files[f as usize].lineno;
        if n > cur {
            self.skip_lines(f, (n - cur) as i64, act)
        } else if n == cur && self.files[f as usize].line.is_empty() {
            Ok(())
        } else {
            self.new_page(f)?;
            self.skip_lines(f, n as i64 - 1, act).map(|_| ())
        }
    }

    /// Writes text at the current position (wrapping at LINESIZE).
    fn out_text(&mut self, f: FileId, text: &[u8], act: usize) -> R<()> {
        let ls = self.files[f as usize].linesize;
        let len = text.len();
        if !self.files[f as usize].line.is_empty()
            && self.files[f as usize].line.len() + len > ls
            && len <= ls
        {
            self.skip_lines(f, 1, act)?;
        }
        for &c in text {
            if self.files[f as usize].line.len() >= ls {
                self.skip_lines(f, 1, act)?;
            }
            self.files[f as usize].line.push(c);
        }
        Ok(())
    }

    fn column(&mut self, f: FileId, n: usize, act: usize) -> R<()> {
        let cur = self.files[f as usize].line.len() + 1;
        if n < cur {
            self.skip_lines(f, 1, act)?;
        }
        let ls = self.files[f as usize].linesize;
        let n = if n > ls || n < 1 { 1 } else { n };
        let st = &mut self.files[f as usize];
        while st.line.len() + 1 < n {
            st.line.push(SPACE);
        }
        Ok(())
    }

    /// The list-directed form of an item.
    fn list_text(&mut self, v: &Value, ty: &Ty, print: bool, data: bool) -> Vec<u8> {
        match (v, ty) {
            (Value::Char(s), Ty::Pic(_)) => s.clone(),
            (Value::Char(s), _) => {
                if print && !data {
                    s.clone()
                } else {
                    let q = ebcdic("'")[0];
                    let mut out = vec![q];
                    for &c in s {
                        out.push(c);
                        if c == q {
                            out.push(q);
                        }
                    }
                    out.push(q);
                    out
                }
            }
            (Value::Bit(b), _) => {
                let s: String = b.iter().map(|&x| if x { '1' } else { '0' }).collect();
                ebcdic(&format!("'{s}'B"))
            }
            (Value::Fixed { .. } | Value::Float(_), t) => ebcdic(&self.arith_text(v, t)),
            (Value::Ptr(p), _) => ebcdic(&format!("{p:08X}")),
            (Value::File(i), _) => ebcdic(&self.module.files[*i as usize].name.clone()),
            (other, _) => ebcdic(&format!("{other:?}")),
        }
    }

    /// Writes one list-directed (or data-directed) item.
    fn put_item(&mut self, f: FileId, text: Vec<u8>, act: usize) -> R<()> {
        let st = &self.files[f as usize];
        let col = st.line.len() + 1;
        if st.print {
            if col > 1 {
                match TABS.iter().find(|&&t| t > col) {
                    Some(&t) if t <= st.linesize => self.column(f, t, act)?,
                    _ => self.skip_lines(f, 1, act)?,
                }
            }
        } else if col > 1 {
            self.out_text(f, &[SPACE], act)?;
        }
        self.out_text(f, &text, act)
    }

    pub(crate) fn put(&mut self, s: &Stream, act: usize) -> R<()> {
        let string_mode = s.string.is_some();
        let f = if string_mode {
            self.string_file()
        } else {
            let f = self.file_id(&s.file, "SYSPRINT", act)?;
            if !self.files[f as usize].open && !self.open_file(f, true, false, false, act)? {
                return Ok(());
            }
            f
        };
        if s.page {
            self.new_page(f)?;
        }
        if let Some(n) = &s.line {
            let n = self.eval(n, act)?.as_i64();
            self.line_to(f, n, act)?;
        }
        if let Some(n) = &s.skip {
            let n = self.eval(n, act)?.as_i64();
            self.skip_lines(f, n, act)?;
        }
        match &s.data {
            None => {}
            Some(Transmit::List(items)) => {
                let print = self.files[f as usize].print;
                let mut vals = Vec::new();
                self.collect_out(items, act, &mut vals)?;
                for (v, t, _) in vals {
                    let text = self.list_text(&v, &t, print, false);
                    self.put_item(f, text, act)?;
                }
            }
            Some(Transmit::Data(items)) => {
                let mut vals = Vec::new();
                self.collect_out(items, act, &mut vals)?;
                let n = vals.len();
                for (i, (v, t, name)) in vals.into_iter().enumerate() {
                    let mut text = ebcdic(&format!("{name}="));
                    text.extend(self.list_text(&v, &t, false, true));
                    if i + 1 == n {
                        text.extend(ebcdic(";"));
                    }
                    self.put_item(f, text, act)?;
                }
            }
            Some(Transmit::Edit(pairs)) => {
                for (items, fmts) in pairs {
                    let mut vals = Vec::new();
                    self.collect_out(items, act, &mut vals)?;
                    self.put_edit(f, vals, fmts, act)?;
                }
            }
        }
        if let Some(StringTarget::Target(t, _)) = &s.string {
            let st = &mut self.files[f as usize];
            let mut text = std::mem::take(&mut st.line);
            if let Some(Sink::Buffer(b)) = st.sink.take() {
                let mut all = b;
                all.extend(text);
                text = all;
            }
            self.files.pop();
            let l = self.target_loc(t, act)?;
            self.assign_target(t, l, Value::Char(text), &crate::sema::expr::char_any(), act)?;
        }
        Ok(())
    }

    /// A temporary file for PUT / GET STRING.
    fn string_file(&mut self) -> FileId {
        let decl = FileDecl {
            name: "*STRING".into(),
            ddname: String::new(),
            stream: true,
            print: false,
            input: false,
            output: true,
            update: false,
            keyed: false,
            direct: false,
        };
        let mut st = FileState::new(&decl);
        st.open = true;
        st.linesize = usize::MAX / 2;
        st.sink = Some(Sink::Buffer(Vec::new()));
        self.files.push(st);
        (self.files.len() - 1) as FileId
    }

    /// The values of output data items: value, type and name.
    fn collect_out(
        &mut self,
        items: &[DataItem],
        act: usize,
        out: &mut Vec<(Value, Ty, String)>,
    ) -> R<()> {
        for it in items {
            match it {
                DataItem::Expr(e) => {
                    let v = self.eval(e, act)?;
                    out.push((v, e.ty.clone(), String::new()));
                }
                DataItem::Aggregate(r) => {
                    let names = self.element_names(r, act)?;
                    for (l, n) in self.elements(r, act)?.into_iter().zip(names) {
                        let v = self.load(&l)?;
                        out.push((v, l.ty, n));
                    }
                }
                DataItem::Target(..) => {}
                DataItem::Named(name, inner) => {
                    let start = out.len();
                    self.collect_out(std::slice::from_ref(inner), act, out)?;
                    for x in &mut out[start..] {
                        if x.2.is_empty() {
                            x.2 = name.clone();
                        } else if !x.2.contains('(') && !x.2.contains('.') {
                            x.2 = format!("{name}{}", &x.2[x.2.find('(').unwrap_or(x.2.len())..]);
                        }
                    }
                }
                DataItem::Repeat(inner, kind) => {
                    self.repeat_items(kind, act, &mut |me, a| me.collect_out(inner, a, out))?;
                }
            }
        }
        Ok(())
    }

    /// Names of the elements of an aggregate (for PUT DATA).
    fn element_names(&mut self, r: &Ref, act: usize) -> R<Vec<String>> {
        let m = self.module.clone();
        let var = &m.vars[r.var as usize];
        let mut qual = vec![var.name.clone()];
        let mut p = var.parent;
        while let Some(x) = p {
            qual.insert(0, m.vars[x as usize].name.clone());
            p = m.vars[x as usize].parent;
        }
        let name = qual.join(".");
        let (_, dims) = self.base_of(r, act)?;
        let free: Vec<(i64, i64)> = r
            .subs
            .iter()
            .enumerate()
            .filter(|(_, s)| s.is_none())
            .map(|(k, _)| (dims[k].0, dims[k].1))
            .collect();
        if var.ty.is_none() {
            // structure: member names
            let n = self.elements(r, act)?.len();
            return Ok(vec![name; n]);
        }
        Ok(super::index_space(&free)
            .into_iter()
            .map(|ix| {
                if ix.is_empty() {
                    name.clone()
                } else {
                    format!(
                        "{name}({})",
                        ix.iter()
                            .map(|i| i.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                }
            })
            .collect())
    }

    /// Runs `f` for each iteration of a repetitive specification.
    fn repeat_items(
        &mut self,
        kind: &DoKind,
        act: usize,
        f: &mut dyn FnMut(&mut Machine, usize) -> R<()>,
    ) -> R<()> {
        let DoKind::Iter { var, var_ty, specs } = kind else {
            return f(self, act);
        };
        for s in specs {
            let loc = self.target_loc(var, act)?;
            let start = self.eval(&s.start, act)?;
            self.assign_target(var, loc.clone(), start, var_ty, act)?;
            let to = match &s.to {
                Some(e) => Some((self.eval(e, act)?, e.ty.clone())),
                None => None,
            };
            let by = match &s.by {
                Some(e) => Some((self.eval(e, act)?, e.ty.clone())),
                None if to.is_some() => Some((Value::fixed(1, 0, true), Ty::fixed_dec(1, 0))),
                None => None,
            };
            while let TargetLoc::Loc(l) = &loc {
                let cur = self.load(l)?;
                if let (Some((tv, tt)), Some((bv, _))) = (&to, &by) {
                    let o = self.cmp_arith(&cur, var_ty, tv, tt)?;
                    let neg = bv.as_f64() < 0.0;
                    if (neg && o == std::cmp::Ordering::Less)
                        || (!neg && o == std::cmp::Ordering::Greater)
                    {
                        break;
                    }
                }
                if let Some(w) = &s.while_
                    && !self.eval(w, act)?.truth()
                {
                    break;
                }
                f(self, act)?;
                if let Some(u) = &s.until
                    && self.eval(u, act)?.truth()
                {
                    break;
                }
                match &by {
                    Some((bv, bt)) => {
                        let (v, t) = self.add_values(&cur, var_ty, bv, bt, act)?;
                        let cur2 = self.load(l)?;
                        let _ = cur2;
                        self.assign_target(var, loc.clone(), v, &t, act)?;
                    }
                    None => break,
                }
            }
        }
        Ok(())
    }

    fn fmt_int(&mut self, e: &Expr, act: usize) -> R<i64> {
        Ok(self.eval(e, act)?.as_i64())
    }

    /// Flattens format items (iterations, remote formats) into a list,
    /// evaluating iteration factors.
    fn flatten_formats(
        &mut self,
        fmts: &[Fmt],
        act: usize,
        out: &mut Vec<Fmt>,
        depth: usize,
    ) -> R<()> {
        if depth > 20 {
            return err("format list nested too deeply");
        }
        for f in fmts {
            match f {
                Fmt::Repeat(n, inner) => {
                    let n = self.fmt_int(n, act)?;
                    for _ in 0..n.max(0) {
                        self.flatten_formats(inner, act, out, depth + 1)?;
                    }
                }
                Fmt::R(id) => {
                    let inner = self.module.formats[*id as usize].clone();
                    self.flatten_formats(&inner, act, out, depth + 1)?;
                }
                other => out.push(other.clone()),
            }
        }
        Ok(())
    }

    fn put_edit(
        &mut self,
        f: FileId,
        vals: Vec<(Value, Ty, String)>,
        fmts: &[Fmt],
        act: usize,
    ) -> R<()> {
        let mut flat = Vec::new();
        self.flatten_formats(fmts, act, &mut flat, 0)?;
        if flat.iter().all(|x| {
            matches!(
                x,
                Fmt::X(_) | Fmt::Skip(_) | Fmt::Page | Fmt::Line(_) | Fmt::Column(_)
            )
        }) {
            if vals.is_empty() {
                for x in &flat {
                    self.put_control(f, x, act)?;
                }
                return Ok(());
            }
            return err("format list without data format items");
        }
        let mut k = 0usize;
        let n = vals.len();
        for (i, (v, ty, _)) in vals.into_iter().enumerate() {
            loop {
                let fm = flat[k % flat.len()].clone();
                k += 1;
                if self.put_control(f, &fm, act)? {
                    continue;
                }
                let text = self.edit_out(&fm, &v, &ty, act)?;
                self.out_text(f, &text, act)?;
                break;
            }
            // control items after the last data item are not executed
            let _ = (i, n);
        }
        Ok(())
    }

    /// Executes a control format item; false for data format items.
    fn put_control(&mut self, f: FileId, fm: &Fmt, act: usize) -> R<bool> {
        match fm {
            Fmt::X(w) => {
                let w = self.fmt_int(w, act)?.max(0) as usize;
                self.out_text(f, &vec![SPACE; w], act)?;
            }
            Fmt::Skip(n) => {
                let n = match n {
                    Some(e) => self.fmt_int(e, act)?,
                    None => 1,
                };
                self.skip_lines(f, n, act)?;
            }
            Fmt::Page => self.new_page(f)?,
            Fmt::Line(n) => {
                let n = self.fmt_int(n, act)?;
                self.line_to(f, n, act)?;
            }
            Fmt::Column(n) => {
                let n = self.fmt_int(n, act)?.max(1) as usize;
                self.column(f, n, act)?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// The output text of a data format item.
    fn edit_out(&mut self, fm: &Fmt, v: &Value, ty: &Ty, act: usize) -> R<Vec<u8>> {
        Ok(match fm {
            Fmt::A(w) => {
                let s = match self.convert(v.clone(), ty, &crate::sema::expr::char_any(), act)? {
                    Value::Char(s) => s,
                    _ => Vec::new(),
                };
                match w {
                    Some(w) => {
                        let w = self.fmt_int(w, act)?.max(0) as usize;
                        let mut s = s;
                        s.resize(w, SPACE);
                        s
                    }
                    None => s,
                }
            }
            Fmt::B(w) => {
                let b = match self.convert(v.clone(), ty, &crate::sema::expr::bit_any(), act)? {
                    Value::Bit(b) => b,
                    _ => Vec::new(),
                };
                let mut s: Vec<u8> = b.iter().map(|&x| if x { 0xF1 } else { 0xF0 }).collect();
                if let Some(w) = w {
                    let w = self.fmt_int(w, act)?.max(0) as usize;
                    s.resize(w, SPACE);
                }
                s
            }
            Fmt::F(w, d, sc) => {
                let w = self.fmt_int(w, act)?.max(0) as usize;
                let d = match d {
                    Some(d) => self.fmt_int(d, act)?.max(0) as u32,
                    None => 0,
                };
                let scale = match sc {
                    Some(s) => self.fmt_int(s, act)?,
                    None => 0,
                };
                let x = self.convert(v.clone(), ty, &Ty::Float { dec: true, p: 16 }, act)?;
                let text = match v {
                    Value::Fixed { m, q, dec: true } if scale == 0 => fixed_f_text(*m, *q, d),
                    _ => {
                        let f = x.as_f64() * 10f64.powi(scale as i32);
                        float_f_text(f, d)
                    }
                };
                let text = if text.len() > w && w > 0 {
                    if self.enabled_now(act, cond::SIZE) {
                        self.raise(Cond::Size, 341, act)?;
                    }
                    "*".repeat(w)
                } else {
                    format!("{text:>w$}")
                };
                ebcdic(&text)
            }
            Fmt::E(w, d, s) => {
                let w = self.fmt_int(w, act)?.max(0) as usize;
                let d = match d {
                    Some(d) => self.fmt_int(d, act)?.max(0) as usize,
                    None => 0,
                };
                let s = match s {
                    Some(s) => self.fmt_int(s, act)?.max(1) as usize,
                    None => d + 1,
                };
                let f = self
                    .convert(v.clone(), ty, &Ty::Float { dec: true, p: 16 }, act)?
                    .as_f64();
                ebcdic(&format!("{:>w$}", e_text(f, d, s)))
            }
            Fmt::P(p) => {
                let pt = Ty::Pic(p.clone());
                match self.convert(v.clone(), ty, &pt, act)? {
                    Value::Char(c) => c,
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        })
    }

    // ---- stream input ------------------------------------------------------

    pub(crate) fn get(&mut self, s: &Stream, act: usize) -> R<()> {
        let f = if let Some(StringTarget::Source(e)) = &s.string {
            let Value::Char(text) = self.eval(e, act)? else {
                return err("GET STRING");
            };
            let mut st = FileState::new(&FileDecl {
                name: "*STRING".into(),
                ddname: String::new(),
                stream: true,
                print: false,
                input: true,
                output: false,
                update: false,
                keyed: false,
                direct: false,
            });
            st.open = true;
            st.inrecs = vec![text];
            self.files.push(st);
            (self.files.len() - 1) as FileId
        } else {
            let f = self.file_id(&s.file, "SYSIN", act)?;
            if !self.files[f as usize].open && !self.open_file(f, false, false, false, act)? {
                return Ok(());
            }
            f
        };
        let string = s.string.is_some();
        let r = (|| -> R<()> {
            if let Some(n) = &s.skip {
                let n = self.eval(n, act)?.as_i64();
                let st = &mut self.files[f as usize];
                for _ in 0..n.max(1) {
                    st.rec += 1;
                    st.pos = 0;
                }
            }
            match &s.data {
                None => Ok(()),
                Some(Transmit::List(items)) => {
                    let mut targets = Vec::new();
                    self.collect_targets(items, &mut targets);
                    for t in targets {
                        self.get_list_item(f, &t, act)?;
                    }
                    Ok(())
                }
                Some(Transmit::Edit(pairs)) => {
                    for (items, fmts) in pairs {
                        let mut targets = Vec::new();
                        self.collect_targets(items, &mut targets);
                        let mut flat = Vec::new();
                        self.flatten_formats(fmts, act, &mut flat, 0)?;
                        let mut k = 0;
                        for t in targets {
                            loop {
                                let fm = flat[k % flat.len()].clone();
                                k += 1;
                                if self.get_control(f, &fm, act)? {
                                    continue;
                                }
                                self.get_edit_item(f, &fm, &t, act)?;
                                break;
                            }
                        }
                    }
                    Ok(())
                }
                Some(Transmit::Data(items)) => self.get_data(f, items, act),
            }
        })();
        if string {
            self.files.pop();
        }
        match r {
            Err(Intr::Fatal(m)) if m == "ENDFILE" => {
                if string {
                    self.raise(Cond::Error, 1002, act)
                } else {
                    self.raise(Cond::EndFile(f), 70, act)
                }
            }
            other => other,
        }
    }

    fn collect_targets(&self, items: &[DataItem], out: &mut Vec<DataItem>) {
        for it in items {
            match it {
                DataItem::Named(_, x) => self.collect_targets(std::slice::from_ref(x), out),
                other => out.push(other.clone()),
            }
        }
    }

    /// The next list-directed field: None at end of file.
    fn next_field(&mut self, f: FileId) -> Option<Vec<u8>> {
        let comma = ebcdic(",")[0];
        let quote = ebcdic("'")[0];
        let st = &mut self.files[f as usize];
        // skip blanks (record ends count as blanks)
        loop {
            let rec = st.inrecs.get(st.rec)?;
            if st.pos >= rec.len() {
                st.rec += 1;
                st.pos = 0;
                continue;
            }
            if rec[st.pos] == SPACE {
                st.pos += 1;
                continue;
            }
            break;
        }
        let rec = st.inrecs[st.rec].clone();
        if rec[st.pos] == comma {
            st.pos += 1;
            return Some(Vec::new());
        }
        let mut out = Vec::new();
        if rec[st.pos] == quote {
            // quoted string (may continue on the next record)
            out.push(quote);
            st.pos += 1;
            loop {
                let rec = st.inrecs.get(st.rec)?.clone();
                if st.pos >= rec.len() {
                    st.rec += 1;
                    st.pos = 0;
                    continue;
                }
                let c = rec[st.pos];
                st.pos += 1;
                if c == quote {
                    if rec.get(st.pos) == Some(&quote) {
                        out.push(quote);
                        out.push(quote);
                        st.pos += 1;
                        continue;
                    }
                    out.push(quote);
                    // suffix B / X
                    while let Some(&s) = rec.get(st.pos)
                        && s != SPACE
                        && s != comma
                    {
                        out.push(s);
                        st.pos += 1;
                    }
                    break;
                }
                out.push(c);
            }
        } else {
            while st.pos < rec.len() && rec[st.pos] != SPACE && rec[st.pos] != comma {
                out.push(rec[st.pos]);
                st.pos += 1;
            }
        }
        // a following comma (after blanks) ends the field
        let mut p = st.pos;
        while p < rec.len() && rec[p] == SPACE {
            p += 1;
        }
        if p < rec.len() && rec[p] == comma {
            st.pos = p + 1;
        }
        Some(out)
    }

    fn get_list_item(&mut self, f: FileId, t: &DataItem, act: usize) -> R<()> {
        match t {
            DataItem::Target(target, ty) => {
                let Some(field) = self.next_field(f) else {
                    return Err(Intr::Fatal("ENDFILE".into()));
                };
                if field.is_empty() {
                    return Ok(());
                }
                let (v, vt) = list_value(&field);
                let l = self.target_loc(target, act)?;
                let _ = ty;
                self.assign_target(target, l, v, &vt, act)
            }
            DataItem::Aggregate(r) => {
                for l in self.elements(r, act)? {
                    let Some(field) = self.next_field(f) else {
                        return Err(Intr::Fatal("ENDFILE".into()));
                    };
                    if field.is_empty() {
                        continue;
                    }
                    let (v, vt) = list_value(&field);
                    self.assign_loc(&l, v, &vt, act)?;
                }
                Ok(())
            }
            DataItem::Repeat(inner, kind) => {
                let inner = inner.clone();
                self.repeat_items(kind, act, &mut |me, a| {
                    let mut ts = Vec::new();
                    me.collect_targets(&inner, &mut ts);
                    for t in ts {
                        me.get_list_item(f, &t, a)?;
                    }
                    Ok(())
                })
            }
            _ => Ok(()),
        }
    }

    /// Takes `w` characters from the input stream.
    fn take_chars(&mut self, f: FileId, w: usize) -> R<Vec<u8>> {
        let st = &mut self.files[f as usize];
        let mut out = Vec::new();
        while out.len() < w {
            let Some(rec) = st.inrecs.get(st.rec) else {
                if out.is_empty() {
                    return Err(Intr::Fatal("ENDFILE".into()));
                }
                return err("end of file in the middle of an item");
            };
            if st.pos >= rec.len() {
                st.rec += 1;
                st.pos = 0;
                continue;
            }
            out.push(rec[st.pos]);
            st.pos += 1;
        }
        Ok(out)
    }

    fn get_control(&mut self, f: FileId, fm: &Fmt, act: usize) -> R<bool> {
        match fm {
            Fmt::X(w) => {
                let w = self.fmt_int(w, act)?.max(0) as usize;
                self.take_chars(f, w)?;
            }
            Fmt::Skip(n) => {
                let n = match n {
                    Some(e) => self.fmt_int(e, act)?,
                    None => 1,
                };
                let st = &mut self.files[f as usize];
                st.rec += n.max(1) as usize;
                st.pos = 0;
            }
            Fmt::Column(n) => {
                let n = self.fmt_int(n, act)?.max(1) as usize;
                let st = &mut self.files[f as usize];
                if st.pos + 1 > n {
                    st.rec += 1;
                }
                st.pos = n - 1;
            }
            Fmt::Page | Fmt::Line(_) => {}
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn get_edit_item(&mut self, f: FileId, fm: &Fmt, t: &DataItem, act: usize) -> R<()> {
        let width = |me: &mut Machine, w: &Option<Expr>| -> R<usize> {
            Ok(match w {
                Some(w) => me.fmt_int(w, act)?.max(0) as usize,
                None => {
                    // rest of the record
                    let st = &me.files[f as usize];
                    st.inrecs
                        .get(st.rec)
                        .map_or(0, |r| r.len().saturating_sub(st.pos))
                }
            })
        };
        let (field, vt): (Value, Ty) = match fm {
            Fmt::A(w) => {
                let w = width(self, w)?;
                (Value::Char(self.take_chars(f, w)?), Ty::char(w as u32))
            }
            Fmt::B(w) => {
                let w = width(self, w)?;
                let s = self.take_chars(f, w)?;
                let t = host(&s);
                let t = t.trim();
                if !t.chars().all(|c| c == '0' || c == '1') {
                    self.raise(Cond::Conversion, 610, act)?;
                }
                (
                    Value::Bit(t.chars().map(|c| c == '1').collect()),
                    crate::sema::expr::bit_any(),
                )
            }
            Fmt::F(w, d, sc) => {
                let w = self.fmt_int(w, act)?.max(0) as usize;
                let d = match d {
                    Some(d) => self.fmt_int(d, act)?,
                    None => 0,
                };
                let s = self.take_chars(f, w)?;
                let text = host(&s);
                let mut v = match value::parse_number(&text) {
                    Some(v) => v,
                    None => {
                        self.raise(Cond::Conversion, 616, act)?;
                        Value::fixed(0, 0, true)
                    }
                };
                if !text.contains('.')
                    && let Value::Fixed { m, dec: true, .. } = v
                {
                    v = Value::fixed(m, d as i16, true);
                }
                if let Some(sc) = sc {
                    let sc = self.fmt_int(sc, act)?;
                    if let Value::Fixed { m, q, dec } = v {
                        v = Value::fixed(m, q - sc as i16, dec);
                    }
                }
                let ty = match &v {
                    Value::Fixed { q, .. } => Ty::fixed_dec(31, *q),
                    _ => Ty::Float { dec: true, p: 16 },
                };
                (v, ty)
            }
            Fmt::E(w, ..) => {
                let w = self.fmt_int(w, act)?.max(0) as usize;
                let s = self.take_chars(f, w)?;
                let text = host(&s);
                let v = match value::parse_number(&text) {
                    Some(v) if !text.trim().is_empty() => v,
                    _ => {
                        self.raise(Cond::Conversion, 617, act)?;
                        Value::Float(0.0)
                    }
                };
                (Value::Float(v.as_f64()), Ty::Float { dec: true, p: 16 })
            }
            Fmt::P(p) => {
                let s = self.take_chars(f, p.len)?;
                (Value::Char(s), Ty::Pic(p.clone()))
            }
            _ => return Ok(()),
        };
        match t {
            DataItem::Target(target, _) => {
                let l = self.target_loc(target, act)?;
                self.assign_target(target, l, field, &vt, act)
            }
            _ => err("GET EDIT into an aggregate is not supported"),
        }
    }

    fn get_data(&mut self, f: FileId, items: &[DataItem], act: usize) -> R<()> {
        // name=value pairs up to ';'
        let mut text = Vec::new();
        let semi = ebcdic(";")[0];
        let quote = ebcdic("'")[0];
        let mut in_quote = false;
        loop {
            let st = &mut self.files[f as usize];
            let Some(rec) = st.inrecs.get(st.rec).cloned() else {
                if text.is_empty() {
                    return Err(Intr::Fatal("ENDFILE".into()));
                }
                break;
            };
            if st.pos >= rec.len() {
                st.rec += 1;
                st.pos = 0;
                text.push(SPACE);
                continue;
            }
            let c = rec[st.pos];
            st.pos += 1;
            if c == quote {
                in_quote = !in_quote;
            }
            if c == semi && !in_quote {
                break;
            }
            text.push(c);
        }
        let s = host(&text);
        for part in split_data(&s) {
            let Some((name, val)) = part.split_once('=') else {
                continue;
            };
            let name = name.trim().to_ascii_uppercase();
            let target = items.iter().find_map(|it| match it {
                DataItem::Named(n, x) if n.to_ascii_uppercase() == name => Some(x.clone()),
                _ => None,
            });
            match target {
                Some(t) => {
                    if let DataItem::Target(tg, _) = &*t {
                        let (v, vt) = list_value(&ebcdic(val.trim()));
                        let l = self.target_loc(tg, act)?;
                        self.assign_target(tg, l, v, &vt, act)?;
                    }
                }
                None => {
                    self.raise(Cond::Name(f), 10, act)?;
                }
            }
        }
        Ok(())
    }
}

fn split_data(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut q = false;
    for c in s.chars() {
        if c == '\'' {
            q = !q;
        }
        if !q && (c == ',' || c == ' ') {
            if cur.contains('=') && !cur.trim_end().ends_with('=') {
                out.push(std::mem::take(&mut cur));
            } else if !cur.is_empty() && c == ' ' {
                // blanks around '='
            }
            continue;
        }
        cur.push(c);
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

/// The value of a list-directed input field: a quoted string, a bit
/// string, a number, or unquoted characters.
fn list_value(field: &[u8]) -> (Value, Ty) {
    let t = host(field);
    if t.starts_with('\'') {
        let close = t.rfind('\'').unwrap_or(0);
        let body = t[1..close.max(1)].replace("''", "'");
        let suffix = t[close + 1..].to_ascii_uppercase();
        if suffix == "B" {
            let bits: Vec<bool> = body.chars().map(|c| c == '1').collect();
            return (Value::Bit(bits), crate::sema::expr::bit_any());
        }
        return (Value::Char(ebcdic(&body)), crate::sema::expr::char_any());
    }
    match value::parse_number(&t) {
        Some(v) => {
            let ty = match &v {
                Value::Fixed { q, dec: true, .. } => Ty::fixed_dec(31, *q),
                Value::Fixed { q, .. } => Ty::fixed_bin(63, *q),
                _ => Ty::Float { dec: true, p: 16 },
            };
            (v, ty)
        }
        None => (Value::Char(field.to_vec()), crate::sema::expr::char_any()),
    }
}

/// F(w,d) text of a FIXED DECIMAL value, rounded at d decimals.
fn fixed_f_text(m: i128, q: i16, d: u32) -> String {
    let neg = m < 0;
    let a = m.abs();
    let a = if i32::from(q) > d as i32 {
        let drop = (i32::from(q) - d as i32) as u32;
        let p = 10i128.pow(drop);
        (a + p / 2) / p
    } else {
        a * 10i128.pow((d as i32 - i32::from(q)) as u32)
    };
    let s = a.to_string();
    let body = if d == 0 {
        s
    } else {
        let s = format!("{s:0>w$}", w = d as usize + 1);
        let (i, f) = s.split_at(s.len() - d as usize);
        format!("{i}.{f}")
    };
    if neg && a != 0 {
        format!("-{body}")
    } else {
        body
    }
}

fn float_f_text(f: f64, d: u32) -> String {
    let s = format!("{:.*}", d as usize, f.abs());
    let zero = s.chars().all(|c| c == '0' || c == '.');
    if f < 0.0 && !zero { format!("-{s}") } else { s }
}

/// E(w,d,s) text: s significant digits, d after the point, 2-digit
/// exponent.
fn e_text(f: f64, d: usize, s: usize) -> String {
    let int_digits = s.saturating_sub(d).max(if d == s { 0 } else { 1 });
    if f == 0.0 {
        let mant = if d == 0 {
            "0".to_string()
        } else {
            format!("{}.{}", "0".repeat(int_digits.max(1)), "0".repeat(d))
        };
        return format!("{mant}E+00");
    }
    let neg = f < 0.0;
    let a = f.abs();
    // exponent so that the mantissa has int_digits integer digits
    let mut e = a.log10().floor() as i32 - (int_digits as i32 - 1);
    let mut mant = a / 10f64.powi(e);
    let scale = 10f64.powi(d as i32);
    mant = (mant * scale).round() / scale;
    if mant >= 10f64.powi(int_digits as i32) {
        mant /= 10.0;
        e += 1;
    }
    let m = if d == 0 {
        format!("{mant:.0}")
    } else {
        format!("{mant:.d$}")
    };
    format!(
        "{}{m}E{}{:02}",
        if neg { "-" } else { "" },
        if e < 0 { '-' } else { '+' },
        e.abs()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(fixed_f_text(1234, 2, 2), "12.34");
        assert_eq!(fixed_f_text(-15, 1, 2), "-1.50");
        assert_eq!(fixed_f_text(5, 1, 2), "0.50");
        assert_eq!(fixed_f_text(1237, 1, 0), "124");
        assert_eq!(e_text(12.5, 3, 4), "1.250E+01");
        assert_eq!(e_text(-0.00125, 3, 4), "-1.250E-03");
    }
}
