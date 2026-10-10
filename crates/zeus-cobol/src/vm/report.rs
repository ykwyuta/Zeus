//! Report Writer: INITIATE, GENERATE and TERMINATE.
//!
//! A report is presented group by group. Each group is laid out on the
//! page following its LINE clauses and the regions of the PAGE clause
//! (heading, first detail, last detail, footing, page limit); a body group
//! (control heading, detail, control footing) that does not fit causes a
//! page advance with the page footing and page heading. Control breaks are
//! detected on GENERATE by comparing the control data items with their
//! values at the previous GENERATE; the footings are produced with the
//! previous values in place.

use zeus_decimal::Dec;

use super::{Machine, RResult};
use crate::ebcdic;
use crate::zir::{FieldValue, GroupKind, LinePos, Loc, ReportDef, SumOperand};

/// Run-time state of one report.
#[derive(Debug, Clone, Default)]
pub(super) struct ReportState {
    active: bool,
    /// A GENERATE was executed since INITIATE.
    generated: bool,
    /// LINE-COUNTER and PAGE-COUNTER.
    line: u32,
    page: u32,
    /// Lines written on the current physical page.
    phys: u32,
    /// A body group was presented on the current page.
    body_started: bool,
    /// NEXT GROUP NEXT PAGE was given by the last body group.
    next_page: bool,
    sums: Vec<Dec>,
    /// Control values at the last GENERATE.
    controls: Vec<Vec<u8>>,
    /// GROUP INDICATE items are printed by the next detail.
    indicate: bool,
}

fn loc(desc: u32) -> Loc {
    Loc {
        desc,
        subs: Vec::new(),
        refmod: None,
    }
}

impl Machine {
    fn report_def(&self, cur: usize, r: u32) -> ReportDef {
        self.instances[cur].code.reports[r as usize].clone()
    }

    fn rstate(&mut self, cur: usize, r: u32) -> &mut ReportState {
        &mut self.instances[cur].reports[r as usize]
    }

    /// Updates LINE-COUNTER and PAGE-COUNTER.
    fn report_counters(&mut self, cur: usize, def: &ReportDef, r: u32) -> RResult<()> {
        let (l, p) = {
            let st = self.rstate(cur, r);
            (st.line, st.page)
        };
        self.set_value(cur, &loc(def.line_counter), Dec::from_i64(l as i64))?;
        self.set_value(cur, &loc(def.page_counter), Dec::from_i64(p as i64))
    }

    pub(super) fn report_initiate(&mut self, cur: usize, r: u32) -> RResult<()> {
        let def = self.report_def(cur, r);
        *self.rstate(cur, r) = ReportState {
            active: true,
            page: 1,
            sums: vec![Dec::ZERO; def.sums.len()],
            indicate: true,
            ..ReportState::default()
        };
        self.report_counters(cur, &def, r)
    }

    fn control_values(&mut self, cur: usize, def: &ReportDef) -> RResult<Vec<Vec<u8>>> {
        let mut out = Vec::new();
        for c in &def.controls {
            out.push(match c {
                Some(l) => {
                    let (a, n) = self.addr(cur, l)?;
                    self.mem[a..a + n].to_vec()
                }
                None => Vec::new(),
            });
        }
        Ok(out)
    }

    fn set_control_values(&mut self, cur: usize, def: &ReportDef, vals: &[Vec<u8>]) -> RResult<()> {
        for (c, v) in def.controls.iter().zip(vals) {
            if let Some(l) = c {
                let (a, n) = self.addr(cur, l)?;
                let k = n.min(v.len());
                self.mem[a..a + k].copy_from_slice(&v[..k]);
            }
        }
        Ok(())
    }

