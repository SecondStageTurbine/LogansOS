// SPDX-License-Identifier: MPL-2.0
//! x86_64 FPU/SIMD state save policy.
//!
//! The `xsave-fpu` feature upgrades the scheduler/syscall/timer boundaries
//! from the legacy 512-byte FXSAVE image to an XSAVE image covering x87, SSE,
//! and AVX. Runtime CPUID decides whether XSAVE is active; unsupported CPUs
//! keep using the legacy FXSAVE/FXRSTOR helpers.

use alloc::boxed::Box;
use alloc::vec;
use core::arch::{asm, global_asm};
use core::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};

use super::context::FXSAVE_INIT_IMAGE;

const FXSAVE_SIZE: usize = 512;
const XSAVE_ALIGN: usize = 64;

const XSTATE_X87: u64 = 1 << 0;
const XSTATE_SSE: u64 = 1 << 1;
const XSTATE_AVX: u64 = 1 << 2;
const REQUIRED_XSTATE_MASK: u64 = XSTATE_X87 | XSTATE_SSE | XSTATE_AVX;

const CR4_OSXSAVE: u64 = 1 << 18;

const CPUID_1_ECX_XSAVE: u32 = 1 << 26;
const CPUID_1_ECX_AVX: u32 = 1 << 28;

#[unsafe(no_mangle)]
pub static FPU_XSAVE_ENABLED: AtomicU8 = AtomicU8::new(0);
#[unsafe(no_mangle)]
pub static FPU_SAVE_AREA_SIZE: AtomicU64 = AtomicU64::new(FXSAVE_SIZE as u64);
#[unsafe(no_mangle)]
pub static FPU_XSTATE_MASK_LO: AtomicU32 = AtomicU32::new((XSTATE_X87 | XSTATE_SSE) as u32);
#[unsafe(no_mangle)]
pub static FPU_XSTATE_MASK_HI: AtomicU32 = AtomicU32::new(0);

global_asm!(
    r#"
.global fpu_save_area_asm
.type fpu_save_area_asm, @function
fpu_save_area_asm:
    cmp byte ptr [rip + FPU_XSAVE_ENABLED], 0
    jne 1f
    fxsave [rdi]
    ret
1:
    # XSAVE does not overwrite every reserved byte in the save header. Clear
    # the CPUID-sized buffer first so later XRSTOR never sees stale stack data
    # in reserved header fields.
    mov rsi, rdi
    mov rcx, qword ptr [rip + FPU_SAVE_AREA_SIZE]
    shr rcx, 3
    xor eax, eax
    cld
    rep stosq
    mov rdi, rsi
    mov eax, dword ptr [rip + FPU_XSTATE_MASK_LO]
    mov edx, dword ptr [rip + FPU_XSTATE_MASK_HI]
    xsave [rdi]
    ret
.size fpu_save_area_asm, . - fpu_save_area_asm

.global fpu_restore_area_asm
.type fpu_restore_area_asm, @function
fpu_restore_area_asm:
    cmp byte ptr [rip + FPU_XSAVE_ENABLED], 0
    jne 2f
    fxrstor [rdi]
    ret
2:
    mov eax, dword ptr [rip + FPU_XSTATE_MASK_LO]
    mov edx, dword ptr [rip + FPU_XSTATE_MASK_HI]
    xrstor [rdi]
    ret
.size fpu_restore_area_asm, . - fpu_restore_area_asm
"#
);

