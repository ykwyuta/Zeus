//! Synchronous OS-linkage condition handlers owned by dynamic language frames.
//! Normative interfaces: z/OS LE Programming Reference, CEEHDLR/CEEHDLU/CEESGL.
use super::super::{Machine, RResult, RunError};
use crate::hlasm;
use std::collections::{HashMap, VecDeque};

#[derive(Clone)]
pub(crate) struct Handler {
    id: u64,
    entry: u32,
    token: [u8; 4],
}
#[derive(Default)]
pub(crate) struct Conditions {
    pub frames: Vec<Vec<Handler>>,
    next_handler: u64,
    next_isi: u32,
    isi: HashMap<u32, ([u8; 8], [u8; 4])>,
    active: Vec<u64>,
}

impl Machine {
    /// Begin a dynamic language invocation. PERFORM does not create a frame.
    /// External runtimes may bracket their own activations with these methods.
    pub fn enter_language_frame(&mut self) -> usize {
        let depth = self.le_conditions.frames.len();
        self.le_conditions.frames.push(Vec::new());
        depth
    }
    /// Remove this invocation and any frames unwound by an error/STOP RUN.
    pub fn leave_language_frame(&mut self, depth: usize) {
        self.le_conditions.frames.truncate(depth);
        if depth == 0 {
            self.le_conditions.isi.clear();
        }
    }

    fn condition_entry(&self, entry: u32) -> bool {
        if entry & 1 != 0 {
            return false;
        } // 64-bit entry tags need a different ABI.
        let address = (entry & 0x7fff_ffff) as usize;
        self.hlasm_residency_condition_name(address).is_some()
            || self.hlasm.values().any(|(m, base, _)| {
                address.checked_sub(*base).is_some_and(|offset| {
                    offset < m.storage.len()
                        && m.entry_mode(offset as u32) != 64
                        && (m.code.iter().any(|i| i.offset == offset as u32)
                            || m.machine_code
                                .iter()
                                .any(|r| r.start <= offset as u32 && (offset as u32) < r.end))
                })
            })
    }

