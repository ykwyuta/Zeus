//! COBOL sort interface controls. Scratch runs use host temporary files;
//! they are private to an operation and removed on all exit paths.
use std::cmp::Ordering;
use std::io::{Read, Seek, Write};

use super::{Machine, RResult, RunError, err};
use crate::zir::Loc;

const DEFAULT_MEMORY: usize = 64 * 1024 * 1024;
pub(super) const CANCELLED: &str = "internal SORT cancellation";

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct Controls {
    #[serde(default)]
    pub input_runs: Vec<usize>,
    #[serde(default)]
    pub file_slot: Option<usize>,
    #[serde(default = "input_phase_default")]
    pub input_phase: bool,
    #[serde(default)]
    maximum_memory: bool,
    #[serde(default)]
    reserved_memory: usize,
    #[serde(default)]
    pub exact_count: Option<u128>,
    #[serde(default)]
    pub checkpoint: bool,
    #[serde(default)]
    pub continuation: Option<Continuation>,
    pub memory: usize,
    pub estimate: usize,
    pub modal: usize,
    message: String,
    overrides: Vec<String>,
}

fn input_phase_default() -> bool {
    true
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct Continuation {
    pub owner: usize,
    pub end: u32,
    pub performs: usize,
    pub stack: usize,
    pub language_frames: usize,
}

impl Machine {
    pub(super) fn sort_control_failure(
        &mut self,
        cur: usize,
        controls: &Controls,
        message: &str,
    ) -> RResult<()> {
        self.set_sort_status(cur, 16)?;
        self.sort_message(&controls.message, message)
    }
    pub(super) fn sort_cancelled(&mut self, cur: usize) -> RResult<bool> {
        Ok(self
            .sort_register(cur, "SORT-RETURN")?
            .is_some_and(|b| zeus_decimal::binary::read(&b, true) == 16))
    }
    pub(super) fn begin_sort_controls(&mut self, cur: usize, file: u32) -> RResult<()> {
        let mut c = self.sort_controls(cur)?;
        c.file_slot = Some(self.instances[cur].files[file as usize]);
        // FILSZ is an estimate, never a restriction on the number of input records.
        let slots = (c.memory
            / c.modal
                .max(1)
                .saturating_add(std::mem::size_of::<Vec<u8>>()))
        .min(c.estimate);
        self.sort_records
            .try_reserve(slots)
            .map_err(|e| RunError(e.to_string()))?;
        self.active_sort_controls = Some(c);
        self.set_sort_status(cur, 0)?;
        Ok(())
    }

    fn set_sort_status(&mut self, cur: usize, value: i128) -> RResult<()> {
        if let Some(desc) = self.instances[cur]
            .code
            .descs
            .iter()
            .rposition(|d| d.name == "SORT-RETURN")
        {
            let (a, n) = self.addr(
                cur,
                &Loc {
                    desc: desc as u32,
                    subs: vec![],
                    refmod: None,
                },
            )?;
            zeus_decimal::binary::write(&mut self.mem[a..a + n], value);
        }
        Ok(())
    }

    pub(super) fn abort_sort_if_requested(&mut self, state: &mut super::RunState) -> RResult<bool> {
        let Some(continuation) = self
            .active_sort_controls
            .as_ref()
            .and_then(|c| c.continuation.clone())
        else {
            return Ok(false);
        };
        let value = self.sort_register(continuation.owner, "SORT-RETURN")?;
        if !value.is_some_and(|b| zeus_decimal::binary::read(&b, true) == 16) {
            return Ok(false);
        }
        while state.stack.len() > continuation.stack {
            let instance = state.cur;
            self.instances[instance].active = false;
            self.instances[instance].perform.clear();
            if self.instances[instance].code.initial {
                self.close_program_files(instance)?;
                self.reset_initial_children(instance)?;
            }
            let frame = state.stack.pop().unwrap();
            state.cur = frame.inst;
        }
        self.le_conditions
            .frames
            .truncate(continuation.language_frames);
        state.cur = continuation.owner;
        self.instances[state.cur]
            .perform
            .truncate(continuation.performs);
        state.pc = continuation.end;
        self.sort_records.clear();
        self.sort_return = None;
        Ok(true)
    }
    fn sort_register(&mut self, cur: usize, name: &str) -> RResult<Option<Vec<u8>>> {
        let Some(desc) = self.instances[cur]
            .code
            .descs
            .iter()
            .rposition(|d| d.name == name)
        else {
            return Ok(None);
        };
        let (a, n) = self.addr(
            cur,
            &Loc {
                desc: desc as u32,
                subs: vec![],
                refmod: None,
            },
        )?;
        Ok(Some(self.mem[a..a + n].to_vec()))
    }

    pub(super) fn sort_controls(&mut self, cur: usize) -> RResult<Controls> {
        let mut number = |name| -> RResult<i64> {
            Ok(self
                .sort_register(cur, name)?
                .map(|b| zeus_decimal::binary::read(&b, true) as i64)
                .unwrap_or(0))
        };
        let core = number("SORT-CORE-SIZE")?;
        let estimate = number("SORT-FILE-SIZE")?;
        let modal = number("SORT-MODE-SIZE")?;
        if estimate < 0 || modal < 0 {
            return err("negative SORT-FILE-SIZE or SORT-MODE-SIZE");
        }
        let cp = crate::codepage::get(self.program_ccsid(cur)).unwrap();
        let message = self
            .sort_register(cur, "SORT-MESSAGE")?
            .map(|b| cp.decode(&b).trim().to_string())
            .unwrap_or_else(|| "SYSOUT".into());
        let control = self
            .sort_register(cur, "SORT-CONTROL")?
            .map(|b| cp.decode(&b).trim().to_string())
            .unwrap_or_else(|| "IGZSRTCD".into());
        let mut c = Controls {
            input_runs: vec![],
            file_slot: None,
            input_phase: true,
            maximum_memory: core <= 0 || matches!(core, 999999 | 9999999),
            reserved_memory: if core < 0 {
                core.unsigned_abs() as usize
            } else {
                0
            },
            exact_count: None,
            checkpoint: false,
            continuation: None,
            memory: match core {
                0 | 999999 | 9999999 => DEFAULT_MEMORY,
                n if n < 0 => DEFAULT_MEMORY,
                n => n as usize,
            },
            estimate: estimate as usize,
            modal: modal as usize,
            message,
            overrides: vec![],
        };
        if let Some(a) = self.dd.allocation(&[&control]) {
            let bytes = std::fs::read(a.path)
                .map_err(|e| RunError(format!("SORT control {control}: {e}")))?;
            let fmt = a.format.unwrap_or(zeus_ds::format::RecordFormat::Text);
            let records =
                zeus_ds::format::decode(fmt, &bytes).map_err(|e| RunError(e.to_string()))?;
            let lines: Vec<String> = records
                .iter()
                .map(|r| {
                    if fmt == zeus_ds::format::RecordFormat::Text {
                        String::from_utf8_lossy(r).into_owned()
                    } else {
                        cp.decode(r)
                    }
                })
                .collect();
            let statements = if fmt == zeus_ds::format::RecordFormat::Text {
                lines.join("\n")
            } else {
                control_cards(&records, cp)?
            };
            c.parse(&statements)?;
        }
        if c.maximum_memory {
            c.memory = DEFAULT_MEMORY.saturating_sub(c.reserved_memory).max(1);
        }
        for name in &c.overrides {
            self.sort_message(
                &c.message,
                &format!("Zeus SORT: control statement overrides {name}"),
            )?;
        }
        Ok(c)
    }

    pub(super) fn sort_message(&mut self, dd: &str, message: &str) -> RResult<()> {
        if let Some(a) = self.dd.allocation(&[dd]) {
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(a.path)
                .map_err(|e| RunError(e.to_string()))?;
            let bytes =
                if a.format == Some(zeus_ds::format::RecordFormat::Text) || a.format.is_none() {
                    format!("{message}\n").into_bytes()
                } else {
                    let mut bytes = vec![];
                    zeus_ds::format::encode_into(
                        a.format.unwrap(),
                        &crate::ebcdic::encode(message),
                        crate::ebcdic::SPACE,
                        &mut bytes,
                    );
                    bytes
                };
            f.write_all(&bytes).map_err(|e| RunError(e.to_string()))?;
        } else if dd == "SYSOUT" {
            writeln!(self.stdout, "{message}").map_err(|e| RunError(e.to_string()))?;
        } else {
            return err(format!("SORT message DD {dd} is not allocated"));
        }
        Ok(())
    }
}

/// DFSORT card images: labels, operand/remark separation, continuation
/// column 72 and the identification field in columns 73 through 80.
fn control_cards(records: &[Vec<u8>], cp: &crate::codepage::CodePage) -> RResult<String> {
    let mut statements = Vec::new();
    let mut operands = String::new();
    let mut pending = false;
    let mut broken_operand = false;
    let mut continuing_remark = false;
    for record in records {
        let mut card = record.clone();
        card.resize(80, 0x40);
        if card[0] == 0x5c || card[..71].iter().all(|&b| b == 0x40) {
            continue;
        }
        let explicit = card[71] != 0x40;
        if continuing_remark {
            continuing_remark = explicit;
            continue;
        }
        if pending && card[0] != 0x40 {
            return err("SORT continuation cannot contain a label");
        }
        let text = cp.decode(&card[..71]);
        let body = if pending {
            if broken_operand && card[1..15].iter().all(|&b| b == 0x40) {
                cp.decode(&card[15..71])
            } else {
                text.trim_start().to_string()
            }
        } else {
            let body = if card[0] == 0x40 {
                text.trim_start()
            } else {
                text.split_once(char::is_whitespace)
                    .map(|(_, rest)| rest.trim_start())
                    .unwrap_or("")
            };
            if body.is_empty() {
                continue;
            }
            let (operation, rest) = body.split_once(char::is_whitespace).unwrap_or((body, ""));
            let rest = if operation.eq_ignore_ascii_case("OPTION") {
                rest
            } else if operation.to_ascii_uppercase().starts_with("SMS=") {
                body
            } else {
                return err(format!("unsupported SORT control operation {operation}"));
            };
            if rest.trim().is_empty() {
                return err("SORT OPTION operands are missing");
            }
            operands.clear();
            rest.trim_start().to_string()
        };
        let part = body
            .split_once(char::is_whitespace)
            .map_or(body.as_str(), |(part, _)| part);
        operands.push_str(part);
        let comma = part.ends_with(',');
        broken_operand = explicit && card[70] != 0x40 && part.len() == body.len();
        pending = comma || broken_operand;
        if pending && explicit && !broken_operand {
            continuing_remark = true;
        }
        if !pending {
            statements.push(format!("OPTION {operands}"));
            continuing_remark = explicit;
        }
    }
    if pending || continuing_remark {
        return err("missing SORT control continuation");
    }
    Ok(statements.join("\n"))
}

impl Controls {
    fn parse(&mut self, text: &str) -> RResult<()> {
        for line in text.lines() {
            let line = line.trim().to_ascii_uppercase();
            if line.is_empty() || line.starts_with('*') {
                continue;
            }
            let line = line
                .strip_prefix("OPTION ")
                .or_else(|| line.strip_prefix("OPTION\t"))
                .unwrap_or(&line);
            for part in line.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                if matches!(part, "CKPT" | "CHKPT") {
                    self.checkpoint = true;
                    continue;
                }
                let Some((key, value)) = part.split_once('=') else {
                    return err(format!("invalid SORT control: {part}"));
                };
                let key = key.trim();
                let value = value.trim();
                let n = || {
                    value
                        .parse::<usize>()
                        .map_err(|_| RunError(format!("invalid SORT control value: {part}")))
                };
                let storage = || -> RResult<usize> {
                    let (digits, scale) = if let Some(digits) = value.strip_suffix('K') {
                        (digits, 1024usize)
                    } else if let Some(digits) = value.strip_suffix('M') {
                        (digits, 1048576usize)
                    } else {
                        (value, 1usize)
                    };
                    digits
                        .parse::<usize>()
                        .ok()
                        .and_then(|n| n.checked_mul(scale))
                        .ok_or_else(|| RunError(format!("invalid SORT storage value: {part}")))
                };
                let register = match key {
                    "MAINSIZE" => {
                        self.maximum_memory = value == "MAX";
                        self.memory = if value == "MAX" {
                            DEFAULT_MEMORY
                        } else {
                            storage()?.max(1)
                        };
                        "SORT-CORE-SIZE"
                    }
                    "RESINV" => {
                        self.reserved_memory = storage()?;
                        "SORT-CORE-SIZE"
                    }
                    "FILSZ" => {
                        let digits = value.strip_prefix(['E', 'U']).unwrap_or(value);
                        if digits.is_empty()
                            || digits.len() > 28
                            || !digits.bytes().all(|b| b.is_ascii_digit())
                        {
                            return err(format!("invalid FILSZ: {value}"));
                        }
                        let count = digits
                            .parse::<u128>()
                            .map_err(|_| RunError(format!("invalid FILSZ: {value}")))?;
                        self.estimate = usize::try_from(count).unwrap_or(usize::MAX);
                        self.exact_count = (digits == value).then_some(count);
                        "SORT-FILE-SIZE"
                    }
                    "SMS" => {
                        self.modal = n()?;
                        "SORT-MODE-SIZE"
                    }
                    "MSGDDN" => {
                        if value.is_empty()
                            || value.as_bytes()[0].is_ascii_digit()
                            || value.len() > 8
                            || !value
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b"@$#".contains(&b))
                        {
                            return err("invalid MSGDDN");
                        }
                        self.message = value.into();
                        "SORT-MESSAGE"
                    }
                    _ => return err(format!("unsupported SORT control keyword {key}")),
                };
                if !self.overrides.iter().any(|s| s == register) {
                    self.overrides.push(register.into());
                }
            }
        }
        Ok(())
    }
}

