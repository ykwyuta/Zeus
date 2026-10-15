use zeus_cobol::hlasm;

#[test]
fn adata_user_records_validate_without_output_and_preserve_macro_and_copy_locations() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("main.asm");
    std::fs::write(dir.path().join("DATA.cpy"), " ADATA 1,,3,,'Copy,a''b'\n").unwrap();
    std::fs::write(&source, " MACRO\n NOTE &N,&T\n ADATA &N,-2,3,4,'&T'\n MEND\nMAIN CSECT\n BR 14\n ADATA\n COPY DATA\n NOTE 42,macro\n END\n").unwrap();
    let dirs = [dir.path().to_path_buf()];
    let plain = hlasm::compile_report(&source, &dirs);
    assert!(plain.module.is_some(), "{:?}", plain.diagnostics);
    let report = hlasm::compile_report_with_options(
        &source,
        &dirs,
        &hlasm::AssemblyOptions {
            collect_listing: true,
            ..Default::default()
        },
    );
    assert_eq!(
        serde_json::to_value(plain.module).unwrap(),
        serde_json::to_value(&report.module).unwrap()
    );
    let listing = report.listing.unwrap();
    assert_eq!(listing.user_data.len(), 3);
    assert_eq!(listing.user_data[0].data.values, [0; 4]);
    assert_eq!(listing.user_data[0].data.text, "");
    assert!(listing.user_data[1].location.file.ends_with("DATA.cpy"));
    assert_eq!(listing.user_data[1].data.values, [1, 0, 3, 0]);
    assert_eq!(listing.user_data[1].data.text, "Copy,a'b");
    assert_eq!(listing.user_data[2].location.line, 9);
    assert_eq!(listing.user_data[2].data.values, [42, -2, 3, 4]);
    assert_eq!(listing.user_data[2].data.text, "macro");
    let module = report.module.unwrap();
    assert_eq!(module.code.len(), 1);
    assert!(listing.text().contains("ADATA"));
    let back: hlasm::AssemblyListing =
        serde_json::from_value(serde_json::to_value(&listing).unwrap()).unwrap();
    assert_eq!(back.user_data[2].data, listing.user_data[2].data);
}

#[test]
fn user_data_binary_record_has_architecture_header_offsets_and_signed_fields() {
    let data = hlasm::UserData {
        values: [i32::MIN, i32::MAX, -1, 0],
        text: "a'b".into(),
    };
    let record = data.record(37).unwrap();
    // Independent consumer reads the documented header and offsets, all big-endian.
    assert_eq!(&record[..10], &[16, 0, 0x70, 3, 0, 1, 0, 0, 0, 0]);
    assert_eq!(u16::from_be_bytes(record[10..12].try_into().unwrap()), 27);
    for (index, expected) in data.values.iter().enumerate() {
        let at = 12 + index * 4;
        assert_eq!(
            i32::from_be_bytes(record[at..at + 4].try_into().unwrap()),
            *expected
        );
    }
    let offset = u32::from_be_bytes(record[28..32].try_into().unwrap()) as usize;
    let length = u32::from_be_bytes(record[32..36].try_into().unwrap()) as usize;
    assert_eq!((offset, length, record.len()), (36, 3, 39));
    assert_eq!(&record[offset..offset + length], &[0x81, 0x7d, 0x82]);
    let empty = hlasm::UserData {
        values: [0; 4],
        text: String::new(),
    }
    .record(37)
    .unwrap();
    assert_eq!(empty.len(), 36);
    assert_eq!(&empty[28..36], &[0; 8]);
}

