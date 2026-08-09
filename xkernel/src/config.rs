//! Architecture-dependent constants for the kernel/user address layout.
//!
//! Most addresses are shared across all supported architectures; only the
//! width of the user address space (and therefore the stack top) and the
//! per-arch user stack size differ.

cfg_if::cfg_if! {
    if #[cfg(target_arch = "riscv64")] {
        // Sv39 user space: 38-bit addressable region.
        pub const USER_SPACE_SIZE: usize = 0x3f_ffff_f000;
        pub const USER_STACK_TOP: usize = 0x4_0000_0000;
        // RISC-V musl libc binaries push noticeably more onto the stack than
        // the LoongArch ones, so give them a bit more headroom.
        pub const USER_STACK_SIZE: usize = 0x8_0000;
    } else if #[cfg(target_arch = "loongarch64")] {
        pub const USER_SPACE_SIZE: usize = 0x3f_ffff_f000;
        pub const USER_STACK_TOP: usize = 0x4_0000_0000;
        pub const USER_STACK_SIZE: usize = 0x5_0000;
    } else {
        compile_error!("unsupported target architecture for xkernel::config");
    }
}

/// Lowest user-space virtual address actually used for code/data.
pub const USER_SPACE_BASE: usize = 0x1000;
/// Base address used when loading a dynamic ELF interpreter (ld.so).
pub const USER_INTERP_BASE: usize = 0x400_0000;

/// Lowest user-space heap address; `brk` grows up from here.
pub const USER_HEAP_BASE: usize = 0x4000_0000;
/// Initial size of the user heap.
pub const USER_HEAP_SIZE: usize = 0x1_0000;

/// Per-thread kernel stack size.
pub const KERNEL_STACK_SIZE: usize = 0x40000;

/// Lowest virtual address of the Linux vDSO data (vvar) mapping.
///
/// The Linux image reaches this page with a negative PC-relative offset.
pub const USER_VDSO_DATA: usize = 0x4001_0000;

/// vDSO ELF base published as `AT_SYSINFO_EHDR`.
///
/// Linux 6.8 reserves two vvar pages on RISC-V. LoongArch additionally
/// reserves one architecture-data page for the supported external image.
#[cfg(target_arch = "riscv64")]
pub const USER_VDSO_BASE: usize = USER_VDSO_DATA + 0x2000;
#[cfg(target_arch = "loongarch64")]
pub const USER_VDSO_BASE: usize = USER_VDSO_DATA + 0x3000;
