// SPDX-License-Identifier: MPL-2.0
//! Interrupt Descriptor Table + handlers for the standard CPU exceptions.
//!
//! Phase 1.0: every standard exception has a handler that prints a diagnostic
//! to serial and then halts the kernel. Two exceptions to that rule:
//!   - #BP (breakpoint) returns to the instruction after `int3` — it's a
//!     debugger trap, not fatal.
//!   - #DF (double fault) uses IST 0 (a dedicated stack from `gdt`) so a
//!     stack-overflow #PF doesn't recurse and triple-fault.
//!   - #MC (machine check) and the error-code vectors (#DF, #TS, #NP, #SS,
//!     #GP, #PF, #AC) diverge (`-> !`); the error-code ones enter through a
//!     realigning shim that has no way back (CP-46). Everything else calls
//!     `halt_forever()` from a `()` return.
//!
//! Phase 1.1+ will add hardware IRQ vectors (timer, keyboard, etc.); Phase 1.3
//! adds the syscall vector (or moves to SYSCALL/SYSRET).

#[cfg(feature = "power-stop-cpus")]
use crate::arch::x86_64::cpu_stop::stop_ipi;
use core::arch::global_asm;
#[cfg(feature = "neo-test")]
use core::sync::atomic::{AtomicU64, Ordering};

use spin::Lazy;
use x86_64::VirtAddr;
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::{
    DivergingHandlerFunc, DivergingHandlerFuncWithErrCode, Entry, EntryOptions, HandlerFunc,
    HandlerFuncWithErrCode, InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode,
    PageFaultHandlerFunc,
};

use crate::arch::halt_forever;
use crate::arch::x86_64::apic;
use crate::arch::x86_64::timer::timer_isr_entry;
use crate::serial_println;
use crate::user_fault_logic::{self, Route, Vector};

use super::gdt::{DOUBLE_FAULT_IST_INDEX, NMI_IST_INDEX, TIMER_IST_INDEX};

// === exception entry ===

// Exception delivery clears neither RFLAGS.AC nor DF, and ring 3 can set both
// (`popfq`, `std`). With CR4.SMAP on, AC=1 lets every supervisor access to a
// user page succeed, so a ring-3 fault taken with AC=1 ran its handler with
// SMAP off. The `x86-interrupt` prologue emits `cld` but leaves AC alone. That
// mattered on the ring-3 #PF kill path: it keeps running kernel code, then
// switches to another thread, and `context_switch` does not save RFLAGS, so
// that thread inherited AC=1 until its next `clac` or `iretq` (CP-40).
//
// So every installed exception vector enters through a shim that applies the
// `swapgs_irq_entry` flags mask before any compiler-generated code runs, then
// jumps to the handler with RSP, every GPR and the CPU-pushed frame exactly as
// delivered (error-code vectors realign and call instead; see below). `clac`
// would #UD without SMAP. `iretq` restores the caller's AC and DF from the
// untouched frame.
//
// A *returning* exception handler (today only `breakpoint`, and only from
// ring 0: every gate is DPL0, so a ring-3 `int3` raises #GP) gets the GPRs and
// xmm0-15 saved by the prologue and AC=0/DF=0 from the shim. Nothing else of
// the SIMD/x87 state is saved, MXCSR included, and only `page_fault` does a
// `swapgs`. Such a handler, and everything it calls, must therefore not:
//   - execute any instruction that writes SIMD/x87 state beyond xmm0-15: VEX
//     (AVX/AVX2, `vzeroupper` included: YMM upper halves), EVEX (AVX-512:
//     upper ZMM lanes, zmm16-31, opmask k0-7) or x87;
//   - do SSE floating-point arithmetic, which runs under the interrupted
//     context's MXCSR and can set its sticky status bits (integer SSE, as in
//     `core::fmt` of integers, is fine);
//   - touch user memory outside `with_user_access`;
//   - use GS-relative per-CPU state if it can be entered from ring 3. Gate DPL
//     only stops software `int n`; a hardware exception raised at CPL3 still
//     arrives on the user's GS. Such a handler needs a saved-CS-qualified
//     `swapgs` before the first per-CPU access and the matching one before
//     `iretq` (as `swapgs_irq_entry` does), or a common wrapper that does both.
// The non-returning handlers halt, except the ring-3 #PF kill path, which
// ends in `scheduler::exit()`. `page_fault` does its one `swapgs` first thing,
// before any per-CPU access in any build (CP-44), and the kill path adds
// `load_kernel_fp_control` (CP-41), so the kill runs on the kernel GS and
// MXCSR; with `xsave-fpu`, the switch loads the next thread's own FPU image.
/// Install, at IDT entry `$entry`, an `exception_entry` shim generated for
/// `$handler`. The shim is a naked function built from the handler's own name,
/// so it can only jump there, and passing the handler keeps the compiler
/// checking its signature against the vector (error code or not, diverging or
/// not), as `set_handler_fn` does. Evaluates to the entry's options.
macro_rules! set_exception_entry {
    ($entry:expr, $handler:ident) => {{
        #[unsafe(naked)]
        extern "C" fn shim() {
            core::arch::naked_asm!(
                "pushfq",
                "and qword ptr [rsp], -263169", // ~(AC (1<<18) | DF (1<<10))
                "popfq",
                "jmp {handler}",
                handler = sym $handler,
            )
        }
        // SAFETY: `shim` is the `exception_entry` shim for `$handler`.
        unsafe { install_exception_shim(&mut $entry, $handler, shim) }
    }};
}

// Vectors that push an error code do not use `x86-interrupt` handlers. The
// CPU leaves RSP 16-byte aligned *at* the error code (it aligns, then pushes
// six qwords), and LLVM's `x86-interrupt` prologue for those vectors calls
// the handler's callees at RSP ≡ 8 (mod 16), not SysV's RSP+8 ≡ 0. Any
// callee that keeps a 16-byte stack slot with `movaps` then raises #GP, and
// one does on the ring-3 #PF kill path (`purge_pid` -> `recompute_earliest`
// with a timed futex waiter) (CP-46). So each error-code vector enters
// through `set_error_code_entry!`: the same flags mask, then `rdi` = the
// error code and CPU frame as delivered, `and rsp, -16`, and a call into an
// `extern "C" fn(&ErrorCodeFrame) -> !` body. Every one of these handlers
// diverges (halt, or the #PF kill path's switch-away), so nothing returns
// through the realigned stack and no state needs restoring.

/// The stack an error-code vector leaves, as the CPU pushed it.
#[repr(C)]
pub struct ErrorCodeFrame {
    pub error_code: u64,
    pub frame: InterruptStackFrame,
}

// Six qwords, error code first: exactly what the CPU pushed.
const _: () = {
    assert!(core::mem::size_of::<ErrorCodeFrame>() == 48);
    assert!(core::mem::offset_of!(ErrorCodeFrame, frame) == 8);
};

