//! PICTURE character-string analysis.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    Alphabetic,
    Alphanumeric,
    /// DBCS (PIC G, PIC N with NSYMBOL(DBCS) / USAGE DISPLAY-1): two bytes
    /// per character.
    Dbcs,
    Numeric,
    NumericEdited,
    AlphanumericEdited,
}

/// One position (or symbol) of an edited picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditSym {
    /// `9`
    Digit,
    /// `Z`
    ZeroSuppress,
    /// `*`
    Asterisk,
    /// A position of a floating insertion string (`$$$`, `+++`, `---`);
    /// the first one of the string is not a digit position.
    Floating { ch: u8, first: bool },
    /// `.` (or `,` with DECIMAL-POINT IS COMMA): the decimal point.
    Point,
    /// Simple insertion: `B` (space), `0`, `/`, `,` (or `.`).
    Insert(u8),
    /// Fixed `+` or `-`.
    Sign(u8),
    /// `CR` or `DB` (two positions).
    Credit(bool),
    /// Single currency symbol.
    Currency,
    /// `X` or `A` (alphanumeric-edited).
    Char,
    /// `V`: implied decimal point (no position).
    ImpliedPoint,
    /// `P`: scaling position (no position).
    Scale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Picture {
    pub category: Category,
    /// Character positions occupied (USAGE DISPLAY size).
    pub size: u32,
    /// Numeric: digit positions (9s; for edited pictures, all digit
    /// positions).
    pub digits: u32,
    /// Decimal scale: digits right of the (implied) point; negative for
    /// trailing P positions.
    pub scale: i32,
    pub signed: bool,
    /// Edited pictures: the symbols, one per position (V and P excluded).
    pub edit: Vec<EditSym>,
}

/// Expands repetition factors: `X(3)9(2)` -> `XXX99`.
pub fn expand(pic: &str) -> Result<String, String> {
    let mut out = String::new();
    let chars: Vec<char> = pic.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i].to_ascii_uppercase();
        if c == '(' {
            let end = chars[i..]
                .iter()
                .position(|&x| x == ')')
                .ok_or("unbalanced parenthesis in PICTURE")?
                + i;
            let n: usize = chars[i + 1..end]
                .iter()
                .collect::<String>()
                .trim()
                .parse()
                .map_err(|_| "invalid repetition factor in PICTURE")?;
            let last = out.pop().ok_or("repetition factor without symbol")?;
            // CR/DB cannot be repeated; others repeat the previous symbol.
            for _ in 0..n {
                out.push(last);
            }
            i = end + 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    Ok(out)
}

pub fn analyze(pic: &str, decimal_comma: bool, currency: char) -> Result<Picture, String> {
    let s = expand(pic)?;
    let chars: Vec<char> = s.chars().collect();
    let point_char = if decimal_comma { ',' } else { '.' };
    let comma_char = if decimal_comma { '.' } else { ',' };
    let has = |c: char| chars.contains(&c);

    // Numeric: only 9 S V P.
    if chars.iter().all(|c| matches!(c, '9' | 'S' | 'V' | 'P')) {
        let signed = has('S');
        let digits = chars.iter().filter(|c| **c == '9').count() as u32;
        let scale = numeric_scale(&chars);
        return Ok(Picture {
            category: Category::Numeric,
            size: digits,
            digits,
            scale,
            signed,
            edit: Vec::new(),
        });
    }
    if chars.iter().all(|c| matches!(c, 'G' | 'N')) {
        return Ok(Picture {
            category: Category::Dbcs,
            size: 2 * chars.len() as u32,
            digits: 0,
            scale: 0,
            signed: false,
            edit: Vec::new(),
        });
    }
    if chars.iter().all(|c| *c == 'A') {
        return Ok(Picture {
            category: Category::Alphabetic,
            size: chars.len() as u32,
            digits: 0,
            scale: 0,
            signed: false,
            edit: Vec::new(),
        });
    }
    if chars.iter().all(|c| matches!(c, 'X' | 'A' | '9')) {
        return Ok(Picture {
            category: Category::Alphanumeric,
            size: chars.len() as u32,
            digits: 0,
            scale: 0,
            signed: false,
            edit: Vec::new(),
        });
    }
    if chars
        .iter()
        .all(|c| matches!(c, 'X' | 'A' | '9' | 'B' | '0' | '/'))
        && chars.iter().any(|c| matches!(c, 'X' | 'A'))
    {
        let edit = chars
            .iter()
            .map(|c| match c {
                'B' => EditSym::Insert(b' '),
                '0' => EditSym::Insert(b'0'),
                '/' => EditSym::Insert(b'/'),
                _ => EditSym::Char,
            })
            .collect::<Vec<_>>();
        return Ok(Picture {
            category: Category::AlphanumericEdited,
            size: edit.len() as u32,
            digits: 0,
            scale: 0,
            signed: false,
            edit,
        });
    }

    // Numeric-edited.
    let float_ch = ['$', '+', '-']
        .into_iter()
        .map(|c| if c == '$' { currency } else { c })
        .find(|&c| chars.iter().filter(|x| **x == c).count() >= 2);
    let mut edit = Vec::new();
    let mut first_float = true;
    let mut i = 0;
    let mut digits = 0u32;
    let mut frac = 0i32;
    let mut after_point = false;
    let mut p_left = 0i32;
    let mut p_right = 0i32;
    while i < chars.len() {
        let c = chars[i];
        let sym = if Some(c) == float_ch {
            let s = EditSym::Floating {
                ch: c as u8,
                first: first_float,
            };
            if !first_float {
                digits += 1;
                if after_point {
                    frac += 1;
                }
            }
            first_float = false;
            Some(s)
        } else if c == '9' || c == 'Z' || c == '*' {
            digits += 1;
            if after_point {
                frac += 1;
            }
            Some(match c {
                '9' => EditSym::Digit,
                'Z' => EditSym::ZeroSuppress,
                _ => EditSym::Asterisk,
            })
        } else if c == point_char {
            after_point = true;
            Some(EditSym::Point)
        } else if c == 'V' {
            after_point = true;
            Some(EditSym::ImpliedPoint)
        } else if c == 'P' {
            if digits == 0 || after_point {
                p_left += 1;
            } else {
                p_right += 1;
            }
            Some(EditSym::Scale)
        } else if c == comma_char {
            Some(EditSym::Insert(comma_char as u8))
        } else if c == 'B' {
            Some(EditSym::Insert(b' '))
        } else if c == '0' {
            Some(EditSym::Insert(b'0'))
        } else if c == '/' {
            Some(EditSym::Insert(b'/'))
        } else if c == '+' || c == '-' {
            Some(EditSym::Sign(c as u8))
        } else if c == 'C' && chars.get(i + 1) == Some(&'R') {
            i += 1;
            Some(EditSym::Credit(true))
        } else if c == 'D' && chars.get(i + 1) == Some(&'B') {
            i += 1;
            Some(EditSym::Credit(false))
        } else if c == currency {
            Some(EditSym::Currency)
        } else if c == 'S' {
            None
        } else {
            return Err(format!("invalid PICTURE character {c:?} in {pic}"));
        };
        if let Some(sym) = sym {
            edit.push(sym);
        }
        i += 1;
    }
    let size = edit
        .iter()
        .map(|s| match s {
            EditSym::ImpliedPoint | EditSym::Scale => 0,
            EditSym::Credit(_) => 2,
            _ => 1,
        })
        .sum();
    edit.retain(|s| !matches!(s, EditSym::ImpliedPoint | EditSym::Scale));
    let signed = edit.iter().any(|s| {
        matches!(
            s,
            EditSym::Sign(_)
                | EditSym::Credit(_)
                | EditSym::Floating {
                    ch: b'+' | b'-',
                    ..
                }
        )
    });
    Ok(Picture {
        category: Category::NumericEdited,
        size,
        digits,
        scale: frac + p_left - p_right,
        signed,
        edit,
    })
}

