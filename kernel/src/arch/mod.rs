// SPDX-License-Identifier: MPL-2.0
//! Architecture-specific code. For now, only x86_64.
//!
//! Phase 1.0 brings up GDT/TSS/IDT via `init_traps()`. Phase 5+ may add
//! aarch64 (Pi 5 target); when it does, every fn here will dispatch on
//! `cfg(target_arch)` and route into the per-arch submodule.

pub mod x86_64;

/// Halt the CPU forever. Used as the Phase 0/1 idle loop and from fatal
/// trap handlers. Disables interrupts then `hlt` in a loop.
#[inline(always)]
pub fn halt_forever() -> ! {
    x86_64::halt_forever()
}

/// Enable early CPU features in the control registers. See
/// `x86_64::enable_sse` — must be called very early in `_kernel_start` before
/// any vectorizable code.
///
/// # Safety
/// Touches CR0/CR4. Caller must hold exclusive control of the CPU (true
/// during single-threaded boot).
#[inline(always)]
pub unsafe fn enable_sse() {
    // The boot CPU publishes the FPU save policy, so it is always admitted.
    let admitted = unsafe { x86_64::enable_sse() };
    debug_assert!(admitted);
}

/// Install the GDT (with TSS) and IDT. After this returns, CPU exceptions
/// are caught by our handlers. Interrupts remain MASKED — call
/// `enable_interrupts()` separately when you're ready to take them.
#[inline(always)]
pub fn init_traps() {
    x86_64::init_traps()
}

/// Allow maskable interrupts (sti). The IDT must already be loaded via
/// `init_traps()`.
#[inline(always)]
pub fn enable_interrupts() {
    x86_64::enable_interrupts()
}

/// Trigger a software breakpoint (`int3` → vector 3 → #BP). The breakpoint
/// handler prints a diagnostic and returns; the kernel continues normal
/// execution. Used as a smoke test that the IDT works.
#[inline(always)]
pub fn breakpoint() {
    x86_64::breakpoint()
}
