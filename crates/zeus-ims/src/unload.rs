//! Strict reader for the non-HALDB DFSURGU0 (DFSURGUF) unload variant used
//! by CardDemo, and the writer of the same form. The VB RDW is retained
//! in downloaded sequential files.
//!
//! The records: a header (88 bytes, X'0080' after the RDW), a record per
//! segment in hierarchical order (segment code = position in the DBD,
//! X'80', X'0023', the data length, the segment name, the data at offset
//! 39 and a pad byte; the RDW length is the data length + 40) and a
//! trailer (88 bytes, X'0098').
use crate::Dbd;
use crate::call_adapter::PcbEncoding;

pub struct UnloadedSegment {
    pub segment: String,
    pub parent_index: Option<usize>,
    pub data: Vec<u8>,
}

fn be16(bytes: &[u8]) -> usize {
    usize::from(u16::from_be_bytes([bytes[0], bytes[1]]))
}

pub fn parse_dfsurgu0(input: &[u8], dbd: &Dbd) -> Result<Vec<UnloadedSegment>, String> {
    if dbd.access != "HIDAM" && dbd.access != "HDAM" {
        return Err("DFSURGU0 import currently requires HDAM or HIDAM".into());
    }
    let mut offset = 0;
    let mut records = Vec::new();
    // the last record of each segment type: the parent of what follows
    let mut last: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut saw_header = false;
    let mut saw_trailer = false;
    while offset < input.len() {
        if input.len() - offset < 4 {
            return Err(format!("truncated RDW at offset {offset}"));
        }
        let length = be16(&input[offset..offset + 2]);
        if length < 4 || length > input.len() - offset || input[offset + 2..offset + 4] != [0, 0] {
            return Err(format!("invalid VB record at offset {offset}"));
        }
        let record = &input[offset..offset + length];
        offset += length;
        if !saw_header {
            if length != 88 || record[4..6] != [0, 0x80] {
                return Err("DFSURGU0 header record is missing or unsupported".into());
            }
            saw_header = true;
            continue;
        }
        if length == 88 && record[4..6] == [0, 0x98] {
            if offset != input.len() {
                return Err("DFSURGU0 trailer is not the final record".into());
            }
            saw_trailer = true;
            break;
        }
        if length < 40
            || record[5] != 0x80
            || record[6..8] != [0, 0x23]
            || be16(&record[8..10]) != length - 40
        {
            return Err(format!(
                "unsupported DFSURGUF prefix at offset {}",
                offset - length
            ));
        }
        let segment_code = usize::from(record[4]);
        let definition = segment_code
            .checked_sub(1)
            .and_then(|index| dbd.segments.get(index))
            .ok_or_else(|| format!("unknown DFSURGUF segment code {segment_code}"))?;
        let name: Vec<u8> = record[10..18]
            .iter()
            .map(|byte| PcbEncoding::Cp1047.decode_control(*byte))
            .collect();
        if name != format!("{:<8}", definition.name).as_bytes() {
            return Err(format!(
                "DFSURGUF segment name does not match code {segment_code}"
            ));
        }
        // DFSURGUF uses a 39-byte prefix followed by the segment and one
        // zero pad byte. The RDW record length is still segment length + 40.
        if record[length - 1] != 0 {
            return Err(format!("invalid DFSURGUF pad at offset {}", offset - 1));
        }
        let data = record[39..length - 1].to_vec();
        definition.validate_data(&data)?;
        let parent_index = match &definition.parent {
            Some(p) => Some(
                *last
                    .get(p)
                    .ok_or_else(|| format!("segment {} precedes its parent", definition.name))?,
            ),
            None => None,
        };
        // a new parent ends the children of the previous one
        let mut below = vec![definition.name.clone()];
        while let Some(n) = below.pop() {
            last.remove(&n);
            below.extend(
                dbd.segments
                    .iter()
                    .filter(|s| s.parent.as_deref() == Some(n.as_str()))
                    .map(|s| s.name.clone()),
            );
        }
        last.insert(definition.name.clone(), records.len());
        records.push(UnloadedSegment {
            segment: definition.name.clone(),
            parent_index,
            data,
        });
    }
    if !saw_header || !saw_trailer {
        return Err("DFSURGU0 header or trailer is missing".into());
    }
    Ok(records)
}

/// One VB record of the unload.
fn push_record(out: &mut Vec<u8>, body: &[u8]) {
    out.extend(((body.len() + 4) as u16).to_be_bytes());
    out.extend([0, 0]);
    out.extend(body);
}