    pub(super) fn condition_service(&mut self, name: &str, args: &[Option<usize>]) -> RResult<()> {
        let required = |n: usize| {
            args.get(n)
                .copied()
                .flatten()
                .ok_or_else(|| RunError(format!("{name}: missing required parameter {}", n + 1)))
        };
        let first = required(0)?;
        let feedback = args
            .get(if name == "CEEHDLU" { 1 } else { 2 })
            .copied()
            .flatten();
        match name {
            "CEEHDLR" | "CEEHDLU" => {
                let entry = u32::from_be_bytes(self.mem[first..first + 4].try_into().unwrap());
                let token = if name == "CEEHDLR" {
                    let at = required(1)?;
                    Some(self.mem[at..at + 4].try_into().unwrap())
                } else {
                    None
                };
                let condition =
                    if name == "CEEHDLR" && !self.condition_entry(entry) {
                        Some((3, 257))
                    } else {
                        let frame = self.le_conditions.frames.last_mut().ok_or_else(|| {
                            RunError(format!("{name}: no active language invocation"))
                        })?;
                        let old = frame.iter().position(|h| h.entry == entry);
                        if let Some(old) = old {
                            frame.remove(old);
                        }
                        if let Some(token) = token {
                            self.le_conditions.next_handler =
                                self.le_conditions.next_handler.checked_add(1).ok_or_else(
                                    || RunError("condition handler identity exhausted".into()),
                                )?;
                            frame.push(Handler {
                                id: self.le_conditions.next_handler,
                                entry,
                                token,
                            });
                            old.map(|_| (1, 256))
                        } else {
                            old.is_none().then_some((1, 252))
                        }
                    };
                self.condition_feedback(feedback, condition);
                if condition.is_some_and(|(s, _)| s >= 2) && feedback.is_none() {
                    return super::super::err(format!("{}U4038", super::ABEND));
                }
            }
            "CEEGQDT" => {
                let output = required(1)?;
                let token: [u8; 12] = self.mem[first..first + 12].try_into().unwrap();
                let isi = u32::from_be_bytes(token[8..].try_into().unwrap());
                let qdata = self
                    .le_conditions
                    .isi
                    .get(&isi)
                    .filter(|(condition, _)| condition == &token[..8])
                    .map(|(_, q)| *q);
                if let Some(qdata) = qdata {
                    self.mem[output..output + 4].copy_from_slice(&qdata);
                }
                let condition = qdata
                    .is_none()
                    .then_some((3, if isi == 0 { 464 } else { 462 }));
                self.condition_feedback(feedback, condition);
                if condition.is_some() && feedback.is_none() {
                    return super::super::err(format!("{}U4038", super::ABEND));
                }
            }
            "CEESGL" => {
                let mut token: [u8; 12] = self.mem[first..first + 12].try_into().unwrap();
                if !super::condition::valid_token(&token) {
                    self.condition_feedback(feedback, Some((3, 102)));
                    if feedback.is_none() {
                        return super::super::err(format!("{}U4038", super::ABEND));
                    }
                    return Ok(());
                }
                // Snapshot inputs before any output or handler can alias them.
                let qdata = args
                    .get(1)
                    .copied()
                    .flatten()
                    .map(|a| <[u8; 4]>::try_from(&self.mem[a..a + 4]).unwrap());
                let old_isi = u32::from_be_bytes(token[8..].try_into().unwrap());
                if old_isi != 0
                    && self
                        .le_conditions
                        .isi
                        .get(&old_isi)
                        .is_none_or(|(c, _)| c != &token[..8])
                {
                    self.condition_feedback(feedback, Some((3, 462)));
                    if feedback.is_none() {
                        return super::super::err(format!("{}U4038", super::ABEND));
                    }
                    return Ok(());
                }
                if let Some(qdata) = qdata {
                    let isi = if old_isi != 0 {
                        old_isi
                    } else {
                        self.le_conditions.next_isi =
                            self.le_conditions.next_isi.checked_add(1).ok_or_else(|| {
                                RunError("condition ISI identity exhausted".into())
                            })?;
                        self.le_conditions.next_isi
                    };
                    self.le_conditions
                        .isi
                        .insert(isi, (token[..8].try_into().unwrap(), qdata));
                    token[8..].copy_from_slice(&isi.to_be_bytes());
                }
                self.signal_condition(token, feedback)?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn invoke_condition_handler(
        &mut self,
        entry: u32,
        args: &[Option<usize>],
    ) -> Result<(), String> {
        let address = (entry & 0x7fff_ffff) as usize;
        if let Some(name) = self.hlasm_residency_condition_name(address) {
            self.dispatch_assembler_target(hlasm::AssemblerCall {
                name: &name,
                args,
                terminated: true,
                parameter_mode: 31,
                allow_64: false,
                floating: None,
            })?;
            return Ok(());
        }
        let (module, base, _) = self
            .hlasm
            .values()
            .find(|(m, b, _)| address >= *b && address - b < m.storage.len())
            .cloned()
            .ok_or("condition handler entry is no longer loaded")?;
        if self.hlasm_call_depth >= 128 {
            return Err("HLASM nested call limit exceeded".into());
        }
        let offset = (address - base) as u32;
        if module.entry_mode(offset) == 64 {
            return Err("condition handler requires OS AMODE 24/31 linkage".into());
        }
        *self.hlasm_residency.active.entry(base).or_default() += 1;
        let frame = self.enter_language_frame();
        self.hlasm_call_depth += 1;
        let mut memory = std::mem::take(&mut self.mem);
        let mut storage = std::mem::take(&mut self.hlasm_storage);
        let result = hlasm::execute_entry_with_host(
            &module,
            &mut memory,
            base,
            args,
            offset,
            &mut storage,
            self,
            true,
            Some(31),
            None,
            None,
        );
        self.mem = memory;
        self.hlasm_storage = storage;
        self.hlasm_call_depth -= 1;
        self.leave_language_frame(frame);
        let cleanup = self.leave_hlasm(base);
        result.and(cleanup.map(|_| 0)).map(|_| ())
    }

    fn signal_condition(&mut self, mut token: [u8; 12], feedback: Option<usize>) -> RResult<()> {
        if self.le_conditions.active.len() >= 32 {
            return super::super::err("LE condition nesting limit exceeded");
        }
        // The handle cursor visits newest registrations in the youngest frame first.
        // Snapshot identities, checking liveness again before invoking each handler.
        let mut handlers: VecDeque<_> = self
            .le_conditions
            .frames
            .iter()
            .rev()
            .flat_map(|f| f.iter().rev().cloned())
            .collect();
        let base = self
            .hlasm_storage
            .frame(&mut self.mem, 32)
            .map_err(RunError)?;
        let saved_rc = self.return_code;
        let result = (|| {
            let mut skip_frame = None;
            let mut dispatches = 0u32;
            while let Some(handler) = handlers.pop_front() {
                dispatches += 1;
                if dispatches > 10_000 {
                    return super::super::err("LE condition promotion limit exceeded");
                }
                if self.le_conditions.active.contains(&handler.id) {
                    continue;
                }
                let owner = self
                    .le_conditions
                    .frames
                    .iter()
                    .position(|f| f.iter().any(|h| h.id == handler.id));
                let Some(owner) = owner else { continue };
                if skip_frame == Some(owner) {
                    continue;
                }
                self.mem[base..base + 12].copy_from_slice(&token);
                self.mem[base + 12..base + 16].copy_from_slice(&handler.token);
                self.mem[base + 16..base + 20].copy_from_slice(&20i32.to_be_bytes());
                self.mem[base + 20..base + 32].fill(0);
                self.le_conditions.active.push(handler.id);
                let called = self.invoke_condition_handler(
                    handler.entry,
                    &[
                        Some(base),
                        Some(base + 12),
                        Some(base + 16),
                        Some(base + 20),
                    ],
                );
                self.le_conditions.active.pop();
                called.map_err(RunError)?;
                let action = i32::from_be_bytes(self.mem[base + 16..base + 20].try_into().unwrap());
                match action {
                    10 if feedback.is_some() => {
                        self.condition_feedback(feedback, None);
                        return Ok(());
                    }
                    20 => {}
                    21 => {
                        skip_frame = Some(owner);
                    }
                    30..=33 => {
                        let promoted: [u8; 12] = self.mem[base + 20..base + 32].try_into().unwrap();
                        if !super::condition::valid_token(&promoted) {
                            return super::super::err(
                                "LE handler returned an invalid promoted condition",
                            );
                        }
                        // Returning the same condition means percolation, even
                        // for a restart/frame promotion request. ISI is not part
                        // of condition equivalence.
                        if promoted[..8] == token[..8] {
                            continue;
                        }
                        token = promoted;
                        if action == 31 {
                            skip_frame = Some(owner);
                        } else if action == 32 || action == 33 {
                            let youngest = if action == 32 {
                                owner + 1
                            } else {
                                self.le_conditions.frames.len()
                            };
                            handlers = self.le_conditions.frames[..youngest]
                                .iter()
                                .rev()
                                .flat_map(|frame| frame.iter().rev().cloned())
                                .collect();
                            skip_frame = None;
                        }
                    }
                    60 if feedback.is_some() => {
                        let new: [u8; 12] = self.mem[base + 20..base + 32].try_into().unwrap();
                        if !super::condition::valid_token(&new) {
                            return super::super::err(
                                "LE handler returned an invalid fix-up condition",
                            );
                        }
                        self.mem[feedback.unwrap()..feedback.unwrap() + 12].copy_from_slice(&new);
                        return Ok(());
                    }
                    _ => {
                        return super::super::err(format!(
                            "LE handler action {action} requires unsupported resume-cursor or fix-up semantics"
                        ));
                    }
                }
            }
            if let Some(feedback) = feedback {
                // LE returns the unhandled condition itself to a caller providing FC.
                self.mem[feedback..feedback + 12].copy_from_slice(&token);
                Ok(())
            } else if (token[4] >> 3) & 7 <= 1 {
                Ok(())
            } else {
                super::super::err(format!("{}U4038", super::ABEND))
            }
        })();
        self.return_code = saved_rc;
        self.hlasm_storage.release_frame(&mut self.mem, base);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn handler(m: &mut Machine, args: &[Option<usize>]) -> Result<i32, String> {
        let cond = args[0].unwrap();
        let token = args[1].unwrap();
        let result = args[2].unwrap();
        assert_eq!(&m.mem[token..token + 4], &1234i32.to_be_bytes());
        let q = m.alloc(&[0; 4]);
        let fc = m.alloc(&[0; 12]);
        m.le_service("CEEGQDT", &[Some(cond), Some(q), Some(fc)])
            .map_err(|e| e.0)?;
        assert_eq!(&m.mem[q..q + 4], &5678i32.to_be_bytes());
        assert_eq!(&m.mem[fc..fc + 12], &[0; 12]);
        // A nested signal skips the currently running registration, percolates,
        // and returns the condition without recursively calling this handler.
        m.le_service("CEESGL", &[Some(cond), None, Some(fc)])
            .map_err(|e| e.0)?;
        assert_eq!(&m.mem[fc..fc + 8], &m.mem[cond..cond + 8]);
        m.mem[result..result + 4].copy_from_slice(&10i32.to_be_bytes());
        Ok(0)
    }
    fn setup() -> (Machine, usize, Vec<Option<usize>>) {
        let mut m = Machine::new(vec![]);
        m.add_native_machine("HANDLER", handler);
        let frame = m.enter_language_frame();
        let (entry, _) = m.load_assembler_service("HANDLER").unwrap();
        let entry = m.alloc(&entry.to_be_bytes());
        let token = m.alloc(&1234i32.to_be_bytes());
        let fc = m.alloc(&[0; 12]);
        (m, frame, vec![Some(entry), Some(token), Some(fc)])
    }
    #[test]
    fn saved_tokens_isi_nested_signals_and_dynamic_frame_cleanup() {
        let (mut m, frame, args) = setup();
        m.le_service("CEEHDLR", &args).unwrap();
        // Registration takes a value, not a pointer to the caller's token.
        m.mem[args[1].unwrap()..args[1].unwrap() + 4].fill(0);
        let token = m.alloc(&[0, 3, 0, 99, 0x58, 0xe9, 0xe9, 0xe9, 0, 0, 0, 0]);
        let q = m.alloc(&5678i32.to_be_bytes());
        let fc = args[2].unwrap();
        m.le_service("CEESGL", &[Some(token), Some(q), Some(fc)])
            .unwrap();
        assert_eq!(&m.mem[fc..fc + 12], &[0; 12]);
        assert!(m.le_conditions.active.is_empty());
        assert_eq!(m.le_conditions.frames.len(), 1);
        assert_eq!(m.le_conditions.isi.len(), 1);
        m.leave_language_frame(frame);
        assert!(m.le_conditions.frames.is_empty());
        assert!(m.le_conditions.isi.is_empty());
        assert!(m.le_conditions.active.is_empty());
    }
    #[test]
    fn reregistration_and_unregister_are_local_to_the_current_frame() {
        let (mut m, outer, args) = setup();
        m.le_service("CEEHDLR", &args).unwrap();
        m.le_service("CEEHDLR", &args).unwrap();
        let fc = args[2].unwrap();
        assert_eq!(&m.mem[fc..fc + 4], &[0, 1, 1, 0]);
        assert_eq!(m.le_conditions.frames[0].len(), 1);
        let child = m.enter_language_frame();
        m.le_service("CEEHDLU", &[args[0], args[2]]).unwrap();
        assert_eq!(&m.mem[fc..fc + 4], &[0, 1, 0, 252]);
        assert_eq!(m.le_conditions.frames[0].len(), 1);
        m.le_service("CEEHDLR", &args).unwrap();
        m.leave_language_frame(child);
        assert_eq!(m.le_conditions.frames[0].len(), 1);
        m.le_service("CEEHDLU", &[args[0], args[2]]).unwrap();
        assert_eq!(&m.mem[fc..fc + 12], &[0; 12]);
        assert!(m.le_conditions.frames[0].is_empty());
        m.leave_language_frame(outer);
    }
    fn restart_handler(m: &mut Machine, args: &[Option<usize>]) -> Result<i32, String> {
        let token = args[1].unwrap();
        let context = u32::from_be_bytes(m.mem[token..token + 4].try_into().unwrap()) as usize;
        let count = context + 4;
        let n = u32::from_be_bytes(m.mem[count..count + 4].try_into().unwrap()) + 1;
        m.mem[count..count + 4].copy_from_slice(&n.to_be_bytes());
        let cond = args[0].unwrap();
        let action = if m.mem[cond + 3] == 99 {
            let new = args[3].unwrap();
            let mut promoted: [u8; 12] = m.mem[cond..cond + 12].try_into().unwrap();
            promoted[3] = 100;
            m.mem[new..new + 12].copy_from_slice(&promoted);
            u32::from_be_bytes(m.mem[context..context + 4].try_into().unwrap())
        } else {
            10
        };
        let result = args[2].unwrap();
        m.mem[result..result + 4].copy_from_slice(&action.to_be_bytes());
        Ok(0)
    }
    fn younger_handler(m: &mut Machine, args: &[Option<usize>]) -> Result<i32, String> {
        let token = args[1].unwrap();
        let context = u32::from_be_bytes(m.mem[token..token + 4].try_into().unwrap()) as usize;
        let count = context + 8;
        let n = u32::from_be_bytes(m.mem[count..count + 4].try_into().unwrap()) + 1;
        m.mem[count..count + 4].copy_from_slice(&n.to_be_bytes());
        let result = args[2].unwrap();
        m.mem[result..result + 4].copy_from_slice(&20i32.to_be_bytes());
        Ok(0)
    }
    #[test]
    fn promotion_32_restarts_at_handle_frame_33_at_younger_resume_frame() {
        for action in [32u32, 33] {
            let mut m = Machine::new(vec![]);
            m.add_native_machine("OLDER", restart_handler);
            m.add_native_machine("YOUNGER", younger_handler);
            let context = m.alloc(&[0; 12]);
            m.mem[context..context + 4].copy_from_slice(&action.to_be_bytes());
            let token = m.alloc(&(context as u32).to_be_bytes());
            let fc = m.alloc(&[0; 12]);
            let outer = m.enter_language_frame();
            let (entry, _) = m.load_assembler_service("OLDER").unwrap();
            let entry = m.alloc(&entry.to_be_bytes());
            m.le_service("CEEHDLR", &[Some(entry), Some(token), Some(fc)])
                .unwrap();
            let inner = m.enter_language_frame();
            let (entry, _) = m.load_assembler_service("YOUNGER").unwrap();
            let entry = m.alloc(&entry.to_be_bytes());
            m.le_service("CEEHDLR", &[Some(entry), Some(token), Some(fc)])
                .unwrap();
            let condition = m.alloc(&[0, 3, 0, 99, 0x58, 0xe9, 0xe9, 0xe9, 0, 0, 0, 0]);
            m.le_service("CEESGL", &[Some(condition), None, Some(fc)])
                .unwrap();
            assert_eq!(&m.mem[fc..fc + 12], &[0; 12]);
            assert_eq!(&m.mem[context + 4..context + 8], &2u32.to_be_bytes());
            assert_eq!(
                &m.mem[context + 8..context + 12],
                &(if action == 32 { 1u32 } else { 2 }).to_be_bytes()
            );
            m.leave_language_frame(inner);
            m.leave_language_frame(outer);
            assert!(m.le_conditions.frames.is_empty());
            assert!(m.le_conditions.active.is_empty());
        }
    }
    fn cycling_handler(m: &mut Machine, args: &[Option<usize>]) -> Result<i32, String> {
        let condition = args[0].unwrap();
        let new = args[3].unwrap();
        let mut promoted: [u8; 12] = m.mem[condition..condition + 12].try_into().unwrap();
        promoted[3] = if promoted[3] == 99 { 100 } else { 99 };
        m.mem[new..new + 12].copy_from_slice(&promoted);
        let action = args[2].unwrap();
        m.mem[action..action + 4].copy_from_slice(&32i32.to_be_bytes());
        Ok(0)
    }
    #[test]
    fn promotion_cycle_reports_error_and_releases_dispatch_storage_and_active_state() {
        let mut m = Machine::new(vec![]);
        m.add_native_machine("CYCLE", cycling_handler);
        let outer = m.enter_language_frame();
        let (entry, _) = m.load_assembler_service("CYCLE").unwrap();
        let entry = m.alloc(&entry.to_be_bytes());
        let saved = m.alloc(&[0; 4]);
        let fc = m.alloc(&[0; 12]);
        m.le_service("CEEHDLR", &[Some(entry), Some(saved), Some(fc)])
            .unwrap();
        let condition = m.alloc(&[0, 3, 0, 99, 0x58, 0xe9, 0xe9, 0xe9, 0, 0, 0, 0]);
        let before = m.mem.len();
        let error = m
            .le_service("CEESGL", &[Some(condition), None, Some(fc)])
            .unwrap_err();
        assert!(error.0.contains("promotion limit"), "{}", error.0);
        assert_eq!(m.mem.len(), before);
        assert_eq!(m.le_conditions.frames.len(), 1);
        assert!(m.le_conditions.active.is_empty());
        m.leave_language_frame(outer);
    }
    #[test]
    fn every_service_preflights_all_parameters_before_mutating_state_or_memory() {
        for (name, layout) in [
            ("CEEHDLR", vec![4, 4, 12]),
            ("CEEHDLU", vec![4, 12]),
            ("CEESGL", vec![12, 4, 12]),
            ("CEEGQDT", vec![12, 4, 12]),
            ("CEEDCOD", vec![12, 2, 2, 2, 2, 2, 3, 4, 12]),
        ] {
            for index in 0..layout.len() {
                for bad in [usize::MAX, 127] {
                    let mut m = Machine::new(vec![]);
                    m.mem = vec![0xaa; 128];
                    let mut args = vec![Some(32); layout.len()];
                    args[index] = Some(bad);
                    let before = m.mem.clone();
                    assert!(m.le_service(name, &args).is_err(), "{name}/{index}");
                    assert_eq!(m.mem, before, "{name}/{index}");
                    assert!(m.le_conditions.frames.is_empty());
                    assert!(m.le_conditions.isi.is_empty());
                }
            }
        }
    }
}
