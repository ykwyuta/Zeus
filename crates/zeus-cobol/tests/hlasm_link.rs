use std::path::Path;
use zeus_cobol::{hlasm, vm::Machine};

fn assemble(dir: &Path, name: &str, source: &str) -> hlasm::Module {
    let path = dir.join(format!("{name}.asm"));
    std::fs::write(&path, source).unwrap();
    hlasm::compile_file(&path, &[]).unwrap()
}

const MAIN: &str = "MAIN CSECT
         USING *,15
         LR 12,15
         USING MAIN,12
         DROP 15
         LR 10,14
         L 15,=V(SUB)
         BASR 14,15
         LR 14,10
         BR 14
         EXTRN SUB
         END MAIN
";

const SUB: &str = "SUB CSECT
         L 2,0(1)
         L 3,0(2)
         AHI 3,7
         ST 3,0(2)
         LA 15,9
         BR 14
         END
";

#[test]
fn linked_subprogram_keeps_original_argument_addresses() {
    let dir = tempfile::tempdir().unwrap();
    let main = assemble(dir.path(), "MAIN", MAIN);
    let sub = assemble(dir.path(), "SUB", SUB);
    assert!(
        main.storage_at(128)
            .unwrap_err()
            .contains("unresolved external symbol SUB")
    );
    let linked = hlasm::link_modules(&[main, sub]).unwrap();
    let linked = hlasm::load_module(&hlasm::module_bytes(&linked).unwrap()).unwrap();
    let mut mem = vec![0; 128];
    mem[..4].copy_from_slice(&5i32.to_be_bytes());
    mem.extend(linked.storage_at(128).unwrap());
    assert_eq!(
        hlasm::execute(&linked, &mut mem, 128, &[Some(0)]).unwrap(),
        9
    );
    assert_eq!(&mem[..4], &12i32.to_be_bytes());
    let retained = mem.len();
    assert_eq!(
        hlasm::execute(&linked, &mut mem, 128, &[Some(0)]).unwrap(),
        9
    );
    assert_eq!(&mem[..4], &19i32.to_be_bytes());
    assert_eq!(mem.len(), retained);
}

