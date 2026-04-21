//! Platform-specific operations.

cfg_if::cfg_if! {
    if #[cfg(all(target_arch = "riscv64", platform_family = "riscv64-qemu-virt"))] {
        mod riscv64_qemu_virt;
        pub use self::riscv64_qemu_virt::*;
    } else if #[cfg(all(target_arch = "riscv64", platform_family = "riscv64-visionfive2"))] {
        mod riscv64_visionfive2;
        pub use self::riscv64_visionfive2::*;
    } else if #[cfg(all(target_arch = "loongarch64", platform_family = "loongarch64-qemu-virt"))] {
        mod loongarch64_qemu_virt;
        pub use self::loongarch64_qemu_virt::*;
    } else {
        mod dummy;
        pub use self::dummy::*;
    }
}
