//! IND$FILE: file transfer between a 3270 emulator and the region (DFT
//! mode, structured fields), as described in IBM's "IND$FILE Datastream
//! Description" (SS-HCS12-1372-00).
//!
//! The emulator types `IND$FILE GET name (options` (download) or
//! `IND$FILE PUT name (options` (upload). The host file is a temporary
//! storage queue (a name of up to 8 characters without periods, as CICS's
//! IND$FILE uses), or a catalogued sequential data set. Options: ASCII
//! (text in the host code page converted to and from the workstation's),
//! CRLF (records are lines), BINARY, APPEND, LRECL n and RECFM F | V for a
//! new data set; space options are accepted and ignored.
//!
//! The transfer is a sequence of Write Structured Field records and the
//! terminal's replies (AID X'88'), exchanged through the terminal's
//! converse hook (the server sends the records, not the screen, while
//! `Terminal::transfer` is on).

use zeus_cobol::ebcdic;
use zeus_ds::catalog::{Entry, EntryKind, Recfm, WriteMode};

use crate::region::{Region, Shared, Terminal};

/// Write Structured Field.
const WSF: u8 = 0xF3;
/// Data bytes per Insert (the default DFT buffer is 2K).
const CHUNK: usize = 2000;

/// Transfer options.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Options {
    ascii: bool,
    crlf: bool,
    append: bool,
    lrecl: Option<usize>,
    recfm: Option<char>,
}

/// The IND$FILE command: (GET?, host file, options) or a TRANSxx message.
fn parse(cmd: &str) -> Result<(bool, String, Options), String> {
    let (cmd, opts) = match cmd.split_once('(') {
        Some((c, o)) => (c, o.trim_end_matches(')')),
        None => (cmd, ""),
    };
    let w: Vec<&str> = cmd.split_whitespace().collect();
    let get = match w.get(1).map(|s| s.to_ascii_uppercase()).as_deref() {
        Some("GET") => true,
        Some("PUT") => false,
        _ => return Err("TRANS16 Incorrect request code: file transfer canceled".into()),
    };
    let Some(name) = w.get(2) else {
        return Err("TRANS17 Missing or incorrect file name: file transfer canceled".into());
    };
    let mut o = Options::default();
    let words: Vec<String> = opts
        .replace(['(', ')'], " ")
        .split_whitespace()
        .map(str::to_ascii_uppercase)
        .collect();
    let mut i = 0;
    while i < words.len() {
        let next = words.get(i + 1);
        match words[i].as_str() {
            "ASCII" => o.ascii = true,
            "CRLF" => o.crlf = true,
            "BINARY" | "NOCRLF" => {}
            "APPEND" => o.append = true,
            "LRECL" => {
                o.lrecl = next.and_then(|n| n.parse().ok());
                i += 1;
            }
            "RECFM" => {
                o.recfm = next.and_then(|r| r.chars().next());
                i += 1;
            }
            "BLKSIZE" | "SPACE" | "PRIMARY" | "SECONDARY" => i += 1,
            "TRACKS" | "CYLINDERS" | "AVBLOCK" => {}
            other => {
                return Err(format!(
                    "TRANS28 Invalid option {other}: file transfer canceled"
                ));
            }
        }
        i += 1;
    }
    Ok((get, name.to_ascii_uppercase(), o))
}

/// A structured field: the length, then the bytes.
fn sf(body: &[u8]) -> Vec<u8> {
    let mut v = ((body.len() + 2) as u16).to_be_bytes().to_vec();
    v.extend_from_slice(body);
    v
}

/// The DDM Open request: download (Insert), upload (Get) or messages.
fn open(upload: bool, messages: bool) -> Vec<u8> {
    let mut b = vec![0xD0, 0x00, 0x12, 0x01, 0x06, 0x01, 0x01, 0x04, 0x03];
    b.extend(if upload {
        [0x0A, 0x0A, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00]
    } else {
        [0x0A, 0x0A, 0x00, 0x00, 0x00, 0x00, 0x11, 0x01, 0x01, 0x00]
    });
    b.extend([0x50, 0x05, 0x52, 0x03, 0xF0, 0x03, 0x09]);
    // the description shows the message file as 'FT:' and blanks; the
    // emulators (x3270's family) recognize 'FT:MSG '
    b.extend_from_slice(if messages { b"FT:MSG " } else { b"FT:DATA" });
    sf(&b)
}