unsafe extern "C" {
    /// Save the current x87/SSE(/AVX) processor state into `area`.
    ///
    /// SAFETY: `area` must point at a writable buffer of at least
    /// `save_area_size()` bytes. When XSAVE is active it must be 64-byte
    /// aligned; when fallback FXSAVE is active 16-byte alignment is sufficient.
    #[allow(dead_code)] // invoked from global_asm! switch/syscall/timer shims.
    pub fn fpu_save_area_asm(area: *mut u8);

    /// Restore the current x87/SSE(/AVX) processor state from `area`.
    ///
    /// SAFETY: `area` must point at a previously saved image with the same
    /// runtime save policy and at least `save_area_size()` readable bytes.
    /// When XSAVE is active it must be 64-byte aligned; when fallback FXSAVE
    /// is active 16-byte alignment is sufficient.
    #[allow(dead_code)] // invoked from global_asm! switch/syscall/timer shims.
    pub fn fpu_restore_area_asm(area: *const u8);
}

#[repr(C, align(64))]
#[derive(Clone, Copy)]
struct FpuBlock([u8; XSAVE_ALIGN]);

impl FpuBlock {
    const ZERO: Self = Self([0; XSAVE_ALIGN]);
}

pub struct FpuState {
    blocks: Box<[FpuBlock]>,
}

impl FpuState {
    /// The image a new thread starts from, built entirely from constants: no
    /// byte comes from the creating context's live registers, which hold
    /// whatever kernel code last computed (hash state, pointers) and would
    /// reach ring 3 on the thread's first switch-in (CP-45).
    ///
    /// The legacy region is [`FXSAVE_INIT_IMAGE`]: FCW = `KERNEL_X87_FCW`,
    /// MXCSR = `KERNEL_MXCSR`, and zero everywhere else (FSW, abridged FTW =
    /// every register empty, FOP/FIP/FDP, ST0-7, xmm0-15). The XSAVE header is
    /// all zero. With XSAVE active, XSTATE_BV=0 makes XRSTOR put every enabled
    /// component in its init state (x87 FNINIT state, xmm and the YMM upper
    /// halves zero); XCOMP_BV=0 selects the standard form, and the header's
    /// reserved bytes must stay zero or XRSTOR faults. MXCSR is the exception:
    /// standard-form XRSTOR loads it from the legacy region whenever SSE or AVX
    /// is requested, whatever XSTATE_BV says, so it must be `KERNEL_MXCSR` there
    /// (zero would unmask every SIMD exception). With the FXRSTOR fallback the
    /// legacy region is the whole image. Either way a new thread never inherits
    /// a ring-3 MXCSR or x87 control word (CP-41).
    pub fn new_initial() -> Self {
        let mut blocks = vec![FpuBlock::ZERO; save_area_blocks()].into_boxed_slice();
        for (block, chunk) in blocks
            .iter_mut()
            .zip(FXSAVE_INIT_IMAGE.chunks_exact(XSAVE_ALIGN))
        {
            block.0.copy_from_slice(chunk);
        }
        Self { blocks }
    }

    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.blocks.as_mut_ptr().cast::<u8>()
    }
}

pub fn save_area_size() -> usize {
    FPU_SAVE_AREA_SIZE.load(Ordering::Relaxed) as usize
}

pub fn xsave_active() -> bool {
    FPU_XSAVE_ENABLED.load(Ordering::Relaxed) != 0
}

pub fn xstate_mask() -> u64 {
    u64::from(FPU_XSTATE_MASK_LO.load(Ordering::Relaxed))
        | (u64::from(FPU_XSTATE_MASK_HI.load(Ordering::Relaxed)) << 32)
}

#[cfg(any(
    feature = "neo-test-xsave-ymm-context-switch",
    feature = "neo-test-inference-preemptible-liveness"
))]
pub fn required_xstate_mask() -> u64 {
    REQUIRED_XSTATE_MASK
}

/// Set once the boot CPU has published the save policy (#52 review, item 1).
/// From then on the policy is immutable: an AP checks it (`check_ap`) and
/// never stores it, and one that cannot run under it stays offline.
static POLICY_PUBLISHED: AtomicU8 = AtomicU8::new(0);

fn policy_published() -> bool {
    POLICY_PUBLISHED.load(Ordering::Acquire) != 0
}

