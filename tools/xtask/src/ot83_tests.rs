// SPDX-License-Identifier: MPL-2.0
//! OT-83 host KAT: the ring-3 exception routing decision the kernel's IDT
//! handlers apply (`kernel/src/user_fault_logic.rs`, `#[path]`-included so the
//! test exercises the kernel's own code), checked against an independent
//! hand-written table (the byte-oracle rule: not derived from the code under
//! test).

#[path = "../../../kernel/src/user_fault_logic.rs"]
mod user_fault_logic;

use user_fault_logic::{Route, Vector, is_user_cs, route};

/// Vector → exit code a ring-3 fault kills the process with: 128 + the signal
/// Linux delivers for it (SIGILL 4, SIGTRAP 5, SIGBUS 7, SIGFPE 8, SIGSEGV 11).
/// `#NM` is the exception: Linux repairs it (or dies with SIGSEGV) and never
/// signals it; its 136 is a Neo defensive fallback for an unreachable vector.
const EXPECTED: &[(Vector, &str, i32)] = &[
    (Vector::DivideError, "#DE", 136),
    (Vector::Debug, "#DB", 133),
    (Vector::Overflow, "#OF", 139),
    (Vector::BoundRange, "#BR", 139),
    (Vector::InvalidOpcode, "#UD", 132),
    (Vector::DeviceNotAvailable, "#NM", 136),
    (Vector::SegmentNotPresent, "#NP", 135),
    (Vector::StackSegment, "#SS", 135),
    (Vector::GeneralProtection, "#GP", 139),
    (Vector::PageFault, "#PF", 139),
    (Vector::X87FloatingPoint, "#MF", 136),
    (Vector::AlignmentCheck, "#AC", 135),
    (Vector::SimdFloatingPoint, "#XM", 136),
];

/// Selectors (kernel/src/arch/x86_64/gdt.rs): kernel code 0x08 (RPL 0); user data 0x23 and user code 0x2b
/// (RPL 3). Any RPL-3 selector is user mode, any RPL-0 one is kernel mode.
const KERNEL_CS: u16 = 0x08;
const USER_CS: [u16; 2] = [0x23, 0x2b];

#[test]
fn ring3_exceptions_kill_the_process_with_128_plus_signal() {
    for &(vector, name, code) in EXPECTED {
        assert_eq!(vector.name(), name);
        for cs in USER_CS {
            assert_eq!(
                route(vector, cs),
                Route::KillProcess { exit_code: code },
                "{name} at cs={cs:#x}"
            );
        }
    }
}

#[test]
fn ring0_exceptions_still_halt() {
    for &(vector, name, _) in EXPECTED {
        assert_eq!(
            route(vector, KERNEL_CS),
            Route::Halt,
            "{name} at the kernel CS"
        );
        assert_eq!(route(vector, 0), Route::Halt, "{name} at a null CS");
    }
}

#[test]
fn cpl_is_the_saved_cs_rpl_for_every_selector() {
    for cs in 0..=u16::MAX {
        assert_eq!(is_user_cs(cs), cs & 3 == 3, "cs={cs:#x}");
        // RPL 1 and 2 are not user mode: they halt like ring 0.
        if cs & 3 == 1 || cs & 3 == 2 {
            assert_eq!(route(Vector::InvalidOpcode, cs), Route::Halt);
        }
    }
}

#[test]
fn every_vector_is_in_the_expected_table() {
    // A vector added to the enum without an expectation here would be unchecked.
    let src = include_str!("../../../kernel/src/user_fault_logic.rs");
    let body = &src[src.find("pub enum Vector {").unwrap()..];
    let body = &body[..body.find('}').unwrap()];
    let variants: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//") && l.ends_with(','))
        .map(|l| l.trim_end_matches(','))
        .collect();
    assert_eq!(variants.len(), EXPECTED.len(), "variants: {variants:?}");
}

#[test]
fn page_fault_exit_code_is_unchanged_from_c1_1() {
    // OT-61 C1.1 shipped 139 for a ring-3 #PF; OT-83 must not change it.
    assert_eq!(Vector::PageFault.exit_code(), 139);
}
