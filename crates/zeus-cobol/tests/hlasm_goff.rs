use zeus_cobol::hlasm;
fn word(b: &mut [u8], at: usize, n: u32) {
    b[at..at + 4].copy_from_slice(&n.to_be_bytes());
}
fn half(b: &mut [u8], at: usize, n: usize) {
    b[at..at + 2].copy_from_slice(&(n as u16).to_be_bytes());
}
fn header() -> Vec<u8> {
    let mut b = vec![0; 60];
    b[..3].copy_from_slice(&[3, 0xf0, 0]);
    word(&mut b, 48, 1);
    b
}
fn esd(id: u32, kind: u8, parent: u32, name: &str, origin: u32, length: u32) -> Vec<u8> {
    let text = zeus_cobol::ebcdic::encode(name);
    let mut b = vec![0; 72 + text.len()];
    b[0] = 3;
    b[3] = kind;
    word(&mut b, 4, id);
    word(&mut b, 8, parent);
    word(&mut b, 16, origin);
    word(&mut b, 24, length);
    b[40] = 1;
    b[60] = 2;
    b[61] = 3;
    b[63] = 2;
    b[66] = 3;
    half(&mut b, 70, text.len());
    b[72..].copy_from_slice(&text);
    b
}
fn text(id: u32, at: u32, bytes: &[u8]) -> Vec<u8> {
    let mut b = vec![0; 24 + bytes.len()];
    b[0] = 3;
    b[1] = 0x10;
    word(&mut b, 4, id);
    word(&mut b, 12, at);
    half(&mut b, 22, bytes.len());
    b[24..].copy_from_slice(bytes);
    b
}
fn end(id: u32, at: u32, count: u32) -> Vec<u8> {
    let mut b = vec![0; 26];
    b[0] = 3;
    b[1] = 0x40;
    b[3] = 1;
    b[4] = 2;
    word(&mut b, 8, count);
    word(&mut b, 12, id);
    word(&mut b, 20, at);
    b
}
fn cards(records: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for b in records {
        let n = b.len().min(80);
        let mut card = [0; 80];
        card[..n].copy_from_slice(&b[..n]);
        let mut rest = &b[n..];
        if !rest.is_empty() {
            card[1] |= 1;
        }
        out.extend(card);
        while !rest.is_empty() {
            let n = rest.len().min(77);
            let mut c = [0; 80];
            c[..3].copy_from_slice(&[3, (b[1] & 0xf0) | 2 | u8::from(rest.len() > 77), 0]);
            c[3..3 + n].copy_from_slice(&rest[..n]);
            out.extend(c);
            rest = &rest[n..];
        }
    }
    out
}
fn rdw(records: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for b in records {
        out.extend(((b.len() + 4) as u16).to_be_bytes());
        out.extend([0, 0]);
        out.extend(b);
    }
    out
}
fn execute(m: &hlasm::Module) -> (i32, Vec<u8>) {
    let mut mem = vec![0; 4096];
    mem.extend(m.storage_at(4096).unwrap());
    let rc = hlasm::execute(m, &mut mem, 4096, &[]).unwrap();
    (rc, mem)
}
fn simple() -> Vec<Vec<u8>> {
    vec![
        header(),
        esd(1, 0, 0, "MAIN", 0, 0),
        esd(2, 1, 1, "B_TEXT", 0, 6),
        text(2, 0, &[0x41, 0xf0, 0, 37, 0x07, 0xfe]),
        end(2, 0, 5),
    ]
}

#[test]
fn fixed_and_rdw_goff_execute_identically_and_retain_class_alignment() {
    let records = simple();
    for input in [cards(&records), rdw(&records)] {
        assert!(hlasm::is_module(&input));
        let m = hlasm::load_module(&input).unwrap();
        assert_eq!(execute(&m).0, 37);
        assert_eq!(m.bindings.sections[0].class.as_deref(), Some("B_TEXT"));
        assert_eq!(m.alignment, 8);
        let m = hlasm::load_module(&hlasm::module_bytes(&m).unwrap()).unwrap();
        assert_eq!(execute(&m).0, 37);
    }
}

