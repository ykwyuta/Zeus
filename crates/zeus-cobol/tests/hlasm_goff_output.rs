use zeus_cobol::hlasm;

fn compile(source: &str) -> hlasm::Module {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("goff-output.asm");
    std::fs::write(&path, source).unwrap();
    hlasm::compile_file(&path, &[]).unwrap()
}
fn roundtrip(module: &hlasm::Module) -> hlasm::Module {
    let bytes = hlasm::goff_bytes(module).unwrap();
    assert!(bytes.len().is_multiple_of(80));
    assert_eq!(&bytes[..3], &[3, 0xf0, 0]);
    assert_eq!(bytes, hlasm::goff_bytes(module).unwrap());
    let loaded = hlasm::load_module(&bytes).unwrap();
    assert!(loaded.code.is_empty());
    assert_eq!(loaded.storage.len(), module.storage.len());
    assert_eq!(loaded.entry, module.entry);
    assert_eq!(loaded.amode, module.amode);
    assert_eq!(loaded.rmode, module.rmode);
    assert_eq!(loaded.alignment, module.alignment);
    assert_eq!(loaded.entries, module.entries);
    for at in module.entries.values() {
        assert_eq!(loaded.entry_mode(*at), module.entry_mode(*at));
    }
    loaded
}
fn execute(module: &hlasm::Module, base: usize) -> (i32, Vec<u8>) {
    let mut mem = vec![0; base];
    mem.extend(module.storage_at(base).unwrap());
    let rc = hlasm::execute(module, &mut mem, base, &[]).unwrap();
    (rc, mem)
}

fn protected(module: &hlasm::Module, at: u32) -> bool {
    module.read_only.iter().any(|r| r.contains(&at))
}

#[test]
fn partial_rsect_code_data_and_scratch_keep_addresses_and_protection() {
    let source = compile(
        "MAIN RSECT\n USING MAIN,15\n LR 12,14\n L 3,PTR\n L 2,0(3)\n AHI 2,7\n ST 2,0(3)\n LA 15,21\n BR 12\nPTR DC A(VALUE)\nDATA CSECT\n ENTRY VALUE\nVALUE DC F'5'\n END MAIN\n",
    );
    let object = roundtrip(&source);
    assert!(object.bindings.sections.len() >= 3);
    for at in 0..source.storage.len() as u32 {
        assert_eq!(protected(&object, at), protected(&source, at));
    }
    assert!(object.bindings.sections.iter().any(|s| s.alignment == 1));
    let value = source.entries["VALUE"] as usize;
    for base in [0, 4096, 2 << 20] {
        let (rc, mem) = execute(&object, base);
        assert_eq!(rc, 21);
        assert_eq!(&mem[base + value..base + value + 4], &12i32.to_be_bytes());
        let at = source.relocations[0].offset as usize;
        assert_eq!(
            &object.storage_at(base).unwrap()[at..at + 4],
            &source.storage_at(base).unwrap()[at..at + 4]
        );
    }
    let second = roundtrip(&object);
    assert_eq!(second.machine_code, object.machine_code);
    assert_eq!(second.read_only, object.read_only);
    assert_eq!(execute(&second, 4096).0, 21);
}

#[test]
fn emitted_rsect_still_rejects_stores_into_protected_data() {
    let source =
        compile("MAIN RSECT\n USING MAIN,15\n MVI CONSTANT,C'X'\n BR 14\nCONSTANT DC C'A'\n END\n");
    let object = roundtrip(&source);
    let error = zeus_cobol::vm::Machine::new(vec![])
        .run_hlasm_main(object)
        .unwrap_err();
    assert!(error.contains("storage protection"), "{error}");
}

#[test]
fn generated_classes_are_deterministic_and_distinguish_different_sections() {
    let first = roundtrip(&compile("FIRST RSECT\n BR 14\n END\n"));
    let second = roundtrip(&compile("SECOND RSECT\n BR 14\n END\n"));
    let names: std::collections::HashSet<_> =
        first.bindings.sections.iter().map(|s| &s.name).collect();
    for section in &second.bindings.sections {
        assert_eq!(section.name.len(), 16);
        assert!(section.name.starts_with("Z_"));
        assert!(!names.contains(&section.name));
    }
}