/// The IDT entry types whose vectors push an error code. Only these may take
/// a `set_error_code_entry!` shim: on any other vector, `rdi` would point at
/// RIP instead of an error code.
trait ErrorCodeVector {}
impl ErrorCodeVector for HandlerFuncWithErrCode {}
impl ErrorCodeVector for DivergingHandlerFuncWithErrCode {}
impl ErrorCodeVector for PageFaultHandlerFunc {}

/// Install, at the error-code vector `$entry`, a shim that applies the
/// `exception_entry` flags mask (CP-40) and calls `$body` on a SysV-aligned
/// stack (CP-46). `$body` must be an `extern "C" fn(&ErrorCodeFrame) -> !`.
/// Evaluates to the entry's options.
macro_rules! set_error_code_entry {
    ($entry:expr, $body:ident) => {{
        #[unsafe(naked)]
        extern "C" fn shim() {
            core::arch::naked_asm!(
                "pushfq",
                "and qword ptr [rsp], -263169", // ~(AC (1<<18) | DF (1<<10))
                "popfq",
                "mov rdi, rsp", // &ErrorCodeFrame
                "and rsp, -16",
                "call {body}",
                "ud2",
                body = sym $body,
            )
        }
        let _: extern "C" fn(&ErrorCodeFrame) -> ! = $body;
        // SAFETY: `shim` masks only RFLAGS.AC/DF, then hands `$body` the
        // untouched error code and CPU frame and never returns.
        unsafe { install_error_code_shim(&mut $entry, shim) }
    }};
}

/// # Safety
///
/// `shim` must be a `set_error_code_entry!` shim.
unsafe fn install_error_code_shim<F: ErrorCodeVector>(
    entry: &mut Entry<F>,
    shim: extern "C" fn(),
) -> &mut EntryOptions {
    // SAFETY: `F: ErrorCodeVector`, so the CPU pushes an error code on this
    // vector and the shim's `rdi` points at it; the body never returns, so
    // the realigned stack is never unwound into the CPU frame.
    unsafe { entry.set_handler_addr(VirtAddr::new(shim as *const () as u64)) }
}

/// The IDT entry types whose vectors push no error code. Only these may take
/// a `set_exception_entry!` shim: on an error-code vector its `x86-interrupt`
/// handler would call Rust on a misaligned stack (CP-46).
trait NoErrorCodeVector {}
impl NoErrorCodeVector for HandlerFunc {}
impl NoErrorCodeVector for DivergingHandlerFunc {}

/// # Safety
///
/// `shim` must be an `exception_entry` shim that jumps to `handler`.
unsafe fn install_exception_shim<F: NoErrorCodeVector>(
    entry: &mut Entry<F>,
    _handler: F,
    shim: extern "C" fn(),
) -> &mut EntryOptions {
    // SAFETY: the shim changes only RFLAGS.AC/DF, leaves RSP, every GPR and
    // the CPU-pushed frame as delivered, and (caller contract) jumps to
    // `handler`, whose type matches this vector's entry.
    unsafe { entry.set_handler_addr(VirtAddr::new(shim as *const () as u64)) }
}

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();

    // Every exception vector enters through its `exception_entry` shim (CP-40).
    set_exception_entry!(idt.divide_error, divide_error);
    set_exception_entry!(idt.debug, debug);
    set_exception_entry!(idt.breakpoint, breakpoint);
    set_exception_entry!(idt.overflow, overflow);
    set_exception_entry!(idt.bound_range_exceeded, bound_range);
    set_exception_entry!(idt.invalid_opcode, invalid_opcode);
    set_exception_entry!(idt.device_not_available, device_not_available);
    // Error-code vectors also realign the stack for their body (CP-46).
    set_error_code_entry!(idt.invalid_tss, invalid_tss);
    set_error_code_entry!(idt.segment_not_present, segment_not_present);
    set_error_code_entry!(idt.stack_segment_fault, stack_segment_fault);
    set_error_code_entry!(idt.general_protection_fault, general_protection_fault);
    set_error_code_entry!(idt.page_fault, page_fault);
    set_exception_entry!(idt.x87_floating_point, x87_floating_point);
    set_error_code_entry!(idt.alignment_check, alignment_check);
    set_exception_entry!(idt.simd_floating_point, simd_floating_point);
    set_exception_entry!(idt.machine_check, machine_check);

    // SAFETY (all three `set_stack_index` calls): DOUBLE_FAULT_IST_INDEX,
    // NMI_IST_INDEX, and TIMER_IST_INDEX are all valid IST slots populated in
    // gdt.rs before this module's init runs. Setting an IST index that the TSS
    // hasn't populated would dispatch onto a zero stack pointer.
    let double_fault_options = set_error_code_entry!(idt.double_fault, double_fault);
    unsafe { double_fault_options.set_stack_index(DOUBLE_FAULT_IST_INDEX) };
    let nmi_options = set_exception_entry!(idt.non_maskable_interrupt, nmi);
    unsafe { nmi_options.set_stack_index(NMI_IST_INDEX) };
    // OT-90 P3b: the shutdown's STOP IPI (an interrupt gate: IF clears on
    // entry, and the handler never returns).
    #[cfg(feature = "power-stop-cpus")]
    set_exception_entry!(idt[apic::STOP_IPI_VECTOR], stop_ipi);
    // SAFETY: as above for the timer's IST slot; every address below is a
    // hand-written entry stub that preserves the interrupted context.
    unsafe {
        // Vector 32 = first hardware IRQ after PIC remap = PIC IRQ0 (timer).
        // The asm wrapper provides the entry point; we pass its address
        // directly since extern "x86-interrupt" wouldn't auto-save GP regs.
        // Default Entry options give an INTERRUPT GATE (clears IF on entry),
        // not a trap gate — exactly what we want for non-reentrant timer.
        idt[32]
            .set_handler_addr(VirtAddr::new(timer_isr_entry as *const () as u64))
            .set_stack_index(TIMER_IST_INDEX);
        idt[apic::TLB_SHOOTDOWN_IPI_VECTOR]
            .set_handler_addr(VirtAddr::new(tlb_shootdown_ipi_entry as *const () as u64));
        idt[apic::RESCHEDULE_IPI_VECTOR]
            .set_handler_addr(VirtAddr::new(reschedule_ipi_entry as *const () as u64));
        idt[crate::drivers::block::ahci::AHCI_MSI_VECTOR]
            .set_handler_addr(VirtAddr::new(ahci_msi_interrupt_entry as *const () as u64));
        idt[crate::drivers::block::nvme::NVME_MSI_VECTOR]
            .set_handler_addr(VirtAddr::new(nvme_msi_interrupt_entry as *const () as u64));
        #[cfg(feature = "virtio-blk-irq")]
        idt[crate::drivers::block::virtio_blk::VIRTIO_BLK_MSI_VECTOR].set_handler_addr(
            VirtAddr::new(virtio_blk_msi_interrupt_entry as *const () as u64),
        );
        #[cfg(feature = "usb-host")]
        idt[crate::drivers::usb::xhci::XHCI_MSI_VECTOR]
            .set_handler_addr(VirtAddr::new(xhci_msi_interrupt_entry as *const () as u64));
        #[cfg(feature = "virtio-accel")]
        idt[crate::drivers::accel::virtio_accel::VIRTIO_ACCEL_MSI_VECTOR].set_handler_addr(
            VirtAddr::new(virtio_accel_msi_interrupt_entry as *const () as u64),
        );
        #[cfg(feature = "ps2-keyboard")]
        idt[crate::drivers::ps2::i8042::VECTOR]
            .set_handler_addr(VirtAddr::new(i8042_interrupt_entry as *const () as u64));
        #[cfg(feature = "i2c-hid-touchpad")]
        idt[crate::drivers::i2c_hid::touchpad::VECTOR]
            .set_handler_addr(VirtAddr::new(i2c_hid_interrupt_entry as *const () as u64));
        idt[apic::SPURIOUS_VECTOR]
            .set_handler_addr(VirtAddr::new(spurious_interrupt_entry as *const () as u64));
        #[cfg(feature = "neo-test-irq-xmm-preserve")]
        idt[IRQ_XMM_TEST_VECTOR]
            .set_handler_addr(VirtAddr::new(irq_xmm_test_entry as *const () as u64));
        #[cfg(feature = "neo-test-syscall-exit-irq-window")]
        idt[SYSCALL_EXIT_IRQ_TEST_VECTOR].set_handler_addr(VirtAddr::new(
            syscall_exit_irq_test_entry as *const () as u64,
        ));
    }

    idt
});