    pub(super) fn report_generate(
        &mut self,
        cur: usize,
        r: u32,
        group: Option<u32>,
    ) -> RResult<()> {
        let def = self.report_def(cur, r);
        if !self.rstate(cur, r).active {
            return super::err(format!("GENERATE for report {} before INITIATE", def.name));
        }
        if !self.rstate(cur, r).generated {
            self.rstate(cur, r).generated = true;
            self.present_kind(cur, r, &def, GroupKind::ReportHeading, None)?;
            self.present_kind(cur, r, &def, GroupKind::PageHeading, None)?;
            for level in 0..def.controls.len() as u32 {
                self.present_kind(cur, r, &def, GroupKind::ControlHeading, Some(level))?;
            }
            let vals = self.control_values(cur, &def)?;
            self.rstate(cur, r).controls = vals;
        } else {
            let now = self.control_values(cur, &def)?;
            let saved = self.rstate(cur, r).controls.clone();
            let brk =
                (0..def.controls.len()).find(|&k| def.controls[k].is_some() && now[k] != saved[k]);
            if let Some(k) = brk {
                // control footings see the previous control values
                self.set_control_values(cur, &def, &saved)?;
                for level in (k..def.controls.len()).rev() {
                    self.control_footing(cur, r, &def, level as u32)?;
                }
                self.set_control_values(cur, &def, &now)?;
                for level in k..def.controls.len() {
                    self.present_kind(cur, r, &def, GroupKind::ControlHeading, Some(level as u32))?;
                }
                let st = self.rstate(cur, r);
                st.controls = now;
                st.indicate = true;
            }
        }
        // subtotalling
        for (k, s) in def.sums.iter().enumerate() {
            if !(s.upon.is_empty() || group.is_some_and(|g| s.upon.contains(&g))) {
                continue;
            }
            for op in &s.operands {
                if let SumOperand::Data(ex) = op {
                    let v = self.eval_ex(cur, ex)?;
                    let st = self.rstate(cur, r);
                    st.sums[k] = st.sums[k].checked_add(v).unwrap_or(st.sums[k]);
                }
            }
        }
        if let Some(g) = group {
            self.present(cur, r, &def, g)?;
            self.rstate(cur, r).indicate = false;
        }
        Ok(())
    }

    pub(super) fn report_terminate(&mut self, cur: usize, r: u32) -> RResult<()> {
        let def = self.report_def(cur, r);
        if !self.rstate(cur, r).active {
            return Ok(());
        }
        if self.rstate(cur, r).generated {
            let now = self.control_values(cur, &def)?;
            let saved = self.rstate(cur, r).controls.clone();
            self.set_control_values(cur, &def, &saved)?;
            for level in (0..def.controls.len()).rev() {
                self.control_footing(cur, r, &def, level as u32)?;
            }
            self.set_control_values(cur, &def, &now)?;
            self.present_kind(cur, r, &def, GroupKind::PageFooting, None)?;
            self.present_kind(cur, r, &def, GroupKind::ReportFooting, None)?;
        }
        self.rstate(cur, r).active = false;
        Ok(())
    }

    /// Control footing of `level`: present the CF groups, roll their sum
    /// counters forward, reset the counters of the level.
    fn control_footing(&mut self, cur: usize, r: u32, def: &ReportDef, level: u32) -> RResult<()> {
        let groups: Vec<u32> = (0..def.groups.len() as u32)
            .filter(|&g| {
                let gd = &def.groups[g as usize];
                gd.kind == GroupKind::ControlFooting && gd.control == Some(level)
            })
            .collect();
        for &g in &groups {
            self.present(cur, r, def, g)?;
        }
        for (k, s) in def.sums.iter().enumerate() {
            for op in &s.operands {
                if let SumOperand::Sum(t) = op
                    && groups.contains(&def.sums[*t as usize].group)
                {
                    let st = self.rstate(cur, r);
                    let add = st.sums[*t as usize];
                    st.sums[k] = st.sums[k].checked_add(add).unwrap_or(st.sums[k]);
                }
            }
        }
        for (k, s) in def.sums.iter().enumerate() {
            let own = def.groups[s.group as usize].control;
            if s.reset.or(own) == Some(level) {
                self.rstate(cur, r).sums[k] = Dec::ZERO;
            }
        }
        Ok(())
    }

    fn present_kind(
        &mut self,
        cur: usize,
        r: u32,
        def: &ReportDef,
        kind: GroupKind,
        control: Option<u32>,
    ) -> RResult<()> {
        for g in 0..def.groups.len() as u32 {
            let gd = &def.groups[g as usize];
            if gd.kind == kind && (control.is_none() || gd.control == control) {
                self.present(cur, r, def, g)?;
            }
        }
        Ok(())
    }

