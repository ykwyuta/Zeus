use std::path::Path;
use zeus_cobol::hlasm::{self, Module};

fn assemble(dir: &Path, name: &str, body: &str) -> Module {
    let path = dir.join(format!("{name}.asm"));
    std::fs::write(&path, body).unwrap();
    let module = hlasm::compile_file(&path, &[]).unwrap();
    hlasm::save_module(&module, &dir.join(format!("{name}.zo"))).unwrap();
    hlasm::load_module(&hlasm::module_bytes(&module).unwrap()).unwrap()
}

fn definitions(dir: &Path) -> (Module, Module) {
    let alpha = assemble(
        dir,
        "ALPHA",
        "ALPHA CSECT\n         BR 14\n         DS 3H\nOMEGA DC F'17'\n         ENTRY OMEGA\n         END\n",
    );
    let table = assemble(
        dir,
        "TABLE",
        "TABLE CSECT\n         BR 14\n         DC F'41,42,43'\n         END\n",
    );
    (alpha, table)
}

fn words(module: &Module, base: usize, label: &str, count: usize) -> Vec<u32> {
    let storage = module.storage_at(base).unwrap();
    let at = module.entries[label] as usize;
    storage[at..at + count * 4]
        .chunks_exact(4)
        .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
        .collect()
}

#[test]
fn composite_adcons_evaluate_coefficients_equ_aliases_and_negative_differences_at_load_origin() {
    let dir = tempfile::tempdir().unwrap();
    let main = assemble(
        dir.path(),
        "MAIN",
        "MAIN CSECT\n         BR 14\n         EXTRN ALPHA,OMEGA,TABLE\nTARGET EQU TABLE+SPAN\nSPAN EQU OMEGA-ALPHA\nPTR DC A(TARGET,SPAN,-SPAN,TABLE+TABLE,-TABLE,MAIN+TABLE-ALPHA)\n         ENTRY PTR\n         END\n",
    );
    assert_eq!(main.zeus_hlasm, 4);
    assert!(
        main.storage_at(128)
            .unwrap_err()
            .contains("unresolved external symbol")
    );
    let (alpha, table) = definitions(dir.path());
    for modules in [
        vec![main.clone(), alpha.clone(), table.clone()],
        vec![table.clone(), alpha.clone(), main.clone()],
    ] {
        let linked = hlasm::link_modules(&modules).unwrap();
        let linked = hlasm::load_module(&hlasm::module_bytes(&linked).unwrap()).unwrap();
        assert_eq!(linked.zeus_hlasm, 4);
        let prefix = assemble(
            dir.path(),
            "PREFIX",
            "PREFIX CSECT\n         BR 14\n         DS 17H\n         END\n",
        );
        let rebound = hlasm::link_modules(&[prefix, linked.clone()]).unwrap();
        for module in [&linked, &rebound] {
            for base in [128, 1024, 100000] {
                let a = base as i64 + i64::from(module.entries["ALPHA"]);
                let z = base as i64 + i64::from(module.entries["OMEGA"]);
                let t = base as i64 + i64::from(module.entries["TABLE"]);
                let m = base as i64 + i64::from(module.entries["MAIN"]);
                assert_eq!(
                    words(module, base, "PTR", 6),
                    [t + z - a, z - a, a - z, 2 * t, -t, m + t - a].map(|n| n as u32)
                );
            }
        }
    }
}