#[test]
fn adata_checks_decimal_terms_quoted_strings_labels_and_encoded_byte_limit() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("invalid.asm");
    for (args, expected) in [
        ("2147483648", "signed 32"),
        ("-2147483649", "signed 32"),
        ("1+2", "decimal self-defining"),
        ("X'01'", "decimal self-defining"),
        ("+", "decimal self-defining"),
        ("1,2,3,4,unquoted", "apostrophes"),
        ("1,2,3,4,'a'b'", "unpaired apostrophe"),
        ("1,2,3,4,'a',6", "four decimal"),
    ] {
        std::fs::write(
            &source,
            format!("MAIN CSECT\n BR 14\n ADATA {args}\n END\n"),
        )
        .unwrap();
        let report = hlasm::compile_report(&source, &[]);
        assert!(report.module.is_none(), "{args}: accepted");
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.line == 3 && d.message.contains(expected)),
            "{args}: {:?}",
            report.diagnostics
        );
    }
    for (label, valid) in [("NAME", false), (".SEQ", true)] {
        std::fs::write(
            &source,
            format!("MAIN CSECT\n BR 14\n{label} ADATA -2147483648,+2147483647\n END\n"),
        )
        .unwrap();
        assert_eq!(
            hlasm::compile_report(&source, &[]).module.is_some(),
            valid,
            "{label}"
        );
    }
    for (length, valid) in [(255, true), (256, false)] {
        let mut statement = format!(" ADATA ,,,,'{}'", "A".repeat(length));
        let mut records = String::new();
        let mut continuation = false;
        while !statement.is_empty() {
            let limit = if continuation { 56 } else { 71 };
            let end = limit.min(statement.len());
            let prefix = if continuation { "               " } else { "" };
            records.push_str(prefix);
            records.push_str(&statement[..end]);
            statement = statement[end..].to_string();
            if !statement.is_empty() {
                records.push('+');
            }
            records.push('\n');
            continuation = true;
        }
        std::fs::write(&source, format!("MAIN CSECT\n BR 14\n{records} END\n")).unwrap();
        assert_eq!(
            hlasm::compile_report(&source, &[]).module.is_some(),
            valid,
            "{length}"
        );
    }
}

#[test]
fn adata_mixed_dbcs_strings_use_encoded_byte_count_and_reject_unmappable_data() {
    let cp = zeus_cobol::codepage::get(939).unwrap();
    let data = hlasm::UserData {
        values: [0; 4],
        text: "日本語".into(),
    };
    let encoded = cp.encode(&data.text);
    let record = data.record(939).unwrap();
    assert_eq!(&record[36..], encoded);
    assert_eq!(
        u32::from_be_bytes(record[32..36].try_into().unwrap()) as usize,
        encoded.len()
    );
    assert!(
        data.record(37)
            .unwrap_err()
            .contains("cannot be represented")
    );
    let overlong = hlasm::UserData {
        values: [0; 4],
        text: "日".repeat(127),
    };
    assert!(
        overlong
            .record(939)
            .unwrap_err()
            .contains("255 encoded bytes")
    );
    assert!(
        data.record(999)
            .unwrap_err()
            .contains("unsupported ADATA CCSID")
    );
}

#[test]
fn ebcdic_adata_source_validates_against_the_input_ccsid() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dbcs.asm");
    let cp = zeus_cobol::codepage::get(939).unwrap();
    let bytes: Vec<_> = ["MAIN CSECT", " BR 14", " ADATA ,,,,'日本語'", " END"]
        .iter()
        .flat_map(|line| {
            let mut bytes = cp.encode(line);
            bytes.resize(80, 0x40);
            bytes
        })
        .collect();
    std::fs::write(&path, bytes).unwrap();
    let report = hlasm::compile_report_with_options(
        &path,
        &[],
        &hlasm::AssemblyOptions {
            collect_listing: true,
            source_encoding: hlasm::SourceEncoding::Ebcdic {
                ccsid: 939,
                record_length: 80,
            },
            ..Default::default()
        },
    );
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    assert_eq!(report.listing.unwrap().user_data[0].data.text, "日本語");
}