#[test]
fn generated_element_order_agrees_in_ascii_and_ebcdic() {
    let mut source = compile("ORDERING RSECT\n BR 14\n END\n");
    source.read_only = (0..64).step_by(4).map(|n| n..n + 2).collect();
    let object = roundtrip(&source);
    assert!(object.bindings.sections.len() > 30);
    for pair in object.bindings.sections.windows(2) {
        assert!(pair[0].name < pair[1].name);
        assert!(
            zeus_cobol::ebcdic::encode(&pair[0].name) < zeus_cobol::ebcdic::encode(&pair[1].name)
        );
    }
}

#[test]
fn end_and_relative_external_fixups_use_the_nonfirst_element() {
    let source = compile(
        "BASE RSECT\n DC D'0'\nMAIN CSECT\n ENTRY MAIN\n LR 12,14\n EXTRN TARGET\n BRASL 14,TARGET\n BR 12\n END MAIN\n",
    );
    let object = roundtrip(&source);
    assert_eq!(object.entry, 8);
    assert!(!object.machine_code.iter().any(|r| r.contains(&0)));
    assert_eq!(object.object_relocations[0].offset, 12);
    let target = compile("TARGET CSECT\n LA 15,39\n BR 14\n END\n");
    let linked = hlasm::link_modules(&[object, target]).unwrap();
    for base in [0, 4096, 2 << 20] {
        assert_eq!(execute(&linked, base).0, 39);
    }
}

#[test]
fn mode_sensitive_pointer_in_a_later_element_retains_its_tag() {
    for mode in [24, 31, 64] {
        let mut source = tagged(mode, false);
        source.read_only = std::iter::once(0..2).collect();
        let object = roundtrip(&source);
        assert!(object.bindings.sections.len() > 1);
        for base in [0, 4096, 2 << 20] {
            assert_eq!(
                &source.storage_at(base).unwrap()[8..16],
                &object.storage_at(base).unwrap()[8..16]
            );
        }
    }
}

#[test]
fn imported_nonexecutable_holes_keep_their_exact_bytes_and_offsets() {
    let mut source = compile("HOLES CSECT\n LA 15,37\n BR 14\n END\n");
    source.storage.truncate(6);
    let mut object = roundtrip(&source);
    object.storage = vec![
        0x41, 0xf0, 0, 37, 7, 0xfe, 0xAA, 0xBB, 0x41, 0xf0, 0, 19, 7, 0xfe,
    ];
    object.machine_code = vec![0..6, 8..14];
    object.bindings.sections.clear();
    object.entry = 8;
    object.entries.insert("SECOND".into(), 8);
    let loaded = roundtrip(&object);
    assert_eq!(loaded.storage, object.storage);
    assert_eq!(loaded.machine_code, object.machine_code);
    assert_eq!(execute(&loaded, 4096).0, 19);
    let reemitted = roundtrip(&loaded);
    assert_eq!(reemitted.machine_code, loaded.machine_code);
    assert_eq!(reemitted.storage, loaded.storage);
}

#[test]
fn instruction_and_address_fields_crossing_elements_are_rejected() {
    let mut source = compile("SPLIT RSECT\n BR 14\n END\n");
    source.read_only = std::iter::once(0..1).collect();
    assert!(
        hlasm::goff_bytes(&source)
            .unwrap_err()
            .contains("element boundary")
    );
    source.code.clear();
    source.read_only = std::iter::once(0..5).collect();
    source.relocations.push(hlasm::Relocation {
        offset: 4,
        width: 4,
        target: 0,
        symbol: None,
        addend: 0,
        weak: false,
        expression: None,
    });
    assert!(
        hlasm::goff_bytes(&source)
            .unwrap_err()
            .contains("element boundary")
    );
}

