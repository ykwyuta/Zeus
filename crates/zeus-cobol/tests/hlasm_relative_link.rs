use std::path::Path;
use zeus_cobol::{hlasm, vm::Machine};

fn assemble(dir: &Path, name: &str, source: &str) -> hlasm::Module {
    let path = dir.join(format!("{name}.asm"));
    std::fs::write(&path, source).unwrap();
    let module = hlasm::compile_file(&path, &[]).unwrap();
    hlasm::save_module(&module, &dir.join(format!("{name}.zo"))).unwrap();
    hlasm::load_module(&hlasm::module_bytes(&module).unwrap()).unwrap()
}

const MAIN: &str = "MAIN CSECT\n         LR 9,14\n         LARL 15,SUB\n         BRASL 14,SUB\n         LR 14,9\n         BR 14\n         EXTRN SUB\n         END\n";
const SUB: &str = "SUB CSECT\n         LR 8,14\n         LARL 15,LEAF\n         BRASL 14,LEAF\n         LR 14,8\n         BR 14\n         EXTRN LEAF\n         END\n";
const LEAF: &str = "LEAF CSECT\n         L 2,0(1)\n         L 3,0(2)\n         AHI 3,7\n         ST 3,0(2)\n         LGHI 15,42\n         BR 14\n         END\n";

fn execute(module: &hlasm::Module, base: usize, initial: i32, rc: i32) -> Vec<u8> {
    let mut mem = vec![0; base];
    mem[..4].copy_from_slice(&initial.to_be_bytes());
    mem.extend(module.storage_at(base).unwrap());
    let length = mem.len();
    assert_eq!(
        hlasm::execute(module, &mut mem, base, &[Some(0)]).unwrap(),
        rc
    );
    assert_eq!(mem.len(), length);
    mem
}

#[test]
fn external_relative_calls_resolve_transitive_libraries_and_share_original_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let main = assemble(dir.path(), "MAIN", MAIN);
    assert_eq!(main.zeus_hlasm, 2);
    assemble(dir.path(), "SUB", SUB);
    assemble(dir.path(), "LEAF", LEAF);
    assert!(
        main.storage_at(128)
            .unwrap_err()
            .contains("unresolved external symbol SUB")
    );
    let linked = hlasm::link_with_libraries(vec![main], &[dir.path().to_path_buf()]).unwrap();
    assert!(linked.code.iter().all(|i| i.relative_external.is_none()));
    assert_eq!(linked.zeus_hlasm, 1);
    for base in [128, 1024] {
        assert_eq!(&execute(&linked, base, 5, 42)[..4], &12i32.to_be_bytes());
    }
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    let addr = machine.getmain(128, 0);
    machine.storage_mut()[addr..addr + 4].copy_from_slice(&5i32.to_be_bytes());
    assert_eq!(
        machine.call_hlasm("MAIN", &[Some(addr)]).unwrap().unwrap(),
        42
    );
    let retained = machine.storage().len();
    assert_eq!(
        machine.call_hlasm("MAIN", &[Some(addr)]).unwrap().unwrap(),
        42
    );
    assert_eq!(machine.storage().len(), retained);
    assert_eq!(&machine.storage()[addr..addr + 4], &19i32.to_be_bytes());
}

#[test]
fn external_data_addends_and_equ_aliases_resolve_at_final_instruction_locations() {
    let dir = tempfile::tempdir().unwrap();
    let data = assemble(
        dir.path(),
        "DATA",
        "DATA CSECT\n         BR 14\nVALUES DC F'10,20,30'\n         ENTRY VALUES\n         END\n",
    );
    for expression in ["VALUES+4", "NEXT-4", "VALUES+(AFTER-MAIN)"] {
        let source = format!(
            "MAIN CSECT\n         EXTRN VALUES\nNEXT EQU VALUES+8\n         L 2,0(1)\nAFTER LARL 3,{expression}\n         L 4,0(3)\n         ST 4,0(2)\n         LGHI 15,0\n         BR 14\n         END\n"
        );
        let main = assemble(dir.path(), "MAIN", &source);
        // Put MAIN after DATA too, requiring a negative relative displacement.
        for reversed in [false, true] {
            let mut linked = if reversed {
                hlasm::link_modules(&[data.clone(), main.clone()])
            } else {
                hlasm::link_modules(&[main.clone(), data.clone()])
            }
            .unwrap();
            linked.entry = linked.entries["MAIN"];
            let linked = hlasm::load_module(&hlasm::module_bytes(&linked).unwrap()).unwrap();
            for base in [128, 1024] {
                assert_eq!(&execute(&linked, base, 0, 0)[..4], &20i32.to_be_bytes());
            }
        }
    }
}