#[test]
fn physical_record_transport_preserves_all_families_endianness_and_continuations() {
    let mut records: Vec<_> = [
        0, 1, 2, 0x0a, 0x0b, 0x10, 0x20, 0x30, 0x32, 0x34, 0x35, 0x36, 0x40, 0x42, 0x44, 0x45,
        0x60, 0x62, 0x70, 0x80, 0x90, 0xffff,
    ]
    .into_iter()
    .map(|kind| hlasm::AdataRecord::new(kind, 1, vec![0x41, 0x42, 0xff]))
    .collect();
    records[0].little_endian = true;
    let mut first = hlasm::AdataRecord::new(0x70, 1, vec![1, 2, 3]);
    first.continued = true;
    records.push(first);
    records.push(hlasm::AdataRecord::new(0x70, 1, vec![4, 5]));
    for rdw in [false, true] {
        let bytes = hlasm::encode_adata_records(&records, rdw).unwrap();
        let start = if rdw { 4 } else { 0 };
        assert_eq!(
            &bytes[start..start + 12],
            &[16, 0, 0, 3, 2, 1, 0, 0, 0, 0, 3, 0]
        );
        assert_eq!(hlasm::decode_adata_records(&bytes, rdw).unwrap(), records);
        if rdw {
            assert_eq!(&bytes[..4], &[0, 19, 0, 0]);
        }
        for length in [1, 2, 3, 4, 10, 11, bytes.len() - 1] {
            assert!(
                hlasm::decode_adata_records(&bytes[..length], rdw).is_err(),
                "{rdw}/{length}"
            );
        }
        for (at, value) in [(start, 15), (start + 3, 4), (start + 4, 4), (start + 6, 1)] {
            let mut bad = bytes.clone();
            bad[at] = value;
            assert!(hlasm::decode_adata_records(&bad, rdw).is_err());
        }
    }
    let mut bad = hlasm::AdataRecord::new(0x70, 1, vec![]);
    bad.continued = true;
    assert!(hlasm::encode_adata_records(&[bad.clone()], false).is_err());
    assert!(
        hlasm::encode_adata_records(&[bad, hlasm::AdataRecord::new(0x80, 1, vec![])], false)
            .is_err()
    );
    let oversized = hlasm::AdataRecord::new(0x70, 1, vec![0; 65535]);
    assert!(hlasm::encode_adata_records(&[oversized], true).is_err());
}

