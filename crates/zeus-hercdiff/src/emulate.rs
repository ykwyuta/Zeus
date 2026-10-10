//! Replays cases through `zeus-decimal` and compares with oracle records.

use zeus_decimal::insn;

use crate::Record;
use crate::cases::{Case, Op};
use crate::image::Observed;

/// Result of a case computed by zeus-decimal, in the oracle's terms.
pub fn emulate(case: &Case) -> Observed {
    let mut op1 = case.op1.clone();
    let mut op2 = case.op2.clone();
    let before1 = op1.clone();
    let result: Result<Option<u8>, insn::Exception> = match case.op {
        Op::Ap => insn::ap(&mut op1, &op2).map(Some),
        Op::Sp => insn::sp(&mut op1, &op2).map(Some),
        Op::Zap => insn::zap(&mut op1, &op2).map(Some),
        Op::Cp => insn::cp(&op1, &op2).map(Some),
        // MP, DP and the moves leave the condition code (0, set before
        // the instruction by the test program) unchanged.
        Op::Mp => insn::mp(&mut op1, &op2).map(|_| Some(0)),
        Op::Dp => insn::dp(&mut op1, &op2).map(|_| Some(0)),
        Op::Srp => {
            let (shift, round) = case.srp.unwrap_or((0, 0));
            insn::srp(&mut op1, shift, round).map(Some)
        }
        Op::Mvo => {
            insn::mvo(&mut op1, &op2);
            Ok(Some(0))
        }
        Op::Pack => {
            insn::pack(&mut op1, &op2);
            Ok(Some(0))
        }
        Op::Unpk => {
            insn::unpk(&mut op1, &op2);
            Ok(Some(0))
        }
    };
    match result {
        Ok(cc) => Observed {
            op1,
            op2: std::mem::take(&mut op2),
            cc,
            interruption: None,
        },
        Err(e) => Observed {
            op1: before1,
            op2,
            cc: None,
            interruption: Some(e.code()),
        },
    }
}

/// Records whose oracle result differs from zeus-decimal.
pub fn mismatches(records: &[Record]) -> Vec<(&Record, Observed)> {
    records
        .iter()
        .filter_map(|r| {
            let ours = emulate(&r.case);
            (ours != r.result).then_some((r, ours))
        })
        .collect()
}
