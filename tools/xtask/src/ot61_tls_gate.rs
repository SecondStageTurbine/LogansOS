// SPDX-License-Identifier: MPL-2.0
//! OT-61 C1.2 — the key-only TLS gate (ratified amendment P3).
//!
//! The LogansOS loader maps `PT_LOAD` segments only (`kernel/src/elf.rs`); it
//! never builds a per-thread `.tdata`/`.tbss` block or a static TLS layout. A
//! user ELF that carries native TLS therefore runs with every `__thread` /
//! `#[thread_local]` access resolving against an FS.base that holds a key
//! block, not that variable: silently wrong. Worse, such an ELF links without
//! complaint (C1.2's spike: a planted `#[thread_local]` and a C `__thread` both
//! link statically). So the rule is enforced here, on the final ELF, before
//! xtask stages it: **thread-locals use keys (`libneo::ntls`, std's `os` key
//! route), never native TLS.**
//!
//! [`native_tls_findings`] reports, for one ELF64 little-endian image:
//! - a `PT_TLS` program header;
//! - a section with `SHF_TLS` (`.tdata`, `.tbss`);
//! - an `STT_TLS` symbol in any symbol table;
//! - an x86-64 TLS relocation in any `SHT_RELA` section.
//!
//! It does not flag `fs:`-relative instructions: the key layer's own
//! `mov reg, fs:[0]` is one. Native TLS always leaves one of the four marks
//! above, which a key-only build never has.

const PT_TLS: u32 = 7;
const SHT_SYMTAB: u32 = 2;
const SHT_RELA: u32 = 4;
const SHT_DYNSYM: u32 = 11;
const SHF_TLS: u64 = 0x400;
const STT_TLS: u8 = 6;
/// x86-64 TLS relocation types (System V x86-64 psABI, Table 4.10).
const TLS_RELOCATIONS: &[(u32, &str)] = &[
    (16, "R_X86_64_DTPMOD64"),
    (17, "R_X86_64_DTPOFF64"),
    (18, "R_X86_64_TPOFF64"),
    (19, "R_X86_64_TLSGD"),
    (20, "R_X86_64_TLSLD"),
    (21, "R_X86_64_DTPOFF32"),
    (22, "R_X86_64_GOTTPOFF"),
    (23, "R_X86_64_TPOFF32"),
    (34, "R_X86_64_GOTPC32_TLSDESC"),
    (35, "R_X86_64_TLSDESC_CALL"),
    (36, "R_X86_64_TLSDESC"),
];

fn u16_at(b: &[u8], off: usize) -> Result<u16, String> {
    b.get(off..off + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| format!("truncated ELF at {off:#x}"))
}

fn u32_at(b: &[u8], off: usize) -> Result<u32, String> {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| format!("truncated ELF at {off:#x}"))
}

fn u64_at(b: &[u8], off: usize) -> Result<u64, String> {
    b.get(off..off + 8)
        .map(|s| u64::from_le_bytes(s.try_into().unwrap()))
        .ok_or_else(|| format!("truncated ELF at {off:#x}"))
}

fn usize_at(b: &[u8], off: usize) -> Result<usize, String> {
    usize::try_from(u64_at(b, off)?).map_err(|_| format!("offset at {off:#x} too large"))
}

/// The NUL-terminated name at `off` in the string table at `strtab..strtab+len`.
fn name_at(b: &[u8], strtab: usize, len: usize, off: usize) -> String {
    if off >= len {
        return format!("<name {off:#x}>");
    }
    let start = strtab + off;
    let end = b[start..strtab + len.min(b.len() - strtab)]
        .iter()
        .position(|&c| c == 0)
        .map_or(strtab + len, |n| start + n);
    String::from_utf8_lossy(&b[start..end]).into_owned()
}