pub fn init() {
    IDT.load();
    #[cfg(feature = "virtio-blk-irq")]
    serial_println!(
        "[ kernel ] idt: loaded (18 exception handlers + timer at vector 32 + AHCI MSI at vector 240 + NVMe MSI/MSI-X at vector 241 + virtio-blk MSI/MSI-X at vector 244 + TLB shootdown IPI at vector 252 + resched IPI at vector 253)"
    );
    #[cfg(not(feature = "virtio-blk-irq"))]
    serial_println!(
        "[ kernel ] idt: loaded (18 exception handlers + timer at vector 32 + AHCI MSI at vector 240 + NVMe MSI/MSI-X at vector 241 + TLB shootdown IPI at vector 252 + resched IPI at vector 253)"
    );
}

/// Load the shared IDT on a non-BSP CPU. The IDT itself is global because
/// exception handlers are stateless and IST indices resolve through the
/// current CPU's TSS.
pub fn load_for_ap() {
    IDT.load();
}

// === handlers ===

// The Rust IRQ bodies are compiled with SSE2 and freely use xmm registers
// (struct moves, `core::fmt`, blake3), which are caller-saved in SysV. An
// interrupt is not a call boundary, so the stub must preserve the interrupted
// context's x87/SSE state itself, exactly like the timer stub does. Without
// this, an MSI landing mid-hash on an IF-on kernel path (e.g. NeoFS
// `resolve_path` under `sys_read`) corrupts the interrupted computation.
// Exception handlers without an error code use the `x86-interrupt` ABI, whose
// prologue spills xmm0-15; see "exception entry" above for what it leaves
// unsaved. The error-code handlers never return (CP-46).
#[cfg(not(feature = "xsave-fpu"))]
macro_rules! irq_fpu_save {
    () => {
        r#"
    # `and rsp, ~0xF` aligns RSP to 16. `sub rsp, 512` keeps it 16-aligned.
    and rsp, 0xFFFFFFFFFFFFFFF0
    sub rsp, 512
    fxsave [rsp]
    # The body must not run on the interrupted context's MXCSR (CP-41);
    # fxrstor puts it back. No fninit: the kernel has no x87 code.
    ldmxcsr [rip + KERNEL_DEFAULT_MXCSR]
"#
    };
}

#[cfg(not(feature = "xsave-fpu"))]
macro_rules! irq_fpu_restore {
    () => {
        r#"
    fxrstor [rsp]
"#
    };
}

#[cfg(feature = "xsave-fpu")]
macro_rules! irq_fpu_save {
    () => {
        r#"
    # XSAVE requires 64-byte alignment. FPU_SAVE_AREA_SIZE is CPUID.0xD-sized
    # and 64-byte aligned by arch::x86_64::fpu.
    and rsp, 0xFFFFFFFFFFFFFFC0
    sub rsp, qword ptr [rip + FPU_SAVE_AREA_SIZE]
    mov rdi, rsp
    call fpu_save_area_asm
    # The body must not run on the interrupted context's MXCSR (CP-41);
    # the restore puts it back. No fninit: the kernel has no x87 code.
    ldmxcsr [rip + KERNEL_DEFAULT_MXCSR]
"#
    };
}

#[cfg(feature = "xsave-fpu")]
macro_rules! irq_fpu_restore {
    () => {
        r#"
    mov rdi, rsp
    call fpu_restore_area_asm
"#
    };
}

/// Test-only vector for `irq_xmm_preserve`, routed through `swapgs_irq_entry`
/// like the device MSIs. Not programmed into any device or the LAPIC.
#[cfg(feature = "neo-test-irq-xmm-preserve")]
pub const IRQ_XMM_TEST_VECTOR: u8 = 0xe0;

/// IRQ1 (`ps2-keyboard`): assembled only with the feature, so feature-off
/// builds carry no trace of the driver.
#[cfg(feature = "ps2-keyboard")]
macro_rules! i8042_irq_instance {
    () => {
        "swapgs_irq_entry i8042_interrupt_entry, i8042_interrupt_body\n"
    };
}

#[cfg(not(feature = "ps2-keyboard"))]
macro_rules! i8042_irq_instance {
    () => {
        ""
    };
}

/// The touchpad's GSI (`i2c-hid-touchpad`): assembled only with the
/// feature, so feature-off builds carry no trace of the driver.
#[cfg(feature = "i2c-hid-touchpad")]
macro_rules! i2c_hid_irq_instance {
    () => {
        "swapgs_irq_entry i2c_hid_interrupt_entry, i2c_hid_interrupt_body\n"
    };
}

#[cfg(not(feature = "i2c-hid-touchpad"))]
macro_rules! i2c_hid_irq_instance {
    () => {
        ""
    };
}

#[cfg(feature = "neo-test-irq-xmm-preserve")]
macro_rules! irq_xmm_test_instance {
    () => {
        "swapgs_irq_entry irq_xmm_test_entry, irq_xmm_test_body\n"
    };
}

#[cfg(not(feature = "neo-test-irq-xmm-preserve"))]
macro_rules! irq_xmm_test_instance {
    () => {
        ""
    };
}

/// Test-only vector for `syscall_exit_irq_window`, raised by the LAPIC timer
/// and routed through `swapgs_irq_entry` like the device MSIs. The body is a
/// test-harness shim that also passes the interrupted frame.
#[cfg(feature = "neo-test-syscall-exit-irq-window")]
pub const SYSCALL_EXIT_IRQ_TEST_VECTOR: u8 = 0xe1;

