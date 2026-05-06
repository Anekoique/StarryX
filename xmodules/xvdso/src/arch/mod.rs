//! Per-arch trap shims used by the vDSO.

#[cfg(target_arch = "riscv64")]
mod riscv64;
#[cfg(target_arch = "riscv64")]
pub use riscv64::*;

#[cfg(target_arch = "loongarch64")]
mod loongarch64;
#[cfg(target_arch = "loongarch64")]
pub use loongarch64::*;

#[cfg(not(any(target_arch = "riscv64", target_arch = "loongarch64")))]
compile_error!("xvdso: unsupported target arch");
