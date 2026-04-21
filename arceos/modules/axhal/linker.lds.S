OUTPUT_ARCH(%ARCH%)

BASE_ADDRESS = %KERNEL_BASE%;
BASE_PADDR = %KERNEL_BASE_PADDR%;
VMA_TO_LMA = BASE_ADDRESS - BASE_PADDR;

/* QEMU loads the LoongArch ELF kernel directly without firmware, so the entry
 * must be a physical address. On other archs the firmware (OpenSBI on RISC-V,
 * EDK2 on aarch64) handles the V→P translation, but giving them the PA here is
 * still correct. */
ENTRY(BASE_PADDR)

SECTIONS
{
    . = BASE_ADDRESS;
    _skernel = .;

    .text : AT(ADDR(.text) - VMA_TO_LMA) ALIGN(4K) {
        _stext = .;
        KEEP(*(.text.boot))
        *(.text .text.*)
        . = ALIGN(4K);
        _etext = .;
    }

    _srodata = .;
    .rodata : AT(ADDR(.rodata) - VMA_TO_LMA) ALIGN(4K) {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
        *(.sdata2 .sdata2.*)
    }

    .init_array : AT(ADDR(.init_array) - VMA_TO_LMA) ALIGN(0x10) {
        __init_array_start = .;
        *(.init_array .init_array.*)
        __init_array_end = .;
    }

    . = ALIGN(4K);
    _erodata = .;

    .data : AT(ADDR(.data) - VMA_TO_LMA) ALIGN(4K) {
        _sdata = .;
        *(.data.boot_page_table)
        . = ALIGN(4K);
        *(.data .data.*)
        *(.sdata .sdata.*)
        *(.got .got.*)
    }

    .tdata : AT(ADDR(.tdata) - VMA_TO_LMA) ALIGN(0x10) {
        _stdata = .;
        *(.tdata .tdata.*)
        _etdata = .;
    }

    .tbss : AT(ADDR(.tbss) - VMA_TO_LMA) ALIGN(0x10) {
        _stbss = .;
        *(.tbss .tbss.*)
        *(.tcommon)
        _etbss = .;
    }

    . = ALIGN(4K);
    _percpu_start = .;
    _percpu_end = _percpu_start + SIZEOF(.percpu);
    .percpu 0x0 : AT(_percpu_start - VMA_TO_LMA) {
        _percpu_load_start = .;
        *(.percpu .percpu.*)
        _percpu_load_end = .;
        . = _percpu_load_start + ALIGN(64) * %SMP%;
    }
    . = _percpu_end;

    . = ALIGN(4K);
    _edata = .;

    .bss : AT(. - VMA_TO_LMA) ALIGN(4K) {
        boot_stack = .;
        *(.bss.stack)
        . = ALIGN(4K);
        boot_stack_top = .;

        _sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        *(COMMON)
        . = ALIGN(4K);
        _ebss = .;
    }

    _ekernel = .;

	/DISCARD/ : {
        *(.comment) *(.gnu*) *(.note*) *(.eh_frame*)
    }
}

SECTIONS {
    linkme_IRQ : { *(linkme_IRQ) }
    linkm2_IRQ : { *(linkm2_IRQ) }
    linkme_PAGE_FAULT : { *(linkme_PAGE_FAULT) }
    linkm2_PAGE_FAULT : { *(linkm2_PAGE_FAULT) }
    linkme_SYSCALL : { *(linkme_SYSCALL) }
    linkm2_SYSCALL : { *(linkm2_SYSCALL) }
    linkme_POST_TRAP : { *(linkme_POST_TRAP) }
    linkm2_POST_TRAP : { *(linkm2_POST_TRAP) }
    axns_resource : { *(axns_resource) }
}
INSERT AFTER .tbss;
