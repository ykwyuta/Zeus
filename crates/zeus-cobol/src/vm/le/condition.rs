//! Condition-token construction from the public LE parameter/bit-field ABI.
use crate::{
    ebcdic,
    vm::{Machine, RResult, RunError},
};

impl Machine {
    /// Decompose a token after common preflight, preserving aliased inputs.
    pub(super) fn decompose_condition(&mut self, args: &[Option<usize>]) -> RResult<()> {
        let fields = (0..8)
            .map(|n| {
                args.get(n).copied().flatten().ok_or_else(|| {
                    RunError(format!("CEEDCOD: missing required parameter {}", n + 1))
                })
            })
            .collect::<RResult<Vec<_>>>()?;
        let token: [u8; 12] = self.mem[fields[0]..fields[0] + 12].try_into().unwrap();
        let feedback = args.get(8).copied().flatten();
        if !valid_token(&token) {
            self.condition_feedback(feedback, Some((3, 102)));
            if feedback.is_none() {
                return super::super::err(format!("{}U4038", super::ABEND));
            }
            return Ok(());
        }
        let case = u16::from(token[4] >> 6).to_be_bytes();
        let severity = u16::from((token[4] >> 3) & 7).to_be_bytes();
        let control = u16::from(token[4] & 7).to_be_bytes();
        for (n, bytes) in [
            &token[..2],
            &token[2..4],
            &case,
            &severity,
            &control,
            &token[5..8],
            &token[8..12],
        ]
        .into_iter()
        .enumerate()
        {
            self.mem[fields[n + 1]..fields[n + 1] + bytes.len()].copy_from_slice(bytes);
        }
        self.condition_feedback(feedback, None);
        Ok(())
    }

    /// Called after the common LE range/protection preflight has validated
    /// every supplied argument. Read all inputs before writing aliased outputs.
    pub(super) fn construct_condition(&mut self, args: &[Option<usize>]) -> RResult<()> {
        let fields = (0..8)
            .map(|n| {
                args.get(n).copied().flatten().ok_or_else(|| {
                    RunError(format!("CEENCOD: missing required parameter {}", n + 1))
                })
            })
            .collect::<RResult<Vec<_>>>()?;
        let half =
            |n: usize| i16::from_be_bytes(self.mem[fields[n]..fields[n] + 2].try_into().unwrap());
        let (case, severity, control) = (half(2), half(3), half(4));
        let mut token = [0u8; 12];
        token[..2].copy_from_slice(&self.mem[fields[0]..fields[0] + 2]);
        token[2..4].copy_from_slice(&self.mem[fields[1]..fields[1] + 2]);
        token[5..8].copy_from_slice(&self.mem[fields[5]..fields[5] + 3]);
        token[8..12].copy_from_slice(&self.mem[fields[6]..fields[6] + 4]);
        let facility = ebcdic::decode(&token[5..8]);
        let alphanumeric =
            facility.len() == 3 && facility.bytes().all(|c| c.is_ascii_alphanumeric());
        let condition = if !matches!(case, 1 | 2) {
            Some((3, 401))
        } else if !matches!(control, 0 | 1) {
            Some((3, 402))
        } else if !(0..=4).contains(&severity) {
            Some((3, 403))
        } else if !alphanumeric {
            Some((1, 404))
        } else if control == 0
            && !matches!(facility.as_bytes()[0].to_ascii_uppercase(), b'J'..=b'Z')
        {
            Some((3, 452))
        } else {
            None
        };
        let feedback = args.get(8).copied().flatten();
        if condition.is_none_or(|(severity, _)| severity < 2) {
            token[4] = ((case as u8) << 6) | ((severity as u8) << 3) | control as u8;
            self.mem[fields[7]..fields[7] + 12].copy_from_slice(&token);
        }
        self.condition_feedback(feedback, condition);
        if condition.is_some_and(|(severity, _)| severity >= 2) && feedback.is_none() {
            return Err(RunError(format!("{}U4038", super::ABEND)));
        }
        Ok(())
    }
}

pub(super) fn valid_token(token: &[u8; 12]) -> bool {
    *token == [0; 12]
        || (matches!(token[4] >> 6, 1 | 2) && ((token[4] >> 3) & 7) <= 4 && token[4] & 6 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inputs() -> (Machine, Vec<Option<usize>>) {
        let mut machine = Machine::new(vec![]);
        machine.mem = vec![0xAA; 128];
        machine.mem[32..42].copy_from_slice(&[0, 3, 0xFE, 0xDC, 0, 1, 0, 3, 0, 1]);
        machine.mem[42..45].copy_from_slice(&ebcdic::encode("CEE"));
        machine.mem[45..49].fill(0xFF);
        (
            machine,
            [32, 34, 36, 38, 40, 42, 45, 64, 80]
                .into_iter()
                .map(Some)
                .collect(),
        )
    }
    #[test]
    fn missing_or_out_of_bounds_parameters_do_not_partially_write_outputs() {
        for n in 0..9 {
            for bad in [Some(127), Some(usize::MAX)] {
                let (mut machine, mut args) = inputs();
                let before = machine.mem.clone();
                args[n] = bad;
                assert!(
                    machine.le_service("CEENCOD", &args).is_err(),
                    "parameter {n}"
                );
                assert_eq!(machine.mem, before);
            }
            if n < 8 {
                let (mut machine, mut args) = inputs();
                let before = machine.mem.clone();
                args[n] = None;
                assert!(
                    machine
                        .le_service("CEENCOD", &args)
                        .unwrap_err()
                        .0
                        .contains("missing required")
                );
                assert_eq!(machine.mem, before);
            }
        }
    }
    #[test]
    fn constructor_reads_all_input_fields_before_writing_an_aliasing_token() {
        let (mut machine, mut args) = inputs();
        args[7] = Some(32);
        machine.le_service("CEENCOD", &args).unwrap();
        assert_eq!(
            &machine.mem[32..44],
            &[
                0, 3, 0xFE, 0xDC, 0x59, 0xC3, 0xC5, 0xC5, 0xFF, 0xFF, 0xFF, 0xFF
            ]
        );
        assert_eq!(&machine.mem[80..92], &[0; 12]);
    }
}
