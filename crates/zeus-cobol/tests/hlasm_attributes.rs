use zeus_cobol::hlasm;

fn compile(source: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("attrs.asm");
    std::fs::write(&path, source).unwrap();
    hlasm::compile_file(&path, &[])
}

fn words(module: &hlasm::Module, count: usize) -> Vec<u32> {
    let storage = module.storage_at(128).unwrap();
    let at = module.entries["RESULT"] as usize;
    storage[at..at + count * 4]
        .chunks_exact(4)
        .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
        .collect()
}

fn ebcdic(character: &str) -> u32 {
    u32::from(zeus_cobol::ebcdic::encode(character)[0])
}

#[test]
fn equ_explicit_length_and_type_override_independently_and_preserve_symbol_value() {
    let module = compile("MAIN CSECT\n         BR 14\nBUFFER DC CL12'ABCDEFGHIJKL'\nPART EQU BUFFER+3,5,C'Q'\nCOPY EQU PART\nTYPED EQU BUFFER,,C'X'\nZERO EQU 7,0,255\nMAXIMUM EQU 3,65535,0\nRESULT DC A(L'PART,T'PART,L'COPY,T'COPY,L'TYPED,T'TYPED)\n         DC A(L'ZERO,T'ZERO,L'MAXIMUM,T'MAXIMUM,PART-BUFFER)\n         ENTRY RESULT\n         END\n").unwrap();
    assert_eq!(
        words(&module, 11),
        vec![
            5,
            ebcdic("Q"),
            5,
            ebcdic("U"),
            12,
            ebcdic("X"),
            0,
            255,
            65535,
            0,
            3
        ]
    );
}

#[test]
fn equ_forward_values_use_attributes_captured_at_the_definition() {
    let module = compile("MAIN CSECT\n         BR 14\nWIDTH EQU 6\nKIND EQU C'R'\nFORWARD EQU LATER,WIDTH+2,KIND\nCHAIN EQU FORWARD\nLENGTH EQU L'FORWARD\nLATER DC F'19'\nRESULT DC A(L'FORWARD,T'FORWARD,L'CHAIN,T'CHAIN,LENGTH)\n         DC A(FORWARD)\n         ENTRY RESULT,LATER\n         END\n").unwrap();
    assert_eq!(
        words(&module, 6),
        vec![
            8,
            ebcdic("R"),
            8,
            ebcdic("U"),
            8,
            128 + module.entries["LATER"]
        ]
    );
}

#[test]
fn source_order_attributes_allow_a_forward_equ_only_after_its_value_becomes_known() {
    let module = compile("MAIN CSECT\n         BR 14\nDEFER EQU TARGET\nTARGET DC CL9'ABCDEFGHI'\nVIEW EQU DEFER,L'DEFER,C'Z'\nRESULT DC A(L'VIEW,T'VIEW)\n         ENTRY RESULT\n         END\n").unwrap();
    assert_eq!(words(&module, 2), vec![9, ebcdic("Z")]);
    for body in [
        "VIEW EQU 0,WIDTH\nWIDTH EQU 7\n",
        "VIEW EQU 0,,KIND\nKIND EQU C'R'\n",
        "DEFER EQU WIDTH\nVIEW EQU 0,DEFER\nWIDTH EQU 7\n",
        "DEFER EQU TARGET\nVIEW EQU 0,L'DEFER\nTARGET DC CL9'ABCDEFGHI'\n",
    ] {
        let error =
            compile(&format!("MAIN CSECT\n         BR 14\n{body}         END\n")).unwrap_err();
        assert!(
            error.contains("must be known at its definition"),
            "{body}: {error}"
        );
    }
}

#[test]
fn implicit_attributes_use_physical_instruction_lengths_and_storage_types() {
    let module = compile("MAIN CSECT\n         BR 14\nTWO LR 2,2\nFOUR LGHI 3,0\nSIX LARL 4,TWO\nALIAS EQU SIX\nCHARS DC CL7'ABCDEFG'\nHEX DC XL3'123456'\nFULL DC F'4'\nRESULT DC A(L'TWO,L'FOUR,L'SIX,L'ALIAS,T'TWO,T'ALIAS)\n         DC A(L'CHARS,T'CHARS,L'HEX,T'HEX,L'FULL,T'FULL)\n         ENTRY RESULT\n         END\n").unwrap();
    assert_eq!(
        words(&module, 12),
        vec![
            2,
            4,
            6,
            6,
            ebcdic("I"),
            ebcdic("U"),
            7,
            ebcdic("C"),
            3,
            ebcdic("X"),
            4,
            ebcdic("F")
        ]
    );
}