#[test]
fn mixed_imported_caller_and_source_rsect_callee_keep_later_data_relocations() {
    let caller = roundtrip(&compile(
        "CALLER CSECT\n LR 12,14\n EXTRN TARGET\n BRASL 14,TARGET\n BR 12\n END\n",
    ));
    let target = compile(
        "TARGET RSECT\n USING TARGET,10\n LARL 10,TARGET\n L 3,PTR\n L 2,0(3)\n AHI 2,1\n ST 2,0(3)\n LA 15,45\n BR 14\nPTR DC A(VALUE)\nDATA CSECT\n ENTRY VALUE\nVALUE DC F'9'\n END TARGET\n",
    );
    let linked = hlasm::link_modules(&[caller, target]).unwrap();
    let output = roundtrip(&linked);
    let value = output.entries["VALUE"] as usize;
    for at in 0..linked.storage.len() as u32 {
        assert_eq!(protected(&linked, at), protected(&output, at));
    }
    for base in [0, 4096, 2 << 20] {
        let (rc, memory) = execute(&output, base);
        assert_eq!(rc, 45);
        assert_eq!(
            &memory[base + value..base + value + 4],
            &10i32.to_be_bytes()
        );
    }
    assert_eq!(execute(&roundtrip(&output), 4096).0, 45);
}

#[test]
fn source_emits_independent_goff_headers_attributes_opcodes_and_record_count() {
    let mut source = compile("SMALL CSECT\n LA 15,37\n BR 14\n END\n");
    source.storage.truncate(6);
    let bytes = hlasm::goff_bytes(&source).unwrap();
    assert_eq!(bytes.len(), 6 * 80);
    assert_eq!(&bytes[48..54], &[0, 0, 0, 1, 0, 0]);
    // SD=1, ED=2, LD=3; callable AMODE is on LD, RMODE on ED.
    assert_eq!(&bytes[80..92], &[3, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0]);
    assert_eq!(bytes[80 + 60], 0);
    assert_eq!(bytes[240 + 60], 2);
    assert_eq!(&bytes[160 + 60..160 + 67], &[0, 3, 0, 2, 0, 0, 3]);
    assert_eq!(&bytes[240..252], &[3, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 2]);
    assert_eq!(&bytes[320 + 24..320 + 30], &[0x41, 0xf0, 0, 37, 7, 0xfe]);
    assert_eq!(&bytes[400 + 8..400 + 16], &[0, 0, 0, 6, 0, 0, 0, 2]);
    let object = roundtrip(&source);
    for base in [0, 4096, 2 << 20] {
        assert_eq!(execute(&object, base).0, 37);
    }
    let reemitted = roundtrip(&object);
    assert_eq!(object.storage, reemitted.storage);
}

#[test]
fn long_external_names_and_text_cross_logical_and_physical_record_boundaries() {
    let mut source = compile("LONGROOT CSECT\n LA 15,19\n BR 14\n DC 70000C'X'\n END\n");
    let module_name = "A".repeat(170);
    source.entries.remove("LONGROOT");
    source.entries.insert(module_name.clone(), 0);
    source.name = module_name;
    source.entries.insert("B".repeat(240), 4);
    source.externals.insert("C".repeat(32767), true);
    let bytes = hlasm::goff_bytes(&source).unwrap();
    assert!(bytes.as_chunks::<80>().0.iter().any(|b| b[1] == 3));
    assert!(
        bytes
            .as_chunks::<80>()
            .0
            .iter()
            .filter(|b| b[1] == 0x11)
            .count()
            >= 3
    );
    let object = roundtrip(&source);
    assert_eq!(source.name, object.name);
    assert_eq!(source.externals, object.externals);
    assert_eq!(execute(&object, 4096).0, 19);
    assert_eq!(&object.storage[6..], &source.storage[6..]);
}

