//! PFPO converts between radixes using exact integer ratios. Range traps use
//! the target radix's normalized scaled result and return its signed exponent.
use super::{State, Value, decimal, decode, hex, mode, read, signaling, write};
use crate::hlasm::float::{self, Format, Ratio, Rounding};
use num_bigint::BigUint;
use num_traits::{One, ToPrimitive, Zero};

#[derive(Clone, Copy)]
struct Shape {
    format: Format,
    width: usize,
}
fn shape(code: u8) -> Option<Shape> {
    let (format, width) = match code {
        0 => (Format::Hex, 4),
        1 => (Format::Hex, 8),
        2 => (Format::Hex, 16),
        5 => (Format::Binary, 4),
        6 => (Format::Binary, 8),
        7 => (Format::Binary, 16),
        8 => (Format::Decimal, 4),
        9 => (Format::Decimal, 8),
        10 => (Format::Decimal, 16),
        _ => return None,
    };
    Some(Shape { format, width })
}
fn value(raw: u128, shape: Shape) -> Value {
    match shape.format {
        Format::Hex => Value::Finite(hex::decode(raw, shape.width).0),
        Format::Binary => decode(raw, shape.width),
        Format::Decimal => decimal::decode(raw, shape.width).0,
    }
}
fn decimal_ratio(negative: bool, q: BigUint, exponent: i32) -> Ratio {
    if exponent >= 0 {
        Ratio {
            negative,
            n: q * BigUint::from(10u8).pow(exponent as u32),
            d: BigUint::one(),
        }
    } else {
        Ratio {
            negative,
            n: q,
            d: BigUint::from(10u8).pow(exponent.unsigned_abs()),
        }
    }
}
fn different(a: &Ratio, b: &Ratio) -> bool {
    &a.n * &b.d != &b.n * &a.d
}
fn increased(a: &Ratio, b: &Ratio) -> bool {
    &a.n * &b.d > &b.n * &a.d
}
fn hex_precision(width: usize) -> u32 {
    if width == 16 {
        112
    } else {
        (width * 8 - 8) as u32
    }
}
fn hex_raw(negative: bool, q: u128, e: i32, width: usize) -> u128 {
    let characteristic = ((e + 64) & 127) as u128;
    let sign = u128::from(negative);
    if width == 16 {
        ((sign << 63 | characteristic << 56 | q >> 56) << 64)
            | (sign << 63)
            | (((characteristic + 114) & 127) << 56)
            | (q & 0x00ff_ffff_ffff_ffff)
    } else {
        (sign << (width * 8 - 1)) | (characteristic << (width * 8 - 8)) | q
    }
}
fn hex_zero(negative: bool, width: usize) -> u128 {
    let sign = u128::from(negative) << (width * 8 - 1);
    if width == 16 {
        sign | (u128::from(negative) << 63)
    } else {
        sign
    }
}
fn hex_max(negative: bool, width: usize) -> u128 {
    hex_raw(negative, (1u128 << hex_precision(width)) - 1, 63, width)
}
fn magnitude_increased(raw: u128, target: Shape, source: &Ratio) -> bool {
    match value(raw, target) {
        Value::Infinity(_) => true,
        Value::Finite(v) => increased(&v, source),
        _ => false,
    }
}