fn read_word(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn record_text(bytes: &[u8], at: usize) -> String {
    let offset = read_word(bytes, at) as usize;
    let length = read_word(bytes, at + 4) as usize;
    zeus_cobol::codepage::get(37)
        .unwrap()
        .decode(&bytes[offset..offset + length])
}
#[test]
fn assembly_stream_has_independent_offsets_instruction_bytes_and_exact_unit_counts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("analysis.asm");
    std::fs::write(&path,"MAIN CSECT\n USING MAIN,15\n ADBR 1,2\n BR 14\n ADATA 7,-8,,,'note'\n MNOTE 0,'information'\nVALUE DC F'3'\n END\n").unwrap();
    let options = hlasm::AssemblyOptions {
        collect_listing: true,
        ..Default::default()
    };
    let mut report = hlasm::compile_report_with_options(&path, &[], &options);
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    // Force multiple physical xref records without depending on encoder helpers.
    report
        .listing
        .as_mut()
        .unwrap()
        .symbols
        .get_mut("VALUE")
        .unwrap()
        .references = (1..=9001)
        .map(|statement| hlasm::SourceLocation {
            statement,
            file: "analysis.asm".into(),
            line: statement,
        })
        .collect();
    for rdw in [false, true] {
        let bytes = hlasm::adata_bytes(&report, &options, 37, 0x123456789abcdef0, rdw).unwrap();
        let records = hlasm::decode_adata_records(&bytes, rdw).unwrap();
        assert_eq!(records[0].record_type, 2);
        assert_eq!(records[0].data, &[0; 8]);
        assert_eq!(records[1].record_type, 1);
        assert_eq!(
            records[1].data,
            &[0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0, 37]
        );
        let last = records.last().unwrap();
        assert_eq!(last.record_type, 2);
        assert_eq!(&last.data[..4], &[0, 1, 0, 0]);
        assert_eq!(read_word(&last.data, 4) as usize, records.len());
        let instruction = records.iter().find(|r| r.record_type == 0x36).unwrap();
        assert_eq!(read_word(&instruction.data, 0), 1);
        assert_eq!(read_word(&instruction.data, 4), 3);
        assert_eq!(read_word(&instruction.data, 20), 40);
        assert_eq!(read_word(&instruction.data, 24), 4);
        assert_eq!(&instruction.data[28..], &[0xb3, 0x1a, 0, 0x12]);
        let source = records
            .iter()
            .find(|r| r.record_type == 0x30 && read_word(&r.data, 4) == 3)
            .unwrap();
        let encoded = hlasm::encode_adata_records(std::slice::from_ref(source), false).unwrap();
        assert_eq!(record_text(&encoded, 120), " ADBR 1,2");
        assert_eq!(read_word(&source.data, 60), 1);
        assert_eq!(read_word(&source.data, 64), 4);
        let user = records.iter().find(|r| r.record_type == 0x70).unwrap();
        let user = hlasm::encode_adata_records(std::slice::from_ref(user), false).unwrap();
        assert_eq!(&user[12..20], &[0, 0, 0, 7, 0xff, 0xff, 0xff, 0xf8]);
        assert_eq!(record_text(&user, 28), "note");
        let symbol = records
            .iter()
            .find(|r| {
                r.record_type == 0x42 && {
                    let full =
                        hlasm::encode_adata_records(std::slice::from_ref(*r), false).unwrap();
                    record_text(&full, 54) == "VALUE"
                }
            })
            .unwrap();
        assert_eq!(read_word(&symbol.data, 26), 4);
        let xrefs: Vec<_> = records
            .iter()
            .filter(|r| r.record_type == 0x44 && r.data.len() > 1000)
            .collect();
        assert_eq!(xrefs.len(), 3);
        assert!(xrefs[0].continued);
        assert!(xrefs[1].continued);
        assert!(!xrefs[2].continued);
        assert_eq!(read_word(&xrefs[0].data, 20), 9001);
        assert_eq!(
            xrefs.iter().map(|r| read_word(&r.data, 24)).sum::<u32>(),
            9001
        );
        let diagnostic = records.iter().find(|r| r.record_type == 0x32).unwrap();
        let full = hlasm::encode_adata_records(std::slice::from_ref(diagnostic), false).unwrap();
        assert!(record_text(&full, 35).contains("information"));
    }
}

#[test]
fn cli_writes_adata_and_preserves_existing_output_on_encoding_failure() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("file.asm");
    let output = dir.path().join("file.adata");
    std::fs::write(&source, "MAIN CSECT\n BR 14\n ADATA 1,2,3,4,'cli'\n END\n").unwrap();
    let invoke = || {
        std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
            .arg("check")
            .arg(&source)
            .arg("--adata")
            .arg(&output)
            .args(["--adata-ut", "123456", "--adata-rdw"])
            .output()
            .unwrap()
    };
    let result = invoke();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let before = std::fs::read(&output).unwrap();
    let records = hlasm::decode_adata_records(&before, true).unwrap();
    assert!(records.iter().any(|r| r.record_type == 0x70));
    std::fs::write(&source, "MAIN CSECT\n MVST 2,4\n BR 14\n END\n").unwrap();
    let result = invoke();
    assert_eq!(result.status.code(), Some(12));
    assert!(String::from_utf8_lossy(&result.stderr).contains("ADATA instruction output"));
    assert_eq!(std::fs::read(&output).unwrap(), before);
    for command in ["run", "link"] {
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
            .arg(command)
            .arg(&source)
            .arg("--adata")
            .arg(&output)
            .args(["--adata-ut", "1"])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(12));
        assert!(String::from_utf8_lossy(&result.stderr).contains("compile/check"));
    }
}

