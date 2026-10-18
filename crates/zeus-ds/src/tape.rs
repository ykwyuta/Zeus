//! Uncompressed AWSTAPE images, with physical blocks and tape marks.
use crate::{DsError, Result};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Entry {
    Block(Vec<u8>),
    Mark,
}

/// Retention in a standard label; no-scratch dates are independent of today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Expiration {
    Date { year: u16, day: u16 },
    Never,
}

impl Expiration {
    fn date(year: u16, day: u16) -> Result<Self> {
        let leap =
            year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
        if !(1900..=2155).contains(&year) || day == 0 || day > if leap { 366 } else { 365 } {
            return Err(DsError::Damaged("invalid tape expiration date".into()));
        }
        Ok(Self::Date { year, day })
    }

    pub fn from_jcl(value: &str) -> Result<Self> {
        if matches!(value, "99365" | "99366") {
            return Ok(Self::Never);
        }
        let (year, day) = if value.len() == 5 && value.bytes().all(|b| b.is_ascii_digit()) {
            (
                1900 + value[..2].parse::<u16>().unwrap(),
                value[2..].parse::<u16>().unwrap(),
            )
        } else if value.len() == 8
            && value.as_bytes()[4] == b'/'
            && value
                .bytes()
                .enumerate()
                .all(|(i, b)| i == 4 || b.is_ascii_digit())
        {
            (
                value[..4].parse::<u16>().unwrap(),
                value[5..].parse::<u16>().unwrap(),
            )
        } else {
            return Err(DsError::Damaged("invalid EXPDT syntax".into()));
        };
        Self::date(year, day)
    }

    /// The six host-character bytes after EBCDIC decoding.
    pub fn from_label(field: &[u8]) -> Result<Option<Self>> {
        if field.len() != 6 {
            return Err(DsError::Damaged("invalid expiration field".into()));
        }
        if field.iter().all(|&b| b == b' ' || b == b'0') {
            return Ok(None);
        }
        if field[0] == b' ' && matches!(&field[1..], b"99365" | b"99366") {
            return Ok(Some(Self::Never));
        }
        if !field[1..].iter().all(u8::is_ascii_digit)
            || !(field[0] == b' ' || field[0].is_ascii_digit())
        {
            return Err(DsError::Damaged("invalid expiration field".into()));
        }
        let century = if field[0] == b' ' {
            19
        } else {
            20 + u16::from(field[0] - b'0')
        };
        let number = |s: &[u8]| std::str::from_utf8(s).unwrap().parse::<u16>().unwrap();
        Self::date(century * 100 + number(&field[1..3]), number(&field[3..6])).map(Some)
    }

    pub fn label(self) -> Result<[u8; 6]> {
        match self {
            Self::Never => Ok(*b" 99365"),
            Self::Date { year, day } => {
                Self::date(year, day)?;
                let century = if year < 2000 {
                    b' '
                } else {
                    b'0' + (year / 100 - 20) as u8
                };
                let mut field = [century; 6];
                field[1..].copy_from_slice(format!("{:02}{day:03}", year % 100).as_bytes());
                Ok(field)
            }
        }
    }