#[cfg(feature = "neo-test-syscall-exit-irq-window")]
macro_rules! syscall_exit_irq_test_instance {
    () => {
        "swapgs_irq_entry syscall_exit_irq_test_entry, syscall_exit_irq_test_shim\n"
    };
}

#[cfg(not(feature = "neo-test-syscall-exit-irq-window"))]
macro_rules! syscall_exit_irq_test_instance {
    () => {
        ""
    };
}

global_asm!(concat!(
    r#"
.macro swapgs_irq_entry entry, body
.global \entry
.type \entry, @function
\entry:
    # ENTRY: CPU has pushed the interrupt frame at RSP. CS is always
    # at [rsp+8]. If saved CS.RPL=3, GS is still the userspace GS base.
    # Swap once before running Rust, then swap back exactly once before iretq.
    test byte ptr [rsp + 8], 3
    jz 1f
    swapgs
    push 1                              # from_ring3 flag for Rust body
    jmp 2f
1:
    push 0                              # ring-0 source; no swap happened
2:
    # Save all GP registers: interrupt delivery is not a SysV call boundary.
    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15

    # Interrupt delivery clears neither RFLAGS.DF nor RFLAGS.AC, and ring 3 can
    # set both (`std`, `popfq`). The SysV Rust body and compiler_builtins'
    # `rep movs`/`stos` memcpy/memset assume DF=0 (CP-39), and AC=1 suspends
    # SMAP for every supervisor access (CP-40). Clear both before any Rust
    # runs. A flags-mask is used instead of `clac`, which #UDs without SMAP.
    # The saved frame is untouched, so iretq restores the caller's DF/AC.
    pushfq
    and qword ptr [rsp], -263169        # ~(AC (1<<18) | DF (1<<10))
    popfq

    # Anchor pre-align RSP via rbp. The from_ring3 flag is 128 bytes above
    # this anchor: 15 saved GP registers (120 bytes) plus the anchor slot.
    push rbp
    mov rbp, rsp
"#,
    irq_fpu_save!(),
    r#"
    # ALIGNMENT INVARIANT (do not insert any push between here and `call`):
    # the save area leaves RSP 16-aligned, so the body enters at RSP = 8 mod 16.
    mov rdi, [rbp + 128]
    call \body
"#,
    irq_fpu_restore!(),
    r#"
    mov rsp, rbp
    pop rbp

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax

    # RSP points at the from_ring3 flag. A nonzero flag came only from the
    # saved-CS.RPL=3 branch above, so it needs the symmetric swapgs exit.
    cmp qword ptr [rsp], 0
    je 3f
    swapgs
3:
    add rsp, 8
    iretq
.size \entry, . - \entry
.endm

swapgs_irq_entry tlb_shootdown_ipi_entry, tlb_shootdown_ipi_body
swapgs_irq_entry reschedule_ipi_entry, reschedule_ipi_body
swapgs_irq_entry ahci_msi_interrupt_entry, ahci_msi_interrupt_body
swapgs_irq_entry nvme_msi_interrupt_entry, nvme_msi_interrupt_body
swapgs_irq_entry virtio_blk_msi_interrupt_entry, virtio_blk_msi_interrupt_body
swapgs_irq_entry virtio_accel_msi_interrupt_entry, virtio_accel_msi_interrupt_body
swapgs_irq_entry xhci_msi_interrupt_entry, xhci_msi_interrupt_body
swapgs_irq_entry spurious_interrupt_entry, spurious_interrupt_body
"#,
    irq_xmm_test_instance!(),
    syscall_exit_irq_test_instance!(),
    i8042_irq_instance!(),
    i2c_hid_irq_instance!(),
));

unsafe extern "C" {
    fn tlb_shootdown_ipi_entry();
    fn reschedule_ipi_entry();
    fn ahci_msi_interrupt_entry();
    fn nvme_msi_interrupt_entry();
    #[cfg(feature = "virtio-blk-irq")]
    fn virtio_blk_msi_interrupt_entry();
    #[cfg(feature = "virtio-accel")]
    fn virtio_accel_msi_interrupt_entry();
    #[allow(dead_code)] // Assembly entry symbol installed by feature-gated xHCI paths.
    fn xhci_msi_interrupt_entry();
    #[cfg(feature = "ps2-keyboard")]
    fn i8042_interrupt_entry();
    #[cfg(feature = "i2c-hid-touchpad")]
    fn i2c_hid_interrupt_entry();
    fn spurious_interrupt_entry();
    #[cfg(feature = "neo-test-irq-xmm-preserve")]
    fn irq_xmm_test_entry();
    #[cfg(feature = "neo-test-syscall-exit-irq-window")]
    fn syscall_exit_irq_test_entry();
}

const IRQ_RING3_TLB_SHOOTDOWN: usize = 0;
const IRQ_RING3_RESCHEDULE: usize = 1;
const IRQ_RING3_AHCI: usize = 2;
const IRQ_RING3_NVME: usize = 3;
const IRQ_RING3_VIRTIO_ACCEL: usize = 4;
const IRQ_RING3_SPURIOUS: usize = 5;
const IRQ_RING3_XHCI: usize = 6;
#[cfg(feature = "virtio-blk-irq")]
const IRQ_RING3_VIRTIO_BLK: usize = 7;

#[cfg(all(feature = "neo-test", feature = "virtio-blk-irq"))]
static IRQ_RING3_ENTRIES: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];

#[cfg(all(feature = "neo-test", not(feature = "virtio-blk-irq")))]
static IRQ_RING3_ENTRIES: [AtomicU64; 7] = [const { AtomicU64::new(0) }; 7];

