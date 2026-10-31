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
//! The caller opens the SMAP window (`with_user_access`) and has checked the
//! range is in the user half (`validate_user_buf`): this routine only makes
//! the fault on a valid-looking address survivable. Kill-switch:
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
.text
"#
);

unsafe extern "C" {
    fn neo_user_copy(dst: *mut u8, src: *const u8, len: usize) -> usize;
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
