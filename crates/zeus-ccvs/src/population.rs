//! Parsing of the CCVS85 "population file" (`newcob.val`).
//!
//! The population file is a concatenation of members, each delimited by
//!
//! ```text
//! *HEADER,<type>,<name>[,<subtype>,<subname>]
//! ...80-column card images...
//! *END-OF,<name>
//! ```
//!
//! `<type>` is `COBOL` (a program), `CLBRY` (a COPY library member) or
//! `DATA*` (input data for a program). A COBOL member with a subtype is
//! either a called subroutine (`SUBRTN`) or a follow-on program that runs
//! after its parent and shares its files (`SUBPRG`); its own name is then
//! `<subname>` and `<name>` is the parent program.

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MemberKind {
    Cobol,
    Library,
    Data,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubKind {
    /// Called subprogram, compiled as a loadable module.
    Subroutine,
    /// Separate main program run after its parent.
    FollowOn,
}

#[derive(Debug, Clone)]
pub struct Member {
    pub kind: MemberKind,
    /// The member's own name (the program-id for programs).
    pub name: String,
    /// For subprograms: the parent program and the relation to it.
    pub parent: Option<(String, SubKind)>,
    /// The `*HEADER` card without its identification field.
    pub header: String,
    /// Card images between the header and the `*END-OF` card.
    pub lines: Vec<String>,
}

impl Member {
    /// The test module a program belongs to (`NC`, `SQ`, ...): the first two
    /// characters of the (parent) program name.
    pub fn module(&self) -> &str {
        let base = match &self.parent {
            Some((parent, _)) => parent.as_str(),
            None => self.name.as_str(),
        };
        &base[..base.len().min(2)]
    }
}

pub fn parse(text: &str) -> Result<Vec<Member>> {
    let mut members = Vec::new();
    let mut lines = text.lines().enumerate();
    while let Some((idx, line)) = lines.next() {
        let line = line.trim_end_matches('\r');
        if !line.starts_with("*HEADER,") {
            continue;
        }
        // Columns 73-80 may hold an identification field.
        let header = line.split_whitespace().next().unwrap_or("").to_string();
        let fields: Vec<&str> = header.split(',').map(str::trim).collect();
        let kind = match fields.get(1).copied() {
            Some("COBOL") => MemberKind::Cobol,
            Some("CLBRY") => MemberKind::Library,
            Some("DATA*") => MemberKind::Data,
            other => bail!("line {}: unknown member type {:?}", idx + 1, other),
        };
        let (name, parent) = match fields.as_slice() {
            [_, _, name] => (name.to_string(), None),
            [_, _, parent, sub, name] => {
                let sub = match *sub {
                    "SUBRTN" => SubKind::Subroutine,
                    "SUBPRG" => SubKind::FollowOn,
                    other => bail!("line {}: unknown subtype {other:?}", idx + 1),
                };
                (name.to_string(), Some((parent.to_string(), sub)))
            }
            _ => bail!("line {}: malformed header {header:?}", idx + 1),
        };
        let end = format!("*END-OF,{name}");
        let mut body = Vec::new();
        loop {
            let (_, l) = lines
                .next()
                .with_context(|| format!("member {name}: missing {end}"))?;
            let l = l.trim_end_matches('\r');
            if l.starts_with("*END-OF,") {
                // Columns 73-80 may hold an identification field.
                if l.split_whitespace().next() != Some(end.as_str()) {
                    bail!("member {name}: expected {end}, found {:?}", l.trim_end());
                }
                break;
            }
            body.push(l.to_string());
        }
        members.push(Member {
            kind,
            name,
            parent,
            header,
            lines: body,
        });
    }
    if members.is_empty() {
        bail!("no *HEADER members found; is this a CCVS85 population file?");
    }
    Ok(members)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_members_and_subprograms() {
        let text = "CCVS85  VERSION 4.0\n\
                    *HEADER,COBOL,IC101A\n\
                    000100 IDENTIFICATION DIVISION.\n\
                    *END-OF,IC101A\n\
                    *HEADER,COBOL,IC101A,SUBRTN,IC102A\n\
                    000100 IDENTIFICATION DIVISION.\n\
                    000200 PROGRAM-ID. IC102A.\n\
                    *END-OF,IC102A\n\
                    *HEADER,CLBRY,K101A\n\
                    *END-OF,K101A\n";
        let m = parse(text).unwrap();
        assert_eq!(m.len(), 3);
        assert_eq!(m[0].name, "IC101A");
        assert_eq!(m[1].name, "IC102A");
        assert_eq!(
            m[1].parent,
            Some(("IC101A".to_string(), SubKind::Subroutine))
        );
        assert_eq!(m[1].module(), "IC");
        assert_eq!(m[1].lines.len(), 2);
        assert_eq!(m[2].kind, MemberKind::Library);
        assert!(m[2].lines.is_empty());
    }

    #[test]
    fn rejects_mismatched_end() {
        let text = "*HEADER,COBOL,NC101A\n*END-OF,NC102A\n";
        assert!(parse(text).is_err());
    }
}