#[test]
fn generated_data_layout_preserves_each_nominal_bit_position_and_relocation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("layout.asm");
    std::fs::write(&path,"MAIN CSECT\n BR 14\nMIX DC 2FP(123)L2S1'1,2',BL.3'101',BL.5'11010',AD(VALUE)\nHERE DC 3A(*)\nCHARS DC 2C'*'\nEMPTY DS 0F\nSPACE DS 3CL4\nVALUE DC F'7'\n END\n").unwrap();
    let options = hlasm::AssemblyOptions {
        collect_listing: true,
        ..Default::default()
    };
    let report = hlasm::compile_report_with_options(&path, &[], &options);
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let listing = report.listing.as_ref().unwrap();
    let mix = listing
        .statements
        .iter()
        .find(|r| r.label == "MIX")
        .unwrap();
    assert!(
        mix.data_layout_error.is_none(),
        "{:?}",
        mix.data_layout_error
    );
    assert_eq!(mix.data_operands.len(), 4);
    let fields = &mix.data_operands;
    assert_eq!(fields[0].duplication, 2);
    assert_eq!(fields[0].program_type, 123);
    assert_eq!(fields[0].scale_attribute, 1);
    assert_eq!(fields[0].values[0].data, &[0, 2]);
    assert_eq!(fields[0].values[1].data, &[0, 4]);
    assert_eq!(fields[1].bit_offset, 0);
    assert_eq!(fields[1].values[0].bit_length, 3);
    assert_eq!(fields[1].values[0].data, &[0xa0]);
    assert_eq!(fields[2].bit_offset, 3);
    assert_eq!(fields[2].values[0].bit_length, 5);
    assert_eq!(fields[2].values[0].data, &[0xd0]);
    assert_eq!(fields[3].offset % 8, 0);
    assert_eq!(fields[3].values[0].byte_length, 8);
    let bytes = hlasm::adata_bytes(&report, &options, 37, 42, false).unwrap();
    let records = hlasm::decode_adata_records(&bytes, false).unwrap();
    let first = records.iter().position(|r| r.record_type == 0x34).unwrap();
    assert!(records[first].continued);
    assert!(records[first + 1].continued);
    assert!(records[first + 2].continued);
    assert!(!records[first + 3].continued);
    let full = hlasm::encode_adata_records(&records[first..first + 4], false).unwrap();
    // First operand pointer = header(12)+fixed data(22), relative to the record.
    assert_eq!(read_word(&full, 30), 34);
    assert_eq!(read_word(&full, 42), 2);
    assert_eq!(read_word(&full, 49), 123);
    let extension: Vec<_> = records.iter().filter(|r| r.record_type == 0x35).collect();
    assert_eq!(extension.len(), 1);
    let here = listing
        .statements
        .iter()
        .find(|r| r.label == "HERE")
        .unwrap();
    assert_eq!(
        &extension[0].data[28..],
        &report.module.as_ref().unwrap().storage[here.offset as usize..here.offset as usize + 12]
    );
    for n in 0..3 {
        assert_eq!(
            read_word(&extension[0].data, 28 + n * 4),
            here.offset + n as u32 * 4
        );
    }
    let wide_rld = records
        .iter()
        .find(|r| r.record_type == 0x40 && read_word(&r.data, 12) == fields[3].offset)
        .unwrap();
    assert_eq!(wide_rld.data[16], 0x4c);
    assert_eq!(read_word(&wide_rld.data, 4), 1);
    let empty = listing
        .statements
        .iter()
        .find(|r| r.label == "EMPTY")
        .unwrap();
    assert_eq!(empty.data_operands[0].duplication, 0);
    assert!(empty.data_operands[0].values[0].data.is_empty());
    let stats = records.iter().find(|r| r.record_type == 0x90).unwrap();
    assert_eq!(stats.edition, 2);
    assert_eq!(read_word(&stats.data, 8), 9);
    assert_eq!(read_word(&stats.data, 32) as usize, records.len());
    assert!(records.iter().any(|r| r.record_type == 0x0b));
    assert!(
        records
            .iter()
            .any(|r| r.record_type == 0x10 && r.edition == 3)
    );
}

