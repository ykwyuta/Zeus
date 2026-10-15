//! Exact source floating-point conversion. All rounding decisions use integers;
//! no host floating-point arithmetic participates in the assembled bits.
use super::{assemble::Symbols, constants};
use num_bigint::{BigInt, BigUint};
use num_traits::{One, ToPrimitive, Zero};
use std::cmp::Ordering;
pub(super) mod convert;
pub(super) mod operations;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Format {
    Hex,
    Binary,
    Decimal,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Rounding {
    NearestAway,
    NearestEven,
    Zero,
    Positive,
    Negative,
    NearestZero,
    Away,
    Prepare,
}

impl Rounding {
    fn source(n: u32, format: Format) -> Result<Self, String> {
        use Rounding::*;
        if format == Format::Decimal && !(8..=15).contains(&n) {
            return Err("decimal floating-point rounding mode must be in 8..15".into());
        }
        Ok(match (format, n) {
            (Format::Decimal, 8) | (_, 4) => NearestEven,
            (Format::Decimal, 9) | (_, 5) => Zero,
            (Format::Decimal, 10) | (_, 6) => Positive,
            (Format::Decimal, 11) | (_, 7) => Negative,
            (Format::Decimal, 12) | (_, 1) => NearestAway,
            (Format::Decimal, 13) => NearestZero,
            (Format::Decimal, 14) => Away,
            (Format::Decimal, 15) => Prepare,
            _ => return Err("invalid floating-point rounding mode".into()),
        })
    }
}

/// Finite magnitude, with sign retained independently to preserve signed zero.
#[derive(Clone, Debug)]
pub(super) struct Ratio {
    pub negative: bool,
    pub n: BigUint,
    pub d: BigUint,
}

impl Ratio {
    pub fn add(&self, other: &Self, zero_negative: bool) -> Self {
        let a = &self.n * &other.d;
        let b = &other.n * &self.d;
        let d = &self.d * &other.d;
        let (negative, n) = if self.negative == other.negative {
            (self.negative, a + b)
        } else if a > b {
            (self.negative, a - b)
        } else if a < b {
            (other.negative, b - a)
        } else {
            (zero_negative, BigUint::zero())
        };
        Self { negative, n, d }
    }
    pub fn multiply(&self, other: &Self) -> Self {
        Self {
            negative: self.negative ^ other.negative,
            n: &self.n * &other.n,
            d: &self.d * &other.d,
        }
    }
    pub fn divide(&self, other: &Self) -> Self {
        Self {
            negative: self.negative ^ other.negative,
            n: &self.n * &other.d,
            d: &self.d * &other.n,
        }
    }
    pub fn binary(negative: bool, significand: u128, exponent: i32) -> Self {
        let mut v = Self {
            negative,
            n: significand.into(),
            d: BigUint::one(),
        };
        if exponent >= 0 {
            v.n <<= exponent as usize;
        } else {
            v.d <<= exponent.unsigned_abs() as usize;
        }
        v
    }

    fn decimal(text: &str, modifier: i32) -> Result<(Self, String, i32), String> {
        let negative = text.starts_with('-');
        let text = text.strip_prefix(['+', '-']).unwrap_or(text);
        let (digits, exponent) = if let Some((v, e)) = text.split_once(['E', 'e']) {
            (
                v,
                e.parse::<BigInt>()
                    .map_err(|_| "invalid floating-point exponent")?,
            )
        } else {
            (text, BigInt::zero())
        };
        let mut point = None;
        let mut coefficient = String::new();
        for c in digits.chars() {
            if c == '.' && point.is_none() {
                point = Some(0i32);
            } else if c.is_ascii_digit() {
                coefficient.push(c);
                if let Some(n) = &mut point {
                    *n += 1;
                }
            } else {
                return Err("floating-point nominal requires decimal digits".into());
            }
        }
        if coefficient.is_empty() || coefficient.len() > 65536 {
            return Err("floating-point nominal requires 1..65536 decimal digits".into());
        }
        // Combine the nominal exponent before bounding it. In particular,
        // an exponent just outside i32 may cancel an opposite modifier.
        // Beyond i32 the bounded representative has the same range result
        // for every supported format and permitted coefficient length.
        let exponent = (exponent + modifier - point.unwrap_or(0))
            .clamp(BigInt::from(i32::MIN), BigInt::from(i32::MAX))
            .to_i32()
            .unwrap();
        let mut n = BigUint::parse_bytes(coefficient.as_bytes(), 10).unwrap();
        let mut d = BigUint::one();
        let order = n.to_str_radix(10).len() as i64 + i64::from(exponent);
        // Outside every supported format, a bounded representative gives the
        // same overflow/underflow and directed-rounding decision.
        if n.is_zero() {
        } else if order > 20000 {
            n = BigUint::from(10u8).pow(20000);
        } else if order < -20000 {
            n = BigUint::one();
            d = BigUint::from(10u8).pow(20000);
        } else if exponent >= 0 {
            n *= BigUint::from(10u8).pow(exponent as u32);
        } else {
            d = BigUint::from(10u8).pow(exponent.unsigned_abs());
        }
        Ok((Self { negative, n, d }, coefficient, exponent))
    }

