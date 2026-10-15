use zeus_cobol::hlasm::{self, DynamicStorage, Module};

fn module(name: &str, body: &str) -> Module {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(format!("{name}.asm"));
    std::fs::write(&path, format!("{name} CSECT\n         USING {name},12\n         LR 12,15\n{body}\n         BR 14\n         END\n")).unwrap();
    hlasm::compile_file(&path, &[]).unwrap()
}

fn install(memory: &mut Vec<u8>, module: &Module) -> usize {
    let base = memory.len().next_multiple_of(module.alignment as usize);
    memory.resize(base, 0);
    memory.extend(module.storage_at(base).unwrap());
    base
}

#[test]
fn getmain_survives_return_and_is_freed_by_another_module_without_frame_leaks() {
    let take = module(
        "TAKE",
        "         L 2,0(1)\n         GETMAIN RU,LV=32,SP=7\n         ST 1,0(2)\n         MVI 0(1),165",
    );
    let free = module(
        "FREE",
        "         L 2,0(1)\n         L 3,0(2)\n         FREEMAIN RU,LV=32,A=(3),SP=7",
    );
    let mut memory = vec![0; 128];
    let take_at = install(&mut memory, &take);
    let free_at = install(&mut memory, &free);
    let baseline = memory.len();
    let mut storage = DynamicStorage::default();
    for _ in 0..100 {
        assert_eq!(
            hlasm::execute_with_storage(&take, &mut memory, take_at, &[Some(0)], &mut storage)
                .unwrap(),
            0
        );
        let pointer = u32::from_be_bytes(memory[..4].try_into().unwrap()) as usize;
        assert_eq!(memory[pointer], 165);
        assert_eq!(storage.allocated_bytes(), 32);
        assert_eq!(
            hlasm::execute_with_storage(&free, &mut memory, free_at, &[Some(0)], &mut storage)
                .unwrap(),
            0
        );
        assert_eq!(storage.allocated_bytes(), 0);
        assert_eq!(memory.len(), baseline);
    }
}

#[test]
fn allocator_splits_coalesces_reuses_blocks_and_checks_ownership() {
    let mut memory = vec![0; 129];
    let mut storage = DynamicStorage::with_limit(64);
    let first = storage.obtain(&mut memory, 32, 3, false, false).unwrap();
    let second = storage.obtain(&mut memory, 32, 3, false, false).unwrap();
    assert!(storage.obtain(&mut memory, 1, 3, false, false).is_err());
    assert!(storage.release(&mut memory, first, 32, 4).is_err());
    assert!(storage.release(&mut memory, 0, 8, 3).is_err());
    storage.release(&mut memory, first + 8, 16, 3).unwrap();
    let reused = storage.obtain(&mut memory, 16, 5, false, false).unwrap();
    assert_eq!(reused, first + 8);
    storage.release(&mut memory, reused, 16, 5).unwrap();
    storage.release(&mut memory, first, 8, 3).unwrap();
    storage.release(&mut memory, first + 24, 8, 3).unwrap();
    storage.release(&mut memory, second, 32, 3).unwrap();
    assert_eq!(memory.len(), 129);
    assert_eq!(storage.allocated_bytes(), 0);
    assert!(storage.release(&mut memory, first, 8, 3).is_err());
}

#[test]
fn page_alignment_fragmentation_cannot_bypass_the_run_unit_storage_budget() {
    let mut memory = Vec::new();
    let mut storage = DynamicStorage::with_limit(64 << 10);
    let mut count = 0;
    while storage.obtain(&mut memory, 8, 1, false, true).is_ok() {
        count += 1;
        assert!(count < 100);
    }
    assert!(storage.allocated_bytes() < 64 << 10);
    assert!(memory.len() <= (64 << 10) + 4096);
    storage.release_subpool(&mut memory, 1).unwrap();
    assert_eq!(memory.len(), 0);
    assert!(
        storage
            .obtain(&mut memory, 64 << 10, 1, false, false)
            .is_ok()
    );
}

#[test]
fn storage_release_subpools_and_register_return_codes_use_their_declared_destinations() {
    let module = module(
        "MAIN",
        "         STORAGE OBTAIN,LENGTH=8,SP=4\n         STORAGE OBTAIN,LENGTH=16,SP=4\n         STORAGE RELEASE,SP=4,COND=YES,RTCD=(6)\n         LR 15,6",
    );
    let mut memory = vec![0; 128];
    let base = install(&mut memory, &module);
    let baseline = memory.len();
    let mut storage = DynamicStorage::default();
    assert_eq!(
        hlasm::execute_with_storage(&module, &mut memory, base, &[], &mut storage).unwrap(),
        0
    );
    assert_eq!(storage.allocated_bytes(), 0);
    assert_eq!(memory.len(), baseline);
}