/// An Insert request with its data.
fn insert(data: &[u8]) -> Vec<u8> {
    let mut v = sf(&[0xD0, 0x47, 0x11, 0x01, 0x05, 0x00, 0x80, 0x00]);
    let mut d = vec![0xD0, 0x47, 0x04, 0xC0, 0x80, 0x61];
    d.extend(((data.len() + 5) as u16).to_be_bytes());
    d.extend_from_slice(data);
    v.extend(sf(&d));
    v
}

/// The structured fields of an inbound record (after the AID): their
/// bodies (without the length).
fn fields(data: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 2 <= data.len() {
        let l = u16::from_be_bytes([data[i], data[i + 1]]) as usize;
        if l < 2 || i + l > data.len() {
            break;
        }
        out.push(data[i + 2..i + l].to_vec());
        i += l;
    }
    out
}

/// Sends a Write Structured Field and waits for the terminal's reply: its
/// structured fields, or None when the operator pressed a key or left.
fn exchange(term: &Shared<Terminal>, sfs: &[u8]) -> Option<Vec<Vec<u8>>> {
    let mut rec = vec![WSF];
    rec.extend_from_slice(sfs);
    term.borrow_mut().outbound.push(rec);
    let hook = term.borrow_mut().converse.take()?;
    let mut hook = hook;
    let aid = hook(term);
    term.borrow_mut().converse = Some(hook);
    if aid != Some(0x88) {
        return None;
    }
    let data = term.borrow_mut().inbound_sf.take().unwrap_or_default();
    Some(fields(&data))
}

/// Whether a reply is the DDM reply `code` (e.g. D0 00 09, Open reply).
fn is(reply: &[Vec<u8>], code: [u8; 3]) -> bool {
    reply.iter().any(|f| f.len() >= 3 && f[..3] == code)
}

/// Runs an IND$FILE transfer; the TRANSxx message it ends with.
pub fn run(region: &Shared<Region>, term: &Shared<Terminal>, cmd: &str) -> String {
    term.borrow_mut().transfer = true;
    let msg = match parse(cmd) {
        Ok((true, name, o)) => download(region, term, &name, &o),
        Ok((false, name, o)) => upload(region, term, &name, &o),
        Err(m) => Err(m),
    };
    let msg = match msg {
        Ok(m) | Err(m) => m,
    };
    // the message goes to the emulator as a message "file"
    let sent = (|| {
        let r = exchange(term, &open(false, true))?;
        if !is(&r, [0xD0, 0x00, 0x09]) {
            return None;
        }
        exchange(term, &insert(msg.as_bytes()))
    })();
    if sent.is_none() {
        region
            .borrow_mut()
            .log
            .push(format!("IND$FILE: {msg} (not delivered)"));
    }
    term.borrow_mut().transfer = false;
    msg
}

/// A temporary storage queue name (no periods, up to 8 characters).
fn is_queue(name: &str) -> bool {
    !name.contains('.') && name.len() <= 8
}

/// Records as the workstation's file.
fn to_file(records: &[Vec<u8>], o: &Options) -> Vec<u8> {
    let mut out = Vec::new();
    for r in records {
        if o.ascii {
            let mut t = ebcdic::to_host(r);
            if o.crlf {
                while t.last() == Some(&b' ') {
                    t.pop();
                }
            }
            out.extend(t);
        } else {
            out.extend_from_slice(r);
        }
        if o.crlf {
            out.extend_from_slice(b"\r\n");
        }
    }
    out
}

