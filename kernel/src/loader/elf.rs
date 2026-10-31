// SPDX-License-Identifier: MPL-2.0
//! Minimal ELF64 loader. Phase 1.4.
//!
//! Parses the static-linked ELF for `neo-init` (which Limine loaded as a
//! module) and maps its PT_LOAD segments into a fresh `Pmap`. Validates:
//!   - ELF64 magic + class + endianness + version
//!   - e_machine = AMD64 (0x3E)
//!   - e_type in {EXEC, DYN}  (we currently only really test EXEC)
//!   - All PT_LOAD virt addresses fit in user half (< 0x0000_8000_0000_0000)
//!   - Entry RIP falls inside one of the loaded segments
//!
//! Does NOT support: dynamic linking, relocations, PT_INTERP, PT_TLS,
//! sections (we only walk program headers). Phase 2+ will revisit.

use core::slice;

use x86_64::VirtAddr;
#[cfg(not(feature = "mem-quota"))]
use x86_64::structures::paging::FrameAllocator;
use x86_64::structures::paging::{PhysFrame, Size4KiB};

#[cfg(not(feature = "mem-quota"))]
use crate::mm::with_frame_alloc;
use crate::mm::{
    phys_to_virt,
    pmap::{Pmap, flags as pmap_flags},
};
use crate::serial_println;

const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const EV_CURRENT: u8 = 1;
const EM_X86_64: u16 = 0x3E;
const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;

const PT_LOAD: u32 = 1;
const PF_X: u32 = 1;
const PF_W: u32 = 2;

const USER_HALF_LIMIT: u64 = 0x0000_8000_0000_0000;

#[derive(Debug)]
pub enum ElfError {
    TooSmall,
    BadMagic,
    NotElf64,
    NotLittleEndian,
    BadVersion,
    NotX86_64,
    NotExecutable,
    BadProgramHeader,
    SegmentInKernelHalf,
    /// Per-segment p_flags has both PF_W and PF_X set. We refuse W+X mappings
    /// to enforce W^X — defends against malformed/malicious ELFs that would
    /// otherwise install writable+executable user pages.
    SegmentWriteAndExecute,
    EntryNotInLoadedSegment,
    OutOfFrames,
    PmapMapFailed,
}

#[repr(C)]
struct Elf64Header {
    ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

#[repr(C)]
pub(crate) struct Elf64ProgramHeader {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}

pub struct Elf<'a> {
    bytes: &'a [u8],
    header: &'a Elf64Header,
}

