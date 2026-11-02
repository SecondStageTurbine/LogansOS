// SPDX-License-Identifier: MPL-2.0
//! x86_64-specific primitives and CPU bring-up.
//!
//! Phase 0 only had `halt_forever` and `enable_sse`. Phase 1.0 adds GDT, TSS,
//! and IDT bring-up via the `gdt` and `idt` submodules, plus `init_traps()`
//! to wire them up in the right order (GDT first, then IDT).
//!
//! Phase 5+ may add aarch64 if we target the Pi 5; everything in this module
//! is x86-only and will get gated behind `#[cfg(target_arch = "x86_64")]`
//! when that lands.

use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

use x86_64::VirtAddr;
use x86_64::instructions::interrupts;
use x86_64::registers::control::Cr3;

pub mod apic;
pub mod apic_logic;
pub mod context;
/// Arthur latency round 2: APERF/MPERF evidence and the HWP request.
#[cfg(any(feature = "prefill-perf", feature = "hwp-performance"))]
pub mod cpufreq;
#[cfg(any(feature = "prefill-perf", feature = "hwp-performance"))]
pub mod cpufreq_logic;
#[cfg(feature = "xsave-fpu")]
pub mod fpu;
#[cfg(feature = "xsave-fpu")]
pub mod fpu_logic;
pub mod gdt;
pub mod idt;
/// i8042 lane: one I/O APIC redirection entry for an ISA IRQ (IRQ1).
#[cfg(any(feature = "ps2-keyboard", feature = "i2c-hid-touchpad"))]
pub mod ioapic;
#[cfg(any(feature = "ps2-keyboard", feature = "i2c-hid-touchpad"))]
#[path = "ioapic_logic.rs"]
pub mod ioapic_logic;
#[cfg(feature = "lapic-timer")]
pub mod lapic_timer;
#[cfg(feature = "lapic-timer")]
pub mod lapic_timer_logic;
pub mod per_cpu;
pub mod pic;
pub mod pit;
#[cfg(feature = "lapic-timer")]
pub mod refclock;
pub mod smp;
#[allow(clippy::uninit_vec, clippy::collapsible_if, clippy::useless_conversion)]
// syscall.rs: uninit_vec is intentional user-memory fill; collapsible_if/useless_conversion deferred
pub mod syscall;
pub mod timer;
pub mod user_copy;
// OT-85: the boot-time RTC epoch seed (CLOCK_REALTIME's base).
#[cfg(feature = "rtc-epoch")]
pub mod rtc;
#[cfg(feature = "rtc-epoch")]
#[cfg_attr(feature = "rtc-epoch-force-invalid", allow(dead_code))]
pub mod rtc_logic;

const CR4_OSFXSR: u64 = 1 << 9;
const CR4_OSXMMEXCPT: u64 = 1 << 10;
const CR4_SMEP: u64 = 1 << 20;
const CR4_SMAP: u64 = 1 << 21;
const CR4_OSXSAVE: u64 = 1 << 18;

const CPUID_LEAF7_EBX_SMEP: u32 = 1 << 7;
const CPUID_LEAF7_EBX_SMAP: u32 = 1 << 20;

static SMAP_ACTIVE: AtomicBool = AtomicBool::new(false);

/// MXCSR the kernel's SSE code runs under: every exception masked, round to
/// nearest, FTZ and DAZ off (the power-on value, and what LLVM assumes).
pub const KERNEL_MXCSR: u32 = 0x1F80;
/// x87 control word after `fninit`: every exception masked, round to nearest,
/// 64-bit precision.
pub const KERNEL_X87_FCW: u16 = 0x037F;

/// Memory operand for `ldmxcsr` in the hand-written entry stubs (CP-41).
#[unsafe(no_mangle)]
static KERNEL_DEFAULT_MXCSR: u32 = KERNEL_MXCSR;