#[test]
fn long_names_and_text_cross_multiple_continuations_without_counting_padding() {
    let long = "A".repeat(50);
    let mut records = simple();
    records[1] = esd(1, 0, 0, &long, 0, 0);
    records[2] = esd(2, 1, 1, "B_TEXT", 0, 240);
    let mut bytes = vec![0x42; 240];
    bytes[..6].copy_from_slice(&[0x41, 0xf0, 0, 17, 7, 0xfe]);
    records[3] = text(2, 0, &bytes);
    let m = hlasm::load_module(&cards(&records)).unwrap();
    assert_eq!(m.name, long);
    assert_eq!(execute(&m).0, 17);
    assert_eq!(m.storage, bytes);
}

#[test]
fn deferred_lengths_and_repeated_text_are_expanded_with_fill_bytes() {
    let mut records = simple();
    records[2] = esd(2, 1, 1, "B_TEXT", 0, u32::MAX);
    records[2][41] = 0x80;
    records[2][42] = 0xa5;
    let mut repeated = text(2, 8, &[0, 3, 0, 2, b'A', b'B']);
    word(&mut repeated, 16, 6);
    half(&mut repeated, 20, 1);
    records.insert(4, repeated);
    let mut len = vec![0; 20];
    len[0] = 3;
    len[1] = 0x30;
    half(&mut len, 6, 12);
    word(&mut len, 8, 2);
    word(&mut len, 16, 24);
    records.insert(5, len);
    records[6] = end(2, 0, 7);
    let m = hlasm::load_module(&cards(&records)).unwrap();
    let (_, mem) = execute(&m);
    assert_eq!(&mem[4096 + 8..4096 + 14], b"ABABAB");
    assert!(mem[4096 + 14..4096 + 24].iter().all(|b| *b == 0xa5));
}

#[test]
fn assembler_nonzero_origins_named_entries_and_noload_data_are_handled() {
    let mut records = simple();
    records[2] = esd(2, 1, 1, "B_TEXT", 0x100, 6);
    records[3] = text(2, 0x100, &[0x41, 0xf0, 0, 13, 7, 0xfe]);
    let name = "AN_ENTRY_WITH_A_CONTINUED_NAME";
    records.insert(4, esd(3, 2, 2, name, 0x100, 0));
    let mut debug = esd(4, 1, 1, "B_DEBUG", 0, 12);
    debug[62] = 0x20;
    debug[65] = 0x80;
    records.insert(5, debug);
    let mut payload = text(4, 0, b"debug");
    payload[3] = 2;
    records.insert(6, payload);
    let nominal = zeus_cobol::ebcdic::encode(name);
    let mut last = vec![0; 26 + nominal.len()];
    last[0] = 3;
    last[1] = 0x40;
    last[3] = 2;
    last[4] = 2;
    word(&mut last, 8, 8);
    half(&mut last, 24, nominal.len());
    last[26..].copy_from_slice(&nominal);
    records[7] = last;
    let m = hlasm::load_module(&cards(&records)).unwrap();
    assert_eq!(execute(&m).0, 13);
    assert_eq!(m.entries[name], 0);
    assert_eq!(m.storage.len(), 6);
}

#[test]
fn invalid_continuations_record_counts_ids_and_ranges_are_rejected() {
    let mut cases = Vec::new();
    let mut r = simple();
    word(&mut r[4], 8, 6);
    cases.push(cards(&r));
    let mut r = simple();
    word(&mut r[2], 4, 7);
    cases.push(cards(&r));
    let mut r = simple();
    word(&mut r[3], 12, 5);
    cases.push(cards(&r));
    let mut r = simple();
    r[2][1] |= 2;
    cases.push(cards(&r));
    let mut r = simple();
    r[1] = esd(1, 0, 0, &"A".repeat(100), 0, 0);
    let mut b = cards(&r);
    b[80 + 160 + 1] = 0x12;
    cases.push(b);
    let mut b = cards(&simple());
    b[80 + 1] |= 1;
    cases.push(b);
    let mut b = rdw(&simple());
    b[2] = 1;
    cases.push(b);
    for b in cases {
        assert!(hlasm::load_module(&b).is_err());
    }
}