struct Converted {
    raw: u128,
    flags: u8,
    rounded: Option<Ratio>,
    quantum: bool,
    incremented: bool,
}
fn finite(source: &Ratio, target: Shape, rounding: Rounding, controls: u8, masks: u8) -> Converted {
    if target.format == Format::Hex {
        if source.n.is_zero() {
            return Converted {
                raw: hex_zero(source.negative, target.width),
                flags: 0,
                rounded: Some(source.clone()),
                quantum: false,
                incremented: false,
            };
        }
        let precision = hex_precision(target.width);
        let mut e = source.floor_log2().div_euclid(4) + 1;
        let quantum = 4 * e - precision as i32;
        let (mut q, _) = source.quantize(quantum, rounding);
        let rounded = Ratio::binary(source.negative, q.to_u128().unwrap(), quantum);
        if q.bits() > u64::from(precision) {
            q >>= 4usize;
            e += 1;
        }
        let (raw, flags) = if e > 63 {
            (
                hex_max(source.negative, target.width),
                if controls & 0x20 != 0 { 0x28 } else { 0x80 },
            )
        } else if e < -64 {
            (
                hex_zero(source.negative, target.width),
                8 | if controls & 0x10 != 0 { 0x10 } else { 0 },
            )
        } else {
            (
                hex_raw(source.negative, q.to_u128().unwrap(), e, target.width),
                if different(&rounded, source) { 8 } else { 0 },
            )
        };
        return Converted {
            raw,
            flags,
            incremented: magnitude_increased(raw, target, source),
            rounded: Some(rounded),
            quantum: false,
        };
    }
    if target.format == Format::Binary {
        let (precision, bias) = float::binary_shape(target.width);
        let rounded = if source.n.is_zero() {
            source.clone()
        } else {
            let exponent = source.floor_log2() - precision as i32;
            let (q, _) = source.quantize(exponent, rounding);
            Ratio::binary(source.negative, q.to_u128().unwrap(), exponent)
        };
        let (raw, status) = float::encode_binary(source, target.width, precision, rounding);
        let mut flags = (u8::from(status.overflow) * 0x20)
            | (u8::from(status.underflow) * 0x10)
            | (u8::from(status.inexact) * 8);
        // PFPO detects tininess before rounding, including a value which
        // rounds upward to the smallest normal number.
        if !source.n.is_zero()
            && (masks & 0x10 != 0 || status.inexact)
            && source.floor_log2() < 1 - bias
        {
            flags |= 0x10;
        }
        return Converted {
            raw,
            flags,
            rounded: Some(rounded),
            quantum: false,
            incremented: magnitude_increased(raw, target, source),
        };
    }
    let (precision, bias, expbits) = float::decimal_shape(target.width);
    let max_exp = (3i32 << expbits) - 1 - bias;
    let rounded = if source.n.is_zero() {
        source.clone()
    } else {
        let mut exponent = decimal::floor_log10(source) - (precision as i32 - 1);
        let (mut q, _) = decimal::quantize(source, exponent, rounding);
        if q.to_str_radix(10).len() > precision {
            q /= 10u8;
            exponent += 1;
        }
        decimal_ratio(source.negative, q, exponent)
    };
    let preferred = if controls & 0x10 != 0 { 0 } else { max_exp };
    let (raw, _, flags, _) =
        decimal::encode(source, preferred, target.width, rounding, 0, false, None);
    let mut flags = flags & 0xf8;
    if !source.n.is_zero()
        && masks & 0x10 != 0
        && decimal::floor_log10(source) < -bias + precision as i32 - 1
    {
        flags |= 0x10;
    }
    let quantum =
        flags & 8 != 0 || matches!(decimal::decode(raw,target.width),(Value::Finite(_),e) if e>0);
    Converted {
        raw,
        flags,
        rounded: Some(rounded),
        quantum,
        incremented: magnitude_increased(raw, target, source),
    }
}
fn special(source: &Value, raw_source: u128, from: Shape, to: Shape) -> Converted {
    let negative = super::negative(source);
    if to.format == Format::Hex {
        return Converted {
            raw: hex_max(negative && !super::nan(source), to.width),
            flags: 0x80,
            rounded: None,
            quantum: false,
            incremented: false,
        };
    }
    let sign = u128::from(negative) << (to.width * 8 - 1);
    if matches!(source, Value::Infinity(_)) {
        let raw = if to.format == Format::Binary {
            let (f, b) = float::binary_shape(to.width);
            sign | ((2 * b + 1) as u128) << f
        } else {
            sign | (30 << (to.width * 8 - 6))
        };
        return Converted {
            raw,
            flags: 0,
            rounded: None,
            quantum: false,
            incremented: false,
        };
    }
    let mut payload = if from.format == Format::Decimal {
        decimal::coefficient(raw_source, from.width)
    } else {
        let (f, _) = float::binary_shape(from.width);
        BigUint::from(raw_source & ((1u128 << (f - 1)) - 1))
    };
    let raw = if to.format == Format::Binary {
        let (f, b) = float::binary_shape(to.width);
        if payload >= (BigUint::one() << (f - 1)) {
            payload = BigUint::zero();
        }
        sign | ((2 * b + 1) as u128) << f | (1u128 << (f - 1)) | payload.to_u128().unwrap()
    } else {
        let (p, _, _) = float::decimal_shape(to.width);
        if payload >= BigUint::from(10u8).pow(p as u32 - 1) {
            payload = BigUint::zero();
        }
        let trailing = (p - 1) / 3 * 10;
        sign | (31 << (to.width * 8 - 6))
            | (float::pack_decimal(false, &payload, 0, to.width) & ((1u128 << trailing) - 1))
    };
    Converted {
        raw,
        flags: if signaling(source) { 0x80 } else { 0 },
        rounded: None,
        quantum: false,
        incremented: false,
    }
}
fn scaled(rounded: &Ratio, target: Shape) -> (u128, i32) {
    let order = match target.format {
        Format::Binary => rounded.floor_log2(),
        Format::Hex => rounded.floor_log2().div_euclid(4),
        Format::Decimal => decimal::floor_log10(rounded),
    };
    let omega = -order;
    let mut scaled = rounded.clone();
    if target.format == Format::Decimal {
        if omega >= 0 {
            scaled.n *= BigUint::from(10u8).pow(omega as u32);
        } else {
            scaled.d *= BigUint::from(10u8).pow(omega.unsigned_abs());
        }
    } else {
        let shift = omega * if target.format == Format::Hex { 4 } else { 1 };
        if shift >= 0 {
            scaled.n <<= shift as usize;
        } else {
            scaled.d <<= shift.unsigned_abs() as usize;
        }
    }
    let raw = match target.format {
        Format::Hex => hex::pack(&scaled, target.width, None, Rounding::Zero, 0).0,
        Format::Binary => {
            float::encode_binary(
                &scaled,
                target.width,
                float::binary_shape(target.width).0,
                Rounding::Zero,
            )
            .0
        }
        Format::Decimal => {
            decimal::encode(
                &scaled,
                -float::decimal_shape(target.width).1,
                target.width,
                Rounding::Zero,
                0,
                false,
                None,
            )
            .0
        }
    };
    (raw, omega)
}
fn trap(state: &mut State, dxc: u8) -> String {
    state.control = (state.control & !0xff00) | (u32::from(dxc) << 8);
    format!("PFPO IEEE exception DXC={dxc:02X}")
}