/// Stable bounded-workspace merge sort. Input/output record ownership stays
/// in the VM dataset model; the sorting workspace is limited by MAINSIZE.
pub(super) fn order(
    mut records: Vec<Vec<u8>>,
    controls: &Controls,
    compare: impl Fn(&[u8], &[u8]) -> Ordering,
) -> RResult<Vec<Vec<u8>>> {
    let mut runs: Vec<Option<std::fs::File>> = vec![];
    let mut start = 0;
    while start < records.len() {
        let mut end = start;
        let mut bytes = 0usize;
        while end < records.len() {
            let cost = records[end]
                .len()
                .max(controls.modal)
                .saturating_add(std::mem::size_of::<Vec<u8>>() * 2);
            if end > start && bytes.saturating_add(cost) > controls.memory {
                break;
            }
            bytes = bytes.saturating_add(cost);
            end += 1;
        }
        records[start..end].sort_by(|a, b| compare(a, b));
        if start == 0 && end == records.len() {
            return Ok(records);
        }
        let mut run = tempfile::tempfile().map_err(|e| RunError(e.to_string()))?;
        for record in &mut records[start..end] {
            run.write_all(&(record.len() as u64).to_le_bytes())
                .and_then(|_| run.write_all(record))
                .map_err(|e| RunError(e.to_string()))?;
            *record = vec![];
        }
        run.rewind().map_err(|e| RunError(e.to_string()))?;
        // A binary merge stack bounds both open files and record heads by
        // log2(number of runs), rather than opening one file per record.
        let mut level = 0;
        loop {
            if level == runs.len() {
                runs.push(Some(run));
                break;
            }
            if let Some(prior) = runs[level].take() {
                run = merge_runs(prior, run, &compare)?;
                level += 1;
            } else {
                runs[level] = Some(run);
                break;
            }
        }
        start = end;
    }
    let mut result = None;
    for run in runs.into_iter().rev().flatten() {
        result = Some(match result {
            Some(prior) => merge_runs(prior, run, &compare)?,
            None => run,
        });
    }
    if let Some(mut result) = result {
        for slot in &mut records {
            *slot =
                next(&mut result)?.ok_or_else(|| RunError("truncated SORT scratch run".into()))?;
        }
    }
    Ok(records)
}

