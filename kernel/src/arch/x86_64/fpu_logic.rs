// SPDX-License-Identifier: MPL-2.0
//! The XSAVE area size for an XCR0 mask, from CPUID, without trusting
//! CPUID.(EAX=0DH,ECX=0):EBX alone.
//!
//! EBX is architecturally "the size of the XSAVE area for the features
//! currently enabled in XCR0", so it is only right if whoever answers CPUID
//! knows the guest's XCR0. QEMU under Windows Hypervisor Platform (WHPX,
//! `kernel-irqchip=off`) reported 576 (x87 and SSE only) after the kernel had
//! set XCR0 to x87|SSE|AVX (2026-11-02, the preview). An area that small makes
//! every `xsave` write AVX state 256 bytes past its end, over the timer stub's
//! saved registers and iretq frame: the first tick then took a #GP(0) on
//! `iretq`.
//!
//! So the size is the largest of: EBX, the legacy region plus the XSAVE
//! header (576), and, for each enabled component i >= 2, its offset
//! (CPUID.(0DH,i):EBX) plus its size (CPUID.(0DH,i):EAX) in the standard
//! (non-compacted) format, which `xsave` uses. Those per-component values are
//! fixed properties of the CPU model, not of XCR0, so a hypervisor that gets
//! EBX wrong still reports them right. The result is rounded up to 64 bytes
//! (XSAVE's alignment).

/// The legacy (FXSAVE) region plus the 64-byte XSAVE header.
pub const XSAVE_LEGACY_AND_HEADER: usize = 512 + 64;

/// XSAVE area alignment.
pub const XSAVE_ALIGN: usize = 64;

/// One XSAVE state component's CPUID.(0DH,i) report: its size (EAX) and its
/// offset in the standard format (EBX).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Component {
    pub size: u32,
    pub offset: u32,
}

/// The XSAVE area size for `components` (each enabled component i >= 2),
/// given CPUID.(0DH,0):EBX as `ebx`.
pub fn xsave_area_size(ebx: u32, components: impl IntoIterator<Item = Component>) -> usize {
    let mut size = XSAVE_LEGACY_AND_HEADER.max(ebx as usize);
    for c in components {
        let end = (c.offset as usize).saturating_add(c.size as usize);
        size = size.max(end);
    }
    size.saturating_add(XSAVE_ALIGN - 1) & !(XSAVE_ALIGN - 1)
}

/// Whether `ebx` understates the size the components need.
pub fn ebx_understated(ebx: u32, area: usize) -> bool {
    (ebx as usize) < area
}

/// The save policy the boot CPU published (#52 review, item 1). It is fixed
/// once published: every CPU saves and restores with this mask into areas
/// of this size, so no CPU may change it afterwards.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Policy {
    /// XSAVE (`true`) or the legacy FXSAVE image (`false`).
    pub xsave: bool,
    /// The XCR0 mask every CPU saves with.
    pub mask: u64,
    /// The save-area size every buffer is allocated with.
    pub area: usize,
}

/// What an application processor found when it checked [`Policy`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ApXsave {
    /// CPUID reports XSAVE, AVX, leaf 0xD and the policy's components.
    pub plan_supported: bool,
    /// XCR0 as read back after writing the policy's mask (`None` if the CPU
    /// could not take it: no CR4.OSXSAVE).
    pub xcr0: Option<u64>,
    /// The area this CPU's own CPUID says the mask needs (`xsave_area_size`).
    pub need: usize,
}

/// Whether an application processor may come online under the policy.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ApVerdict {
    /// It can save and restore exactly the policy's state in the policy's
    /// area (always, for the FXSAVE policy: every x86-64 CPU has FXSAVE).
    Admit,
    /// The policy saves with XSAVE and this CPU cannot.
    NoXsave,
    /// XCR0 did not take the policy's mask.
    Xcr0Mismatch { xcr0: u64 },
    /// This CPU's XSAVE writes more than the policy's area holds.
    AreaTooSmall { need: usize, area: usize },
}

/// Decide whether an application processor can run under `policy`. A CPU
/// that cannot must stay offline: changing the policy to fit it would
/// resize areas other CPUs already allocated and save into.
pub fn ap_verdict(policy: Policy, ap: ApXsave) -> ApVerdict {
    if !policy.xsave {
        return ApVerdict::Admit;
    }
    if !ap.plan_supported {
        return ApVerdict::NoXsave;
    }
    match ap.xcr0 {
        None => return ApVerdict::NoXsave,
        Some(xcr0) if xcr0 != policy.mask => return ApVerdict::Xcr0Mismatch { xcr0 },
        Some(_) => {}
    }
    if ap.need > policy.area {
        return ApVerdict::AreaTooSmall {
            need: ap.need,
            area: policy.area,
        };
    }
    ApVerdict::Admit
}