    pub fn floor_log2(&self) -> i32 {
        let mut e = self.n.bits() as i32 - self.d.bits() as i32;
        let cmp = if e >= 0 {
            self.n.cmp(&(&self.d << e as usize))
        } else {
            (&self.n << e.unsigned_abs() as usize).cmp(&self.d)
        };
        if cmp == Ordering::Less {
            e -= 1;
        }
        e
    }

    pub fn quantize(&self, exponent: i32, rounding: Rounding) -> (BigUint, bool) {
        let (n, d) = if exponent >= 0 {
            (self.n.clone(), &self.d << exponent as usize)
        } else {
            (&self.n << exponent.unsigned_abs() as usize, self.d.clone())
        };
        round_quotient(&n, &d, self.negative, rounding, 2)
    }
}

pub(super) fn round_quotient(
    n: &BigUint,
    d: &BigUint,
    negative: bool,
    mode: Rounding,
    radix: u32,
) -> (BigUint, bool) {
    let mut q = n / d;
    let r = n % d;
    let inexact = !r.is_zero();
    let half = (&r << 1usize).cmp(d);
    let increase = inexact
        && match mode {
            Rounding::NearestAway => half != Ordering::Less,
            Rounding::NearestEven => {
                half == Ordering::Greater || (half == Ordering::Equal && q.bit(0))
            }
            Rounding::NearestZero => half == Ordering::Greater,
            Rounding::Zero => false,
            Rounding::Positive => !negative,
            Rounding::Negative => negative,
            Rounding::Away => true,
            Rounding::Prepare => {
                let last = (&q % radix).to_u32().unwrap();
                last == 0 || (radix == 10 && last == 5)
            }
        };
    if increase {
        q += 1u8;
    }
    (q, inexact)
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Flags {
    pub overflow: bool,
    pub underflow: bool,
    pub inexact: bool,
}

pub(super) fn binary_shape(width: usize) -> (u32, i32) {
    match width {
        4 => (23, 127),
        8 => (52, 1023),
        16 => (112, 16383),
        _ => unreachable!(),
    }
}

pub(super) fn encode_binary(
    v: &Ratio,
    width: usize,
    precision: u32,
    mode: Rounding,
) -> (u128, Flags) {
    let (fraction, bias) = binary_shape(width);
    let sign = u128::from(v.negative) << (width * 8 - 1);
    if v.n.is_zero() {
        return (sign, Flags::default());
    }
    let e = v.floor_log2();
    let min = 1 - bias;
    let mut quantum = e.max(min) - precision as i32;
    let (mut q, inexact) = v.quantize(quantum, mode);
    if q.bits() > u64::from(precision + 1) {
        q >>= 1usize;
        quantum += 1;
    }
    let actual = quantum + precision as i32;
    if actual > bias {
        let infinity = match mode {
            Rounding::Zero | Rounding::Prepare => false,
            Rounding::Positive => !v.negative,
            Rounding::Negative => v.negative,
            _ => true,
        };
        let all_exp = (2 * bias + 1) as u128;
        let magnitude = if infinity {
            all_exp << fraction
        } else {
            ((all_exp - 1) << fraction) | (((1u128 << precision) - 1) << (fraction - precision))
        };
        return (
            sign | magnitude,
            Flags {
                overflow: true,
                inexact: true,
                underflow: false,
            },
        );
    }
    let q = q.to_u128().unwrap();
    let normalized = q >= 1u128 << precision;
    let exp = if normalized {
        (actual + bias) as u128
    } else {
        0
    };
    let stored = (q & ((1u128 << precision) - 1)) << (fraction - precision);
    (
        sign | (exp << fraction) | stored,
        Flags {
            overflow: false,
            underflow: inexact && !normalized,
            inexact,
        },
    )
}

fn encode_hex(
    v: &Ratio,
    width: usize,
    bits: u32,
    scale: i32,
    mode: Rounding,
    extended: bool,
) -> Result<u128, String> {
    if v.n.is_zero() {
        let sign = u128::from(v.negative) << (width * 8 - 1);
        return Ok(sign
            | if extended {
                u128::from(v.negative) << 63
            } else {
                0
            });
    }
    let precision = if extended {
        bits.saturating_sub(16).min(112)
    } else {
        bits.saturating_sub(8).min(56)
    };
    if precision == 0 {
        return Err("hexadecimal constant needs fraction bits".into());
    }
    let mut e = (v.floor_log2().div_euclid(4) + 1 + scale).max(-64);
    let (mut fraction, _) = v.quantize(4 * e - precision as i32, mode);
    if fraction.bits() > u64::from(precision) {
        fraction >>= 4usize;
        e += 1;
    }
    let mut fraction = fraction.to_u128().unwrap();
    if e > 63 {
        e = 63;
        fraction = (1u128 << precision) - 1;
    }
    if extended {
        fraction <<= 112 - precision;
        let hi =
            (u64::from(v.negative) << 63) | (((e + 64) as u64) << 56) | (fraction >> 56) as u64;
        let low_fraction = fraction as u64 & 0x00ff_ffff_ffff_ffff;
        let lo = (u64::from(v.negative) << 63) | ((((e + 50) & 127) as u64) << 56) | low_fraction;
        Ok((u128::from(hi) << 64) | u128::from(lo))
    } else {
        let pad = width as u32 * 8 - 8 - precision;
        Ok((u128::from(v.negative) << (width * 8 - 1))
            | (((e + 64) as u128) << (width * 8 - 8))
            | (fraction << pad))
    }
}

/// Encode a decimal declet from three BCD digits according to the eight DPD
/// steering cases; the high bit of each digit selects the case.
pub(super) fn declet(value: u16) -> u16 {
    let (a, e, i) = (value / 100, value / 10 % 10, value % 10);
    let (b, c, d, f, g, h, j, k, m) = (
        (a >> 2) & 1,
        (a >> 1) & 1,
        a & 1,
        (e >> 2) & 1,
        (e >> 1) & 1,
        e & 1,
        (i >> 2) & 1,
        (i >> 1) & 1,
        i & 1,
    );
    let bits = match ((a >> 3) << 2) | ((e >> 3) << 1) | (i >> 3) {
        0 => [b, c, d, f, g, h, 0, j, k, m],
        1 => [b, c, d, f, g, h, 1, 0, 0, m],
        2 => [b, c, d, j, k, h, 1, 0, 1, m],
        3 => [b, c, d, 1, 0, h, 1, 1, 1, m],
        4 => [j, k, d, f, g, h, 1, 1, 0, m],
        5 => [f, g, d, 0, 1, h, 1, 1, 1, m],
        6 => [j, k, d, 0, 0, h, 1, 1, 1, m],
        _ => [0, 0, d, 1, 1, h, 1, 1, 1, m],
    };
    bits.into_iter().fold(0, |v, b| (v << 1) | b)
}

pub(super) fn decimal_shape(width: usize) -> (usize, i32, u32) {
    match width {
        4 => (7, 101, 6),
        8 => (16, 398, 8),
        16 => (34, 6176, 12),
        _ => unreachable!(),
    }
}

pub(super) fn pack_decimal(
    negative: bool,
    coefficient: &BigUint,
    exponent: i32,
    width: usize,
) -> u128 {
    let (precision, bias, expbits) = decimal_shape(width);
    let digits = format!("{:0>precision$}", coefficient.to_str_radix(10));
    let msd = u128::from(digits.as_bytes()[0] - b'0');
    let biased = (exponent + bias) as u128;
    let high = biased >> expbits;
    let combination = if msd < 8 {
        (high << 3) | msd
    } else {
        24 | (high << 1) | (msd & 1)
    };
    let mut continuation = 0u128;
    for triple in digits.as_bytes()[1..].chunks_exact(3) {
        let value = u16::from(triple[0] - b'0') * 100
            + u16::from(triple[1] - b'0') * 10
            + u16::from(triple[2] - b'0');
        continuation = (continuation << 10) | u128::from(declet(value));
    }
    let trailing = ((precision - 1) / 3) * 10;
    (u128::from(negative) << (width * 8 - 1))
        | (combination << (width * 8 - 6))
        | ((biased & ((1 << expbits) - 1)) << trailing)
        | continuation
}

pub(super) fn encode_decimal(
    negative: bool,
    coefficient: &str,
    mut exponent: i32,
    width: usize,
    mode: Rounding,
) -> u128 {
    let (precision, bias, expbits) = decimal_shape(width);
    let max_exp = (3i32 << expbits) - 1 - bias;
    let mut n = BigUint::parse_bytes(coefficient.as_bytes(), 10).unwrap();
    let significant = n.to_str_radix(10).len();
    let shift = (significant as i32 - precision as i32)
        .max((-bias).saturating_sub(exponent))
        .max(0);
    if shift > 0 {
        n = round_quotient(
            &n,
            &BigUint::from(10u8).pow(shift.min(significant as i32 + 1) as u32),
            negative,
            mode,
            10,
        )
        .0;
        exponent = exponent.saturating_add(shift);
    }
    if n.to_str_radix(10).len() > precision {
        n /= 10u8;
        exponent = exponent.saturating_add(1);
    }
    if exponent > max_exp && !n.is_zero() {
        let space = precision - n.to_str_radix(10).len();
        let shift = (exponent - max_exp).min(space as i32);
        n *= BigUint::from(10u8).pow(shift as u32);
        exponent -= shift;
        if exponent > max_exp {
            let finite = matches!(mode, Rounding::Zero | Rounding::Prepare)
                || matches!(mode, Rounding::Positive) && negative
                || matches!(mode, Rounding::Negative) && !negative;
            if finite {
                return pack_decimal(
                    negative,
                    &(BigUint::from(10u8).pow(precision as u32) - 1u8),
                    max_exp,
                    width,
                );
            }
            return (u128::from(negative) << (width * 8 - 1)) | (30u128 << (width * 8 - 6));
        }
    }
    pack_decimal(negative, &n, exponent.clamp(-bias, max_exp), width)
}

pub(super) struct Storage {
    pub bytes: Vec<u8>,
    pub alignment: u32,
    pub length: u32,
    pub bits: usize,
    pub warnings: Vec<String>,
}

pub(super) fn storage(
    spec: &str,
    initialized: bool,
    symbols: &Symbols,
) -> Result<Option<Storage>, String> {
    let (at, count) = constants::duplication(spec, symbols)?;
    let kind = spec.as_bytes()[at];
    if !matches!(kind, b'E' | b'D' | b'L') {
        return Ok(None);
    }
    let mut tail = &spec[at + 1..];
    let extension = tail
        .chars()
        .next()
        .filter(|c| matches!(c, 'B' | 'D' | 'H' | 'Q'));
    if extension.is_some() {
        tail = &tail[1..];
    }
    if extension == Some('Q') && kind != b'L' {
        return Err("Q extension requires L type".into());
    }
    let format = match extension {
        Some('B') => Format::Binary,
        Some('D') => Format::Decimal,
        _ => Format::Hex,
    };
    let native = match kind {
        b'E' => 4,
        b'D' => 8,
        _ => 16,
    };
    let mut length = None;
    let mut bit_length = false;
    if tail.starts_with('L') {
        bit_length = tail.starts_with("L.");
        if bit_length {
            tail = &tail[1..];
        }
        let input = if bit_length {
            format!("L{}", &tail[1..])
        } else {
            tail.to_string()
        };
        let (n, rest) = constants::modifier(&input, 'L', symbols)?;
        let used = input.len() - rest.len();
        tail = &tail[used..];
        length = Some(u32::try_from(n).map_err(|_| "floating-point length must be positive")?);
    }
    let scale_specified = tail.starts_with('S');
    let (scale, rest) = constants::modifier(tail, 'S', symbols)?;
    if scale_specified && format == Format::Decimal {
        return Err("scale is only valid for hexadecimal floating-point".into());
    }
    let (exponent, rest) = constants::modifier(rest, 'E', symbols)?;
    tail = rest;
    let bits = match length {
        None => native as u32 * 8,
        Some(n) if bit_length => n,
        Some(n) => n.checked_mul(8).ok_or("floating-point length overflow")?,
    };
    let max_bits = if kind == b'L' { 128 } else { 64 };
    let min_bits = if format == Format::Binary {
        match kind {
            b'E' => 9,
            b'D' => 12,
            _ => 16,
        }
    } else if extension.is_some() {
        12
    } else {
        1
    };
    if bits < min_bits
        || bits > max_bits
        || (format == Format::Decimal && bits != native as u32 * 8)
    {
        return Err("floating-point length is outside the format range".into());
    }
    if format == Format::Hex
        && !(0..=(if kind == b'L' {
            if extension == Some('Q') { 28 } else { 27 }
        } else if kind == b'D' {
            13
        } else {
            5
        }))
            .contains(&scale)
    {
        return Err("hexadecimal floating-point scale is out of range".into());
    }
    let align = if length.is_some() {
        1
    } else if extension == Some('Q') {
        16
    } else {
        native.min(8) as u32
    };
    let size = bits.div_ceil(8) as usize;
    let items = if let Some(value) = tail.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        value.split(',').map(str::trim).collect::<Vec<_>>()
    } else if tail.is_empty() && (!initialized || count == 0) {
        vec!["0"]
    } else {
        return Err(
            "floating-point nominal must be quoted or a parenthesized special value".into(),
        );
    };
    let total = size
        .checked_mul(items.len())
        .and_then(|n| n.checked_mul(count as usize))
        .filter(|n| *n <= 1 << 20)
        .ok_or("storage declaration exceeds 1 MiB")?;
    let total_bits = (bits as usize)
        .checked_mul(items.len())
        .and_then(|n| n.checked_mul(count as usize))
        .ok_or("storage declaration exceeds 1 MiB")?;
    let mut fields = Vec::new();
    let mut warnings = Vec::new();
    if scale_specified && format == Format::Binary {
        warnings.push("binary floating-point scale modifier is ignored".into());
    }
    for (item_index, item) in items.into_iter().enumerate() {
        if !initialized {
            fields.push(vec![0; size]);
            continue;
        }
        let normalized_item = item.to_ascii_uppercase();
        let item = normalized_item.as_str();
        let (value, rounding) = if let Some((v, r)) = item.rsplit_once('R') {
            if extension.is_none() {
                return Err("explicit HFP rounding requires a type extension".into());
            }
            (
                v,
                Rounding::source(
                    r.parse()
                        .map_err(|_| "invalid floating-point rounding mode")?,
                    format,
                )?,
            )
        } else {
            (
                item,
                if format == Format::Hex {
                    Rounding::NearestAway
                } else {
                    Rounding::NearestEven
                },
            )
        };
        let width = if format == Format::Hex && (kind != b'L' || bits <= 64) {
            size.max(2)
        } else {
            native
        };
        let special_value = value
            .strip_prefix(['+', '-'])
            .unwrap_or(value)
            .starts_with('(');
        let encoded = if special_value {
            if extension.is_none() {
                return Err("HFP special values require a type extension".into());
            }
            if format == Format::Binary
                && bits
                    < (match kind {
                        b'E' => 11,
                        b'D' => 14,
                        _ => 18,
                    })
            {
                return Err("binary special value requires two fraction bits".into());
            }
            special(value, format, width)?
        } else {
            if extension.is_none() {
                let nominal_exponent =
                    value.split_once('E').map_or(Ok(BigInt::zero()), |(_, e)| {
                        e.parse::<BigInt>()
                            .map_err(|_| "invalid floating-point exponent")
                    })?;
                if (nominal_exponent + exponent)
                    .to_i32()
                    .is_none_or(|n| !(-85..=75).contains(&n))
                {
                    return Err("legacy HFP exponent must be in -85..75".into());
                }
            }
            let (mut ratio, coefficient, power) = Ratio::decimal(value, exponent)?;
            if format == Format::Hex && extension != Some('H') && ratio.n.is_zero() {
                ratio.negative = false;
            }
            match format {
                Format::Binary => {
                    let (fraction, _) = binary_shape(native);
                    let removed = (native as u32 * 8).saturating_sub(bits);
                    let precision = fraction.saturating_sub(removed);
                    let (raw, flags) = encode_binary(&ratio, native, precision, rounding);
                    if count != 0 && flags.inexact {
                        let (_, bias) = binary_shape(native);
                        let max = Ratio::binary(
                            false,
                            (1u128 << (precision + 1)) - 1,
                            bias - precision as i32,
                        );
                        let min = Ratio::binary(false, 1, 1 - bias - precision as i32);
                        let reason = if &ratio.n * &max.d > &max.n * &ratio.d {
                            "exceeds the largest finite magnitude"
                        } else if !ratio.n.is_zero() && &ratio.n * &min.d < &min.n * &ratio.d {
                            "is below the smallest nonzero magnitude"
                        } else {
                            "cannot be represented exactly"
                        };
                        warnings.push(format!("binary floating-point nominal {} {reason}; rounded using the selected mode", item_index + 1));
                    }
                    raw
                }
                Format::Hex => encode_hex(
                    &ratio,
                    width,
                    bits.max(12),
                    scale,
                    rounding,
                    kind == b'L' && bits > 64,
                )?,
                Format::Decimal => {
                    encode_decimal(ratio.negative, &coefficient, power, native, rounding)
                }
            }
        };
        let raw = encoded.to_be_bytes();
        let mut encoded = raw[16 - width..].to_vec();
        encoded.resize(size, 0);
        encoded.truncate(size);
        if !bits.is_multiple_of(8) {
            let last = encoded.last_mut().unwrap();
            *last &= 0xff << (8 - bits % 8);
        }
        fields.push(encoded);
    }
    let mut bytes = Vec::with_capacity(total);
    let mut position = 0;
    for _ in 0..count {
        for field in &fields {
            super::bit_constants::append(&mut bytes, &mut position, field, bits as usize);
        }
    }
    debug_assert_eq!(position, total_bits);
    Ok(Some(Storage {
        bytes,
        alignment: align,
        length: bits.div_ceil(8),
        bits: total_bits,
        warnings,
    }))
}

fn special(value: &str, format: Format, width: usize) -> Result<u128, String> {
    let negative = value.starts_with('-');
    let value = value
        .strip_prefix(['+', '-'])
        .unwrap_or(value)
        .strip_prefix('(')
        .and_then(|v| v.strip_suffix(')'))
        .ok_or("invalid floating-point special")?;
    let sign = u128::from(negative) << (width * 8 - 1);
    match format {
        Format::Binary => {
            let (f, bias) = binary_shape(width);
            let all = ((2 * bias + 1) as u128) << f;
            Ok(sign
                | match value {
                    "INF" => all,
                    "NAN" => all | (1 << (f - 1)),
                    "QNAN" => all | (3 << (f - 2)),
                    "SNAN" => all | (1 << (f - 2)),
                    "MAX" => all - 1,
                    "MIN" => 1 << f,
                    "DMIN" => 1,
                    _ => return Err("unknown binary floating-point special".into()),
                })
        }
        Format::Decimal => {
            let (p, bias, e) = decimal_shape(width);
            Ok(match value {
                "INF" => sign | (30 << (width * 8 - 6)),
                "NAN" | "QNAN" => sign | (31 << (width * 8 - 6)),
                "SNAN" => sign | (31 << (width * 8 - 6)) | (1 << (width * 8 - 7)),
                "MAX" => pack_decimal(
                    negative,
                    &(BigUint::from(10u8).pow(p as u32) - 1u8),
                    (3 << e) - 1 - bias,
                    width,
                ),
                "MIN" => pack_decimal(
                    negative,
                    &BigUint::from(10u8).pow(p as u32 - 1),
                    -bias,
                    width,
                ),
                "DMIN" => pack_decimal(negative, &BigUint::one(), -bias, width),
                _ => return Err("unknown decimal floating-point special".into()),
            })
        }
        Format::Hex => {
            let precision = if width == 16 { 112 } else { width * 8 - 8 };
            let (fraction, exp) = match value {
                "MAX" => ((1u128 << precision) - 1, 63),
                "MIN" => (1u128 << (precision - 4), -64),
                "DMIN" => (1, -64),
                _ => return Err("unknown hexadecimal floating-point special".into()),
            };
            // Build via an exact ratio to share extended exponent placement.
            encode_hex(
                &Ratio::binary(negative, fraction, 4 * exp - precision as i32),
                width,
                width as u32 * 8,
                0,
                Rounding::Zero,
                width == 16,
            )
        }
    }
}
