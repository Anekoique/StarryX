//! RISC-V 64 vDSO trap shims.

use core::arch::asm;

const NR_RT_SIGRETURN: usize = 139;

/// Patched by the kernel at vDSO install time to the user-space VA of
/// the shared `VDSO_DATA` page.
///
/// Lives in `.rodata` so it's part of the single PT_LOAD; the kernel
/// flips the code page R/W transiently to write it, then re-protects R-X.
#[unsafe(link_section = ".rodata.vdso_data_addr")]
#[unsafe(no_mangle)]
pub static VDSO_DATA_ADDR: usize = 0;

/// Returns the runtime VA of the kernel-published `VDSO_DATA` page.
#[inline(always)]
pub fn vdso_data_addr() -> usize {
    // Volatile so LLVM doesn't inline the link-time `0`.
    unsafe { core::ptr::read_volatile(&VDSO_DATA_ADDR) }
}

/// Read the architectural counter from U-mode.
#[inline(always)]
pub unsafe fn rdtime() -> u64 {
    let t: u64;
    unsafe {
        asm!("rdtime {0}", out(reg) t, options(nomem, nostack, preserves_flags));
    }
    t
}

#[inline]
pub unsafe fn syscall2(nr: usize, a0: usize, a1: usize) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "ecall",
            in("a7") nr,
            inlateout("a0") a0 => ret,
            in("a1") a1,
            options(nostack)
        );
    }
    ret
}

#[inline]
pub unsafe fn syscall3(nr: usize, a0: usize, a1: usize, a2: usize) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "ecall",
            in("a7") nr,
            inlateout("a0") a0 => ret,
            in("a1") a1,
            in("a2") a2,
            options(nostack)
        );
    }
    ret
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __vdso_rt_sigreturn() {
    core::arch::naked_asm!(
        "li a7, {nr}",
        "ecall",
        nr = const NR_RT_SIGRETURN,
    );
}
