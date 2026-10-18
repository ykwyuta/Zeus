use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use zeus_ims::call_adapter::PcbEncoding;
use zeus_ims::catalog::{GsamDefinition, Pcb};
use zeus_ims::gsam::GsamSession;

#[test]
fn fixed_gsam_gn_gu_rsa_insert_commit_and_rollback() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("imsdb-gsam-{}-{stamp}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let input = dir.join("input.fb");
    let output = dir.join("output.fb");
    fs::write(&input, b"ABCDWXYZ").unwrap();
    let pcb_def = Pcb {
        external_name: None,
        procseq: None,
        indices: Vec::new(),
        name: None,
        kind: "GSAM".into(),
        dbd_name: Some("GSAMDB".into()),
        procopt: Some("LS".into()),
        keylen: None,
        lterm: None,
        modifiable: false,
        sensitive_segments: vec![],
    };
    let mut session = GsamSession::new(
        "GSAMDB",
        &pcb_def,
        GsamDefinition {
            input_dd: "IMSGSIN".into(),
            output_dd: "IMSGSOUT".into(),
            record_len: 4,
        },
        PcbEncoding::Ascii,
    )
    .with_dds(
        [
            ("IMSGSIN".to_string(), input.clone()),
            ("IMSGSOUT".to_string(), output.clone()),
        ]
        .into_iter()
        .collect(),
    );
    let mut pcb = [0u8; 44];
    let mut io = [0u8; 4];
    let mut rsa = [0u8; 8];
    session.call("OPEN", &mut pcb, &mut [], None).unwrap();
    assert_eq!(&pcb[10..12], b"  ");
    session
        .call("GN", &mut pcb, &mut io, Some(&mut rsa))
        .unwrap();
    assert_eq!(&io, b"ABCD");
    assert_eq!(rsa, 1u64.to_be_bytes());
    session
        .call("GN", &mut pcb, &mut io, Some(&mut rsa))
        .unwrap();
    assert_eq!(&io, b"WXYZ");
    assert_eq!(rsa, 2u64.to_be_bytes());
    session.call("GN", &mut pcb, &mut io, None).unwrap();
    assert_eq!(&pcb[10..12], b"GB");
    session.call("GN", &mut pcb, &mut io, None).unwrap();
    assert_eq!(&io, b"ABCD");
    rsa = 2u64.to_be_bytes();
    session
        .call("GU", &mut pcb, &mut io, Some(&mut rsa))
        .unwrap();
    assert_eq!(&io, b"WXYZ");
    session.call("GU", &mut pcb, &mut io, None).unwrap();
    assert_eq!(&pcb[10..12], b"AH");
    io = *b"IJKL";
    session
        .call("ISRT", &mut pcb, &mut io, Some(&mut rsa))
        .unwrap();
    assert!(!output.exists());
    session.rollback();
    assert!(!output.exists());
    session
        .call("ISRT", &mut pcb, &mut io, Some(&mut rsa))
        .unwrap();
    session.commit().unwrap();
    assert_eq!(fs::read(&output).unwrap(), b"IJKL");
    session.call("CLSE", &mut pcb, &mut [], None).unwrap();
    assert_eq!(&pcb[10..12], b"  ");
    fs::remove_dir_all(dir).unwrap();
}
