//! Primary and secondary CPU bring-up for the LoongArch QEMU `virt` machine.
//!
//! Boot flow:
//! 1. QEMU enters at the kernel image's physical address (the linker sets
//!    `ENTRY(BASE_PADDR)` so the entry survives QEMU's 48-bit phys mask).
//! 2. Configure DMW0 to map `0x9000_xxxx_xxxx_xxxx` (cached, PLV0) to
//!    physical 0, then *jump into* that DMW window. PC must already live in
//!    a DMW window before CRMD.PG flips, otherwise the next fetch faults.
//! 3. Set up a boot page table that maps device MMIO (2 MiB pages,
//!    `BOOT_PT_L1[0]` -> `BOOT_PT_L2`) and the kernel image
//!    (1 GiB huge page at `BOOT_PT_L1[2]`) into `phys-virt-offset`
//!    (`0xffff_8000_*`).
//! 4. Enable paging atomically by writing `CRMD` once (PG=1, DA=0, MAT=CC).
//! 5. Rebase SP into the paged kernel VA region and jump absolute to
//!    `rust_entry`.

use axconfig::{
    TASK_STACK_SIZE,
    plat::{PHYS_BOOT_OFFSET, PHYS_VIRT_OFFSET},
};
use page_table_entry::{GenericPTE, MappingFlags, loongarch64::LA64PTE};

/// Forces 4 KiB alignment on the wrapped value.
#[repr(align(4096))]
struct Aligned4K<T>(T);

impl<T> core::ops::Deref for Aligned4K<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> core::ops::DerefMut for Aligned4K<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

#[unsafe(link_section = ".bss.stack")]
static mut BOOT_STACK: [u8; TASK_STACK_SIZE] = [0; TASK_STACK_SIZE];

#[unsafe(link_section = ".data.boot_page_table")]
static mut BOOT_PT_L0: Aligned4K<[LA64PTE; 512]> = Aligned4K([LA64PTE::empty(); 512]);
#[unsafe(link_section = ".data.boot_page_table")]
static mut BOOT_PT_L1: Aligned4K<[LA64PTE; 512]> = Aligned4K([LA64PTE::empty(); 512]);
#[unsafe(link_section = ".data.boot_page_table")]
static mut BOOT_PT_L2: Aligned4K<[LA64PTE; 512]> = Aligned4K([LA64PTE::empty(); 512]);

/// Translate a kernel-link-time virtual address to its physical counterpart.
///
/// Boot code runs through the DMW1 window (`0x9000_*`), so the same physical
/// memory is also visible at `phys + PHYS_BOOT_OFFSET`. Both this and
/// [`PHYS_VIRT_OFFSET`] resolve to the same physical address, but the boot
/// window is the only one valid before paging is enabled.
const fn boot_virt_to_phys(va: usize) -> usize {
    va - PHYS_BOOT_OFFSET
}

unsafe fn init_boot_page_table() {
    unsafe {
        // L0[0x100] covers VA 0xffff_8000_0000_0000 .. 0xffff_8080_0000_0000.
        BOOT_PT_L0[0x100] =
            LA64PTE::new_table(pa!(boot_virt_to_phys(&raw const BOOT_PT_L1 as usize)));

        // 0..1 GiB: 2 MiB pages so device MMIO and low memory each get their
        // own TLB entries. The single-1GiB-huge-page approach trips a TLB
        // pair fault around the goldfish RTC at PA 0x100d_0100.
        BOOT_PT_L1[0] = LA64PTE::new_table(pa!(boot_virt_to_phys(&raw const BOOT_PT_L2 as usize)));
        for i in 0..512 {
            // First 256 MiB are RAM/code; the rest is device MMIO.
            let mut flags = MappingFlags::READ | MappingFlags::WRITE;
            flags |= if i < 128 {
                MappingFlags::EXECUTE
            } else {
                MappingFlags::DEVICE
            };
            BOOT_PT_L2[i] = LA64PTE::new_page(pa!(i << 21), flags, true);
        }

        // 2..3 GiB: kernel image, mapped as a single 1 GiB huge page.
        BOOT_PT_L1[2] = LA64PTE::new_page(
            pa!(0x8000_0000),
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
            true,
        );
    }
}