#[test]
fn explicit_equ_lengths_control_storage_instruction_operands_and_type_expressions() {
    let module = compile("MAIN CSECT\n         USING MAIN,15\n         MVC SHORT,SOURCE\n         LGHI 15,L'SHORT+T'SHORT-C'Z'\n         BR 14\nSOURCE DC CL6'ABCDEF'\nDEST DC CL6' '\nSHORT EQU DEST,3,C'Z'\n         ENTRY DEST\n         END\n").unwrap();
    let module = hlasm::load_module(&hlasm::module_bytes(&module).unwrap()).unwrap();
    let mut memory = vec![0; 128];
    memory.extend(module.storage_at(128).unwrap());
    let length = memory.len();
    assert_eq!(hlasm::execute(&module, &mut memory, 128, &[]).unwrap(), 3);
    assert_eq!(memory.len(), length);
    let dest = 128 + module.entries["DEST"] as usize;
    assert_eq!(
        &memory[dest..dest + 6],
        zeus_cobol::ebcdic::encode("ABC   ")
    );
}

#[test]
fn equ_invalid_attribute_ranges_relocation_duplicates_and_extra_operands_are_diagnosed() {
    // Statements after END must not enter the forward-EQU or attribute passes.
    compile("MAIN CSECT\n         BR 14\n         END\nIGNORED EQU MISSING,999999,999\n").unwrap();
    for (body, diagnostic) in [
        ("BAD EQU 0,-1", "0..=65535"),
        ("BAD EQU 0,65536", "0..=65535"),
        ("BAD EQU 0,,-1", "0..=255"),
        ("BAD EQU 0,,256", "0..=255"),
        ("BAD EQU 0,MAIN", "must be absolute"),
        ("BAD EQU 0,,MAIN", "must be absolute"),
        ("BAD EQU 0,1/0", "division by zero"),
        ("BAD EQU 0,1,C'AB'", "0..=255"),
        ("BAD EQU 0,1,2,3,GR,EXTRA", "at most five"),
        ("BAD EQU 0,1,2\nBAD EQU 0,2,2", "duplicate symbol"),
        ("BAD EQU 0,1,2\nBAD EQU 0,1,3", "duplicate symbol"),
        ("         EQU 0,1,2", "symbol required"),
    ] {
        let source = format!("MAIN CSECT\n         BR 14\n{body}\n         END\n");
        let error = compile(&source).unwrap_err();
        assert!(error.contains(diagnostic), "{body}: {error}");
        assert!(error.contains("attrs.asm:"), "{error}");
    }
}

#[test]
fn equ_program_tags_and_register_types_accept_all_keywords_without_changing_execution() {
    for tag in [
        "0",
        "-1",
        "-2147483648",
        "4294967295",
        "C'TAG'",
        "X'89ABCDEF'",
        "B'101010'",
        "L'SOURCE+4",
    ] {
        let mut body = String::from(
            "MAIN CSECT\n         USING MAIN,15\n         LGHI WORK,7\n         L ARG,0(1)\n         ST WORK,0(ARG)\n         LGHI 15,L'VALUE+T'VALUE-C'F'\n         BR 14\nSOURCE DC CL5'ABCDE'\n",
        );
        for (n, kind) in [
            "AR", "CR", "CR32", "CR64", "FPR", "GR", "GR32", "GR64", "VR",
        ]
        .iter()
        .enumerate()
        {
            body.push_str(&format!("TYPED{n} EQU {n},,,{tag},{kind}\n"));
        }
        body.push_str(&format!(
            "ARG EQU 2,,,,gr32\nWORK EQU 3,,,,GR64\nVALUE EQU 0,4,C'F',{tag},GR\n         END\n"
        ));
        let module = compile(&body).unwrap();
        let module = hlasm::load_module(&hlasm::module_bytes(&module).unwrap()).unwrap();
        let mut memory = vec![0; 128];
        memory.extend(module.storage_at(128).unwrap());
        assert_eq!(
            hlasm::execute(&module, &mut memory, 128, &[Some(0)]).unwrap(),
            4
        );
        assert_eq!(&memory[..4], &7u32.to_be_bytes());
    }
}

#[test]
fn equ_program_type_obeys_source_order_and_rejects_invalid_tags_or_keywords() {
    for (body, diagnostic) in [
        ("BAD EQU 0,,,-2147483649", "must be absolute"),
        ("BAD EQU 0,,,4294967296", "must be absolute"),
        ("BAD EQU 0,,,MAIN", "must be absolute"),
        ("BAD EQU 0,,,LATER\nLATER EQU 7", "known at its definition"),
        ("BAD EQU 0,,,C'ABCDE'", "exceeds four bytes"),
        ("BAD EQU 0,,,,GR128", "invalid EQU assembler type"),
        ("BAD EQU 0,,,,'GR'", "invalid EQU assembler type"),
        ("BAD EQU 0,,,0\nBAD EQU 0,,,1", "duplicate symbol"),
        ("BAD EQU 0,,,,GR32\nBAD EQU 0,,,,GR64", "duplicate symbol"),
    ] {
        let error = compile(&format!(
            "MAIN CSECT\n         BR 14\n{body}\n         END\n"
        ))
        .unwrap_err();
        assert!(error.contains(diagnostic), "{body}: {error}");
    }
    compile("MAIN CSECT\n         BR 14\nTAG EQU 17\nFORWARD EQU LATER,,,TAG,GR32\nLATER DC F'3'\n         END\n").unwrap();
}