#[cfg(feature = "neo-test")]
fn record_irq_entry(index: usize, from_ring3: u64) {
    if from_ring3 != 0 {
        IRQ_RING3_ENTRIES[index].fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(not(feature = "neo-test"))]
fn record_irq_entry(_index: usize, _from_ring3: u64) {}

#[cfg(feature = "neo-test")]
pub fn reset_irq_entry_test_counters() {
    for counter in IRQ_RING3_ENTRIES.iter() {
        counter.store(0, Ordering::Relaxed);
    }
}

#[cfg(feature = "neo-test")]
pub fn tlb_shootdown_ipi_ring3_entries_for_test() -> u64 {
    IRQ_RING3_ENTRIES[IRQ_RING3_TLB_SHOOTDOWN].load(Ordering::Relaxed)
}

#[cfg(feature = "neo-test")]
pub fn reschedule_ipi_ring3_entries_for_test() -> u64 {
    IRQ_RING3_ENTRIES[IRQ_RING3_RESCHEDULE].load(Ordering::Relaxed)
}

fn dump(name: &str, sf: &InterruptStackFrame) {
    serial_println!("[ trap  ] {} {:?}", name, sf);
}

/// OT-83 (I-14b): if `sf` was raised at CPL 3, kill the faulting process and
/// never return; a CPL-0 fault returns so the caller halts as before. Call it
/// FIRST in a handler, before any print or per-CPU access: a ring-3 exception
/// arrives with the USER GS base live (the exception shims do not swap), so
/// the swap must precede everything that may read per-CPU state (CP-44).
/// Test hooks that must see the raw trap (CP-47/CP-48's `syscall_msr_trap`)
/// run before this and diverge. `detail` is the error code (0 if none).
fn contain_user_fault(vector: Vector, sf: &InterruptStackFrame, detail: u64) {
    if let Route::KillProcess { .. } = user_fault_logic::route(vector, sf.code_segment.0) {
        // SAFETY: CS.RPL=3 ⇒ the user GS base is live and KERNEL_GS_BASE
        // holds &PER_CPU; one swapgs restores the kernel-mode invariant on a
        // path that never returns to ring 3.
        unsafe { core::arch::asm!("swapgs", options(nostack, preserves_flags)) };
        // The faulting thread's MXCSR/x87 control word must not follow the
        // kill path (CP-41). FNINIT is non-waiting, so a pending unmasked x87
        // exception (#MF's cause) cannot re-fault here.
        super::load_kernel_fp_control();
        crate::sched::process::fault_kill_current_user_exception(
            vector,
            sf.instruction_pointer.as_u64(),
            detail,
        );
    }
}

/// OT-83: the CPL-0 end of a routed exception — a kernel bug, so halt, exactly
/// as before. The `ring0_fault_halts` control scenario proves a kernel-mode
/// fault reaches here and not the kill path.
fn halt_kernel_fault(vector: Vector) -> ! {
    #[cfg(feature = "neo-test-ring0-fault-halts")]
    crate::test_harness::ring0_fault_reached_halt(vector);
    let _ = vector;
    halt_forever();
}

extern "x86-interrupt" fn divide_error(sf: InterruptStackFrame) {
    contain_user_fault(Vector::DivideError, &sf, 0);
    dump("DIVIDE_ERROR (#DE)", &sf);
    halt_kernel_fault(Vector::DivideError);
}

extern "x86-interrupt" fn debug(sf: InterruptStackFrame) {
    // OT-83: ring 3 can raise #DB on itself (RFLAGS.TF via popfq).
    contain_user_fault(Vector::Debug, &sf, 0);
    dump("DEBUG (#DB)", &sf);
    halt_kernel_fault(Vector::Debug);
}

#[cfg(feature = "neo-test-exception-entry-flags")]
static BREAKPOINT_RUNS: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "neo-test-exception-entry-flags")]
static BREAKPOINT_AC_SET: AtomicU64 = AtomicU64::new(0);

/// `(runs, runs with RFLAGS.AC set)` for `breakpoint`; the second must be 0 (CP-40).
#[cfg(feature = "neo-test-exception-entry-flags")]
pub fn breakpoint_entry_ac_for_test() -> (u64, u64) {
    (
        BREAKPOINT_RUNS.load(Ordering::Relaxed),
        BREAKPOINT_AC_SET.load(Ordering::Relaxed),
    )
}

extern "x86-interrupt" fn breakpoint(sf: InterruptStackFrame) {
    #[cfg(feature = "neo-test-exception-entry-flags")]
    {
        BREAKPOINT_RUNS.fetch_add(1, Ordering::Relaxed);
        if x86_64::registers::rflags::read_raw() & (1 << 18) != 0 {
            BREAKPOINT_AC_SET.fetch_add(1, Ordering::Relaxed);
        }
    }
    dump("BREAKPOINT (#BP)", &sf);
    // Non-fatal — return to the instruction after int3.
}

extern "x86-interrupt" fn overflow(sf: InterruptStackFrame) {
    contain_user_fault(Vector::Overflow, &sf, 0);
    dump("OVERFLOW (#OF)", &sf);
    halt_kernel_fault(Vector::Overflow);
}

extern "x86-interrupt" fn bound_range(sf: InterruptStackFrame) {
    contain_user_fault(Vector::BoundRange, &sf, 0);
    dump("BOUND_RANGE_EXCEEDED (#BR)", &sf);
    halt_kernel_fault(Vector::BoundRange);
}

extern "x86-interrupt" fn invalid_opcode(sf: InterruptStackFrame) {
    contain_user_fault(Vector::InvalidOpcode, &sf, 0);
    dump("INVALID_OPCODE (#UD)", &sf);
    halt_kernel_fault(Vector::InvalidOpcode);
}

extern "x86-interrupt" fn device_not_available(sf: InterruptStackFrame) {
    contain_user_fault(Vector::DeviceNotAvailable, &sf, 0);
    dump("DEVICE_NOT_AVAILABLE (#NM)", &sf);
    halt_kernel_fault(Vector::DeviceNotAvailable);
}

extern "C" fn invalid_tss(f: &ErrorCodeFrame) -> ! {
    serial_println!("[ trap  ] INVALID_TSS (#TS) err=0x{:x}", f.error_code);
    dump("INVALID_TSS (#TS)", &f.frame);
    halt_forever();
}

extern "C" fn segment_not_present(f: &ErrorCodeFrame) -> ! {
    contain_user_fault(Vector::SegmentNotPresent, &f.frame, f.error_code);
    serial_println!(
        "[ trap  ] SEGMENT_NOT_PRESENT (#NP) err=0x{:x}",
        f.error_code
    );
    dump("SEGMENT_NOT_PRESENT (#NP)", &f.frame);
    halt_kernel_fault(Vector::SegmentNotPresent);
}

extern "C" fn stack_segment_fault(f: &ErrorCodeFrame) -> ! {
    contain_user_fault(Vector::StackSegment, &f.frame, f.error_code);
    serial_println!(
        "[ trap  ] STACK_SEGMENT_FAULT (#SS) err=0x{:x}",
        f.error_code
    );
    dump("STACK_SEGMENT_FAULT (#SS)", &f.frame);
    halt_kernel_fault(Vector::StackSegment);
}

extern "C" fn general_protection_fault(f: &ErrorCodeFrame) -> ! {
    let sf = &f.frame;
    // CP-47/CP-48 scenarios: decide the scenario here, before any print
    // (the hook establishes the kernel GS first).
    #[cfg(any(
        feature = "neo-test-syscall-nt-masked",
        feature = "neo-test-compat-mode-unreachable"
    ))]
    crate::test_harness::syscall_msr_trap(
        crate::test_harness::SyscallMsrTrap::GeneralProtection,
        sf,
        f.error_code,
    );
    contain_user_fault(Vector::GeneralProtection, sf, f.error_code);

    serial_println!(
        "[ trap  ] GENERAL_PROTECTION_FAULT (#GP) err=0x{:x} (selector index)",
        f.error_code
    );
    serial_println!(
        "[ trap  ] #GP rip=0x{:016x} rsp=0x{:016x} cs=0x{:04x} ss=0x{:04x} rflags=0x{:016x}",
        sf.instruction_pointer.as_u64(),
        sf.stack_pointer.as_u64(),
        sf.code_segment.0,
        sf.stack_segment.0,
        sf.cpu_flags.bits()
    );
    dump("GENERAL_PROTECTION_FAULT (#GP)", sf);
    halt_kernel_fault(Vector::GeneralProtection);
}

