use zeus_cobol::{hlasm, vm::Machine};
fn hex(text: &str) -> Vec<u8> {
    let s = text.split_whitespace().collect::<String>();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
fn card(kind: &str) -> [u8; 80] {
    let mut c = [0x40; 80];
    c[0] = 2;
    c[1..4].copy_from_slice(&zeus_cobol::ebcdic::encode(kind));
    c
}
fn number(c: &mut [u8], at: usize, width: usize, value: u32) {
    c[at..at + width].copy_from_slice(&value.to_be_bytes()[4 - width..]);
}
fn symbol(name: &str, kind: u8, origin: u32, flags: u8, length: u32) -> [u8; 16] {
    let mut s = [0x40; 16];
    let text = zeus_cobol::ebcdic::encode(name);
    s[..text.len()].copy_from_slice(&text);
    s[8] = kind;
    number(&mut s, 9, 3, origin);
    s[12] = flags;
    number(&mut s, 13, 3, length);
    s
}
fn esd(id: u16, items: &[[u8; 16]]) -> [u8; 80] {
    let mut c = card("ESD");
    number(&mut c, 10, 2, (items.len() * 16) as u32);
    number(&mut c, 14, 2, u32::from(id));
    for (i, s) in items.iter().enumerate() {
        c[16 + i * 16..32 + i * 16].copy_from_slice(s);
    }
    c
}
fn text(id: u16, at: u32, data: &[u8]) -> [u8; 80] {
    let mut c = card("TXT");
    number(&mut c, 5, 3, at);
    number(&mut c, 10, 2, data.len() as u32);
    number(&mut c, 14, 2, u32::from(id));
    c[16..16 + data.len()].copy_from_slice(data);
    c
}
fn end(id: u16, at: u32) -> [u8; 80] {
    let mut c = card("END");
    number(&mut c, 5, 3, at);
    number(&mut c, 14, 2, u32::from(id));
    c
}
fn deck(cards: &[[u8; 80]]) -> Vec<u8> {
    cards.concat()
}
fn execute(m: &hlasm::Module) -> (i32, Vec<u8>) {
    let mut mem = vec![0; 4096];
    mem.extend(m.storage_at(4096).unwrap());
    let rc = hlasm::execute(m, &mut mem, 4096, &[]).unwrap();
    (rc, mem)
}
fn add_deck() -> Vec<u8> {
    let code = hex("58201000 58301004 58402000 A74A0007 50403000 41F00009 07FE");
    deck(&[
        esd(1, &[symbol("ASMADD", 0, 0, 6, code.len() as u32)]),
        text(1, 0, &code),
        end(1, 0),
    ])
}

#[test]
fn obj_deck_executes_real_machine_code_and_survives_zeus_serialization() {
    let input = deck(&[
        esd(1, &[symbol("SIMPLE", 0, 0, 6, 6)]),
        text(1, 0, &hex("41F00025 07FE")),
        end(1, 0),
    ]);
    assert!(hlasm::is_module(&input));
    let m = hlasm::load_module(&input).unwrap();
    assert_eq!(m.zeus_hlasm, 11);
    assert_eq!(m.amode, 31);
    assert_eq!(execute(&m).0, 37);
    let restored = hlasm::load_module(&hlasm::module_bytes(&m).unwrap()).unwrap();
    assert_eq!(execute(&restored).0, 37);
}

#[test]
fn cobol_calls_raw_obj_with_original_shared_arguments_and_reclaims_frames() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("ASMADD.obj"), add_deck()).unwrap();
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    let a = machine.getmain(8, 0);
    machine.storage_mut()[a..a + 4].copy_from_slice(&5i32.to_be_bytes());
    assert_eq!(
        machine
            .call_hlasm("ASMADD", &[Some(a), Some(a + 4)])
            .unwrap(),
        Ok(9)
    );
    assert_eq!(&machine.storage()[a + 4..a + 8], 12i32.to_be_bytes());
    let mark = machine.storage().len();
    for _ in 0..20 {
        assert_eq!(
            machine.call_hlasm("ASMADD", &[Some(a), Some(a)]).unwrap(),
            Ok(9)
        );
        assert_eq!(machine.storage().len(), mark);
    }
    let src = dir.path().join("main.cbl");
    std::fs::write(&src,"       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAINPGM.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 A PIC S9(9) COMP VALUE 5.\n       01 B PIC S9(9) COMP VALUE 0.\n       PROCEDURE DIVISION.\n           CALL 'ASMADD' USING A B\n           IF B = 12 AND RETURN-CODE = 9\n             MOVE 0 TO RETURN-CODE\n           ELSE MOVE 99 TO RETURN-CODE END-IF\n           STOP RUN.\n").unwrap();
    let m = zeus_cobol::compile_file(&src, &Default::default()).unwrap();
    assert_eq!(
        Machine::new(vec![dir.path().to_path_buf()])
            .run_main(m)
            .unwrap(),
        0
    );
}

