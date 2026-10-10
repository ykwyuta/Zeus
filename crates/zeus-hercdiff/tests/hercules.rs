//! End-to-end check of the oracle pipeline on Hercules with known results.
//! Skipped (passes trivially) when the `hercules` binary is not installed.

use zeus_hercdiff::cases::{Case, Op};
use zeus_hercdiff::hercules::Hercules;
use zeus_hercdiff::image;

#[test]
fn known_results() {
    let herc = Hercules {
        binary: "hercules".into(),
        run_seconds: 1,
    };
    if !herc.available() {
        eprintln!("hercules not installed; skipping");
        return;
    }
    let case = |op, op1: &[u8], op2: &[u8]| Case {
        op,
        op1: op1.to_vec(),
        op2: op2.to_vec(),
        srp: None,
    };
    let cases = [
        // 123 + 456 = 579, result positive -> CC2
        case(Op::Ap, &[0x12, 0x3C], &[0x45, 0x6C]),
        // invalid digit -> data exception (code 7)
        case(Op::Ap, &[0x1A, 0x3C], &[0x45, 0x6C]),
        // 999 + 1 overflows a 2-byte field -> CC3 (overflow masked)
        case(Op::Ap, &[0x99, 0x9C], &[0x1C]),
        // ZAP -5 into 3 bytes -> 00005D, CC1
        case(Op::Zap, &[0xFF, 0xFF, 0xFF], &[0x5D]),
        // MP 3 bytes (00012C) * 2C -> 00024C, CC unchanged by MP
        case(Op::Mp, &[0x00, 0x01, 0x2C], &[0x2C]),
    ];
    let dir = tempfile::tempdir().unwrap();
    let (dump, version) = herc
        .run(
            dir.path(),
            &image::build(&cases, false),
            image::dump_end(cases.len()),
        )
        .unwrap();
    assert!(version.starts_with("Hercules Version"), "{version}");
    let obs = image::decode(&cases, &dump).unwrap();

    assert_eq!(obs[0].op1, [0x57, 0x9C]);
    assert_eq!(obs[0].cc, Some(2));
    assert_eq!(obs[1].cc, None);
    assert_eq!(obs[1].interruption, Some(7));
    assert_eq!(obs[2].cc, Some(3));
    assert_eq!(obs[2].op1, [0x00, 0x0C]);
    assert_eq!(obs[3].op1, [0x00, 0x00, 0x5D]);
    assert_eq!(obs[3].cc, Some(1));
    assert_eq!(obs[4].op1, [0x00, 0x02, 0x4C]);
    assert_eq!(obs[4].cc, Some(0));
}