pub fn prepare_cr4_for_xsave(cr4: &mut u64) -> bool {
    if policy_published() {
        // An AP: CR4.OSXSAVE only when the policy saves with XSAVE (so ring 3
        // on this CPU sees AVX only when it is saved) and this CPU can.
        if xsave_active() && xsave_plan_supported() {
            *cr4 |= CR4_OSXSAVE;
            return true;
        }
        return false;
    }
    if xsave_plan_supported() {
        *cr4 |= CR4_OSXSAVE;
        true
    } else {
        publish_fxsave_fallback();
        false
    }
}

/// Finish this CPU's FPU save setup. The boot CPU publishes the policy; an
/// AP checks that it can run under it, and the verdict says whether it may
/// come online.
pub fn finish_xsave_init(requested: bool) -> super::fpu_logic::ApVerdict {
    if policy_published() {
        return check_ap(requested);
    }
    publish_policy(requested);
    #[cfg(feature = "neo-test-xsave-ap-policy")]
    POLICY_AREA_AT_PUBLISH.store(save_area_size() as u64, Ordering::Relaxed);
    POLICY_PUBLISHED.store(1, Ordering::Release);
    super::fpu_logic::ApVerdict::Admit
}

/// An AP against the published policy. Writes only this CPU's XCR0.
fn check_ap(requested: bool) -> super::fpu_logic::ApVerdict {
    use super::fpu_logic::{ApXsave, Policy, ap_verdict};

    let policy = Policy {
        xsave: xsave_active(),
        mask: xstate_mask(),
        area: save_area_size(),
    };
    if !policy.xsave {
        return ap_verdict(policy, no_xsave_ap());
    }
    let xcr0 = if requested {
        // SAFETY: `prepare_cr4_for_xsave` set CR4.OSXSAVE only after CPUID
        // reported XSAVE, AVX, leaf 0xD and x87|SSE|AVX, the policy's mask.
        unsafe { write_xcr0(policy.mask) };
        // SAFETY: CR4.OSXSAVE is set, so XGETBV(0) is legal.
        Some(unsafe { read_xcr0() })
    } else {
        None
    };
    let need = if requested {
        ap_test_need(xsave_area_size_quiet(policy.mask).0)
    } else {
        0
    };
    let ap = ApXsave {
        plan_supported: requested,
        xcr0,
        need,
    };
    // The revert-check: an AP republishes its own policy, as before.
    #[cfg(feature = "neo-test-xsave-ap-policy-control")]
    if requested {
        FPU_SAVE_AREA_SIZE.store(need as u64, Ordering::Relaxed);
        return super::fpu_logic::ApVerdict::Admit;
    }
    ap_verdict(policy, ap)
}

fn no_xsave_ap() -> super::fpu_logic::ApXsave {
    super::fpu_logic::ApXsave {
        plan_supported: false,
        xcr0: None,
        need: 0,
    }
}

/// `xsave_ap_policy`: the first AP checked needs 64 bytes more than the
/// policy holds (it must stay offline), the second 64 bytes less (it comes
/// online, and the policy must not shrink to it).
#[cfg(feature = "neo-test-xsave-ap-policy")]
fn ap_test_need(need: usize) -> usize {
    match AP_CHECKS.fetch_add(1, Ordering::AcqRel) {
        0 => need + XSAVE_ALIGN,
        1 => need - XSAVE_ALIGN,
        _ => need,
    }
}

#[cfg(not(feature = "neo-test-xsave-ap-policy"))]
fn ap_test_need(need: usize) -> usize {
    need
}

#[cfg(feature = "neo-test-xsave-ap-policy")]
static AP_CHECKS: AtomicU32 = AtomicU32::new(0);

/// `xsave_ap_policy`: the area the boot CPU published.
#[cfg(feature = "neo-test-xsave-ap-policy")]
static POLICY_AREA_AT_PUBLISH: AtomicU64 = AtomicU64::new(0);

