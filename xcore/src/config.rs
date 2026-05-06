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
        compile_error!("unsupported target architecture for xcore::config");
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

/// vDSO code-page base virtual address (R-X to user, alloc-backed,
/// per-process copy of the embedded ELF blob).
///
/// Reuses the slot freed by removing the legacy `SIGNAL_TRAMPOLINE` page;
/// `__vdso_rt_sigreturn` lives inside the same image now.
pub const USER_VDSO_BASE: usize = 0x4001_0000;

/// vDSO data-page virtual address (R-only to user, single shared phys
/// page across all processes; see `xcore::vdso::data::VDSO_DATA`).
pub const USER_VDSO_DATA: usize = 0x4001_2000;