fn rld(items: &[u8]) -> Vec<u8> {
    let mut b = vec![0; 6 + items.len()];
    b[0] = 3;
    b[1] = 0x20;
    half(&mut b, 4, items.len());
    b[6..].copy_from_slice(items);
    b
}
fn fix(reference: u32, position: u32, offset: u32, flags: [u8; 6]) -> Vec<u8> {
    let mut b = vec![0; 20];
    b[..6].copy_from_slice(&flags);
    word(&mut b, 8, reference);
    word(&mut b, 12, position);
    word(&mut b, 16, offset);
    b
}
#[test]
fn compressed_signed_relocations_keep_original_contents_and_nonzero_origins() {
    let mut r = simple();
    r[2] = esd(2, 1, 1, "B_TEXT", 0x100, 16);
    let mut bytes = vec![0; 16];
    bytes[..6].copy_from_slice(&[0x41, 0xf0, 0, 5, 7, 0xfe]);
    word(&mut bytes, 8, 0x10a);
    word(&mut bytes, 12, 7);
    r[3] = text(2, 0x100, &bytes);
    let mut data = fix(2, 2, 0x108, [0, 0, 0, 0, 4, 0]);
    // New field offset, reusing R/P IDs; adding and then subtracting the
    // same element base must leave the original field value 7.
    data.extend([0xc0, 0, 0, 0, 4, 0, 0, 0]);
    data.extend(0x10cu32.to_be_bytes());
    data.extend([0xe0, 0, 2, 0, 4, 0, 0, 0]);
    r.insert(4, rld(&data));
    r[5] = end(2, 0x100, 6);
    let m = hlasm::load_module(&cards(&r)).unwrap();
    let (rc, mem) = execute(&m);
    assert_eq!(rc, 5);
    assert_eq!(&mem[4096 + 8..4096 + 12], 4106u32.to_be_bytes());
    assert_eq!(&mem[4096 + 12..4096 + 16], 7u32.to_be_bytes());
}

#[test]
fn goff_external_relocations_link_to_source_and_weak_references_stay_zero() {
    let mut r = simple();
    r[2] = esd(2, 1, 1, "B_TEXT", 0, 24);
    r.insert(3, esd(3, 4, 1, "SUB", 0, 0));
    let bytes = [
        0x18, 0xce, 0x58, 0x30, 0xf0, 0x14, 0x0d, 0xe3, 0x18, 0xec, 0x07, 0xfe, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ];
    r[4] = text(2, 0, &bytes);
    r.insert(5, rld(&fix(3, 2, 20, [0, 0, 0, 0, 4, 0])));
    r[6] = end(2, 0, 7);
    let m = hlasm::load_module(&cards(&r)).unwrap();
    assert!(m.storage_at(4096).unwrap_err().contains("SUB"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sub.asm");
    std::fs::write(&path, "SUB CSECT\n LA 15,39\n BR 14\n END\n").unwrap();
    let linked = hlasm::link_modules(&[m, hlasm::compile_file(&path, &[]).unwrap()]).unwrap();
    assert_eq!(execute(&linked).0, 39);
    r[3][64] = 1; // Weak reference must not cause a library read.
    let m = hlasm::load_module(&rdw(&r)).unwrap();
    assert_eq!(&m.storage_at(4096).unwrap()[20..24], [0, 0, 0, 0]);
}

#[test]
fn relocation_store_flag_ignores_old_contents_and_label_references_use_label_addresses() {
    let mut r = simple();
    r[2] = esd(2, 1, 1, "B_TEXT", 0, 16);
    let mut bytes = vec![0; 16];
    bytes[..6].copy_from_slice(&[0x41, 0xf0, 0, 3, 7, 0xfe]);
    word(&mut bytes, 12, 0x12345678);
    r[3] = text(2, 0, &bytes);
    r.insert(4, esd(3, 2, 2, "DATA", 8, 0));
    r.insert(5, rld(&fix(3, 2, 12, [0, 0, 1, 0, 4, 0])));
    r[6] = end(2, 0, 7);
    let m = hlasm::load_module(&cards(&r)).unwrap();
    let (_, mem) = execute(&m);
    assert_eq!(&mem[4096 + 12..4096 + 16], 4104u32.to_be_bytes());
    // Missing earlier pointer values and fields extending past the element
    // are errors, including when wrapped in syntactically valid records.
    r[5] = rld(&[0xe0, 0, 0, 0, 4, 0, 0, 0]);
    assert!(
        hlasm::load_module(&cards(&r))
            .unwrap_err()
            .contains("preceding")
    );
    r[5] = rld(&fix(3, 2, 14, [0, 0, 0, 0, 4, 0]));
    assert!(
        hlasm::load_module(&cards(&r))
            .unwrap_err()
            .contains("exceeds")
    );
}

#[test]
fn standalone_cli_loads_goff_and_links_a_serialized_machine_image() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("main.goff");
    let output = dir.path().join("linked.zo");
    std::fs::write(&input, cards(&simple())).unwrap();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .arg("run")
        .arg(&input)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(37));
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .arg("link")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let module = hlasm::load_module(&std::fs::read(output).unwrap()).unwrap();
    assert_eq!(execute(&module).0, 37);
}