/// Every native-TLS mark in the ELF64 LE image `b`, one line each; empty when
/// it is key-only clean. `Err` when `b` is not a well-formed ELF64 LE image
/// (the gate then fails closed).
pub fn native_tls_findings(b: &[u8]) -> Result<Vec<String>, String> {
    if b.get(0..4) != Some(&b"\x7fELF"[..]) || b.get(4) != Some(&2) || b.get(5) != Some(&1) {
        return Err("not an ELF64 little-endian image".into());
    }
    let mut out = Vec::new();

    // Program headers: e_phoff @0x20, e_phentsize @0x36, e_phnum @0x38.
    let phoff = usize_at(b, 0x20)?;
    let phentsize = usize::from(u16_at(b, 0x36)?);
    let phnum = usize::from(u16_at(b, 0x38)?);
    for i in 0..phnum {
        let ph = phoff + i * phentsize;
        if u32_at(b, ph)? == PT_TLS {
            out.push(format!(
                "PT_TLS program header #{i} (memsz {:#x})",
                u64_at(b, ph + 0x28)?
            ));
        }
    }

    // Section headers: e_shoff @0x28, e_shentsize @0x3a, e_shnum @0x3c,
    // e_shstrndx @0x3e. A section header: sh_name @0, sh_type @4, sh_flags @8,
    // sh_offset @0x18, sh_size @0x20, sh_link @0x28, sh_entsize @0x38.
    let shoff = usize_at(b, 0x28)?;
    let shentsize = usize::from(u16_at(b, 0x3a)?);
    let shnum = usize::from(u16_at(b, 0x3c)?);
    let shstrndx = usize::from(u16_at(b, 0x3e)?);
    if shoff == 0 || shnum == 0 {
        return Ok(out);
    }
    let sh = |i: usize| shoff + i * shentsize;
    let (shstr_off, shstr_len) = if shstrndx < shnum {
        (
            usize_at(b, sh(shstrndx) + 0x18)?,
            usize_at(b, sh(shstrndx) + 0x20)?,
        )
    } else {
        (0, 0)
    };
    for i in 0..shnum {
        let h = sh(i);
        let name = name_at(b, shstr_off, shstr_len, u32_at(b, h)? as usize);
        let sh_type = u32_at(b, h + 4)?;
        let flags = u64_at(b, h + 8)?;
        let offset = usize_at(b, h + 0x18)?;
        let size = usize_at(b, h + 0x20)?;
        let link = u32_at(b, h + 0x28)? as usize;
        let entsize = usize_at(b, h + 0x38)?;
        if flags & SHF_TLS != 0 {
            out.push(format!("SHF_TLS section {name} (size {size:#x})"));
        }
        if (sh_type == SHT_SYMTAB || sh_type == SHT_DYNSYM) && entsize >= 24 && link < shnum {
            // Elf64_Sym: st_name @0 (u32), st_info @4 (u8); 24 bytes each.
            let (str_off, str_len) = (usize_at(b, sh(link) + 0x18)?, usize_at(b, sh(link) + 0x20)?);
            for s in 0..size / entsize {
                let sym = offset + s * entsize;
                let info = *b.get(sym + 4).ok_or("truncated symbol table")?;
                if info & 0xf == STT_TLS {
                    let sym_name = name_at(b, str_off, str_len, u32_at(b, sym)? as usize);
                    out.push(format!("STT_TLS symbol {sym_name} in {name}"));
                }
            }
        }
        if sh_type == SHT_RELA && entsize >= 24 {
            // Elf64_Rela: r_info @8, type in its low 32 bits; 24 bytes each.
            for r in 0..size / entsize {
                let rel_type = (u64_at(b, offset + r * entsize + 8)? & 0xffff_ffff) as u32;
                if let Some((_, tls_name)) = TLS_RELOCATIONS.iter().find(|(t, _)| *t == rel_type) {
                    out.push(format!("{tls_name} relocation in {name}"));
                }
            }
        }
    }
    Ok(out)
}

/// Fail with every finding if the ELF at `path` carries native TLS.
pub fn check_key_only(path: &std::path::Path) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let findings = native_tls_findings(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    if findings.is_empty() {
        return Ok(());
    }
    Err(format!(
        "OT-61 key-only TLS gate: {} carries native TLS, which the PT_LOAD-only loader never \
         instantiates; use TLS keys (libneo::ntls, std's os key route):\n  {}",
        path.display(),
        findings.join("\n  ")
    ))
}

/// The gate's oracle: ELF64 images laid out byte by byte here, from the ELF64
/// field offsets (System V gABI, chapter 4), never produced by the parser
/// under test.
#[cfg(test)]
mod tests {
    use super::native_tls_findings;

