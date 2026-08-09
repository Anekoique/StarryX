# kernel-elf-parser

[![Crates.io](https://img.shields.io/crates/v/kernel-elf-parser)](https://crates.io/crates/kernel-elf-parser)
[![Docs.rs](https://docs.rs/kernel-elf-parser/badge.svg)](https://docs.rs/kernel-elf-parser)
[![CI](https://github.com/Azure-stars/kernel-elf-parser/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Azure-stars/kernel-elf-parser/actions/workflows/ci.yml)

A lightweight ELF parser written in Rust, providing assistance for loading applications into the kernel.

It reads the data of the ELF file, and generates Sections, Relocations, Segments and so on.

It also generate a layout of the user stack according to the given user parameters and environment variables,which will be 
used for loading a given application into the physical memory of the kernel.

## Examples

```rust
use std::collections::BTreeMap;
use kernel_elf_parser::{AuxvEntry, AuxvType};
let args: Vec<String> = vec!["arg1".to_string(), "arg2".to_string(), "arg3".to_string()];
let envs: Vec<String> = vec!["LOG=file".to_string()];
let mut auxv: [AuxvEntry; 17] = [
    AuxvEntry::new(AuxvType::PHDR, 0x1000),
    AuxvEntry::new(AuxvType::PHENT, 1024),
    AuxvEntry::new(AuxvType::PHNUM, 10),
    AuxvEntry::new(AuxvType::PAGESZ, 0x1000),
    AuxvEntry::new(AuxvType::BASE, 0),
    AuxvEntry::new(AuxvType::FLAGS, 0),
    AuxvEntry::new(AuxvType::ENTRY, 0x1000),
    AuxvEntry::new(AuxvType::HWCAP, 0),
    AuxvEntry::new(AuxvType::CLKTCK, 100),
    AuxvEntry::new(AuxvType::PLATFORM, 0),
    AuxvEntry::new(AuxvType::UID, 0),
    AuxvEntry::new(AuxvType::EUID, 0),
    AuxvEntry::new(AuxvType::GID, 0),
    AuxvEntry::new(AuxvType::EGID, 0),
    AuxvEntry::new(AuxvType::RANDOM, 0),
    AuxvEntry::new(AuxvType::EXECFN, 0),
    AuxvEntry::new(AuxvType::NULL, 0),
];
// The highest address of the user stack.
let ustack_end = 0x4000_0000;
let ustack_size = 0x1_0000;
let ustack_start = ustack_end - ustack_size;

let stack_data = kernel_elf_parser::app_stack_region(&args, &envs, &mut auxv, ustack_start.into(), ustack_size);

// args length
assert_eq!(stack_data[0..8], [3, 0, 0, 0, 0, 0, 0, 0]);

```