// SPDX-License-Identifier: MPL-2.0
//! OT-83: what a CPU exception does, decided from the privilege level it was
//! raised at.
//!
//! A ring-3 exception is the faulting PROCESS's problem: the kernel kills that
//! process with exit code 128 + the POSIX signal the fault corresponds to, and
//! everything else keeps running (I-14, fault containment). A ring-0 exception
//! is a kernel bug and halts, exactly as before. This module holds only the
//! decision; `arch::x86_64::idt` applies it.
//!
//! Pure and allocation-free: host-tested by `tools/xtask/src/ot83_tests.rs`
//! through `#[path]`, so keep it free of `crate::` paths and feature cfgs.

/// The exception vectors routed through [`route`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vector {
    /// #DE, vector 0: divide error.
    DivideError,
    /// #DB, vector 1: debug (e.g. ring 3 set RFLAGS.TF).
    Debug,
    /// #OF, vector 4: `into` overflow (`into` is #UD in 64-bit mode).
    Overflow,
    /// #BR, vector 5: `bound` range exceeded (`bound` is #UD in 64-bit mode).
    BoundRange,
    /// #UD, vector 6: invalid opcode.
    InvalidOpcode,
    /// #NM, vector 7: device not available.
    DeviceNotAvailable,
    /// #NP, vector 11: segment not present.
    SegmentNotPresent,
    /// #SS, vector 12: stack-segment fault (e.g. a non-canonical RSP).
    StackSegment,
    /// #GP, vector 13: general protection.
    GeneralProtection,
    /// #PF, vector 14: page fault.
    PageFault,
    /// #MF, vector 16: x87 floating-point error.
    X87FloatingPoint,
    /// #AC, vector 17: alignment check.
    AlignmentCheck,
    /// #XM, vector 19: SIMD floating-point exception.
    SimdFloatingPoint,
}

/// SIGILL.
pub const SIGILL: i32 = 4;
/// SIGTRAP.
pub const SIGTRAP: i32 = 5;
/// SIGBUS.
pub const SIGBUS: i32 = 7;
/// SIGFPE.
pub const SIGFPE: i32 = 8;
/// SIGSEGV.
pub const SIGSEGV: i32 = 11;

/// What the handler must do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// A ring-3 fault: kill the current process with this exit code.
    KillProcess { exit_code: i32 },
    /// A ring-0 fault: a kernel bug. Dump and halt, as before OT-83.
    Halt,
}

impl Vector {
    /// The mnemonic used in logs.
    pub const fn name(self) -> &'static str {
        match self {
            Vector::DivideError => "#DE",
            Vector::Debug => "#DB",
            Vector::Overflow => "#OF",
            Vector::BoundRange => "#BR",
            Vector::InvalidOpcode => "#UD",
            Vector::DeviceNotAvailable => "#NM",
            Vector::SegmentNotPresent => "#NP",
            Vector::StackSegment => "#SS",
            Vector::GeneralProtection => "#GP",
            Vector::PageFault => "#PF",
            Vector::X87FloatingPoint => "#MF",
            Vector::AlignmentCheck => "#AC",
            Vector::SimdFloatingPoint => "#XM",
        }
    }

    /// The POSIX signal for this exception in user mode: the one Linux delivers,
    /// except `#NM`. Linux never signals `#NM` (it restores the FPU state, or
    /// dies with SIGSEGV when it cannot), and here it is unreachable (`CR0.EM=0`,
    /// `CR0.TS` never set). Its SIGFPE is a Neo defensive fallback, not taken
    /// from Linux.
    pub const fn signal(self) -> i32 {
        match self {
            Vector::InvalidOpcode => SIGILL,
            Vector::Debug => SIGTRAP,
            Vector::DivideError
            | Vector::DeviceNotAvailable
            | Vector::X87FloatingPoint
            | Vector::SimdFloatingPoint => SIGFPE,
            Vector::SegmentNotPresent | Vector::StackSegment | Vector::AlignmentCheck => SIGBUS,
            Vector::Overflow
            | Vector::BoundRange
            | Vector::GeneralProtection
            | Vector::PageFault => SIGSEGV,
        }
    }

    /// The exit code a process killed by this exception reports: 128 + signal.
    pub const fn exit_code(self) -> i32 {
        128 + self.signal()
    }
}

/// Whether a saved CS selector was running at CPL 3. The CPL is the saved
/// CS's RPL (its low two bits).
pub const fn is_user_cs(cs: u16) -> bool {
    cs & 3 == 3
}

/// The routing decision for `vector` raised with saved CS `cs`.
pub const fn route(vector: Vector, cs: u16) -> Route {
    if is_user_cs(cs) {
        Route::KillProcess {
            exit_code: vector.exit_code(),
        }
    } else {
        Route::Halt
    }
}