#[test]
fn signed_local_external_and_weak_address_terms_keep_all_eight_field_widths() {
    let mut source = compile(
        "ADCONS CSECT\n BR 14\n EXTRN TARGET\n WXTRN ABSENT\n DC AL2(*),AL2(*),AL3(*),AL4(*)\n DC ADL5(*),ADL6(*),ADL7(*),AD(*)\n DC A(TARGET-ADCONS+7),AD(TARGET+3),A(ABSENT-4)\n END\n",
    );
    source.relocations[0].width = 1;
    let object = roundtrip(&source);
    let target = compile("TARGET CSECT\n BR 14\n END\n");
    let original = hlasm::link_modules(&[source, target.clone()]).unwrap();
    let restored = hlasm::link_modules(&[object, target]).unwrap();
    for base in [0, 4096, 2 << 20] {
        let a = original.storage_at(base).unwrap();
        let b = restored.storage_at(base).unwrap();
        for r in &original.relocations {
            let at = r.offset as usize;
            assert_eq!(&a[at..at + r.width as usize], &b[at..at + r.width as usize]);
        }
    }
}

#[test]
fn source_relative_externals_link_for_both_displacement_widths_and_both_directions() {
    for operation in ["BRAS", "BRASL"] {
        for reverse in [false, true] {
            let caller = compile(&format!(
                "CALLER CSECT\n LR 12,14\n EXTRN TARGET\n {operation} 14,TARGET+2\n BR 12\n DS 0D\n END\n"
            ));
            let object = roundtrip(&caller);
            assert_eq!(object.object_relocations.len(), 1);
            assert!(object.object_relocations[0].relative_immediate);
            let target = compile("TARGET CSECT\n BR 14\n LA 15,43\n BR 14\n DS 0D\n END\n");
            let mut modules = vec![object, roundtrip(&target)];
            if reverse {
                modules.reverse();
            }
            let mut linked = hlasm::link_modules(&modules).unwrap();
            linked.entry = linked.entries["CALLER"];
            for base in [0, 4096, 2 << 20] {
                assert_eq!(execute(&linked, base).0, 43);
            }
            // A resolved PC-relative field can be emitted as constant text.
            if !reverse {
                let second = roundtrip(&linked);
                assert_eq!(execute(&second, 4096).0, 43);
            }
        }
    }
}

#[test]
fn loaded_relative_fixups_reemit_and_rebase_without_an_external_declaration_table() {
    let source =
        compile("CALLER CSECT\n LR 12,14\n EXTRN TARGET\n BRASL 14,TARGET-2\n BR 12\n END\n");
    let mut object = roundtrip(&source);
    object.externals.clear();
    let second = roundtrip(&object);
    assert_eq!(second.externals.get("TARGET"), Some(&false));
    let mut target = compile("TARGET CSECT\n LA 15,29\n BR 14\n END\n");
    // Export an interior address so the negative RI addend reaches the code.
    target.entries.insert("TARGET".into(), 2);
    let mut linked = hlasm::link_modules(&[second, target]).unwrap();
    linked.entry = 0;
    for base in [4096, 2 << 20] {
        assert_eq!(execute(&linked, base).0, 29);
    }
}

fn tagged(mode: u8, external: bool) -> hlasm::Module {
    let mut source = compile(&format!(
        "TAGS CSECT\n AMODE {mode}\n ENTRY PTR\n BR 14\nPTR DC AD(0)\n END\n"
    ));
    source.zeus_hlasm = 11;
    source.object_relocations.push(hlasm::ObjectRelocation {
        offset: source.entries["PTR"],
        width: if mode == 64 { 8 } else { 4 },
        relative_immediate: false,
        addressing_mode: if external {
            hlasm::ObjectAddressMode::External {
                symbol: "TARGET".into(),
                weak: false,
            }
        } else {
            hlasm::ObjectAddressMode::Known(mode)
        },
        expression: hlasm::RelocationExpression {
            addend: 0,
            local_coefficient: if external { 0 } else { 1 },
            terms: if external {
                vec![hlasm::RelativeTerm {
                    symbol: "TARGET".into(),
                    coefficient: 1,
                    weak: false,
                }]
            } else {
                vec![]
            },
        },
    });
    source
}

