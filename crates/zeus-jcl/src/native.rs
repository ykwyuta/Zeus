//! Application subroutines written in assembler, implemented natively.
//!
//! Zeus does not run S/390 machine code yet, so assembler routines that
//! COBOL programs CALL are reimplemented here from their published
//! sources and registered with every COBOL step. A program of the same
//! name found in STEPLIB/JOBLIB takes precedence.
//!
//! * `COBDATFT` (AWS CardDemo, Apache-2.0): converts a date between
//!   `YYYYMMDD` and `YYYY-MM-DD`. Parameter: input type (1 byte, `1` =
//!   `YYYYMMDD`, `2` = `YYYY-MM-DD`), input date (20), output type (1),
//!   output date (20), error message (38).
//! * `MVSWAIT` (AWS CardDemo): waits for the fullword interval given in
//!   hundredths of a second (STIMER WAIT,BINTVL=).

use zeus_cobol::ebcdic;
use zeus_cobol::vm::{Machine, NativeFn};

pub const ROUTINES: &[(&str, NativeFn)] = &[("COBDATFT", cobdatft), ("MVSWAIT", mvswait)];

/// Registers the native routines with a machine.
pub fn register(m: &mut Machine) {
    for (name, f) in ROUTINES {
        m.add_native(name, *f);
    }
    // the MQI (IBM MQ through its C client library)
    zeus_mq::register(m);
}

fn arg(args: &[Option<usize>], i: usize, len: usize, mem: &[u8]) -> Result<usize, String> {
    match args.get(i).copied().flatten() {
        Some(a) if a + len <= mem.len() => Ok(a),
        _ => Err("parameter missing or out of storage".into()),
    }
}

fn cobdatft(mem: &mut [u8], args: &[Option<usize>]) -> Result<i32, String> {
    let a = arg(args, 0, 80, mem)?;
    let (in_type, input, out_type, output, msg) = (a, a + 1, a + 21, a + 22, a + 42);
    let c = |ch: char| ebcdic::encode(&ch.to_string())[0];
    let ok = match mem[in_type] {
        t if t == c('1') && mem[input + 4] != c('-') && mem[out_type] != c('2') => {
            // YYYYMMDD -> YYYY-MM-DD
            mem.copy_within(input..input + 4, output);
            mem[output + 4] = c('-');
            mem.copy_within(input + 4..input + 6, output + 5);
            mem[output + 7] = c('-');
            mem.copy_within(input + 6..input + 8, output + 8);
            true
        }
        t if t == c('2') && mem[out_type] != c('1') => {
            // YYYY-MM-DD -> YYYYMMDD
            mem.copy_within(input..input + 4, output);
            mem.copy_within(input + 5..input + 7, output + 4);
            mem.copy_within(input + 8..input + 10, output + 6);
            true
        }
        _ => false,
    };
    if !ok {
        let text = ebcdic::encode("INVALID INPUT");
        mem[msg..msg + text.len()].copy_from_slice(&text);
    }
    Ok(0)
}

fn mvswait(mem: &mut [u8], args: &[Option<usize>]) -> Result<i32, String> {
    let a = arg(args, 0, 4, mem)?;
    let v = u32::from_be_bytes([mem[a], mem[a + 1], mem[a + 2], mem[a + 3]]);
    std::thread::sleep(std::time::Duration::from_millis(u64::from(v) * 10));
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(t: &str, input: &str, o: &str) -> Vec<u8> {
        let mut r = ebcdic::encode(&format!("{t}{input:<20}{o}"));
        r.resize(80, ebcdic::SPACE);
        r
    }

    #[test]
    fn cobdatft_formats() {
        let mut m = rec("2", "2023-04-05", "2");
        cobdatft(&mut m, &[Some(0)]).unwrap();
        assert_eq!(&m[22..30], &ebcdic::encode("20230405")[..]);
        let mut m = rec("1", "20230405", "1");
        cobdatft(&mut m, &[Some(0)]).unwrap();
        assert_eq!(&m[22..32], &ebcdic::encode("2023-04-05")[..]);
        let mut m = rec("1", "2023-04-05", "1");
        cobdatft(&mut m, &[Some(0)]).unwrap();
        assert_eq!(&m[42..55], &ebcdic::encode("INVALID INPUT")[..]);
    }
}
