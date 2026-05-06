//! Architecture-specific signal handling
//!
//! This module provides architecture-specific implementations for signal handling.
//! Different CPU architectures have different calling conventions, register layouts,
//! and signal delivery mechanisms, so each supported architecture has its own
//! implementation.
//!
//! Currently supported architectures:
//! - x86_64: Intel/AMD 64-bit processors
//! - RISC-V: Both 32-bit and 64-bit RISC-V processors  
//! - AArch64: ARM 64-bit processors
//! - LoongArch64: Loongson 64-bit processors
//!
//! The main functionality provided by each architecture module includes:
//! - Signal frame setup and restoration
//! - Register context manipulation
//! - Signal trampoline code
//! - Architecture-specific signal delivery mechanisms

cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        mod x86_64;
        pub use self::x86_64::*;
    } else if #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))] {
        mod riscv;
        pub use self::riscv::*;
    } else if #[cfg(target_arch = "aarch64")]{
        mod aarch64;
        pub use self::aarch64::*;
    } else if #[cfg(target_arch = "loongarch64")] {
        mod loongarch64;
        pub use self::loongarch64::*;
    } else {
        compile_error!("Unsupported architecture");
    }
}

// The legacy fixed signal trampoline (`signal_trampoline` / `signal_trampoline_address`)
// has been replaced by the vDSO-resident `__vdso_rt_sigreturn`. The per-process
// restorer address is published by `xcore::vdso::install` via
// `ProcessSignalManager::set_default_restorer`.
