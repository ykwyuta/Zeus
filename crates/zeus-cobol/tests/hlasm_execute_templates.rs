use zeus_cobol::hlasm;

fn run(body: &str, data: &str) -> Result<i32, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("execute.asm");
    std::fs::write(&path, format!("MAIN CSECT\n         USING MAIN,12\n         LR 12,15\n{body}\n         BR 14\n{data}\n         END\n")).unwrap();
    let module = hlasm::compile_file(&path, &[])?;
    let mut memory = vec![0; 128];
    memory.extend(module.storage_at(128)?);
    let baseline = memory.len();
    let result = hlasm::execute(&module, &mut memory, 128, &[]);
    assert_eq!(memory.len(), baseline);
    result
}

#[test]
fn execute_ors_variable_ss_length_without_mutating_the_template() {
    assert_eq!(run("         LGHI 2,4\n         EX 2,TEMPLATE\n         CLC OUT(5),INPUT\n         BNE BAD\n         CLI TEMPLATE+1,0\n         BNE BAD\n         LGHI 15,0\n         BR 14\nBAD LGHI 15,99", "TEMPLATE MVC OUT(1),INPUT\nINPUT DC C'ABCDE'\nOUT DS CL5").unwrap(), 0);
}

#[test]
fn execute_reads_current_second_byte_and_register_zero_suppresses_oring() {
    assert_eq!(run("         LGHI 0,255\n         MVI TEMPLATE+1,0\n         EX 0,TEMPLATE\n         CLC OUT(5),=C'A....'\n         BNE BAD\n         LGHI 15,0\n         BR 14\nBAD LGHI 15,99", "TEMPLATE MVC OUT(4),INPUT\nINPUT DC C'ABCDE'\nOUT DC C'.....'").unwrap(), 0);
}

#[test]
fn execute_modifies_register_fields_and_uses_execute_return_address_for_linking() {
    assert_eq!(
        run(
            "         LGHI 3,9\n         LGHI 2,64\n         EX 2,TEMPLATE\n         LR 15,4",
            "TEMPLATE LR 0,3"
        )
        .unwrap(),
        9
    );
    assert_eq!(run("         LR 9,14\n         LA 4,SUB\n         EX 0,TEMPLATE\nAFTER LR 14,9\n         BR 14\nSUB LA 6,AFTER\n         CR 5,6\n         BNE BAD\n         LGHI 15,0\n         BR 5\nBAD LGHI 15,99\n         BR 5", "TEMPLATE BASR 5,4").unwrap(), 0);
}

#[test]
fn execute_relative_branches_use_template_origin_and_execute_link_continuation() {
    assert_eq!(run("         LGHI 2,128\n         CR 2,2\n         EX 2,TEMPLATE\n         LGHI 15,99\n         BR 14\nGOOD LGHI 15,7", "TEMPLATE JNOP GOOD").unwrap(), 7);
    assert_eq!(run("         LR 9,14\n         EX 0,TEMPLATE\nAFTER LR 14,9\n         BR 14\nSUB LA 6,AFTER\n         NILF 5,X'7FFFFFFF'\n         CR 5,6\n         BNE BAD\n         LGHI 15,0\n         BR 5\nBAD LGHI 15,99\n         BR 5", "TEMPLATE BRAS 5,SUB").unwrap(), 0);
}

#[test]
fn execute_relative_long_uses_relative_templates_and_rejects_nested_execute_forms() {
    assert_eq!(run("         LGHI 2,4\n         EXRL 2,TEMPLATE\n         CLC OUT(5),INPUT\n         BNE BAD\n         LGHI 15,0\n         BR 14\nBAD LGHI 15,99", "TEMPLATE MVC OUT(1),INPUT\nINPUT DC C'ABCDE'\nOUT DS CL5").unwrap(), 0);
    for target in ["EX", "EXRL"] {
        assert!(
            run(
                "         EXRL 0,TEMPLATE",
                &format!("TEMPLATE {target} 0,TEMPLATE")
            )
            .unwrap_err()
            .contains("nested EXECUTE")
        );
    }
}

#[test]
fn execute_rre_second_byte_is_an_opcode_and_does_not_change_register_fields() {
    assert_eq!(run("         LGHI 4,20\n         LGHI 5,7\n         LGHI 2,1\n         EX 2,TEMPLATE\n         LR 15,4", "TEMPLATE AGR 4,5").unwrap(), 13);
    assert_eq!(run("         LGHI 5,31\n         MVI TEMPLATE+1,4\n         EX 0,TEMPLATE\n         LR 15,4", "TEMPLATE LGR 4,5").unwrap(), 31);
    assert!(
        run(
            "         LGHI 2,255\n         EX 2,TEMPLATE",
            "TEMPLATE LGR 4,5"
        )
        .unwrap_err()
        .contains("unsupported RRE opcode")
    );
}

#[test]
fn execute_rejects_nested_unmapped_odd_and_unrepresented_template_changes() {
    assert!(
        run("         EX 0,TEMPLATE", "TEMPLATE EX 0,TEMPLATE")
            .unwrap_err()
            .contains("nested EXECUTE")
    );
    assert!(
        run("         EX 0,TEMPLATE+1", "TEMPLATE LR 2,3")
            .unwrap_err()
            .contains("halfword aligned")
    );
    assert!(
        run("         EX 0,DATA", "DATA DC X'0000'")
            .unwrap_err()
            .contains("compiled instruction")
    );
    assert!(
        run(
            "         MVI TEMPLATE+2,1\n         EX 0,TEMPLATE",
            "TEMPLATE MVC OUT(1),INPUT\nINPUT DC C'A'\nOUT DS C"
        )
        .unwrap_err()
        .contains("outside its supported second-byte")
    );
}