#[test]
fn known_and_external_mode_tags_survive_output_linking_and_origin_changes() {
    for mode in [24, 31, 64] {
        let source = tagged(mode, false);
        let object = roundtrip(&source);
        assert_eq!(object.object_relocations.len(), 1);
        for base in [0, 4096, 2 << 20] {
            assert_eq!(
                source.storage_at(base).unwrap()[8..16],
                object.storage_at(base).unwrap()[8..16]
            );
        }
        let external = tagged(mode, true);
        let object = roundtrip(&external);
        let target = compile(&format!("TARGET CSECT\n AMODE {mode}\n BR 14\n END\n"));
        let original = hlasm::link_modules(&[external, target.clone()]).unwrap();
        let restored = hlasm::link_modules(&[object, target]).unwrap();
        for base in [0, 4096, 2 << 20] {
            assert_eq!(
                original.storage_at(base).unwrap()[8..16],
                restored.storage_at(base).unwrap()[8..16]
            );
        }
    }
}

#[test]
fn secondary_entry_modes_alignment_and_full_readonly_storage_are_retained() {
    let source =
        compile("MAIN CSECT\n BR 14\nOTHER CSECT\n AMODE 64\n LA 15,31\n BR 14\n END MAIN\n");
    let object = roundtrip(&source);
    assert_eq!(object.entry_mode(object.entries["OTHER"]), 64);
    let mut readonly = compile("READONLY RSECT\n AMODE 64\n LA 15,31\n BR 14\n END\n");
    readonly
        .storage
        .truncate(readonly.read_only[0].end as usize);
    readonly.alignment = 4096;
    let object = roundtrip(&readonly);
    assert_eq!(object.read_only, readonly.read_only);
    assert_eq!(execute(&object, 4096).0, 31);
}

#[test]
fn malformed_unrepresentable_metadata_and_relative_fields_are_diagnosed() {
    let mut class = compile("CLASSDEF CSECT\n BR 14\n END\n");
    class.bindings.labels[0].class = Some("OTHERCLASS".into());
    assert!(
        hlasm::goff_bytes(&class)
            .unwrap_err()
            .contains("label classes")
    );
    for (source, message) in [
        (
            "SERVICE CSECT\n LINK EP=TARGET\n BR 14\n END\n",
            "macro expansion",
        ),
        (
            "UNENC CSECT\n MVST 2,4\n BR 14\n END\n",
            "encoding is not implemented",
        ),
        (
            "ODD CSECT\n EXTRN TARGET\n BRASL 14,TARGET+1\n END\n",
            "halfword aligned",
        ),
        (
            "BIG CSECT\n EXTRN TARGET\n BRAS 14,TARGET+65536\n END\n",
            "out of range",
        ),
    ] {
        let error = hlasm::goff_bytes(&compile(source)).unwrap_err();
        assert!(error.contains(message), "{error}");
    }
    let mut source = compile("BADNAME CSECT\n BR 14\n END\n");
    source
        .externals
        .insert("UNREPRESENTABLE 日本語".into(), false);
    assert!(
        hlasm::goff_bytes(&source)
            .unwrap_err()
            .contains("representable")
    );
    let mut mode = tagged(31, false);
    mode.object_relocations[0].expression.addend = -1;
    assert!(
        hlasm::goff_bytes(&mode)
            .unwrap_err()
            .contains("wraps its field")
    );
}

#[test]
fn weak_relative_and_mode_sensitive_references_preserve_missing_symbol_behavior() {
    let source = compile("WEAK CSECT\n WXTRN TARGET\n BRASL 14,TARGET+2\n BR 14\n END\n");
    let object = roundtrip(&source);
    assert_eq!(object.externals.get("TARGET"), Some(&true));
    for base in [0, 4096, 2 << 20] {
        let field = object.storage_at(base).unwrap();
        let displacement = i32::from_be_bytes(field[2..6].try_into().unwrap());
        assert_eq!(i64::from(displacement) * 2, 2 - base as i64);
    }
    let mut source = tagged(31, true);
    source.object_relocations[0].addressing_mode = hlasm::ObjectAddressMode::External {
        symbol: "TARGET".into(),
        weak: true,
    };
    source.object_relocations[0].expression.terms[0].weak = true;
    let object = roundtrip(&source);
    assert_eq!(&object.storage_at(4096).unwrap()[8..12], &[0; 4]);
}

