/* neo-worker linker script — Phase 1.5.
 * Same shape as neo-init's. Pinned at virt 0x400000 in the user half.
 */

ENTRY(_start)
OUTPUT_FORMAT(elf64-x86-64)

PHDRS
{
    text PT_LOAD FLAGS(5);    /* R + X */
    rodata PT_LOAD FLAGS(4);  /* R only */
    data PT_LOAD FLAGS(6);    /* R + W */
}

SECTIONS
{
    . = 0x400000;

    .text : ALIGN(0x1000) {
        *(.text .text.*)
    } :text

    .rodata : ALIGN(0x1000) {
        *(.rodata .rodata.*)
    } :rodata

    .data : ALIGN(0x1000) {
        *(.data .data.*)
    } :data

    .bss : ALIGN(0x1000) {
        *(.bss .bss.* COMMON)
    } :data

    /DISCARD/ : {
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.note .note.*)
        *(.comment)
    }
}