impl<'a> Elf<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, ElfError> {
        if bytes.len() < core::mem::size_of::<Elf64Header>() {
            return Err(ElfError::TooSmall);
        }
        // SAFETY: size check above guarantees the read is in-bounds. The
        // `Elf64Header` struct is `#[repr(C)]` and matches the on-disk
        // little-endian layout that ELF64-LE produces.
        let header = unsafe { &*(bytes.as_ptr() as *const Elf64Header) };

        if header.ident[..4] != ELF_MAGIC {
            return Err(ElfError::BadMagic);
        }
        if header.ident[4] != ELFCLASS64 {
            return Err(ElfError::NotElf64);
        }
        if header.ident[5] != ELFDATA2LSB {
            return Err(ElfError::NotLittleEndian);
        }
        if header.ident[6] != EV_CURRENT {
            return Err(ElfError::BadVersion);
        }
        if header.e_machine != EM_X86_64 {
            return Err(ElfError::NotX86_64);
        }
        if header.e_type != ET_EXEC && header.e_type != ET_DYN {
            return Err(ElfError::NotExecutable);
        }

        Ok(Self { bytes, header })
    }

    pub fn entry_rip(&self) -> u64 {
        self.header.e_entry
    }

    pub fn program_headers(&self) -> Result<&'a [Elf64ProgramHeader], ElfError> {
        let phoff = self.header.e_phoff as usize;
        let phnum = self.header.e_phnum as usize;
        let phentsize = self.header.e_phentsize as usize;
        if phentsize != core::mem::size_of::<Elf64ProgramHeader>() {
            return Err(ElfError::BadProgramHeader);
        }
        let total = phnum
            .checked_mul(phentsize)
            .ok_or(ElfError::BadProgramHeader)?;
        if phoff.checked_add(total).ok_or(ElfError::BadProgramHeader)? > self.bytes.len() {
            return Err(ElfError::BadProgramHeader);
        }
        // SAFETY: bounds + size checks above. Repr(C) layout match.
        let ph_ptr = unsafe { self.bytes.as_ptr().add(phoff) as *const Elf64ProgramHeader };
        let phs = unsafe { slice::from_raw_parts(ph_ptr, phnum) };
        Ok(phs)
    }

    /// Walk PT_LOAD segments, allocate user-half frames for each, copy file
    /// bytes via HHDM, and install mappings in `pmap` with per-segment flags.
    /// Tail bytes (mem_sz - file_sz) are zeroed for .bss.
    pub fn load_into(&self, pmap: &Pmap) -> Result<u64, ElfError> {
        let entry = self.header.e_entry;
        let mut entry_in_segment = false;
        let mut segments_loaded = 0u32;

        for ph in self.program_headers()? {
            if ph.p_type != PT_LOAD {
                continue;
            }

            let vaddr = ph.p_vaddr;
            let memsz = ph.p_memsz;
            let filesz = ph.p_filesz;
            let offset = ph.p_offset as usize;

            if vaddr >= USER_HALF_LIMIT
                || vaddr.checked_add(memsz).is_none_or(|e| e > USER_HALF_LIMIT)
            {
                return Err(ElfError::SegmentInKernelHalf);
            }

            // W^X enforcement — refuse segments that are simultaneously
            // writable AND executable. Modern ELFs never produce these;
            // a PT_LOAD with both bits set is either malformed or hostile.
            if (ph.p_flags & (PF_W | PF_X)) == (PF_W | PF_X) {
                return Err(ElfError::SegmentWriteAndExecute);
            }

            // Page-align the virt range we'll map.
            let vaddr_start = vaddr & !0xFFF;
            let vaddr_end = (vaddr + memsz + 0xFFF) & !0xFFF;
            let pages = ((vaddr_end - vaddr_start) / 0x1000) as usize;

            // Compose flags via the precomposed Pmap constants. After the
            // W^X check above, the three legal cases are: (X,!W) → CODE,
            // (!X, W) → DATA_RW, (!X, !W) → DATA_RO.
            let flags = if (ph.p_flags & PF_X) != 0 {
                pmap_flags::USER_CODE
            } else if (ph.p_flags & PF_W) != 0 {
                pmap_flags::USER_DATA_RW
            } else {
                pmap_flags::USER_DATA_RO
            };

            // For each page in the segment: allocate a frame, copy/zero
            // the appropriate slice of file bytes, install mapping.
            for page_idx in 0..pages {
                #[cfg(not(feature = "mem-quota"))]
                let frame: PhysFrame<Size4KiB> =
                    with_frame_alloc(|fa| fa.allocate_frame()).ok_or(ElfError::OutOfFrames)?;
                // OT-61 C2.2: charged to the process before it is allocated.
                // #143: `page` frees and releases it on any return before
                // the map below succeeds.
                #[cfg(feature = "mem-quota")]
                let page = pmap.alloc_user_frame().map_err(|_| ElfError::OutOfFrames)?;
                #[cfg(feature = "mem-quota")]
                let frame: PhysFrame<Size4KiB> = page.frame();

                // Zero the frame first (covers .bss tail and gaps).
                // SAFETY: HHDM-mapped, freshly-allocated, owned exclusively.
                unsafe {
                    let dst = phys_to_virt(frame.start_address()).as_mut_ptr::<u8>();
                    core::ptr::write_bytes(dst, 0, 0x1000);
                }

                // Compute the file-byte slice for this page.
                let page_va = vaddr_start + (page_idx as u64) * 0x1000;
                // Bytes in this page that come FROM the file:
                //   start in segment = max(page_va - vaddr, 0) but page_va >= vaddr_start <= vaddr
                //   end in segment = min(page_va + 4096, vaddr + filesz)
                let seg_byte_start = page_va.saturating_sub(vaddr);
                let seg_byte_end = (page_va + 0x1000).saturating_sub(vaddr).min(filesz);
                if seg_byte_end > seg_byte_start {
                    let copy_len = (seg_byte_end - seg_byte_start) as usize;
                    let src_off = offset + seg_byte_start as usize;
                    if src_off + copy_len > self.bytes.len() {
                        return Err(ElfError::BadProgramHeader);
                    }
                    let dst_off_in_page = (page_va.max(vaddr) - page_va) as usize;
                    // SAFETY: HHDM-mapped frame is exclusively ours until we
                    // install the mapping below. Treat it as a 4 KiB &mut [u8].
                    let dst_page: &mut [u8; 0x1000] = unsafe {
                        &mut *(phys_to_virt(frame.start_address()).as_mut_ptr::<[u8; 0x1000]>())
                    };
                    dst_page[dst_off_in_page..dst_off_in_page + copy_len]
                        .copy_from_slice(&self.bytes[src_off..src_off + copy_len]);
                }

                pmap.map_user(VirtAddr::new(page_va), frame, flags)
                    .map_err(|_| ElfError::PmapMapFailed)?;
                #[cfg(feature = "mem-quota")]
                page.mapped();
            }

            // Entry can validly fall in the .bss tail of a segment (memsz >
            // filesz) per the ELF spec — use memsz, not filesz, for the
            // containment check. Phase 1.4's neo-init has entry in .text
            // which is always file-backed, but be spec-correct anyway.
            if entry >= vaddr && entry < vaddr + memsz {
                entry_in_segment = true;
            }
            segments_loaded += 1;
        }

        if !entry_in_segment {
            return Err(ElfError::EntryNotInLoadedSegment);
        }

        serial_println!(
            "[ kernel ] elf: loaded entry=0x{:016x} segments={}",
            entry,
            segments_loaded
        );
        Ok(entry)
    }
}