#[test]
fn nonzero_section_origins_labels_compressed_signed_rld_and_overwritten_text_are_resolved() {
    let mut rld = card("RLD");
    number(&mut rld, 10, 2, 16);
    // Two compressed terms for the same field cancel, then a V-type field
    // in the first section refers to the second section's nonzero origin.
    rld[16..32].copy_from_slice(&hex("00010001 0D000108 0E000108 00020001"));
    // Above final pointer pair begins a new full item, so length is 20 bytes.
    number(&mut rld, 10, 2, 20);
    rld[32..36].copy_from_slice(&hex("1C00010C"));
    let input = deck(&[
        esd(
            1,
            &[
                symbol("FIRST", 0, 0x100, 6, 16),
                symbol("SECOND", 0, 0x200, 6, 8),
                symbol("ALT", 1, 0x102, 0x40, 1),
            ],
        ),
        text(1, 0x100, &hex("41F00001 07FE 0000 00000005 00000203")),
        text(1, 0x100, &hex("41F0002B")),
        text(2, 0x200, &hex("07FE000000000000")),
        rld,
        end(1, 0x100),
    ]);
    let m = hlasm::load_module(&input).unwrap();
    assert_eq!(m.entries["ALT"], 2);
    assert_eq!(m.entries["SECOND"], 16);
    let relocated = m.storage_at(4096).unwrap();
    assert_eq!(&relocated[8..12], 5u32.to_be_bytes());
    assert_eq!(&relocated[12..16], 4115u32.to_be_bytes());
    assert_eq!(execute(&m).0, 43);
}

#[test]
fn external_obj_relocations_link_to_source_and_deferred_lengths_are_honored() {
    let mut rld = card("RLD");
    number(&mut rld, 10, 2, 8);
    rld[16..24].copy_from_slice(&hex("00020001 1C000014"));
    let mut last = end(1, 0);
    number(&mut last, 28, 4, 24);
    let input = deck(&[
        esd(
            1,
            &[symbol("MAIN", 0, 0, 6, 0), symbol("SUB", 2, 0, 0x40, 0)],
        ),
        text(
            1,
            0,
            &hex("18CE 5830F014 0DE3 18EC 07FE 0000000000000000 00000000"),
        ),
        rld,
        last,
    ]);
    let caller = hlasm::load_module(&input).unwrap();
    assert!(caller.storage_at(4096).unwrap_err().contains("SUB"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sub.asm");
    std::fs::write(
        &path,
        "SUB CSECT\n         LA 15,31\n         BR 14\n         END\n",
    )
    .unwrap();
    let linked = hlasm::link_modules(&[caller, hlasm::compile_file(&path, &[]).unwrap()]).unwrap();
    assert_eq!(execute(&linked).0, 31);
}

#[test]
fn multiple_obj_modules_link_and_weak_externals_do_not_require_a_definition() {
    let mut rld = card("RLD");
    number(&mut rld, 10, 2, 8);
    rld[16..24].copy_from_slice(&hex("00020001 0C000008"));
    let input = deck(&[
        esd(
            1,
            &[
                symbol("MAIN", 0, 0, 6, 12),
                symbol("ABSENT", 10, 0, 0x40, 0),
            ],
        ),
        text(1, 0, &hex("41F00007 07FE 0000 00000005")),
        rld,
        end(1, 0),
        esd(1, &[symbol("OTHER", 0, 0, 6, 6)]),
        text(1, 0, &hex("41F00009 07FE")),
        end(1, 0),
    ]);
    let m = hlasm::load_module(&input).unwrap();
    assert!(m.entries.contains_key("OTHER"));
    assert_eq!(execute(&m).0, 7);
    assert_eq!(&m.storage_at(4096).unwrap()[8..12], 5u32.to_be_bytes());
}

#[test]
fn malformed_obj_records_report_the_record_and_reject_invalid_layouts() {
    let first = esd(1, &[symbol("BAD", 0, 0, 6, 6)]);
    let mut bad_size = text(1, 0, &hex("07FE"));
    number(&mut bad_size, 10, 2, 57);
    let mut bad_rld = card("RLD");
    number(&mut bad_rld, 10, 2, 8);
    bad_rld[16..24].copy_from_slice(&hex("00010001 0D000000"));
    for cards in [
        vec![first],
        vec![first, bad_size, end(1, 0)],
        vec![first, text(1, 5, &hex("07FE")), end(1, 0)],
        vec![first, bad_rld, end(1, 0)],
        vec![first, text(2, 0, &hex("07FE")), end(1, 0)],
    ] {
        assert!(hlasm::load_module(&deck(&cards)).is_err());
    }
    let mut truncated = add_deck();
    truncated.pop();
    assert!(
        hlasm::load_module(&truncated)
            .unwrap_err()
            .contains("80-byte")
    );
}