#[test]
fn fully_linked_relative_references_survive_serialization_and_another_link_origin() {
    let dir = tempfile::tempdir().unwrap();
    let main = assemble(dir.path(), "MAIN", MAIN);
    let sub = assemble(dir.path(), "SUB", SUB);
    let leaf = assemble(dir.path(), "LEAF", LEAF);
    let linked = hlasm::link_modules(&[main, sub, leaf]).unwrap();
    let linked = hlasm::load_module(&hlasm::module_bytes(&linked).unwrap()).unwrap();
    let prefix = assemble(
        dir.path(),
        "PREFIX",
        "PREFIX CSECT\n         BR 14\nSPACE DS 1000X\n         END\n",
    );
    let mut linked = hlasm::link_modules(&[prefix, linked]).unwrap();
    linked.entry = linked.entries["MAIN"];
    assert_eq!(&execute(&linked, 512, 5, 42)[..4], &12i32.to_be_bytes());
}

#[test]
fn weak_relative_references_do_not_pull_libraries_and_stay_zero_at_nonzero_load_origins() {
    let dir = tempfile::tempdir().unwrap();
    let source = "MAIN CSECT\n         L 3,0(1)\n         LARL 2,OPTIONAL\n         STG 2,0(3)\n         LGHI 15,0\n         BR 14\n         WXTRN OPTIONAL\n         END\n";
    let main = assemble(dir.path(), "MAIN", source);
    let optional = assemble(
        dir.path(),
        "OPTIONAL",
        "OPTIONAL CSECT\n         BR 14\n         END\n",
    );
    let weak = hlasm::link_with_libraries(vec![main.clone()], &[dir.path().to_path_buf()]).unwrap();
    assert!(!weak.entries.contains_key("OPTIONAL"));
    assert!(
        weak.code
            .iter()
            .any(|i| i.relative_external.as_ref().is_some_and(|r| r.weak))
    );
    let weak = hlasm::load_module(&hlasm::module_bytes(&weak).unwrap()).unwrap();
    assert_eq!(weak.zeus_hlasm, 2);
    for base in [128, 100_000] {
        assert_eq!(&execute(&weak, base, 0, 0)[..8], &[0; 8]);
    }
    // A later explicit definition resolves the weak reference retained in the object.
    let resolved = hlasm::link_modules(&[weak, optional]).unwrap();
    assert_eq!(resolved.zeus_hlasm, 1);
    let target = 128 + u64::from(resolved.entries["OPTIONAL"]);
    assert_eq!(&execute(&resolved, 128, 0, 0)[..8], &target.to_be_bytes());
    let short = assemble(
        dir.path(),
        "SHORT",
        "SHORT CSECT\n         BRC 0,OPTIONAL\n         LGHI 15,0\n         BR 14\n         WXTRN OPTIONAL\n         END\n",
    );
    let short = hlasm::link_modules(&[short]).unwrap();
    assert!(
        short
            .storage_at(100_000)
            .unwrap_err()
            .contains("halfword displacement out of range")
    );
}