/// The workstation's file as records of length `lrecl` (padded with
/// blanks when fixed).
fn to_records(data: &[u8], o: &Options, lrecl: usize, fixed: bool) -> Vec<Vec<u8>> {
    let pieces: Vec<Vec<u8>> = if o.crlf {
        let mut lines: Vec<&[u8]> = data.split(|&b| b == b'\n').collect();
        if lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        lines
            .into_iter()
            .map(|l| l.strip_suffix(b"\r").unwrap_or(l).to_vec())
            .collect()
    } else {
        data.chunks(lrecl.max(1)).map(<[u8]>::to_vec).collect()
    };
    pieces
        .into_iter()
        .map(|p| {
            let mut r = if o.ascii { ebcdic::from_host(&p) } else { p };
            if fixed {
                r.resize(lrecl, if o.ascii { ebcdic::SPACE } else { 0 });
            } else {
                r.truncate(lrecl);
            }
            r
        })
        .collect()
}

fn download(
    region: &Shared<Region>,
    term: &Shared<Terminal>,
    name: &str,
    o: &Options,
) -> Result<String, String> {
    let records = if is_queue(name) {
        region.borrow().ts.get(name).cloned().ok_or_else(|| {
            "TRANS17 Missing or incorrect TS queue name: file transfer canceled".to_string()
        })?
    } else {
        region.borrow().catalog.read_records(name).map_err(|_| {
            "TRANS17 Missing or incorrect data set name: file transfer canceled".to_string()
        })?
    };
    let data = to_file(&records, o);
    let aborted = || "TRANS99 Host program error code 00 00000001: file transfer canceled";
    let r = exchange(term, &open(false, false)).ok_or_else(aborted)?;
    if !is(&r, [0xD0, 0x00, 0x09]) {
        return Err("TRANS14 Error reading file from host: file transfer canceled".into());
    }
    for chunk in data.chunks(CHUNK) {
        let r = exchange(term, &insert(chunk)).ok_or_else(aborted)?;
        if !is(&r, [0xD0, 0x47, 0x05]) {
            return Err("TRANS14 Error reading file from host: file transfer canceled".into());
        }
    }
    let r = exchange(term, &sf(&[0xD0, 0x41, 0x12])).ok_or_else(aborted)?;
    if !is(&r, [0xD0, 0x41, 0x09]) {
        return Err("TRANS14 Error reading file from host: file transfer canceled".into());
    }
    Ok("TRANS03 File transfer complete".into())
}

