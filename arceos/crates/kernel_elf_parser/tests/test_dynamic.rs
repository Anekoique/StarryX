#[test]
fn test_elf_parser() {
    use memory_addr::VirtAddr;
    let elf_bytes = include_bytes!("ld-linux-x86-64.so.2");
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
    assert_eq!(base_addr, interp_base);

    let segments = elf_parser.ph_load();
    assert_eq!(segments.len(), 4);
    for segment in segments.iter() {
        println!("{:?} {:?}", segment.vaddr, segment.flags);
    }
    assert_eq!(segments[0].vaddr, VirtAddr::from_usize(0x1000));
}