/// Put x87 and SSE control state back to the kernel defaults: `fninit` (default
/// control word, empty register stack, no pending exceptions) then
/// `ldmxcsr KERNEL_MXCSR`. Ring 3 owns both registers, so kernel float code
/// must not run on whatever the user left there (CP-41). For the cold paths:
/// the syscall, timer and `swapgs_irq_entry` stubs inline only the `ldmxcsr`,
/// because the kernel contains no x87 instructions and FNINIT costs ~70
/// cycles. Kernel code that ever uses x87 must `fninit` first.
#[inline(always)]
pub fn load_kernel_fp_control() {
    // SAFETY: both instructions only write x87/SSE state; FNINIT empties the
    // x87 register stack, which is declared clobbered. FNINIT is non-waiting,
    // so a pending unmasked x87 exception left by ring 3 cannot fault here,
    // and KERNEL_MXCSR sets no reserved bits.
    unsafe {
        asm!(
            "fninit",
            "ldmxcsr [{}]",
            in(reg) &KERNEL_DEFAULT_MXCSR,
            out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
            out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
            options(nostack, preserves_flags)
        );
    }
}

/// Disable interrupts and `hlt` forever. Used as the Phase 0/1 idle loop and
/// from fatal trap handlers. Returns `!` because there is no path back: even
/// an NMI just resumes the loop.
#[inline(always)]
pub fn halt_forever() -> ! {
    unsafe {
        asm!("cli", options(nomem, nostack, preserves_flags));
        loop {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

/// Turn on early CPU features in CR0 + CR4:
///
/// - SSE/SSE2 OS support bits, so compiler-emitted vector instructions do not
///   #UD on first use.
/// - SMEP/SMAP when the CPU advertises them, closing ADR-003's cheap security
///   debt before Phase 2 starts.
///
/// Limine hands control to the kernel without committing to specific CR0/CR4
/// state for SIMD, and rustc compiles for `x86_64-unknown-none` which has SSE2
/// on by default — so the kernel has to flip the OS-supports-SSE bits itself
/// before any compiled function might emit a `movdqa` / `xorps` / etc.
///
/// MUST be called before any code that LLVM might vectorize. In practice that
/// means: very early in `_kernel_start`, right after `serial::init()`.
#[inline(always)]
pub unsafe fn enable_sse() {
    let mut cr0: u64;
    // SAFETY: caller guarantees early single-CPU boot ownership of CR0/CR4.
    unsafe {
        asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
        cr0 &= !(1 << 2); // CR0.EM = 0  (no x87 emulation — use real FPU)
        cr0 |= 1 << 1; // CR0.MP = 1  (monitor coprocessor)
        // CR0.AM = 0: ring 3 can set RFLAGS.AC, which must stay inert (no #AC,
        // which would halt this CPU). Not left to the bootloader (CP-40).
        cr0 &= !(1 << 18);
        // CR0.NE = 1: x87 errors are reported natively as #MF (OT-83), which a
        // ring-3 fault kills the process for. Left to the bootloader, NE could
        // be 0, and then an unmasked ring-3 x87 error goes out on the legacy
        // FERR#/IRQ13 path, which nothing services: the error is silently lost.
        cr0 |= 1 << 5;
        asm!("mov cr0, {}", in(reg) cr0, options(nomem, nostack, preserves_flags));
    }

    let mut cr4 = read_cr4_raw();
    cr4 |= CR4_OSFXSR; // OS uses FXSAVE/FXRSTOR.
    cr4 |= CR4_OSXMMEXCPT; // OS handles SIMD FP exceptions.
    // CR4.OSXSAVE is what tells ring 3 (CPUID.1:ECX.OSXSAVE, then XCR0) that
    // AVX state is usable, and std's `is_x86_feature_detected!` (memchr's
    // AVX2 choice, OT-61 C2.3b) trusts it. Set it only when the context switch
    // saves that state: `xsave-fpu` sets it again below once XSAVE is
    // confirmed. Not left to the firmware or the bootloader, which may hand
    // over OSXSAVE with YMM enabled while this kernel saves only FXSAVE state.
    cr4 &= !CR4_OSXSAVE;

    let has_smep = cpu_has_smep();
    let has_smap = cpu_has_smap();

    if has_smep {
        cr4 |= CR4_SMEP;
    }
    if has_smap {
        cr4 |= CR4_SMAP;
    }

    #[cfg(feature = "xsave-fpu")]
    let xsave_requested = fpu::prepare_cr4_for_xsave(&mut cr4);

    // SAFETY: caller guarantees early CPU-local ownership of CR4. With
    // `xsave-fpu`, `prepare_cr4_for_xsave` only adds CR4.OSXSAVE after CPUID
    // confirms the CPU can accept the subsequent XCR0 x87|SSE|AVX mask.
    unsafe {
        write_cr4_raw(cr4);
    }

    #[cfg(feature = "xsave-fpu")]
    fpu::finish_xsave_init(xsave_requested);

    // Don't trust the bootloader (BSP) or the AP trampoline to hand over the
    // power-on x87/MXCSR state: the adopted boot thread (and, without
    // `xsave-fpu`, every kernel thread) runs on whatever is live here. With
    // `xsave-fpu`, new threads' images are constructed by
    // `FpuState::new_initial` instead (CP-45).
    load_kernel_fp_control();

    if cr4 & CR4_SMAP != 0 {
        clac();
        SMAP_ACTIVE.store(true, Ordering::Release);
    }

    #[cfg(feature = "xsave-fpu")]
    crate::serial_println!(
        "[ kernel ] cpu: FPU save={} area={} mask=0x{:x} cpuid_0d_ebx={}{}",
        if fpu::xsave_active() {
            "xsave"
        } else {
            "fxsave"
        },
        fpu::save_area_size(),
        fpu::xstate_mask(),
        fpu::ebx_report().0,
        // CPUID.(0DH,0):EBX smaller than the enabled components need: the
        // area is sized from the components instead (`fpu_logic`).
        if fpu::ebx_report().1 {
            " (understated; sized from the components)"
        } else {
            ""
        }
    );

    crate::serial_println!(
        "[ kernel ] cpu: SSE/SSE2 ready; SMEP={} SMAP={}",
        smep_enabled(),
        smap_enabled()
    );

    // Arthur latency (NOTE fix 2): this CPU's own AVX2 answer, now that its
    // XCR0 is final and before it can poll for or run any matmul. A "no" from
    // any CPU decides "no" for all; "yes" needs every online CPU (the census in
    // `ops::qk_matmul`, published after `bring_up_aps`).
    #[cfg(any(feature = "phase9-quantized-matmul", feature = "phase9-real-quant"))]
    crate::ai::ops::qk_matmul::report_cpu_avx2();
    // Arthur latency round 2: HWP with the performance preference, on every
    // CPU as it starts (IA32_PM_ENABLE and IA32_HWP_REQUEST are per logical
    // CPU). Off ⇒ not compiled, and the kernel never writes a power MSR.
    #[cfg(feature = "hwp-performance")]
    cpufreq::hwp_performance();
}

#[inline]
pub fn cpu_has_smep() -> bool {
    cpuid_leaf7_ebx() & CPUID_LEAF7_EBX_SMEP != 0
}

#[inline]
pub fn cpu_has_smap() -> bool {
    cpuid_leaf7_ebx() & CPUID_LEAF7_EBX_SMAP != 0
}

#[inline]
pub fn smep_enabled() -> bool {
    read_cr4_raw() & CR4_SMEP != 0
}

#[inline]
pub fn smap_enabled() -> bool {
    read_cr4_raw() & CR4_SMAP != 0
}

#[inline]
pub fn read_cr4_raw() -> u64 {
    let cr4: u64;
    unsafe {
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
    }
    cr4
}

#[inline]
pub unsafe fn write_cr4_raw(cr4: u64) {
    unsafe {
        asm!("mov cr4, {}", in(reg) cr4, options(nomem, nostack, preserves_flags));
    }
}

/// Reload CR3 with its current value to flush local TLB/paging-structure
/// caches after mutating the active address space.
#[inline]
pub fn flush_local_tlb() {
    let (frame, flags) = Cr3::read();
    unsafe {
        Cr3::write(frame, flags);
    }
}

/// Invalidate the local TLB entry for `virt_addr`.
#[inline]
pub fn invlpg(virt_addr: VirtAddr) {
    // SAFETY: this kernel helper is only callable at CPL=0. `VirtAddr`
    // construction enforces canonical-address form, and INVLPG is defined for
    // any canonical linear address regardless of whether it is currently
    // mapped.
    unsafe {
        asm!(
            "invlpg [{}]",
            in(reg) virt_addr.as_u64(),
            options(nostack, preserves_flags)
        );
    }
}

/// Run `f` with SMAP lifted (`stac` … `clac`) so it can touch user memory.
///
/// The window does not nest and must not be held across a thread switch
/// (CP-49):
/// - `f` must not call `with_user_access`. The inner `clac` would close the
///   window while `f` still has user accesses to make.
/// - `f` must not block, yield or otherwise reach `context_switch`, which does
///   not save RFLAGS: the next thread would run with AC=1 (SMAP off) and this
///   one would resume with that thread's AC.
///
/// Outside these windows kernel code runs with AC=0: SYSCALL masks it through
/// SFMASK, and every interrupt and exception entry clears it (CP-40). Entering
/// with AC=1 therefore means a nested call or an AC that leaked out of a
/// blocked window. Both are bugs, so this panics instead of opening a window
/// it would close at the wrong time or not at all.
#[inline]
pub fn with_user_access<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let active = SMAP_ACTIVE.load(Ordering::Acquire);
    if active {
        if x86_64::registers::rflags::read_raw() & (1 << 18) != 0 {
            // Close the window being abandoned so the panic path runs under SMAP.
            clac();
            panic!(
                "with_user_access entered with RFLAGS.AC set: nested call or leaked SMAP window (CP-49)"
            );
        }
        stac();
    }
    let result = f();
    if active {
        clac();
    }
    result
}

fn cpuid_leaf7_ebx() -> u32 {
    if cpuid_max_basic_leaf() < 7 {
        return 0;
    }

    let ebx: u32;
    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "mov {ebx:e}, ebx",
            "pop rbx",
            inout("eax") 7_u32 => _,
            inout("ecx") 0_u32 => _,
            lateout("edx") _,
            ebx = lateout(reg) ebx,
            options(nomem)
        );
    }
    ebx
}