#[test]
fn missing_symbols_short_overflow_odd_targets_and_bad_metadata_fail_before_execution() {
    let dir = tempfile::tempdir().unwrap();
    let missing = assemble(dir.path(), "MAIN", MAIN);
    assert!(
        hlasm::link_modules(std::slice::from_ref(&missing))
            .unwrap_err()
            .contains("unresolved external symbol SUB")
    );
    let mut mem = vec![0; 128];
    mem.extend(&missing.storage);
    let before = mem.clone();
    assert!(
        hlasm::execute(&missing, &mut mem, 128, &[Some(0)])
            .unwrap_err()
            .contains("link the module")
    );
    assert_eq!(mem, before);
    let sub = assemble(
        dir.path(),
        "SUB",
        "SUB CSECT\n         LGHI 15,0\n         BR 14\n         END\n",
    );
    for op in ["BRAS", "BRCT", "BRC"] {
        let root = assemble(
            dir.path(),
            "MAIN",
            &format!(
                "MAIN CSECT\n         {op} 14,SUB\n         BR 14\nGAP DS 70000X\n         EXTRN SUB\n         END\n"
            ),
        );
        let error = hlasm::link_modules(&[root, sub.clone()]).unwrap_err();
        assert!(
            error.contains("halfword displacement out of range") && error.contains("external SUB"),
            "{error}"
        );
    }
    let odd = assemble(
        dir.path(),
        "MAIN",
        "MAIN CSECT\n         LARL 2,SUB+1\n         BR 14\n         EXTRN SUB\n         END\n",
    );
    assert!(
        hlasm::link_modules(&[odd, sub.clone()])
            .unwrap_err()
            .contains("halfword aligned")
    );
    for malformed in 0..3 {
        let mut root = assemble(
            dir.path(),
            "MAIN",
            "MAIN CSECT\n         BRASL 14,SUB\n         BR 14\n         EXTRN SUB\n         END\n",
        );
        let ins = root
            .code
            .iter_mut()
            .find(|i| i.relative_external.is_some())
            .unwrap();
        match malformed {
            0 => ins.relative_external.as_mut().unwrap().addend = i64::MAX,
            1 => ins.address.as_mut().unwrap().base = 1,
            _ => ins.op = "BR".into(),
        }
        assert!(hlasm::link_modules(&[root, sub.clone()]).is_err());
    }
}

#[test]
fn short_external_displacement_boundaries_are_checked_at_bind_time() {
    let dir = tempfile::tempdir().unwrap();
    let root = assemble(
        dir.path(),
        "MAIN",
        "MAIN CSECT\n         BRAS 14,SUB\n         BR 14\n         EXTRN SUB\n         END\n",
    );
    let sub = assemble(
        dir.path(),
        "SUB",
        "SUB CSECT\n         BR 14\n         END\n",
    );
    let origin = root.storage.len().next_multiple_of(8) as i64;
    for (distance, valid) in [
        (-65536, true),
        (65534, true),
        (-65538, false),
        (65536, false),
    ] {
        let mut root = root.clone();
        root.code[0].relative_external.as_mut().unwrap().addend = distance - origin;
        assert_eq!(
            hlasm::link_modules(&[root, sub.clone()]).is_ok(),
            valid,
            "{distance}"
        );
    }
}