#[test]
fn mode_sensitive_internal_addresses_get_their_architectural_linkage_tags() {
    for (amode, width, expected) in [(1, 4, 4096u64), (2, 4, 0x80001000), (4, 8, 4097)] {
        let mut r = simple();
        r[2] = esd(2, 1, 1, "B_TEXT", 0x100, 16);
        r[2][60] = amode;
        let mut bytes = vec![0; 16];
        bytes[..6].copy_from_slice(&[0x41, 0xf0, 0, 0, 7, 0xfe]);
        bytes[8 + width as usize - 2] = 1;
        r[3] = text(2, 0x100, &bytes);
        r.insert(4, rld(&fix(2, 2, 0x108, [1, 0, 0, 0, width, 0])));
        r[5] = end(2, 0x100, 6);
        r[5][4] = amode;
        let m = hlasm::load_module(&cards(&r)).unwrap();
        assert!(m.relocations.is_empty());
        assert_eq!(m.object_relocations.len(), 1);
        let (_, mem) = execute(&m);
        assert_eq!(
            &mem[4096 + 8..4096 + 8 + width as usize],
            &expected.to_be_bytes()[8 - width as usize..]
        );
        let m = hlasm::load_module(&hlasm::module_bytes(&m).unwrap()).unwrap();
        assert_eq!(execute(&m).0, 0);
    }
}

#[test]
fn external_mode_sensitive_v_constants_resolve_callee_mode_during_linking() {
    let mut r = simple();
    r[2] = esd(2, 1, 1, "B_TEXT", 0, 24);
    r.insert(3, esd(3, 4, 1, "SUB", 0, 0));
    // BASSM, rather than BASR, consumes the tagged loaded entry pointer.
    let bytes = [
        0x18, 0xce, 0x58, 0x30, 0xf0, 0x14, 0x0c, 0xe3, 0x18, 0xec, 0x07, 0xfe, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ];
    r[4] = text(2, 0, &bytes);
    r.insert(5, rld(&fix(3, 2, 20, [1, 0, 1, 0, 4, 0])));
    r[6] = end(2, 0, 7);
    for (mode, tag) in [(24, 0u32), (31, 0x80000000), (64, 1)] {
        let caller = hlasm::load_module(&cards(&r)).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub.asm");
        std::fs::write(
            &path,
            format!("SUB CSECT\n AMODE {mode}\n LGHI 15,47\n BSM 0,14\n END\n"),
        )
        .unwrap();
        let linked =
            hlasm::link_modules(&[caller, hlasm::compile_file(&path, &[]).unwrap()]).unwrap();
        assert_eq!(execute(&linked).0, 47, "AMODE {mode}");
        let pointer =
            u32::from_be_bytes(linked.storage_at(4096).unwrap()[20..24].try_into().unwrap());
        assert_eq!(pointer, (4096 + linked.entries["SUB"]) | tag);
    }
    r[3][64] = 1;
    let m = hlasm::load_module(&cards(&r)).unwrap();
    assert_eq!(&m.storage_at(4096).unwrap()[20..24], [0, 0, 0, 0]);
}

#[test]
fn object_relocation_attributes_validate_versions_bounds_overlaps_and_modes() {
    let mut r = simple();
    r[2] = esd(2, 1, 1, "B_TEXT", 0, 16);
    r[3] = text(2, 0, &[0x41, 0xf0, 0, 0, 7, 0xfe]);
    r.insert(4, rld(&fix(2, 2, 8, [1, 0, 1, 0, 4, 0])));
    r[5] = end(2, 0, 6);
    let m = hlasm::load_module(&cards(&r)).unwrap();
    for (version, offset, width, mode) in [
        (10, 8, 4, 31),
        (11, 14, 4, 31),
        (11, 8, 1, 31),
        (11, 8, 4, 7),
    ] {
        let mut broken = m.clone();
        broken.zeus_hlasm = version;
        broken.object_relocations[0].offset = offset;
        broken.object_relocations[0].width = width;
        broken.object_relocations[0].addressing_mode = hlasm::ObjectAddressMode::Known(mode);
        assert!(hlasm::load_module(&hlasm::module_bytes(&broken).unwrap()).is_err());
    }
    let mut broken = m.clone();
    broken
        .object_relocations
        .push(broken.object_relocations[0].clone());
    assert!(hlasm::load_module(&hlasm::module_bytes(&broken).unwrap()).is_err());
}