#[test]
fn generated_using_and_gpr_records_distinguish_floating_registers_and_drop_targets() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("refs.asm");
    std::fs::write(&path,"MAIN CSECT\n USING MAIN,15\n LR 12,15\n USING MAIN,12\n LE 4,NUM\n SFPC 7\n EFPC 8\n LM 9,11,WORK\n DROP 12\n BR 14\nNUM DC EB'1'\nWORK DS 3F\n END\n").unwrap();
    let options = hlasm::AssemblyOptions {
        collect_listing: true,
        ..Default::default()
    };
    let report = hlasm::compile_report_with_options(&path, &[], &options);
    let records = hlasm::decode_adata_records(
        &hlasm::adata_bytes(&report, &options, 37, 1, true).unwrap(),
        true,
    )
    .unwrap();
    let registers: Vec<_> = records.iter().filter(|r| r.record_type == 0x45).collect();
    assert!(!registers.iter().any(|r| r.data[0] == 4));
    for (register, statement, flag) in [
        (15, 2, 0xe4),
        (12, 4, 0xe4),
        (12, 9, 0xc4),
        (7, 6, 0x40),
        (8, 7, 0xd4),
        (9, 8, 0xd4),
        (10, 8, 0xd4),
        (11, 8, 0xd4),
    ] {
        let record = registers.iter().find(|r| r.data[0] == register).unwrap();
        let encoded = hlasm::encode_adata_records(std::slice::from_ref(*record), false).unwrap();
        let at = read_word(&encoded, 24) as usize;
        let n = read_word(&encoded, 20) as usize;
        assert!(
            encoded[at..at + n * 5]
                .chunks_exact(5)
                .any(|r| read_word(r, 0) == statement && r[4] == flag),
            "register {register}"
        );
    }
    let maps: Vec<_> = records.iter().filter(|r| r.record_type == 0x80).collect();
    assert_eq!(maps.len(), 3);
    assert_eq!(maps[0].data[26], 15);
    assert_eq!(maps[1].data[26], 12);
    assert_eq!(maps[2].data[0], 0x80);
    assert_eq!(maps[2].data[26], 12);
    for map in maps {
        assert_eq!(read_word(&map.data, 40) % 8, 0);
    }
}

#[test]
fn data_nominals_keep_implicit_lengths_dummy_constants_and_a_versus_v_rld() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nominals.asm");
    std::fs::write(&path, "MAIN CSECT\n EXTRN EXT\n BR 14\nTXT DC C'HELLO',X'123456',P'12345',Z'12345'\nPTR DC A(EXT),V(EXT)\nAREA DSECT\nDUMMY DC C'WORLD',P'12345'\n END MAIN\n").unwrap();
    let options = hlasm::AssemblyOptions {
        collect_listing: true,
        ..Default::default()
    };
    let report = hlasm::compile_report_with_options(&path, &[], &options);
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let listing = report.listing.as_ref().unwrap();
    let text = listing
        .statements
        .iter()
        .find(|r| r.label == "TXT")
        .unwrap();
    assert!(
        text.data_layout_error.is_none(),
        "{:?}",
        text.data_layout_error
    );
    assert_eq!(
        text.data_operands
            .iter()
            .map(|o| o.values[0].byte_length)
            .collect::<Vec<_>>(),
        vec![5, 3, 3, 5]
    );
    assert_eq!(
        &text.data_operands[0].values[0].data,
        &[0xc8, 0xc5, 0xd3, 0xd3, 0xd6]
    );
    let dummy = listing
        .statements
        .iter()
        .find(|r| r.label == "DUMMY")
        .unwrap();
    assert!(
        dummy.data_layout_error.is_none(),
        "{:?}",
        dummy.data_layout_error
    );
    assert_eq!(
        &dummy.data_operands[0].values[0].data,
        &[0xe6, 0xd6, 0xd9, 0xd3, 0xc4]
    );
    assert_eq!(&dummy.data_operands[1].values[0].data, &[0x12, 0x34, 0x5c]);
    let records = hlasm::decode_adata_records(
        &hlasm::adata_bytes(&report, &options, 37, 1, false).unwrap(),
        false,
    )
    .unwrap();
    let flags: Vec<_> = records
        .iter()
        .filter(|r| r.record_type == 0x40)
        .map(|r| r.data[16])
        .collect();
    assert_eq!(flags, vec![0x0c, 0x1c]);
    let data = records
        .iter()
        .filter(|r| r.record_type == 0x34)
        .collect::<Vec<_>>();
    assert_eq!(data[0].data[4], 0x80);
    for r in &data[1..4] {
        assert_eq!(r.data[4], 0);
    }
}