    /// Writes one print line at line `line` of the current page.
    fn report_line(&mut self, cur: usize, r: u32, def: &ReportDef, line: u32, text: &[u8]) {
        let st = self.rstate(cur, r);
        let blank = line.saturating_sub(st.phys + 1);
        st.phys = st.phys.max(line.saturating_sub(1)) + 1;
        let mut out = vec![b'\n'; blank as usize];
        if let Some(c) = &def.code {
            out.extend(ebcdic::decode_latin1(c));
        }
        let mut t = ebcdic::decode_latin1(text);
        while t.last() == Some(&b' ') {
            t.pop();
        }
        out.extend(t);
        out.push(b'\n');
        self.report_out(cur, def.file, &out);
    }

    fn report_out(&mut self, cur: usize, f: u32, bytes: &[u8]) {
        let fs = self.state(cur, f);
        fs.text.extend_from_slice(bytes);
        fs.modified = true;
        fs.written = true;
    }

    /// Ends the physical page.
    fn report_eject(&mut self, cur: usize, r: u32, def: &ReportDef) {
        let phys = self.rstate(cur, r).phys;
        let out = match def.page {
            Some(p) => vec![b'\n'; p.limit.saturating_sub(phys) as usize],
            None => vec![0x0C],
        };
        self.report_out(cur, def.file, &out);
        self.rstate(cur, r).phys = 0;
    }

    /// Page advance: page footing, new page, page heading.
    fn report_page_advance(&mut self, cur: usize, r: u32, def: &ReportDef) -> RResult<()> {
        self.present_kind(cur, r, def, GroupKind::PageFooting, None)?;
        self.report_eject(cur, r, def);
        {
            let st = self.rstate(cur, r);
            st.page += 1;
            st.line = 0;
            st.body_started = false;
            st.indicate = true;
        }
        self.report_counters(cur, def, r)?;
        self.present_kind(cur, r, def, GroupKind::PageHeading, None)
    }

    /// Line numbers of the lines of group `g` when its first line goes
    /// after line `base` (relative) or at its absolute position.
    fn group_lines(def: &ReportDef, g: u32, first: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let mut prev = first;
        for (i, l) in def.groups[g as usize].lines.iter().enumerate() {
            let n = if i == 0 {
                first
            } else {
                match l.pos {
                    LinePos::Absolute(n) | LinePos::AbsoluteNextPage(n) => n,
                    LinePos::Plus(n) => prev + n,
                    LinePos::NextPage => prev + 1,
                }
            };
            out.push(n);
            prev = n;
        }
        out
    }

