//! HFP arithmetic, with one hexadecimal guard digit for add/subtract.
use super::{State, Value, cc, compare, read, write};
use crate::hlasm::{
    DynamicStorage, Instruction,
    float::{
        Ratio, Rounding,
        operations::{Hex, Kind},
    },
};
use num_traits::{One, ToPrimitive, Zero};

pub(super) fn decode(raw: u128, width: usize) -> (Ratio, i32) {
    let precision = if width == 16 { 112 } else { width * 8 - 8 };
    let negative = raw >> (width * 8 - 1) != 0;
    let exponent = ((raw >> (width * 8 - 8)) & 127) as i32 - 64;
    let fraction = if width == 16 {
        ((raw >> 64) & 0x00ff_ffff_ffff_ffff) << 56 | (raw & 0x00ff_ffff_ffff_ffff)
    } else {
        raw & ((1u128 << precision) - 1)
    };
    (
        Ratio::binary(negative, fraction, 4 * exponent - precision as i32),
        exponent,
    )
}

pub(super) fn pack(
    value: &Ratio,
    width: usize,
    exponent: Option<i32>,
    rounding: Rounding,
    program_mask: u8,
) -> (u128, Option<&'static str>) {
    if value.n.is_zero() {
        return (0, None);
    }
    let precision = if width == 16 { 112 } else { width * 8 - 8 };
    let mut e = exponent.unwrap_or_else(|| value.floor_log2().div_euclid(4) + 1);
    let (mut q, _) = value.quantize(4 * e - precision as i32, rounding);
    if q.bits() > precision as u64 {
        q >>= 4usize;
        e += 1;
    }
    // The default HFP exponent-underflow program mask is zero. Underflow
    // produces a true zero; overflow is always an interrupting condition.
    if e < -64 && program_mask & 2 == 0 {
        return (0, None);
    }
    let q = q.to_u128().unwrap();
    let overflow = e > 63;
    let characteristic = ((e + 64) & 127) as u128;
    let raw = if width == 16 {
        let hi = (u128::from(value.negative) << 63) | (characteristic << 56) | (q >> 56);
        let lo = (u128::from(value.negative) << 63)
            | (((characteristic + 114) & 127) << 56)
            | (q & 0x00ff_ffff_ffff_ffff);
        (hi << 64) | lo
    } else {
        (u128::from(value.negative) << (width * 8 - 1)) | (characteristic << (width * 8 - 8)) | q
    };
    (
        raw,
        if overflow {
            Some("HFP exponent-overflow exception")
        } else if e < -64 {
            Some("HFP exponent-underflow exception")
        } else {
            None
        },
    )
}