unsafe fn init_mmu() {
    use loongArch64::register::{pgdh, pgdl, stlbps, tlbidx, tlbrehi, tlbrentry};
    use page_table_multiarch::loongarch64::LA64MetaData;

    const PS_4K: usize = 0x0c;
    tlbidx::set_ps(PS_4K);
    stlbps::set_ps(PS_4K);
    tlbrehi::set_ps(PS_4K);

    crate::arch::set_pwc(LA64MetaData::PWCL_VALUE, LA64MetaData::PWCH_VALUE);

    unsafe extern "C" {
        fn handle_tlb_refill();
    }
    tlbrentry::set_tlbrentry(boot_virt_to_phys(handle_tlb_refill as *const () as usize));

    pgdh::set_base(boot_virt_to_phys(&raw const BOOT_PT_L0 as usize));
    pgdl::set_base(0);

    crate::arch::flush_tlb(None);

    // Atomically transition CRMD to paged mode. Newer QEMU versions trap on
    // the very next fetch if any of these bits is wrong:
    //
    //   bit 3   DA   = 0  (disable direct addressing)
    //   bit 4   PG   = 1  (enable paging)
    //   bits 5-6 DATF = 1  (Coherent Cached for fetch)
    //   bits 7-8 DATM = 1  (Coherent Cached for load/store)
    const CRMD_DA: usize = 1 << 3;
    const CRMD_PG: usize = 1 << 4;
    const CRMD_DATF_MASK: usize = 0b11 << 5;
    const CRMD_DATM_MASK: usize = 0b11 << 7;
    const CRMD_DAT_CC: usize = (0b01 << 5) | (0b01 << 7);
    let mut crmd: usize;
    unsafe { core::arch::asm!("csrrd {0}, 0x0", out(reg) crmd) };
    crmd &= !(CRMD_DA | CRMD_DATF_MASK | CRMD_DATM_MASK);
    crmd |= CRMD_PG | CRMD_DAT_CC;
    unsafe { core::arch::asm!("csrwr {0}, 0x0", inlateout(reg) crmd => _) };
}

/// Linked-VA -> DMW1-VA bias used to rebase SP after paging is enabled.
const BOOT_TO_VIRT: usize = PHYS_VIRT_OFFSET.wrapping_sub(PHYS_BOOT_OFFSET);

/// The earliest entry point for the primary CPU.
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
unsafe extern "C" fn _start() -> ! {
    core::arch::naked_asm!(
        "
        # DMW0: VSEG=0x9 -> physical 0, PLV0, MAT=Coherent Cached.
        li.d        $t0, {phys_boot_offset}
        ori         $t0, $t0, 0x11
        csrwr       $t0, 0x180

        # Hop into DMW1 so PC stays in a valid window after CRMD.PG=1.
        la.local    $t0, 1f
        li.d        $t1, {phys_boot_offset}
        or          $t0, $t0, $t1
        jirl        $zero, $t0, 0
    1:
        # `la.global` is PC-relative on LoongArch, so this resolves to the
        # DMW1 alias of BOOT_STACK (we're running in DMW1 right now).
        la.global   $sp, {boot_stack}
        li.d        $t0, {boot_stack_size}
        add.d       $sp, $sp, $t0

        bl          {init_boot_page_table}
        bl          {init_mmu}

        # Paging is on. Rebase SP into the paged kernel VA region and jump
        # absolute to rust_entry at its link-time virtual address.
        li.d        $t0, {boot_to_virt}
        add.d       $sp, $sp, $t0

        csrrd       $a0, 0x20           # cpuid -> a0
        la.abs      $t0, {entry}
        li.d        $ra, 0
        jirl        $zero, $t0, 0",
        phys_boot_offset = const PHYS_BOOT_OFFSET,
        boot_to_virt = const BOOT_TO_VIRT,
        boot_stack = sym BOOT_STACK,
        boot_stack_size = const TASK_STACK_SIZE,
        init_boot_page_table = sym init_boot_page_table,
        init_mmu = sym init_mmu,
        entry = sym super::rust_entry,
    )
}

/// The earliest entry point for secondary CPUs.
#[cfg(feature = "smp")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
unsafe extern "C" fn _start_secondary() -> ! {
    core::arch::naked_asm!(
        "
        li.d        $t0, {phys_boot_offset}
        ori         $t0, $t0, 0x11
        csrwr       $t0, 0x180

        la.local    $t0, 1f
        li.d        $t1, {phys_boot_offset}
        or          $t0, $t0, $t1
        jirl        $zero, $t0, 0
    1:
        # SMP_BOOT_STACK_TOP is published as a paged kernel VA; rebase to DMW1.
        la.abs      $t0, {sm_boot_stack_top}
        ld.d        $sp, $t0, 0
        li.d        $t0, {boot_to_virt}
        sub.d       $sp, $sp, $t0

        bl          {init_mmu}

        li.d        $t0, {boot_to_virt}
        add.d       $sp, $sp, $t0

        csrrd       $a0, 0x20
        la.abs      $t0, {entry}
        li.d        $ra, 0
        jirl        $zero, $t0, 0",
        phys_boot_offset = const PHYS_BOOT_OFFSET,
        boot_to_virt = const BOOT_TO_VIRT,
        sm_boot_stack_top = sym super::mp::SMP_BOOT_STACK_TOP,
        init_mmu = sym init_mmu,
        entry = sym super::rust_entry_secondary,
    )
}