/// Writes the unload of a database's segments (in hierarchical order,
/// each with the index of its parent among them).
pub fn write_dfsurgu0(segments: &[UnloadedSegment], dbd: &Dbd) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    // the header and trailer: their type and the DBD name
    let mut header = vec![0u8; 84];
    header[1] = 0x80;
    header[2..4].copy_from_slice(&[0, 0x50]);
    let dbd_name = PcbEncoding::Cp1047
        .encode_control(format!("{:<8}", dbd.name).as_bytes())
        .map_err(|e| e.to_string())?;
    header[4..12].copy_from_slice(&dbd_name);
    let mut trailer = header.clone();
    trailer[1] = 0x98;
    push_record(&mut out, &header);
    for s in segments {
        let code = dbd
            .segments
            .iter()
            .position(|d| d.name == s.segment)
            .ok_or_else(|| format!("segment {} is not in DBD {}", s.segment, dbd.name))?
            + 1;
        let mut body = vec![0u8; 36 + s.data.len()];
        body[0] = code as u8;
        body[1] = 0x80;
        body[2..4].copy_from_slice(&[0, 0x23]);
        body[4..6].copy_from_slice(&(s.data.len() as u16).to_be_bytes());
        let name = PcbEncoding::Cp1047
            .encode_control(format!("{:<8}", s.segment).as_bytes())
            .map_err(|e| e.to_string())?;
        body[6..14].copy_from_slice(&name);
        body[35..35 + s.data.len()].copy_from_slice(&s.data);
        push_record(&mut out, &body);
    }
    push_record(&mut out, &trailer);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::parse_dbd;

    fn record(kind: u8, name: &str, data: &[u8]) -> Vec<u8> {
        let length = if kind == 0 || kind == 0x98 {
            88
        } else {
            40 + data.len()
        };
        let mut bytes = vec![0; length];
        bytes[0..2].copy_from_slice(&(length as u16).to_be_bytes());
        bytes[4] = if kind == 0x98 { 0 } else { kind };
        bytes[5] = if kind == 0x98 { 0x98 } else { 0x80 };
        if kind != 0 && kind != 0x98 {
            bytes[6..8].copy_from_slice(&[0, 0x23]);
            bytes[8..10].copy_from_slice(&(data.len() as u16).to_be_bytes());
            bytes[10..18].copy_from_slice(
                &PcbEncoding::Cp1047
                    .encode_control(format!("{name:<8}").as_bytes())
                    .unwrap(),
            );
            bytes[39..length - 1].copy_from_slice(data);
        }
        bytes
    }

    #[test]
    fn parses_parent_child_and_rejects_corrupt_prefix() {
        let dbd = parse_dbd(
            " DBD NAME=TEST,ACCESS=(HIDAM,VSAM)\n\
            SEGM NAME=ROOT,PARENT=0,BYTES=6\n\
            FIELD NAME=(ID,SEQ,U),START=1,BYTES=2,TYPE=P\n\
            SEGM NAME=CHILD,PARENT=((ROOT,)),BYTES=5\n\
            FIELD NAME=(SEQNO,SEQ,U),START=1,BYTES=2,TYPE=C\n\
            DBDGEN\n END\n",
        )
        .unwrap();
        let mut input = Vec::new();
        input.extend(record(0, "", &[]));
        input.extend(record(1, "ROOT", &[0x12, 0x3c, 0, 0xff, 0, 0]));
        input.extend(record(2, "CHILD", &[0x40, 0x41, 0x0e, 0x42, 0x0f]));
        input.extend(record(0x98, "", &[]));
        let rows = parse_dfsurgu0(&input, &dbd).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].parent_index, None);
        assert_eq!(rows[1].parent_index, Some(0));
        assert_eq!(rows[1].data, [0x40, 0x41, 0x0e, 0x42, 0x0f]);
        let mut broken = input.clone();
        broken[88 + 8] = 0xff;
        assert!(parse_dfsurgu0(&broken, &dbd).is_err());
        let mut broken = input.clone();
        broken[88 + 40 + 6 - 1] = 0xff;
        assert!(parse_dfsurgu0(&broken, &dbd).is_err());
        assert!(parse_dfsurgu0(&input[..input.len() - 1], &dbd).is_err());
        // written back the same (the header and trailer carry the DBD name)
        let back = write_dfsurgu0(&rows, &dbd).unwrap();
        assert_eq!(back[88..back.len() - 88], input[88..input.len() - 88]);
        assert_eq!(parse_dfsurgu0(&back, &dbd).unwrap().len(), 2);
    }

    #[test]
    fn three_levels_round_trip() {
        let dbd = parse_dbd(
            " DBD NAME=TEST,ACCESS=(HIDAM,VSAM)\n\
            SEGM NAME=A,PARENT=0,BYTES=2\n\
            FIELD NAME=(AK,SEQ,U),START=1,BYTES=2,TYPE=C\n\
            SEGM NAME=B,PARENT=((A,)),BYTES=2\n\
            FIELD NAME=(BK,SEQ,U),START=1,BYTES=2,TYPE=C\n\
            SEGM NAME=C,PARENT=((B,)),BYTES=2\n\
            FIELD NAME=(CK,SEQ,U),START=1,BYTES=2,TYPE=C\n\
            SEGM NAME=D,PARENT=((A,)),BYTES=2\n\
            FIELD NAME=(DK,SEQ,U),START=1,BYTES=2,TYPE=C\n\
            DBDGEN\n END\n",
        )
        .unwrap();
        let seg = |segment: &str, parent_index, data: &[u8]| UnloadedSegment {
            segment: segment.into(),
            parent_index,
            data: data.to_vec(),
        };
        let rows = vec![
            seg("A", None, b"a1"),
            seg("B", Some(0), b"b1"),
            seg("C", Some(1), b"c1"),
            seg("B", Some(0), b"b2"),
            seg("D", Some(0), b"d1"),
            seg("A", None, b"a2"),
            seg("D", Some(5), b"d2"),
        ];
        let bytes = write_dfsurgu0(&rows, &dbd).unwrap();
        let back = parse_dfsurgu0(&bytes, &dbd).unwrap();
        let shape: Vec<_> = back
            .iter()
            .map(|r| (r.segment.as_str(), r.parent_index, r.data.as_slice()))
            .collect();
        let want: Vec<_> = rows
            .iter()
            .map(|r| (r.segment.as_str(), r.parent_index, r.data.as_slice()))
            .collect();
        assert_eq!(shape, want);
        // a C before any B is rejected
        let bad = write_dfsurgu0(&[seg("A", None, b"a1"), seg("C", Some(0), b"c1")], &dbd).unwrap();
        assert!(parse_dfsurgu0(&bad, &dbd).is_err());
    }
}
