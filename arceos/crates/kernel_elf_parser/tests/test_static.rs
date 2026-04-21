use kernel_elf_parser::ELFParser;
use memory_addr::PAGE_SIZE_4K;

#[test]
fn test_elf_parser() {
    use memory_addr::VirtAddr;
    // A simple elf file compiled by the x86_64-linux-musl-gcc.
    let elf_bytes = include_bytes!("elf_static");
    // Ensure the alignment of the byte array
    let mut aligned_elf_bytes = unsafe {
        let ptr = elf_bytes.as_ptr() as *mut u8;
        std::slice::from_raw_parts_mut(ptr, elf_bytes.len())
    }
    .to_vec();
    if aligned_elf_bytes.len() % 16 != 0 {
        let padding = vec![0u8; 16 - aligned_elf_bytes.len() % 16];
        aligned_elf_bytes.extend(padding);
    }
    let elf =
        xmas_elf::ElfFile::new(aligned_elf_bytes.as_slice()).expect("Failed to read elf file");

    let interp_base = 0x1000;
    let elf_parser = kernel_elf_parser::ELFParser::new(&elf, interp_base, None, 0).unwrap();
    let base_addr = elf_parser.base();
    assert_eq!(base_addr, 0);

    let segments = elf_parser.ph_load();
    assert_eq!(segments.len(), 4);
    let mut last_start = VirtAddr::from_usize(0);
    for segment in segments.iter() {
        // start vaddr should be sorted
        assert!(segment.vaddr > last_start);
        last_start = segment.vaddr;
    }
    assert_eq!(segments[0].vaddr, VirtAddr::from_usize(0x400000));

    test_ustack(&elf_parser);
}

fn test_ustack(elf_parser: &ELFParser) {
    let mut auxv = elf_parser.auxv_vector(PAGE_SIZE_4K);
    // let phent = auxv.get(&AT_PHENT).unwrap();
    // assert_eq!(*phent, 56);
    auxv.iter().for_each(|entry| {
        if entry.get_type() == kernel_elf_parser::AuxvType::PHENT {
            assert_eq!(entry.value(), 56);
        }
    });

    let args: Vec<String> = vec!["arg1".to_string(), "arg2".to_string(), "arg3".to_string()];
    let envs: Vec<String> = vec!["LOG=file".to_string()];

    // The highest address of the user stack.
    let ustack_end = 0x4000_0000;
    let ustack_size = 0x2_0000;
    let ustack_bottom = ustack_end - ustack_size;

    let stack_data = kernel_elf_parser::app_stack_region(
        &args,
        &envs,
        &mut auxv,
        ustack_bottom.into(),
        ustack_size,
    );
    // The first 8 bytes of the stack is the number of arguments.
    assert_eq!(stack_data[0..8], [3, 0, 0, 0, 0, 0, 0, 0]);
}