    pub fn retained(self, year: u16, day: u16) -> bool {
        match self {
            Self::Never => true,
            Self::Date { year: y, day: d } => (y, d) > (year, day),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TapeFile {
    pub start: usize,
    pub next: usize,
    pub blocks: Vec<Vec<u8>>,
    pub data_start: usize,
    pub trailer_start: usize,
    pub continued: bool,
}

fn label(block: &[u8], id: &[u8]) -> bool {
    // Labels use the invariant EBCDIC uppercase letters/digits.
    let e: Vec<u8> = id
        .iter()
        .map(|b| match b {
            b'A'..=b'I' => 0xc1 + (b - b'A'),
            b'J'..=b'R' => 0xd1 + (b - b'J'),
            b'S'..=b'Z' => 0xe2 + (b - b'S'),
            b'0'..=b'9' => 0xf0 + (b - b'0'),
            _ => 0x40,
        })
        .collect();
    block.len() == 80 && block.starts_with(&e)
}

pub fn file(entries: &[Entry], position: usize) -> Result<TapeFile> {
    let labeled = matches!(entries.first(), Some(Entry::Block(b)) if label(b, b"VOL1"));
    file_with_labels(entries, position, labeled)
}

pub fn file_with_labels(entries: &[Entry], position: usize, labeled: bool) -> Result<TapeFile> {
    let start = position;
    let mut pos = position;
    if labeled
        && position == 0
        && !matches!(entries.get(pos), Some(Entry::Block(b)) if label(b, b"VOL1"))
    {
        return Err(DsError::Damaged("missing VOL1 label".into()));
    }
    if labeled
        && let Some(Entry::Block(b)) = entries.get(pos)
        && label(b, b"VOL1")
    {
        pos += 1;
    }
    if labeled && !matches!(entries.get(pos), Some(Entry::Block(b)) if label(b, b"HDR1")) {
        return Err(DsError::Damaged("missing HDR1 label".into()));
    }
    if labeled
        && let Some(Entry::Block(b)) = entries.get(pos)
        && label(b, b"HDR1")
    {
        while matches!(entries.get(pos), Some(Entry::Block(_))) {
            pos += 1;
        }
        if entries.get(pos) != Some(&Entry::Mark) {
            return Err(DsError::Damaged("missing tape header mark".into()));
        }
        pos += 1;
    }
    let data_start = pos;
    let mut blocks = Vec::new();
    while let Some(Entry::Block(b)) = entries.get(pos) {
        blocks.push(b.clone());
        pos += 1;
    }
    if entries.get(pos) != Some(&Entry::Mark) {
        return Err(DsError::Damaged("missing data tape mark".into()));
    }
    pos += 1;
    let trailer_start = pos;
    let continued =
        labeled && matches!(entries.get(pos), Some(Entry::Block(b)) if label(b, b"EOV1"));
    if labeled
        && !matches!(entries.get(pos), Some(Entry::Block(b)) if label(b, if continued { b"EOV1" } else { b"EOF1" }))
    {
        return Err(DsError::Damaged("missing EOF1/EOV1 label".into()));
    }
    if labeled
        && let Some(Entry::Block(b)) = entries.get(pos)
        && (label(b, b"EOF1") || label(b, b"EOV1"))
    {
        while matches!(entries.get(pos), Some(Entry::Block(_))) {
            pos += 1;
        }
        if entries.get(pos) != Some(&Entry::Mark) {
            return Err(DsError::Damaged("missing trailer tape mark".into()));
        }
        pos += 1;
    }
    Ok(TapeFile {
        start,
        next: pos,
        blocks,
        data_start,
        trailer_start,
        continued,
    })
}

/// Mount ordered physical volumes as one logical stream. EOV is a volume
/// boundary, never a logical end of file. Header identity and format must agree.
pub fn continues(entries: &[Entry], labeled: bool) -> Result<bool> {
    let mut position = 0;
    while position < entries.len() && entries[position] != Entry::Mark {
        let file = file_with_labels(entries, position, labeled)?;
        if file.continued {
            return Ok(true);
        }
        position = file.next;
    }
    Ok(false)
}

fn label_number(bytes: &[u8]) -> Result<usize> {
    bytes.iter().try_fold(0usize, |number, &digit| {
        if (0xf0..=0xf9).contains(&digit) {
            Ok(number * 10 + (digit - 0xf0) as usize)
        } else {
            Err(DsError::Damaged("invalid tape label numeric field".into()))
        }
    })
}

/// IBM label 1 uses six low-order digits and four optional high-order
/// digits. High-order zeroes are blanks; reserved bytes 73..76 are separate.
pub fn block_count(label: &[u8]) -> Result<usize> {
    if label.len() != 80 {
        return Err(DsError::Damaged("invalid block-count label length".into()));
    }
    let high = &label[76..80];
    let first = high
        .iter()
        .position(|&byte| byte != 0x40)
        .unwrap_or(high.len());
    let high = if first == high.len() {
        0
    } else {
        label_number(&high[first..])?
    };
    high.checked_mul(1_000_000)
        .and_then(|n| n.checked_add(label_number(&label[54..60]).ok()?))
        .ok_or_else(|| DsError::Damaged("invalid tape block count".into()))
}

pub fn write_block_count(label: &mut [u8], count: usize) -> Result<()> {
    if label.len() != 80 || count > 9_999_999_999usize {
        return Err(DsError::Damaged(
            "tape block count exceeds label capacity".into(),
        ));
    }
    decimal_field(label, 54, 6, count % 1_000_000);
    label[76..80].fill(0x40);
    if count >= 1_000_000 {
        let digits = (count / 1_000_000).to_string();
        for (target, digit) in label[80 - digits.len()..].iter_mut().zip(digits.bytes()) {
            *target = 0xf0 + (digit - b'0');
        }
    }
    Ok(())
}

/// Dataset sequence field of label 1 (decimal or extended binary form).
pub fn data_sequence(label: &[u8]) -> Result<usize> {
    if label.len() != 80 {
        return Err(DsError::Damaged("invalid sequence label length".into()));
    }
    let number = if label[31] == 0x6f {
        u32::from_be_bytes([0, label[32], label[33], label[34]]) as usize
    } else {
        label_number(&label[31..35])?
    };
    if !(1..=65535).contains(&number) {
        return Err(DsError::Damaged(
            "tape dataset sequence out of range".into(),
        ));
    }
    Ok(number)
}

pub fn write_data_sequence(label: &mut [u8], number: usize) -> Result<()> {
    if label.len() != 80 || !(1..=65535).contains(&number) {
        return Err(DsError::Damaged(
            "tape dataset sequence out of range".into(),
        ));
    }
    if number <= 9999 {
        decimal_field(label, 31, 4, number);
    } else {
        label[31] = 0x6f;
        label[32..35].copy_from_slice(&(number as u32).to_be_bytes()[1..]);
    }
    Ok(())
}

pub fn join_volumes(volumes: &[Vec<Entry>], labeled: bool) -> Result<Vec<Entry>> {
    join_volumes_with_warnings(volumes, labeled, |_| {})
}

/// Assembles volumes while reporting nonfatal serial/order diagnostics.
/// Dataset identity, physical format and block counts remain fatal checks.
pub fn join_volumes_with_warnings(
    volumes: &[Vec<Entry>],
    labeled: bool,
    mut warning: impl FnMut(&str),
) -> Result<Vec<Entry>> {
    let mut out = Vec::new();
    let mut expected: Option<Vec<Vec<u8>>> = None;
    for volume in volumes {
        let mut position = 0;
        while position < volume.len() && volume[position] != Entry::Mark {
            let file = file_with_labels(volume, position, labeled)?;
            let headers: Vec<Vec<u8>> = volume[position..file.data_start]
                .iter()
                .filter_map(|e| match e {
                    Entry::Block(b) if label(b, b"HDR1") || label(b, b"HDR2") => Some(b.clone()),
                    _ => None,
                })
                .collect();
            let continuing = expected.is_some();
            if labeled {
                let Some(Entry::Block(trailer)) = volume.get(file.trailer_start) else {
                    return Err(DsError::Damaged("missing tape trailer".into()));
                };
                if headers.is_empty()
                    || trailer[4..21] != headers[0][4..21]
                    || data_sequence(trailer)? != data_sequence(&headers[0])?
                    || block_count(trailer)? != file.blocks.len()
                {
                    return Err(DsError::Damaged(
                        "tape header/trailer identity or block count mismatch".into(),
                    ));
                }
            }
            if let Some(prior) = expected.take() {
                if headers.is_empty()
                    || prior.is_empty()
                    || headers[0][4..21] != prior[0][4..21]
                    || data_sequence(&headers[0])? != data_sequence(&prior[0])?
                    || headers
                        .get(1)
                        .zip(prior.get(1))
                        .is_some_and(|(current, previous)| {
                            current[4..15] != previous[4..15]
                                || current[38] != previous[38]
                                || current[5..10] == [0xf0; 5]
                                    && current[70..80] != previous[70..80]
                        })
                {
                    return Err(DsError::Damaged("EOV continuation header mismatch".into()));
                }
                if headers[0][21..27] != prior[0][21..27]
                    || label_number(&headers[0][27..31])? != label_number(&prior[0][27..31])? + 1
                {
                    warning("IEC709I: tape volume set serial or volume sequence is inconsistent");
                }
            }
            let start = if continuing {
                file.data_start
            } else if !out.is_empty()
                && matches!(volume.get(position), Some(Entry::Block(b)) if label(b, b"VOL1"))
            {
                position + 1
            } else {
                position
            };
            if file.continued {
                out.extend_from_slice(&volume[start..file.trailer_start - 1]);
                expected = Some(headers);
            } else {
                out.extend_from_slice(&volume[start..file.next]);
            }
            position = file.next;
            if file.continued {
                break;
            }
        }
    }
    if expected.is_some() {
        return Err(DsError::Damaged("missing EOV continuation volume".into()));
    }
    if !out.is_empty() {
        out.push(Entry::Mark);
    }
    Ok(out)
}

fn decimal_field(label: &mut [u8], start: usize, width: usize, number: usize) {
    let text = format!("{number:0width$}");
    for (target, byte) in label[start..start + width].iter_mut().zip(text.bytes()) {
        *target = 0xf0 + (byte - b'0');
    }
}

/// NL images contain no EOV label. The virtual volume directory supplies
/// whether the last file of each physical volume continues on the next.
pub fn join_unlabeled(volumes: &[Vec<Entry>], continued: &[bool]) -> Result<Vec<Entry>> {
    if volumes.len() != continued.len() || continued.last() == Some(&true) {
        return Err(DsError::Damaged("invalid NL volume directory".into()));
    }
    let mut logical = vec![];
    for (entries, continuation) in volumes.iter().zip(continued) {
        let mut position = 0;
        loop {
            let file = file_with_labels(entries, position, false)?;
            let last = entries.get(file.next).is_none_or(|e| *e == Entry::Mark);
            let end = if last && *continuation {
                file.next - 1
            } else {
                file.next
            };
            logical.extend_from_slice(&entries[position..end]);
            if last {
                break;
            }
            position = file.next;
        }
    }
    logical.push(Entry::Mark);
    Ok(logical)
}

pub fn split_unlabeled(entries: &[Entry], capacity: usize) -> Result<(Vec<Vec<Entry>>, Vec<bool>)> {
    split_unlabeled_at(entries, capacity, &[])
}

/// Explicit boundaries identify a logical file and the number of blocks
/// written before CLOSE REEL. They flush a section even below capacity.
pub fn split_unlabeled_at(
    entries: &[Entry],
    capacity: usize,
    breaks: &[(usize, usize)],
) -> Result<(Vec<Vec<Entry>>, Vec<bool>)> {
    if capacity == 0 {
        return Err(DsError::Damaged("zero tape volume capacity".into()));
    }
    let mut volumes = vec![];
    let mut continued = vec![];
    let mut current = vec![];
    let mut used = 0;
    let mut position = 0;
    let mut number = 0;
    while position < entries.len() && (entries[position] != Entry::Mark || current.is_empty()) {
        let file = file_with_labels(entries, position, false)?;
        if file
            .blocks
            .first()
            .is_some_and(|b| used + b.len() > capacity)
            && used != 0
        {
            current.push(Entry::Mark);
            volumes.push(std::mem::take(&mut current));
            continued.push(false);
            used = 0;
        }
        for index in 0..=file.blocks.len() {
            let block = file.blocks.get(index);
            if block.is_some_and(|b| b.len() > capacity) {
                return Err(DsError::Damaged(
                    "block exceeds tape volume capacity".into(),
                ));
            }
            let forced = breaks
                .iter()
                .filter(|&&(f, b)| f == number && b == index)
                .count();
            let switches = forced.max(usize::from(
                block.is_some_and(|b| used + b.len() > capacity),
            ));
            for _ in 0..switches {
                current.push(Entry::Mark);
                current.push(Entry::Mark);
                volumes.push(std::mem::take(&mut current));
                continued.push(true);
                used = 0;
            }
            if let Some(block) = block {
                current.push(Entry::Block(block.clone()));
                used += block.len();
            }
        }
        current.push(Entry::Mark);
        position = file.next;
        number += 1;
        if entries.get(position).is_none_or(|e| *e == Entry::Mark) {
            break;
        }
    }
    current.push(Entry::Mark);
    volumes.push(current);
    continued.push(false);
    Ok((volumes, continued))
}

/// Split a labeled logical stream at physical block boundaries. The limit
/// counts data bytes; labels and marks do not consume simulated data capacity.
pub fn split_volumes(entries: &[Entry], capacity: usize) -> Result<Vec<Vec<Entry>>> {
    split_volumes_at(entries, capacity, &[])
}

pub fn split_volumes_at(
    entries: &[Entry],
    capacity: usize,
    breaks: &[(usize, usize)],
) -> Result<Vec<Vec<Entry>>> {
    if capacity == 0 {
        return Err(DsError::Damaged("zero tape volume capacity".into()));
    }
    let volume_label = match entries.first() {
        Some(Entry::Block(b)) if label(b, b"VOL1") => b.clone(),
        _ => {
            return Err(DsError::Damaged(
                "volume splitting requires standard labels".into(),
            ));
        }
    };
    let mut result = Vec::new();
    let mut current = vec![Entry::Block(volume_label.clone())];
    let mut used = 0;
    let mut position = 0;
    let mut number = 0;
    while position < entries.len() && entries[position] != Entry::Mark {
        let file = file_with_labels(entries, position, true)?;
        if file.continued {
            return Err(DsError::Damaged(
                "split requires a mounted logical file".into(),
            ));
        }
        let headers: Vec<Entry> = entries[position..file.data_start]
            .iter()
            .filter(|e| !matches!(e, Entry::Block(b) if label(b, b"VOL1")))
            .cloned()
            .collect();
        let trailers = &entries[file.trailer_start..file.next];
        let mut section = 1;
        let mut count = 0;
        // A new data set starts on the next volume if its first block does
        // not fit. There is no empty continuation section on the old volume.
        if file
            .blocks
            .first()
            .is_some_and(|block| used + block.len() > capacity)
            && used != 0
        {
            current.push(Entry::Mark);
            result.push(current);
            current = vec![Entry::Block(volume_label.clone())];
            used = 0;
        }
        current.extend(headers.clone());
        for index in 0..=file.blocks.len() {
            let block = file.blocks.get(index);
            if block.is_some_and(|b| b.len() > capacity) {
                return Err(DsError::Damaged(
                    "block exceeds tape volume capacity".into(),
                ));
            }
            let forced = breaks
                .iter()
                .filter(|&&(f, b)| f == number && b == index)
                .count();
            let switches = forced.max(usize::from(
                block.is_some_and(|b| used + b.len() > capacity),
            ));
            for _ in 0..switches {
                current.push(Entry::Mark);
                for entry in trailers {
                    if matches!(entry, Entry::Block(b) if label(b, b"UTL1") || b.starts_with(&[0xe4, 0xe3, 0xd3]))
                    {
                        continue;
                    }
                    let mut entry = entry.clone();
                    if let Entry::Block(b) = &mut entry {
                        if label(b, b"EOF1") {
                            decimal_field(b, 27, 4, section);
                            write_block_count(b, count)?;
                        }
                        if label(b, b"EOF1") || label(b, b"EOF2") {
                            b[2] = 0xe5;
                        }
                    }
                    current.push(entry);
                }
                current.push(Entry::Mark);
                result.push(current);
                section += 1;
                current = vec![Entry::Block(volume_label.clone())];
                for entry in &headers {
                    if matches!(entry, Entry::Block(b) if b.starts_with(&[0xe4, 0xc8, 0xd3])) {
                        continue;
                    }
                    let mut entry = entry.clone();
                    if let Entry::Block(b) = &mut entry
                        && label(b, b"HDR1")
                    {
                        decimal_field(b, 27, 4, section);
                    }
                    current.push(entry);
                }
                used = 0;
                count = 0;
            }
            if let Some(block) = block {
                used += block.len();
                count += 1;
                current.push(Entry::Block(block.clone()));
            }
        }
        current.push(Entry::Mark);
        for entry in trailers {
            let mut entry = entry.clone();
            if let Entry::Block(b) = &mut entry
                && label(b, b"EOF1")
            {
                decimal_field(b, 27, 4, section);
                write_block_count(b, count)?;
            }
            current.push(entry);
        }
        position = file.next;
        number += 1;
    }
    current.push(Entry::Mark);
    result.push(current);
    Ok(result)
}

pub fn sequence(entries: &[Entry], number: usize) -> Result<usize> {
    let labeled = matches!(entries.first(), Some(Entry::Block(b)) if label(b, b"VOL1"));
    sequence_with_labels(entries, number, labeled)
}

pub fn sequence_with_labels(entries: &[Entry], number: usize, labeled: bool) -> Result<usize> {
    if number == 0 {
        return Err(DsError::Damaged(
            "tape file sequence must be positive".into(),
        ));
    }
    let mut first_sequence = 1;
    if labeled && number > 1 && !entries.is_empty() {
        let first = file_with_labels(entries, 0, true)?;
        let header = entries[..first.data_start]
            .iter()
            .find_map(|entry| match entry {
                Entry::Block(b) if label(b, b"HDR1") => Some(b),
                _ => None,
            })
            .ok_or_else(|| DsError::Damaged("missing first tape header".into()))?;
        first_sequence = data_sequence(header)?;
    }
    if number < first_sequence {
        return Err(DsError::Damaged(
            "requested tape dataset precedes the mounted aggregate".into(),
        ));
    }
    let mut position = 0;
    for _ in first_sequence..number {
        position = file_with_labels(entries, position, labeled)?.next;
    }
    Ok(position)
}

pub fn decode(bytes: &[u8]) -> Result<Vec<Entry>> {
    let bad = || DsError::Damaged("invalid AWSTAPE header/segment".into());
    let mut entries = Vec::new();
    let mut offset = 0;
    let mut previous = 0;
    let mut pending: Option<Vec<u8>> = None;
    while offset < bytes.len() {
        let h = bytes.get(offset..offset + 6).ok_or_else(bad)?;
        let n = u16::from_le_bytes([h[0], h[1]]) as usize;
        let prev = u16::from_le_bytes([h[2], h[3]]) as usize;
        if prev != previous || h[5] != 0 || h[4] & !0xe0 != 0 {
            return Err(bad());
        }
        offset += 6;
        let data = bytes.get(offset..offset + n).ok_or_else(bad)?;
        if h[4] == 0x40 {
            if n != 0 || pending.is_some() {
                return Err(bad());
            }
            entries.push(Entry::Mark);
        } else {
            if n == 0 || h[4] & 0x40 != 0 {
                return Err(bad());
            }
            if h[4] & 0x80 != 0 {
                if pending.is_some() {
                    return Err(bad());
                }
                pending = Some(Vec::new());
            }
            pending.as_mut().ok_or_else(bad)?.extend_from_slice(data);
            if h[4] & 0x20 != 0 {
                entries.push(Entry::Block(pending.take().ok_or_else(bad)?));
            }
        }
        previous = n;
        offset += n;
    }
    if pending.is_some() {
        return Err(bad());
    }
    Ok(entries)
}

pub fn encode(entries: &[Entry]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut previous = 0u16;
    for entry in entries {
        let Entry::Block(data) = entry else {
            bytes.extend_from_slice(&0u16.to_le_bytes());
            bytes.extend_from_slice(&previous.to_le_bytes());
            bytes.extend_from_slice(&[0x40, 0]);
            previous = 0;
            continue;
        };
        if data.is_empty() {
            return Err(DsError::Damaged("empty AWSTAPE data block".into()));
        }
        let count = data.len().div_ceil(u16::MAX as usize);
        for (index, segment) in data.chunks(u16::MAX as usize).enumerate() {
            let n = segment.len() as u16;
            let flag =
                if index == 0 { 0x80 } else { 0 } | if index + 1 == count { 0x20 } else { 0 };
            bytes.extend_from_slice(&n.to_le_bytes());
            bytes.extend_from_slice(&previous.to_le_bytes());
            bytes.extend_from_slice(&[flag, 0]);
            bytes.extend_from_slice(segment);
            previous = n;
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    #[test]
    fn expiration_dates_validate_leap_days_centuries_and_never_scratch() {
        for date in [
            "00001", "96366", "2000/366", "2100/365", "2155/365", "99365", "99366",
        ] {
            let value = super::Expiration::from_jcl(date).unwrap();
            assert_eq!(
                super::Expiration::from_label(&value.label().unwrap()).unwrap(),
                Some(value)
            );
        }
        for invalid in [
            "2100/366", "2001/366", "2156/001", "2026/000", "2026001", "2026/x01",
        ] {
            assert!(super::Expiration::from_jcl(invalid).is_err());
        }
        assert!(super::Expiration::Never.retained(2155, 365));
        let value = super::Expiration::from_jcl("2026/291").unwrap();
        assert!(value.retained(2026, 290));
        assert!(!value.retained(2026, 291));
        assert_eq!(super::Expiration::from_label(b" 00000").unwrap(), None);
        assert!(super::Expiration::from_label(b"0ABCDE").is_err());
    }
    #[test]
    fn large_blocks_round_trip_across_aws_segments_and_marks() {
        let data: Vec<u8> = (0..150000).map(|n| (n % 251) as u8).collect();
        let entries = vec![
            super::Entry::Block(data),
            super::Entry::Mark,
            super::Entry::Block(vec![7; 65536]),
            super::Entry::Mark,
        ];
        let encoded = super::encode(&entries).unwrap();
        assert_eq!(encoded[4], 0x80);
        assert_eq!(encoded[65535 + 6 + 4], 0);
        assert_eq!(encoded[2 * (65535 + 6) + 4], 0x20);
        assert_eq!(super::decode(&encoded).unwrap(), entries);
    }
    #[test]
    fn dataset_sequences_use_decimal_and_extended_binary_fields() {
        let mut label = [0x40; 80];
        for number in [1, 9999, 10000, 65535] {
            super::write_data_sequence(&mut label, number).unwrap();
            assert_eq!(super::data_sequence(&label).unwrap(), number);
            assert_eq!(label[31] == 0x6f, number > 9999);
        }
        for number in [0, 65536] {
            assert!(super::write_data_sequence(&mut label, number).is_err());
        }
        label[31..35].copy_from_slice(&[0x6f, 1, 0, 0]);
        assert!(super::data_sequence(&label).is_err());
    }
    use super::*;
    #[test]
    fn tape_counts_use_both_fields_and_validate_optional_high_digits() {
        for count in [0, 1, 999_999, 1_000_000, 9_999_999_999] {
            let mut label = vec![0x40; 80];
            write_block_count(&mut label, count).unwrap();
            assert_eq!(block_count(&label).unwrap(), count);
            assert_eq!(&label[73..76], &[0x40; 3]);
        }
        let mut label = vec![0x40; 80];
        write_block_count(&mut label, 1_000_001).unwrap();
        assert_eq!(&label[54..60], &[0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF1]);
        assert_eq!(&label[76..80], &[0x40, 0x40, 0x40, 0xF1]);
        label[78] = 0xC1;
        assert!(block_count(&label).is_err());
        assert!(write_block_count(&mut label, 10_000_000_000).is_err());
        assert!(block_count(&label[..79]).is_err());
    }
    #[test]
    fn unlabeled_directory_preserves_continuations_and_natural_file_boundaries() {
        for source in [
            vec![
                Entry::Block(b"AA".to_vec()),
                Entry::Block(b"BB".to_vec()),
                Entry::Mark,
                Entry::Mark,
            ],
            vec![
                Entry::Block(b"AAA".to_vec()),
                Entry::Mark,
                Entry::Block(b"BB".to_vec()),
                Entry::Mark,
                Entry::Mark,
            ],
            vec![Entry::Mark, Entry::Mark],
        ] {
            let (volumes, continued) = split_unlabeled(&source, 3).unwrap();
            assert_eq!(join_unlabeled(&volumes, &continued).unwrap(), source);
            assert_eq!(continued.last(), Some(&false));
        }
    }
    #[test]
    fn aws_header_flags_lengths_marks_and_file_positions() {
        let bytes = b"\x03\0\0\0\xa0\0ABC\0\0\x03\0\x40\0\x02\0\0\0\xa0\0DE\0\0\x02\0\x40\0";
        let entries = decode(bytes).unwrap();
        assert_eq!(
            entries,
            vec![
                Entry::Block(b"ABC".to_vec()),
                Entry::Mark,
                Entry::Block(b"DE".to_vec()),
                Entry::Mark
            ]
        );
        assert_eq!(encode(&entries).unwrap(), bytes);
        assert_eq!(file(&entries, 0).unwrap().blocks, vec![b"ABC".to_vec()]);
        assert_eq!(sequence(&entries, 2).unwrap(), 2);
        assert_eq!(file(&entries, 2).unwrap().blocks, vec![b"DE".to_vec()]);
        let mut bad = bytes.to_vec();
        bad[11] = 4;
        assert!(decode(&bad).is_err());
        assert!(decode(&bytes[..bytes.len() - 1]).is_err());
    }
}