#[test]
fn multiple_classes_and_private_or_long_section_names_do_not_conflict() {
    for name in [" ".to_string(), "SECTION".repeat(20)] {
        let mut data = esd(2, 1, 1, "b_data", 0, 3);
        data[63] = 1;
        data[66] = 0;
        let mut code = esd(3, 1, 1, "b_text", 0, 7);
        code[66] = 0;
        // A byte-aligned element begins at odd image offset 3, but its first
        // instruction is halfword-aligned at element offset 1.
        let r = vec![
            header(),
            esd(1, 0, 0, &name, 0, 0),
            data,
            code,
            text(2, 0, &[1, 2, 3]),
            text(3, 0, &[0, 0x41, 0xf0, 0, 19, 7, 0xfe]),
            end(3, 1, 7),
        ];
        let m = hlasm::load_module(&cards(&r)).unwrap();
        assert_eq!(m.entry, 4);
        assert_eq!(m.machine_code[0], 3..10);
        assert_eq!(execute(&m).0, 19);
        assert_eq!(
            m.bindings
                .sections
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>(),
            ["b_data", "b_text"]
        );
    }
}

#[test]
fn object_expression_references_pull_libraries_without_redundant_external_metadata() {
    let mut r = simple();
    r[2] = esd(2, 1, 1, "B_TEXT", 0, 24);
    r.insert(3, esd(3, 4, 1, "SUB", 0, 0));
    r[4] = text(
        2,
        0,
        &[
            0x18, 0xce, 0x58, 0x30, 0xf0, 0x14, 0x0c, 0xe3, 0x18, 0xec, 0x07, 0xfe, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0,
        ],
    );
    r.insert(5, rld(&fix(3, 2, 20, [1, 0, 1, 0, 4, 0])));
    r[6] = end(2, 0, 7);
    let mut caller = hlasm::load_module(&cards(&r)).unwrap();
    caller.externals.clear();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sub.asm");
    std::fs::write(
        &path,
        "SUB CSECT\n AMODE 64\n LGHI 15,53\n BSM 0,14\n END\n",
    )
    .unwrap();
    hlasm::save_module(
        &hlasm::compile_file(&path, &[]).unwrap(),
        &dir.path().join("SUB.zo"),
    )
    .unwrap();
    let linked = hlasm::link_with_libraries(vec![caller], &[dir.path().to_path_buf()]).unwrap();
    assert_eq!(execute(&linked).0, 53);
}

#[test]
fn class_names_differing_only_by_case_remain_distinct() {
    let records = vec![
        header(),
        esd(1, 0, 0, "MAIN", 0, 0),
        esd(2, 1, 1, "Case", 0, 6),
        esd(3, 1, 1, "case", 0, 6),
        text(2, 0, &[0x41, 0xf0, 0, 37, 7, 0xfe]),
        text(3, 0, &[0x41, 0xf0, 0, 19, 7, 0xfe]),
        end(3, 0, 7),
    ];
    let module = hlasm::load_module(&cards(&records)).unwrap();
    assert_eq!(
        module
            .bindings
            .sections
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["Case", "case"]
    );
    assert_eq!(module.entry, 8);
    assert_eq!(execute(&module).0, 19);
    let output = hlasm::goff_bytes(&module).unwrap();
    let restored = hlasm::load_module(&output).unwrap();
    assert_eq!(restored.storage, module.storage);
    assert_eq!(restored.machine_code, module.machine_code);
    assert_eq!(
        restored
            .bindings
            .sections
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["Case", "case"]
    );
    assert_eq!(execute(&restored).0, 19);
    let mut nonzero_gap = module.clone();
    nonzero_gap.storage[6] = 0xAA;
    assert!(
        hlasm::goff_bytes(&nonzero_gap)
            .unwrap_err()
            .contains("image gaps")
    );
    let mut reordered = module.clone();
    for (section, name) in reordered
        .bindings
        .sections
        .iter_mut()
        .zip(["zClass", "aClass"])
    {
        section.name = name.into();
        section.class = Some(name.into());
    }
    assert!(
        hlasm::goff_bytes(&reordered)
            .unwrap_err()
            .contains("class order")
    );
}