/// Allocate and map a user stack of `size` bytes ending at `top` (exclusive).
/// Used by `process::spawn_user` to give the new user thread a stack.
/// Returns the bottom-of-stack virt addr (caller can compute initial RSP =
/// top - 16 for SysV alignment).
pub fn allocate_user_stack(pmap: &Pmap, top: VirtAddr, size: usize) -> Result<VirtAddr, ElfError> {
    assert!(
        size.is_multiple_of(0x1000),
        "user stack size must be page-multiple"
    );
    let bottom = VirtAddr::new(top.as_u64() - size as u64);
    let pages = size / 0x1000;
    let flags = pmap_flags::USER_DATA_RW;
    for i in 0..pages {
        #[cfg(not(feature = "mem-quota"))]
        let frame: PhysFrame<Size4KiB> =
            with_frame_alloc(|fa| fa.allocate_frame()).ok_or(ElfError::OutOfFrames)?;
        // OT-61 C2.2: charged to the process before it is allocated.
        // #143: freed and released if the map below fails.
        #[cfg(feature = "mem-quota")]
        let page = pmap.alloc_user_frame().map_err(|_| ElfError::OutOfFrames)?;
        #[cfg(feature = "mem-quota")]
        let frame: PhysFrame<Size4KiB> = page.frame();
        // Zero the stack page (catches stale data from previous boot).
        // SAFETY: HHDM-mapped, freshly-allocated.
        unsafe {
            let dst = phys_to_virt(frame.start_address()).as_mut_ptr::<u8>();
            core::ptr::write_bytes(dst, 0, 0x1000);
        }
        let page_va = VirtAddr::new(bottom.as_u64() + (i as u64) * 0x1000);
        pmap.map_user(page_va, frame, flags)
            .map_err(|_| ElfError::PmapMapFailed)?;
        #[cfg(feature = "mem-quota")]
        page.mapped();
    }
    Ok(bottom)
}