#[test]
fn libraries_are_included_transitively_and_loader_uses_same_linker() {
    let dir = tempfile::tempdir().unwrap();
    let main = assemble(dir.path(), "MAIN", MAIN);
    let sub = assemble(
        dir.path(),
        "SUB",
        "SUB CSECT\n         USING *,15\n         LR 9,14\n         L 15,=V(LEAF)\n         BASR 14,15\n         LR 14,9\n         BR 14\n         END\n",
    );
    let leaf = assemble(
        dir.path(),
        "LEAF",
        "LEAF CSECT\n         LA 15,42\n         BR 14\n         END\n",
    );
    for m in [&main, &sub, &leaf] {
        hlasm::save_module(m, &dir.path().join(format!("{}.zo", m.name))).unwrap();
    }
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    machine.getmain(128, 0);
    assert_eq!(machine.call_hlasm("MAIN", &[]).unwrap().unwrap(), 42);
    let retained = machine.storage().len();
    assert_eq!(machine.call_hlasm("LEAF", &[]).unwrap().unwrap(), 42);
    assert_eq!(machine.storage().len(), retained);
    let output = dir.path().join("linked.zo");
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
        .arg(output)
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(42),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn weak_references_do_not_pull_libraries_but_resolve_included_definitions() {
    let dir = tempfile::tempdir().unwrap();
    let main = assemble(
        dir.path(),
        "WEAK",
        "WEAK CSECT\n         USING *,15\n         L 15,PTR\n         BR 14\nPTR DC V(OPTIONAL)\n         WXTRN OPTIONAL\n         END\n",
    );
    let optional = assemble(
        dir.path(),
        "OPTIONAL",
        "OPTIONAL CSECT\n         LA 15,99\n         BR 14\n         END\n",
    );
    hlasm::save_module(&optional, &dir.path().join("OPTIONAL.zo")).unwrap();
    let weak = hlasm::link_with_libraries(vec![main.clone()], &[dir.path().to_path_buf()]).unwrap();
    assert!(!weak.entries.contains_key("OPTIONAL"));
    let mut mem = weak.storage_at(0).unwrap();
    assert_eq!(hlasm::execute(&weak, &mut mem, 0, &[]).unwrap(), 0);
    let linked = hlasm::link_modules(&[main, optional]).unwrap();
    let origin = 128;
    let mut mem = vec![0; origin];
    mem.extend(linked.storage_at(origin).unwrap());
    assert_eq!(
        hlasm::execute(&linked, &mut mem, origin, &[]).unwrap(),
        (origin + linked.entries["OPTIONAL"] as usize) as i32
    );
}

#[test]
fn external_a_addends_and_data_entries_are_relocated() {
    let dir = tempfile::tempdir().unwrap();
    let main = assemble(
        dir.path(),
        "READ",
        "READ CSECT\n         USING *,15\n         L 2,PTR\n         L 15,4(2)\n         BR 14\nPTR DC A(DATA-4)\n         EXTRN DATA\n         END\n",
    );
    let data = assemble(
        dir.path(),
        "DEFS",
        "DEFS CSECT\n         BR 14\nDATA DC F'1234'\n         ENTRY DATA\n         END\n",
    );
    let linked = hlasm::link_modules(&[main, data]).unwrap();
    let mut mem = vec![0; 256];
    mem.extend(linked.storage_at(256).unwrap());
    assert_eq!(hlasm::execute(&linked, &mut mem, 256, &[]).unwrap(), 1234);
}

#[test]
fn unresolved_and_duplicate_symbols_fail_without_writing_output() {
    let dir = tempfile::tempdir().unwrap();
    let main = assemble(dir.path(), "MAIN", MAIN);
    assert!(
        hlasm::link_modules(std::slice::from_ref(&main))
            .unwrap_err()
            .contains("unresolved external symbol SUB")
    );
    let sub = assemble(dir.path(), "SUB", SUB);
    assert!(
        hlasm::link_modules(&[main, sub.clone(), sub])
            .unwrap_err()
            .contains("duplicate external definition SUB")
    );
    let input = dir.path().join("MAIN.zo");
    let output = dir.path().join("out.zo");
    let main = assemble(dir.path(), "MAIN", MAIN);
    hlasm::save_module(&main, &input).unwrap();
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .args(["link", "-o"])
        .arg(&output)
        .arg(input)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(12));
    assert!(!output.exists());
}

#[test]
fn end_selects_main_entry_without_changing_control_section_export() {
    let dir = tempfile::tempdir().unwrap();
    let main = assemble(
        dir.path(),
        "STARTUP",
        "STARTUP CSECT\nHEADER DC F'99'\nGO LA 15,42\n         BR 14\n         END GO\n",
    );
    assert_eq!(main.entries["STARTUP"], 0);
    assert_eq!(main.entry, 4);
    hlasm::save_module(&main, &dir.path().join("STARTUP.zo")).unwrap();
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    assert_eq!(machine.call_hlasm("STARTUP", &[]).unwrap().unwrap(), 42);
}

#[test]
fn new_linked_images_do_not_replace_previously_loaded_static_instances() {
    let dir = tempfile::tempdir().unwrap();
    let main = assemble(dir.path(), "MAIN", MAIN);
    let sub = assemble(
        dir.path(),
        "SUB",
        "SUB CSECT\n         USING *,15\n         L 2,COUNT\n         AHI 2,1\n         ST 2,COUNT\n         LR 15,2\n         BR 14\nCOUNT DC F'0'\n         END\n",
    );
    for module in [&main, &sub] {
        hlasm::save_module(module, &dir.path().join(format!("{}.zo", module.name))).unwrap();
    }
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    assert_eq!(machine.call_hlasm("SUB", &[]).unwrap().unwrap(), 1);
    assert_eq!(machine.call_hlasm("SUB", &[]).unwrap().unwrap(), 2);
    assert_eq!(machine.call_hlasm("MAIN", &[]).unwrap().unwrap(), 1);
    assert_eq!(machine.call_hlasm("SUB", &[]).unwrap().unwrap(), 3);
}
