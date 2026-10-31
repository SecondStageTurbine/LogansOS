/* NeoPlutarch Kernel Linker Script
 *
 * Higher-half kernel layout. The preferred link base is the canonical
 * higher-half address (0xffffffff80000000), but BUILD-3 links as PIE so
 * Limine can relocate the image when `kaslr: yes` is active.
 *
 * The Limine boot protocol scans the kernel image for a request block
 * delimited by `.requests_start_marker` and `.requests_end_marker`.
 * Both markers and the requests themselves must survive LTO and end
 * up in a loaded segment in the order: start, requests, end.
 */

OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_kernel_start)

PHDRS
{
    text    PT_LOAD FLAGS((1 << 0) | (1 << 2)) ;  /* Execute + Read */
    rodata  PT_LOAD FLAGS((1 << 2)) ;             /* Read */
    data    PT_LOAD FLAGS((1 << 1) | (1 << 2)) ;  /* Write + Read */
    dynamic PT_DYNAMIC FLAGS((1 << 1) | (1 << 2)) ;
}

SECTIONS
{
    /* Higher-half preferred base: -2 GiB. KASLR may relocate at boot. */
    . = 0xffffffff80000000;
    __kernel_image_start = .;

    .text : {
        __kernel_text_start = .;
        *(.text .text.*)
        __kernel_text_end = .;
    } :text

    . = ALIGN(CONSTANT(MAXPAGESIZE));

    .rodata : {
        *(.rodata .rodata.*)
    } :rodata

    /* Limine request block. Each marker and the requests themselves get
     * their own section + KEEP() so the linker preserves order and LTO
     * cannot drop them. Limine reads the start marker, walks forward
     * collecting requests, and stops at the end marker. */
    .requests_start_marker : {
        KEEP(*(.requests_start_marker))
    } :rodata

    .requests : {
        KEEP(*(.requests))
    } :rodata

    .requests_end_marker : {
        KEEP(*(.requests_end_marker))
    } :rodata

    .dynsym : {
        *(.dynsym)
    } :rodata

    .dynstr : {
        *(.dynstr)
    } :rodata

    .hash : {
        *(.hash)
    } :rodata

    .gnu.hash : {
        *(.gnu.hash)
    } :rodata

    .rela.dyn : {
        *(.rela.dyn)
    } :rodata

    . = ALIGN(CONSTANT(MAXPAGESIZE));

    .data : {
        *(.data .data.*)
    } :data

    .got : {
        *(.got .got.*)
    } :data

    .got.plt : {
        *(.got.plt)
    } :data

    .dynamic : {
        *(.dynamic)
    } :data :dynamic

    .bss : {
        *(.bss .bss.*)
        *(COMMON)
    } :data

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __kernel_image_end = .;

    /DISCARD/ : {
        *(.eh_frame)
        *(.note .note.*)
    }
}