#[test]
fn member_cross_references_preserve_nested_copy_parents_and_global_totals() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("DATA.cpy"), " COPY INNER\n").unwrap();
    std::fs::write(dir.path().join("INNER.cpy"), " DC X'12'\n").unwrap();
    let path = dir.path().join("main.asm");
    std::fs::write(&path, " MACRO\n FIRST\n COPY DATA\n MEND\n MACRO\n SECOND\n COPY DATA\n MEND\nMAIN CSECT\n BR 14\n COPY DATA\n FIRST\n SECOND\n END\n").unwrap();
    let options = hlasm::AssemblyOptions {
        collect_listing: true,
        ..Default::default()
    };
    let report = hlasm::compile_report_with_options(&path, &[dir.path().to_path_buf()], &options);
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let listing = report.listing.as_ref().unwrap();
    assert_eq!(listing.copy_calls.len(), 6);
    assert_eq!(
        listing
            .copy_calls
            .iter()
            .map(|c| c.parent.as_str())
            .collect::<Vec<_>>(),
        vec!["PRIMARY INPUT", "DATA", "FIRST", "DATA", "SECOND", "DATA"]
    );
    let records = hlasm::decode_adata_records(
        &hlasm::adata_bytes(&report, &options, 37, 1, false).unwrap(),
        false,
    )
    .unwrap();
    let cp = zeus_cobol::codepage::get(37).unwrap();
    let mut found = std::collections::BTreeMap::new();
    for r in records.iter().filter(|r| r.record_type == 0x62) {
        let full = hlasm::encode_adata_records(std::slice::from_ref(r), false).unwrap();
        let field = |at| {
            let start = read_word(&r.data, at) as usize;
            let length = read_word(&r.data, at + 4) as usize;
            cp.decode(&full[start..start + length])
        };
        let name = field(18);
        if !matches!(name.as_str(), "DATA" | "INNER") {
            continue;
        }
        let parent = field(26);
        assert_eq!(r.data[8], 0xd3);
        assert_eq!(read_word(&r.data, 34), 3);
        let n = read_word(&r.data, 38);
        let start = read_word(&r.data, 42) as usize;
        for reference in full[start..start + n as usize * 5].chunks_exact(5) {
            assert_eq!(reference[4], 0xc3);
        }
        found.insert((name, parent), n);
    }
    assert_eq!(found.len(), 4);
    assert_eq!(found[&("INNER".into(), "DATA".into())], 3);
    for parent in ["PRIMARY INPUT", "FIRST", "SECOND"] {
        assert_eq!(found[&("DATA".into(), parent.into())], 1);
    }
}

#[test]
fn quoted_asterisks_and_multiplication_do_not_request_location_extensions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("star.asm");
    std::fs::write(
        &path,
        "STAR CSECT\n BR 14\n DC 2A(C'*'),2A(3*7),2C'*'\n END\n",
    )
    .unwrap();
    let options = hlasm::AssemblyOptions {
        collect_listing: true,
        ..Default::default()
    };
    let report = hlasm::compile_report_with_options(&path, &[], &options);
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let records = hlasm::decode_adata_records(
        &hlasm::adata_bytes(&report, &options, 37, 1, false).unwrap(),
        false,
    )
    .unwrap();
    assert!(!records.iter().any(|r| r.record_type == 0x35));
}