#[cfg(any(
    feature = "neo-test-threads-guard-fault",
    feature = "neo-test-threads-guard-fault-observability",
    feature = "neo-test-threads-guard-fault-timed"
))]
static USER_PF_FRAME_AC: AtomicU64 = AtomicU64::new(0);

#[cfg(any(
    feature = "neo-test-threads-guard-fault",
    feature = "neo-test-threads-guard-fault-observability",
    feature = "neo-test-threads-guard-fault-timed"
))]
static USER_PF_LIVE_AC: AtomicU64 = AtomicU64::new(0);

/// Ring-3 page faults `(taken with RFLAGS.AC set, handled with AC set)`. The
/// first proves the probe armed AC; the second must be 0 (CP-40).
#[cfg(any(
    feature = "neo-test-threads-guard-fault",
    feature = "neo-test-threads-guard-fault-observability",
    feature = "neo-test-threads-guard-fault-timed"
))]
pub fn user_page_fault_ac_for_test() -> (u64, u64) {
    (
        USER_PF_FRAME_AC.load(Ordering::Relaxed),
        USER_PF_LIVE_AC.load(Ordering::Relaxed),
    )
}

extern "C" fn page_fault(f: &ErrorCodeFrame) -> ! {
    let sf = &f.frame;
    // The exception path performs NO swapgs (unlike swapgs_irq_entry and the
    // timer stub): a fault from ring 3 arrives with the USER GS base live,
    // and everything below may read per-CPU state through GS
    // (`record_page_fault` -> `current_cpu_id()` under kernel-observability,
    // the fault-kill's scheduler calls). Swap before any of it, keyed on the
    // saved CS.RPL like the IRQ stubs (CP-44). Nothing swaps back because no
    // path returns: the test hook diverges or falls through, the fault-kill
    // switches away for good, and the fatal path halts. A recoverable fault
    // cannot be resumed from here at all (the shim keeps no registers,
    // CP-46); it needs a returning entry stub that also swaps back.
    if sf.code_segment.0 & 3 == 3 {
        // SAFETY: CS.RPL=3 ⇒ the user GS base is live and KERNEL_GS_BASE
        // holds &PER_CPU; one swapgs restores the kernel-mode invariant on a
        // path that never returns to ring 3.
        unsafe { core::arch::asm!("swapgs", options(nostack, preserves_flags)) };
    }
    let err = PageFaultErrorCode::from_bits_retain(f.error_code);

    // CP-48: ring 0 fetching at RIP 0 is a compatibility-mode SYSCALL through
    // an unprogrammed CSTAR (on the user's GS; the hook swaps). The scenario's
    // only ring-3 code is its probe, which must not fault at all.
    #[cfg(feature = "neo-test-compat-mode-unreachable")]
    if sf.code_segment.0 & 3 == 3
        || (sf.code_segment.0 & 3 == 0 && sf.instruction_pointer.as_u64() == 0)
    {
        crate::test_harness::syscall_msr_trap(
            crate::test_harness::SyscallMsrTrap::PageFault,
            sf,
            err.bits(),
        );
    }

    #[cfg(any(
        feature = "neo-test-threads-guard-fault",
        feature = "neo-test-threads-guard-fault-observability",
        feature = "neo-test-threads-guard-fault-timed"
    ))]
    if err.contains(PageFaultErrorCode::USER_MODE) {
        if sf.cpu_flags.bits() & (1 << 18) != 0 {
            USER_PF_FRAME_AC.fetch_add(1, Ordering::Relaxed);
        }
        if x86_64::registers::rflags::read_raw() & (1 << 18) != 0 {
            USER_PF_LIVE_AC.fetch_add(1, Ordering::Relaxed);
        }
    }

    crate::observability::record_page_fault(Cr2::read_raw(), err.bits());

    #[cfg(feature = "neo-test")]
    crate::test_harness::handle_page_fault(sf, err);

    // OT-61 C1.1 (I-14b): a RING-3 fault is contained to the faulting
    // process — kill it (guard-page stack overflow is the archetype) and
    // keep the kernel + governance live. Runs AFTER the test-harness
    // expectation hook so scenarios that arm an expected user fault (SMEP/
    // SMAP/atrium-RO probes) keep their pass/fail semantics. Kernel-mode
    // faults fall through to the fail-loud dump below, unchanged.
    // USER_MODE ⇒ CPL 3, so the swapgs at the top already ran. OT-83: no
    // longer behind `browser-runtime` — every build contains a ring-3 #PF.
    if err.contains(PageFaultErrorCode::USER_MODE) {
        // One-way, like the swapgs at the top: the faulting thread's
        // MXCSR/x87 control word must not follow the kill path or, without
        // `xsave-fpu`, the next thread this CPU switches to (CP-41).
        super::load_kernel_fp_control();
        crate::sched::process::fault_kill_current_user(
            Cr2::read_raw(),
            sf.instruction_pointer.as_u64(),
        );
    }

    // Print the most useful diagnostics one line at a time, so a fault
    // mid-Debug-print (e.g., stack overflow into the dump path) still
    // leaves enough on serial to localize.
    serial_println!("[ trap  ] PAGE_FAULT (#PF) cr2=0x{:016x}", Cr2::read_raw());
    serial_println!("[ trap  ] PAGE_FAULT err = {:?}", err);
    serial_println!(
        "[ trap  ] PAGE_FAULT rip=0x{:016x} cs={:#x} rflags={:#x} rsp=0x{:016x} ss={:#x}",
        sf.instruction_pointer.as_u64(),
        sf.code_segment.0,
        sf.cpu_flags.bits(),
        sf.stack_pointer.as_u64(),
        sf.stack_segment.0,
    );
    #[cfg(feature = "neo-test")]
    crate::test_harness::unexpected_trap("page_fault");
    #[cfg(not(feature = "neo-test"))]
    halt_forever();
}

extern "x86-interrupt" fn x87_floating_point(sf: InterruptStackFrame) {
    contain_user_fault(Vector::X87FloatingPoint, &sf, 0);
    dump("X87_FP_EXCEPTION (#MF)", &sf);
    halt_kernel_fault(Vector::X87FloatingPoint);
}

extern "C" fn alignment_check(f: &ErrorCodeFrame) -> ! {
    contain_user_fault(Vector::AlignmentCheck, &f.frame, f.error_code);
    serial_println!("[ trap  ] ALIGNMENT_CHECK (#AC) err=0x{:x}", f.error_code);
    dump("ALIGNMENT_CHECK (#AC)", &f.frame);
    halt_kernel_fault(Vector::AlignmentCheck);
}