#[test]
fn rdw_length_starting_with_goff_prefix_is_not_mistaken_for_fixed_records() {
    let mut r = simple();
    r[0].resize(1004, 0); // RDW length 0x03F0.
    let input = rdw(&r);
    assert_eq!(&input[..4], &[3, 0xf0, 0, 0]);
    assert_eq!(execute(&hlasm::load_module(&input).unwrap()).0, 37);
}

#[test]
fn zero_length_goff_elements_remain_linkable_without_executable_padding() {
    let mut last = end(0, 0, 4);
    last[3] = 0;
    last[4] = 0;
    let r = vec![
        header(),
        esd(1, 0, 0, "EMPTY", 0, 0),
        esd(2, 1, 1, "B_TEXT", 0, 0),
        last,
    ];
    let m = hlasm::load_module(&cards(&r)).unwrap();
    assert!(m.machine_code.is_empty());
    assert_eq!(m.bindings.sections[0].length, 0);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.asm");
    std::fs::write(&path, "MAIN CSECT\n LA 15,7\n BR 14\n END\n").unwrap();
    let linked = hlasm::link_modules(&[hlasm::compile_file(&path, &[]).unwrap(), m]).unwrap();
    assert!(linked.entries.contains_key("EMPTY"));
    assert_eq!(execute(&linked).0, 7);
}

#[test]
fn element_and_label_lengths_relocate_without_origin_or_load_base_dependence() {
    let mut records = simple();
    records[2] = esd(2, 1, 1, "B_TEXT", 0x100, 48);
    let mut image = vec![0; 48];
    image[..6].copy_from_slice(&[0x41, 0xf0, 0, 37, 7, 0xfe]);
    word(&mut image, 8, 7);
    word(&mut image, 12, 100);
    word(&mut image, 16, 999);
    image[24..32].copy_from_slice(&[0xaa; 8]);
    records[3] = text(2, 0x100, &image);
    records.insert(4, esd(3, 2, 2, "LABEL", 0x108, 0));
    let mut fields = fix(2, 2, 0x108, [0, 0x21, 0, 0, 4, 0]);
    fields.extend(fix(2, 2, 0x10c, [0, 0x21, 2, 0, 4, 0]));
    fields.extend(fix(3, 2, 0x110, [0, 0x20, 1, 0, 4, 0]));
    fields.extend(fix(2, 2, 0x118, [0, 0x21, 1, 0, 8, 0]));
    // Reuse the R/P/offset values and subtract the same length again.
    fields.extend([0xe0, 0x21, 2, 0, 8, 0, 0, 0]);
    records.insert(5, rld(&fields));
    records[6] = end(2, 0x100, 7);
    let module = hlasm::load_module(&cards(&records)).unwrap();
    for base in [0, 4096, 0x200000] {
        let image = module.storage_at(base).unwrap();
        assert_eq!(&image[8..12], 55u32.to_be_bytes());
        assert_eq!(&image[12..16], 52u32.to_be_bytes());
        assert_eq!(&image[16..20], [0; 4]);
        assert_eq!(&image[24..32], [0; 8]);
    }
    assert_eq!(execute(&module).0, 37);
    let module = hlasm::load_module(&hlasm::module_bytes(&module).unwrap()).unwrap();
    let mut first = hlasm::load_module(&cards(&simple())).unwrap();
    first.name = "FIRST".into();
    first.entries.clear();
    first.entries.insert("FIRST".into(), 0);
    let linked = hlasm::link_modules(&[first, module]).unwrap();
    let at = linked.entries["LABEL"] as usize - 8;
    let image = linked.storage_at(4096).unwrap();
    assert_eq!(&image[at + 8..at + 12], 55u32.to_be_bytes());
}