fn upload(
    region: &Shared<Region>,
    term: &Shared<Terminal>,
    name: &str,
    o: &Options,
) -> Result<String, String> {
    let aborted = || "TRANS99 Host program error code 00 00000001: file transfer canceled";
    let r = exchange(term, &open(true, false)).ok_or_else(aborted)?;
    if !is(&r, [0xD0, 0x00, 0x09]) {
        return Err("TRANS13 Error writing file to host: file transfer canceled".into());
    }
    let mut data = Vec::new();
    let get = {
        let mut g = sf(&[
            0xD0, 0x45, 0x11, 0x01, 0x05, 0x00, 0x06, 0x00, 0x09, 0x05, 0x01, 0x03, 0x00,
        ]);
        g.extend(sf(&[0xD0, 0x46, 0x11, 0x01, 0x04, 0x00, 0x80]));
        g
    };
    loop {
        let r = exchange(term, &get).ok_or_else(aborted)?;
        // Data for Get: D0 46 05, record number, C0 80 61, length + 5, data
        if let Some(f) = r
            .iter()
            .find(|f| f.len() >= 14 && f[..3] == [0xD0, 0x46, 0x05])
        {
            let n = (u16::from_be_bytes([f[12], f[13]]) as usize).saturating_sub(5);
            data.extend_from_slice(&f[14..(14 + n).min(f.len())]);
            continue;
        }
        // Get past end of file
        if is(&r, [0xD0, 0x46, 0x08]) {
            break;
        }
        return Err("TRANS13 Error writing file to host: file transfer canceled".into());
    }
    let r = exchange(term, &sf(&[0xD0, 0x41, 0x12])).ok_or_else(aborted)?;
    if !is(&r, [0xD0, 0x41, 0x09]) {
        return Err("TRANS13 Error writing file to host: file transfer canceled".into());
    }
    let mut segmented = false;
    if is_queue(name) {
        let lrecl = o.lrecl.unwrap_or(if o.crlf { 32767 } else { 80 });
        let recs = to_records(&data, o, lrecl, false);
        let mut r = region.borrow_mut();
        let q = r.ts.entry(name.to_string()).or_default();
        if !o.append {
            q.clear();
        }
        q.extend(recs);
    } else {
        let mut r = region.borrow_mut();
        let existing = r.catalog.get(name).map(|e| e.kind.clone());
        let (recfm, lrecl) = match existing {
            Some(EntryKind::Sequential { recfm, lrecl }) => (recfm, lrecl as usize),
            Some(_) => {
                return Err(
                    "TRANS17 Missing or incorrect data set name: file transfer canceled".into(),
                );
            }
            None => {
                let lrecl = o.lrecl.unwrap_or(80);
                let recfm = if o.recfm == Some('V') {
                    Recfm::Vb
                } else {
                    Recfm::Fb
                };
                r.catalog
                    .define(Entry {
                        name: name.to_string(),
                        kind: EntryKind::Sequential {
                            recfm,
                            lrecl: lrecl as u32,
                        },
                    })
                    .map_err(
                        |_| "TRANS19 Error reading or writing to host disk: file transfer canceled",
                    )?;
                (recfm, lrecl)
            }
        };
        let fixed = matches!(recfm, Recfm::F | Recfm::Fb | Recfm::Fba);
        let full = to_records(&data, o, usize::MAX >> 1, false);
        segmented = full.iter().any(|x| x.len() > lrecl);
        let recs = to_records(&data, o, lrecl, fixed);
        let mode = if o.append {
            WriteMode::Append
        } else {
            WriteMode::Replace
        };
        r.catalog
            .write_records(name, &recs, mode)
            .and_then(|_| r.catalog.save())
            .map_err(|_| "TRANS19 Error reading or writing to host disk: file transfer canceled")?;
    }
    Ok(if segmented {
        "TRANS04 File transfer complete with records segmented".into()
    } else {
        "TRANS03 File transfer complete".into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command() {
        let (get, name, o) = parse("IND$FILE GET MYQUEUE (ASCII CRLF").unwrap();
        assert!(get);
        assert_eq!(name, "MYQUEUE");
        assert!(o.ascii && o.crlf && !o.append);
        let (get, _, o) = parse("IND$FILE PUT A.B.C (BINARY LRECL 100 RECFM V APPEND").unwrap();
        assert!(!get);
        assert_eq!((o.lrecl, o.recfm, o.append), (Some(100), Some('V'), true));
        assert!(
            parse("IND$FILE PUT X (BOGUS")
                .unwrap_err()
                .starts_with("TRANS28")
        );
        assert!(parse("IND$FILE COPY X").unwrap_err().starts_with("TRANS16"));
    }

    #[test]
    fn structured_fields() {
        // the formats of the description: Open for Download is 35 bytes
        let o = open(false, false);
        assert_eq!(o.len(), 0x23);
        assert_eq!(&o[..5], &[0x00, 0x23, 0xD0, 0x00, 0x12]);
        assert!(o.ends_with(b"FT:DATA"));
        let i = insert(b"AB");
        assert_eq!(
            &i[..10],
            &[0x00, 0x0A, 0xD0, 0x47, 0x11, 0x01, 0x05, 0x00, 0x80, 0x00]
        );
        assert_eq!(
            &i[10..],
            &[
                0x00, 0x0C, 0xD0, 0x47, 0x04, 0xC0, 0x80, 0x61, 0x00, 0x07, b'A', b'B'
            ]
        );
        assert_eq!(
            fields(&[0x00, 0x05, 0xD0, 0x00, 0x09]),
            vec![vec![0xD0, 0x00, 0x09]]
        );
    }

    #[test]
    fn records_and_files() {
        let o = Options {
            ascii: true,
            crlf: true,
            ..Options::default()
        };
        let recs = to_records(b"AB\r\nC\r\n", &o, 4, true);
        assert_eq!(recs, vec![ebcdic::encode("AB  "), ebcdic::encode("C   ")]);
        assert_eq!(to_file(&recs, &o), b"AB\r\nC\r\n");
        let bin = Options::default();
        assert_eq!(
            to_records(&[1, 2, 3], &bin, 2, true),
            vec![vec![1, 2], vec![3, 0]]
        );
    }
}
