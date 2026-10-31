/* neo-init linker script — Phase 1.4.
 *
 * The kernel's ELF loader maps PT_LOAD segments at the virtual addresses
 * encoded in the ELF program headers. We pin the binary at virt 0x400000
 * (4 MiB into the user half) — well above the canonical lower-half
 * boundary (0) and well below the user-stack region (~0x7fff_*).
 *
 * The kernel-side ELF loader expects PT_LOAD entries with sane p_vaddr,
 * p_filesz, p_memsz, and per-segment p_flags (PF_X / PF_W / PF_R). The
 * standard rustc + lld output for a no_std executable produces those.
 *
 * No .got / .got.plt: code-model=small + relocation-model=static gives us
 * direct relocations, no PLT/GOT.
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