#[test]
fn deferred_element_lengths_and_invalid_length_referents_are_checked() {
    let mut records = simple();
    records[2] = esd(2, 1, 1, "B_TEXT", 0, u32::MAX);
    let mut image = vec![0; 16];
    image[..6].copy_from_slice(&[0x41, 0xf0, 0, 37, 7, 0xfe]);
    records[3] = text(2, 0, &image);
    let mut length = vec![0; 20];
    length[..3].copy_from_slice(&[3, 0x30, 0]);
    half(&mut length, 6, 12);
    word(&mut length, 8, 2);
    word(&mut length, 16, 16);
    records.insert(4, length);
    records.insert(5, rld(&fix(2, 2, 8, [0, 0x21, 1, 0, 4, 0])));
    records[6] = end(2, 0, 7);
    let module = hlasm::load_module(&rdw(&records)).unwrap();
    assert_eq!(
        &module.storage_at(4096).unwrap()[8..12],
        16u32.to_be_bytes()
    );
    for (flags, message) in [
        ([1, 0x21, 1, 0, 4, 0], "sensitivity"),
        ([0, 0x20, 1, 0, 4, 0], "referent type"),
        ([0, 0x22, 1, 0, 4, 0], "class-length"),
    ] {
        records[5] = rld(&fix(2, 2, 8, flags));
        assert!(
            hlasm::load_module(&cards(&records))
                .unwrap_err()
                .contains(message)
        );
    }
}

#[test]
fn length_store_replaces_prior_address_expression_and_mode_attributes() {
    let mut records = simple();
    records[2] = esd(2, 1, 1, "B_TEXT", 0, 16);
    let mut image = vec![0; 16];
    image[..6].copy_from_slice(&[0x41, 0xf0, 0, 37, 7, 0xfe]);
    records[3] = text(2, 0, &image);
    let mut fields = fix(2, 2, 8, [1, 0, 1, 0, 4, 0]);
    fields.extend(fix(2, 2, 8, [0, 0x21, 1, 0, 4, 0]));
    records.insert(4, rld(&fields));
    records[5] = end(2, 0, 6);
    let module = hlasm::load_module(&cards(&records)).unwrap();
    assert!(module.object_relocations.is_empty());
    assert_eq!(
        &module.storage_at(4096).unwrap()[8..12],
        16u32.to_be_bytes()
    );
}

fn relative_caller(width: u8, constant: i32) -> Vec<Vec<u8>> {
    let mut image = if width == 2 {
        let mut b = vec![0xa7, 0xf4];
        b.extend((constant as i16).to_be_bytes());
        b
    } else {
        let mut b = vec![0xc0, 0xf4];
        b.extend(constant.to_be_bytes());
        b
    };
    image.extend([7, 0xfe]);
    vec![
        header(),
        esd(1, 0, 0, "MAIN", 0, 0),
        esd(2, 1, 1, "B_TEXT", 0x100, image.len() as u32),
        esd(3, 4, 1, "SUB", 0, 0),
        text(2, 0x100, &image),
        rld(&fix(3, 2, 0x102, [0, 0x60, 0, 0, width, 0])),
        end(2, 0x100, 7),
    ]
}
fn relative_target() -> hlasm::Module {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sub.asm");
    std::fs::write(&path, "SUB CSECT\n LA 15,67\n BR 14\n END\n").unwrap();
    hlasm::compile_file(&path, &[]).unwrap()
}

#[test]
fn external_relative_immediates_link_forward_and_backward_and_survive_rebasing() {
    for width in [2, 4] {
        let caller = hlasm::load_module(&cards(&relative_caller(width, 0))).unwrap();
        assert_eq!(caller.zeus_hlasm, 12);
        assert!(caller.object_relocations[0].relative_immediate);
        let caller = hlasm::load_module(&hlasm::module_bytes(&caller).unwrap()).unwrap();
        for reverse in [false, true] {
            let modules = if reverse {
                vec![relative_target(), caller.clone()]
            } else {
                vec![caller.clone(), relative_target()]
            };
            let mut linked = hlasm::link_modules(&modules).unwrap();
            linked.entry = linked.entries["MAIN"];
            for base in [0, 4096, 0x200000] {
                let image = linked.storage_at(base).unwrap();
                let field = linked.entries["MAIN"] as usize + 2;
                let expected =
                    (i64::from(linked.entries["SUB"]) - i64::from(linked.entries["MAIN"])) / 2;
                assert_eq!(
                    &image[field..field + width as usize],
                    &expected.to_be_bytes()[8 - width as usize..]
                );
            }
            assert_eq!(execute(&linked).0, 67);
            let mut prefix = relative_target();
            prefix.name = "PREFIX".into();
            prefix.entries.clear();
            let mut rebased = hlasm::link_modules(&[prefix, linked]).unwrap();
            rebased.entry = rebased.entries["MAIN"];
            assert_eq!(rebased.zeus_hlasm, 12);
            assert_eq!(execute(&rebased).0, 67);
        }
    }
}

