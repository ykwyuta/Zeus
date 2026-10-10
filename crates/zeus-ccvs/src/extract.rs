//! Extraction of the CCVS85 population file into a directory tree:
//!
//! ```text
//! <out>/copy/<member>          COPY library members
//! <out>/copyalt/ALTLB          the alternate library (member ALTL1, X-48)
//! <out>/<MOD>/<prog>.CBL       main programs
//! <out>/<MOD>/<prog>.SUB       follow-on programs (run after their parent)
//! <out>/<MOD>/lib/<prog>.CBL   called subprograms
//! <out>/<MOD>/<prog>.DAT       input data (standard input of <prog>)
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::exec85::Config;
use crate::population::{Member, MemberKind, SubKind};

/// A transformed member ready to be written.
#[derive(Debug, Clone)]
pub struct Extracted {
    pub member: Member,
    pub text: String,
}

impl Extracted {
    /// Path relative to the suite root.
    /// Contents of the file written to the suite tree. For data members, a
    /// card filled up to column 80 continues on the next card, so that
    /// records longer than a card (NC204M accepts a 200-character record)
    /// become one line of standard input.
    pub fn file_contents(&self) -> String {
        if self.member.kind != MemberKind::Data {
            return self.text.clone();
        }
        let mut out = String::new();
        for line in self.text.lines() {
            out.push_str(line);
            if line.chars().count() < 80 {
                out.push('\n');
            }
        }
        out
    }

    pub fn rel_path(&self) -> String {
        let m = &self.member;
        match m.kind {
            MemberKind::Library if m.name == "ALTL1" => "copyalt/ALTLB".to_string(),
            MemberKind::Library => format!("copy/{}", m.name),
            MemberKind::Data => format!("{}/{}.DAT", &m.name[..2], m.name),
            MemberKind::Cobol => match &m.parent {
                Some((_, SubKind::Subroutine)) => format!("{}/lib/{}.CBL", m.module(), m.name),
                Some((_, SubKind::FollowOn)) => format!("{}/{}.SUB", m.module(), m.name),
                None => format!("{}/{}.CBL", m.module(), m.name),
            },
        }
    }

    /// Manifest key identifying the member independently of the layout.
    pub fn key(&self) -> String {
        manifest_key(&self.member.header)
    }
}

/// Normalises a `*HEADER` card into a manifest key (`COBOL,IC101A,SUBRTN,IC102A`).
pub fn manifest_key(header: &str) -> String {
    header
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_start_matches("*HEADER,")
        .split(',')
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(",")
}

pub fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Joins card images the way EXEC85 writes them to a line-sequential file:
/// trailing blanks removed, one line per card.
pub fn join_lines<I: IntoIterator<Item = String>>(lines: I) -> String {
    let mut text = String::new();
    for l in lines {
        text.push_str(l.trim_end());
        text.push('\n');
    }
    text
}

/// Applies the EXEC85 transformations to every member. The EXEC85 program
/// itself (module `EX`) is skipped.
pub fn transform(members: &[Member], cfg: &Config) -> Vec<Extracted> {
    members
        .iter()
        .filter(|m| !(m.kind == MemberKind::Cobol && m.module() == "EX"))
        .map(|m| {
            let text = match m.kind {
                MemberKind::Data => join_lines(m.lines.iter().cloned()),
                MemberKind::Cobol | MemberKind::Library => {
                    let mut lines = m.lines.clone();
                    cfg.apply_updates(&m.name, &mut lines);
                    join_lines(lines.iter().filter_map(|l| cfg.transform_line(l)))
                }
            };
            Extracted {
                member: m.clone(),
                text,
            }
        })
        .collect()
}

pub fn write_tree(extracted: &[Extracted], out: &Path) -> Result<()> {
    for e in extracted {
        let path = out.join(e.rel_path());
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        fs::write(&path, e.file_contents())
            .with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(())
}

/// Manifest: member key -> SHA-256 of its extracted text.
pub type Manifest = BTreeMap<String, String>;

pub fn manifest_of(extracted: &[Extracted]) -> Manifest {
    extracted
        .iter()
        .map(|e| (e.key(), sha256_hex(&e.text)))
        .collect()
}

pub fn format_manifest(manifest: &Manifest) -> String {
    let mut s = String::from(
        "# SHA-256 of each CCVS85 member as extracted by EXEC85 (reference output).\n\
         # Format: <sha256> <member key>\n",
    );
    for (key, hash) in manifest {
        s.push_str(&format!("{hash} {key}\n"));
    }
    s
}

pub fn parse_manifest(text: &str) -> Result<Manifest> {
    let mut m = Manifest::new();
    for (i, line) in text.lines().enumerate() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let Some((hash, key)) = line.split_once(' ') else {
            bail!("manifest line {}: malformed", i + 1);
        };
        m.insert(key.to_string(), hash.to_string());
    }
    Ok(m)
}

