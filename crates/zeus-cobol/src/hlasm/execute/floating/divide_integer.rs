//! BFP divide-to-integer, from the architectural result tables (19-28--31).
//! Quotient and remainder are formed together from exact integer ratios.
use super::{
    State, Value, decode, encode, exceptions, mode, negative, quiet, read, signaling, write, zero,
};
use crate::hlasm::{
    Instruction,
    float::{self, Format, Ratio, Rounding},
};
use num_traits::{One, Zero};

pub(super) fn execute(
    ins: &Instruction,
    width: usize,
    state: &mut State,
    cc: &mut u8,
) -> Result<(), String> {
    let reg = |n| {
        ins.registers
            .get(n)
            .copied()
            .filter(|r| *r < 16)
            .map(usize::from)
            .ok_or("invalid DIVIDE TO INTEGER FPR")
    };
    let (r, q, b) = (reg(0)?, reg(1)?, reg(2)?);
    if r == q || r == b || q == b {
        return Err("DIVIDE TO INTEGER requires three different FPRs".into());
    }
    let rounding = float::convert::rounding(ins.immediate, state.control, Format::Binary)?;
    let current = mode(state.control)?;
    let a = decode(read(state, r, width), width);
    let divisor = decode(read(state, b, width), width);
    // SNaNs take priority, dividend before divisor; then QNaNs follow the
    // same order. Invalid operation suppresses both destinations when trapped.
    let special = if signaling(&a) {
        Some((quiet(&a), true))
    } else if signaling(&divisor) {
        Some((quiet(&divisor), true))
    } else if matches!(a, Value::Nan { .. }) {
        Some((a.clone(), false))
    } else if matches!(divisor, Value::Nan { .. }) {
        Some((divisor.clone(), false))
    } else if matches!(a, Value::Infinity(_)) || zero(&divisor) {
        Some((super::default_nan(), true))
    } else {
        None
    };
    if let Some((value, invalid)) = special {
        if invalid {
            exceptions(state, 0x80)?;
        }
        let raw = encode(&value, width, current).0;
        write(state, r, width, raw);
        write(state, q, width, raw);
        *cc = 1;
        return Ok(());
    }
    let sign = negative(&a) ^ negative(&divisor);
    if zero(&a) || matches!(divisor, Value::Infinity(_)) {
        write(state, q, width, u128::from(sign) << (width * 8 - 1));
        *cc = 0;
        return Ok(());
    }
    let (Value::Finite(a), Value::Finite(b)) = (a, divisor) else {
        unreachable!()
    };
    let exact = Ratio {
        negative: sign,
        n: &a.n * &b.d,
        d: &a.d * &b.n,
    };
    let precision = if width == 4 { 24 } else { 53 };
    let e = exact.floor_log2();
    // Above the precision limit, retain a representable portion toward zero.
    // When it divides exactly the result is final, even at a large exponent.
    let partial_candidate = e >= precision;
    let shift = if partial_candidate {
        e - precision + 1
    } else {
        0
    };
    let (n, _) = exact.quantize(
        shift,
        if partial_candidate {
            Rounding::Zero
        } else {
            rounding
        },
    );
    let mut quotient = Ratio {
        negative: sign,
        n,
        d: num_bigint::BigUint::one(),
    };
    quotient.n <<= shift as usize;
    let mut product = b.multiply(&quotient);
    product.negative = !product.negative;
    let remainder = a.add(&product, a.negative);
    let partial = partial_candidate && !remainder.n.is_zero();
    let bias = if width == 4 { 127 } else { 1023 };
    let alpha = if width == 4 { 192 } else { 1536 };
    let overflow = !quotient.n.is_zero() && quotient.floor_log2() > bias;
    if overflow {
        quotient.d <<= alpha;
    }
    let quotient_raw = encode(&Value::Finite(quotient), width, Rounding::Zero).0;
    let tiny = !partial && !remainder.n.is_zero() && remainder.floor_log2() < 1 - bias;
    let underflow_trap = tiny && state.control & 0x10000000 != 0;
    let mut result = remainder.clone();
    if underflow_trap {
        result.n <<= alpha;
    }
    let (remainder_raw, mut flags) = encode(&Value::Finite(result.clone()), width, current);
    // A partial remainder is exact and must not recognize underflow.
    if partial {
        flags = 0;
    }
    if underflow_trap {
        flags |= 0x10;
    }
    if flags & 8 != 0
        && let Value::Finite(rounded) = decode(remainder_raw, width)
        && &rounded.n * &result.d > &result.n * &rounded.d
    {
        flags |= 4;
    }
    write(state, r, width, remainder_raw);
    write(state, q, width, quotient_raw);
    *cc = u8::from(partial) * 2 + u8::from(overflow);
    exceptions(state, flags)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn step(state: &mut State, cc: &mut u8) -> Result<(), String> {
        step_round(state, cc, 5)
    }
    fn step_round(state: &mut State, cc: &mut u8, rounding: u32) -> Result<(), String> {
        let ins = serde_json::from_value(serde_json::json!({
            "offset": 0, "op": "DIEBR", "registers": [0,4,2],
            "address": null, "source": null, "length": 0, "immediate": rounding,
            "file": "own-test", "line": 1
        }))
        .unwrap();
        execute(&ins, 4, state, cc)
    }
    #[test]
    fn invalid_trap_suppresses_both_destinations_and_preserves_cc() {
        let mut state = State {
            control: 0x80000000,
            ..Default::default()
        };
        state.registers[0] = 0x3f80000011223344;
        state.registers[4] = 0x4040000055667788;
        let mut cc = 2;
        assert!(step(&mut state, &mut cc).unwrap_err().contains("DXC=80"));
        assert_eq!(state.registers[0], 0x3f80000011223344);
        assert_eq!(state.registers[4], 0x4040000055667788);
        assert_eq!(cc, 2);
        assert_eq!(state.control, 0x80008000);
    }
    #[test]
    fn final_tiny_remainder_traps_with_scaled_result_and_complete_quotient() {
        let mut state = State {
            control: 0x10000000,
            ..Default::default()
        };
        state.registers[0] = 0x0000000511223344;
        state.registers[2] = 0x0000000355667788;
        state.registers[4] = 0x000000009abcdef0;
        let mut cc = 3;
        assert!(step(&mut state, &mut cc).unwrap_err().contains("DXC=10"));
        assert_eq!(state.registers[0], 0x5580000011223344); // 2**(-148+192)
        assert_eq!(state.registers[4], 0x3f8000009abcdef0);
        assert_eq!(cc, 0);
        assert_eq!(state.control, 0x10001000);
    }
    #[test]
    fn partial_tiny_remainder_does_not_recognize_underflow_until_final_iteration() {
        let mut state = State {
            control: 0x10000000,
            ..Default::default()
        };
        // 2**-123 / (3 * minimum subnormal) has a tiny partial remainder.
        state.registers[0] = 0x0200000000000000;
        state.registers[2] = 0x0000000300000000;
        let mut cc = 0;
        step(&mut state, &mut cc).unwrap();
        assert_eq!(cc, 2);
        assert_eq!(state.control, 0x10000000);
        assert!(state.registers[0] > 0);
        assert!(step(&mut state, &mut cc).unwrap_err().contains("DXC=10"));
        assert_eq!(cc, 0);
    }
    #[test]
    fn inexact_final_remainder_uses_current_rounding_and_completes_before_trapping() {
        for (control, raw, dxc) in [
            (0x08000000, 0xf1800000u64, "DXC=0C"),
            (0x08000001, 0xf17fffffu64, "DXC=08"),
        ] {
            let mut state = State {
                control,
                ..Default::default()
            };
            state.registers[0] = 0x3f80000011223344;
            state.registers[2] = 0x7180000000000000;
            let mut cc = 3;
            assert!(
                step_round(&mut state, &mut cc, 6)
                    .unwrap_err()
                    .contains(dxc)
            );
            assert_eq!(state.registers[0], raw << 32 | 0x11223344);
            assert_eq!(state.registers[4], 0x3f80000000000000);
            assert_eq!(cc, 0);
        }
    }
}
