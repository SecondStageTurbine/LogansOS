// SPDX-License-Identifier: MPL-2.0
//! The kernel's one routine that touches user memory with a fault fixup.
//!
//! `copy` moves bytes with a single `rep movsb`. If that instruction faults
//! in ring 0 on a user address (a page that is unmapped, or not writable, or
//! unmapped by another thread mid-copy), the page-fault entry shim
//! (`idt::page_fault_shim`) recognises its RIP, rewrites the saved RIP to
//! `neo_user_copy_fixup` and returns there: `copy` then returns the bytes it
//! did not copy (RCX, which `rep movsb` keeps current). Any other kernel-mode
//! fault still halts. This is the shape of Redox's `arch_copy_to_user` (a
//! RIP-ranged `rep movsb`) and FreeBSD's `pcb_onfault`, read for principle.
//!
//! `load_u32` is the second registered instruction: one aligned 4-byte load
//! (`mov eax, [rdi]`), for the futex word's compare, which must read the word
//! in one access, not byte by byte. It shares the fixup: its RCX holds 4 (the
//! bytes not loaded) before the load, so a fault returns nonzero.
//!
//! The caller has checked the range is in the user half
//! (`validate_user_buf`); `try_copy` and `try_load_u32` open the SMAP window
//! (`with_user_access`) around the one access. This only makes the fault on
//! a valid-looking address survivable. Kill-switch:
//! `user-copy-fixup` (off: the shim does not look, and a fault halts, as
//! before).

core::arch::global_asm!(
    r#"
.section .text.neo_user_copy,"ax",@progbits
.global neo_user_copy
.type neo_user_copy, @function
neo_user_copy:
    mov rcx, rdx
.global neo_user_copy_insn
neo_user_copy_insn:
    rep movsb
    xor eax, eax
    ret
.global neo_user_copy_fixup
neo_user_copy_fixup:
    mov rax, rcx
    ret
.size neo_user_copy, . - neo_user_copy
.global neo_user_load_u32
.type neo_user_load_u32, @function
neo_user_load_u32:
    mov ecx, 4
.global neo_user_load_u32_insn
neo_user_load_u32_insn:
    mov eax, dword ptr [rdi]
    mov dword ptr [rsi], eax
    xor eax, eax
    ret
.size neo_user_load_u32, . - neo_user_load_u32
.text
"#
);

unsafe extern "C" {
    fn neo_user_copy(dst: *mut u8, src: *const u8, len: usize) -> usize;
    // The futex is its only caller.
    #[cfg(feature = "browser-runtime")]
    fn neo_user_load_u32(src: *const u32, out: *mut u32) -> usize;
}

/// Copy `len` bytes from `src` to `dst`, one of them user memory. Returns
/// the number of bytes NOT copied: 0 on success, nonzero when a user page
/// faulted (the copy stops at the first faulting byte).
///
/// # Safety
///
/// - The kernel side is a valid buffer of `len` bytes.
/// - The user side is in the user half (`validate_user_buf`), and the call
///   runs inside `with_user_access`.
/// - The two don't overlap.
#[inline]
pub unsafe fn copy(dst: *mut u8, src: *const u8, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    // SAFETY: per this function's contract; `neo_user_copy` is a SysV
    // function that clobbers rcx, rsi, rdi and rax only, with DF clear
    // (the ABI's state at a call).
    unsafe { neo_user_copy(dst, src, len) }
}

/// `copy` inside its own SMAP window: `true` when all `len` bytes moved,
/// `false` when a user page faulted (some prefix may have moved).
///
/// # Safety
///
/// As `copy`, except that the caller must NOT be inside `with_user_access`
/// (this opens it; the window refuses to nest, CP-49).
pub unsafe fn try_copy(dst: *mut u8, src: *const u8, len: usize) -> bool {
    // SAFETY: per this function's contract.
    crate::arch::x86_64::with_user_access(|| unsafe { copy(dst, src, len) }) == 0
}

/// `copy` inside its own SMAP window, returning how many bytes moved (all
/// `len`, or the prefix before the first faulting byte), for a caller that
/// must report a partial count (`SYS_PIPE_READ`, Chad #56 item 5).
///
/// # Safety
///
/// As `try_copy`.
#[cfg(feature = "pipe")]
pub unsafe fn try_copy_count(dst: *mut u8, src: *const u8, len: usize) -> usize {
    // SAFETY: per this function's contract.
    let left = crate::arch::x86_64::with_user_access(|| unsafe { copy(dst, src, len) });
    len - left
}

/// Read the user word at `src` with one aligned 4-byte load, in its own SMAP
/// window: `None` when its page faults.
///
/// # Safety
///
/// `src` is 4-aligned and in the user half (`validate_user_buf`); the caller
/// is not inside `with_user_access`.
#[cfg(feature = "browser-runtime")]
pub unsafe fn try_load_u32(src: *const u32) -> Option<u32> {
    let mut out = 0u32;
    // SAFETY: per this function's contract; `neo_user_load_u32` is a SysV
    // function that clobbers rcx and rax only and writes `out`.
    let left =
        crate::arch::x86_64::with_user_access(|| unsafe { neo_user_load_u32(src, &mut out) });
    if left == 0 { Some(out) } else { None }
}