fn cpuid_max_basic_leaf() -> u32 {
    let eax: u32;
    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "pop rbx",
            inout("eax") 0_u32 => eax,
            inout("ecx") 0_u32 => _,
            lateout("edx") _,
            options(nomem)
        );
    }
    eax
}

// No `nomem` on `stac`/`clac` (CP-42). With `nomem` the compiler may move
// memory accesses across them: a user access moved outside the
// `with_user_access` window faults under SMAP, and a kernel access moved
// inside runs with SMAP lifted. Without it each acts as a compiler memory
// barrier, the same job Linux's `"memory"` clobber does.
#[inline]
fn stac() {
    unsafe {
        asm!("stac", options(nostack));
    }
}

#[inline]
fn clac() {
    unsafe {
        asm!("clac", options(nostack));
    }
}

/// Install the GDT (with TSS), IDT, and PIC. After this returns, CPU
/// exceptions are caught by our handlers and the PIC has been remapped to
/// vectors 32..47 with all IRQs MASKED. Interrupts (IF) remain disabled —
/// call `enable_interrupts()` separately, then unmask individual IRQs (e.g.
/// `pic::unmask_irq(0)` for the timer) once the kernel is ready to handle
/// them.
///
/// Order matters: GDT first (so IDT entries can reference kernel CS, TSS
/// exists for IST, and user selectors exist for STAR), then IDT (handlers
/// must exist before any IRQ can fire), then PIC (remap legacy vectors
/// away from CPU exception range BEFORE we `sti`), then syscall (MSR
/// setup needs valid user selectors from GDT).
pub fn init_traps_bsp() {
    gdt::init_bsp();
    idt::init();
    pic::init();
    syscall::init_bsp();

    debug_assert_eq!(
        per_cpu::current_cpu_id(),
        0,
        "BSP GS-base sentinel: current_cpu_id() != 0 after init_bsp"
    );
}