#[test]
fn zas_link_and_run_resolve_external_relative_calls_and_do_not_write_failed_output() {
    let dir = tempfile::tempdir().unwrap();
    let main = assemble(
        dir.path(),
        "MAIN",
        "MAIN CSECT\n         LR 9,14\n         BRASL 14,SUB\n         LR 14,9\n         BR 14\n         EXTRN SUB\n         END\n",
    );
    let output = dir.path().join("linked.zo");
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .args(["link", "-L"])
        .arg(dir.path())
        .arg("-o")
        .arg(&output)
        .arg(dir.path().join("MAIN.zo"))
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(12));
    assert!(!output.exists());
    assemble(
        dir.path(),
        "SUB",
        "SUB CSECT\n         LGHI 15,42\n         BR 14\n         END\n",
    );
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .args(["link", "-L"])
        .arg(dir.path())
        .arg("-o")
        .arg(&output)
        .arg(dir.path().join("MAIN.zo"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .arg("run")
        .arg(&output)
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(42),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(main.code.iter().any(|i| i.relative_external.is_some()));
}

#[test]
fn old_objects_remain_readable_and_unresolved_instruction_fixups_require_format_two() {
    let dir = tempfile::tempdir().unwrap();
    let old = assemble(
        dir.path(),
        "OLD",
        "OLD CSECT\n         LGHI 15,0\n         BR 14\n         END\n",
    );
    let bytes = hlasm::module_bytes(&old).unwrap();
    assert_eq!(old.zeus_hlasm, 1);
    assert!(!String::from_utf8_lossy(&bytes).contains("relative_external"));
    let old = hlasm::load_module(&bytes).unwrap();
    execute(&old, 128, 0, 0);
    let mut pending = assemble(dir.path(), "MAIN", MAIN);
    assert_eq!(pending.zeus_hlasm, 2);
    pending.zeus_hlasm = 1;
    assert!(
        hlasm::load_module(&hlasm::module_bytes(&pending).unwrap())
            .unwrap_err()
            .contains("requires HLASM module version 2")
    );
    pending.zeus_hlasm = 13;
    assert!(
        hlasm::load_module(&hlasm::module_bytes(&pending).unwrap())
            .unwrap_err()
            .contains("unsupported HLASM module version")
    );
}

fn address_main(dir: &Path, expression: &str) -> hlasm::Module {
    assemble(
        dir,
        "MAIN",
        &format!(
            "MAIN CSECT\n         EXTRN FIRST,LAST,TABLE\n         LARL 3,{expression}\n         L 2,0(1)\n         ST 3,0(2)\n         LGHI 15,0\n         BR 14\n         END\n"
        ),
    )
}

fn composite_definitions(dir: &Path) -> (hlasm::Module, hlasm::Module) {
    let first = assemble(
        dir,
        "FIRST",
        "FIRST CSECT\n         BR 14\n         DS 3H\nLAST DC F'99'\n         ENTRY LAST\n         END\n",
    );
    let table = assemble(
        dir,
        "TABLE",
        "TABLE CSECT\n         BR 14\n         DC F'10,20,30'\n         END\n",
    );
    (first, table)
}

#[test]
fn composite_external_targets_resolve_libraries_coefficients_local_terms_and_link_origins() {
    let dir = tempfile::tempdir().unwrap();
    let (first, table) = composite_definitions(dir.path());
    for expression in [
        "LAST-FIRST+TABLE",
        "TABLE+TABLE-MAIN",
        "MAIN+TABLE-FIRST",
        "TABLE-(LAST-FIRST)+16",
    ] {
        let main = address_main(dir.path(), expression);
        assert_eq!(main.zeus_hlasm, 3);
        assert!(
            main.storage_at(128)
                .unwrap_err()
                .contains("unresolved external symbol")
        );
        for reversed in [false, true] {
            let mut linked = if reversed {
                hlasm::link_modules(&[first.clone(), table.clone(), main.clone()]).unwrap()
            } else {
                hlasm::link_with_libraries(vec![main.clone()], &[dir.path().to_path_buf()]).unwrap()
            };
            linked.entry = linked.entries["MAIN"];
            assert_eq!(linked.zeus_hlasm, 1);
            // Re-link a fully resolved image at another origin as well.
            let prefix = assemble(
                dir.path(),
                "PREFIX",
                "PREFIX CSECT\n         BR 14\n         DS 13H\n         END\n",
            );
            let mut rebound = hlasm::link_modules(&[prefix, linked.clone()]).unwrap();
            rebound.entry = rebound.entries["MAIN"];
            for module in [&linked, &rebound] {
                let a = i64::from(module.entries["FIRST"]);
                let b = i64::from(module.entries["LAST"]);
                let t = i64::from(module.entries["TABLE"]);
                let m = i64::from(module.entries["MAIN"]);
                let target = match expression {
                    "LAST-FIRST+TABLE" => b - a + t,
                    "TABLE+TABLE-MAIN" => 2 * t - m,
                    "MAIN+TABLE-FIRST" => m + t - a,
                    _ => t - (b - a) + 16,
                };
                for base in [128, 1024, 100000] {
                    let mem = execute(module, base, 0, 0);
                    assert_eq!(
                        u32::from_be_bytes(mem[..4].try_into().unwrap()),
                        (base as i64 + target) as u32,
                        "{expression}"
                    );
                }
            }
        }
    }
}

#[test]
fn composite_weak_terms_keep_other_terms_and_remain_relocatable_when_relinked() {
    let dir = tempfile::tempdir().unwrap();
    let main = assemble(
        dir.path(),
        "MAIN",
        "MAIN CSECT\n         EXTRN FIRST\n         WXTRN OPTIONAL\n         LARL 3,MAIN+FIRST-OPTIONAL+4\n         L 2,0(1)\n         ST 3,0(2)\n         LGHI 15,0\n         BR 14\n         END\n",
    );
    assemble(
        dir.path(),
        "FIRST",
        "FIRST CSECT\n         BR 14\n         END\n",
    );
    let optional = assemble(
        dir.path(),
        "OPTIONAL",
        "OPTIONAL CSECT\n         BR 14\n         END\n",
    );
    // OPTIONAL exists in the library but must not be pulled by a weak term.
    let pending = hlasm::link_with_libraries(vec![main], &[dir.path().to_path_buf()]).unwrap();
    assert_eq!(pending.zeus_hlasm, 3);
    assert!(!pending.entries.contains_key("OPTIONAL"));
    let prefix = assemble(
        dir.path(),
        "PREFIX",
        "PREFIX CSECT\n         BR 14\n         DS 13H\n         END\n",
    );
    let mut rebound = hlasm::link_modules(&[prefix, pending.clone()]).unwrap();
    rebound.entry = rebound.entries["MAIN"];
    for module in [&pending, &rebound] {
        let module = hlasm::load_module(&hlasm::module_bytes(module).unwrap()).unwrap();
        for base in [128, 100000] {
            let mem = execute(&module, base, 0, 0);
            let target = 2 * base as u32 + module.entries["MAIN"] + module.entries["FIRST"] + 4;
            assert_eq!(u32::from_be_bytes(mem[..4].try_into().unwrap()), target);
        }
    }
    let mut resolved = hlasm::link_modules(&[optional, rebound]).unwrap();
    resolved.entry = resolved.entries["MAIN"];
    assert_eq!(resolved.zeus_hlasm, 1);
    for base in [128, 100000] {
        let mem = execute(&resolved, base, 0, 0);
        let target = base as u32 + resolved.entries["MAIN"] + resolved.entries["FIRST"]
            - resolved.entries["OPTIONAL"]
            + 4;
        assert_eq!(u32::from_be_bytes(mem[..4].try_into().unwrap()), target);
    }
}

#[test]
fn composite_relocations_validate_versions_sections_ranges_and_malformed_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let (first, table) = composite_definitions(dir.path());
    let main = address_main(dir.path(), "LAST-FIRST+TABLE");
    for version in [1, 2] {
        let mut old = main.clone();
        old.zeus_hlasm = version;
        assert!(
            hlasm::load_module(&hlasm::module_bytes(&old).unwrap())
                .unwrap_err()
                .contains("version 3")
        );
    }
    for operand in [
        "FIRST+TABLE",
        "FIRST-LAST",
        "FIRST*TABLE",
        "DUMMY+FIRST-LAST",
    ] {
        let path = dir.path().join("bad.asm");
        std::fs::write(&path, format!("MAIN CSECT\n         EXTRN FIRST,LAST,TABLE\n         LARL 3,{operand}\n         BR 14\nMAP DSECT\nDUMMY DS F\n         END MAIN\n")).unwrap();
        assert!(hlasm::compile_file(&path, &[]).is_err(), "{operand}");
    }
    for operand in ["LAST-FIRST+TABLE+1", "LAST-FIRST+TABLE+5000000000"] {
        let bad = address_main(dir.path(), operand);
        assert!(hlasm::link_modules(&[bad, first.clone(), table.clone()]).is_err());
    }
    let mut bad = main.clone();
    bad.code[0].relative_expression.as_mut().unwrap().addend = i64::MAX;
    assert!(
        hlasm::link_modules(&[bad, first.clone(), table.clone()])
            .unwrap_err()
            .contains("overflow")
    );
    let mut bad = main;
    bad.code[0]
        .relative_expression
        .as_mut()
        .unwrap()
        .local_coefficient = i64::MAX;
    assert!(
        hlasm::link_modules(&[bad, first, table])
            .unwrap_err()
            .contains("overflow")
    );
}