extern "x86-interrupt" fn simd_floating_point(sf: InterruptStackFrame) {
    // OT-83: ring-3 float code with unmasked exceptions kills its process.
    contain_user_fault(Vector::SimdFloatingPoint, &sf, 0);
    dump("SIMD_FP_EXCEPTION (#XF)", &sf);
    // SSE code raised an exception its MXCSR left unmasked in ring 0: kernel
    // code on a ring-3 MXCSR (CP-41). Fail the scenario by name now instead of
    // hanging until the QEMU budget.
    #[cfg(feature = "neo-test")]
    crate::test_harness::unexpected_trap("simd_floating_point");
    #[cfg(not(feature = "neo-test"))]
    halt_forever();
}

extern "C" fn double_fault(f: &ErrorCodeFrame) -> ! {
    #[cfg(any(
        feature = "neo-test-syscall-nt-masked",
        feature = "neo-test-compat-mode-unreachable"
    ))]
    crate::test_harness::syscall_msr_trap(
        crate::test_harness::SyscallMsrTrap::DoubleFault,
        &f.frame,
        f.error_code,
    );

    // B1c (F5): an overflow of the inference stack faults on its low guard,
    // and the `#PF` frame cannot be pushed onto that same unmapped page, so it
    // arrives here (on the IST stack). Name it, then halt like any `#DF`.
    #[cfg(feature = "inference-stack")]
    {
        let cr2 = Cr2::read_raw();
        let rsp = f.frame.stack_pointer.as_u64();
        let hit = crate::mm::inference_stack::guard_hit(cr2)
            .or_else(|| crate::mm::inference_stack::guard_hit(rsp));
        if let Some(hit) = hit {
            serial_println!(
                "[ trap  ] inference stack overflow guard={:?} cr2={:#x} rsp={:#x} rip={:#x} — halting",
                hit,
                cr2,
                rsp,
                f.frame.instruction_pointer.as_u64()
            );
            #[cfg(feature = "neo-test-inference-stack-overflow")]
            crate::test_harness::inference_stack_overflow_trap(hit, cr2);
            #[cfg(not(feature = "neo-test-inference-stack-overflow"))]
            halt_forever();
        }
    }
    serial_println!(
        "[ trap  ] DOUBLE_FAULT (#DF) err=0x{:x} — kernel state is unrecoverable",
        f.error_code
    );
    dump("DOUBLE_FAULT (#DF)", &f.frame);
    halt_forever();
}

extern "x86-interrupt" fn machine_check(sf: InterruptStackFrame) -> ! {
    serial_println!("[ trap  ] MACHINE_CHECK (#MC) — hardware reported uncorrected error");
    dump("MACHINE_CHECK (#MC)", &sf);
    halt_forever();
}

#[unsafe(no_mangle)]
extern "C" fn reschedule_ipi_body(from_ring3: u64) {
    record_irq_entry(IRQ_RING3_RESCHEDULE, from_ring3);
    // OT-61 C1.1 hang discriminator: per-CPU receive count for the reschedule
    // IPI, regardless of the interrupted ring. `record_irq_entry` above is
    // global AND ring-3-only, so it cannot answer "did CPU 2 actually receive
    // the IPI CPU 0 sent it?" — the first question a stranded-wake hunt must
    // settle. Deliberately still EOI-only otherwise (CP-15: the reschedule is
    // implicit, taken on the target's next scheduler entry).
    #[cfg(feature = "neo-test")]
    crate::sched::wake_trace::note_ipi_recv();
    apic::end_of_interrupt();
    // OT-61 C1.3b (CP-86): a process retire on another CPU sends this IPI to
    // the CPUs running a sibling. If it interrupted ring 3, that sibling holds
    // no kernel lock, so it can end here (after the EOI). From ring 0 it is
    // inside a syscall, whose return path checks.
    #[cfg(feature = "browser-runtime")]
    if from_ring3 != 0 {
        crate::sched::process::retire_current_if_dying();
    }
    // OT-93: `terminate` sends this IPI to the CPUs running a thread of the
    // process it killed. From ring 3 that thread ends here; from ring 0 its
    // syscall return checks.
    if from_ring3 != 0 {
        crate::sched::exit_current_if_force_killed();
    }
}

#[unsafe(no_mangle)]
extern "C" fn tlb_shootdown_ipi_body(from_ring3: u64) {
    record_irq_entry(IRQ_RING3_TLB_SHOOTDOWN, from_ring3);
    crate::mm::tlb_shootdown::handle_ipi();
    apic::end_of_interrupt();
}

#[unsafe(no_mangle)]
extern "C" fn ahci_msi_interrupt_body(from_ring3: u64) {
    record_irq_entry(IRQ_RING3_AHCI, from_ring3);
    crate::drivers::block::ahci::handle_interrupt();
    apic::end_of_interrupt();
}

#[unsafe(no_mangle)]
extern "C" fn nvme_msi_interrupt_body(from_ring3: u64) {
    record_irq_entry(IRQ_RING3_NVME, from_ring3);
    crate::drivers::block::nvme::handle_interrupt();
    apic::end_of_interrupt();
}

#[unsafe(no_mangle)]
extern "C" fn virtio_blk_msi_interrupt_body(from_ring3: u64) {
    #[cfg(feature = "virtio-blk-irq")]
    {
        record_irq_entry(IRQ_RING3_VIRTIO_BLK, from_ring3);
        crate::drivers::block::virtio_blk::handle_interrupt();
    }
    #[cfg(not(feature = "virtio-blk-irq"))]
    let _ = from_ring3;
    apic::end_of_interrupt();
}

/// Phase 7.5 BUILD-A: the xHCI MSI body. The asm entry exists
/// unconditionally (the global_asm block cannot be feature-split); without
/// `usb-host` the body is an EOI-only stub no vector ever targets. I-14(a):
/// the handler touches only USB state — no AI-governance lock is reachable.
#[unsafe(no_mangle)]
extern "C" fn xhci_msi_interrupt_body(from_ring3: u64) {
    record_irq_entry(IRQ_RING3_XHCI, from_ring3);
    #[cfg(feature = "usb-host")]
    crate::drivers::usb::xhci::handle_interrupt();
    apic::end_of_interrupt();
}

/// IRQ1 through the I/O APIC (`ps2-keyboard`): the i8042 ISR is lock-free
/// (it only moves bytes into a ring), then the local APIC EOI.
#[cfg(feature = "ps2-keyboard")]
#[unsafe(no_mangle)]
extern "C" fn i8042_interrupt_body(_from_ring3: u64) {
    crate::drivers::ps2::i8042::handle_interrupt();
    apic::end_of_interrupt();
}