/// Install this AP's GDT/TSS, load the shared IDT, and wire AP-local syscall
/// MSRs. Sub-commit 1a defines this entry point; sub-commit 1b calls it from
/// AP startup.
pub fn init_traps_ap(cpu_id: u32) {
    gdt::init_ap(cpu_id);
    // Loading GS during GDT install resets the visible GS base on APs. Rewire
    // it after segment setup so all later per-CPU accessors resolve correctly.
    per_cpu::init_ap(cpu_id);
    idt::load_for_ap();
    apic::init_ap(cpu_id);
    syscall::init_ap();

    debug_assert_eq!(
        per_cpu::current_cpu_id(),
        cpu_id,
        "AP GS-base sentinel: current_cpu_id() mismatch after init_ap"
    );
}

/// Compatibility shim retained for sub-commit 1a. The existing boot path stays
/// single-CPU and unchanged; sub-commit 1b will call `init_traps_bsp()`.
pub fn init_traps() {
    init_traps_bsp();
}

/// Set the IF flag, allow maskable interrupts. The IDT must already be
/// loaded — calling this with no IDT means the next hardware interrupt
/// triple-faults. Marked safe because the underlying instruction does not
/// violate Rust's memory model; correctness is the caller's concern.
#[inline(always)]
pub fn enable_interrupts() {
    interrupts::enable();
}