/// `xsave_ap_policy`: (area published by the boot CPU, APs checked).
#[cfg(feature = "neo-test-xsave-ap-policy")]
pub fn ap_policy_for_test() -> (usize, u32) {
    (
        POLICY_AREA_AT_PUBLISH.load(Ordering::Relaxed) as usize,
        AP_CHECKS.load(Ordering::Acquire),
    )
}

/// The boot CPU's half: decide and publish the policy.
fn publish_policy(requested: bool) {
    if !requested {
        publish_fxsave_fallback();
        return;
    }

    // SAFETY: `prepare_cr4_for_xsave` only returns true when CPUID reports
    // XSAVE, AVX, leaf 0xD, and x87|SSE|AVX support. The caller has already
    // written CR4.OSXSAVE before reaching this point.
    unsafe { write_xcr0(REQUIRED_XSTATE_MASK) };

    // SAFETY: CR4.OSXSAVE was set above, so XGETBV(0) is legal.
    let xcr0 = unsafe { read_xcr0() };
    if xcr0 & REQUIRED_XSTATE_MASK != REQUIRED_XSTATE_MASK {
        publish_fxsave_fallback();
        return;
    }

    let size = xsave_area_size_for(REQUIRED_XSTATE_MASK);
    FPU_SAVE_AREA_SIZE.store(size as u64, Ordering::Relaxed);
    FPU_XSTATE_MASK_LO.store(REQUIRED_XSTATE_MASK as u32, Ordering::Relaxed);
    FPU_XSTATE_MASK_HI.store((REQUIRED_XSTATE_MASK >> 32) as u32, Ordering::Relaxed);
    FPU_XSAVE_ENABLED.store(1, Ordering::Release);
}

/// Whether CPUID.(0DH,0):EBX understated the XSAVE area (seen at init).
static EBX_UNDERSTATED: AtomicU8 = AtomicU8::new(0);

/// CPUID.(0DH,0):EBX as this boot read it, for the boot log.
static EBX_SEEN: AtomicU32 = AtomicU32::new(0);

/// The XSAVE area size for `mask` (already in XCR0): see `fpu_logic`. Not
/// CPUID.(0DH,0):EBX alone, which a hypervisor can get wrong (WHPX reported
/// x87|SSE's 576 with AVX enabled); every enabled component's offset plus
/// size bounds it from below.
fn xsave_area_size_for(mask: u64) -> usize {
    let (size, ebx) = xsave_area_size_quiet(mask);
    EBX_SEEN.store(ebx, Ordering::Relaxed);
    if super::fpu_logic::ebx_understated(ebx, size) {
        EBX_UNDERSTATED.store(1, Ordering::Relaxed);
    }
    size
}

/// `xsave_area_size_for` without recording anything: `(size, CPUID EBX)`.
fn xsave_area_size_quiet(mask: u64) -> (usize, u32) {
    use super::fpu_logic::Component;

    // `xsave_ebx_understated` test: answer EBX the way WHPX did, x87|SSE's
    // size, whatever XCR0 holds.
    #[cfg(not(feature = "test-xsave-ebx-576"))]
    let ebx = cpuid_count(0xD, 0).ebx;
    #[cfg(feature = "test-xsave-ebx-576")]
    let ebx = super::fpu_logic::XSAVE_LEGACY_AND_HEADER as u32;
    let components = (2..63u32).filter(|&i| mask & (1u64 << i) != 0).map(|i| {
        let leaf = cpuid_count(0xD, i);
        Component {
            size: leaf.eax,
            offset: leaf.ebx,
        }
    });
    #[cfg(not(feature = "test-xsave-ebx-trusted"))]
    let size = super::fpu_logic::xsave_area_size(ebx, components);
    // The revert-check: size the area from EBX alone, as before.
    #[cfg(feature = "test-xsave-ebx-trusted")]
    let size = {
        drop(components);
        align_up((ebx as usize).max(FXSAVE_SIZE), XSAVE_ALIGN)
    };
    (size, ebx)
}

