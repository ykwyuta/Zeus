//! INSPECT, STRING and UNSTRING.

use zeus_decimal::Dec;

use super::eval::{Val, move_into};
use super::{Machine, RResult};
use crate::zir::{DataType, Ex, InspectKind, InspectStep, Loc, Op, Src};

/// Region of the inspected data delimited by BEFORE/AFTER INITIAL.
fn region(data: &[u8], before: Option<&[u8]>, after: Option<&[u8]>) -> (usize, usize) {
    let find = |p: &[u8]| -> Option<usize> {
        if p.is_empty() || p.len() > data.len() {
            return None;
        }
        (0..=data.len() - p.len()).find(|&i| &data[i..i + p.len()] == p)
    };
    let start = match after {
        Some(p) => match find(p) {
            Some(i) => i + p.len(),
            None => data.len(),
        },
        None => 0,
    };
    let end = match before {
        Some(p) => find(p).unwrap_or(data.len()),
        None => data.len(),
    };
    (start, end.max(start).min(data.len()))
}

impl Machine {
    fn operand_bytes(&mut self, cur: usize, s: &Src, len: usize) -> RResult<Vec<u8>> {
        let v = self.eval_src(cur, s)?;
        self.alnum_bytes(cur, &v, len)
    }

    fn add_to(&mut self, cur: usize, loc: &Loc, n: i64) -> RResult<()> {
        if n == 0 {
            return Ok(());
        }
        let v = Ex::Bin(
            Op::Add,
            Box::new(Ex::Load(loc.clone())),
            Box::new(Ex::Lit(Dec::from_i64(n))),
        );
        let x = self.eval_ex(cur, &v)?;
        self.store(cur, x, loc, false, false)
    }

    pub(super) fn inspect(
        &mut self,
        cur: usize,
        target: &Loc,
        steps: &[InspectStep],
    ) -> RResult<()> {
        let (a, l) = self.addr(cur, target)?;
        let ty = self.instances[cur].code.descs[target.desc as usize]
            .ty
            .clone();
        // A signed numeric display item is inspected as if unsigned: the
        // operational sign (zone of the sign digit) is set aside.
        let mut data = self.mem[a..a + l].to_vec();
        let sign_at = match &ty {
            DataType::Numeric(t)
                if target.refmod.is_none()
                    && t.signed
                    && t.usage == crate::zir::NumUsage::Display
                    && l > 0 =>
            {
                match t.sign {
                    crate::zir::SignPos::None | crate::zir::SignPos::Trailing => Some(l - 1),
                    crate::zir::SignPos::Leading => Some(0),
                    _ => None,
                }
            }
            _ => None,
        };
        let saved_zone = sign_at.map(|i| {
            let z = data[i] & 0xF0;
            data[i] = 0xF0 | (data[i] & 0x0F);
            z
        });
        let counts = self.inspect_data(cur, &mut data, steps)?;
        if let (Some(i), Some(z)) = (sign_at, saved_zone)
            && (0xF0..=0xF9).contains(&data[i])
        {
            data[i] = z | (data[i] & 0x0F);
        }
        self.mem[a..a + l].copy_from_slice(&data);
        for (c, n) in counts {
            self.add_to(cur, &c, n)?;
        }
        Ok(())
    }

    /// INSPECT ... TALLYING of a value that is not a data item (a function
    /// result).
    pub(super) fn inspect_value(
        &mut self,
        cur: usize,
        value: &Src,
        steps: &[InspectStep],
    ) -> RResult<()> {
        let mut data = self.operand_bytes(cur, value, 0)?;
        for (c, n) in self.inspect_data(cur, &mut data, steps)? {
            self.add_to(cur, &c, n)?;
        }
        Ok(())
    }