/// Clear the IF flag, mask maskable interrupts. Caller is responsible for
/// restoring interrupts later if appropriate.
#[inline(always)]
#[allow(dead_code)] // public API surface; first caller lands in Phase 1.2 (scheduler critical sections).
pub fn disable_interrupts() {
    interrupts::disable();
}

/// Atomically enable interrupts and halt the CPU (`sti; hlt`). `sti`'s effect
/// is deferred until *after* the following instruction, so no interrupt can be
/// delivered in the gap between enabling and halting: a wake IRQ that the
/// caller ruled out with IF=0 immediately before this call is guaranteed to be
/// delivered and wake the `hlt` rather than being lost. The per-CPU idle
/// thread's halt (OT-49) is the sole caller — it checks its ready queue with
/// interrupts disabled, then halts here race-free.
#[inline(always)]
pub fn enable_interrupts_and_halt() {
    // OT-49 (AAG B3): `enable_and_hlt` is `sti; hlt` with `options(nomem)`, which
    // in principle licenses the compiler to sink a preceding memory store PAST
    // the halt. Today's sole caller (the idle thread) has already released the
    // scheduler lock before reaching here, but a full compiler fence makes the
    // "no store may cross the halt" guarantee explicit and defends any future
    // caller or inlining: a CPU that halted while still holding a lock the waking
    // ISR needs would wedge that ISR in a spin, hanging the CPU. Codegen-only
    // barrier — no runtime cost.
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    interrupts::enable_and_hlt();
}

/// Return whether the local CPU's interrupt flag is currently set.
#[inline(always)]
#[allow(dead_code)]
pub fn interrupts_enabled() -> bool {
    interrupts::are_enabled()
}

/// Run `f` with maskable interrupts enabled, restoring the prior IF state on
/// return. This is intentionally narrower than making syscall dispatch
/// preemptible wholesale: callers wrap only long compute windows after all
/// user-access copies and short IRQ-off critical sections have completed.
#[inline(always)]
#[allow(dead_code)]
pub fn with_interrupts_enabled<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    struct RestoreInterruptFlag {
        prior_if_state: bool,
    }

    impl Drop for RestoreInterruptFlag {
        #[inline(always)]
        fn drop(&mut self) {
            if self.prior_if_state {
                enable_interrupts();
            } else {
                disable_interrupts();
            }
        }
    }

    let prior_if_state = interrupts_enabled();
    enable_interrupts();
    let _restore = RestoreInterruptFlag { prior_if_state };
    f()
}

/// Trigger a software breakpoint exception (`int3` → vector 3 → #BP).
/// The breakpoint handler prints a diagnostic and returns; the kernel
/// continues normal execution. Used as a smoke test that the IDT works.
#[inline(always)]
pub fn breakpoint() {
    interrupts::int3();
}

/// Run `f` with maskable interrupts disabled, restoring the previous IF on
/// return. Re-exported through the `arch` seam so scheduler/IPC code doesn't
/// reach into `x86_64::instructions::interrupts` directly — keeps the
/// arch-agnostic module surface clean for the eventual aarch64 port.
#[inline(always)]
pub fn without_interrupts<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    interrupts::without_interrupts(f)
}
