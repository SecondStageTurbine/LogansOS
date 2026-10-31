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

pub fn prepare_cr4_for_xsave(cr4: &mut u64) -> bool {
    if xsave_plan_supported() {
        *cr4 |= CR4_OSXSAVE;
        true
    } else {
        publish_fxsave_fallback();
        false
    }
}

pub fn finish_xsave_init(requested: bool) {
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

    let leaf = cpuid_count(0xD, 0);
    let size = align_up((leaf.ebx as usize).max(FXSAVE_SIZE), XSAVE_ALIGN);
    FPU_SAVE_AREA_SIZE.store(size as u64, Ordering::Relaxed);
    FPU_XSTATE_MASK_LO.store(REQUIRED_XSTATE_MASK as u32, Ordering::Relaxed);
    FPU_XSTATE_MASK_HI.store((REQUIRED_XSTATE_MASK >> 32) as u32, Ordering::Relaxed);
    FPU_XSAVE_ENABLED.store(1, Ordering::Release);
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
    let eax: u32;
    let ebx: u32;
    let ecx: u32;
    let edx: u32;
    // SAFETY: CPUID is a serializing CPU-query instruction. RBX is preserved
    // around the instruction because LLVM reserves it on x86_64.
    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "mov {ebx_out:e}, ebx",
            "pop rbx",
            inout("eax") leaf => eax,
            inout("ecx") subleaf => ecx,
            lateout("edx") edx,
            ebx_out = lateout(reg) ebx,
            options(nomem)
        );
    }
    CpuidResult { eax, ebx, ecx, edx }
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
