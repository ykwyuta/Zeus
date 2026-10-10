//! Re-implementation of the source transformations performed by the CCVS85
//! executive program EXEC85.
//!
//! EXEC85 reads a control-card file and applies, to every COBOL and COPY
//! library card image:
//!
//! * **optional code letters** in column 7: a line whose column 7 holds a
//!   letter that is one of the selected options becomes an ordinary line
//!   (column 7 blanked); otherwise it is deleted (`*KILL-DELETIONS`) or turned
//!   into a comment;
//! * **X-card substitution**: when columns 12-19 hold `XXXXXnnn`, columns 12-72
//!   are replaced by the implementor text of X-card `nnn`. Column 20 tells
//!   whether the replacement must end with a full stop (`.`) or must not
//!   (space).
//!
//! Data members (`DATA*`) are copied unchanged. The behaviour was derived from
//! the EXEC85 source distributed in `newcob.val` and is verified against the
//! output of EXEC85 itself (see `ccvs85/extract-manifest.txt`).

use anyhow::{Result, bail};

/// Number of X-cards EXEC85 supports.
pub const X_CARDS: usize = 200;
/// Width of an X-card substitution text.
const X_TEXT_WIDTH: usize = 60;

#[derive(Debug, Clone)]
pub struct Config {
    /// Selected optional-code letters.
    pub options: Vec<char>,
    /// Delete (rather than comment out) lines of unselected options.
    pub kill_deletions: bool,
    /// Drop comment lines (`*` in column 7).
    pub remove_comments: bool,
    /// Substitution texts, indexed by X-card number - 1.
    pub x_cards: Vec<String>,
    /// Source updates: (program, replacement card images).
    pub updates: Vec<(String, Vec<String>)>,
}

impl Default for Config {
    /// The EXEC85 defaults before any control card is read.
    fn default() -> Self {
        let mut x_cards = vec!["**** X-CARD UNDEFINED ****".to_string(); X_CARDS];
        x_cards[83] = "    OMITTED".to_string();
        Config {
            options: vec!['A', 'E', 'H', 'L', 'Y', 'T'],
            kill_deletions: false,
            remove_comments: false,
            x_cards,
            updates: Vec::new(),
        }
    }
}

impl Config {
    /// Parses an EXEC85 control-card file.
    ///
    /// Supported monitor cards: `*KILL-DELETIONS`, `*REMOVE-COMMENTS`,
    /// `X-nnnpp text` and `*END-MONITOR`. Module/program selection and listing
    /// cards are accepted and ignored because the extractor always extracts
    /// the whole suite. After `*END-MONITOR`, an optional
    /// `*BEGIN-UPDATE` ... `*END-UPDATE` block holds `*START <program>` cards
    /// followed by replacement card images.
    pub fn parse(text: &str) -> Result<Config> {
        let mut cfg = Config::default();
        let mut lines = text.lines().map(|l| l.trim_end_matches('\r'));
        let mut ended = false;
        for line in lines.by_ref() {
            if line.starts_with("*END-MONITOR") {
                ended = true;
                break;
            } else if line.starts_with("*KILL-DELETIONS") {
                cfg.kill_deletions = true;
            } else if line.starts_with("*REMOVE-COMMENTS") {
                cfg.remove_comments = true;
            } else if line.starts_with("*OPT") {
                bail!("*OPT cards are not supported yet: {line:?}");
            } else if line.starts_with("X-") {
                let card = |range: std::ops::Range<usize>| line.get(range).unwrap_or("");
                let num: usize = card(2..5)
                    .trim()
                    .parse()
                    .map_err(|_| anyhow::anyhow!("invalid X-card number: {line:?}"))?;
                if num == 0 || num > X_CARDS {
                    bail!("X-card number out of range: {line:?}");
                }
                if let Ok(pos) = card(5..7).trim().parse::<u32>()
                    && pos < 55
                {
                    bail!("X-cards with a program-name position are not supported: {line:?}");
                }
                cfg.x_cards[num - 1] = line
                    .get(8..)
                    .unwrap_or("")
                    .chars()
                    .take(X_TEXT_WIDTH)
                    .collect();
            } else if line.starts_with('*') || line.trim().is_empty() {
                // selection / listing cards: not relevant for full extraction
            } else {
                bail!("unexpected monitor card: {line:?}");
            }
        }
        if !ended {
            bail!("*END-MONITOR card not found");
        }
        let mut in_update = false;
        for line in lines {
            if line.starts_with("*BEGIN-UPDATE") {
                in_update = true;
            } else if line.starts_with("*END-UPDATE") {
                in_update = false;
            } else if let Some(prog) = line.strip_prefix("*START") {
                if !in_update {
                    bail!("*START outside *BEGIN-UPDATE");
                }
                let prog = prog.trim();
                if !prog.is_empty() {
                    cfg.updates.push((prog.to_string(), Vec::new()));
                }
            } else if in_update {
                match cfg.updates.last_mut() {
                    Some((_, cards)) => cards.push(line.to_string()),
                    None => bail!("update card before *START: {line:?}"),
                }
            }
        }
        Ok(cfg)
    }