#[test]
fn complex_equ_chains_substitute_external_and_local_terms_before_relative_linking() {
    let dir = tempfile::tempdir().unwrap();
    let (first, table) = composite_definitions(dir.path());
    let definitions = [
        // A forward chain with a net-zero intermediate expression.
        "TARGET EQU TABLE+SPAN\nSPAN EQU LAST-FIRST\n",
        // A net-two intermediate expression becomes a relative target.
        "PAIR EQU TABLE+TABLE\nTARGET EQU PAIR-MAIN\n",
        // Capture '*' at the EQU statement, rather than at the use site.
        "HERE EQU *\nPAIR EQU HERE+TABLE\nTARGET EQU PAIR-FIRST\n",
        // An unresolved chain before code resolves using final local offsets.
        "TARGET EQU MIX-FIRST\nMIX EQU TABLE+AFTER\n",
        // The definition itself is after the use and has a forward dependency.
        "TARGET EQU HERE\n",
    ];
    for (case, definitions) in definitions.iter().enumerate() {
        let deferred = if case == 4 {
            "HERE EQU TABLE+*+AFTER-FIRST-AFTER\n"
        } else {
            ""
        };
        let main = assemble(
            dir.path(),
            "MAIN",
            &format!(
                "MAIN CSECT\n         EXTRN FIRST,LAST,TABLE\n{definitions}         LARL 3,TARGET\n{deferred}         L 2,0(1)\n         ST 3,0(2)\nAFTER LGHI 15,0\n         BR 14\n         END\n"
            ),
        );
        assert_eq!(main.zeus_hlasm, 3);
        for modules in [
            vec![main.clone(), first.clone(), table.clone()],
            vec![table.clone(), first.clone(), main.clone()],
        ] {
            let mut linked = hlasm::link_modules(&modules).unwrap();
            linked.entry = linked.entries["MAIN"];
            let m = i64::from(linked.entries["MAIN"]);
            let t = i64::from(linked.entries["TABLE"]);
            let a = i64::from(linked.entries["FIRST"]);
            let b = i64::from(linked.entries["LAST"]);
            let target = match case {
                0 => t + b - a,
                1 => 2 * t - m,
                2 => m + t - a,
                3 => t + m + 14 - a,
                _ => t + m + 6 - a,
            };
            for base in [128, 100000] {
                let mem = execute(&linked, base, 0, 0);
                assert_eq!(
                    u32::from_be_bytes(mem[..4].try_into().unwrap()),
                    (base as i64 + target) as u32 & 0x7fff_ffff,
                    "case {case}"
                );
            }
        }
    }
}