pub(super) fn execute(low: &mut [u32; 16], state: &mut State, cc: &mut u8) -> Result<(), String> {
    let code = low[0];
    let target = shape((code >> 16) as u8);
    let source = shape((code >> 8) as u8);
    let controls = code as u8;
    let rm = controls & 15;
    let valid = code & 0x7f000000 == 0x01000000
        && (rm <= 1 || rm >= 8)
        && matches!((source,target),(Some(s),Some(t)) if s.format!=t.format && (t.format!=Format::Binary || controls&0x30==0));
    if code & 0x80000000 != 0 {
        low[1] = 0;
        *cc = if valid { 0 } else { 3 };
        return Ok(());
    }
    if !valid {
        return Err("PFPO specification exception: invalid function or controls".into());
    }
    let (from, to) = (source.unwrap(), target.unwrap());
    let rounding = match rm {
        0 => float::convert::decimal_rounding(0, state.control),
        1 => mode(state.control)?,
        _ => float::convert::decimal_rounding(u32::from(rm), state.control),
    };
    let raw_source = read(state, 4, from.width);
    let source = value(raw_source, from);
    let masks = (state.control >> 24) as u8;
    let converted = match &source {
        Value::Finite(v) => finite(v, to, rounding, controls, masks),
        _ => special(&source, raw_source, from, to),
    };
    if converted.flags & 0x80 != 0 {
        if masks & 0x80 != 0 {
            return Err(trap(state, 0x80));
        }
        write(state, 0, to.width, converted.raw);
        low[1] = 0;
        state.control |= 0x00800000;
        *cc = 1;
        return Ok(());
    }
    let range = converted.flags & masks & 0x30;
    if range != 0 {
        let rounded = converted
            .rounded
            .as_ref()
            .ok_or("missing PFPO scaled value")?;
        let source = match &source {
            Value::Finite(v) => v,
            _ => return Err("invalid PFPO scaled source".into()),
        };
        let (raw, omega) = scaled(rounded, to);
        let dxc = if range & 0x20 != 0 { 0x20 } else { 0x10 }
            | if different(rounded, source) {
                8 | if increased(rounded, source) { 4 } else { 0 }
            } else {
                0
            };
        write(state, 0, to.width, raw);
        low[1] = omega as u32;
        let error = trap(state, dxc);
        if controls & 0x40 != 0 {
            *cc = 2;
            return Ok(());
        } else {
            return Err(error);
        }
    }
    write(state, 0, to.width, converted.raw);
    low[1] = 0;
    state.control |= u32::from(converted.flags & 0x30) << 16;
    let inexact = converted.flags & 8 != 0 && controls & 0x80 == 0;
    if inexact && masks & 8 != 0 {
        return Err(trap(state, 8 | if converted.incremented { 4 } else { 0 }));
    }
    if inexact {
        state.control |= 0x00080000;
    }
    let quantum = to.format == Format::Decimal && controls & 0x20 != 0 && converted.quantum;
    if quantum && masks & 4 != 0 {
        return Err(trap(state, 4));
    }
    if quantum {
        state.control |= 0x00040000;
    }
    *cc = u8::from(converted.flags & 0x30 != 0 || inexact || quantum);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registers(target: u8, source: u8, controls: u8) -> [u32; 16] {
        let mut low = [0; 16];
        low[0] =
            0x01000000 | (u32::from(target) << 16) | (u32::from(source) << 8) | u32::from(controls);
        low[1] = 0x12345678;
        low
    }
    fn convert(
        source: u128,
        from: u8,
        to: u8,
        controls: u8,
        fpc: u32,
    ) -> (State, [u32; 16], u8, Result<(), String>) {
        let mut state = State {
            control: fpc,
            ..State::default()
        };
        write(&mut state, 4, shape(from).unwrap().width, source);
        let mut low = registers(to, from, controls);
        let mut cc = 3;
        let result = execute(&mut low, &mut state, &mut cc);
        (state, low, cc, result)
    }

    #[test]
    fn test_bit_checks_all_functions_and_controls_without_touching_floating_state() {
        let mut valid = 0;
        for source in 0..=11 {
            for target in 0..=11 {
                for controls in 0..=255 {
                    let mut low = registers(target, source, controls);
                    low[0] |= 0x80000000;
                    let mut state = State {
                        registers: [u64::MAX; 16],
                        control: 0xfcfcff77,
                        program_mask: 15,
                    };
                    let mut cc = 2;
                    execute(&mut low, &mut state, &mut cc).unwrap();
                    let supported = matches!((shape(source),shape(target)), (Some(s),Some(t)) if s.format != t.format)
                        && (controls & 15 <= 1 || controls & 15 >= 8)
                        && (shape(target).is_some_and(|t| t.format != Format::Binary)
                            || controls & 0x30 == 0);
                    assert_eq!(
                        cc,
                        if supported { 0 } else { 3 },
                        "{target:02x} {source:02x} {controls:02x}"
                    );
                    assert_eq!(low[1], 0);
                    assert_eq!(state.control, 0xfcfcff77);
                    assert_eq!(state.registers, [u64::MAX; 16]);
                    if controls == 8 && supported {
                        valid += 1;
                    }
                }
            }
        }
        assert_eq!(valid, 54);
        let mut low = registers(5, 9, 8);
        low[0] ^= 0x01000000;
        assert!(
            execute(&mut low, &mut State::default(), &mut 0)
                .unwrap_err()
                .contains("specification")
        );
    }

    #[test]
    fn signaling_nan_trap_suppresses_result_return_code_and_cc() {
        let mut state = State {
            registers: [0x123456789abcdef0; 16],
            control: 0x80001200,
            ..State::default()
        };
        // Negative decimal64 SNaN with payload 42.
        write(&mut state, 4, 8, 0xfe00000000000042);
        let before = state.registers;
        let mut low = registers(5, 9, 8);
        let mut cc = 2;
        assert!(
            execute(&mut low, &mut state, &mut cc)
                .unwrap_err()
                .contains("DXC=80")
        );
        assert_eq!(state.registers, before);
        assert_eq!(low[1], 0x12345678);
        assert_eq!(cc, 2);
        assert_eq!(state.control, 0x80008000);
    }

    #[test]
    fn nan_payloads_are_numbers_instead_of_bit_strings_and_overflow_defaults_to_zero() {
        // DPD 42 is encoded 0x42, but its binary payload is 0x2a.
        let (state, low, cc, result) = convert(0xfe00000000000042, 9, 5, 8, 0);
        result.unwrap();
        assert_eq!(read(&state, 0, 4), 0xffc0002a);
        assert_eq!((low[1], cc, state.control), (0, 1, 0x00800000));
        let (state, _, cc, result) = convert(0xffc0002a, 5, 9, 8, 0);
        result.unwrap();
        assert_eq!(read(&state, 0, 8), 0xfc00000000000042);
        assert_eq!((cc, state.control), (0, 0));
        // Decimal32 has room for six decimal payload digits, fewer than this
        // binary64 payload's numeric value 1,000,000.
        let (state, _, cc, result) = convert(0x7ff80000000f4240, 6, 8, 8, 0);
        result.unwrap();
        assert_eq!(read(&state, 0, 4), 0x7c000000);
        assert_eq!(cc, 0);
        let (state, _, cc, result) = convert(0xfff0000000000000, 6, 0, 8, 0);
        result.unwrap();
        assert_eq!(read(&state, 0, 4), 0xffff_ffff);
        assert_eq!((cc, state.control), (1, 0x00800000));
    }

    #[test]
    fn binary_range_traps_return_scaled_values_and_signed_exponents() {
        // HFP64 16**62 = 2**248, exactly representable at BFP32 precision.
        let (state, low, cc, result) = convert(0x7f10000000000000, 1, 5, 0x48, 0x20000000);
        result.unwrap();
        assert_eq!(read(&state, 0, 4), 0x3f800000);
        assert_eq!(low[1] as i32, -248);
        assert_eq!((cc, state.control), (2, 0x20002000));
        // HFP64 16**-65 = 2**-260, with a sign preserved in scaled form.
        let (state, low, cc, result) = convert(0x8010000000000000, 1, 5, 0xc8, 0x10000000);
        result.unwrap();
        assert_eq!(read(&state, 0, 4), 0xbf800000);
        assert_eq!(low[1] as i32, 260);
        assert_eq!((cc, state.control), (2, 0x10001000));
        let (state, low, cc, result) = convert(0x7f10000000000000, 1, 5, 8, 0x20000000);
        assert!(result.unwrap_err().contains("DXC=20"));
        assert_eq!((low[1] as i32, cc, state.control), (-248, 3, 0x20002000));
        assert_eq!(read(&state, 0, 4), 0x3f800000);
    }

    #[test]
    fn binary_tininess_is_detected_before_rounding_up_to_normal() {
        // HFP64 exact 2**-126 - 2**-151. BFP32 rounds upward to Nmin,
        // but PFPO still recognizes underflow on the precise input.
        let source = hex_raw(false, (1u128 << 54) - (1u128 << 29), -31, 8);
        let (state, _, cc, result) = convert(source, 1, 5, 8, 0);
        result.unwrap();
        assert_eq!(read(&state, 0, 4), 0x00800000);
        assert_eq!((cc, state.control), (1, 0x00180000));
        let (state, _, cc, result) = convert(source, 1, 5, 0x88, 0);
        result.unwrap();
        assert_eq!((cc, state.control), (1, 0x00100000));
    }

    #[test]
    fn decimal_preferred_quantum_and_quantum_permission_are_independent() {
        // Exact binary64 100.0.
        for (controls, expected, fpc, cc) in [
            (8, 0x22700001, 0, 0),    // 1E2, greatest quantum
            (0x18, 0x22500080, 0, 0), // 100E0, preferred quantum 1
            (0x28, 0x22700001, 0x00040000, 1),
            (0x38, 0x22500080, 0, 0),
        ] {
            let (state, low, actual_cc, result) = convert(0x4059000000000000, 6, 8, controls, 0);
            result.unwrap();
            assert_eq!(read(&state, 0, 4), expected, "controls={controls:02x}");
            assert_eq!((low[1], actual_cc, state.control), (0, cc, fpc));
        }
        // Inexact binary64 1/3; IS suppresses inexact, while DQPC still
        // recognizes quantum loss independently.
        let (state, _, cc, result) = convert(0x3fd5555555555555, 6, 8, 0xa8, 0);
        result.unwrap();
        assert_eq!((cc, state.control), (1, 0x00040000));
        let (state, low, cc, result) = convert(0x4059000000000000, 6, 8, 0x28, 0x04000000);
        assert!(result.unwrap_err().contains("DXC=04"));
        assert_eq!(read(&state, 0, 4), 0x22700001);
        assert_eq!((low[1], cc, state.control), (0, 3, 0x04000400));
    }

    #[test]
    fn hex_range_controls_choose_invalid_overflow_or_silent_underflow() {
        // BFP64 2**1000, far above all HFP formats.
        for (controls, fpc, cc) in [(8, 0x00800000, 1), (0x28, 0x00280000, 1)] {
            let (state, low, actual_cc, result) = convert(0x7e70000000000000, 6, 0, controls, 0);
            result.unwrap();
            assert_eq!(read(&state, 0, 4), 0x7fffffff);
            assert_eq!((low[1], actual_cc, state.control), (0, cc, fpc));
        }
        for (controls, fpc, cc) in [(8, 0x00080000, 1), (0x88, 0, 0), (0x98, 0x00100000, 1)] {
            let (state, _, actual_cc, result) = convert(0x8170000000000000, 6, 2, controls, 0);
            result.unwrap();
            assert_eq!(read(&state, 0, 16), 0x80000000000000008000000000000000);
            assert_eq!((actual_cc, state.control), (cc, fpc));
        }
        let (state, low, cc, result) = convert(0x7e70000000000000, 6, 0, 0x68, 0x20000000);
        result.unwrap();
        assert_eq!(read(&state, 0, 4), 0x41100000);
        assert_eq!((low[1] as i32, cc, state.control), (-250, 2, 0x20002000));
    }
    #[test]
    fn inexact_range_scaling_uses_target_precision_and_ignores_inexact_suppression() {
        // Independent integer/decimal oracles: 2**1000 = 1.071508607...E301;
        // 2**-1000 = 9.332636185...E-302. Seven significant digits are retained.
        for (source, q, omega, masks, dxc) in [
            (0x7e70000000000000, 1071509u32, -301i32, 0x20u32, 0x2cu32),
            (0x0170000000000000, 9332636, 302, 0x10, 0x18),
        ] {
            for controls in [0x48, 0xc8] {
                let (state, low, cc, result) = convert(source, 6, 8, controls, masks << 24);
                result.unwrap();
                assert_eq!(
                    read(&state, 0, 4),
                    float::pack_decimal(false, &BigUint::from(q), -6, 4)
                );
                assert_eq!(low[1] as i32, omega);
                assert_eq!((cc, state.control), (2, (masks << 24) | (dxc << 8)));
            }
        }
    }

    #[test]
    fn inexact_traps_complete_but_take_precedence_over_quantum() {
        for (controls, mask, dxc, flags) in [
            (0x28, 0x0c000000, 0x08, 0),
            (0xa8, 0x0c000000, 0x04, 0),
            (0x28, 0x04000000, 0x04, 0x00080000),
        ] {
            let (state, low, cc, result) = convert(0x3fd5555555555555, 6, 8, controls, mask);
            assert!(result.unwrap_err().contains(&format!("DXC={dxc:02X}")));
            assert_eq!(
                read(&state, 0, 4),
                float::pack_decimal(false, &BigUint::from(3333333u32), -7, 4)
            );
            assert_eq!(
                (low[1], cc, state.control),
                (0, 3, mask | flags | (dxc << 8))
            );
        }
        // Conversion to short binary increments this exact decimal input.
        let source = float::pack_decimal(false, &BigUint::from(10000001u32), -7, 8);
        let (state, low, cc, result) = convert(source, 9, 5, 8, 0x08000000);
        assert!(result.unwrap_err().contains("DXC=0C"));
        assert_eq!(
            (read(&state, 0, 4), low[1], cc, state.control),
            (0x3f800001, 0, 3, 0x08000c00)
        );
    }
}