fn next(run: &mut std::fs::File) -> RResult<Option<Vec<u8>>> {
    let mut size = [0u8; 8];
    if run
        .read(&mut size[..1])
        .map_err(|e| RunError(e.to_string()))?
        == 0
    {
        return Ok(None);
    }
    run.read_exact(&mut size[1..])
        .map_err(|e| RunError(e.to_string()))?;
    let mut record =
        vec![0; usize::try_from(u64::from_le_bytes(size)).map_err(|e| RunError(e.to_string()))?];
    run.read_exact(&mut record)
        .map_err(|e| RunError(e.to_string()))?;
    Ok(Some(record))
}

fn merge_runs(
    mut left: std::fs::File,
    mut right: std::fs::File,
    compare: &impl Fn(&[u8], &[u8]) -> Ordering,
) -> RResult<std::fs::File> {
    let mut out = tempfile::tempfile().map_err(|e| RunError(e.to_string()))?;
    let mut a = next(&mut left)?;
    let mut b = next(&mut right)?;
    while a.is_some() || b.is_some() {
        let take_left = match (&a, &b) {
            (Some(a), Some(b)) => compare(a, b) != Ordering::Greater,
            (Some(_), None) => true,
            _ => false,
        };
        let record = if take_left {
            a.take().unwrap()
        } else {
            b.take().unwrap()
        };
        out.write_all(&(record.len() as u64).to_le_bytes())
            .and_then(|_| out.write_all(&record))
            .map_err(|e| RunError(e.to_string()))?;
        if take_left {
            a = next(&mut left)?;
        } else {
            b = next(&mut right)?;
        }
    }
    out.rewind().map_err(|e| RunError(e.to_string()))?;
    Ok(out)
}