#[test]
fn complex_equ_reduction_recovers_absolute_registers_immediates_and_simple_adcons() {
    let dir = tempfile::tempdir().unwrap();
    let (first, table) = composite_definitions(dir.path());
    let main = assemble(
        dir.path(),
        "MAIN",
        "MAIN CSECT\n         EXTRN FIRST,TABLE\nPAIR EQU FIRST+TABLE\nSIMPLE EQU PAIR-FIRST\nREGNO EQU PAIR-PAIR+3\nVALUE EQU PAIR-PAIR+27\n         L 2,0(1)\n         LGHI REGNO,VALUE\n         ST 3,0(2)\n         LGHI 15,L'PAIR\n         BR 14\nPTR DC A(SIMPLE)\n         ENTRY PTR\n         END\n",
    );
    assert_eq!(main.zeus_hlasm, 1);
    let linked = hlasm::link_modules(&[main, first, table]).unwrap();
    for base in [128, 1024] {
        // An EQU inherits the leftmost term's length; EXTRN symbols have L'=1.
        let mem = execute(&linked, base, 0, 1);
        assert_eq!(&mem[..4], &27i32.to_be_bytes());
        let ptr = base + linked.entries["PTR"] as usize;
        assert_eq!(
            u32::from_be_bytes(mem[ptr..ptr + 4].try_into().unwrap()),
            base as u32 + linked.entries["TABLE"]
        );
    }
}