#[test]
fn composite_adcons_capture_each_duplicated_constant_location_and_literal_instruction_location() {
    let dir = tempfile::tempdir().unwrap();
    let main = assemble(
        dir.path(),
        "MAIN",
        "MAIN CSECT\n         USING MAIN,15\n         L 3,=A(TABLE+*-ALPHA)\n         L 2,0(1)\n         ST 3,0(2)\n         LGHI 15,0\n         BR 14\n         EXTRN TABLE,ALPHA\nPTR DC 2A(TABLE+*-ALPHA,TABLE+*-ALPHA)\n         ENTRY PTR\n         END\n",
    );
    let (alpha, table) = definitions(dir.path());
    let linked = hlasm::link_modules(&[table, main, alpha]).unwrap();
    let mut linked = hlasm::load_module(&hlasm::module_bytes(&linked).unwrap()).unwrap();
    linked.entry = linked.entries["MAIN"];
    for base in [128, 100000] {
        let target =
            (base as i64 + i64::from(linked.entries["TABLE"]) + i64::from(linked.entries["MAIN"])
                - i64::from(linked.entries["ALPHA"])) as u32;
        let mut mem = vec![0; base];
        mem.extend(linked.storage_at(base).unwrap());
        let size = mem.len();
        assert_eq!(
            hlasm::execute(&linked, &mut mem, base, &[Some(0)]).unwrap(),
            0
        );
        assert_eq!(mem.len(), size);
        assert_eq!(u32::from_be_bytes(mem[..4].try_into().unwrap()), target);
        let ptr_target =
            (base as i64 + i64::from(linked.entries["TABLE"]) + i64::from(linked.entries["PTR"])
                - i64::from(linked.entries["ALPHA"])) as u32;
        assert_eq!(
            words(&linked, base, "PTR", 4),
            (0..4)
                .map(|n| ptr_target.wrapping_add(4 * n))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn composite_data_terms_pull_libraries_without_external_declaration_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let mut main = assemble(
        dir.path(),
        "MAIN",
        "MAIN CSECT\n         BR 14\n         EXTRN TABLE,ALPHA,OMEGA\nPTR DC A(TABLE+OMEGA-ALPHA)\n         ENTRY PTR\n         END\n",
    );
    main.externals.clear();
    definitions(dir.path());
    let linked = hlasm::link_with_libraries(vec![main], &[dir.path().to_path_buf()]).unwrap();
    assert!(linked.entries.contains_key("TABLE"));
    assert!(linked.entries.contains_key("ALPHA"));
    assert_eq!(
        words(&linked, 128, "PTR", 1),
        vec![128 + linked.entries["TABLE"] + 8]
    );
}

#[test]
fn composite_weak_adcons_keep_other_terms_and_bind_after_another_link() {
    let dir = tempfile::tempdir().unwrap();
    let main = assemble(
        dir.path(),
        "MAIN",
        "MAIN CSECT\n         BR 14\n         EXTRN TABLE\n         WXTRN OPTIONAL\nPTR DC A(TABLE+MAIN-OPTIONAL+4)\n         ENTRY PTR\n         END\n",
    );
    definitions(dir.path());
    let optional = assemble(
        dir.path(),
        "OPTIONAL",
        "OPTIONAL CSECT\n         BR 14\n         END\n",
    );
    let linked = hlasm::link_with_libraries(vec![main], &[dir.path().to_path_buf()]).unwrap();
    assert!(!linked.entries.contains_key("OPTIONAL"));
    let prefix = assemble(
        dir.path(),
        "PREFIX",
        "PREFIX CSECT\n         BR 14\n         END\n",
    );
    let pending = hlasm::link_modules(&[prefix, linked]).unwrap();
    let pending = hlasm::load_module(&hlasm::module_bytes(&pending).unwrap()).unwrap();
    for base in [128, 100000] {
        assert_eq!(
            words(&pending, base, "PTR", 1),
            vec![2 * base as u32 + pending.entries["MAIN"] + pending.entries["TABLE"] + 4]
        );
    }
    let resolved = hlasm::link_modules(&[optional, pending]).unwrap();
    for base in [128, 100000] {
        assert_eq!(
            words(&resolved, base, "PTR", 1),
            vec![
                base as u32 + resolved.entries["MAIN"] + resolved.entries["TABLE"]
                    - resolved.entries["OPTIONAL"]
                    + 4
            ]
        );
    }
}

#[test]
fn fullword_adcons_support_signed_absolute_values_and_low_word_composite_results() {
    let dir = tempfile::tempdir().unwrap();
    let main = assemble(
        dir.path(),
        "MAIN",
        "MAIN CSECT\n         BR 14\nPTR DC A(-1,-2147483648,4294967295,MAIN+MAIN+4294967295)\n         ENTRY PTR\n         END\n",
    );
    assert_eq!(
        words(&main, 128, "PTR", 4),
        vec![u32::MAX, 0x80000000, u32::MAX, 255]
    );
}

#[test]
fn composite_adcons_require_format_four_and_reject_bad_metadata_and_unsupported_operands() {
    let dir = tempfile::tempdir().unwrap();
    let main = assemble(
        dir.path(),
        "MAIN",
        "MAIN CSECT\n         BR 14\n         EXTRN ALPHA,TABLE\nPTR DC A(ALPHA+TABLE)\n         ENTRY PTR\n         END\n",
    );
    for version in 1..=3 {
        let mut old = main.clone();
        old.zeus_hlasm = version;
        assert!(
            hlasm::load_module(&hlasm::module_bytes(&old).unwrap())
                .unwrap_err()
                .contains("version 4")
        );
    }
    assert!(
        hlasm::link_modules(std::slice::from_ref(&main))
            .unwrap_err()
            .contains("unresolved external symbol")
    );
    let (alpha, table) = definitions(dir.path());
    let mut bad = main.clone();
    bad.relocations[0].offset = u32::MAX;
    assert!(hlasm::link_modules(&[bad, alpha.clone(), table.clone()]).is_err());
    let mut bad = main.clone();
    bad.relocations[0].symbol = Some("TABLE".into());
    assert!(
        hlasm::link_modules(&[bad, alpha.clone(), table.clone()])
            .unwrap_err()
            .contains("conflicting")
    );
    let mut bad = main.clone();
    bad.relocations[0].expression.as_mut().unwrap().addend = i64::MAX;
    assert!(
        hlasm::link_modules(&[bad, alpha.clone(), table.clone()])
            .unwrap_err()
            .contains("overflow")
    );
    let mut bad = main;
    bad.relocations[0]
        .expression
        .as_mut()
        .unwrap()
        .local_coefficient = i64::MAX;
    assert!(bad.storage_at(128).is_err());
    for operand in ["ALPHA*TABLE", "DUMMY+TABLE", "DUMMY+DUMMY", "=F'1'"] {
        let path = dir.path().join("bad.asm");
        std::fs::write(&path, format!("MAIN CSECT\n         EXTRN ALPHA,TABLE\n         BR 14\n         DC A({operand})\nMAP DSECT\nDUMMY DS F\n         END MAIN\n")).unwrap();
        assert!(hlasm::compile_file(&path, &[]).is_err(), "{operand}");
    }
}