/// Scale of a numeric picture: digits after V, plus P positions to the
/// left of the digits, minus trailing P positions.
fn numeric_scale(chars: &[char]) -> i32 {
    let first_digit = chars.iter().position(|c| *c == '9');
    let v = chars.iter().position(|c| *c == 'V');
    let mut scale = 0i32;
    // Leading P positions put the implied point to their left, so all
    // digits are fraction digits.
    let leading_p = chars
        .iter()
        .position(|c| *c == 'P')
        .is_some_and(|p| first_digit.is_none_or(|d| p < d));
    for (i, c) in chars.iter().enumerate() {
        match c {
            '9' if leading_p || v.is_some_and(|v| i > v) => scale += 1,
            'P' => {
                let left_of_digits = first_digit.is_none_or(|d| i < d);
                if left_of_digits {
                    // VPP99 / PP99: P positions add to the scale
                    scale += 1;
                } else if v.is_none_or(|v| i < v) {
                    // 99PP: trailing scaling positions
                    scale -= 1;
                } else {
                    scale += 1;
                }
            }
            _ => {}
        }
    }
    scale
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(p: &str) -> Picture {
        analyze(p, false, '$').unwrap()
    }

    #[test]
    fn numeric() {
        let p = a("S9(3)V99");
        assert_eq!(p.category, Category::Numeric);
        assert_eq!((p.digits, p.scale, p.signed, p.size), (5, 2, true, 5));
        assert_eq!(a("99PP").scale, -2);
        assert_eq!(a("PP99").scale, 4);
        assert_eq!(a("VPP99").scale, 4);
        assert_eq!(a("SP(8)9").scale, 9);
    }

    #[test]
    fn alphanumeric() {
        assert_eq!(a("X(5)").category, Category::Alphanumeric);
        assert_eq!(a("A(3)").category, Category::Alphabetic);
        assert_eq!(a("XXBXX").category, Category::AlphanumericEdited);
        assert_eq!(a("XXBXX").size, 5);
    }

    #[test]
    fn edited() {
        let p = a("$**.99");
        assert_eq!(p.category, Category::NumericEdited);
        assert_eq!((p.size, p.digits, p.scale), (6, 4, 2));
        let p = a("$$99.99CR");
        assert_eq!((p.size, p.digits, p.scale, p.signed), (9, 5, 2, true));
        assert_eq!(
            p.edit[0],
            EditSym::Floating {
                ch: b'$',
                first: true
            }
        );
        let p = a("-9(16).99");
        assert_eq!((p.size, p.digits, p.scale), (20, 18, 2));
        let p = a("ZZ,ZZZ.9");
        assert_eq!((p.size, p.digits, p.scale), (8, 6, 1));
        let p = a(".9999/99999,99999,99");
        assert_eq!(p.digits, 16);
    }
}