#[test]
fn complex_equ_weak_aliases_keep_their_terms_and_late_definitions() {
    let dir = tempfile::tempdir().unwrap();
    let main = assemble(
        dir.path(),
        "MAIN",
        "MAIN CSECT\n         WXTRN OPTIONAL\nTARGET EQU PAIR-OPTIONAL+4\nPAIR EQU MAIN+MAIN\n         LARL 3,TARGET\n         L 2,0(1)\n         ST 3,0(2)\n         LGHI 15,0\n         BR 14\n         END\n",
    );
    let prefix = assemble(
        dir.path(),
        "PREFIX",
        "PREFIX CSECT\n         BR 14\n         END\n",
    );
    let mut pending = hlasm::link_modules(&[prefix, main]).unwrap();
    pending.entry = pending.entries["MAIN"];
    for base in [128, 100000] {
        let mem = execute(&pending, base, 0, 0);
        assert_eq!(
            u32::from_be_bytes(mem[..4].try_into().unwrap()),
            2 * (base as u32 + pending.entries["MAIN"]) + 4
        );
    }
    let optional = assemble(
        dir.path(),
        "OPTIONAL",
        "OPTIONAL CSECT\n         BR 14\n         END\n",
    );
    let linked = hlasm::link_modules(&[pending, optional]).unwrap();
    for base in [128, 100000] {
        let mem = execute(&linked, base, 0, 0);
        assert_eq!(
            u32::from_be_bytes(mem[..4].try_into().unwrap()),
            base as u32 + 2 * linked.entries["MAIN"] - linked.entries["OPTIONAL"] + 4
        );
    }
}

#[test]
fn complex_equ_invalid_uses_cycles_and_overflow_are_diagnosed() {
    let dir = tempfile::tempdir().unwrap();
    for body in [
        "PAIR EQU FIRST+TABLE\n         LR PAIR,1\n",
        "PAIR EQU FIRST+TABLE\n         LGHI 15,PAIR\n",
        "PAIR EQU FIRST+TABLE\n         USING PAIR,12\n",
        "PAIR EQU FIRST+TABLE\n         DC V(PAIR)\n",
        "PAIR EQU FIRST+TABLE\n         LARL 3,PAIR\n",
        "PAIR EQU NEXT+FIRST\nNEXT EQU PAIR-TABLE\n",
        "PAIR EQU FIRST+TABLE\nPAIR EQU FIRST-TABLE\n",
        "PAIR EQU FIRST+TABLE\nNEXT EQU PAIR*2\n",
    ] {
        let path = dir.path().join("bad-equ.asm");
        std::fs::write(
            &path,
            format!("MAIN CSECT\n         EXTRN FIRST,TABLE\n{body}         BR 14\n         END\n"),
        )
        .unwrap();
        assert!(hlasm::compile_file(&path, &[]).is_err(), "{body}");
    }
    // Doubling coefficients through aliases must report overflow, not panic.
    let mut body = String::from("MAIN CSECT\n         EXTRN FIRST,TABLE\nX0 EQU FIRST+TABLE\n");
    for n in 1..=63 {
        body.push_str(&format!("X{n} EQU X{}+X{}\n", n - 1, n - 1));
    }
    body.push_str("         BR 14\n         END\n");
    let path = dir.path().join("overflow-equ.asm");
    std::fs::write(&path, body).unwrap();
    let error = hlasm::compile_file(&path, &[]).unwrap_err();
    assert!(error.contains("cannot resolve EQU X63"), "{error}");
    assert!(error.contains("overflow"), "{error}");
}