    /// Applies the INSPECT steps to `data`; returns the TALLYING counters
    /// and their increments.
    fn inspect_data(
        &mut self,
        cur: usize,
        data: &mut [u8],
        steps: &[InspectStep],
    ) -> RResult<Vec<(Loc, i64)>> {
        // Resolve the operands of each step.
        struct S {
            kind: u8, // 0 chars, 1 all, 2 leading, 3 first, 4 convert
            pat: Vec<u8>,
            by: Vec<u8>,
            from: Vec<u8>,
            to: Vec<u8>,
            start: usize,
            end: usize,
            counter: Option<Loc>,
            count: i64,
            lead_next: usize,
            lead_active: bool,
            done: bool,
        }
        let mut ss = Vec::new();
        for step in steps {
            let (kind, before, after) = match step {
                InspectStep::Tally {
                    kind,
                    before,
                    after,
                    ..
                }
                | InspectStep::Replace {
                    kind,
                    before,
                    after,
                    ..
                } => (Some(kind), before, after),
                InspectStep::Convert { before, after, .. } => (None, before, after),
            };
            let before_b = match before {
                Some(s) => Some(self.operand_bytes(cur, s, 1)?),
                None => None,
            };
            let after_b = match after {
                Some(s) => Some(self.operand_bytes(cur, s, 1)?),
                None => None,
            };
            let (start, end) = region(data, before_b.as_deref(), after_b.as_deref());
            let (k, pat) = match kind {
                Some(InspectKind::Characters) => (0, vec![]),
                Some(InspectKind::All(p)) => (1, self.operand_bytes(cur, p, 1)?),
                Some(InspectKind::Leading(p)) => (2, self.operand_bytes(cur, p, 1)?),
                Some(InspectKind::First(p)) => (3, self.operand_bytes(cur, p, 1)?),
                None => (4, vec![]),
            };
            let mut s = S {
                kind: k,
                pat: pat.clone(),
                by: Vec::new(),
                from: Vec::new(),
                to: Vec::new(),
                start,
                end,
                counter: None,
                count: 0,
                lead_next: start,
                lead_active: true,
                done: false,
            };
            match step {
                InspectStep::Tally { counter, .. } => s.counter = Some(counter.clone()),
                InspectStep::Replace { by, .. } => {
                    let n = if k == 0 { 1 } else { pat.len() };
                    s.by = self.operand_bytes(cur, by, n)?;
                }
                InspectStep::Convert { from, to, .. } => {
                    s.from = self.operand_bytes(cur, from, 1)?;
                    let n = s.from.len();
                    s.to = self.operand_bytes(cur, to, n)?;
                }
            }
            ss.push(s);
        }

        if ss.iter().all(|s| s.kind == 4) {
            for s in &ss {
                for b in &mut data[s.start..s.end] {
                    if let Some(i) = s.from.iter().position(|x| x == b) {
                        *b = s.to.get(i).copied().unwrap_or(*b);
                    }
                }
            }
        } else {
            let original = data.to_vec();
            let mut pos = 0;
            while pos < original.len() {
                let mut advanced = 0;
                for s in ss.iter_mut() {
                    if pos < s.start || pos >= s.end {
                        continue;
                    }
                    let fits = |p: &[u8]| {
                        !p.is_empty()
                            && pos + p.len() <= s.end
                            && original[pos..pos + p.len()] == *p
                    };
                    let hit = match s.kind {
                        0 => Some(1),
                        1 => fits(&s.pat).then_some(s.pat.len()),
                        2 => (s.lead_active && pos == s.lead_next && fits(&s.pat))
                            .then_some(s.pat.len()),
                        3 => (!s.done && fits(&s.pat)).then_some(s.pat.len()),
                        _ => None,
                    };
                    if let Some(n) = hit {
                        s.count += 1;
                        if s.kind == 2 {
                            s.lead_next = pos + n;
                        }
                        if s.kind == 3 {
                            s.done = true;
                        }
                        if !s.by.is_empty() || s.counter.is_none() {
                            for k in 0..n {
                                data[pos + k] = s.by.get(k).copied().unwrap_or(s.by[0]);
                            }
                        }
                        advanced = n;
                        break;
                    }
                }
                // LEADING stops at the first position it did not match.
                for s in ss.iter_mut() {
                    if s.kind == 2 && s.lead_active && pos >= s.lead_next && pos >= s.start {
                        s.lead_active = false;
                    }
                }
                pos += advanced.max(1);
                for s in ss.iter_mut() {
                    if s.kind == 2 && s.lead_active && pos > s.lead_next {
                        s.lead_active = false;
                    }
                }
            }
        }
        Ok(ss
            .into_iter()
            .filter_map(|s| s.counter.map(|c| (c, s.count)))
            .collect())
    }

    fn pointer_value(&mut self, cur: usize, p: Option<&Loc>) -> RResult<i64> {
        match p {
            Some(p) => {
                let v = self.eval_ex(cur, &Ex::Load(p.clone()))?;
                Ok(super::data::to_raw(v, 0, false) as i64)
            }
            None => Ok(1),
        }
    }