pub(super) fn execute(
    ins: &Instruction,
    spec: Hex,
    low: &[u32; 16],
    mem: &[u8],
    _storage: &DynamicStorage,
    state: &mut State,
    condition: &mut u8,
) -> Result<(), String> {
    let reg = |n: usize| -> Result<usize, String> {
        ins.registers
            .get(n)
            .copied()
            .filter(|r| *r < 16)
            .map(usize::from)
            .ok_or_else(|| "invalid HFP register".into())
    };
    let r = reg(0)?;
    crate::hlasm::float::operations::check_register(r as u8, spec.width.max(spec.destination))?;
    let source = if spec.memory {
        let at = super::super::address(ins.address.as_ref().ok_or("missing HFP address")?, low)?;
        let area = super::super::range(mem, at, spec.width)?;
        let mut bytes = [0u8; 16];
        bytes[16 - spec.width..].copy_from_slice(&mem[area]);
        u128::from_be_bytes(bytes)
    } else {
        let s = reg(1)?;
        crate::hlasm::float::operations::check_register(s as u8, spec.width)?;
        read(state, s, spec.width)
    };
    if spec.kind == Kind::Lengthen {
        let hi = if spec.width == 4 {
            source << 32
        } else {
            source
        };
        let raw = if spec.destination == 8 {
            hi
        } else {
            let sign = hi & (1 << 63);
            if hi & 0x00ff_ffff_ffff_ffff == 0 {
                (sign << 64) | sign
            } else {
                (hi << 64) | sign | (((((hi >> 56) & 127) + 114) & 127) << 56)
            }
        };
        write(state, r, spec.destination, raw);
        return Ok(());
    }
    let (mut b, eb) = decode(source, spec.width);
    if matches!(
        spec.kind,
        Kind::Positive | Kind::Negative | Kind::Complement | Kind::Test
    ) {
        let sign = 1u128 << (spec.width * 8 - 1);
        let raw = match spec.kind {
            Kind::Positive => source & !sign,
            Kind::Negative => source | sign,
            Kind::Complement => source ^ sign,
            _ => source,
        };
        // Extended HFP sign operations make both signs agree; the low-order
        // characteristic and fraction retain their operand values.
        let raw = if spec.width == 16 {
            (raw & !(1u128 << 63)) | (((raw >> 127) & 1) << 63)
        } else {
            raw
        };
        *condition = cc(&Value::Finite(decode(raw, spec.width).0));
        write(state, r, spec.destination, raw);
        return Ok(());
    }
    let (a, ea) = decode(read(state, r, spec.width), spec.width);
    if spec.kind == Kind::Compare {
        *condition = compare(&Value::Finite(a), &Value::Finite(b));
        return Ok(());
    }
    let mut forced = None;
    let mut rounding = Rounding::Zero;
    let result = match spec.kind {
        Kind::RoundInteger => Ratio {
            negative: b.negative,
            n: &b.n / &b.d,
            d: num_bigint::BigUint::one(),
        },
        Kind::FusedAdd | Kind::FusedSubtract => {
            let other = read(state, reg(if spec.memory { 1 } else { 2 })?, spec.width);
            let (other, _) = decode(other, spec.width);
            let product = b.multiply(&other);
            let mut addend = a;
            if spec.kind == Kind::FusedSubtract {
                addend.negative = !addend.negative;
            }
            product.add(&addend, false)
        }
        Kind::Half => {
            b.d <<= 1usize;
            b
        }
        Kind::Sqrt => {
            if b.n.is_zero() {
                b.negative = false;
                b
            } else {
                if b.negative {
                    return Err("HFP square-root exception".into());
                }
                let precision = if spec.width == 16 {
                    112
                } else {
                    spec.width * 8 - 8
                };
                let exponent = b.floor_log2().div_euclid(2).div_euclid(4) + 1;
                let quantum = 4 * exponent - precision as i32;
                let (n, d) = if quantum < 0 {
                    (&b.n << (-2 * quantum) as usize, b.d.clone())
                } else {
                    (b.n.clone(), &b.d << (2 * quantum) as usize)
                };
                let mut q = (&n / &d).sqrt();
                if (&n << 2usize) >= ((&q << 1usize) + 1u8).pow(2) * &d {
                    q += 1u8;
                }
                Ratio::binary(false, q.to_u128().unwrap(), quantum)
            }
        }
        Kind::Convert => {
            forced = Some(eb);
            rounding = Rounding::NearestAway;
            b
        }
        Kind::Add | Kind::Subtract => {
            if spec.kind == Kind::Subtract {
                b.negative = !b.negative;
            }
            let exponent = ea.max(eb);
            let precision = if spec.width == 16 {
                112
            } else {
                spec.width * 8 - 8
            };
            let quantum = 4 * exponent - precision as i32 - 4;
            let a = Ratio::binary(
                a.negative,
                a.quantize(quantum, Rounding::Zero).0.to_u128().unwrap(),
                quantum,
            );
            let b = Ratio::binary(
                b.negative,
                b.quantize(quantum, Rounding::Zero).0.to_u128().unwrap(),
                quantum,
            );
            if spec.unnormalized {
                forced = Some(exponent);
            }
            a.add(&b, false)
        }
        Kind::Multiply => a.multiply(&b),
        Kind::Divide => {
            if b.n.is_zero() {
                return Err("HFP floating-point-divide exception".into());
            }
            a.divide(&b)
        }
        _ => return Err("unsupported HFP operation".into()),
    };
    let (mut raw, mut exception) = pack(
        &result,
        spec.destination,
        forced,
        rounding,
        state.program_mask,
    );
    if matches!(spec.kind, Kind::Add | Kind::Subtract)
        && result.n.is_zero()
        && state.program_mask & 1 != 0
    {
        let exponent = ea.max(eb);
        let characteristic = ((exponent + 64) & 127) as u128;
        raw = if spec.destination == 16 {
            (characteristic << 120) | (((characteristic + 114) & 127) << 56)
        } else {
            characteristic << (spec.destination * 8 - 8)
        };
        exception = Some("HFP significance exception");
    }
    write(state, r, spec.destination, raw);
    if matches!(spec.kind, Kind::Add | Kind::Subtract) {
        *condition = cc(&Value::Finite(decode(raw, spec.destination).0));
    }
    if let Some(exception) = exception {
        return Err(exception.into());
    }
    Ok(())
}

pub(super) fn unnormalized(
    ins: &Instruction,
    spec: crate::hlasm::float::operations::HexUnnormalized,
    low: &[u32; 16],
    mem: &[u8],
    state: &mut State,
) -> Result<(), String> {
    use crate::hlasm::float::operations::HexPart;
    let reg = |n: usize| {
        ins.registers
            .get(n)
            .copied()
            .filter(|r| *r < 16)
            .map(usize::from)
            .ok_or_else(|| "invalid HFP register".to_string())
    };
    let r = reg(0)?;
    if spec.part == HexPart::Full && !spec.add {
        crate::hlasm::float::operations::check_register(r as u8, 16)?;
    }
    let b = read(state, reg(1)?, 8);
    let c = if spec.memory {
        let at = super::super::address(ins.address.as_ref().ok_or("missing HFP address")?, low)?;
        u128::from(u64::from_be_bytes(
            mem[super::super::range(mem, at, 8)?].try_into().unwrap(),
        ))
    } else {
        read(state, reg(2)?, 8)
    };
    let exponent = (((b >> 56) & 127) as i32 - 64) + (((c >> 56) & 127) as i32 - 64);
    let fraction = (b & 0x00ff_ffff_ffff_ffff) * (c & 0x00ff_ffff_ffff_ffff);
    let quantum = 4 * exponent - 112;
    let product = Ratio::binary((b ^ c) & (1 << 63) != 0, fraction, quantum);
    let result = if spec.add {
        product.add(&decode(read(state, r, 8), 8).0, false)
    } else {
        product
    };
    let q = result.quantize(quantum, Rounding::Zero).0
        & ((num_bigint::BigUint::one() << 112usize) - 1u8);
    let q = q.to_u128().unwrap();
    let characteristic = ((exponent + 64) & 127) as u128;
    let sign = u128::from(result.negative && (!spec.add || !result.n.is_zero())) << 63;
    let hi = sign | (characteristic << 56) | (q >> 56);
    let lo = sign | (((characteristic + 114) & 127) << 56) | (q & 0x00ff_ffff_ffff_ffff);
    match spec.part {
        HexPart::Full => write(
            state,
            if spec.add { r & !2 } else { r },
            16,
            (hi << 64) | lo,
        ),
        HexPart::High => write(state, r, 8, hi),
        HexPart::Low => write(state, r, 8, lo),
    }
    Ok(())
}