    fn put16(b: &mut [u8], off: usize, v: u16) {
        b[off..off + 2].copy_from_slice(&v.to_le_bytes());
    }
    fn put32(b: &mut [u8], off: usize, v: u32) {
        b[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn put64(b: &mut [u8], off: usize, v: u64) {
        b[off..off + 8].copy_from_slice(&v.to_le_bytes());
    }

    /// What to plant in the image.
    #[derive(Default, Clone, Copy)]
    struct Plant {
        tls_phdr: bool,
        tls_section: bool,
        tls_symbol: bool,
        tls_reloc: u32,
    }

    /// A 0x400-byte image:
    /// - 0x000 ELF header (64 bytes); 0x040 two program headers (56 each);
    /// - 0x100 .shstrtab "\0.text\0.tbss\0.symtab\0.strtab\0.rela\0\0"
    ///   (names at 1, 7, 13, 21, 29);
    /// - 0x140 .strtab "\0leak\0" (name at 1);
    /// - 0x160 .symtab: the null symbol, then one symbol named "leak";
    /// - 0x190 .rela: one Elf64_Rela;
    /// - 0x200 six section headers (64 each): null, .text, .tbss, .symtab,
    ///   .strtab, .rela, then .shstrtab is index 6.
    fn image(p: Plant) -> Vec<u8> {
        let mut b = vec![0u8; 0x400 + 7 * 64];
        b[0..4].copy_from_slice(b"\x7fELF");
        b[4] = 2; // ELFCLASS64
        b[5] = 1; // ELFDATA2LSB
        b[6] = 1; // EV_CURRENT
        put16(&mut b, 0x10, 2); // ET_EXEC
        put16(&mut b, 0x12, 62); // EM_X86_64
        put64(&mut b, 0x20, 0x40); // e_phoff
        put64(&mut b, 0x28, 0x400); // e_shoff
        put16(&mut b, 0x36, 56); // e_phentsize
        put16(&mut b, 0x38, 2); // e_phnum
        put16(&mut b, 0x3a, 64); // e_shentsize
        put16(&mut b, 0x3c, 7); // e_shnum
        put16(&mut b, 0x3e, 6); // e_shstrndx
        // phdr 0: PT_LOAD; phdr 1: PT_TLS or a second PT_LOAD.
        put32(&mut b, 0x40, 1);
        put32(&mut b, 0x40 + 56, if p.tls_phdr { 7 } else { 1 });
        put64(&mut b, 0x40 + 56 + 0x28, 0x10); // p_memsz
        b[0x100..0x124].copy_from_slice(b"\0.text\0.tbss\0.symtab\0.strtab\0.rela\0\0");
        b[0x140..0x146].copy_from_slice(b"\0leak\0");
        // Symbol 1 at 0x160 + 24: st_name 1, st_info: STT_TLS (6) or STT_OBJECT (1).
        put32(&mut b, 0x160 + 24, 1);
        b[0x160 + 24 + 4] = if p.tls_symbol { 6 } else { 1 };
        // The Elf64_Rela at 0x190: r_info = (sym 1 << 32) | type.
        put64(&mut b, 0x190 + 8, (1 << 32) | u64::from(p.tls_reloc.max(1)));
        // Section headers at 0x400 + 64 * i.
        let sh = |i: usize| 0x400 + 64 * i;
        let section = |b: &mut Vec<u8>, i, name, ty, flags, off, size, link, entsize| {
            put32(b, sh(i), name);
            put32(b, sh(i) + 4, ty);
            put64(b, sh(i) + 8, flags);
            put64(b, sh(i) + 0x18, off);
            put64(b, sh(i) + 0x20, size);
            put32(b, sh(i) + 0x28, link);
            put64(b, sh(i) + 0x38, entsize);
        };
        section(&mut b, 1, 1, 1, 0x6, 0x300, 0x10, 0, 0); // .text, AX
        // .tbss: SHT_NOBITS (8), WA plus SHF_TLS (0x400) when planted.
        section(
            &mut b,
            2,
            7,
            8,
            0x3 | if p.tls_section { 0x400 } else { 0 },
            0x310,
            0x10,
            0,
            0,
        );
        section(&mut b, 3, 13, 2, 0, 0x160, 48, 4, 24); // .symtab, links .strtab
        section(&mut b, 4, 21, 3, 0, 0x140, 6, 0, 0); // .strtab
        section(&mut b, 5, 29, 4, 0, 0x190, 24, 3, 24); // .rela
        section(&mut b, 6, 0, 3, 0, 0x100, 0x24, 0, 0); // .shstrtab
        b
    }

    #[test]
    fn a_key_only_image_is_clean() {
        assert_eq!(native_tls_findings(&image(Plant::default())), Ok(vec![]));
    }

    #[test]
    fn each_native_tls_mark_is_caught() {
        let pt = native_tls_findings(&image(Plant {
            tls_phdr: true,
            ..Plant::default()
        }))
        .unwrap();
        assert_eq!(
            pt,
            vec!["PT_TLS program header #1 (memsz 0x10)".to_string()]
        );
        let sec = native_tls_findings(&image(Plant {
            tls_section: true,
            ..Plant::default()
        }))
        .unwrap();
        assert_eq!(sec, vec!["SHF_TLS section .tbss (size 0x10)".to_string()]);
        let sym = native_tls_findings(&image(Plant {
            tls_symbol: true,
            ..Plant::default()
        }))
        .unwrap();
        assert_eq!(sym, vec!["STT_TLS symbol leak in .symtab".to_string()]);
        let rel = native_tls_findings(&image(Plant {
            tls_reloc: 23,
            ..Plant::default()
        }))
        .unwrap();
        assert_eq!(
            rel,
            vec!["R_X86_64_TPOFF32 relocation in .rela".to_string()]
        );
    }

    #[test]
    fn a_non_elf_fails_closed() {
        assert!(native_tls_findings(b"not an elf").is_err());
        let mut truncated = image(Plant::default());
        truncated.truncate(0x50);
        assert!(native_tls_findings(&truncated).is_err());
    }
}