    pub(super) fn string_stmt(
        &mut self,
        cur: usize,
        sources: &[(Src, Option<Src>)],
        into: &Loc,
        pointer: Option<&Loc>,
    ) -> RResult<()> {
        let (a, l) = self.addr(cur, into)?;
        let mut ptr = self.pointer_value(cur, pointer)?;
        self.overflow = false;
        if ptr < 1 || ptr > l as i64 {
            self.overflow = true;
            return Ok(());
        }
        'outer: for (src, delim) in sources {
            let v = self.eval_src(cur, src)?;
            let bytes = match &v {
                Val::Item(_, b) => b.clone(),
                other => self.alnum_bytes(cur, other, 1)?,
            };
            let part: &[u8] = match delim {
                Some(d) => {
                    let db = self.operand_bytes(cur, d, 1)?;
                    let end = if db.is_empty() || db.len() > bytes.len() {
                        bytes.len()
                    } else {
                        (0..=bytes.len() - db.len())
                            .find(|&i| bytes[i..i + db.len()] == *db)
                            .unwrap_or(bytes.len())
                    };
                    &bytes[..end]
                }
                None => &bytes,
            };
            for &b in part {
                if ptr > l as i64 {
                    self.overflow = true;
                    break 'outer;
                }
                self.mem[a + ptr as usize - 1] = b;
                ptr += 1;
            }
        }
        if let Some(p) = pointer {
            self.set_value(cur, p, Dec::from_i64(ptr))?;
        }
        Ok(())
    }

    pub(super) fn unstring(
        &mut self,
        cur: usize,
        source: &Loc,
        delims: &[(bool, Src)],
        into: &[(Loc, Option<Loc>, Option<Loc>)],
        pointer: Option<&Loc>,
        tallying: Option<&Loc>,
    ) -> RResult<()> {
        let (a, l) = self.addr(cur, source)?;
        let data = self.mem[a..a + l].to_vec();
        let mut ptr = self.pointer_value(cur, pointer)?;
        self.overflow = false;
        if ptr < 1 || ptr > l as i64 {
            self.overflow = true;
            return Ok(());
        }
        let mut dl = Vec::new();
        for (all, d) in delims {
            dl.push((*all, self.operand_bytes(cur, d, 1)?));
        }
        let mut fields = 0i64;
        let mut p = ptr as usize - 1;
        for (target, delim_in, count_in) in into {
            if p >= data.len() {
                break;
            }
            let (field_end, delim): (usize, Option<(usize, usize)>) = if dl.is_empty() {
                let (_, tl) = self.addr(cur, target)?;
                let ty = self.instances[cur].code.descs[target.desc as usize]
                    .ty
                    .clone();
                let n = match &ty {
                    DataType::Numeric(t) => t.digits as usize,
                    _ => tl,
                };
                ((p + n).min(data.len()), None)
            } else {
                let mut found = None;
                'scan: for i in p..data.len() {
                    for (k, (_, d)) in dl.iter().enumerate() {
                        if !d.is_empty() && i + d.len() <= data.len() && data[i..i + d.len()] == **d
                        {
                            found = Some((i, k));
                            break 'scan;
                        }
                    }
                }
                match found {
                    Some((i, k)) => (i, Some((i, k))),
                    None => (data.len(), None),
                }
            };
            let part = data[p..field_end].to_vec();
            let ty = self.instances[cur].code.descs[target.desc as usize]
                .ty
                .clone();
            let (ta, tl) = self.addr(cur, target)?;
            let mut out = self.mem[ta..ta + tl].to_vec();
            let v = match &ty {
                DataType::Numeric(_) => Val::Alnum(part.clone()),
                _ => Val::Alnum(part.clone()),
            };
            move_into(
                &v,
                if target.refmod.is_some() {
                    &DataType::Alphanumeric { just: false }
                } else {
                    &ty
                },
                &mut out,
            );
            self.mem[ta..ta + tl].copy_from_slice(&out);
            fields += 1;
            if let Some(c) = count_in {
                self.set_value(cur, c, Dec::from_i64(part.len() as i64))?;
            }
            p = field_end;
            match delim {
                Some((i, k)) => {
                    let (all, d) = &dl[k];
                    let mut end = i + d.len();
                    if *all {
                        while end + d.len() <= data.len() && data[end..end + d.len()] == **d {
                            end += d.len();
                        }
                    }
                    if let Some(dloc) = delim_in {
                        self.move_val(cur, &Val::Alnum(d.clone()), dloc)?;
                    }
                    p = end;
                }
                None => {
                    if let Some(dloc) = delim_in {
                        self.move_val(cur, &Val::Fig(crate::zir::Fig::Space), dloc)?;
                    }
                }
            }
        }
        if p < data.len() {
            self.overflow = true;
        }
        ptr = p as i64 + 1;
        if let Some(pl) = pointer {
            self.set_value(cur, pl, Dec::from_i64(ptr))?;
        }
        if let Some(t) = tallying {
            self.add_to(cur, t, fields)?;
        }
        Ok(())
    }
}
