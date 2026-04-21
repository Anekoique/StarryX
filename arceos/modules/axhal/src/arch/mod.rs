//! Architecture-specific types and operations.

cfg_if::cfg_if! {
    if #[cfg(target_arch = "riscv64")] {
        mod riscv;
        pub use self::riscv::*;
    } else if #[cfg(target_arch = "loongarch64")] {
        mod loongarch64;
        pub use self::loongarch64::*;
    } else {
        compile_error!("axhal: unsupported target_arch (only riscv64 and loongarch64 are built)");
    }
}