/// Builds a manifest from files written by the real EXEC85 program
/// (`newcob.tmp`, one per selected module). Members appearing in several
/// files must be identical.
pub fn manifest_from_exec85_output(texts: &[String]) -> Result<Manifest> {
    let mut manifest = Manifest::new();
    for text in texts {
        let mut current: Option<(String, Vec<String>)> = None;
        for line in text.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("*HEADER,") {
                current = Some((manifest_key(trimmed), Vec::new()));
            } else if trimmed.starts_with("*END-OF,") {
                let (key, lines) = current.take().context("*END-OF without *HEADER")?;
                let hash = sha256_hex(&join_lines(lines));
                if let Some(prev) = manifest.insert(key.clone(), hash.clone())
                    && prev != hash
                {
                    bail!("member {key} differs between EXEC85 outputs");
                }
            } else if let Some((_, lines)) = current.as_mut() {
                lines.push(line.to_string());
            }
        }
    }
    Ok(manifest)
}

/// Compares an extraction against a reference manifest; returns the
/// differences as human-readable lines (empty when identical).
pub fn verify(extracted: &Manifest, reference: &Manifest) -> Vec<String> {
    let mut diffs = Vec::new();
    for (key, hash) in reference {
        match extracted.get(key) {
            None => diffs.push(format!("missing member: {key}")),
            Some(h) if h != hash => diffs.push(format!("content differs: {key}")),
            Some(_) => {}
        }
    }
    for key in extracted.keys() {
        if !reference.contains_key(key) {
            diffs.push(format!("not in reference: {key}"));
        }
    }
    diffs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::population::parse;

    const POP: &str = "*HEADER,COBOL,EXEC85\n000100 X\n*END-OF,EXEC85\n\
                       *HEADER,COBOL,IX101A\n000100     XXXXX055.\n*END-OF,IX101A\n\
                       *HEADER,COBOL,IX101A,SUBPRG,IX102A\n000100 B\n*END-OF,IX102A\n\
                       *HEADER,COBOL,IC101A,SUBRTN,IC102A\n000100 C\n*END-OF,IC102A\n\
                       *HEADER,CLBRY,ALTL1\n000100 ALT\n*END-OF,ALTL1\n\
                       *HEADER,DATA*,NC109M\nDATA  \n*END-OF,NC109M\n";

    fn extracted() -> Vec<Extracted> {
        let cfg = Config::parse("X-05573 \"REPORT\"\n*END-MONITOR\n").unwrap();
        transform(&parse(POP).unwrap(), &cfg)
    }

    #[test]
    fn layout() {
        let paths: Vec<String> = extracted().iter().map(Extracted::rel_path).collect();
        assert_eq!(
            paths,
            [
                "IX/IX101A.CBL",
                "IX/IX102A.SUB",
                "IC/lib/IC102A.CBL",
                "copyalt/ALTLB",
                "NC/NC109M.DAT"
            ]
        );
    }

    #[test]
    fn long_data_records_span_cards() {
        let card = "D".repeat(80);
        let pop = format!("*HEADER,DATA*,NC204M\nAB\n{card}\n{card}\nEND\nX\n*END-OF,NC204M\n");
        let e = transform(&parse(&pop).unwrap(), &Config::default());
        assert_eq!(e[0].file_contents(), format!("AB\n{card}{card}END\nX\n"));
    }

    #[test]
    fn manifest_round_trip_and_verify() {
        let e = extracted();
        assert_eq!(e[0].text, "000100     \"REPORT\".\n");
        let m = manifest_of(&e);
        let parsed = parse_manifest(&format_manifest(&m)).unwrap();
        assert_eq!(parsed, m);
        assert!(verify(&m, &parsed).is_empty());

        let exec85_out =
            "      *HEADER,COBOL,IX101A\n000100     \"REPORT\".\n      *END-OF,IX101A\n";
        let r = manifest_from_exec85_output(&[exec85_out.to_string()]).unwrap();
        assert_eq!(r.get("COBOL,IX101A"), m.get("COBOL,IX101A"));
    }
}
