//! LoongArch 64 vDSO trap shims.

use core::arch::asm;

const NR_RT_SIGRETURN: usize = 139;

#[unsafe(link_section = ".rodata.vdso_data_addr")]
#[unsafe(no_mangle)]
pub static VDSO_DATA_ADDR: usize = 0;

#[inline(always)]
pub fn vdso_data_addr() -> usize {
    unsafe { core::ptr::read_volatile(&VDSO_DATA_ADDR) }
}

#[inline(always)]
pub unsafe fn rdtime() -> u64 {
    let t: u64;
    let _id: u64;
    unsafe {
        asm!(
            "rdtime.d {0}, {1}",
            out(reg) t,
            out(reg) _id,
            options(nomem, nostack, preserves_flags),
        );
    }
    t
}

#[inline]
pub unsafe fn syscall2(nr: usize, a0: usize, a1: usize) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "syscall 0",
            in("$a7") nr,
            inlateout("$a0") a0 => ret,
            in("$a1") a1,
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
            "syscall 0",
            in("$a7") nr,
            inlateout("$a0") a0 => ret,
            in("$a1") a1,
            in("$a2") a2,
            options(nostack)
        );
    }
    ret
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __vdso_rt_sigreturn() {
    core::arch::naked_asm!(
        "li.w  $a7, {nr}",
        "syscall 0",
        nr = const NR_RT_SIGRETURN,
    );
}
