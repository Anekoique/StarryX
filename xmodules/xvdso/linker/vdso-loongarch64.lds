/* StarryX vDSO linker script — LoongArch 64. */

OUTPUT_ARCH(loongarch)
ENTRY(__vdso_rt_sigreturn)

SECTIONS
{
    . = SIZEOF_HEADERS;

    .hash           : { *(.hash) }              :text
    .gnu.hash       : { *(.gnu.hash) }
    .dynsym         : { *(.dynsym) }
    .dynstr         : { *(.dynstr) }
    .gnu.version    : { *(.gnu.version) }
    .gnu.version_d  : { *(.gnu.version_d) }
    .gnu.version_r  : { *(.gnu.version_r) }
    .dynamic        : { *(.dynamic) }           :text :dynamic

    .rodata         : { *(.rodata .rodata.* .gnu.linkonce.r.*) } :text

    .note           : { *(.note.*) }            :text :note

    .eh_frame_hdr   : { *(.eh_frame_hdr) }      :text :eh_frame_hdr
    .eh_frame       : { KEEP (*(.eh_frame)) }   :text

    . = ALIGN(0x1000);

    .text           : { *(.text*) }             :text

    /DISCARD/       : {
        *(.note.GNU-stack)
        *(.data .data.* .gnu.linkonce.d.*)
        *(.bss .bss.* .gnu.linkonce.b.*)
        *(.got .got.plt)
        *(.plt .plt.* .iplt)
        *(.rela.* .rel.*)
    }
}

PHDRS
{
    text         PT_LOAD    FLAGS(5) FILEHDR PHDRS;
    dynamic      PT_DYNAMIC FLAGS(4);
    note         PT_NOTE    FLAGS(4);
    eh_frame_hdr PT_GNU_EH_FRAME;
}