#[test]
fn storage_memory_destinations_page_alignment_register_lengths_and_return_codes() {
    let module = module(
        "MAIN",
        "         L 2,0(1)\n         LGHI 3,24\n         STORAGE OBTAIN,LENGTH=(3),ADDR=PTR,BNDRY=PAGE\n         L 4,PTR\n         ST 4,0(2)\n         MVI 0(4),77\n         STORAGE RELEASE,LENGTH=24,ADDR=0(4),RTCD=RC,COND=YES\n         L 15,RC\n         BR 14\nPTR DS F\nRC DS F",
    );
    let mut memory = vec![0; 128];
    let base = install(&mut memory, &module);
    let baseline = memory.len();
    let mut storage = DynamicStorage::default();
    assert_eq!(
        hlasm::execute_with_storage(&module, &mut memory, base, &[Some(0)], &mut storage).unwrap(),
        0
    );
    assert_eq!(
        u32::from_be_bytes(memory[..4].try_into().unwrap()) % 4096,
        0
    );
    assert_eq!(memory.len(), baseline);
}

#[test]
fn conditional_failure_returns_four_and_unconditional_errors_reclaim_frames() {
    let conditional = module("COND", "         GETMAIN RC,LV=32");
    let unconditional = module("UNCOND", "         GETMAIN RU,LV=32");
    let invalid_free = module("BADFREE", "         FREEMAIN RU,LV=72,A=(13)");
    let mut memory = vec![0; 128];
    let a = install(&mut memory, &conditional);
    let b = install(&mut memory, &unconditional);
    let c = install(&mut memory, &invalid_free);
    let baseline = memory.len();
    let mut storage = DynamicStorage::with_limit(16);
    assert_eq!(
        hlasm::execute_with_storage(&conditional, &mut memory, a, &[], &mut storage).unwrap(),
        4
    );
    assert!(
        hlasm::execute_with_storage(&unconditional, &mut memory, b, &[], &mut storage).is_err()
    );
    assert!(hlasm::execute_with_storage(&invalid_free, &mut memory, c, &[], &mut storage).is_err());
    assert_eq!(memory.len(), baseline);
    assert_eq!(storage.allocated_bytes(), 0);
}

#[test]
fn subpool_release_frees_only_requested_subpool_and_error_keeps_heap_allocation() {
    let obtain = module(
        "TAKE",
        "         L 2,0(1)\n         GETMAIN RU,LV=16,SP=7\n         ST 1,0(2)\n         LGHI 3,0\n         L 3,4095(3)",
    );
    let free = module("FREE", "         FREEMAIN RU,SP=7");
    let mut memory = vec![0; 128];
    let a = install(&mut memory, &obtain);
    let b = install(&mut memory, &free);
    let baseline = memory.len();
    let mut storage = DynamicStorage::default();
    assert!(
        hlasm::execute_with_storage(&obtain, &mut memory, a, &[Some(0)], &mut storage).is_err()
    );
    assert_eq!(storage.allocated_bytes(), 16);
    assert_eq!(
        hlasm::execute_with_storage(&free, &mut memory, b, &[], &mut storage).unwrap(),
        0
    );
    assert_eq!(storage.allocated_bytes(), 0);
    assert_eq!(memory.len(), baseline);
}

#[test]
fn implicit_r_form_uses_register_zero_subpool_and_length() {
    let take = module(
        "TAKE",
        "         L 2,0(1)\n         LGFI 0,X'07000020'\n         GETMAIN R\n         ST 1,0(2)",
    );
    let free = module(
        "FREE",
        "         L 2,0(1)\n         L 1,0(2)\n         LGFI 0,X'07000020'\n         FREEMAIN R",
    );
    let mut memory = vec![0; 128];
    let a = install(&mut memory, &take);
    let b = install(&mut memory, &free);
    let baseline = memory.len();
    let mut storage = DynamicStorage::default();
    assert_eq!(
        hlasm::execute_with_storage(&take, &mut memory, a, &[Some(0)], &mut storage).unwrap(),
        0
    );
    assert_eq!(storage.allocated_bytes(), 32);
    assert_eq!(
        hlasm::execute_with_storage(&free, &mut memory, b, &[Some(0)], &mut storage).unwrap(),
        0
    );
    assert_eq!(storage.allocated_bytes(), 0);
    assert_eq!(memory.len(), baseline);
}

#[test]
fn machine_retains_shared_allocation_across_distinct_assembler_calls() {
    let dir = tempfile::tempdir().unwrap();
    let take = module(
        "TAKE",
        "         L 2,0(1)\n         GETMAIN RU,LV=32\n         ST 1,0(2)\n         MVI 0(1),77",
    );
    let free = module(
        "FREE",
        "         L 2,0(1)\n         L 3,0(2)\n         FREEMAIN RU,LV=32,A=(3)",
    );
    hlasm::save_module(&take, &dir.path().join("TAKE.zo")).unwrap();
    hlasm::save_module(&free, &dir.path().join("FREE.zo")).unwrap();
    let mut machine = zeus_cobol::vm::Machine::new(vec![dir.path().to_path_buf()]);
    let argument = machine.getmain(4, 0);
    for _ in 0..10 {
        assert_eq!(
            machine
                .call_hlasm("TAKE", &[Some(argument)])
                .unwrap()
                .unwrap(),
            0
        );
        let address = u32::from_be_bytes(
            machine.storage()[argument..argument + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        assert_eq!(machine.storage()[address], 77);
        assert_eq!(
            machine
                .call_hlasm("FREE", &[Some(argument)])
                .unwrap()
                .unwrap(),
            0
        );
    }
}
