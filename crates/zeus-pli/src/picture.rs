//! PICTURE specifications: character pictures (A X 9) and fixed-point
//! numeric pictures (9 Z * Y V , . / B $ S + - CR DB T I R F(n), drifting
//! strings). A numeric picture value is stored as its character image;
//! its arithmetic value is FIXED DECIMAL(p,q).

use serde::{Deserialize, Serialize};
use zeus_decimal::Dec;

fn pow10(n: u32) -> i128 {
    10i128.pow(n)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PicChar {
    Nine,
    Z,
    Star,
    Y,
    /// T, I, R: overpunched digit
    Over(char),
    V,
    /// , . / B (B: always a blank)
    Insert(char),
    /// static or drifting currency symbol
    Currency(String),
    /// S + - (static or drifting)
    Sign(char),
    /// CR or DB
    Credit(String),
    /// A X 9 of character pictures
    CharPos(char),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Picture {
    pub spec: String,
    pub items: Vec<PicChar>,
    pub numeric: bool,
    /// stored characters
    pub len: usize,
    /// digit positions
    pub p: u8,
    /// scale: digits after V minus F
    pub q: i16,
    /// F(n) scale factor
    pub f: i16,
    /// index range of the drifting string in `items`, if any
    pub drift: Option<(usize, usize)>,
    /// A floating-point picture: its mantissa and exponent pictures and
    /// whether the exponent is marked by E (stored) or K (not stored).
    #[serde(default)]
    pub float: Option<Box<FloatPic>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloatPic {
    pub mant: Picture,
    pub exp: Picture,
    pub e: bool,
}

impl Picture {
    pub fn parse(spec: &str) -> Result<Picture, String> {
        // expand (n)c repetitions
        let chars: Vec<char> = spec.chars().collect();
        let mut raw: Vec<char> = Vec::new();
        let mut i = 0;
        let mut f = 0i16;
        while i < chars.len() {
            let c = chars[i].to_ascii_uppercase();
            if c == '(' {
                let close = chars[i..]
                    .iter()
                    .position(|&x| x == ')')
                    .ok_or("unbalanced ( in picture")?
                    + i;
                let n: usize = chars[i + 1..close]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .parse()
                    .map_err(|_| "invalid repetition factor in picture")?;
                let next = *chars
                    .get(close + 1)
                    .ok_or("picture ends after repetition factor")?;
                if next == '<' {
                    let end = chars[close + 1..]
                        .iter()
                        .position(|&x| x == '>')
                        .ok_or("unclosed <")?
                        + close
                        + 1;
                    for _ in 0..n {
                        raw.extend(&chars[close + 1..=end]);
                    }
                    i = end + 1;
                } else {
                    raw.extend(std::iter::repeat_n(next.to_ascii_uppercase(), n));
                    i = close + 2;
                }
                continue;
            }
            if c == 'F' && chars.get(i + 1) == Some(&'(') {
                let close = chars[i..]
                    .iter()
                    .position(|&x| x == ')')
                    .ok_or("unbalanced F(")?
                    + i;
                f = chars[i + 2..close]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .trim_start_matches('+')
                    .parse()
                    .map_err(|_| "invalid F scale factor")?;
                i = close + 1;
                continue;
            }
            raw.push(chars[i]);
            i += 1;
        }
        // a floating-point picture: mantissa E|K exponent
        let mut depth = 0;
        let mut split = None;
        for (k, &c) in raw.iter().enumerate() {
            match c.to_ascii_uppercase() {
                '<' => depth += 1,
                '>' => depth -= 1,
                'E' | 'K' if depth == 0 => {
                    split = Some(k);
                    break;
                }
                _ => {}
            }
        }
        if let Some(k) = split {
            let ms: String = raw[..k].iter().collect();
            let es: String = raw[k + 1..].iter().collect();
            let mant = Picture::parse(&ms)?;
            let exp = Picture::parse(&es)?;
            if !mant.numeric || !exp.numeric || mant.float.is_some() || exp.float.is_some() {
                return Err("invalid floating-point picture".into());
            }
            if exp.q != 0 {
                return Err("the exponent of a floating-point picture has no V".into());
            }
            let e = raw[k].eq_ignore_ascii_case(&'E');
            return Ok(Picture {
                spec: spec.to_string(),
                items: mant.items.clone(),
                numeric: true,
                len: mant.len + usize::from(e) + exp.len,
                p: mant.p,
                q: mant.q,
                f: 0,
                drift: mant.drift,
                float: Some(Box::new(FloatPic { mant, exp, e })),
            });
        }
        let character = raw
            .iter()
            .any(|c| matches!(c.to_ascii_uppercase(), 'A' | 'X'));
        if character {
            let mut items = Vec::new();
            for c in &raw {
                let c = c.to_ascii_uppercase();
                if !matches!(c, 'A' | 'X' | '9') {
                    return Err(format!("invalid character '{c}' in a character picture"));
                }
                items.push(PicChar::CharPos(c));
            }
            return Ok(Picture {
                spec: spec.to_string(),
                len: items.len(),
                items,
                numeric: false,
                p: 0,
                q: 0,
                f: 0,
                drift: None,
                float: None,
            });
        }
        let mut items = Vec::new();
        let mut i = 0;
        while i < raw.len() {
            let c = raw[i].to_ascii_uppercase();
            let it = match c {
                '9' => PicChar::Nine,
                'Z' => PicChar::Z,
                '*' => PicChar::Star,
                'Y' => PicChar::Y,
                'T' | 'I' | 'R' => PicChar::Over(c),
                'V' => PicChar::V,
                ',' | '.' | '/' | 'B' | '\'' => PicChar::Insert(c),
                '$' => PicChar::Currency("$".into()),
                '<' => {
                    let end = raw[i..]
                        .iter()
                        .position(|&x| x == '>')
                        .ok_or("unclosed < in picture")?
                        + i;
                    let text: String = raw[i + 1..end].iter().collect();
                    i = end + 1;
                    items.push(PicChar::Currency(text));
                    continue;
                }
                'S' | '+' | '-' => PicChar::Sign(c),
                'C' | 'D'
                    if matches!(
                        raw.get(i + 1).map(|x| x.to_ascii_uppercase()),
                        Some('R' | 'B')
                    ) =>
                {
                    let s: String = raw[i..i + 2]
                        .iter()
                        .collect::<String>()
                        .to_ascii_uppercase();
                    i += 2;
                    items.push(PicChar::Credit(s));
                    continue;
                }
                _ => return Err(format!("invalid character '{c}' in a numeric picture")),
            };
            items.push(it);
            i += 1;
        }
        // drifting string: two or more of the same $ S + - with insertion
        // characters among them
        let mut drift = None;
        let sym = |p: &PicChar| match p {
            PicChar::Currency(s) => Some(format!("${s}")),
            PicChar::Sign(c) => Some(c.to_string()),
            _ => None,
        };
        let mut k = 0;
        while k < items.len() {
            if let Some(s) = sym(&items[k]) {
                let mut j = k + 1;
                let mut count = 1;
                while j < items.len() {
                    match &items[j] {
                        x if sym(x).as_deref() == Some(&s) => count += 1,
                        PicChar::Insert(_) => {}
                        PicChar::V => {}
                        _ => break,
                    }
                    j += 1;
                }
                // trailing insertions / V do not end inside the string
                while j > k + 1 && matches!(items[j - 1], PicChar::Insert(_) | PicChar::V) {
                    j -= 1;
                }
                if count >= 2 {
                    drift = Some((k, j));
                    break;
                }
            }
            k += 1;
        }
        let mut p = 0u32;
        let mut after_v = 0u32;
        let mut seen_v = false;
        let mut len = 0usize;
        for (idx, it) in items.iter().enumerate() {
            let in_drift = drift.is_some_and(|(a, b)| idx > a && idx < b);
            let digit = match it {
                PicChar::Nine | PicChar::Z | PicChar::Star | PicChar::Y | PicChar::Over(_) => true,
                PicChar::Currency(_) | PicChar::Sign(_) => in_drift,
                _ => false,
            };
            if digit {
                p += 1;
                if seen_v {
                    after_v += 1;
                }
            }
            match it {
                PicChar::V => seen_v = true,
                PicChar::Currency(s) => len += s.chars().count(),
                PicChar::Credit(_) => len += 2,
                _ => len += 1,
            }
        }
        if p == 0 {
            return Err("a numeric picture needs a digit position".into());
        }
        Ok(Picture {
            spec: spec.to_string(),
            items,
            numeric: true,
            len,
            p: p.min(255) as u8,
            q: after_v as i16 - f,
            f,
            drift,
            float: None,
        })
    }

    /// The implied arithmetic attributes.
    pub fn arith(&self) -> crate::types::Ty {
        if self.float.is_some() {
            return crate::types::Ty::Float {
                dec: true,
                p: self.p,
            };
        }
        crate::types::Ty::fixed_dec(self.p, self.q)
    }

    /// Edits a floating-point value into the image of a floating-point
    /// picture: the mantissa with its first digit position nonzero.
    pub fn edit_float(&self, v: f64) -> (String, bool) {
        let Some(fp) = &self.float else {
            return (" ".repeat(self.len), true);
        };
        let p = i32::from(fp.mant.p);
        let q = i32::from(fp.mant.q);
        let int_digits = p - q;
        let (mut coef, mut exp) = (0i128, 0i32);
        if v != 0.0 && v.is_finite() {
            let a = v.abs();
            let mut e10 = a.log10().floor() as i32;
            // the leading digit at the first integer position
            loop {
                exp = e10 - (int_digits - 1);
                let scaled = a / 10f64.powi(exp - q);
                let c = (scaled + 1e-9 * scaled).trunc() as i128;
                if c >= pow10(p as u32) {
                    e10 += 1;
                    continue;
                }
                if c < pow10((p - 1).max(0) as u32) && e10 > -400 {
                    e10 -= 1;
                    continue;
                }
                coef = c;
                break;
            }
            if v < 0.0 {
                coef = -coef;
            }
        }
        let (m, lost_m) = fp.mant.edit(Dec::new(coef, q.max(0) as u32));
        let (e, lost_e) = fp.exp.edit(Dec::new(i128::from(exp), 0));
        let mark = if fp.e { "E" } else { "" };
        (format!("{m}{mark}{e}"), lost_m || lost_e)
    }

    /// The value of the image of a floating-point picture.
    pub fn value_float(&self, text: &str) -> Option<f64> {
        let fp = self.float.as_ref()?;
        let chars: Vec<char> = text.chars().collect();
        let m: String = chars.get(..fp.mant.len)?.iter().collect();
        let start = fp.mant.len + usize::from(fp.e);
        let e: String = chars.get(start..start + fp.exp.len)?.iter().collect();
        let m = fp.mant.value(&m)?;
        let e = fp.exp.value(&e)?;
        let mf = m.coef as f64 / 10f64.powi(m.scale as i32);
        Some(mf * 10f64.powi(e.coef as i32))
    }

    /// Edits an arithmetic value (FIXED DEC at the picture's scale) into
    /// the character image. Returns the host characters and whether
    /// high-order digits were lost.
    pub fn edit(&self, value: Dec) -> (String, bool) {
        let negative = value.is_negative();
        // digits at scale (q + f): the shown value is value / 10^f
        let shown_scale = (self.q + self.f).max(0) as u32;
        let v = value.abs();
        let v = if self.f > 0 {
            Dec::new(v.coef / pow10(self.f as u32).max(1), v.scale)
        } else if self.f < 0 {
            Dec::new(v.coef * pow10((-self.f) as u32), v.scale)
        } else {
            v
        };
        let v = v
            .rescale(shown_scale, zeus_decimal::Rounding::Truncate)
            .unwrap_or(Dec::new(0, shown_scale));
        let digits_n = self.p as usize;
        let mut s = v.coef.to_string();
        let lost = s.len() > digits_n;
        if lost {
            s = s[s.len() - digits_n..].to_string();
        }
        let digits: Vec<u8> = format!("{s:0>digits_n$}")
            .bytes()
            .map(|b| b - b'0')
            .collect();
        let zero = digits.iter().all(|&d| d == 0);
        let (drift_a, drift_b) = self.drift.unwrap_or((usize::MAX, usize::MAX));
        let in_drift = |i: usize| i >= drift_a && i < drift_b;
        let drift_sym = |p: &PicChar| match p {
            PicChar::Currency(s) => s.clone(),
            PicChar::Sign('S') => (if negative { "-" } else { "+" }).to_string(),
            PicChar::Sign('+') => (if negative { " " } else { "+" }).to_string(),
            PicChar::Sign('-') => (if negative { "-" } else { " " }).to_string(),
            _ => String::new(),
        };
        // digit index of each item
        let mut di = Vec::with_capacity(self.items.len());
        let mut d = 0usize;
        let v_index = self.items.iter().position(|x| *x == PicChar::V);
        for (i, it) in self.items.iter().enumerate() {
            let is_digit = match it {
                PicChar::Nine | PicChar::Z | PicChar::Star | PicChar::Y | PicChar::Over(_) => true,
                PicChar::Currency(_) | PicChar::Sign(_) => in_drift(i) && i > drift_a,
                _ => false,
            };
            if is_digit {
                di.push(Some(d));
                d += 1;
            } else {
                di.push(None);
            }
        }
        let after_v = |i: usize| v_index.is_some_and(|v| i > v);
        // significance: positions from the first nonzero digit or the first
        // forced (9, T, I, R) position on are shown
        let first_sig_digit = digits.iter().position(|&x| x != 0);
        let mut forced_from = None;
        for (i, it) in self.items.iter().enumerate() {
            if matches!(it, PicChar::Nine | PicChar::Over(_))
                || (after_v(i) && !zero && di[i].is_some())
            {
                forced_from = di[i];
                break;
            }
        }
        let start = match (first_sig_digit, forced_from) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let significant = |k: usize| start.is_some_and(|s| k >= s);
        let all_suppressed = start.is_none();
        let mut out = String::new();
        let mut started = false;
        let mut fill = ' ';
        // where the drifting symbol goes: the item before the first
        // significant digit within the string
        let drift_symbol_at = if self.drift.is_some() {
            let mut at = drift_a;
            #[allow(clippy::needless_range_loop)]
            for i in drift_a..drift_b {
                if let Some(k) = di[i]
                    && significant(k)
                {
                    break;
                }
                if di[i].is_some() || i == drift_a {
                    at = i;
                }
            }
            // a drifting string followed by significant positions puts the
            // symbol in its last position
            Some(at)
        } else {
            None
        };
        for (i, it) in self.items.iter().enumerate() {
            if in_drift(i) {
                match it {
                    PicChar::Insert('B') => out.push(' '),
                    PicChar::Insert(c) => {
                        if started {
                            out.push(*c);
                        } else {
                            out.push(' ');
                        }
                    }
                    PicChar::V => {}
                    _ => {
                        let k = di[i];
                        if let Some(k) = k
                            && significant(k)
                        {
                            started = true;
                            out.push((b'0' + digits[k]) as char);
                        } else if Some(i) == drift_symbol_at && !all_suppressed {
                            out.push_str(&drift_sym(it));
                        } else {
                            out.push_str(&" ".repeat(drift_sym(it).chars().count().max(1)));
                        }
                    }
                }
                continue;
            }
            match it {
                PicChar::Nine => {
                    started = true;
                    out.push((b'0' + digits[di[i].unwrap_or(0)]) as char);
                }
                PicChar::Z | PicChar::Star => {
                    let k = di[i].unwrap_or(0);
                    let sup = if *it == PicChar::Z { ' ' } else { '*' };
                    fill = sup;
                    if significant(k) {
                        started = true;
                        out.push((b'0' + digits[k]) as char);
                    } else {
                        out.push(sup);
                    }
                }
                PicChar::Y => {
                    let k = di[i].unwrap_or(0);
                    started = true;
                    out.push(if digits[k] == 0 {
                        ' '
                    } else {
                        (b'0' + digits[k]) as char
                    });
                }
                PicChar::Over(c) => {
                    let x = digits[di[i].unwrap_or(0)];
                    started = true;
                    let pos = "{ABCDEFGHI".as_bytes()[x as usize] as char;
                    let neg = "}JKLMNOPQR".as_bytes()[x as usize] as char;
                    out.push(match (c, negative) {
                        ('T', false) | ('I', false) => pos,
                        ('T', true) | ('R', true) => neg,
                        _ => (b'0' + x) as char,
                    });
                }
                PicChar::V => {}
                PicChar::Insert('B') => out.push(' '),
                PicChar::Insert(c) => {
                    let show = started
                        || (after_v(i) && !zero)
                        || out.chars().all(|x| !x.is_ascii_digit())
                            && !self.items[..i]
                                .iter()
                                .any(|x| matches!(x, PicChar::Z | PicChar::Star));
                    if show {
                        out.push(*c);
                    } else {
                        out.push(fill);
                    }
                }
                PicChar::Currency(s) => out.push_str(s),
                PicChar::Sign('S') => out.push(if negative { '-' } else { '+' }),
                PicChar::Sign('+') => out.push(if negative { ' ' } else { '+' }),
                PicChar::Sign(_) => out.push(if negative { '-' } else { ' ' }),
                PicChar::Credit(s) => {
                    if negative {
                        out.push_str(s);
                    } else {
                        out.push_str("  ");
                    }
                }
                PicChar::CharPos(_) => out.push(' '),
            }
        }
        // a value suppressed entirely blanks the whole field (also static
        // currency symbols) unless '*' is the fill
        if all_suppressed
            && self.items.iter().any(|x| matches!(x, PicChar::Z))
            && !self
                .items
                .iter()
                .any(|x| matches!(x, PicChar::Star | PicChar::Nine | PicChar::Over(_)))
        {
            out = " ".repeat(self.len);
        }
        (out, lost)
    }

    /// The arithmetic value of a character image (as host text).
    pub fn value(&self, text: &str) -> Option<Dec> {
        let chars: Vec<char> = text.chars().collect();
        let mut pos = 0usize;
        let mut digits = String::new();
        let mut negative = false;
        let mut after = 0u32;
        let mut seen_v = false;
        let (drift_a, drift_b) = self.drift.unwrap_or((usize::MAX, usize::MAX));
        for (i, it) in self.items.iter().enumerate() {
            let in_drift = i >= drift_a && i < drift_b;
            let digit_at = |c: char, digits: &mut String| -> Option<()> {
                match c {
                    '0'..='9' => digits.push(c),
                    ' ' | '*' => digits.push('0'),
                    _ => return None,
                }
                Some(())
            };
            match it {
                PicChar::V => {
                    seen_v = true;
                    continue;
                }
                PicChar::Nine | PicChar::Z | PicChar::Star | PicChar::Y => {
                    let c = *chars.get(pos)?;
                    digit_at(c, &mut digits)?;
                    if seen_v {
                        after += 1;
                    }
                    pos += 1;
                }
                PicChar::Over(_) => {
                    let c = *chars.get(pos)?;
                    let (d, neg) = if let Some(k) = "{ABCDEFGHI".find(c) {
                        (k, false)
                    } else if let Some(k) = "}JKLMNOPQR".find(c) {
                        (k, true)
                    } else if c.is_ascii_digit() {
                        (c as usize - '0' as usize, false)
                    } else {
                        return None;
                    };
                    digits.push((b'0' + d as u8) as char);
                    negative |= neg;
                    if seen_v {
                        after += 1;
                    }
                    pos += 1;
                }
                PicChar::Currency(s) => {
                    let n = s.chars().count();
                    if in_drift && i > drift_a {
                        // a drifting position: a digit, blank or the symbol
                        let c = *chars.get(pos)?;
                        if c.is_ascii_digit() {
                            digits.push(c);
                            pos += 1;
                        } else {
                            digits.push('0');
                            pos += n.max(1);
                        }
                        if seen_v {
                            after += 1;
                        }
                    } else {
                        pos += n;
                    }
                }
                PicChar::Sign(_) => {
                    let c = *chars.get(pos)?;
                    if c == '-' {
                        negative = true;
                    }
                    if in_drift && i > drift_a {
                        if c.is_ascii_digit() {
                            digits.push(c);
                        } else {
                            digits.push('0');
                        }
                        if seen_v {
                            after += 1;
                        }
                    }
                    pos += 1;
                }
                PicChar::Credit(s) => {
                    let t: String = chars.get(pos..pos + 2)?.iter().collect();
                    if t.eq_ignore_ascii_case(s) {
                        negative = true;
                    }
                    pos += 2;
                }
                PicChar::Insert(_) | PicChar::CharPos(_) => pos += 1,
            }
        }
        let coef: i128 = if digits.is_empty() {
            0
        } else {
            digits.parse().ok()?
        };
        let mut d = Dec::new(if negative { -coef } else { coef }, after);
        if self.f > 0 {
            d = Dec::new(d.coef * pow10(self.f as u32), d.scale);
        } else if self.f < 0 {
            d = Dec::new(d.coef, d.scale + (-self.f) as u32);
        }
        Some(d)
    }

    /// Whether a character string fits a character picture.
    pub fn valid_char(&self, text: &str) -> bool {
        text.chars().zip(self.items.iter()).all(|(c, it)| match it {
            PicChar::CharPos('A') => c.is_ascii_alphabetic() || matches!(c, '#' | '@' | '$' | ' '),
            PicChar::CharPos('9') => c.is_ascii_digit() || c == ' ',
            _ => true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed(pic: &str, v: &str) -> String {
        let p = Picture::parse(pic).unwrap();
        let d = Dec::parse(v).unwrap();
        let d = d
            .rescale(p.q.max(0) as u32, zeus_decimal::Rounding::Truncate)
            .unwrap_or(d);
        p.edit(d).0
    }

    #[test]
    fn editing() {
        assert_eq!(ed("99999", "123"), "00123");
        assert_eq!(ed("999V99", "123.45"), "12345");
        assert_eq!(ed("ZZZ99", "100"), "  100");
        assert_eq!(ed("ZZZZZ", "0"), "     ");
        assert_eq!(ed("ZZZV99", "1.23"), "  123");
        assert_eq!(ed("*****", "100"), "**100");
        assert_eq!(ed("9,999", "1234"), "1,234");
        assert_eq!(ed("ZZ/ZZ/ZZ", "1234"), "   12/34");
        assert_eq!(ed("ZZ.VZZ", "12.34"), "12.34");
        assert_eq!(ed("ZZV.ZZ", "0.03"), "  .03");
        assert_eq!(ed("$999V.99", "123.45"), "$123.45");
        assert_eq!(ed("$$$9V.99", "1.23"), "  $1.23");
        assert_eq!(ed("$$$,999", "1234"), " $1,234");
        assert_eq!(ed("SZZZV.99", "2.45"), "+  2.45");
        assert_eq!(ed("-999V.99", "-123.45"), "-123.45");
        assert_eq!(ed("999V.99S", "123.45"), "123.45+");
        assert_eq!(ed("SS,SS9", "-4"), "    -4");
        assert_eq!(ed("$ZZV.99DB", "12.34"), "$12.34  ");
        assert_eq!(ed("999I", "1021"), "102A");
        assert_eq!(ed("Z99R", "-1021"), "102J");
        assert_eq!(ed("YYYYY", "100"), "  1  ");
        assert_eq!(ed("99F(2)", "1200"), "12");
        let p = Picture::parse("S999V99").unwrap();
        assert_eq!(p.arith(), crate::types::Ty::fixed_dec(5, 2));
        assert_eq!(p.value("-12345"), Some(Dec::new(-12345, 2)));
        assert_eq!(Picture::parse("$$$9V.99").unwrap().len, 7);
        let f = Picture::parse("S9V.999ES99").unwrap();
        assert_eq!(f.len, 10);
        assert_eq!(f.edit_float(1234.5).0, "+1.234E+03");
        assert_eq!(f.edit_float(-0.05).0, "-5.000E-02");
        assert_eq!(f.value_float("+1.234E+03"), Some(1234.0));
        let k = Picture::parse("99V99KS9").unwrap();
        assert_eq!(k.edit_float(12.5).0, "1250+0");
    }
}