    /// Applies `*START` updates for `program`: a card replaces the card with
    /// the same sequence number (columns 1-6), or is inserted in sequence order.
    pub fn apply_updates(&self, program: &str, lines: &mut Vec<String>) {
        for (_, cards) in self.updates.iter().filter(|(p, _)| p == program) {
            for card in cards {
                let seq = card.get(..6).unwrap_or(card);
                match lines.iter().position(|l| l.get(..6) == Some(seq)) {
                    Some(i) => lines[i] = card.clone(),
                    None => {
                        let at = lines
                            .iter()
                            .position(|l| l.get(..6).is_some_and(|s| s > seq))
                            .unwrap_or(lines.len());
                        lines.insert(at, card.clone());
                    }
                }
            }
        }
    }

    /// Transforms one COBOL/library card image. Returns `None` when the line
    /// is removed.
    pub fn transform_line(&self, line: &str) -> Option<String> {
        let mut card: Vec<char> = line.chars().collect();
        if card.len() < 80 {
            card.resize(80, ' ');
        }
        let col7 = card[6];
        if col7 == '*' && self.remove_comments {
            return None;
        }
        if col7 != 'D' && col7.is_ascii_alphabetic() {
            if self.options.contains(&col7) {
                card[6] = ' ';
            } else if self.kill_deletions {
                return None;
            } else {
                card[7] = col7;
                card[6] = '*';
            }
        }
        self.substitute(&mut card);
        let out: String = card.into_iter().collect();
        Some(out.trim_end().to_string())
    }

    fn substitute(&self, card: &mut [char]) {
        let is = |range: std::ops::Range<usize>, pred: &dyn Fn(char) -> bool| {
            card[range].iter().all(|&c| pred(c))
        };
        if !is(11..15, &|c| c == 'X') || !is(16..19, &|c| c.is_ascii_digit()) {
            return;
        }
        let num: usize = card[16..19].iter().collect::<String>().parse().unwrap();
        if num == 0 || num > X_CARDS {
            return;
        }
        let mut text: Vec<char> = self.x_cards[num - 1].chars().collect();
        text.resize(X_TEXT_WIDTH, ' ');
        match card[19] {
            ' ' => remove_full_stop(&mut text),
            '.' => insert_full_stop(&mut text),
            _ => return,
        }
        // WC-SUB-DATA spans columns 12-72 (61 positions); the 60-character
        // text is moved into it and padded with a space.
        for (i, slot) in card[11..72].iter_mut().enumerate() {
            *slot = text.get(i).copied().unwrap_or(' ');
        }
    }
}

/// Removes a trailing full stop from the last non-blank position.
fn remove_full_stop(text: &mut [char]) {
    if let Some(last) = text.iter().rposition(|&c| c != ' ')
        && text[last] == '.'
    {
        text[last] = ' ';
    }
}

/// Appends a full stop after the last non-blank position unless present.
fn insert_full_stop(text: &mut Vec<char>) {
    if let Some(last) = text.iter().rposition(|&c| c != ' ')
        && text[last] != '.'
    {
        if last + 1 < text.len() {
            text[last + 1] = '.';
        } else {
            text.push('.');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config::parse(
            "*KILL-DELETIONS\n\
             X-05573 \"REPORT\"\n\
             X-08273 GNU-Linux\n\
             X-09073 \"A\".\n\
             *END-MONITOR\n",
        )
        .unwrap()
    }

    fn card(s: &str) -> String {
        format!("{s:<72}NC1014.2")
    }

    #[test]
    fn substitutes_with_full_stop() {
        let out = config()
            .transform_line(&card("004200     XXXXX055."))
            .unwrap();
        assert_eq!(out, card("004200     \"REPORT\"."));
    }

    #[test]
    fn substitutes_without_full_stop() {
        let out = config()
            .transform_line(&card("004200     XXXXX090"))
            .unwrap();
        assert_eq!(out, card("004200     \"A\""));
    }

    #[test]
    fn undefined_x_card_is_marked() {
        let out = config()
            .transform_line(&card("000100     XXXXX001."))
            .unwrap();
        assert!(out.contains("**** X-CARD UNDEFINED ****."));
    }

    #[test]
    fn optional_letters() {
        let cfg = config();
        assert_eq!(
            cfg.transform_line(&card("039100Y    IF A")).unwrap(),
            card("039100     IF A")
        );
        assert_eq!(cfg.transform_line(&card("032700S    EXIT PROGRAM.")), None);
        let keep = Config {
            kill_deletions: false,
            ..cfg
        };
        assert_eq!(
            keep.transform_line(&card("032700S    EXIT PROGRAM."))
                .unwrap(),
            card("032700*S   EXIT PROGRAM.")
        );
    }

    #[test]
    fn debugging_and_comment_lines_are_kept() {
        let cfg = config();
        let d = card("000100D    DISPLAY X.");
        assert_eq!(cfg.transform_line(&d).unwrap(), d);
        // Only a marker starting in column 12 is substituted.
        let c = card("000100*         XXXXX055");
        assert_eq!(cfg.transform_line(&c).unwrap(), c);
        assert_eq!(
            cfg.transform_line(&card("000100*    XXXXX055.")).unwrap(),
            card("000100*    \"REPORT\".")
        );
    }

    #[test]
    fn updates_replace_and_insert() {
        let cfg = Config::parse(
            "*END-MONITOR\n*BEGIN-UPDATE\n*START NC101A\n000200 NEW\n000150 INS\n*END-UPDATE\n",
        )
        .unwrap();
        let mut lines = vec!["000100 A".to_string(), "000200 B".to_string()];
        cfg.apply_updates("NC101A", &mut lines);
        assert_eq!(lines, ["000100 A", "000150 INS", "000200 NEW"]);
    }
}