/// The touchpad's level-triggered GSI through the I/O APIC
/// (`i2c-hid-touchpad`): the ISR masks the line and signals the service
/// thread (lock-free), then the local APIC EOI. The masked entry stays
/// masked until the thread has read the report.
#[cfg(feature = "i2c-hid-touchpad")]
#[unsafe(no_mangle)]
extern "C" fn i2c_hid_interrupt_body(_from_ring3: u64) {
    crate::drivers::i2c_hid::touchpad::handle_interrupt();
    apic::end_of_interrupt();
}

#[unsafe(no_mangle)]
extern "C" fn virtio_accel_msi_interrupt_body(from_ring3: u64) {
    record_irq_entry(IRQ_RING3_VIRTIO_ACCEL, from_ring3);
    #[cfg(feature = "virtio-accel")]
    crate::drivers::accel::virtio_accel::handle_interrupt();
    apic::end_of_interrupt();
}

/// Test-only record of the state an interrupt body was entered with. The stub
/// must hand every body DF=0 (CP-39), AC=0 (CP-40) and the kernel's default
/// MXCSR (CP-41), whatever the interrupted context had.
#[cfg(feature = "neo-test-irq-xmm-preserve")]
pub struct IrqBodyEntryFlags {
    runs: AtomicU64,
    df_set: AtomicU64,
    ac_set: AtomicU64,
    mxcsr_bad: AtomicU64,
}

#[cfg(feature = "neo-test-irq-xmm-preserve")]
#[derive(Clone, Copy)]
pub struct IrqBodyEntryCounts {
    pub runs: u64,
    pub df_set: u64,
    pub ac_set: u64,
    pub mxcsr_bad: u64,
}

#[cfg(feature = "neo-test-irq-xmm-preserve")]
impl IrqBodyEntryFlags {
    const fn new() -> Self {
        Self {
            runs: AtomicU64::new(0),
            df_set: AtomicU64::new(0),
            ac_set: AtomicU64::new(0),
            mxcsr_bad: AtomicU64::new(0),
        }
    }

    /// Call first thing in the body, before any code that could change RFLAGS
    /// or MXCSR.
    #[inline(always)]
    pub fn note(&self) {
        let rflags: u64;
        // SAFETY: reads RFLAGS through the stack; no other state is touched.
        unsafe {
            core::arch::asm!("pushfq", "pop {}", out(reg) rflags, options(preserves_flags));
        }
        let mut mxcsr = 0u32;
        // SAFETY: STMXCSR stores the control/status register to the local.
        unsafe {
            core::arch::asm!("stmxcsr [{}]", in(reg) &mut mxcsr, options(nostack, preserves_flags));
        }
        self.runs.fetch_add(1, Ordering::Relaxed);
        if rflags & (1 << 10) != 0 {
            self.df_set.fetch_add(1, Ordering::Relaxed);
        }
        if rflags & (1 << 18) != 0 {
            self.ac_set.fetch_add(1, Ordering::Relaxed);
        }
        // Control bits only: the status flags are sticky and not the stub's business.
        if mxcsr & !0x3F != super::KERNEL_MXCSR {
            self.mxcsr_bad.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn counts(&self) -> IrqBodyEntryCounts {
        IrqBodyEntryCounts {
            runs: self.runs.load(Ordering::Relaxed),
            df_set: self.df_set.load(Ordering::Relaxed),
            ac_set: self.ac_set.load(Ordering::Relaxed),
            mxcsr_bad: self.mxcsr_bad.load(Ordering::Relaxed),
        }
    }
}

/// Entry state seen by the test-vector body (`swapgs_irq_entry`).
#[cfg(feature = "neo-test-irq-xmm-preserve")]
pub static IRQ_XMM_TEST_BODY_FLAGS: IrqBodyEntryFlags = IrqBodyEntryFlags::new();

/// Entry state seen by `timer_rust_handler` (the timer stub), every tick.
#[cfg(feature = "neo-test-irq-xmm-preserve")]
pub static TIMER_BODY_FLAGS: IrqBodyEntryFlags = IrqBodyEntryFlags::new();

/// Overwrite every xmm register the way any SSE-using Rust IRQ body may. Called
/// from the test-vector body and, in this scenario's build, from the timer body,
/// so each stub's x87/SSE save is what keeps the interrupted context intact.
#[cfg(feature = "neo-test-irq-xmm-preserve")]
#[inline(always)]
pub fn irq_test_clobber_xmm() {
    // SAFETY: only writes xmm registers, which are caller-saved for the extern "C"
    // body this is inlined into; the stub is responsible for the interrupted
    // context.
    unsafe {
        core::arch::asm!(
            "pcmpeqd xmm0, xmm0",
            "pcmpeqd xmm1, xmm1",
            "pcmpeqd xmm2, xmm2",
            "pcmpeqd xmm3, xmm3",
            "pcmpeqd xmm4, xmm4",
            "pcmpeqd xmm5, xmm5",
            "pcmpeqd xmm6, xmm6",
            "pcmpeqd xmm7, xmm7",
            "pcmpeqd xmm8, xmm8",
            "pcmpeqd xmm9, xmm9",
            "pcmpeqd xmm10, xmm10",
            "pcmpeqd xmm11, xmm11",
            "pcmpeqd xmm12, xmm12",
            "pcmpeqd xmm13, xmm13",
            "pcmpeqd xmm14, xmm14",
            "pcmpeqd xmm15, xmm15",
            out("xmm0") _, out("xmm1") _, out("xmm2") _, out("xmm3") _,
            out("xmm4") _, out("xmm5") _, out("xmm6") _, out("xmm7") _,
            out("xmm8") _, out("xmm9") _, out("xmm10") _, out("xmm11") _,
            out("xmm12") _, out("xmm13") _, out("xmm14") _, out("xmm15") _,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// Body for the test-only vector: overwrite every xmm register the way any
/// SSE-using Rust IRQ body may, and send no EOI (raised by `int`, not the LAPIC).
#[cfg(feature = "neo-test-irq-xmm-preserve")]
#[unsafe(no_mangle)]
extern "C" fn irq_xmm_test_body(_from_ring3: u64) {
    IRQ_XMM_TEST_BODY_FLAGS.note();
    irq_test_clobber_xmm();
}

#[unsafe(no_mangle)]
extern "C" fn spurious_interrupt_body(from_ring3: u64) {
    record_irq_entry(IRQ_RING3_SPURIOUS, from_ring3);
    apic::end_of_interrupt();
}

/// NMI runs on IST 1 so a corrupted thread stack can't wedge it. Phase 1.3
/// has no real NMI source we expect — this handler exists as defense-in-depth.
extern "x86-interrupt" fn nmi(sf: InterruptStackFrame) {
    // OT-90 P3b: during a shutdown, a CPU that never took the STOP IPI is
    // parked here, quietly (no dump: it would take the serial lock).
    #[cfg(feature = "power-stop-cpus")]
    crate::arch::x86_64::cpu_stop::park_from_nmi();
    dump("NMI", &sf);
    // NMI is non-fatal in principle; for Phase 1.3 we have no recovery story
    // so we halt to avoid silently masking a hardware fault.
    halt_forever();
}