/// CPUID.(0DH,0):EBX as read at init, and whether it understated the area
/// the enabled components need (the size used is then larger than EBX).
pub fn ebx_report() -> (u32, bool) {
    (
        EBX_SEEN.load(Ordering::Relaxed),
        EBX_UNDERSTATED.load(Ordering::Relaxed) != 0,
    )
}

fn xsave_plan_supported() -> bool {
    if cpuid_max_basic_leaf() < 0xD {
        return false;
    }

    let leaf1 = cpuid_count(1, 0);
    if leaf1.ecx & (CPUID_1_ECX_XSAVE | CPUID_1_ECX_AVX) != (CPUID_1_ECX_XSAVE | CPUID_1_ECX_AVX) {
        return false;
    }

    let leaf_d0 = cpuid_count(0xD, 0);
    let supported_mask = u64::from(leaf_d0.eax) | (u64::from(leaf_d0.edx) << 32);
    supported_mask & REQUIRED_XSTATE_MASK == REQUIRED_XSTATE_MASK
}

fn publish_fxsave_fallback() {
    FPU_XSAVE_ENABLED.store(0, Ordering::Release);
    FPU_SAVE_AREA_SIZE.store(FXSAVE_SIZE as u64, Ordering::Relaxed);
    FPU_XSTATE_MASK_LO.store((XSTATE_X87 | XSTATE_SSE) as u32, Ordering::Relaxed);
    FPU_XSTATE_MASK_HI.store(0, Ordering::Relaxed);
}

fn save_area_blocks() -> usize {
    align_up(save_area_size(), XSAVE_ALIGN) / XSAVE_ALIGN
}

fn align_up(value: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    (value + align - 1) & !(align - 1)
}

struct CpuidResult {
    eax: u32,
    ebx: u32,
    ecx: u32,
    edx: u32,
}

fn cpuid_max_basic_leaf() -> u32 {
    cpuid_count(0, 0).eax
}

fn cpuid_count(leaf: u32, subleaf: u32) -> CpuidResult {
    // `__cpuid_count`, not hand-written `push rbx; cpuid; mov {out:e}, ebx;
    // pop rbx`: with a `reg` output the allocator may pick rbx itself, and the
    // `pop` then discards CPUID's EBX (seen in the opt-level-3 product kernel,
    // #52 follow-up: an AP read CPUID.(0DH,2):EBX as a stack address).
    let r = core::arch::x86_64::__cpuid_count(leaf, subleaf);
    CpuidResult {
        eax: r.eax,
        ebx: r.ebx,
        ecx: r.ecx,
        edx: r.edx,
    }
}

/// Read XCR0.
///
/// SAFETY: the caller must ensure CR4.OSXSAVE is set before executing XGETBV,
/// otherwise the CPU raises #UD/#GP depending on platform state.
unsafe fn read_xcr0() -> u64 {
    let eax: u32;
    let edx: u32;
    // SAFETY: upheld by the function contract.
    unsafe {
        asm!(
            "xgetbv",
            in("ecx") 0_u32,
            lateout("eax") eax,
            lateout("edx") edx,
            options(nomem, nostack, preserves_flags)
        );
    }
    u64::from(eax) | (u64::from(edx) << 32)
}

/// Write XCR0.
///
/// SAFETY: the caller must ensure CR4.OSXSAVE is set and `value` contains only
/// CPUID.0xD-supported xstate bits with x87 enabled when any dependent state
/// is enabled. Invalid masks raise #GP.
unsafe fn write_xcr0(value: u64) {
    // SAFETY: upheld by the function contract.
    unsafe {
        asm!(
            "xsetbv",
            in("ecx") 0_u32,
            in("eax") value as u32,
            in("edx") (value >> 32) as u32,
            options(nomem, nostack, preserves_flags)
        );
    }
}