#[test]
fn relative_immediate_fetch_sign_extends_addends_and_store_ignores_them() {
    for width in [2, 4] {
        let mut records = relative_caller(width, -3);
        let caller = hlasm::load_module(&cards(&records)).unwrap();
        let linked = hlasm::link_modules(&[caller, relative_target()]).unwrap();
        let expected = (i64::from(linked.entries["SUB"]) - 6) / 2;
        assert_eq!(
            &linked.storage_at(4096).unwrap()[2..2 + width as usize],
            &expected.to_be_bytes()[8 - width as usize..]
        );
        records[5] = rld(&fix(3, 2, 0x102, [0, 0x60, 1, 0, width, 0]));
        let caller = hlasm::load_module(&cards(&records)).unwrap();
        let linked = hlasm::link_modules(&[caller, relative_target()]).unwrap();
        assert_eq!(execute(&linked).0, 67);
    }
}

#[test]
fn relative_immediate_rejects_odd_targets_out_of_range_and_older_module_readers() {
    let caller = hlasm::load_module(&cards(&relative_caller(2, 0))).unwrap();
    let mut old = caller.clone();
    old.zeus_hlasm = 11;
    assert!(
        hlasm::load_module(&hlasm::module_bytes(&old).unwrap())
            .unwrap_err()
            .contains("version 12")
    );
    let mut odd = relative_target();
    odd.entries.insert("SUB".into(), 1);
    assert!(
        hlasm::link_modules(&[caller.clone(), odd])
            .unwrap_err()
            .contains("halfword aligned")
    );
    let mut far = relative_target();
    far.storage.resize(100000, 0);
    far.entries.insert("SUB".into(), 80000);
    assert!(
        hlasm::link_modules(&[caller, far])
            .unwrap_err()
            .contains("signed field width")
    );
    for flags in [
        [0, 0x60, 0, 0, 8, 0],
        [1, 0x60, 0, 0, 4, 0],
        [0, 0x61, 0, 0, 4, 0],
        [0, 0x60, 2, 0, 4, 0],
    ] {
        let mut records = relative_caller(4, 0);
        records[5] = rld(&fix(3, 2, 0x102, flags));
        assert!(hlasm::load_module(&cards(&records)).is_err(), "{flags:?}");
    }
}

#[test]
fn relative_immediate_store_replaces_mode_attributes_and_can_be_replaced_by_length_store() {
    let mut records = relative_caller(4, 0);
    let mut fields = fix(3, 2, 0x102, [1, 0, 1, 0, 4, 0]);
    fields.extend(fix(3, 2, 0x102, [0, 0x60, 1, 0, 4, 0]));
    records[5] = rld(&fields);
    let caller = hlasm::load_module(&cards(&records)).unwrap();
    assert!(caller.object_relocations[0].relative_immediate);
    assert_eq!(
        execute(&hlasm::link_modules(&[caller, relative_target()]).unwrap()).0,
        67
    );
    fields.extend(fix(2, 2, 0x102, [0, 0x21, 1, 0, 4, 0]));
    records[5] = rld(&fields);
    let caller = hlasm::load_module(&cards(&records)).unwrap();
    assert!(caller.object_relocations.is_empty());
    assert_eq!(caller.zeus_hlasm, 11);
    assert_eq!(&caller.storage_at(4096).unwrap()[2..6], 8u32.to_be_bytes());
}

#[test]
fn relative_immediate_library_discovery_uses_relocation_terms_without_external_table() {
    let mut caller = hlasm::load_module(&cards(&relative_caller(4, 0))).unwrap();
    caller.externals.clear();
    let dir = tempfile::tempdir().unwrap();
    hlasm::save_module(&relative_target(), &dir.path().join("SUB.zo")).unwrap();
    let linked = hlasm::link_with_libraries(vec![caller], &[dir.path().to_path_buf()]).unwrap();
    assert_eq!(execute(&linked).0, 67);
}