    /// Presents report group `g`.
    fn present(&mut self, cur: usize, r: u32, def: &ReportDef, g: u32) -> RResult<()> {
        let gd = def.groups[g as usize].clone();
        if let Some((start, end)) = gd.use_proc {
            self.suppress_printing = false;
            let pc = self.instances[cur].code.paragraphs[start as usize].start;
            self.run_procedure(cur, pc, end, &mut Vec::new())?;
            if std::mem::take(&mut self.suppress_printing) {
                return Ok(());
            }
        }
        let body = matches!(
            gd.kind,
            GroupKind::ControlHeading | GroupKind::Detail | GroupKind::ControlFooting
        );
        if body && self.rstate(cur, r).next_page {
            self.rstate(cur, r).next_page = false;
            self.report_page_advance(cur, r, def)?;
        }
        // placement
        let mut lines = Vec::new();
        if !gd.lines.is_empty() {
            for attempt in 0..2 {
                let st = self.rstate(cur, r).clone();
                let first_pos = gd.lines[0].pos;
                let mut advance = false;
                let first = match (def.page, gd.kind) {
                    (None, _) => match first_pos {
                        LinePos::Absolute(n) if n > st.line => n,
                        LinePos::Plus(n) => st.line + n,
                        _ => st.line + 1,
                    },
                    (Some(p), GroupKind::ReportHeading | GroupKind::PageHeading) => match first_pos
                    {
                        LinePos::Plus(n) => st.line.max(p.heading - 1) + n,
                        LinePos::Absolute(n) | LinePos::AbsoluteNextPage(n) => n,
                        LinePos::NextPage => p.heading,
                    },
                    (Some(p), GroupKind::PageFooting) => match first_pos {
                        LinePos::Plus(n) => st.line.max(p.footing) + n,
                        LinePos::Absolute(n) | LinePos::AbsoluteNextPage(n) => n,
                        LinePos::NextPage => p.footing + 1,
                    },
                    (Some(p), GroupKind::ReportFooting) => match first_pos {
                        LinePos::Plus(n) => st.line + n,
                        LinePos::Absolute(n) => {
                            advance = n <= st.line;
                            n
                        }
                        LinePos::AbsoluteNextPage(n) => {
                            advance = true;
                            n
                        }
                        LinePos::NextPage => {
                            advance = true;
                            p.heading
                        }
                    },
                    (Some(p), _) => match first_pos {
                        LinePos::Plus(n) => {
                            if st.line < p.first_detail {
                                p.first_detail
                            } else {
                                st.line + n
                            }
                        }
                        LinePos::Absolute(n) => {
                            advance = n <= st.line;
                            n
                        }
                        LinePos::AbsoluteNextPage(n) => {
                            advance = attempt == 0;
                            n
                        }
                        LinePos::NextPage => {
                            advance = attempt == 0;
                            p.first_detail
                        }
                    },
                };
                lines = Self::group_lines(def, g, first);
                if let (Some(p), true) = (def.page, body) {
                    let limit = if gd.kind == GroupKind::ControlFooting {
                        p.footing
                    } else {
                        p.last_detail
                    };
                    if lines.last().is_some_and(|&l| l > limit) {
                        advance = true;
                    }
                }
                if advance && attempt == 0 {
                    if gd.kind == GroupKind::ReportFooting {
                        self.report_eject(cur, r, def);
                        let st = self.rstate(cur, r);
                        st.page += 1;
                        st.line = 0;
                        self.report_counters(cur, def, r)?;
                    } else {
                        self.report_page_advance(cur, r, def)?;
                    }
                    continue;
                }
                break;
            }
            let indicate = self.rstate(cur, r).indicate;
            for (line, &n) in gd.lines.iter().zip(&lines) {
                // LINE-COUNTER is the number of the line being presented
                self.rstate(cur, r).line = n;
                self.report_counters(cur, def, r)?;
                for f in &line.fields {
                    match &f.value {
                        FieldValue::Constant => {}
                        FieldValue::Source(src) => {
                            let v = self.eval_src(cur, src)?;
                            self.move_val(cur, &v, &loc(f.desc))?;
                        }
                        FieldValue::Sum(k) => {
                            let v = self.rstate(cur, r).sums[*k as usize];
                            self.store(cur, v, &loc(f.desc), false, false)?;
                        }
                    }
                }
                let width = line
                    .fields
                    .iter()
                    .filter_map(|f| {
                        let len = self.instances[cur].code.descs[f.desc as usize].len;
                        f.column.map(|c| (c + len - 1) as usize)
                    })
                    .max()
                    .unwrap_or(0);
                let mut buf = vec![ebcdic::SPACE; width];
                for f in &line.fields {
                    let Some(c) = f.column else { continue };
                    if f.group_indicate && !indicate {
                        continue;
                    }
                    let (a, len) = self.addr(cur, &loc(f.desc))?;
                    let at = c as usize - 1;
                    buf[at..at + len].copy_from_slice(&self.mem[a..a + len]);
                }
                self.report_line(cur, r, def, n, &buf);
                self.rstate(cur, r).line = n;
            }
        }
        {
            let st = self.rstate(cur, r);
            if body {
                st.body_started = true;
            }
            match gd.next_group {
                Some(LinePos::Absolute(n)) => st.line = st.line.max(n),
                Some(LinePos::Plus(n)) => st.line += n,
                // a report heading ejects its own page below
                Some(LinePos::NextPage) | Some(LinePos::AbsoluteNextPage(_)) => {
                    st.next_page = gd.kind != GroupKind::ReportHeading;
                }
                None => {}
            }
        }
        if gd.kind == GroupKind::ReportHeading && matches!(gd.next_group, Some(LinePos::NextPage)) {
            self.report_eject(cur, r, def);
            let st = self.rstate(cur, r);
            st.page += 1;
            st.line = 0;
        }
        self.report_counters(cur, def, r)
    }
}