#[test]
fn large_signed_relocation_dictionary_crosses_multiple_logical_and_physical_records() {
    let mut source = compile("DICTION CSECT\n BR 14\nPTR DC F'0'\n END\n");
    source.zeus_hlasm = 7;
    source.relocations.push(hlasm::Relocation {
        offset: 4,
        width: 4,
        target: 0,
        symbol: None,
        addend: 0,
        weak: false,
        expression: Some(hlasm::RelocationExpression {
            addend: 3,
            local_coefficient: -1700,
            terms: vec![],
        }),
    });
    let bytes = hlasm::goff_bytes(&source).unwrap();
    assert_eq!(
        bytes
            .as_chunks::<80>()
            .0
            .iter()
            .filter(|b| b[1] == 0x21)
            .count(),
        2
    );
    let object = roundtrip(&source);
    for base in [0, 4096, 2 << 20] {
        assert_eq!(
            &object.storage_at(base).unwrap()[4..8],
            &source.storage_at(base).unwrap()[4..8]
        );
    }
    source.relocations[0]
        .expression
        .as_mut()
        .unwrap()
        .local_coefficient = i64::MIN;
    assert!(
        hlasm::goff_bytes(&source)
            .unwrap_err()
            .contains("size limit")
    );
}

#[test]
fn non_executable_architectural_data_and_class_identity_survive_reemission() {
    let source = compile("DATAOBJ CSECT\n BR 14\n END\n");
    let mut object = roundtrip(&source);
    object.machine_code.clear();
    object.bindings.sections[0].name = "MyData".into();
    object.bindings.sections[0].class = Some("MyData".into());
    let loaded = roundtrip(&object);
    assert!(loaded.machine_code.is_empty());
    assert_eq!(loaded.bindings.sections[0].class.as_deref(), Some("MyData"));
    assert_eq!(loaded.storage, object.storage);
}

#[test]
fn cli_selects_goff_and_links_source_relative_instructions_without_template_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("caller.asm");
    std::fs::write(
        &path,
        "CALLER CSECT\n LR 12,14\n EXTRN TARGET\n BRASL 14,TARGET\n BR 12\n DS 0D\n END\n",
    )
    .unwrap();
    let target = compile("TARGET CSECT\n LA 15,41\n BR 14\n DS 0D\n END\n");
    hlasm::save_module(&target, &dir.path().join("target.zo")).unwrap();
    let zas = env!("CARGO_BIN_EXE_zas");
    let result = std::process::Command::new(zas)
        .args(["compile", "--output-format", "goff"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let input = path.with_extension("goff");
    let output = dir.path().join("bound.goff");
    let result = std::process::Command::new(zas)
        .args(["link", "--output-format", "goff", "-o"])
        .arg(&output)
        .arg(&input)
        .arg(dir.path().join("target.zo"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let mixed = hlasm::load_module(&std::fs::read(&output).unwrap()).unwrap();
    assert!(mixed.bindings.sections.len() > 1);
    assert_eq!(execute(&mixed, 4096).0, 41);
    let target_path = dir.path().join("target.goff");
    std::fs::write(&target_path, hlasm::goff_bytes(&target).unwrap()).unwrap();
    // Contiguous contributions to one GOFF class can be emitted together.
    let result = std::process::Command::new(zas)
        .args(["link", "--output-format", "goff", "-o"])
        .arg(&output)
        .arg(&input)
        .arg(&target_path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        std::process::Command::new(zas)
            .arg("run")
            .arg(&output)
            .status()
            .unwrap()
            .code(),
        Some(41)
    );
}
