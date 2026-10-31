// SPDX-License-Identifier: MPL-2.0
//! Intel Gen9.5 BUILD-4a minimal compute bring-up.
//!
//! This driver is deliberately narrow: one UHD 630 / Gen9.5 Add proof behind
//! the default-off `accel-intel` feature. It never falls back to CPU compute;
//! Stage-B bare metal must prove the real GPU path or report the failure.

extern crate alloc;

use alloc::sync::Arc;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use core::time::Duration;

use x86_64::VirtAddr;

use crate::ai::ops::OpKind;
use crate::ai::tensor::{Device, Dtype, PAGE_SIZE};
use crate::drivers::block::DmaBuffer;
use crate::drivers::pci::{self, Device as PciDevice};
use crate::sched;
use crate::serial_println;
use crate::sync::{IrqSafeMutex, SwitchMutex};

use super::intel_gen9_4a::{
    ADD_BATCH_BYTES, ADD_F32X2_KERNEL_GEN9, ADD_VECTOR_BYTES, ADD_VECTOR_ELEMENTS,
    BLUR_BATCH_BYTES, BLUR_KERNEL_GEN9, BUILD4A_LAYOUT, BlurBatchLayout, CURBE_BYTES,
    CURBE_CROSS_THREAD_GRF, CURBE_PAYLOAD_BYTES, CURBE_PER_THREAD_GRF, GELU_CROSS_THREAD_GRF,
    GELU_F32_KERNEL_GEN9, GGTT_CURBE_BASE, GGTT_KERNEL_BASE, GGTT_LHS_BASE, GGTT_OUTPUT_BASE,
    GGTT_RHS_BASE, GGTT_RING_BASE, GGTT_SEQNO_BASE, GGTT_STATE_BASE, KERNEL_BYTES,
    MATMUL_F32_KERNEL_GEN9, NOOP_KERNEL_EOT_SOURCE, NOOP_KERNEL_GEN9, RING_BYTES,
    SMOKE_BATCH_BYTES, STATE_BYTES, WALKER_INDIRECT_DATA_LENGTH, add_kernel_has_pinned_eot_tail,
    add_oracle_output, blur_group_dims, blur_interface_descriptor_dwords, blur_kernel_has_eot,
    build_blur_curbe_payload, build_blur_local_id_payload, build_curbe_payload,
    build_gelu_curbe_payload, build_local_id_payload_n, build_matmul_curbe_payload,
    encode_add_batch, encode_blur_batch, encode_blur_h_batch, encode_gelu_batch,
    encode_matmul_batch, encode_noop_batch, encode_smoke_batch, gelu_kernel_has_eot,
    matmul_kernel_has_eot, noop_kernel_has_eot,
};
use super::intel_gen9_execlist as execlist;
use super::intel_gen9_ppgtt::{
    self, ENTRIES_PER_TABLE, PDP_LOAD_BYTES, PPGTT_LHS_VA, PPGTT_OUTPUT_VA, PPGTT_RHS_VA,
    dir_entry, encode_pdp_load, gfx_mode_clear_48b, leaf_pte, pd_index, pdp_index, pt_index,
};
use super::{
    AccelDescriptor, AccelDispatchKind, AccelDriver, AccelError, AccelMemoryKind, AccelResult,
    BlurDispatchMode, ComputeBindings, ComputePipeline, DeviceStorage, DispatchStatus,
    DispatchToken, OpBatch,
};

/// Phase 7 BUILD-4-1: when set, the Intel compute path maps the tenant's data buffers
/// (lhs/rhs/output) through a per-tenant **PPGTT** instead of GGTT-global, loads the
/// PPGTT root on the legacy ring before the batch, and puts the engine in 48-bit PPGTT
/// mode. Default off — the BUILD-4a GGTT path (and `accel_intel_add_bringup`) is
/// byte-for-byte unchanged. The `accel_intel_ppgtt_bringup` scenario sets this before
/// dispatching the self-proving Add/matmul/GELU.
static PPGTT_ENABLED: AtomicBool = AtomicBool::new(false);

/// Enable/disable the BUILD-4-1 PPGTT data path (see [`PPGTT_ENABLED`]).
pub fn set_ppgtt_enabled(on: bool) {
    PPGTT_ENABLED.store(on, Ordering::Release);
}

/// The PML4 root (the "PDP root") loaded for the most recent PPGTT batch — the ISC-4
/// capture. 0 = no PPGTT batch submitted. Read by the gate scenario to confirm a root
/// was captured before the run.
static PPGTT_LAST_ROOT: AtomicU64 = AtomicU64::new(0);

/// The last PPGTT root loaded (ISC-4 capture). See [`PPGTT_LAST_ROOT`].
pub fn debug_ppgtt_last_root() -> u64 {
    PPGTT_LAST_ROOT.load(Ordering::Acquire)
}

pub fn debug_blur_walker_kicks() -> u64 {
    BLUR_WALKER_KICKS.load(Ordering::Acquire)
}

pub fn debug_blur_completions() -> u64 {
    BLUR_COMPLETIONS.load(Ordering::Acquire)
}

pub fn debug_blur_mid_captured() -> bool {
    BLUR_LAST_MID_CAPTURED.load(Ordering::Acquire)
}

pub fn debug_blur_dst_captured() -> bool {
    BLUR_LAST_DST_CAPTURED.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn reset_blur_evidence() {
    BLUR_WALKER_KICKS.store(0, Ordering::Release);
    BLUR_COMPLETIONS.store(0, Ordering::Release);
    BLUR_LAST_MID_CAPTURED.store(false, Ordering::Release);
    BLUR_LAST_DST_CAPTURED.store(false, Ordering::Release);
    PPGTT_LAST_ROOT.store(0, Ordering::Release);
    LAST_COMMAND_ORACLE_MATCH.store(false, Ordering::Release);
    LAST_EXECLIST_ORACLE_MATCH.store(false, Ordering::Release);
    BLUR_FORCE_OBSERVER_TIMEOUT.store(false, Ordering::Release);
    BLUR_FORCE_OUT_OF_DOMAIN_WEIGHTS.store(false, Ordering::Release);
}

/// Let a real blur batch retire but suppress completion observation until the
/// bounded wait expires. The timeout path then quarantines the device.
#[cfg(feature = "neo-test")]
pub fn set_blur_force_observer_timeout(enabled: bool) {
    BLUR_FORCE_OBSERVER_TIMEOUT.store(enabled, Ordering::Release);
}

/// Narrowed-domain negative control: allocate only the weights page outside
/// the graphics arena so the iGPU's real read must raise a VT-d fault.
#[cfg(feature = "neo-test")]
pub fn set_blur_force_out_of_domain_weights(enabled: bool) {
    BLUR_FORCE_OUT_OF_DOMAIN_WEIGHTS.store(enabled, Ordering::Release);
}

const DEVICE_ID: Device = Device::Accel(3);
const INTEL_VENDOR_ID: u16 = 0x8086;
const GTTMMADR_BAR: u8 = 0;
const GTTMMADR_LEN: usize = 16 * 1024 * 1024;
const GGTT_PTE_BASE: usize = 8 * 1024 * 1024;
const GGTT_PAGE_SIZE: u64 = 4096;
const GGTT_PTE_PRESENT_LLC: u64 = 0x3;
/// Slice-4 pages beyond the Phase-6 0x130000..0x134000 data/CURBE window.
const GGTT_BLUR_WEIGHTS_BASE: u64 = 0x0013_5000;
const GGTT_BLUR_CURBE_V_BASE: u64 = 0x0013_6000;

const RENDER_RING_MMIO_BASE: usize = 0x02000;
const RING_TAIL: usize = RENDER_RING_MMIO_BASE + 0x30;
const RING_HEAD: usize = RENDER_RING_MMIO_BASE + 0x34;
const RING_START: usize = RENDER_RING_MMIO_BASE + 0x38;
const RING_CTL: usize = RENDER_RING_MMIO_BASE + 0x3c;
const RING_ACTHD_UDW: usize = RENDER_RING_MMIO_BASE + 0x5c;
const RING_INSTDONE: usize = RENDER_RING_MMIO_BASE + 0x6c;
const RING_ACTHD: usize = RENDER_RING_MMIO_BASE + 0x74;
const RING_R_PWR_CLK_STATE: usize = RENDER_RING_MMIO_BASE + 0xc8;
const RING_MODE: usize = RENDER_RING_MMIO_BASE + 0x29c;
const RING_CTL_VALID: u32 = 1;
const RING_CTL_SIZE_MASK: u32 = 0x001f_f000;
const GFX_RUN_LIST_ENABLE: u32 = 1 << 15;

// ── Phase 7 BUILD-4X-1: execlist/LRC GGTT layout ─────────────────────────────────
// The LRC (Logical Ring Context) and the per-engine HWSP get their own GGTT windows
// above the 4a window (ring 0x10_0000 … curbe 0x13_4000, and the 4-1 PD experiment at
// 0x14_0000). The ring/state/kernel/data/seqno pages reuse the proven 4a GGTT bases.
/// GGTT base of the LRC's first page (= the LRCA in the context descriptor). 22 pages.
const GGTT_LRC_BASE: u64 = 0x0020_0000;
/// Pages mapped for the LRC. PPHWSP (page 0) + register-state (page 1) are populated;
/// the rest cover the Gen9 RCS context extent so an engine context-save never touches
/// an unmapped page (i915 `GEN8_LR_CONTEXT_RENDER_SIZE`).
const LRC_PAGES: usize = 22;
/// GGTT base of the per-engine Hardware Status Page (programmed into RING_HWS_PGA).
const GGTT_HWSP_BASE: u64 = 0x0024_0000;
/// Software context id placed in the descriptor's upper dword (any nonzero value).
const EXECLIST_CTX_ID: u32 = 1;

const FORCEWAKE_RENDER_GEN9: usize = 0x0a278;
const FORCEWAKE_ACK_RENDER_GEN9: usize = 0x00d84;
const FORCEWAKE_GT_GEN9: usize = 0x0a188;
const FORCEWAKE_ACK_GT_GEN9: usize = 0x130044;
const FORCEWAKE_KERNEL: u32 = 1 << 0;
const FORCEWAKE_POLL_LIMIT: usize = 100_000;

const GEN8_RING_FAULT_REG_RCS: usize = 0x4094;
// iter7 ISC-5a: Gen8/9 fault-address pair (KBL PRM Vol. 2c GT fault registers,
// mirrored by i915 GEN8_FAULT_TLB_DATA0/1). On a valid RING_FAULT these hold the
// faulting GPU virtual address and the GTT-vs-PPGTT select, de-blinding a repeat
// of iter6's 0x9e1 (invalid-PTE) fault. Address reconstruction (i915
// gen8_check_faults): ((DATA1 & 0xf) << 44) | (DATA0 << 12); DATA1 bit4 = GTT sel.
const GEN8_FAULT_TLB_DATA0: usize = 0x4b10;
const GEN8_FAULT_TLB_DATA1: usize = 0x4b14;
const FAULT_TLB_DATA1_VA_HIGH_MASK: u32 = 0x0000_000f;
const FAULT_TLB_DATA1_GTT_SEL: u32 = 1 << 4;
const RING_FAULT_VALID: u32 = 1 << 0;
const GEN6_GT_CORE_STATUS: usize = 0x138060;

// Iter5 ISC-2 EU / thread-dispatch status registers (Gen9 render GT MMIO).
// These widen the iter4 `RING_INSTDONE` (0x206c) view down into the slice/row
// EU units so the next Stage-B run can name the unit that parks the GPGPU
// thread. Offsets are the Kaby Lake PRM Vol. 2c GT MMIO definitions (mirrored by
// the Linux i915 register map):
//   SC_INSTDONE      0x07100 — slice-common instruction-done.
//   SAMPLER_INSTDONE 0x0e160 — sampler instruction-done.
//   ROW_INSTDONE     0x0e100 — per-row EU instruction-done (Gen8 form).
//   TD_CTL           0x0e400 — thread-dispatch control / force-thread-breakpoint.
// ROW_INSTDONE/TD_CTL live in the multicast (MCR-steered) range; without
// programming GEN8_MCR_SELECTOR they return the boot-default steered slice/row,
// which is sufficient for a "is any EU row parked" read. Reads are best-effort
// under render forcewake and fall back to 0 via `read32`'s bounds guard.
const GEN8_SC_INSTDONE: usize = 0x07100;
const GEN7_SAMPLER_INSTDONE: usize = 0x0e160;
const GEN8_ROW_INSTDONE: usize = 0x0e100;
const GEN8_TD_CTL: usize = 0x0e400;

const MAX_WAIT_POLLS: usize = 100_000;

static SUPPORTED_OPS: &[OpKind] = &[OpKind::Add, OpKind::Matmul, OpKind::Gelu];
static SUPPORTED_DTYPES: &[Dtype] = &[Dtype::F32];

static PRIMARY_DEVICE: SwitchMutex<Option<Arc<IntelAccelDriver>>> = SwitchMutex::new(None);
static PROBED: AtomicBool = AtomicBool::new(false);
static PROBE_OK: AtomicBool = AtomicBool::new(false);
static GTT_MAPPED: AtomicBool = AtomicBool::new(false);
static LEGACY_RING_CONFIGURED: AtomicBool = AtomicBool::new(false);
static LAST_DEVICE_ID: AtomicU32 = AtomicU32::new(0);
static LAST_BDF: AtomicU32 = AtomicU32::new(0);
static SUBMISSIONS: AtomicU64 = AtomicU64::new(0);
static COMPLETIONS: AtomicU64 = AtomicU64::new(0);
static TIMEOUTS: AtomicU64 = AtomicU64::new(0);
static LAST_COMMAND_ORACLE_MATCH: AtomicBool = AtomicBool::new(false);
static LAST_OUTPUT_ORACLE_MATCH: AtomicBool = AtomicBool::new(false);
static LAST_FORCEWAKE_RENDER_ACK: AtomicU32 = AtomicU32::new(0);
static LAST_FORCEWAKE_GT_ACK: AtomicU32 = AtomicU32::new(0);
static LAST_RING_HEAD: AtomicU32 = AtomicU32::new(0);
static LAST_RING_TAIL: AtomicU32 = AtomicU32::new(0);
static LAST_RING_CTL: AtomicU32 = AtomicU32::new(0);
static LAST_RING_MODE: AtomicU32 = AtomicU32::new(0);
static LAST_RING_FAULT: AtomicU32 = AtomicU32::new(0);
static LAST_RING_INSTDONE: AtomicU32 = AtomicU32::new(0);
static LAST_GT_CORE_STATUS: AtomicU32 = AtomicU32::new(0);
static LAST_SEQNO_VALUE: AtomicU32 = AtomicU32::new(0);
static LAST_SMOKE_SEQNO_VALUE: AtomicU32 = AtomicU32::new(0);
static LAST_NOOP_SEQNO_VALUE: AtomicU32 = AtomicU32::new(0);
static LAST_MMIO_MAPPED_LEN: AtomicU32 = AtomicU32::new(0);
static LAST_SC_INSTDONE: AtomicU32 = AtomicU32::new(0);
static LAST_SAMPLER_INSTDONE: AtomicU32 = AtomicU32::new(0);
static LAST_ROW_INSTDONE: AtomicU32 = AtomicU32::new(0);
static LAST_TD_CTL: AtomicU32 = AtomicU32::new(0);
/// Number of real blur batches whose ring tail was kicked. Unlike the service's
/// completion counter, this advances at the hardware-dispatch boundary and
/// survives a timeout as the "EU was actually attempted" discriminator.
static BLUR_WALKER_KICKS: AtomicU64 = AtomicU64::new(0);
static BLUR_COMPLETIONS: AtomicU64 = AtomicU64::new(0);
static BLUR_LAST_MID_CAPTURED: AtomicBool = AtomicBool::new(false);
static BLUR_LAST_DST_CAPTURED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "neo-test")]
static BLUR_FORCE_OBSERVER_TIMEOUT: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "neo-test")]
static BLUR_FORCE_OUT_OF_DOMAIN_WEIGHTS: AtomicBool = AtomicBool::new(false);

// Phase 7 BUILD-4X-1 execlist capture (ISC-4/5): the seqno the no-op context retired,
// the descriptor submitted, the EXECLIST_STATUS readback, the CSB write pointer, and a
// count of non-zero CSB entries (the "did the engine post a switch event" discriminator).
static LAST_EXECLIST_SEQNO: AtomicU32 = AtomicU32::new(0);
static LAST_EXECLIST_DESCRIPTOR: AtomicU64 = AtomicU64::new(0);
static LAST_EXECLIST_STATUS_LO: AtomicU32 = AtomicU32::new(0);
static LAST_EXECLIST_STATUS_HI: AtomicU32 = AtomicU32::new(0);
static LAST_CSB_STATUS_PTR: AtomicU32 = AtomicU32::new(0);
static LAST_CSB_NONZERO: AtomicU32 = AtomicU32::new(0);
static LAST_EXECLIST_ORACLE_MATCH: AtomicBool = AtomicBool::new(false);
/// BUILD-4X-2: did the Add run through the execlist context (data PPGTT-only) produce
/// the correct `[4.0, 6.0]` output? Correct result + data buffers absent from GGTT =
/// the data resolved through the per-context PPGTT (the self-proving gate).
static LAST_EXECLIST_OUTPUT_MATCH: AtomicBool = AtomicBool::new(false);
/// BUILD-4X-3: the `FAULT_TLB_DATA0/1` pair from the last GT status read — used by the
/// negative test to reconstruct the faulting GPU VA and the GTT-vs-PPGTT `select` bit
/// (the gate-defining discriminators: the fault must be PPGTT, at the right VA).
static LAST_FAULT_TLB_DATA0: AtomicU32 = AtomicU32::new(0);
static LAST_FAULT_TLB_DATA1: AtomicU32 = AtomicU32::new(0);
/// BUILD-4X-3 RW-on-RO: was the RO output left unmodified (the write blocked)? Gen9 does
/// not `RING_FAULT` a write-permission fault, so this — not a fault — is the RO-write proof.
static LAST_EXECLIST_RO_WRITE_BLOCKED: AtomicBool = AtomicBool::new(false);

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ProbeError {
    UnsupportedDevice,
    MissingGttMmadr,
    AlreadyAttached,
    MmioOffsetOutOfRange,
    /// I-14(c): the graphics VT-d domain does not (yet) contain this
    /// requester, so granting it bus-master rights would create an
    /// uncontained DMA master. The probe refuses instead.
    ContainmentNotArmed,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct ProbeSnapshot {
    pub probed: bool,
    pub probe_ok: bool,
    pub gtt_mapped: bool,
    pub legacy_ring_configured: bool,
    pub device_id: u32,
    pub bdf: u32,
    pub submissions: u64,
    pub completions: u64,
    pub timeouts: u64,
    pub last_command_oracle_match: bool,
    pub last_output_oracle_match: bool,
    pub forcewake_render_ack: u32,
    pub forcewake_gt_ack: u32,
    pub ring_head: u32,
    pub ring_tail: u32,
    pub ring_ctl: u32,
    pub ring_mode: u32,
    pub ring_fault: u32,
    pub ring_instdone: u32,
    pub gt_core_status: u32,
    pub seqno_value: u32,
    pub mmio_mapped_len: u32,
}

pub fn init() {
    let summary = pci::scan_intel_gpu_devices();
    serial_println!(
        "[ kernel ] accel: intel scan complete functions={} candidates={} probe_ok={}",
        summary.functions_seen,
        summary.intel_gpu_seen,
        summary.intel_gpu_probe_ok
    );
}

/// VT-d presence/state for the containment-refusal breadcrumb. Compiled in
/// both configurations so the refusal message can always discriminate "VT-d is
/// off in firmware" (the expected cause) from "VT-d is up but the graphics
/// domain failed to arm" (a real bug worth chasing).
#[cfg(feature = "vtd")]
fn vtd_available() -> bool {
    crate::drivers::iommu::available()
}

#[cfg(not(feature = "vtd"))]
fn vtd_available() -> bool {
    false
}

#[cfg(feature = "vtd")]
fn vtd_enabled() -> bool {
    crate::drivers::iommu::enabled()
}

#[cfg(not(feature = "vtd"))]
fn vtd_enabled() -> bool {
    false
}

pub fn probe_pci(dev: PciDevice) -> Result<(), ProbeError> {
    PROBED.store(true, Ordering::Release);
    LAST_DEVICE_ID.store(dev.device_id as u32, Ordering::Release);
    LAST_BDF.store(pack_bdf(dev), Ordering::Release);
    LAST_COMMAND_ORACLE_MATCH.store(false, Ordering::Release);
    LAST_OUTPUT_ORACLE_MATCH.store(false, Ordering::Release);

    let Some(sku) = gen9_sku_name(dev.device_id).filter(|_| dev.vendor_id == INTEL_VENDOR_ID)
    else {
        return Err(ProbeError::UnsupportedDevice);
    };
    {
        if PRIMARY_DEVICE.lock().is_some() {
            return Err(ProbeError::AlreadyAttached);
        }
    }

    // I-14(c) fail-closed ordering (Slice-4 AAG bounce, 2026-08-01).
    //
    // Enable *decode only* first: MMIO reads/writes are host-initiated and
    // cannot reach memory the device does not already own, so mapping and
    // validating the GT registers ahead of containment is safe. Bus mastering
    // — the actual DMA grant — is withheld until the graphics VT-d domain
    // provably holds this requester. Previously a single
    // `enable_memory_and_bus_mastering` granted DMA here, before any domain
    // existed, and `alloc_gpu` then handed out unrestricted buddy memory:
    // an uncontained master with unrestricted physical reach.
    pci::enable_memory_decode(dev);
    breadcrumb("mmio-map");
    // AAG finding 2: validated variant — refuses unless GTTMMADR_LEN actually
    // fits inside BAR0's advertised size, so `GtMmio`'s offset checks prove
    // "inside a real BAR", not merely "inside the window we asked for".
    let mmio_base = pci::map_bar_region_validated(dev, GTTMMADR_BAR, 0, GTTMMADR_LEN as u32)
        .ok_or(ProbeError::MissingGttMmadr)?;
    let mmio = GtMmio::new(mmio_base, GTTMMADR_LEN);
    GTT_MAPPED.store(true, Ordering::Release);
    LAST_MMIO_MAPPED_LEN.store(GTTMMADR_LEN as u32, Ordering::Release);
    breadcrumb_mmio("gt-bar-mapped", 0, GTTMMADR_LEN);
    if !validate_gt_mmio_offsets(mmio) {
        return Err(ProbeError::MmioOffsetOutOfRange);
    }

    // I-14(c) containment precondition. Registers are mapped and validated;
    // the device still has NO DMA rights. Refuse to go further unless the
    // graphics VT-d domain actually holds this requester — an accelerator that
    // can master the bus with unrestricted physical addresses is exactly what
    // I-14(c) forbids. Refusing here leaves the device decode-only and
    // unregistered, so no GPU-reachable allocation or submission can occur.
    if !crate::drivers::graphics_arena::containment_armed() {
        // DECISION 2026-08-01 condition 2: the cause must never be silent. One
        // line naming the matched device, that the graphics IOMMU domain could
        // not be armed, and the consequence — so a future bring-up reads WHY
        // there is no GPU off the log instead of re-deriving it. The usual
        // cause is VT-d disabled in firmware; `vtd_available`/`vtd_enabled`
        // discriminate that from an arena/domain failure.
        serial_println!(
            "[ kernel ] accel-intel: ContainmentNotArmed device_id=0x{:04x} sku={} bdf={:02x}:{:02x}.{} \
             vtd_available={} vtd_enabled={} narrowing={} arena={} domain_armed={} \
             — graphics IOMMU domain could not be armed; iGPU left DECODE-ONLY, bus mastering REFUSED, \
             no GPU acceleration this boot (I-14(c) fail-closed; CPU compute floor unaffected)",
            dev.device_id,
            sku,
            dev.bus,
            dev.device,
            dev.function,
            vtd_available(),
            vtd_enabled(),
            crate::drivers::graphics_arena::narrowing_enabled(),
            crate::drivers::graphics_arena::arena_reserved(),
            crate::drivers::graphics_arena::domain_armed(),
        );
        crate::log::flush();
        return Err(ProbeError::ContainmentNotArmed);
    }

    // Containment proven — now, and only now, grant DMA rights.
    pci::enable_bus_mastering(dev);
    crate::drivers::graphics_arena::mark_hardware_accelerator_bound();
    breadcrumb("bus-master-armed");

    let driver = Arc::new(IntelAccelDriver {
        inner: IrqSafeMutex::new(IntelGpuInner {
            mmio,
            bdf: pack_bdf(dev),
            device_id: dev.device_id,
            in_flight: None,
            last_completion: None,
        }),
    });
    {
        *PRIMARY_DEVICE.lock() = Some(driver.clone());
    }
    let trait_device: Arc<dyn AccelDriver> = driver.clone();
    super::register_device(trait_device);
    PROBE_OK.store(true, Ordering::Release);

    serial_println!(
        "[ kernel ] accel-intel: attached Gen9 iGPU candidate bdf={:02x}:{:02x}.{} device_id=0x{:04x} sku={} gttmmadr_len=0x{:x}",
        dev.bus,
        dev.device,
        dev.function,
        dev.device_id,
        sku,
        GTTMMADR_LEN
    );
    Ok(())
}

pub fn last_probe_snapshot() -> ProbeSnapshot {
    ProbeSnapshot {
        probed: PROBED.load(Ordering::Acquire),
        probe_ok: PROBE_OK.load(Ordering::Acquire),
        gtt_mapped: GTT_MAPPED.load(Ordering::Acquire),
        legacy_ring_configured: LEGACY_RING_CONFIGURED.load(Ordering::Acquire),
        device_id: LAST_DEVICE_ID.load(Ordering::Acquire),
        bdf: LAST_BDF.load(Ordering::Acquire),
        submissions: SUBMISSIONS.load(Ordering::Acquire),
        completions: COMPLETIONS.load(Ordering::Acquire),
        timeouts: TIMEOUTS.load(Ordering::Acquire),
        last_command_oracle_match: LAST_COMMAND_ORACLE_MATCH.load(Ordering::Acquire),
        last_output_oracle_match: LAST_OUTPUT_ORACLE_MATCH.load(Ordering::Acquire),
        forcewake_render_ack: LAST_FORCEWAKE_RENDER_ACK.load(Ordering::Acquire),
        forcewake_gt_ack: LAST_FORCEWAKE_GT_ACK.load(Ordering::Acquire),
        ring_head: LAST_RING_HEAD.load(Ordering::Acquire),
        ring_tail: LAST_RING_TAIL.load(Ordering::Acquire),
        ring_ctl: LAST_RING_CTL.load(Ordering::Acquire),
        ring_mode: LAST_RING_MODE.load(Ordering::Acquire),
        ring_fault: LAST_RING_FAULT.load(Ordering::Acquire),
        ring_instdone: LAST_RING_INSTDONE.load(Ordering::Acquire),
        gt_core_status: LAST_GT_CORE_STATUS.load(Ordering::Acquire),
        seqno_value: LAST_SEQNO_VALUE.load(Ordering::Acquire),
        mmio_mapped_len: LAST_MMIO_MAPPED_LEN.load(Ordering::Acquire),
    }
}

#[cfg(feature = "neo-test")]
pub fn debug_last_command_oracle_match() -> bool {
    LAST_COMMAND_ORACLE_MATCH.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_last_output_oracle_match() -> bool {
    LAST_OUTPUT_ORACLE_MATCH.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn add_result_tolerance_ulp() -> u32 {
    super::intel_gen9_4a::ADD_RESULT_TOLERANCE_ULP
}

#[cfg(feature = "neo-test")]
pub fn debug_run_ring_smoke(timeout: Duration) -> AccelResult<u32> {
    let Some(driver) = PRIMARY_DEVICE.lock().as_ref().cloned() else {
        return Err(AccelError::NoDevice);
    };
    let token = super::next_dispatch_token(DEVICE_ID);
    let pending = PendingSmoke::new(token)?;
    driver.inner.lock().run_ring_smoke(pending, timeout)
}

#[cfg(feature = "neo-test")]
pub fn debug_last_smoke_seqno() -> u32 {
    LAST_SMOKE_SEQNO_VALUE.load(Ordering::Acquire)
}

/// Iter5 (A)/(B) bisector: dispatch the hand-laid EOT-only no-op kernel through
/// the *same* VFE/interface-descriptor/GPGPU_WALKER path as the Add kernel. A
/// landed `noop-kernel-seqno` proves dispatch mechanics retire a thread (so the
/// Add hang is its kernel body's EU data access — suspect B); a hang here proves
/// the dispatch setup itself is the bug (suspect A).
#[cfg(feature = "neo-test")]
pub fn debug_run_noop_compute(timeout: Duration) -> AccelResult<u32> {
    let Some(driver) = PRIMARY_DEVICE.lock().as_ref().cloned() else {
        return Err(AccelError::NoDevice);
    };
    let token = super::next_dispatch_token(DEVICE_ID);
    let pending = PendingNoop::new(token)?;
    driver.inner.lock().run_noop_compute(pending, timeout)
}

#[cfg(feature = "neo-test")]
pub fn debug_last_noop_seqno() -> u32 {
    LAST_NOOP_SEQNO_VALUE.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_noop_kernel_has_eot() -> bool {
    noop_kernel_has_eot()
}

/// Phase 7 BUILD-4X-1: submit a single no-op context via the execlist/LRC path (ELSP),
/// GuC-free, and poll for the no-op seqno + CSB switch-in→idle. Proves execlist
/// submission runs a context on this Gen9 silicon (the prerequisite for the PPGTT-in-LRC
/// device boundary BUILD-4-1 falsified on the legacy ring). Returns the retired seqno.
#[cfg(feature = "neo-test")]
pub fn debug_run_execlist_bringup(timeout: Duration) -> AccelResult<u32> {
    let Some(driver) = PRIMARY_DEVICE.lock().as_ref().cloned() else {
        return Err(AccelError::NoDevice);
    };
    let token = super::next_dispatch_token(DEVICE_ID);
    let pending = PendingExeclist::new(token, ExeclistKind::NoOp)?;
    driver.inner.lock().run_execlist_bringup(pending, timeout)
}

/// Phase 7 BUILD-4X-2: run the Add through the execlist context with its **data buffers
/// (lhs/rhs/output) mapped PPGTT-only** (unmapped in GGTT). A correct `[4.0, 6.0]` result
/// proves the tensor data resolved through the per-context PPGTT — BUILD-4-1's
/// `select=PPGTT` gate, now reachable on the execlist context proven by 4X-1.
#[cfg(feature = "neo-test")]
pub fn debug_run_execlist_data_bringup(timeout: Duration) -> AccelResult<u32> {
    let Some(driver) = PRIMARY_DEVICE.lock().as_ref().cloned() else {
        return Err(AccelError::NoDevice);
    };
    let token = super::next_dispatch_token(DEVICE_ID);
    let pending = PendingExeclist::new(token, ExeclistKind::Add)?;
    driver.inner.lock().run_execlist_bringup(pending, timeout)
}

/// BUILD-4X-2: did the execlist-context Add produce the correct `[4.0, 6.0]`? With the
/// data buffers absent from GGTT, a correct result means the data resolved through PPGTT.
#[cfg(feature = "neo-test")]
pub fn debug_last_execlist_output_match() -> bool {
    LAST_EXECLIST_OUTPUT_MATCH.load(Ordering::Acquire)
}

/// Phase 7 BUILD-4X-3 cross-tenant negative test: run the Add in a context whose PML4
/// **omits** the lhs leaf (lhs VA = "tenant A's tensor", absent from this/B's page tree).
/// The EU's A64 read of lhs **must page-fault** — this returns `Err` (the seqno never
/// lands). The gate is that fault, proven correct (`select=PPGTT`, addr = lhs VA, this
/// context's PML4 root active). Paired with [`debug_run_execlist_data_bringup`] (same VA,
/// mapped → succeeds) as the positive control.
#[cfg(feature = "neo-test")]
pub fn debug_run_execlist_cross_tenant(timeout: Duration) -> AccelResult<u32> {
    let Some(driver) = PRIMARY_DEVICE.lock().as_ref().cloned() else {
        return Err(AccelError::NoDevice);
    };
    let token = super::next_dispatch_token(DEVICE_ID);
    let pending = PendingExeclist::new(token, ExeclistKind::AddCrossTenant)?;
    driver.inner.lock().run_execlist_bringup(pending, timeout)
}

/// Phase 7 BUILD-4X-3 RO-READ positive test: run the Add with lhs mapped **read-only**.
/// The EU's read of the RO tensor must **succeed** — the Add completes `[4,6]` (`Ok`),
/// proving a read through an RO PTE is allowed.
#[cfg(feature = "neo-test")]
pub fn debug_run_execlist_ro_read(timeout: Duration) -> AccelResult<u32> {
    let Some(driver) = PRIMARY_DEVICE.lock().as_ref().cloned() else {
        return Err(AccelError::NoDevice);
    };
    let token = super::next_dispatch_token(DEVICE_ID);
    let pending = PendingExeclist::new(token, ExeclistKind::AddRoRead)?;
    driver.inner.lock().run_execlist_bringup(pending, timeout)
}

/// Phase 7 BUILD-4X-3 RW-on-RO negative test: run the Add with output mapped **read-only**.
/// The EU's *write* must be **blocked**. iter1 silicon proved Gen9 does not `RING_FAULT` a
/// write-permission fault (it wedges the EU), so the block is proven by **non-completion**
/// (`Err`) **+ the output frame staying unmodified** ([`debug_last_ro_write_blocked`]).
#[cfg(feature = "neo-test")]
pub fn debug_run_execlist_ro_write(timeout: Duration) -> AccelResult<u32> {
    let Some(driver) = PRIMARY_DEVICE.lock().as_ref().cloned() else {
        return Err(AccelError::NoDevice);
    };
    let token = super::next_dispatch_token(DEVICE_ID);
    let pending = PendingExeclist::new(token, ExeclistKind::AddRoWrite)?;
    driver.inner.lock().run_execlist_bringup(pending, timeout)
}

/// BUILD-4X-3: was the RO-write's output frame left **unmodified** (all-zero) — i.e. the
/// write to the read-only page never landed? With the Add's all-RW positive control known
/// to write `[4,6]`, an unmodified output + non-completion proves the RO write was blocked.
#[cfg(feature = "neo-test")]
pub fn debug_last_ro_write_blocked() -> bool {
    LAST_EXECLIST_RO_WRITE_BLOCKED.load(Ordering::Acquire)
}

/// BUILD-4X-3 fault discriminators (from the last GT status read). The negative-test gate
/// requires each expected fault to be the *right* fault: valid, `select=PPGTT`, at the
/// expected VA, of the expected type.
#[cfg(feature = "neo-test")]
pub fn debug_last_fault_valid() -> bool {
    LAST_RING_FAULT.load(Ordering::Acquire) & RING_FAULT_VALID != 0
}
/// True if the last fault resolved through PPGTT (not GGTT) — `FAULT_TLB_DATA1` bit 4 = 0.
#[cfg(feature = "neo-test")]
pub fn debug_last_fault_select_ppgtt() -> bool {
    LAST_FAULT_TLB_DATA1.load(Ordering::Acquire) & FAULT_TLB_DATA1_GTT_SEL == 0
}
/// The faulting GPU VA: `((DATA1 & 0xf) << 44) | (DATA0 << 12)` (i915 `gen8_check_faults`).
#[cfg(feature = "neo-test")]
pub fn debug_last_fault_gpu_va() -> u64 {
    let data0 = LAST_FAULT_TLB_DATA0.load(Ordering::Acquire) as u64;
    let data1 = LAST_FAULT_TLB_DATA1.load(Ordering::Acquire) as u64;
    ((data1 & FAULT_TLB_DATA1_VA_HIGH_MASK as u64) << 44) | (data0 << 12)
}
/// The fault type (RING_FAULT bits [2:1]): 0 = not-present, 1 = write-access-violation.
#[cfg(feature = "neo-test")]
pub fn debug_last_fault_type() -> u32 {
    (LAST_RING_FAULT.load(Ordering::Acquire) >> 1) & 0x3
}

/// The no-op seqno the execlist context retired (0 = never ran). The gate's primary
/// pass signal alongside [`debug_execlist_csb_nonzero`].
#[cfg(feature = "neo-test")]
pub fn debug_last_execlist_seqno() -> u32 {
    LAST_EXECLIST_SEQNO.load(Ordering::Acquire)
}

/// Count of non-zero Context Status Buffer entries observed (the engine posts a
/// switch-in/idle event here when it processes the submission). 0 = context never ran
/// (failure-mode table mode 1) — the de-blinding discriminator the dispatch mandates.
#[cfg(feature = "neo-test")]
pub fn debug_execlist_csb_nonzero() -> u32 {
    LAST_CSB_NONZERO.load(Ordering::Acquire)
}

/// EXECLIST_STATUS (lo, hi) readback captured after submission (ISC-5).
#[cfg(feature = "neo-test")]
pub fn debug_execlist_status() -> (u32, u32) {
    (
        LAST_EXECLIST_STATUS_LO.load(Ordering::Acquire),
        LAST_EXECLIST_STATUS_HI.load(Ordering::Acquire),
    )
}

/// True when the LRC register-state image, the context descriptor, and the no-op batch
/// were re-laid in-kernel and matched the buffers handed to the engine (ISC-6).
#[cfg(feature = "neo-test")]
pub fn debug_last_execlist_oracle_match() -> bool {
    LAST_EXECLIST_ORACLE_MATCH.load(Ordering::Acquire)
}

// The device-ID allowlist itself lives in the dependency-free
// `accel_intel_ids` module so a host KAT can hold it — QEMU has no Intel iGPU,
// so the QEMU gate structurally cannot cover an allowlist.
pub use super::accel_intel_ids::{gen9_sku_name, is_supported_gen9_igpu_id};

#[derive(Copy, Clone)]
struct GtMmio {
    base: VirtAddr,
    mapped_len: usize,
}

impl GtMmio {
    fn new(base: VirtAddr, mapped_len: usize) -> Self {
        Self { base, mapped_len }
    }

    fn contains(self, offset: usize, width: usize) -> bool {
        offset
            .checked_add(width)
            .is_some_and(|end| end <= self.mapped_len)
    }

    fn write32(self, step: &str, offset: usize, value: u32) -> AccelResult<()> {
        breadcrumb_mmio(step, offset, 4);
        if !self.contains(offset, 4) {
            breadcrumb_mmio_skip(step, offset, 4, self.mapped_len);
            return Err(AccelError::InvalidRequest);
        }
        unsafe {
            // SAFETY: `contains` proves the 32-bit register lies inside the
            // mapped GTTMMADR BAR. Intel Kaby Lake PRM Vol. 2b/13 defines the
            // render-ring and forcewake MMIO offsets used by BUILD-4a.
            write_volatile(
                (self.base.as_u64() as usize + offset) as *mut u32,
                value.to_le(),
            );
        }
        Ok(())
    }

    fn write64(self, step: &str, offset: usize, value: u64) -> AccelResult<()> {
        breadcrumb_mmio(step, offset, 8);
        if !self.contains(offset, 8) {
            breadcrumb_mmio_skip(step, offset, 8, self.mapped_len);
            return Err(AccelError::InvalidRequest);
        }
        unsafe {
            // SAFETY: `contains` proves the 64-bit GGTT PTE lies inside the
            // mapped GTTMMADR BAR. KBL PRM Vol. 5 documents GGTT PTE access via
            // GTTMMADR; BUILD-4a writes only kernel-owned DMA buffer mappings.
            write_volatile(
                (self.base.as_u64() as usize + offset) as *mut u64,
                value.to_le(),
            );
        }
        Ok(())
    }

    fn read32(self, step: &str, offset: usize) -> Option<u32> {
        breadcrumb_mmio(step, offset, 4);
        if !self.contains(offset, 4) {
            breadcrumb_mmio_skip(step, offset, 4, self.mapped_len);
            return None;
        }
        Some(unsafe {
            // SAFETY: `contains` proves the 32-bit register lies inside the
            // mapped GTTMMADR BAR. The caller is responsible for holding the
            // relevant Gen9 forcewake domain before reading GT registers.
            u32::from_le(read_volatile(
                (self.base.as_u64() as usize + offset) as *const u32,
            ))
        })
    }

    unsafe fn read32_raw_unchecked(self, offset: usize) -> u32 {
        unsafe {
            // SAFETY: Callers use this only after `contains(offset, 4)` has
            // already proved the register is within the mapped GTTMMADR BAR.
            u32::from_le(read_volatile(
                (self.base.as_u64() as usize + offset) as *const u32,
            ))
        }
    }
}

fn breadcrumb(step: &str) {
    serial_println!(
        "NEO_TEST/1 event=INFO name=accel_intel_add_bringup accel_intel_breadcrumb={}",
        step
    );
    crate::log::flush();
}

fn breadcrumb_mmio(step: &str, offset: usize, width: usize) {
    serial_println!(
        "NEO_TEST/1 event=INFO name=accel_intel_add_bringup accel_intel_breadcrumb={} offset=0x{:x} width={}",
        step,
        offset,
        width
    );
    crate::log::flush();
}

fn breadcrumb_mmio_value(step: &str, offset: usize, value: u32, matched: bool) {
    serial_println!(
        "NEO_TEST/1 event=INFO name=accel_intel_add_bringup accel_intel_breadcrumb={} offset=0x{:x} value=0x{:08x} matched={}",
        step,
        offset,
        value,
        matched
    );
    crate::log::flush();
}

fn breadcrumb_mmio_skip(step: &str, offset: usize, width: usize, mapped_len: usize) {
    serial_println!(
        "NEO_TEST/1 event=INFO name=accel_intel_add_bringup accel_intel_breadcrumb=mmio-skip step={} offset=0x{:x} width={} mapped_len=0x{:x}",
        step,
        offset,
        width,
        mapped_len
    );
    crate::log::flush();
}

fn validate_gt_mmio_offsets(mmio: GtMmio) -> bool {
    let offsets = [
        ("FORCEWAKE_RENDER_GEN9", FORCEWAKE_RENDER_GEN9, 4usize),
        ("FORCEWAKE_ACK_RENDER_GEN9", FORCEWAKE_ACK_RENDER_GEN9, 4),
        ("FORCEWAKE_GT_GEN9", FORCEWAKE_GT_GEN9, 4),
        ("FORCEWAKE_ACK_GT_GEN9", FORCEWAKE_ACK_GT_GEN9, 4),
        ("RING_HEAD", RING_HEAD, 4),
        ("RING_TAIL", RING_TAIL, 4),
        ("RING_START", RING_START, 4),
        ("RING_CTL", RING_CTL, 4),
        ("RING_MODE", RING_MODE, 4),
        ("RING_ACTHD_UDW", RING_ACTHD_UDW, 4),
        ("RING_INSTDONE", RING_INSTDONE, 4),
        ("RING_ACTHD", RING_ACTHD, 4),
        ("RING_R_PWR_CLK_STATE", RING_R_PWR_CLK_STATE, 4),
        ("GEN8_RING_FAULT_REG_RCS", GEN8_RING_FAULT_REG_RCS, 4),
        ("GEN6_GT_CORE_STATUS", GEN6_GT_CORE_STATUS, 4),
        ("GGTT_PTE_BASE", GGTT_PTE_BASE, 8),
        ("GGTT_SEQNO_PTE", ggtt_pte_offset(GGTT_SEQNO_BASE), 8),
    ];
    let mut ok = true;
    for (name, offset, width) in offsets {
        if mmio.contains(offset, width) {
            serial_println!(
                "NEO_TEST/1 event=INFO name=accel_intel_add_bringup accel_intel_breadcrumb=mmio-offset-ok reg={} offset=0x{:x} width={} mapped_len=0x{:x}",
                name,
                offset,
                width,
                mmio.mapped_len
            );
            crate::log::flush();
        } else {
            ok = false;
            breadcrumb_mmio_skip(name, offset, width, mmio.mapped_len);
        }
    }
    ok
}

struct IntelAccelDriver {
    inner: IrqSafeMutex<IntelGpuInner>,
}

impl AccelDriver for IntelAccelDriver {
    fn descriptor(&self) -> AccelDescriptor {
        AccelDescriptor {
            device: DEVICE_ID,
            driver: "accel_intel",
            name: "accel_intel_gen9_0",
            memory: AccelMemoryKind::DmaBuffer,
            dispatch: AccelDispatchKind::Asynchronous,
        }
    }

    fn supported_ops(&self) -> &'static [OpKind] {
        SUPPORTED_OPS
    }

    fn supported_dtypes(&self) -> &'static [Dtype] {
        SUPPORTED_DTYPES
    }

    fn alloc(&self, size: usize, align: usize) -> AccelResult<DeviceStorage> {
        if size == 0 || align == 0 || align > PAGE_SIZE || !align.is_power_of_two() {
            return Err(AccelError::InvalidRequest);
        }
        let buffer = DmaBuffer::alloc_zeroed(size).map_err(|_| AccelError::OutOfMemory)?;
        Ok(DeviceStorage::dma(DEVICE_ID, buffer))
    }

    fn free(&self, storage: DeviceStorage) {
        drop(storage);
    }

    fn copy_to_device(&self, dst: &mut DeviceStorage, src: &[u8]) -> AccelResult<()> {
        if dst.device() != DEVICE_ID || dst.len() != src.len() {
            return Err(AccelError::InvalidRequest);
        }
        dst.as_bytes_mut().copy_from_slice(src);
        Ok(())
    }

    fn copy_from_device(&self, dst: &mut [u8], src: &DeviceStorage) -> AccelResult<()> {
        if src.device() != DEVICE_ID || src.len() != dst.len() {
            return Err(AccelError::InvalidRequest);
        }
        dst.copy_from_slice(src.as_bytes());
        Ok(())
    }

    fn dispatch(&self, batch: &mut OpBatch<'_, '_>) -> AccelResult<DispatchToken> {
        // Kernel-table op dispatcher (BUILD-4b-1). A covered opcode builds its
        // op-specific PendingCompute (kernel blob + cross-thread payload + walker
        // work-size) on the proven 4a substrate; an uncovered opcode (or a matmul too
        // large for this slice) returns Unsupported so the executor transparently
        // falls back to accel_cpu — never failing the batch (ISC-4).
        let kind = match batch.op {
            OpKind::Add => {
                validate_add_batch(batch)?;
                ComputeKind::Add
            }
            OpKind::Matmul => validate_matmul_batch(batch)?,
            OpKind::Gelu => validate_gelu_batch(batch)?,
            _ => return Err(AccelError::Unsupported),
        };
        let token = super::next_dispatch_token(DEVICE_ID);
        let use_ppgtt = PPGTT_ENABLED.load(Ordering::Acquire);
        let mut inner = self.inner.lock();
        if inner.in_flight.is_some() {
            return Err(AccelError::InvalidRequest);
        }
        let pending = PendingCompute::new(token, batch, kind, use_ppgtt)?;
        inner.submit_compute(pending)?;
        Ok(token)
    }

    /// OT-63 GPU P1 Slice 4 real Gen9 blur submission (ADR-077 v2 §8).
    ///
    /// The pinned EOT-bearing kernel, two-dimensional SIMD32 walker payloads,
    /// source/scratch/destination/weights, and completion seqno are bound into a
    /// per-submit 48-bit PPGTT. Submission uses the silicon-proven ELSP/LRC path;
    /// Gen9 legacy-ring PDP loads are not accepted because ADR-032 BUILD-4-1
    /// falsified them for EU A64 address selection. QEMU never proves this path:
    /// the dedicated bare-metal scenarios must establish byte equality and the
    /// narrowed-domain fault discriminator on UHD 630.
    fn submit_compute_pipeline(
        &self,
        pipeline: &ComputePipeline,
        bindings: &mut ComputeBindings<'_>,
    ) -> AccelResult<DispatchToken> {
        let ComputePipeline::SeparableBlur {
            width,
            height,
            stride,
            radius,
            mode: _,
        } = pipeline
        else {
            // `Noop` is the accel-sim plumbing bisector; the real Gen9 substrate
            // only carries the manifest blur pipeline.
            return Err(AccelError::Unsupported);
        };

        // Manifest-shape validation for the pinned r=8 kernel (ADR-077 v2 §1.2/§1.4).
        if *width == 0 || *height == 0 || *radius > 8 || *stride < width.saturating_mul(4) {
            return Err(AccelError::InvalidRequest);
        }
        // The pinned blob must be finalizer-EOT-terminated (else a Slice-4 run would
        // hang, not blur) — the same guard the Add/GELU paths assert before dispatch.
        if !blur_kernel_has_eot() {
            return Err(AccelError::InvalidRequest);
        }

        let token = super::next_dispatch_token(DEVICE_ID);
        let (xg, yg) = blur_group_dims(*width, *height);
        serial_println!(
            "[ kernel ] accel-intel: blur-pipeline-ready blob_eot={} groups={}x{} radius={} token={} slice4_real_submit=yes",
            blur_kernel_has_eot(),
            xg,
            yg,
            *radius,
            token.sequence
        );
        crate::log::flush();
        let use_ppgtt = PPGTT_ENABLED.load(Ordering::Acquire);
        let pending = PendingCompute::new_blur(token, pipeline, bindings, use_ppgtt)?;
        let mut inner = self.inner.lock();
        if inner.in_flight.is_some() {
            return Err(AccelError::InvalidRequest);
        }
        inner.submit_compute(pending)?;
        Ok(token)
    }

    fn poll(&self, token: DispatchToken) -> DispatchStatus {
        if token.device != DEVICE_ID {
            return DispatchStatus::Failed(AccelError::InvalidRequest);
        }
        self.inner.lock().poll(token)
    }

    fn wait(&self, token: DispatchToken, timeout: Option<Duration>) -> AccelResult<()> {
        breadcrumb("wait-begin");
        let max_polls = timeout
            .map(wait_poll_budget)
            .unwrap_or(MAX_WAIT_POLLS)
            .max(1);
        let mid_poll = (max_polls / 2).max(1);
        let mut mid_logged = false;
        for poll in 0..max_polls {
            match self.poll(token) {
                DispatchStatus::Pending => {
                    if !mid_logged && poll + 1 >= mid_poll {
                        self.inner.lock().log_wait_status(token, "wait-mid-status");
                        mid_logged = true;
                    }
                    sched::yield_now();
                    core::hint::spin_loop();
                }
                DispatchStatus::Complete => return Ok(()),
                DispatchStatus::Failed(err) => return Err(err),
            }
        }
        let mut inner = self.inner.lock();
        inner.log_wait_status(token, "wait-timeout");
        inner.mark_timeout_quarantined(token);
        TIMEOUTS.fetch_add(1, Ordering::AcqRel);
        Err(AccelError::InvalidRequest)
    }

    fn preemptible(&self) -> bool {
        false
    }

    fn preempt(&self, _token: DispatchToken) -> AccelResult<()> {
        Err(AccelError::Unsupported)
    }
}

struct IntelGpuInner {
    mmio: GtMmio,
    bdf: u32,
    device_id: u16,
    in_flight: Option<PendingCompute>,
    last_completion: Option<CompletionRecord>,
}

impl IntelGpuInner {
    fn mark_timeout_quarantined(&mut self, token: DispatchToken) {
        if self.in_flight.as_ref().map(|pending| pending.token) == Some(token) {
            // Retain the DMA allocations: an actually hung engine can still
            // touch them. Refuse future work rather than free/reuse live pages.
            self.last_completion = Some(CompletionRecord {
                token,
                status: DispatchStatus::Failed(AccelError::InvalidRequest),
            });
            serial_println!(
                "[ kernel ] accel-intel: timeout token={} device_quarantined=yes dma_buffers_retained=yes future_submits=refused",
                token.sequence
            );
            crate::log::flush();
        }
    }

    fn submit_compute(&mut self, pending: PendingCompute) -> AccelResult<()> {
        let blur = matches!(pending.output_kind, PendingOutputKind::Blur(_));
        breadcrumb(if blur {
            "submit-blur-begin"
        } else {
            "submit-add-begin"
        });
        let forcewake = ForcewakeGuard::acquire(self.mmio);
        breadcrumb("ggtt-map-begin");
        map_compute_buffers(self.mmio, &pending)?;
        breadcrumb("ggtt-map-end");

        if let Some(context) = &pending.execlist {
            // Gen9's active PPGTT comes from the LRC. The legacy-ring PDP-load
            // experiment was falsified in ADR-032 BUILD-4-1; this is the
            // silicon-proven BUILD-4X ELSP path used by Slice 4.
            map_compute_execlist_context(self.mmio, context)?;
            configure_execlist_engine(self.mmio, forcewake.state())?;
            let descriptor = context.descriptor;
            let root = context.pml4_root_phys;
            let ring_tail = context.ring_tail;
            let token = pending.token;
            PPGTT_LAST_ROOT.store(root, Ordering::Release);
            serial_println!(
                "[ kernel ] accel-intel: blur-execlist-submit token={} descriptor=0x{:016x} lrca=0x{:x} ring_tail=0x{:x} pml4_root=0x{:016x} address_space=PPGTT",
                token.sequence,
                descriptor,
                GGTT_LRC_BASE,
                ring_tail,
                root
            );
            log_ring_batch("blur-execlist", token, 0, pending.command_len);
            let pre_status = read_gt_status(
                self.mmio,
                pending.seqno.as_ptr() as *const u32,
                forcewake.state(),
            );
            log_gt_status("blur-execlist-pre-elsp", token, pre_status);
            let elsp = execlist::elsp_submit_dwords(descriptor);
            // Retain the complete working set before the engine can observe the
            // descriptor. Any MMIO failure leaves it quarantined, never freed
            // underneath a potentially live DMA master.
            self.in_flight = Some(pending);
            for (slot, dword) in elsp.iter().copied().enumerate() {
                if let Err(error) =
                    self.mmio
                        .write32("blur-elsp-write", execlist::RING_ELSP as usize, dword)
                {
                    self.mark_timeout_quarantined(token);
                    return Err(error);
                }
                serial_println!(
                    "[ kernel ] accel-intel: blur-elsp-write token={} slot={} value=0x{:08x}",
                    token.sequence,
                    slot,
                    dword
                );
            }
            // The engine latches element 0 on the fourth ELSP write. Count a
            // kick only after all four writes succeeded, never merely because
            // software constructed or retained a descriptor.
            BLUR_WALKER_KICKS.fetch_add(1, Ordering::AcqRel);
            serial_println!(
                "[ kernel ] accel-intel: blur-walker-kick token={} mechanism=ELSP writes=4 latch=final kicks={} floor_can_not_increment_this=yes",
                token.sequence,
                BLUR_WALKER_KICKS.load(Ordering::Acquire)
            );
            crate::log::flush();
            SUBMISSIONS.fetch_add(1, Ordering::AcqRel);
            dump_execlist_csb(self.mmio, token, "blur-post-elsp");
            return Ok(());
        }

        configure_legacy_ring(self.mmio, forcewake.state())?;
        LEGACY_RING_CONFIGURED.store(true, Ordering::Release);

        // Phase 7 BUILD-4-1 (ISC-2/4): put the engine in 48-bit PPGTT mode (so PDP0 is
        // interpreted as the PML4 root) and log the active PDP root that the ring's LRI
        // prefix is about to load. The capture is what 4-2's cross-tenant discriminator
        // needs ("which PPGTT was active when a fault fired") — captured before the run.
        if let Some(ppgtt) = &pending.ppgtt {
            // iter3: 32-bit legacy PPGTT — CLEAR the 48-bit bit (a prior iter1/iter2 boot
            // may have left it set; GPU GFX_MODE persists across reboot). The PDP root is
            // loaded by the LRI prefix in the ring (4 PDP registers, PDP0 = the PD).
            self.mmio.write32(
                "ppgtt-gfx-mode-clear-48b",
                intel_gen9_ppgtt::RENDER_RING_GFX_MODE as usize,
                gfx_mode_clear_48b(),
            )?;
            serial_println!(
                "[ kernel ] accel-intel: ppgtt-root-load token={} mode=32bit-legacy pdp_root(pd)=0x{:016x} gfx_mode_clear_48b=0x{:08x} lri_in_ring=true select_expected=PPGTT",
                pending.token.sequence,
                ppgtt.root_phys,
                gfx_mode_clear_48b()
            );
            crate::log::flush();
            PPGTT_LAST_ROOT.store(ppgtt.root_phys, Ordering::Release);

            // iter4: ALSO program the Gen7-era PP_DIR_BASE aliasing-PPGTT root (a
            // long-shot before concluding PPGTT needs the LRC). PP_DIR_BASE is a GGTT
            // offset of the page directory, so GGTT-map the PD at a free VA, mark all
            // directory cachelines valid, then point the root at it. The PD/PT contents
            // stay physical (the walk after the root is physical). Belt-and-suspenders
            // with the PDP LRI — whichever the engine honors.
            const PPGTT_PD_GGTT_VA: u64 = 0x14_0000;
            write_named_ggtt_pte(
                self.mmio,
                "ppgtt-pd",
                PPGTT_PD_GGTT_VA,
                ppgtt.pd.physical_addr().as_u64(),
            )?;
            self.mmio.write32(
                "ppgtt-pp-dir-dclv",
                intel_gen9_ppgtt::RENDER_RING_PP_DIR_DCLV as usize,
                intel_gen9_ppgtt::PP_DIR_DCLV_ALL_VALID,
            )?;
            self.mmio.write32(
                "ppgtt-pp-dir-base",
                intel_gen9_ppgtt::RENDER_RING_PP_DIR_BASE as usize,
                PPGTT_PD_GGTT_VA as u32,
            )?;
            serial_println!(
                "[ kernel ] accel-intel: ppgtt-pp-dir-base token={} pd_ggtt_va=0x{:x} pd_phys=0x{:016x} pp_dir_base=0x{:x} dclv=0x{:08x} (gen7 aliasing-ppgtt experiment)",
                pending.token.sequence,
                PPGTT_PD_GGTT_VA,
                ppgtt.pd.physical_addr().as_u64(),
                PPGTT_PD_GGTT_VA,
                intel_gen9_ppgtt::PP_DIR_DCLV_ALL_VALID
            );
            crate::log::flush();
        }

        serial_println!(
            "[ kernel ] accel-intel: submit {} token={} bdf=0x{:06x} device_id=0x{:04x} ring=0x{:x} seqno_ggtt=0x{:x}",
            if blur { "blur" } else { "tensor" },
            pending.token.sequence,
            self.bdf,
            self.device_id,
            GGTT_RING_BASE,
            GGTT_SEQNO_BASE
        );
        log_ring_batch("compute", pending.token, 0, pending.command_len);
        let pre_status = read_gt_status(
            self.mmio,
            pending.seqno.as_ptr() as *const u32,
            forcewake.state(),
        );
        log_gt_status("pre-kick-status", pending.token, pre_status);
        breadcrumb("submit");
        if blur {
            BLUR_WALKER_KICKS.fetch_add(1, Ordering::AcqRel);
            serial_println!(
                "[ kernel ] accel-intel: blur-walker-kick token={} kicks={} floor_can_not_increment_this=yes",
                pending.token.sequence,
                BLUR_WALKER_KICKS.load(Ordering::Acquire)
            );
            crate::log::flush();
        }
        self.mmio
            .write32("ring-tail-write", RING_TAIL, pending.command_len as u32)?;
        let status = read_gt_status(
            self.mmio,
            pending.seqno.as_ptr() as *const u32,
            forcewake.state(),
        );
        log_gt_status("post-kick-status", pending.token, status);
        SUBMISSIONS.fetch_add(1, Ordering::AcqRel);
        self.in_flight = Some(pending);
        Ok(())
    }

    fn run_ring_smoke(&mut self, pending: PendingSmoke, timeout: Duration) -> AccelResult<u32> {
        if self.in_flight.is_some() {
            return Err(AccelError::InvalidRequest);
        }
        breadcrumb("ring-smoke-begin");
        let forcewake = ForcewakeGuard::acquire(self.mmio);
        breadcrumb("ring-smoke-ggtt-map-begin");
        map_smoke_buffers(self.mmio, &pending)?;
        breadcrumb("ring-smoke-ggtt-map-end");
        configure_legacy_ring(self.mmio, forcewake.state())?;
        LEGACY_RING_CONFIGURED.store(true, Ordering::Release);
        log_ring_batch("smoke", pending.token, 0, pending.command_len);
        let pre_status = read_gt_status(
            self.mmio,
            pending.seqno.as_ptr() as *const u32,
            forcewake.state(),
        );
        log_gt_status("smoke-pre-kick-status", pending.token, pre_status);
        self.mmio.write32(
            "ring-smoke-tail-write",
            RING_TAIL,
            pending.command_len as u32,
        )?;
        let post_status = read_gt_status(
            self.mmio,
            pending.seqno.as_ptr() as *const u32,
            forcewake.state(),
        );
        log_gt_status("smoke-post-kick-status", pending.token, post_status);

        let max_polls = wait_poll_budget(timeout).max(1);
        let mid_poll = (max_polls / 2).max(1);
        let mut mid_logged = false;
        for poll in 0..max_polls {
            let seqno = unsafe { read_volatile(pending.seqno.as_ptr() as *const u32) };
            if seqno == pending.token.sequence as u32 {
                LAST_SMOKE_SEQNO_VALUE.store(seqno, Ordering::Release);
                let status = read_gt_status(
                    self.mmio,
                    pending.seqno.as_ptr() as *const u32,
                    forcewake.state(),
                );
                log_gt_status("smoke-complete-status", pending.token, status);
                serial_println!(
                    "[ kernel ] accel-intel: ring-smoke complete token={} smoke-seqno=0x{:08x}",
                    pending.token.sequence,
                    seqno
                );
                crate::log::flush();
                breadcrumb("ring-smoke-complete");
                return Ok(seqno);
            }
            if !mid_logged && poll + 1 >= mid_poll {
                let status = read_gt_status(
                    self.mmio,
                    pending.seqno.as_ptr() as *const u32,
                    forcewake.state(),
                );
                log_gt_status("smoke-mid-status", pending.token, status);
                mid_logged = true;
            }
            sched::yield_now();
            core::hint::spin_loop();
        }

        let status = read_gt_status(
            self.mmio,
            pending.seqno.as_ptr() as *const u32,
            forcewake.state(),
        );
        log_gt_status("smoke-timeout-status", pending.token, status);
        LAST_SMOKE_SEQNO_VALUE.store(status.seqno, Ordering::Release);
        serial_println!(
            "[ kernel ] accel-intel: ring-smoke timeout token={} smoke-seqno=0x{:08x}",
            pending.token.sequence,
            status.seqno
        );
        crate::log::flush();
        Err(AccelError::InvalidRequest)
    }

    fn run_noop_compute(&mut self, pending: PendingNoop, timeout: Duration) -> AccelResult<u32> {
        if self.in_flight.is_some() {
            return Err(AccelError::InvalidRequest);
        }
        breadcrumb("noop-compute-begin");
        let forcewake = ForcewakeGuard::acquire(self.mmio);
        breadcrumb("noop-ggtt-map-begin");
        map_noop_buffers(self.mmio, &pending)?;
        breadcrumb("noop-ggtt-map-end");
        configure_legacy_ring(self.mmio, forcewake.state())?;
        LEGACY_RING_CONFIGURED.store(true, Ordering::Release);
        log_ring_batch("noop", pending.token, 0, pending.command_len);
        let pre_status = read_gt_status(
            self.mmio,
            pending.seqno.as_ptr() as *const u32,
            forcewake.state(),
        );
        log_gt_status("noop-pre-kick-status", pending.token, pre_status);
        self.mmio
            .write32("noop-tail-write", RING_TAIL, pending.command_len as u32)?;
        let post_status = read_gt_status(
            self.mmio,
            pending.seqno.as_ptr() as *const u32,
            forcewake.state(),
        );
        log_gt_status("noop-post-kick-status", pending.token, post_status);

        let max_polls = wait_poll_budget(timeout).max(1);
        let mid_poll = (max_polls / 2).max(1);
        let mut mid_logged = false;
        for poll in 0..max_polls {
            let seqno = unsafe { read_volatile(pending.seqno.as_ptr() as *const u32) };
            if seqno == pending.token.sequence as u32 {
                LAST_NOOP_SEQNO_VALUE.store(seqno, Ordering::Release);
                let status = read_gt_status(
                    self.mmio,
                    pending.seqno.as_ptr() as *const u32,
                    forcewake.state(),
                );
                log_gt_status("noop-complete-status", pending.token, status);
                serial_println!(
                    "[ kernel ] accel-intel: noop-kernel complete token={} noop-kernel-seqno=0x{:08x} bisect=dispatch_ok_addbody_suspect",
                    pending.token.sequence,
                    seqno
                );
                crate::log::flush();
                breadcrumb("noop-kernel-complete");
                return Ok(seqno);
            }
            if !mid_logged && poll + 1 >= mid_poll {
                let status = read_gt_status(
                    self.mmio,
                    pending.seqno.as_ptr() as *const u32,
                    forcewake.state(),
                );
                log_gt_status("noop-mid-status", pending.token, status);
                mid_logged = true;
            }
            sched::yield_now();
            core::hint::spin_loop();
        }

        let status = read_gt_status(
            self.mmio,
            pending.seqno.as_ptr() as *const u32,
            forcewake.state(),
        );
        log_gt_status("noop-timeout-status", pending.token, status);
        LAST_NOOP_SEQNO_VALUE.store(status.seqno, Ordering::Release);
        serial_println!(
            "[ kernel ] accel-intel: noop-kernel timeout token={} noop-kernel-seqno=0x{:08x} bisect=dispatch_setup_suspect",
            pending.token.sequence,
            status.seqno
        );
        crate::log::flush();
        breadcrumb("noop-kernel-timeout");
        Err(AccelError::InvalidRequest)
    }

    /// Phase 7 BUILD-4X-1: submit the no-op LRC via the execlist path and poll for the
    /// seqno + CSB switch. The engine loads the ring pointers + PPGTT root from the LRC
    /// on context switch-in (no MMIO ring programming), runs the no-op batch from HEAD to
    /// TAIL, and posts a context-status event to the CSB. Conclusive on failure: the CSB
    /// dump + EXECLIST_STATUS readback distinguish "submission rejected" (no CSB event)
    /// from "ran but the ring is wrong" (switch-in, no seqno) — the dispatch's mode 1/2.
    #[cfg(feature = "neo-test")]
    fn run_execlist_bringup(
        &mut self,
        pending: PendingExeclist,
        timeout: Duration,
    ) -> AccelResult<u32> {
        if self.in_flight.is_some() {
            return Err(AccelError::InvalidRequest);
        }
        breadcrumb("execlist-bringup-begin");
        let forcewake = ForcewakeGuard::acquire(self.mmio);
        breadcrumb("execlist-ggtt-map-begin");
        map_execlist_buffers(self.mmio, &pending)?;
        breadcrumb("execlist-ggtt-map-end");
        // Enable execlist submission + point the engine at a valid HWSP. Unlike the
        // legacy path (which CLEARS GFX_RUN_LIST_ENABLE), this SETS it.
        configure_execlist_engine(self.mmio, forcewake.state())?;

        LAST_EXECLIST_DESCRIPTOR.store(pending.descriptor, Ordering::Release);
        PPGTT_LAST_ROOT.store(pending.pml4_root_phys, Ordering::Release);
        serial_println!(
            "[ kernel ] accel-intel: execlist-submit token={} descriptor=0x{:016x} lrca=0x{:x} ctx_id={} ring_tail=0x{:x} pml4_root=0x{:016x} oracle_match={}",
            pending.token.sequence,
            pending.descriptor,
            GGTT_LRC_BASE,
            EXECLIST_CTX_ID,
            pending.ring_tail,
            pending.pml4_root_phys,
            LAST_EXECLIST_ORACLE_MATCH.load(Ordering::Acquire)
        );
        crate::log::flush();

        // ISC-3: write the 4-DWord ELSP submit sequence (port1 empty, then port0 hi/lo;
        // the engine latches on the final write).
        let elsp = execlist::elsp_submit_dwords(pending.descriptor);
        for (i, dword) in elsp.iter().enumerate() {
            serial_println!(
                "[ kernel ] accel-intel: elsp-write token={} slot={} value=0x{:08x} reg=0x{:x}",
                pending.token.sequence,
                i,
                dword,
                execlist::RING_ELSP
            );
            crate::log::flush();
            self.mmio
                .write32("elsp-write", execlist::RING_ELSP as usize, *dword)?;
        }
        SUBMISSIONS.fetch_add(1, Ordering::AcqRel);
        dump_execlist_csb(self.mmio, pending.token, "post-elsp");

        let max_polls = wait_poll_budget(timeout).max(1);
        let mid_poll = (max_polls / 2).max(1);
        let mut mid_logged = false;
        for poll in 0..max_polls {
            let seqno = unsafe { read_volatile(pending.seqno.as_ptr() as *const u32) };
            if seqno == pending.token.sequence as u32 {
                LAST_EXECLIST_SEQNO.store(seqno, Ordering::Release);
                COMPLETIONS.fetch_add(1, Ordering::AcqRel);
                let status = read_gt_status(
                    self.mmio,
                    pending.seqno.as_ptr() as *const u32,
                    forcewake.state(),
                );
                log_gt_status("execlist-complete-status", pending.token, status);
                dump_execlist_csb(self.mmio, pending.token, "complete");
                // 4X-2: with the data buffers absent from GGTT, a correct [4.0, 6.0] proves
                // the tensor data resolved through the per-context PPGTT (the self-proving
                // gate). The no-op has no data → output check skipped. The positive paths
                // (Add, AddRoRead) must compute [4,6]; the RO-write path that *completes*
                // means RO was NOT enforced (the write landed) → blocked = false.
                if pending.kind.expects_output() {
                    let output = unsafe { pending.output.as_slice() };
                    let matches = output == add_oracle_output();
                    LAST_EXECLIST_OUTPUT_MATCH.store(matches, Ordering::Release);
                    serial_println!(
                        "[ kernel ] accel-intel: execlist-{} output={:02x?} expected={:02x?} match={} (data resolved via PPGTT; buffers absent from GGTT)",
                        pending.kind.label(),
                        output,
                        add_oracle_output(),
                        matches
                    );
                    crate::log::flush();
                } else if pending.kind == ExeclistKind::AddRoWrite {
                    // The RO-write COMPLETED → the RO bit was not enforced (the write landed).
                    LAST_EXECLIST_RO_WRITE_BLOCKED.store(false, Ordering::Release);
                    serial_println!(
                        "[ kernel ] accel-intel: execlist-ro-write COMPLETED (RO write was NOT blocked — RO bit unenforced) output={:02x?}",
                        unsafe { pending.output.as_slice() }
                    );
                    crate::log::flush();
                }
                serial_println!(
                    "[ kernel ] accel-intel: execlist-bringup complete token={} execlist-seqno=0x{:08x} csb_switch_events={} verdict=execlist_submission_runs_a_context",
                    pending.token.sequence,
                    seqno,
                    LAST_CSB_NONZERO.load(Ordering::Acquire)
                );
                crate::log::flush();
                breadcrumb("execlist-bringup-complete");
                return Ok(seqno);
            }
            if !mid_logged && poll + 1 >= mid_poll {
                let status = read_gt_status(
                    self.mmio,
                    pending.seqno.as_ptr() as *const u32,
                    forcewake.state(),
                );
                log_gt_status("execlist-mid-status", pending.token, status);
                dump_execlist_csb(self.mmio, pending.token, "mid");
                mid_logged = true;
            }
            sched::yield_now();
            core::hint::spin_loop();
        }

        let status = read_gt_status(
            self.mmio,
            pending.seqno.as_ptr() as *const u32,
            forcewake.state(),
        );
        log_gt_status("execlist-timeout-status", pending.token, status);
        dump_execlist_csb(self.mmio, pending.token, "timeout");
        LAST_EXECLIST_SEQNO.store(status.seqno, Ordering::Release);
        TIMEOUTS.fetch_add(1, Ordering::AcqRel);
        // BUILD-4X-3 RW-on-RO: the run did NOT complete. Gen9 doesn't `RING_FAULT` a
        // write-permission fault (it wedges the EU), so the RO-write block is proven here by
        // the **output frame staying unmodified** — the all-RW positive control writes [4,6],
        // so an all-zero output means the write to the RO page never landed.
        if pending.kind == ExeclistKind::AddRoWrite {
            let output = unsafe { pending.output.as_slice() };
            let blocked = output.iter().all(|&b| b == 0);
            LAST_EXECLIST_RO_WRITE_BLOCKED.store(blocked, Ordering::Release);
            serial_println!(
                "[ kernel ] accel-intel: execlist-ro-write did_not_complete output={:02x?} unmodified(blocked)={} (Gen9 wedges the EU on a write-permission fault; no RING_FAULT)",
                output,
                blocked
            );
            crate::log::flush();
        }
        // RESEARCH §7 decision tree (replaces the linear failure-mode table). Captured
        // signals: CSB events, the RING_MODE execlist-enable readback, RING_HEAD movement,
        // and the fault address space (GGTT vs PPGTT) — each maps to a distinct cause so
        // the blind run is self-diagnosing.
        let csb_events = LAST_CSB_NONZERO.load(Ordering::Acquire);
        let execlist_enabled = (status.ring_mode & GFX_RUN_LIST_ENABLE) != 0;
        let ring_head_moved = status.ring_head != 0;
        let fault_valid = (status.ring_fault & RING_FAULT_VALID) != 0;
        let fault_in_ggtt = (status.ring_fault_data1 & FAULT_TLB_DATA1_GTT_SEL) != 0;
        let branch = if csb_events == 0 {
            if ring_head_moved {
                "treeD_head_advanced_csb_zero=>HWSP/CSB_not_set_up"
            } else if !execlist_enabled {
                "treeA_no_csb+execlist_DISABLED=>check_RING_MODE_bit15+forcewake_FIRST"
            } else {
                "treeA_no_csb+execlist_enabled=>suspect_descriptor_VALID/addr_mode/ELSP_order"
            }
        } else if fault_valid {
            if fault_in_ggtt {
                "treeC_csb+fault_GGTT=>bad_ggtt_mapping(ring_start/hwsp/lrc)"
            } else {
                "treeC_csb+fault_PPGTT=>bad_LRC_image_or_PML4_root"
            }
        } else if !ring_head_moved {
            "treeB_csb_active_head_unmoved=>empty_ring/bad_CTX_RING_TAIL/same_tail_trap"
        } else {
            "treeE_csb+head_moved_no_seqno=>ring_ran_but_batch_wrong"
        };
        serial_println!(
            "[ kernel ] accel-intel: execlist-bringup timeout token={} execlist-seqno=0x{:08x} csb_switch_events={} execlist_enabled={} ring_head=0x{:08x} ring_mode=0x{:08x} fault_valid={} fault_space={} branch={}",
            pending.token.sequence,
            status.seqno,
            csb_events,
            execlist_enabled,
            status.ring_head,
            status.ring_mode,
            fault_valid,
            if fault_in_ggtt { "GGTT" } else { "PPGTT" },
            branch
        );
        crate::log::flush();
        breadcrumb("execlist-bringup-timeout");
        Err(AccelError::InvalidRequest)
    }

    fn poll(&mut self, token: DispatchToken) -> DispatchStatus {
        if let Some(record) = self.last_completion
            && record.token == token
        {
            return record.status;
        }
        let Some(pending) = self.in_flight.as_ref() else {
            return DispatchStatus::Failed(AccelError::InvalidRequest);
        };
        if pending.token != token {
            return DispatchStatus::Failed(AccelError::InvalidRequest);
        }
        // SAFETY: `in_flight` owns the four-byte DMA allocation until this
        // completion path removes it, and the GPU/CPU contract makes it a
        // volatile little-endian sequence word.
        let seqno = unsafe { read_volatile(pending.seqno.as_ptr() as *const u32) };
        if seqno != token.sequence as u32 {
            return DispatchStatus::Pending;
        }
        #[cfg(feature = "neo-test")]
        if matches!(pending.output_kind, PendingOutputKind::Blur(_))
            && BLUR_FORCE_OBSERVER_TIMEOUT.load(Ordering::Acquire)
        {
            return DispatchStatus::Pending;
        }

        let status = read_gt_status_guarded(self.mmio, pending.seqno.as_ptr() as *const u32);
        log_gt_status("wait-complete", token, status);
        if pending.execlist.is_some() {
            dump_execlist_csb(self.mmio, token, "blur-complete");
        }

        let pending = self.in_flight.take().expect("pending checked above");
        // SAFETY: the pending record owns this DMA allocation, which remains
        // live and immutable through both comparisons/copies below.
        let output = unsafe { pending.output.as_slice() };
        match pending.output_kind {
            PendingOutputKind::Tensor => {
                // SAFETY: submit validated this destination length and the
                // synchronous wait contract keeps the caller's binding live
                // until its token completes (the pre-existing tensor ABI).
                unsafe {
                    let dst = core::slice::from_raw_parts_mut(
                        pending.output_ptr as *mut u8,
                        pending.output_len,
                    );
                    dst.copy_from_slice(output);
                }
                LAST_OUTPUT_ORACLE_MATCH.store(output == add_oracle_output(), Ordering::Release);
            }
            PendingOutputKind::Blur(mode) => {
                // SAFETY: the pending record owns the mid DMA allocation for
                // the duration of this completion copy.
                let mid = unsafe { pending.rhs.as_slice() };
                // SAFETY: submit validated both binding lengths. The compute
                // service waits before returning to its caller, so its mid/dst
                // slices remain live; direct test submissions likewise retain
                // their arrays through `wait`.
                unsafe {
                    let host_mid = core::slice::from_raw_parts_mut(
                        pending.mid_ptr as *mut u8,
                        pending.mid_len,
                    );
                    host_mid.copy_from_slice(&mid[..pending.mid_len]);
                    if mode == BlurDispatchMode::TwoPass {
                        let host_dst = core::slice::from_raw_parts_mut(
                            pending.output_ptr as *mut u8,
                            pending.output_len,
                        );
                        host_dst.copy_from_slice(&output[..pending.output_len]);
                    }
                }
                BLUR_LAST_MID_CAPTURED.store(true, Ordering::Release);
                BLUR_LAST_DST_CAPTURED.store(mode == BlurDispatchMode::TwoPass, Ordering::Release);
                BLUR_COMPLETIONS.fetch_add(1, Ordering::AcqRel);
                serial_println!(
                    "[ kernel ] accel-intel: blur-capture token={} mode={:?} mid_copied=yes dst_copied={} completions={} seqno_retired=yes",
                    token.sequence,
                    mode,
                    mode == BlurDispatchMode::TwoPass,
                    BLUR_COMPLETIONS.load(Ordering::Acquire)
                );
                crate::log::flush();
            }
        }
        COMPLETIONS.fetch_add(1, Ordering::AcqRel);
        let status = DispatchStatus::Complete;
        self.last_completion = Some(CompletionRecord { token, status });
        serial_println!(
            "[ kernel ] accel-intel: complete token={} output_prefix={:02x?}",
            token.sequence,
            &output[..output.len().min(32)]
        );
        status
    }

    fn log_wait_status(&mut self, token: DispatchToken, label: &'static str) {
        let Some(pending) = self.in_flight.as_ref() else {
            serial_println!(
                "[ kernel ] accel-intel: {} token={} no_in_flight=true",
                label,
                token.sequence
            );
            return;
        };
        if pending.token != token {
            serial_println!(
                "[ kernel ] accel-intel: {} token={} pending_token={}",
                label,
                token.sequence,
                pending.token.sequence
            );
            return;
        }
        let status = read_gt_status_guarded(self.mmio, pending.seqno.as_ptr() as *const u32);
        log_gt_status(label, token, status);
    }
}

/// Which covered op a `PendingCompute` carries. The 4a dispatch substrate (ring,
/// GGTT, STATE_BASE_ADDRESS, indirect-data payload) is identical for both; only the
/// kernel blob, the cross-thread payload, and the walker work-size (live lanes)
/// differ. `Matmul` carries the row-major dims (A is m×k, B is k×n, C is m×n).
#[derive(Copy, Clone)]
enum ComputeKind {
    Add,
    Matmul {
        m: u32,
        n: u32,
        k: u32,
    },
    /// Pointwise GELU over `n` elements (the transformer MLP nonlinearity). Single
    /// input → output; the kernel reads only the `in`/`out` A64 pointers + element
    /// count (its 2-GRF cross-thread payload), so the `rhs` buffer is an unused
    /// placeholder. `n` is the element count = the walker work-item count.
    Gelu {
        n: u32,
    },
}

/// A per-tenant Gen8+ 48-bit PPGTT (Phase 7 BUILD-4-1, ISC-1): the four page-table
/// levels (PML4 → PDP → PD → PT) as physically-contiguous DMA frames, mapping the
/// tenant's data-buffer VAs to the buffers' physical frames, present + R/W. The GPU
/// walks these frames physically from the PML4 root once the root is loaded into PDP0
/// and the engine is in 48-bit mode — no GGTT entry for the tables themselves.
struct Ppgtt {
    pd: Arc<DmaBuffer>,
    pt: Arc<DmaBuffer>,
    /// Physical address of the Page Directory — the value loaded into PDP0 (the "PDP
    /// root" the discriminator logs, ISC-4). 32-bit legacy: the PDP registers are the
    /// top level, so the root is the PD itself (no PML4).
    root_phys: u64,
}

/// Write a 64-bit page-table entry at `index` into a table frame (little-endian, the
/// in-memory PTE byte format the host oracle pins).
fn write_table_entry(table: &Arc<DmaBuffer>, index: usize, entry: u64) {
    debug_assert!(index < ENTRIES_PER_TABLE);
    let bytes = unsafe { table.as_mut_slice() };
    bytes[index * 8..index * 8 + 8].copy_from_slice(&entry.to_le_bytes());
}

impl Ppgtt {
    /// Build a 32-bit legacy PPGTT (iter3) mapping the three data-buffer VAs
    /// (lhs/rhs/output — numerically the 4a GGTT VAs, but resolved here) to the buffers'
    /// physical frames, present + R/W (ISC-1). 3-level walk: the 4 PDP registers are the
    /// top level (PDP0 = this PD), then PD → PT. All three VAs fall under PDP0 → PD[0]
    /// in one PT, so the PD has a single entry and the PT holds the three leaves.
    fn build(lhs_phys: u64, rhs_phys: u64, output_phys: u64) -> AccelResult<Self> {
        // OT-63 Slice 5: the PPGTT page tables (pd/pt) are GPU-reachable — the
        // engine walks them from RAM — so they too route through `alloc_gpu`
        // (graphics arena when narrowing on; buddy bit-for-bit when off).
        let pd = DmaBuffer::alloc_gpu(PAGE_SIZE).map_err(|_| AccelError::OutOfMemory)?;
        let pt = DmaBuffer::alloc_gpu(PAGE_SIZE).map_err(|_| AccelError::OutOfMemory)?;

        let pd_phys = pd.physical_addr().as_u64();
        let pt_phys = pt.physical_addr().as_u64();

        // Directory level: PD[0] → PT (present + R/W). PDP0 (the register) → this PD is
        // programmed by the LRI prefix at submit, not a table entry.
        write_table_entry(&pd, pd_index(PPGTT_LHS_VA), dir_entry(pt_phys));

        // Leaves: each data-buffer VA → its physical frame, present + R/W (4-2 will map
        // RO-shared tensors read-only here; 4-1 is all R/W).
        write_table_entry(&pt, pt_index(PPGTT_LHS_VA), leaf_pte(lhs_phys, true));
        write_table_entry(&pt, pt_index(PPGTT_RHS_VA), leaf_pte(rhs_phys, true));
        write_table_entry(&pt, pt_index(PPGTT_OUTPUT_VA), leaf_pte(output_phys, true));

        serial_println!(
            "[ kernel ] accel-intel: ppgtt built mode=32bit-legacy root(pd/pdp0)=0x{:016x} pt=0x{:016x} pdp_idx={} pd_idx={} lhs_va=0x{:x}->0x{:016x} rhs_va=0x{:x}->0x{:016x} out_va=0x{:x}->0x{:016x}",
            pd_phys,
            pt_phys,
            pdp_index(PPGTT_LHS_VA),
            pd_index(PPGTT_LHS_VA),
            PPGTT_LHS_VA,
            lhs_phys,
            PPGTT_RHS_VA,
            rhs_phys,
            PPGTT_OUTPUT_VA,
            output_phys
        );
        crate::log::flush();

        Ok(Self {
            pd,
            pt,
            root_phys: pd_phys,
        })
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum PendingOutputKind {
    Tensor,
    Blur(BlurDispatchMode),
}

/// Per-submit Gen9 render context for the real blur path. Every command/data
/// page addressed through the context is mapped in its 48-bit PPGTT. Every
/// backing allocation, including the page tables/LRC/HWSP, routes through
/// `alloc_gpu` so Slice-5 narrowing covers the complete working set.
struct ComputeExeclistContext {
    lrc: Arc<DmaBuffer>,
    hwsp: Arc<DmaBuffer>,
    _pml4: Arc<DmaBuffer>,
    _pdp: Arc<DmaBuffer>,
    _pd: Arc<DmaBuffer>,
    _pt: Arc<DmaBuffer>,
    pml4_root_phys: u64,
    descriptor: u64,
    ring_tail: usize,
}

impl ComputeExeclistContext {
    #[allow(clippy::too_many_arguments)]
    fn new(
        token: DispatchToken,
        ring: &DmaBuffer,
        state: &DmaBuffer,
        kernel: &DmaBuffer,
        src: &DmaBuffer,
        mid: &DmaBuffer,
        dst: &DmaBuffer,
        seqno: &DmaBuffer,
        curbe_h: &DmaBuffer,
        weights: &DmaBuffer,
        curbe_v: &DmaBuffer,
        command_len: usize,
    ) -> AccelResult<Self> {
        let lrc =
            DmaBuffer::alloc_gpu(LRC_PAGES * PAGE_SIZE).map_err(|_| AccelError::OutOfMemory)?;
        let hwsp = DmaBuffer::alloc_gpu(PAGE_SIZE).map_err(|_| AccelError::OutOfMemory)?;
        let pml4 = DmaBuffer::alloc_gpu(PAGE_SIZE).map_err(|_| AccelError::OutOfMemory)?;
        let pdp = DmaBuffer::alloc_gpu(PAGE_SIZE).map_err(|_| AccelError::OutOfMemory)?;
        let pd = DmaBuffer::alloc_gpu(PAGE_SIZE).map_err(|_| AccelError::OutOfMemory)?;
        let pt = DmaBuffer::alloc_gpu(PAGE_SIZE).map_err(|_| AccelError::OutOfMemory)?;
        let root = pml4.physical_addr().as_u64();
        write_table_entry(
            &pml4,
            execlist::pml4_index(GGTT_STATE_BASE),
            dir_entry(pdp.physical_addr().as_u64()),
        );
        write_table_entry(
            &pdp,
            execlist::pdp_index_48(GGTT_STATE_BASE),
            dir_entry(pd.physical_addr().as_u64()),
        );
        write_table_entry(
            &pd,
            execlist::pd_index_48(GGTT_STATE_BASE),
            dir_entry(pt.physical_addr().as_u64()),
        );

        let mappings = [
            (GGTT_RING_BASE, ring, ring.len()),
            (GGTT_STATE_BASE, state, state.len()),
            (GGTT_KERNEL_BASE, kernel, kernel.len()),
            (GGTT_LHS_BASE, src, src.len()),
            (GGTT_RHS_BASE, mid, mid.len()),
            (GGTT_OUTPUT_BASE, dst, dst.len()),
            (GGTT_SEQNO_BASE, seqno, seqno.len()),
            (GGTT_CURBE_BASE, curbe_h, curbe_h.len()),
            (GGTT_BLUR_WEIGHTS_BASE, weights, weights.len()),
            (GGTT_BLUR_CURBE_V_BASE, curbe_v, curbe_v.len()),
        ];
        let mut leaves = 0usize;
        for (va, buffer, len) in mappings {
            for page in 0..len.div_ceil(PAGE_SIZE) {
                let offset = (page * PAGE_SIZE) as u64;
                write_table_entry(
                    &pt,
                    execlist::pt_index_48(va + offset),
                    leaf_pte(buffer.physical_addr().as_u64() + offset, true),
                );
                leaves += 1;
            }
        }

        let ring_tail = command_len
            .checked_add(execlist::WA_TAIL_BYTES)
            .filter(|tail| *tail <= RING_BYTES)
            .ok_or(AccelError::InvalidRequest)?;
        let ring_ctl =
            ((RING_BYTES as u32 - PAGE_SIZE as u32) & RING_CTL_SIZE_MASK) | RING_CTL_VALID;
        let params = execlist::LrcParams {
            ring_start: GGTT_RING_BASE as u32,
            ring_ctl,
            ring_head: 0,
            ring_tail: ring_tail as u32,
            pdp: [root, 0, 0, 0],
        };
        // SAFETY: `lrc` is exclusively owned during construction and the
        // register-state slice is inside its two-page allocation.
        unsafe {
            let bytes = lrc.as_mut_slice();
            execlist::write_lrc_reg_state(
                &mut bytes[PAGE_SIZE..PAGE_SIZE + execlist::LRC_REG_STATE_BYTES],
                params,
            )
            .ok_or(AccelError::InvalidRequest)?;
        }
        // A fresh allocation with an unchanged LRCA/tail must not be presented
        // as the same context: Gen9 may treat HEAD==TAIL as an empty resubmit.
        // Give every Slice-4 dispatch a non-zero 21-bit software context id so
        // H-only, all three equality runs, and the terminal negative are
        // distinct contexts even though their GGTT LRCA window is reused.
        let context_id = blur_execlist_context_id(token.sequence);
        let descriptor = execlist::context_descriptor(GGTT_LRC_BASE, context_id);
        // Independently re-lay the LRC bytes and descriptor. The blur command
        // stream has its own oracle in `new_blur`; together they cover both the
        // workload batch and the context state handed to ELSP.
        let mut expected_reg = [0u8; execlist::LRC_REG_STATE_BYTES];
        execlist::write_lrc_reg_state(&mut expected_reg, params)
            .ok_or(AccelError::InvalidRequest)?;
        // SAFETY: `lrc` remains exclusively owned during construction and the
        // compared range is inside its `LRC_PAGES` allocation.
        let reg_ok = unsafe {
            lrc.as_slice()[PAGE_SIZE..PAGE_SIZE + execlist::LRC_REG_STATE_BYTES] == expected_reg
        };
        let desc_ok = descriptor == execlist::context_descriptor(GGTT_LRC_BASE, context_id);
        LAST_EXECLIST_ORACLE_MATCH.store(reg_ok && desc_ok, Ordering::Release);
        serial_println!(
            "[ kernel ] accel-intel: blur-execlist-context token={} context_id={} root=0x{:016x} leaves={} kernel_pages={} ring_tail=0x{:x} descriptor=0x{:016x} lrc_oracle={}",
            token.sequence,
            context_id,
            root,
            leaves,
            kernel.len().div_ceil(PAGE_SIZE),
            ring_tail,
            descriptor,
            LAST_EXECLIST_ORACLE_MATCH.load(Ordering::Acquire)
        );
        crate::log::flush();
        Ok(Self {
            lrc,
            hwsp,
            _pml4: pml4,
            _pdp: pdp,
            _pd: pd,
            _pt: pt,
            pml4_root_phys: root,
            descriptor,
            ring_tail,
        })
    }
}

struct PendingCompute {
    token: DispatchToken,
    lhs: Arc<DmaBuffer>,
    rhs: Arc<DmaBuffer>,
    output: Arc<DmaBuffer>,
    seqno: Arc<DmaBuffer>,
    ring: Arc<DmaBuffer>,
    state: Arc<DmaBuffer>,
    kernel: Arc<DmaBuffer>,
    curbe: Arc<DmaBuffer>,
    blur_weights: Option<Arc<DmaBuffer>>,
    blur_curbe_v: Option<Arc<DmaBuffer>>,
    /// Phase 7 BUILD-4-1: the per-tenant PPGTT, present when the data buffers are
    /// mapped through PPGTT (and so absent from GGTT). `None` = the BUILD-4a GGTT path.
    ppgtt: Option<Ppgtt>,
    /// Slice-4 blur submissions use the silicon-proven ELSP/LRC path because
    /// Gen9 ignores legacy-ring PPGTT roots for EU A64 accesses (ADR-032
    /// BUILD-4-1 falsification). None only for the older legacy tensor path.
    execlist: Option<ComputeExeclistContext>,
    output_ptr: usize,
    output_len: usize,
    mid_ptr: usize,
    mid_len: usize,
    output_kind: PendingOutputKind,
    command_len: usize,
}

impl PendingCompute {
    fn new(
        token: DispatchToken,
        batch: &OpBatch<'_, '_>,
        kind: ComputeKind,
        use_ppgtt: bool,
    ) -> AccelResult<Self> {
        // Per-op inputs: buffer sizes, the bytes to stage into the rhs buffer, the
        // walker work-item count, kernel blob, cross-thread payload + its GRF read
        // length. The Add keeps its exact 4a wiring; matmul sizes A/B/C from the
        // operands (payload per matmul_f32.gen9.zeinfo); GELU is pointwise (single
        // input → output, the rhs buffer is an unused placeholder, 2 cross-thread
        // GRF per gelu_f32.gen9.zeinfo). `in1_src` is what gets copied into rhs;
        // empty for GELU (the kernel never reads rhs).
        let (in0, in1, in1_src, out_bytes, work_items, blob, eot_ok, mode, cross, cross_grf) =
            match kind {
                ComputeKind::Add => (
                    ADD_VECTOR_BYTES,
                    ADD_VECTOR_BYTES,
                    batch.inputs[1].bytes,
                    ADD_VECTOR_BYTES,
                    ADD_VECTOR_ELEMENTS as u32,
                    &ADD_F32X2_KERNEL_GEN9[..],
                    add_kernel_has_pinned_eot_tail(),
                    "add",
                    build_curbe_payload(GGTT_LHS_BASE, GGTT_RHS_BASE, GGTT_OUTPUT_BASE),
                    CURBE_CROSS_THREAD_GRF,
                ),
                ComputeKind::Matmul { m, n, k } => (
                    batch.inputs[0].bytes.len(),
                    batch.inputs[1].bytes.len(),
                    batch.inputs[1].bytes,
                    batch.output.len(),
                    m.saturating_mul(n),
                    &MATMUL_F32_KERNEL_GEN9[..],
                    matmul_kernel_has_eot(),
                    "matmul",
                    build_matmul_curbe_payload(
                        GGTT_LHS_BASE,
                        GGTT_RHS_BASE,
                        GGTT_OUTPUT_BASE,
                        m,
                        n,
                        k,
                    ),
                    CURBE_CROSS_THREAD_GRF,
                ),
                ComputeKind::Gelu { n } => (
                    batch.inputs[0].bytes.len(),
                    batch.inputs[0].bytes.len(),
                    &[][..],
                    batch.output.len(),
                    // Dispatch a FULL SIMD32 thread (all 32 lanes live, local_id 0..31,
                    // walker mask 0xFFFFFFFF) and let the GELU kernel's OWN bounds check
                    // (`if (idx >= count) return;` — visible in gelu_f32.gen9.asm as the
                    // `cmp (lt) r8.4` against the `count` payload) mask the partial. Idle
                    // lanes (idx >= count = n) return BEFORE any A64 read/write, so this is
                    // safe — unlike the Add (no bounds check), which must hand-trim its mask.
                    // BUILD-4b-2 Stage-B exposed that the hand-trimmed partial mask only
                    // dispatched lane 0 at 8 work-items (Add=2 / matmul=4 were the only
                    // lane-counts ever proven on this silicon); a full SIMD32 dispatch +
                    // the kernel bounds check is what real NEO does and dispatches all lanes.
                    GELU_DISPATCH_LANES,
                    &GELU_F32_KERNEL_GEN9[..],
                    gelu_kernel_has_eot(),
                    "gelu",
                    build_gelu_curbe_payload(GGTT_LHS_BASE, GGTT_OUTPUT_BASE, n),
                    GELU_CROSS_THREAD_GRF,
                ),
            };

        // OT-63 Slice 5 (ADR-077 amendment 5): the GPU-reachable working set
        // (operands + ring/seqno/state/kernel/CURBE) routes through the
        // kill-switch chokepoint `alloc_gpu` — the graphics arena when Slice-5
        // narrowing is enabled (so the narrowed VT-d domain contains them),
        // buddy `alloc_zeroed` bit-for-bit when disabled (default ≡ Slice-4).
        let lhs = DmaBuffer::alloc_gpu(in0).map_err(|_| AccelError::OutOfMemory)?;
        let rhs = DmaBuffer::alloc_gpu(in1).map_err(|_| AccelError::OutOfMemory)?;
        let output = DmaBuffer::alloc_gpu(out_bytes).map_err(|_| AccelError::OutOfMemory)?;
        let seqno = DmaBuffer::alloc_gpu(4).map_err(|_| AccelError::OutOfMemory)?;
        let ring = DmaBuffer::alloc_gpu(RING_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        let state = DmaBuffer::alloc_gpu(STATE_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        let kernel = DmaBuffer::alloc_gpu(KERNEL_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        let curbe = DmaBuffer::alloc_gpu(CURBE_BYTES).map_err(|_| AccelError::OutOfMemory)?;

        // Phase 7 BUILD-4-1 (ISC-1/3): build the per-tenant PPGTT mapping the data
        // buffers' physical frames to their VAs (numerically the 4a VAs), present + R/W.
        // The buffers will be UNMAPPED in GGTT (map_compute_buffers skips them), so the
        // EU's A64 access resolves only through this PPGTT — the self-proving gate.
        let ppgtt = if use_ppgtt {
            Some(Ppgtt::build(
                lhs.physical_addr().as_u64(),
                rhs.physical_addr().as_u64(),
                output.physical_addr().as_u64(),
            )?)
        } else {
            None
        };

        // SAFETY: all allocations are exclusively owned during construction;
        // each slice bound was validated above or is fixed by its allocation.
        unsafe {
            lhs.as_mut_slice().copy_from_slice(batch.inputs[0].bytes);
            // Pointwise GELU has no second operand: `in1_src` is empty and the rhs
            // buffer stays zeroed (mapped but never read by the kernel). Add/matmul
            // copy their full second operand.
            rhs.as_mut_slice()[..in1_src.len()].copy_from_slice(in1_src);
            // iter8 indirect-data heap (the GPGPU_WALKER Indirect Data target):
            // [cross-thread 96 B][per-thread local_id 192 B], per the NEO Gen9 model
            // (ADR-032 ISC-1). Cross-thread carries the stateless-A64 base pointers +
            // (matmul) scalar dims; per-thread carries SIMD32 local_id.x = [0,1,…]. The
            // dispatcher splits the heap into the thread's r1..r6 / r7..r9 GRFs.
            let local_id = build_local_id_payload_n(work_items);
            let heap = curbe.as_mut_slice();
            heap[..CURBE_PAYLOAD_BYTES].copy_from_slice(&cross);
            heap[CURBE_PAYLOAD_BYTES..CURBE_PAYLOAD_BYTES + local_id.len()]
                .copy_from_slice(&local_id);
            // §3 capture: log the assembled indirect heap (cross-thread `a` pointer at
            // byte 32, local_id.x lane 1 at byte 96+2) so a failing run separates a
            // mis-assembled heap from wrong GRF placement.
            let cross_a = u64::from_le_bytes(heap[32..40].try_into().expect("8 bytes"));
            let local_id_lane1 = u16::from_le_bytes(
                heap[CURBE_PAYLOAD_BYTES + 2..CURBE_PAYLOAD_BYTES + 4]
                    .try_into()
                    .expect("2 bytes"),
            );
            serial_println!(
                "[ kernel ] accel-intel: indirect-heap op={} curbe_ggtt=0x{:x} indirect_len={} work_items={} cross_a=0x{:016x} local_id_lane1={} (cross[0,96) + per_thread[96,288))",
                mode,
                GGTT_CURBE_BASE,
                WALKER_INDIRECT_DATA_LENGTH,
                work_items,
                cross_a,
                local_id_lane1
            );
            crate::log::flush();
            let kernel_bytes = kernel.as_mut_slice();
            kernel_bytes[..blob.len()].copy_from_slice(blob);
            serial_println!(
                "[ kernel ] accel-intel: kernel-eot op={} eot_present={}",
                mode,
                eot_ok
            );
            crate::log::flush();
            if !eot_ok {
                return Err(AccelError::InvalidRequest);
            }
            // IDD declares the 3-GRF cross-thread + 6-GRF per-thread read lengths so the
            // dispatcher lands per-thread local_id at r1..r6 and the cross-thread pointers
            // at r7..r9 (the kernel reads its base pointers from r8.0/.1/.2).
            let descriptor =
                write_interface_descriptor(state.as_mut_slice(), cross_grf, CURBE_PER_THREAD_GRF);
            log_interface_descriptor(descriptor);
            if !descriptor.kernel_ptr_matches {
                return Err(AccelError::InvalidRequest);
            }
            let ring_bytes = ring.as_mut_slice();
            let seq = token.sequence as u32;
            // PPGTT (ISC-2): prepend the PPGTT-root load (PML4 → PDP0, two LRI(1)) to the
            // ring so the command streamer loads the root before the compute batch runs.
            // The compute batch is byte-identical to 4a (its references are absolute, not
            // ring-relative), only shifted past the prefix.
            let prefix_len = match &ppgtt {
                Some(p) => {
                    encode_pdp_load(ring_bytes, p.root_phys).ok_or(AccelError::InvalidRequest)?
                }
                None => 0,
            };
            let batch_dst = &mut ring_bytes[prefix_len..];
            let batch_len = match kind {
                ComputeKind::Add => encode_add_batch(batch_dst, seq),
                ComputeKind::Matmul { .. } => encode_matmul_batch(batch_dst, seq, work_items),
                ComputeKind::Gelu { .. } => encode_gelu_batch(batch_dst, seq, work_items),
            }
            .ok_or(AccelError::InvalidRequest)?;
            let command_len = prefix_len + batch_len;
            // Command-byte oracle: re-encode prefix + batch and compare (the scenario
            // reads this; ISC-5 re-lay for the PDP-load LRI prefix).
            let mut expected = [0u8; PDP_LOAD_BYTES + ADD_BATCH_BYTES];
            let expected_prefix = match &ppgtt {
                Some(p) => {
                    encode_pdp_load(&mut expected, p.root_phys).ok_or(AccelError::InvalidRequest)?
                }
                None => 0,
            };
            let expected_batch = match kind {
                ComputeKind::Add => encode_add_batch(&mut expected[expected_prefix..], seq),
                ComputeKind::Matmul { .. } => {
                    encode_matmul_batch(&mut expected[expected_prefix..], seq, work_items)
                }
                ComputeKind::Gelu { .. } => {
                    encode_gelu_batch(&mut expected[expected_prefix..], seq, work_items)
                }
            }
            .ok_or(AccelError::InvalidRequest)?;
            let expected_len = expected_prefix + expected_batch;
            LAST_COMMAND_ORACLE_MATCH.store(
                command_len == expected_len
                    && batch_len == ADD_BATCH_BYTES
                    && prefix_len == expected_prefix
                    && ring_bytes[..command_len] == expected[..expected_len],
                Ordering::Release,
            );
            write_volatile(seqno.as_mut_ptr() as *mut u32, 0);
            Ok(Self {
                token,
                lhs,
                rhs,
                output,
                seqno,
                ring,
                state,
                kernel,
                curbe,
                blur_weights: None,
                blur_curbe_v: None,
                ppgtt,
                execlist: None,
                output_ptr: batch.output.as_ptr() as usize,
                output_len: batch.output.len(),
                mid_ptr: 0,
                mid_len: 0,
                output_kind: PendingOutputKind::Tensor,
                command_len,
            })
        }
    }

    fn new_blur(
        token: DispatchToken,
        pipeline: &ComputePipeline,
        bindings: &mut ComputeBindings<'_>,
        use_ppgtt: bool,
    ) -> AccelResult<Self> {
        let ComputePipeline::SeparableBlur {
            width,
            height,
            stride,
            radius,
            mode,
        } = *pipeline
        else {
            return Err(AccelError::Unsupported);
        };
        // ADR-032 BUILD-4-1 proved that a legacy-ring PDP load does not engage
        // PPGTT for Gen9 A64 accesses. Slice 4 must run inside an LRC/execlist
        // context or degrade; accepting the legacy mode would fake isolation.
        if !use_ppgtt {
            return Err(AccelError::Unsupported);
        }
        let row_bytes = width.checked_mul(4).ok_or(AccelError::InvalidRequest)? as usize;
        let need = (height as usize)
            .checked_sub(1)
            .and_then(|h| h.checked_mul(stride as usize))
            .and_then(|body| body.checked_add(row_bytes))
            .ok_or(AccelError::InvalidRequest)?;
        if width == 0
            || height == 0
            || radius > 8
            || stride < width.saturating_mul(4)
            // Slice 4 deliberately proves the fixed gate fixture. The inherited
            // Phase-6 VA window gives each operand one page; reject a larger
            // surface instead of leaving later pages silently unmapped. Slice 6
            // owns the scalable compositor allocation layout.
            || need > PAGE_SIZE
            || bindings.weights.len() != radius as usize * 2 + 1
            || bindings.src.len() < need
            || bindings.mid.len() < need
            || bindings.dst.len() < need
            || !blur_kernel_has_eot()
        {
            return Err(AccelError::InvalidRequest);
        }

        let src = DmaBuffer::alloc_gpu(need).map_err(|_| AccelError::OutOfMemory)?;
        let mid = DmaBuffer::alloc_gpu(need).map_err(|_| AccelError::OutOfMemory)?;
        let dst = DmaBuffer::alloc_gpu(need).map_err(|_| AccelError::OutOfMemory)?;
        let weights_bytes = core::mem::size_of_val(bindings.weights);
        #[cfg(feature = "neo-test")]
        let force_out_of_domain = BLUR_FORCE_OUT_OF_DOMAIN_WEIGHTS.load(Ordering::Acquire);
        #[cfg(not(feature = "neo-test"))]
        let force_out_of_domain = false;
        let weights = if force_out_of_domain {
            DmaBuffer::alloc_zeroed(weights_bytes)
        } else {
            DmaBuffer::alloc_gpu(weights_bytes)
        }
        .map_err(|_| AccelError::OutOfMemory)?;
        let seqno = DmaBuffer::alloc_gpu(4).map_err(|_| AccelError::OutOfMemory)?;
        let ring = DmaBuffer::alloc_gpu(RING_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        let state = DmaBuffer::alloc_gpu(STATE_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        // BLUR_KERNEL_GEN9 is 4,736 bytes: unlike the older point kernels it
        // crosses a page boundary. Round the allocation up so its EOT-bearing
        // tail has physical backing and can be GGTT-mapped below.
        let blur_kernel_storage = BLUR_KERNEL_GEN9.len().div_ceil(PAGE_SIZE) * PAGE_SIZE;
        let kernel =
            DmaBuffer::alloc_gpu(blur_kernel_storage).map_err(|_| AccelError::OutOfMemory)?;
        let curbe_h = DmaBuffer::alloc_gpu(CURBE_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        let curbe_v = DmaBuffer::alloc_gpu(CURBE_BYTES).map_err(|_| AccelError::OutOfMemory)?;

        // SAFETY: all allocations are exclusively owned during blur-batch
        // construction; every copied range was validated above or has a fixed
        // size no larger than its allocation.
        unsafe {
            src.as_mut_slice()[..need].copy_from_slice(&bindings.src[..need]);
            for (index, value) in bindings.weights.iter().copied().enumerate() {
                weights.as_mut_slice()[index * 4..index * 4 + 4]
                    .copy_from_slice(&value.to_le_bytes());
            }
            kernel.as_mut_slice()[..BLUR_KERNEL_GEN9.len()].copy_from_slice(&BLUR_KERNEL_GEN9);
            let local_ids = build_blur_local_id_payload();
            for (heap, cross) in [
                (
                    curbe_h.as_mut_slice(),
                    build_blur_curbe_payload(
                        GGTT_LHS_BASE,
                        GGTT_RHS_BASE,
                        GGTT_BLUR_WEIGHTS_BASE,
                        width,
                        height,
                        stride,
                        radius,
                        0,
                    ),
                ),
                (
                    curbe_v.as_mut_slice(),
                    build_blur_curbe_payload(
                        GGTT_RHS_BASE,
                        GGTT_OUTPUT_BASE,
                        GGTT_BLUR_WEIGHTS_BASE,
                        width,
                        height,
                        stride,
                        radius,
                        1,
                    ),
                ),
            ] {
                heap[..CURBE_PAYLOAD_BYTES].copy_from_slice(&cross);
                heap[CURBE_PAYLOAD_BYTES..CURBE_PAYLOAD_BYTES + local_ids.len()]
                    .copy_from_slice(&local_ids);
            }
            let descriptor = write_blur_interface_descriptor(state.as_mut_slice());
            if !descriptor.kernel_ptr_matches {
                return Err(AccelError::InvalidRequest);
            }
            let layout = BlurBatchLayout {
                state_base: GGTT_STATE_BASE,
                kernel_base: GGTT_KERNEL_BASE,
                seqno_base: GGTT_SEQNO_BASE,
                curbe_h_base: GGTT_CURBE_BASE,
                curbe_v_base: GGTT_BLUR_CURBE_V_BASE,
            };
            let batch_dst = ring.as_mut_slice();
            let batch_len = match mode {
                BlurDispatchMode::HorizontalOnly => {
                    encode_blur_h_batch(batch_dst, token.sequence as u32, width, height, layout)
                }
                BlurDispatchMode::TwoPass => {
                    encode_blur_batch(batch_dst, token.sequence as u32, width, height, layout)
                }
            }
            .ok_or(AccelError::InvalidRequest)?;

            let mut expected = [0u8; BLUR_BATCH_BYTES];
            let expected_batch = match mode {
                BlurDispatchMode::HorizontalOnly => {
                    encode_blur_h_batch(&mut expected, token.sequence as u32, width, height, layout)
                }
                BlurDispatchMode::TwoPass => {
                    encode_blur_batch(&mut expected, token.sequence as u32, width, height, layout)
                }
            }
            .ok_or(AccelError::InvalidRequest)?;
            let command_len = batch_len;
            LAST_COMMAND_ORACLE_MATCH.store(
                batch_len == expected_batch
                    && ring.as_slice()[..command_len] == expected[..expected_batch],
                Ordering::Release,
            );
            let execlist = ComputeExeclistContext::new(
                token,
                ring.as_ref(),
                state.as_ref(),
                kernel.as_ref(),
                src.as_ref(),
                mid.as_ref(),
                dst.as_ref(),
                seqno.as_ref(),
                curbe_h.as_ref(),
                weights.as_ref(),
                curbe_v.as_ref(),
                command_len,
            )?;
            write_volatile(seqno.as_mut_ptr() as *mut u32, 0);
            BLUR_LAST_MID_CAPTURED.store(false, Ordering::Release);
            BLUR_LAST_DST_CAPTURED.store(false, Ordering::Release);
            serial_println!(
                "[ kernel ] accel-intel: blur-bound token={} mode={:?} bytes={} groups={:?} threads_per_group=8 ppgtt={} command_bytes={} oracle={}",
                token.sequence,
                mode,
                need,
                blur_group_dims(width, height),
                true,
                command_len,
                LAST_COMMAND_ORACLE_MATCH.load(Ordering::Acquire)
            );
            crate::log::flush();
            Ok(Self {
                token,
                lhs: src,
                rhs: mid,
                output: dst,
                seqno,
                ring,
                state,
                kernel,
                curbe: curbe_h,
                blur_weights: Some(weights),
                blur_curbe_v: Some(curbe_v),
                ppgtt: None,
                execlist: Some(execlist),
                output_ptr: bindings.dst.as_mut_ptr() as usize,
                output_len: need,
                mid_ptr: bindings.mid.as_mut_ptr() as usize,
                mid_len: need,
                output_kind: PendingOutputKind::Blur(mode),
                command_len,
            })
        }
    }
}

struct PendingSmoke {
    token: DispatchToken,
    seqno: Arc<DmaBuffer>,
    ring: Arc<DmaBuffer>,
    command_len: usize,
}

impl PendingSmoke {
    fn new(token: DispatchToken) -> AccelResult<Self> {
        let seqno = DmaBuffer::alloc_zeroed(4).map_err(|_| AccelError::OutOfMemory)?;
        let ring = DmaBuffer::alloc_zeroed(RING_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        let command_len = unsafe {
            let ring_bytes = ring.as_mut_slice();
            let command_len = encode_smoke_batch(ring_bytes, token.sequence as u32)
                .ok_or(AccelError::InvalidRequest)?;
            let mut expected = [0u8; SMOKE_BATCH_BYTES];
            let expected_len = encode_smoke_batch(&mut expected, token.sequence as u32)
                .ok_or(AccelError::InvalidRequest)?;
            if command_len != expected_len
                || command_len != SMOKE_BATCH_BYTES
                || ring_bytes[..command_len] != expected[..expected_len]
            {
                return Err(AccelError::InvalidRequest);
            }
            write_volatile(seqno.as_mut_ptr() as *mut u32, 0);
            command_len
        };
        Ok(Self {
            token,
            seqno,
            ring,
            command_len,
        })
    }
}

/// Iter5/6 no-op dispatch — the silicon-proven (A)/(B) bisector. Its command
/// stream and interface descriptor are kept byte-for-byte as iter6 (no CURBE,
/// all-lanes-live walker masks, cross-thread read length 0), so a retiring no-op
/// always isolates the dispatch path. iter7's CURBE argument payload + trimmed
/// SIMD32 masks belong to the Add path only (`PendingAdd`); the no-op shares the
/// same seven GGTT pages but no `curbe` buffer (its batch has no MEDIA_CURBE_LOAD).
struct PendingNoop {
    token: DispatchToken,
    lhs: Arc<DmaBuffer>,
    rhs: Arc<DmaBuffer>,
    output: Arc<DmaBuffer>,
    seqno: Arc<DmaBuffer>,
    ring: Arc<DmaBuffer>,
    state: Arc<DmaBuffer>,
    kernel: Arc<DmaBuffer>,
    command_len: usize,
}

impl PendingNoop {
    fn new(token: DispatchToken) -> AccelResult<Self> {
        let lhs = DmaBuffer::alloc_zeroed(ADD_VECTOR_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        let rhs = DmaBuffer::alloc_zeroed(ADD_VECTOR_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        let output =
            DmaBuffer::alloc_zeroed(ADD_VECTOR_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        let seqno = DmaBuffer::alloc_zeroed(4).map_err(|_| AccelError::OutOfMemory)?;
        let ring = DmaBuffer::alloc_zeroed(RING_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        let state = DmaBuffer::alloc_zeroed(STATE_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        let kernel = DmaBuffer::alloc_zeroed(KERNEL_BYTES).map_err(|_| AccelError::OutOfMemory)?;

        let command_len = unsafe {
            let kernel_bytes = kernel.as_mut_slice();
            kernel_bytes[..NOOP_KERNEL_GEN9.len()].copy_from_slice(&NOOP_KERNEL_GEN9);
            serial_println!(
                "[ kernel ] accel-intel: noop-kernel-eot source='{}' eot_present={}",
                NOOP_KERNEL_EOT_SOURCE,
                noop_kernel_has_eot()
            );
            crate::log::flush();
            if !noop_kernel_has_eot() {
                return Err(AccelError::InvalidRequest);
            }
            // No-op IDD declares no cross-thread / per-thread data (read lengths 0):
            // the bisector dispatch stays byte-identical to the silicon-proven iter6 path.
            let descriptor = write_interface_descriptor(state.as_mut_slice(), 0, 0);
            log_interface_descriptor(descriptor);
            if !descriptor.kernel_ptr_matches {
                return Err(AccelError::InvalidRequest);
            }
            let ring_bytes = ring.as_mut_slice();
            let command_len = encode_noop_batch(ring_bytes, token.sequence as u32)
                .ok_or(AccelError::InvalidRequest)?;
            write_volatile(seqno.as_mut_ptr() as *mut u32, 0);
            command_len
        };
        Ok(Self {
            token,
            lhs,
            rhs,
            output,
            seqno,
            ring,
            state,
            kernel,
            command_len,
        })
    }
}

/// What an execlist context runs: the 4X-1 no-op (proves submission) or the 4X-2 Add
/// (proves tensor *data* resolves through the per-context PPGTT). The submission, LRC,
/// descriptor, and CSB path are identical; the kernel/IDD/CURBE/batch + the data-buffer
/// GGTT-clearing differ.
#[cfg(feature = "neo-test")]
#[derive(Copy, Clone, Eq, PartialEq)]
enum ExeclistKind {
    /// BUILD-4X-1: EOT-only no-op, no data; all pages GGTT-mapped + PPGTT-mapped.
    NoOp,
    /// BUILD-4X-2: the proven 4a Add, data buffers PPGTT-only (cleared from GGTT). All
    /// data leaves present + R/W → the Add succeeds `[4,6]` (the positive control).
    Add,
    /// BUILD-4X-3 cross-tenant negative: the Add's lhs leaf is **omitted** from this PML4
    /// (lhs VA = "tenant A's tensor", absent from tenant B's page tree) — the EU's A64
    /// read of lhs must page-fault. The gate is that fault (isolation: a VA in A's PML4 is
    /// unreachable from B's). Paired with `Add` as the positive control (same VA, mapped).
    AddCrossTenant,
    /// BUILD-4X-3 RO-READ positive: lhs mapped **read-only**, output read-write. The EU's
    /// read of an RO-mapped tensor must **succeed** — the Add completes `[4,6]`. Proves a
    /// read through an RO PTE is allowed (the corollary of RO write-blocking).
    AddRoRead,
    /// BUILD-4X-3 RW-on-RO negative: output mapped **read-only** (lhs/rhs read-write). The
    /// EU's *write* to output must be **blocked**. iter1 silicon proved Gen9 does NOT raise
    /// `RING_FAULT` for a write-permission fault (unlike a not-present fault) — the page walk
    /// succeeds and the dataport permission check wedges the EU. So the block is proven by
    /// **non-completion + the output frame staying unmodified** (the write never landed).
    /// The device-side enforcement of BUILD-3's §9.7 RO sharing.
    AddRoWrite,
}

#[cfg(feature = "neo-test")]
impl ExeclistKind {
    /// True for every kind that runs the 4a Add (kernel/IDD/CURBE/batch + input data) — all
    /// non-`NoOp` kinds. They differ only in the per-kind PML4 data-buffer mapping.
    fn is_add(self) -> bool {
        !matches!(self, ExeclistKind::NoOp)
    }
    /// True for kinds expected to **complete** `[4,6]` (the positive paths). The fault/block
    /// kinds (`AddCrossTenant`, `AddRoWrite`) are expected NOT to complete.
    fn expects_output(self) -> bool {
        matches!(self, ExeclistKind::Add | ExeclistKind::AddRoRead)
    }
    fn label(self) -> &'static str {
        match self {
            ExeclistKind::NoOp => "noop",
            ExeclistKind::Add => "add",
            ExeclistKind::AddCrossTenant => "add-cross-tenant",
            ExeclistKind::AddRoRead => "add-ro-read",
            ExeclistKind::AddRoWrite => "add-ro-write",
        }
    }
}

/// Phase 7 BUILD-4X-1/4X-2: a context submitted via the execlist/LRC path. 4X-1 runs the
/// no-op (EOT-only kernel) to prove submission; 4X-2 runs the Add with its **data buffers
/// mapped PPGTT-only** to prove the tensor data resolves through the per-context PPGTT.
/// iter1 silicon proved the execlist context resolves STATE_BASE/IDD/kernel/data through
/// its **context PPGTT**, so the LRC's PML4 maps every page the dispatch dereferences.
/// `pml4`/`pdp`/`pd`/`pt` are the 4-level page tables; `curbe` is the Add's indirect-data
/// heap (None for the no-op); `descriptor` is submitted via ELSP; `ring_tail` is the batch
/// length plus the WaIdleLiteRestore pad.
#[cfg(feature = "neo-test")]
struct PendingExeclist {
    kind: ExeclistKind,
    token: DispatchToken,
    lrc: Arc<DmaBuffer>,
    hwsp: Arc<DmaBuffer>,
    lhs: Arc<DmaBuffer>,
    rhs: Arc<DmaBuffer>,
    output: Arc<DmaBuffer>,
    seqno: Arc<DmaBuffer>,
    ring: Arc<DmaBuffer>,
    state: Arc<DmaBuffer>,
    kernel: Arc<DmaBuffer>,
    /// The Add's CURBE / indirect-data heap (the A64 base pointers + SIMD local_id). None
    /// for the no-op. Held alive + GGTT/PPGTT-mapped so the GPGPU_WALKER can read it.
    curbe: Option<Arc<DmaBuffer>>,
    // The four 48-bit page-table frames — held alive (the engine walks them physically
    // from the PML4 root in CTX_PDP0); only `pml4_root_phys` is read after construction.
    _pml4: Arc<DmaBuffer>,
    _pdp: Arc<DmaBuffer>,
    _pd: Arc<DmaBuffer>,
    _pt: Arc<DmaBuffer>,
    pml4_root_phys: u64,
    descriptor: u64,
    ring_tail: usize,
}

#[cfg(feature = "neo-test")]
impl PendingExeclist {
    fn new(token: DispatchToken, kind: ExeclistKind) -> AccelResult<Self> {
        let lrc =
            DmaBuffer::alloc_zeroed(LRC_PAGES * PAGE_SIZE).map_err(|_| AccelError::OutOfMemory)?;
        let hwsp = DmaBuffer::alloc_zeroed(PAGE_SIZE).map_err(|_| AccelError::OutOfMemory)?;
        let lhs = DmaBuffer::alloc_zeroed(ADD_VECTOR_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        let rhs = DmaBuffer::alloc_zeroed(ADD_VECTOR_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        let output =
            DmaBuffer::alloc_zeroed(ADD_VECTOR_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        let seqno = DmaBuffer::alloc_zeroed(4).map_err(|_| AccelError::OutOfMemory)?;
        let ring = DmaBuffer::alloc_zeroed(RING_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        let state = DmaBuffer::alloc_zeroed(STATE_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        let kernel = DmaBuffer::alloc_zeroed(KERNEL_BYTES).map_err(|_| AccelError::OutOfMemory)?;
        // The Add family (4X-2/4X-3) carries a CURBE / indirect-data heap (the A64 base
        // pointers + SIMD local_id). The no-op (4X-1) has none.
        let curbe = if kind.is_add() {
            Some(DmaBuffer::alloc_zeroed(CURBE_BYTES).map_err(|_| AccelError::OutOfMemory)?)
        } else {
            None
        };

        // ISC-1: build a real **48-bit PML4** (PML4→PDP→PD→PT, four frames) mapping every
        // page the dispatch dereferences via the EU/dataport, reusing 4-1's `leaf_pte`/
        // `dir_entry`. 4X-1 iter1 silicon proved the execlist context resolves STATE_BASE/
        // IDD/kernel/data through its PPGTT (it faulted in PPGTT space at the STATE VA until
        // mapped). Gen9 is 48-bit: this PML4's physical address goes into CTX_PDP0. All the
        // VAs (0x10_0000…0x13_4000) share PML4[0]→PDP[0]→PD[0]→one PT, so four frames suffice.
        // 4X-2 ADDS the CURBE page (the GPGPU_WALKER indirect-data heap the Add reads).
        let pml4 = DmaBuffer::alloc_zeroed(PAGE_SIZE).map_err(|_| AccelError::OutOfMemory)?;
        let pdp = DmaBuffer::alloc_zeroed(PAGE_SIZE).map_err(|_| AccelError::OutOfMemory)?;
        let pd = DmaBuffer::alloc_zeroed(PAGE_SIZE).map_err(|_| AccelError::OutOfMemory)?;
        let pt = DmaBuffer::alloc_zeroed(PAGE_SIZE).map_err(|_| AccelError::OutOfMemory)?;
        let pml4_root_phys = pml4.physical_addr().as_u64();
        write_table_entry(
            &pml4,
            execlist::pml4_index(GGTT_STATE_BASE),
            dir_entry(pdp.physical_addr().as_u64()),
        );
        write_table_entry(
            &pdp,
            execlist::pdp_index_48(GGTT_STATE_BASE),
            dir_entry(pd.physical_addr().as_u64()),
        );
        write_table_entry(
            &pd,
            execlist::pd_index_48(GGTT_STATE_BASE),
            dir_entry(pt.physical_addr().as_u64()),
        );
        // Leaves: every pipeline + data page the engine resolves via the context PPGTT.
        // Pipeline pages are always present + R/W. The **data buffers (lhs/output) vary by
        // kind** — this is the 4X-3 negative test, expressed in the page tables:
        //   `Some(true)`  → present + R/W   `Some(false)` → present + READ-ONLY (RW-bit clear)
        //   `None`        → OMITTED (no leaf — the VA is unreachable in this context)
        //   - Add: lhs RW, output RW (the 4X-2 positive control — succeeds).
        //   - AddCrossTenant: lhs OMITTED (tenant A's VA, absent from B's PML4) → read faults.
        //   - AddRoRead: lhs RO, output RW → the EU's RO read succeeds (the Add completes).
        //   - AddRoWrite: lhs RW, output RO → the EU's write to the RO output is BLOCKED.
        // The data buffers are mapped here AND cleared from GGTT in `map_execlist_buffers`,
        // so the only resolution path is this per-context PPGTT (the self-proving gate).
        let lhs_map = match kind {
            ExeclistKind::AddCrossTenant => None,
            ExeclistKind::AddRoRead => Some(false),
            _ => Some(true),
        };
        let output_map = match kind {
            ExeclistKind::AddRoWrite => Some(false),
            _ => Some(true),
        };
        let curbe_phys = curbe.as_ref().map(|c| c.physical_addr().as_u64());
        // (va, phys, mapping). `mapping` None = omit the leaf entirely.
        let pml4_leaves: [(u64, u64, Option<bool>); 8] = [
            (GGTT_RING_BASE, ring.physical_addr().as_u64(), Some(true)),
            (GGTT_STATE_BASE, state.physical_addr().as_u64(), Some(true)),
            (
                GGTT_KERNEL_BASE,
                kernel.physical_addr().as_u64(),
                Some(true),
            ),
            (GGTT_LHS_BASE, lhs.physical_addr().as_u64(), lhs_map),
            (GGTT_RHS_BASE, rhs.physical_addr().as_u64(), Some(true)),
            (
                GGTT_OUTPUT_BASE,
                output.physical_addr().as_u64(),
                output_map,
            ),
            (GGTT_SEQNO_BASE, seqno.physical_addr().as_u64(), Some(true)),
            (
                GGTT_CURBE_BASE,
                curbe_phys.unwrap_or(0),
                curbe_phys.map(|_| true),
            ),
        ];
        let mut leaf_count = 0u32;
        for &(va, phys, mapping) in &pml4_leaves {
            if let Some(writable) = mapping {
                write_table_entry(&pt, execlist::pt_index_48(va), leaf_pte(phys, writable));
                leaf_count += 1;
            }
        }
        serial_println!(
            "[ kernel ] accel-intel: execlist-pml4 built kind={} root(pml4)=0x{:016x} pt=0x{:016x} leaf_pages={} lhs={} output={} lhs_va=0x{:x} output_va=0x{:x} pt_idx_state=0x{:x}",
            kind.label(),
            pml4_root_phys,
            pt.physical_addr().as_u64(),
            leaf_count,
            match lhs_map {
                None => "omitted",
                Some(true) => "rw",
                Some(false) => "ro",
            },
            match output_map {
                None => "omitted",
                Some(true) => "rw",
                Some(false) => "ro",
            },
            GGTT_LHS_BASE,
            GGTT_OUTPUT_BASE,
            execlist::pt_index_48(GGTT_STATE_BASE)
        );
        crate::log::flush();

        // Same RING_CTL encoding as the legacy ring: 1-page ring, valid.
        let ring_ctl =
            ((RING_BYTES as u32 - PAGE_SIZE as u32) & RING_CTL_SIZE_MASK) | RING_CTL_VALID;

        let (ring_tail, descriptor) = unsafe {
            // ── kernel blob + interface descriptor (kind-specific) ──
            let kernel_bytes = kernel.as_mut_slice();
            let (cross_grf, per_grf) = if kind.is_add() {
                kernel_bytes[..ADD_F32X2_KERNEL_GEN9.len()].copy_from_slice(&ADD_F32X2_KERNEL_GEN9);
                serial_println!(
                    "[ kernel ] accel-intel: execlist-kernel-eot kind={} eot_present={}",
                    kind.label(),
                    add_kernel_has_pinned_eot_tail()
                );
                crate::log::flush();
                if !add_kernel_has_pinned_eot_tail() {
                    return Err(AccelError::InvalidRequest);
                }
                (CURBE_CROSS_THREAD_GRF, CURBE_PER_THREAD_GRF)
            } else {
                kernel_bytes[..NOOP_KERNEL_GEN9.len()].copy_from_slice(&NOOP_KERNEL_GEN9);
                serial_println!(
                    "[ kernel ] accel-intel: execlist-kernel-eot kind=noop source='{}' eot_present={}",
                    NOOP_KERNEL_EOT_SOURCE,
                    noop_kernel_has_eot()
                );
                crate::log::flush();
                if !noop_kernel_has_eot() {
                    return Err(AccelError::InvalidRequest);
                }
                (0, 0)
            };
            let descriptor_dbg =
                write_interface_descriptor(state.as_mut_slice(), cross_grf, per_grf);
            log_interface_descriptor(descriptor_dbg);
            if !descriptor_dbg.kernel_ptr_matches {
                return Err(AccelError::InvalidRequest);
            }

            // ── the Add's data + CURBE heap (4X-2) ──
            if let Some(curbe_buf) = &curbe {
                // Canonical 4a Add inputs: lhs = [1.0, 2.0], rhs = [3.0, 4.0] → [4.0, 6.0]
                // (= `add_oracle_output()`). The bring-up owns its inputs (self-contained).
                const LHS_INPUT: [u8; ADD_VECTOR_BYTES] =
                    [0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x40];
                const RHS_INPUT: [u8; ADD_VECTOR_BYTES] =
                    [0x00, 0x00, 0x40, 0x40, 0x00, 0x00, 0x80, 0x40];
                lhs.as_mut_slice().copy_from_slice(&LHS_INPUT);
                rhs.as_mut_slice().copy_from_slice(&RHS_INPUT);
                // CURBE / indirect-data heap: [cross-thread A64 pointers][per-thread local_id].
                // The A64 pointers are the data VAs (0x13_0000…), which the EU resolves
                // through the context PPGTT (mapped above). Same layout as the 4a/4b path.
                let cross = build_curbe_payload(GGTT_LHS_BASE, GGTT_RHS_BASE, GGTT_OUTPUT_BASE);
                let local_id = build_local_id_payload_n(ADD_VECTOR_ELEMENTS as u32);
                let heap = curbe_buf.as_mut_slice();
                heap[..CURBE_PAYLOAD_BYTES].copy_from_slice(&cross);
                heap[CURBE_PAYLOAD_BYTES..CURBE_PAYLOAD_BYTES + local_id.len()]
                    .copy_from_slice(&local_id);
            }

            // ── the ring batch (kind-specific; both 69 DWords) ──
            let ring_bytes = ring.as_mut_slice();
            let seq = token.sequence as u32;
            let command_len = if kind.is_add() {
                encode_add_batch(ring_bytes, seq)
            } else {
                encode_noop_batch(ring_bytes, seq)
            }
            .ok_or(AccelError::InvalidRequest)?;
            // WaIdleLiteRestore (RESEARCH §8.1): the 2 trailing dwords are MI_NOOP (the ring
            // is zero-filled) and RING_TAIL advances past them so HEAD != TAIL.
            let ring_tail = command_len + execlist::WA_TAIL_BYTES;
            if ring_tail > RING_BYTES {
                return Err(AccelError::InvalidRequest);
            }

            // ISC-1: build the LRC register-state image at page 1 (page 0 = PPHWSP). The
            // engine loads ring HEAD/TAIL/START/CTL + the PML4 root from here on switch-in.
            let params = execlist::LrcParams {
                ring_start: GGTT_RING_BASE as u32,
                ring_ctl,
                ring_head: 0,
                ring_tail: ring_tail as u32,
                pdp: [pml4_root_phys, 0, 0, 0],
            };
            {
                let lrc_bytes = lrc.as_mut_slice();
                let reg_state =
                    &mut lrc_bytes[PAGE_SIZE..PAGE_SIZE + execlist::LRC_REG_STATE_BYTES];
                execlist::write_lrc_reg_state(reg_state, params)
                    .ok_or(AccelError::InvalidRequest)?;
            }

            // ISC-2: the 64-bit execlist context descriptor (LRCA | flags | ctx-id).
            let descriptor = execlist::context_descriptor(GGTT_LRC_BASE, EXECLIST_CTX_ID);

            // ISC-5/6 in-kernel oracle re-lay: independently rebuild the register-state image,
            // the descriptor, and the batch and compare to what the engine consumes.
            let mut expected_reg = [0u8; execlist::LRC_REG_STATE_BYTES];
            execlist::write_lrc_reg_state(&mut expected_reg, params)
                .ok_or(AccelError::InvalidRequest)?;
            let mut expected_batch = [0u8; super::intel_gen9_4a::ADD_BATCH_BYTES];
            let expected_batch_len = if kind.is_add() {
                encode_add_batch(&mut expected_batch, seq)
            } else {
                encode_noop_batch(&mut expected_batch, seq)
            }
            .ok_or(AccelError::InvalidRequest)?;
            let lrc_view = lrc.as_slice();
            let reg_ok =
                lrc_view[PAGE_SIZE..PAGE_SIZE + execlist::LRC_REG_STATE_BYTES] == expected_reg;
            let batch_ok = command_len == expected_batch_len
                && ring_bytes[..command_len] == expected_batch[..expected_batch_len];
            let desc_ok =
                descriptor == execlist::context_descriptor(GGTT_LRC_BASE, EXECLIST_CTX_ID);
            LAST_EXECLIST_ORACLE_MATCH.store(reg_ok && batch_ok && desc_ok, Ordering::Release);

            write_volatile(seqno.as_mut_ptr() as *mut u32, 0);
            (ring_tail, descriptor)
        };

        Ok(Self {
            kind,
            token,
            lrc,
            hwsp,
            lhs,
            rhs,
            output,
            seqno,
            ring,
            state,
            kernel,
            curbe,
            _pml4: pml4,
            _pdp: pdp,
            _pd: pd,
            _pt: pt,
            pml4_root_phys,
            descriptor,
            ring_tail,
        })
    }
}

#[derive(Copy, Clone)]
struct CompletionRecord {
    token: DispatchToken,
    status: DispatchStatus,
}

fn validate_add_batch(batch: &OpBatch<'_, '_>) -> AccelResult<()> {
    if batch.op != OpKind::Add || batch.inputs.len() != 2 {
        return Err(AccelError::Unsupported);
    }
    if batch.output.len() != ADD_VECTOR_BYTES
        || batch.output_shape.rank() != 1
        || batch.output_shape.dim(0) != 2
    {
        return Err(AccelError::InvalidRequest);
    }
    for input in batch.inputs {
        if input.dtype != Dtype::F32
            || input.shape.rank() != 1
            || input.shape.dim(0) != 2
            || input.bytes.len() != ADD_VECTOR_BYTES
        {
            return Err(AccelError::InvalidRequest);
        }
    }
    Ok(())
}

/// This GPU slice handles matmul as one SIMD32 thread over m·n output elements with
/// one GGTT page per operand. Larger shapes return `Unsupported` → accel_cpu.
const MATMUL_MAX_WORK_ITEMS: u64 = 32;

/// Validate a matmul `OpBatch` for the GPU path and extract its dims. Returns
/// `Unsupported` (→ accel_cpu fallback, never a batch failure) for any shape the
/// slice does not cover (m·n > 32, or an operand larger than one GGTT page). The
/// executor has already shape-checked the matmul; this guards the GPU subset.
fn validate_matmul_batch(batch: &OpBatch<'_, '_>) -> AccelResult<ComputeKind> {
    if batch.op != OpKind::Matmul || batch.inputs.len() != 2 {
        return Err(AccelError::Unsupported);
    }
    let a = &batch.inputs[0];
    let b = &batch.inputs[1];
    if a.dtype != Dtype::F32
        || b.dtype != Dtype::F32
        || a.shape.rank() != 2
        || b.shape.rank() != 2
        || batch.output_shape.rank() != 2
    {
        return Err(AccelError::Unsupported);
    }
    let m = batch.output_shape.dim(0);
    let n = batch.output_shape.dim(1);
    let k = a.shape.dim(1);
    if a.shape.dim(0) != m || b.shape.dim(0) != k || b.shape.dim(1) != n {
        return Err(AccelError::Unsupported);
    }
    let page = PAGE_SIZE as u64;
    let mn = m.checked_mul(n);
    let a_bytes = m.checked_mul(k).and_then(|x| x.checked_mul(4));
    let b_bytes = k.checked_mul(n).and_then(|x| x.checked_mul(4));
    let c_bytes = mn.and_then(|x| x.checked_mul(4));
    let (Some(mn), Some(ab), Some(bb), Some(cb)) = (mn, a_bytes, b_bytes, c_bytes) else {
        return Err(AccelError::Unsupported);
    };
    if mn == 0 || mn > MATMUL_MAX_WORK_ITEMS || ab > page || bb > page || cb > page {
        return Err(AccelError::Unsupported); // -> accel_cpu fallback
    }
    // The executor-allocated operand/output byte lengths must match the dims.
    if a.bytes.len() as u64 != ab || b.bytes.len() as u64 != bb || batch.output.len() as u64 != cb {
        return Err(AccelError::InvalidRequest);
    }
    Ok(ComputeKind::Matmul {
        m: m as u32,
        n: n as u32,
        k: k as u32,
    })
}

/// This GPU slice handles GELU as one SIMD32 thread over N elements with one GGTT
/// page for input and one for output. Larger N returns `Unsupported` → accel_cpu.
const GELU_MAX_WORK_ITEMS: u64 = 32;

/// GELU dispatches a full SIMD32 thread (all 32 lanes), relying on the kernel's own
/// `idx >= count` bounds check to mask the partial — not a hand-trimmed walker
/// execution mask (which BUILD-4b-2 Stage-B proved only dispatches lane 0 beyond the
/// Add/matmul-proven ≤4 lanes). `build_local_id_payload_n(32)` fills local_id 0..31 and
/// `lane_mask(32) = 0xFFFFFFFF` makes all 32 lanes live; the count in the cross-thread
/// payload (= N ≤ 32) is what actually bounds the work.
const GELU_DISPATCH_LANES: u32 = 32;

/// Validate a GELU `OpBatch` for the GPU path and extract its element count. Returns
/// `Unsupported` (→ accel_cpu fallback, never a batch failure) for any shape the
/// slice does not cover (N > 32, or input/output larger than one GGTT page). The
/// executor has already shape-checked GELU (output shape == input shape, f32);
/// this guards the GPU subset, mirroring `validate_matmul_batch`.
fn validate_gelu_batch(batch: &OpBatch<'_, '_>) -> AccelResult<ComputeKind> {
    if batch.op != OpKind::Gelu || batch.inputs.len() != 1 {
        return Err(AccelError::Unsupported);
    }
    let input = &batch.inputs[0];
    if input.dtype != Dtype::F32 {
        return Err(AccelError::Unsupported);
    }
    // Element count = product of the input shape dims (pointwise, any rank).
    let mut n: u64 = 1;
    for axis in 0..input.shape.rank() as usize {
        let Some(next) = n.checked_mul(input.shape.dim(axis)) else {
            return Err(AccelError::Unsupported);
        };
        n = next;
    }
    let page = PAGE_SIZE as u64;
    let Some(bytes) = n.checked_mul(4) else {
        return Err(AccelError::Unsupported);
    };
    if n == 0 || n > GELU_MAX_WORK_ITEMS || bytes > page {
        return Err(AccelError::Unsupported); // -> accel_cpu fallback
    }
    // Output shape must match input shape (the executor enforces this) and the
    // executor-allocated input/output byte lengths must match the element count.
    if batch.output_shape.rank() != input.shape.rank() {
        return Err(AccelError::Unsupported);
    }
    for axis in 0..input.shape.rank() as usize {
        if batch.output_shape.dim(axis) != input.shape.dim(axis) {
            return Err(AccelError::Unsupported);
        }
    }
    if input.bytes.len() as u64 != bytes || batch.output.len() as u64 != bytes {
        return Err(AccelError::InvalidRequest);
    }
    Ok(ComputeKind::Gelu { n: n as u32 })
}

fn map_compute_buffers(mmio: GtMmio, pending: &PendingCompute) -> AccelResult<()> {
    let context_ppgtt = pending.ppgtt.is_some() || pending.execlist.is_some();
    // BUILD-4a maps kernel-owned, physically contiguous DMA buffers into the
    // Gen9 GGTT by writing BAR0/GTTMMADR PTEs. Intel KBL PRM Volume 5 documents
    // GGTT PTE programming through GTTMMADR; C2 fixes the coherent path to
    // LLC-backed system RAM plus PIPE_CONTROL flush/seqno ordering.
    write_named_ggtt_pte(
        mmio,
        "ring",
        GGTT_RING_BASE,
        pending.ring.physical_addr().as_u64(),
    )?;
    write_named_ggtt_pte(
        mmio,
        "state",
        GGTT_STATE_BASE,
        pending.state.physical_addr().as_u64(),
    )?;
    // The blur binary crosses a page boundary; map the full backing allocation
    // so instruction fetch can reach its EOT-bearing tail. Older tensor kernels
    // remain a one-page range and are unchanged by this helper.
    write_named_ggtt_range(
        mmio,
        "kernel",
        GGTT_KERNEL_BASE,
        pending.kernel.physical_addr().as_u64(),
        pending.kernel.len(),
    )?;
    // Phase 7 BUILD-4-1 (ISC-3): with PPGTT engaged, the data buffers (lhs/rhs/output)
    // are mapped ONLY in the per-tenant PPGTT — deliberately UNMAPPED in GGTT. That is
    // the self-proving gate: the EU's A64 access to their VAs resolves through PPGTT or
    // (if PPGTT didn't engage) falls to GGTT, finds nothing, and faults. The 4a GGTT
    // path still maps them here.
    if !context_ppgtt {
        write_named_ggtt_pte(
            mmio,
            "lhs",
            GGTT_LHS_BASE,
            pending.lhs.physical_addr().as_u64(),
        )?;
        write_named_ggtt_pte(
            mmio,
            "rhs",
            GGTT_RHS_BASE,
            pending.rhs.physical_addr().as_u64(),
        )?;
        write_named_ggtt_pte(
            mmio,
            "output",
            GGTT_OUTPUT_BASE,
            pending.output.physical_addr().as_u64(),
        )?;
    } else {
        // iter2: actively CLEAR (present=0) the data VAs in GGTT, not just skip the
        // write — the GPU GGTT persists across reboots, so a stale valid entry from a
        // prior GGTT run silently catches an A64 access the PPGTT path means to resolve
        // via PPGTT, masking the self-proving fault (iter1 read zeros from stale frames,
        // never faulting). After clearing, an access that did NOT go through PPGTT
        // faults (or hits GGTT scratch), surfacing the GTT/PPGTT `select` bit.
        clear_named_ggtt_pte(mmio, "lhs", GGTT_LHS_BASE)?;
        clear_named_ggtt_pte(mmio, "rhs", GGTT_RHS_BASE)?;
        clear_named_ggtt_pte(mmio, "output", GGTT_OUTPUT_BASE)?;
        serial_println!(
            "[ kernel ] accel-intel: ppgtt data buffers CLEARED from GGTT (lhs/rhs/output VAs 0x{:x}/0x{:x}/0x{:x} resolve via PPGTT only; stale GGTT invalidated)",
            GGTT_LHS_BASE,
            GGTT_RHS_BASE,
            GGTT_OUTPUT_BASE
        );
        crate::log::flush();
    }
    write_named_ggtt_pte(
        mmio,
        "seqno",
        GGTT_SEQNO_BASE,
        pending.seqno.physical_addr().as_u64(),
    )?;
    // iter7: the cross-thread (CURBE) argument payload page. MEDIA_CURBE_LOAD reads
    // the stateless-A64 pointers from here; it must be GGTT-resident before submit.
    write_named_ggtt_pte(
        mmio,
        "curbe",
        GGTT_CURBE_BASE,
        pending.curbe.physical_addr().as_u64(),
    )?;
    if let Some(weights) = &pending.blur_weights {
        if !context_ppgtt {
            write_named_ggtt_pte(
                mmio,
                "blur-weights",
                GGTT_BLUR_WEIGHTS_BASE,
                weights.physical_addr().as_u64(),
            )?;
        } else {
            clear_named_ggtt_pte(mmio, "blur-weights", GGTT_BLUR_WEIGHTS_BASE)?;
        }
    }
    if let Some(curbe_v) = &pending.blur_curbe_v {
        write_named_ggtt_pte(
            mmio,
            "blur-curbe-v",
            GGTT_BLUR_CURBE_V_BASE,
            curbe_v.physical_addr().as_u64(),
        )?;
    }
    Ok(())
}

fn map_compute_execlist_context(mmio: GtMmio, context: &ComputeExeclistContext) -> AccelResult<()> {
    write_named_ggtt_range(
        mmio,
        "blur-lrc",
        GGTT_LRC_BASE,
        context.lrc.physical_addr().as_u64(),
        context.lrc.len(),
    )?;
    write_named_ggtt_pte(
        mmio,
        "blur-hwsp",
        GGTT_HWSP_BASE,
        context.hwsp.physical_addr().as_u64(),
    )?;
    Ok(())
}

fn map_noop_buffers(mmio: GtMmio, pending: &PendingNoop) -> AccelResult<()> {
    // Identical GGTT layout to the Add path: the no-op shares the same command
    // stream and interface descriptor, so the same seven pages must be mapped
    // (the EU kernel never reads lhs/rhs/output, but the descriptor still names
    // them and the walker dispatch path is byte-for-byte the Add path).
    write_named_ggtt_pte(
        mmio,
        "noop-ring",
        GGTT_RING_BASE,
        pending.ring.physical_addr().as_u64(),
    )?;
    write_named_ggtt_pte(
        mmio,
        "noop-state",
        GGTT_STATE_BASE,
        pending.state.physical_addr().as_u64(),
    )?;
    write_named_ggtt_pte(
        mmio,
        "noop-kernel",
        GGTT_KERNEL_BASE,
        pending.kernel.physical_addr().as_u64(),
    )?;
    write_named_ggtt_pte(
        mmio,
        "noop-lhs",
        GGTT_LHS_BASE,
        pending.lhs.physical_addr().as_u64(),
    )?;
    write_named_ggtt_pte(
        mmio,
        "noop-rhs",
        GGTT_RHS_BASE,
        pending.rhs.physical_addr().as_u64(),
    )?;
    write_named_ggtt_pte(
        mmio,
        "noop-output",
        GGTT_OUTPUT_BASE,
        pending.output.physical_addr().as_u64(),
    )?;
    write_named_ggtt_pte(
        mmio,
        "noop-seqno",
        GGTT_SEQNO_BASE,
        pending.seqno.physical_addr().as_u64(),
    )?;
    Ok(())
}

fn map_smoke_buffers(mmio: GtMmio, pending: &PendingSmoke) -> AccelResult<()> {
    write_named_ggtt_pte(
        mmio,
        "smoke-ring",
        GGTT_RING_BASE,
        pending.ring.physical_addr().as_u64(),
    )?;
    write_named_ggtt_pte(
        mmio,
        "smoke-seqno",
        GGTT_SEQNO_BASE,
        pending.seqno.physical_addr().as_u64(),
    )?;
    Ok(())
}

/// Phase 7 BUILD-4X-1: GGTT-map the execlist context's pages. The LRC is mapped as a
/// contiguous GGTT window (`LRC_PAGES` PTEs), then the HWSP and the proven 4a ring/state/
/// kernel/data/seqno pages. The PPGTT page tables are physical (walked from the LRC's
/// PDP0), so they are NOT GGTT-mapped here.
#[cfg(feature = "neo-test")]
fn map_execlist_buffers(mmio: GtMmio, pending: &PendingExeclist) -> AccelResult<()> {
    let lrc_phys = pending.lrc.physical_addr().as_u64();
    for page in 0..LRC_PAGES {
        let off = (page * PAGE_SIZE) as u64;
        write_named_ggtt_pte(mmio, "lrc", GGTT_LRC_BASE + off, lrc_phys + off)?;
    }
    write_named_ggtt_pte(
        mmio,
        "hwsp",
        GGTT_HWSP_BASE,
        pending.hwsp.physical_addr().as_u64(),
    )?;
    write_named_ggtt_pte(
        mmio,
        "xl-ring",
        GGTT_RING_BASE,
        pending.ring.physical_addr().as_u64(),
    )?;
    write_named_ggtt_pte(
        mmio,
        "xl-state",
        GGTT_STATE_BASE,
        pending.state.physical_addr().as_u64(),
    )?;
    write_named_ggtt_pte(
        mmio,
        "xl-kernel",
        GGTT_KERNEL_BASE,
        pending.kernel.physical_addr().as_u64(),
    )?;
    if !pending.kind.is_add() {
        // 4X-1 no-op: all pages GGTT-mapped (the no-op touches no tensor data).
        write_named_ggtt_pte(
            mmio,
            "xl-lhs",
            GGTT_LHS_BASE,
            pending.lhs.physical_addr().as_u64(),
        )?;
        write_named_ggtt_pte(
            mmio,
            "xl-rhs",
            GGTT_RHS_BASE,
            pending.rhs.physical_addr().as_u64(),
        )?;
        write_named_ggtt_pte(
            mmio,
            "xl-output",
            GGTT_OUTPUT_BASE,
            pending.output.physical_addr().as_u64(),
        )?;
    } else {
        // 4X-2/4X-3 Add family: the data buffers are mapped ONLY in the per-context PML4 —
        // actively CLEARED from GGTT so a non-PPGTT access faults (the self-proving gate).
        // The GPU GGTT persists across reboot, so we clear rather than skip (the BUILD-4-1
        // lesson). The CURBE indirect-data heap stays GGTT-mapped (pipeline state, not the
        // tested tensor data) and is also PML4-mapped. The per-kind PML4 mapping (lhs
        // omitted/RO, output RO) is what the 4X-3 negative test exercises — set in `new`.
        clear_named_ggtt_pte(mmio, "xl-lhs", GGTT_LHS_BASE)?;
        clear_named_ggtt_pte(mmio, "xl-rhs", GGTT_RHS_BASE)?;
        clear_named_ggtt_pte(mmio, "xl-output", GGTT_OUTPUT_BASE)?;
        serial_println!(
            "[ kernel ] accel-intel: execlist-{} data buffers CLEARED from GGTT (lhs/rhs/output VAs 0x{:x}/0x{:x}/0x{:x} resolve via per-context PPGTT only)",
            pending.kind.label(),
            GGTT_LHS_BASE,
            GGTT_RHS_BASE,
            GGTT_OUTPUT_BASE
        );
        crate::log::flush();
        if let Some(curbe) = &pending.curbe {
            write_named_ggtt_pte(
                mmio,
                "xl-curbe",
                GGTT_CURBE_BASE,
                curbe.physical_addr().as_u64(),
            )?;
        }
    }
    write_named_ggtt_pte(
        mmio,
        "xl-seqno",
        GGTT_SEQNO_BASE,
        pending.seqno.physical_addr().as_u64(),
    )?;
    Ok(())
}

/// Phase 7 BUILD-4X-1: point the engine at a valid HWSP and ENABLE execlist submission
/// (the inverse of `configure_legacy_ring`, which clears GFX_RUN_LIST_ENABLE). The ring
/// registers are deliberately NOT programmed — the engine loads them from the LRC on
/// context switch-in.
// OT-69: NOT `neo-test`-gated. This programs GFX_RUN_LIST_ENABLE — it is
// functional engine setup on the real dispatch path, not diagnostics, and its
// call site is ungated. The stale gate made `--features accel-intel` fail to
// compile at all; nothing here depends on `neo-test`.
fn configure_execlist_engine(mmio: GtMmio, forcewake: ForcewakeState) -> AccelResult<()> {
    // RESEARCH §1 enable_execlists sequence (forcewake already held by the caller):
    //   1. RING_HWSTAM ← ~0           (mask all HWSP-write IRQs; we poll)
    //   2. RING_MODE_GEN7 ← 0x80008000 (masked-enable GFX_RUN_LIST_ENABLE, Gen9 bit 15)
    //   3. RING_MI_MODE  ← clear STOP_RING (un-stop the command streamer)
    //   4. RING_HWS_PGA  ← GGTT status-page offset, then POSTING_READ
    // (MOCS is moot for the no-op; it is a 4X-2 requirement.) The ring registers are
    // NOT programmed here — the engine loads them from the LRC on context switch-in.
    // BUILD-4X-3: clear any sticky RING_FAULT from a prior (negative-test) submission so a
    // later context starts on a clean engine — the 4X-3 scenario runs success + two fault
    // contexts in one boot, and a left-over fault must not bleed into the next run's gate.
    mmio.write32("execlist-ring-fault-clear", GEN8_RING_FAULT_REG_RCS, 0)?;
    mmio.write32(
        "execlist-hwstam",
        execlist::RING_HWSTAM as usize,
        0xffff_ffff,
    )?;
    mmio.write32(
        "execlist-mode-enable",
        execlist::RING_MODE as usize,
        execlist::ring_mode_enable_execlists(),
    )?;
    mmio.write32(
        "execlist-mi-mode-unstop",
        execlist::RING_MI_MODE as usize,
        execlist::mi_mode_unstop_ring(),
    )?;
    mmio.write32(
        "execlist-hws-pga",
        execlist::RING_HWS_PGA as usize,
        GGTT_HWSP_BASE as u32,
    )?;
    breadcrumb("execlist-engine-configured");
    if forcewake.render_acquired {
        // POSTING_READ of HWS_PGA (RESEARCH §1 step 5) + the execlist-enable readback —
        // the decision tree's "check execlist-enable FIRST" discriminator.
        let mode = mmio
            .read32("execlist-mode-read", execlist::RING_MODE as usize)
            .unwrap_or(0);
        let hws = mmio
            .read32("execlist-hws-read", execlist::RING_HWS_PGA as usize)
            .unwrap_or(0);
        let mi_mode = mmio
            .read32("execlist-mi-mode-read", execlist::RING_MI_MODE as usize)
            .unwrap_or(0);
        serial_println!(
            "[ kernel ] accel-intel: execlist-engine mode=0x{:08x} execlist_enabled={} mi_mode=0x{:08x} ring_stopped={} hws_pga=0x{:08x} expected_hws=0x{:x}",
            mode,
            (mode & GFX_RUN_LIST_ENABLE) != 0,
            mi_mode,
            (mi_mode & execlist::STOP_RING) != 0,
            hws,
            GGTT_HWSP_BASE
        );
        crate::log::flush();
    }
    Ok(())
}

/// Phase 7 BUILD-4X-1 (ISC-5): dump the Context Status Buffer + EXECLIST_STATUS + the CSB
/// write pointer from MMIO, decoding each entry's status bits (IDLE_ACTIVE switch-in /
/// ACTIVE_IDLE idle / COMPLETE) and cross-checking the ctx-id dword against the descriptor
/// we submitted (RESEARCH §5). A run with **zero** non-zero CSB entries means the
/// submission was never accepted (decision tree tree-A), which the blind-hardware rule
/// forbids leaving undiagnosable. Records the readbacks into the capture statics the gate
/// reads. The PTR's 0x7-reset quirk is logged (a freshly powered engine reads write=0x7).
// OT-69: NOT `neo-test`-gated — its call sites on the dispatch path are
// ungated, and the CSB capture statics it writes are ungated too.
fn dump_execlist_csb(mmio: GtMmio, token: DispatchToken, label: &str) {
    let status_lo = mmio
        .read32(
            "read-EXECLIST_STATUS_LO",
            execlist::RING_EXECLIST_STATUS_LO as usize,
        )
        .unwrap_or(0);
    let status_hi = mmio
        .read32(
            "read-EXECLIST_STATUS_HI",
            execlist::RING_EXECLIST_STATUS_HI as usize,
        )
        .unwrap_or(0);
    let status_ptr = mmio
        .read32(
            "read-CSB_STATUS_PTR",
            execlist::RING_CONTEXT_STATUS_PTR as usize,
        )
        .unwrap_or(0);
    LAST_EXECLIST_STATUS_LO.store(status_lo, Ordering::Release);
    LAST_EXECLIST_STATUS_HI.store(status_hi, Ordering::Release);
    LAST_CSB_STATUS_PTR.store(status_ptr, Ordering::Release);
    let write_ptr = status_ptr & execlist::CSB_PTR_MASK;
    serial_println!(
        "[ kernel ] accel-intel: execlist-csb {} token={} EXECLIST_STATUS=0x{:08x}{:08x} CSB_PTR=0x{:08x} write_idx={} reset_quirk={}",
        label,
        token.sequence,
        status_hi,
        status_lo,
        status_ptr,
        write_ptr,
        write_ptr == execlist::CSB_PTR_RESET
    );
    crate::log::flush();
    // The descriptor's ctx-id (upper dword) the CSB ctx-id should match for our context.
    let our_ctx_id = (LAST_EXECLIST_DESCRIPTOR.load(Ordering::Acquire) >> 32) as u32;
    let mut nonzero = 0u32;
    let mut switch_in_seen = false;
    let mut idle_seen = false;
    for i in 0..execlist::CSB_ENTRIES {
        let csb_status = mmio
            .read32(
                "read-CSB-status",
                execlist::ring_context_status_buf_status(i) as usize,
            )
            .unwrap_or(0);
        let csb_ctxid = mmio
            .read32(
                "read-CSB-ctxid",
                execlist::ring_context_status_buf_ctxid(i) as usize,
            )
            .unwrap_or(0);
        if csb_status != 0 || csb_ctxid != 0 {
            nonzero += 1;
        }
        let idle_active = (csb_status & execlist::CSB_IDLE_ACTIVE) != 0;
        let active_idle = (csb_status & execlist::CSB_ACTIVE_IDLE) != 0;
        let complete = (csb_status & execlist::CSB_COMPLETE) != 0;
        if idle_active {
            switch_in_seen = true;
        }
        if active_idle || complete {
            idle_seen = true;
        }
        serial_println!(
            "[ kernel ] accel-intel: execlist-csb-entry {} token={} i={} status=0x{:08x} ctxid=0x{:08x} switch_in={} went_idle={} complete={} ctxid_matches_ours={}",
            label,
            token.sequence,
            i,
            csb_status,
            csb_ctxid,
            idle_active,
            active_idle,
            complete,
            csb_ctxid == our_ctx_id && our_ctx_id != 0
        );
        crate::log::flush();
    }
    LAST_CSB_NONZERO.store(nonzero, Ordering::Release);
    serial_println!(
        "[ kernel ] accel-intel: execlist-csb-summary {} token={} nonzero_entries={} switch_in_seen={} idle_seen={} switch_in_then_idle={}",
        label,
        token.sequence,
        nonzero,
        switch_in_seen,
        idle_seen,
        switch_in_seen && idle_seen
    );
    crate::log::flush();
}

fn configure_legacy_ring(mmio: GtMmio, forcewake: ForcewakeState) -> AccelResult<()> {
    let ring_ctl = ((RING_BYTES as u32 - PAGE_SIZE as u32) & RING_CTL_SIZE_MASK) | RING_CTL_VALID;
    // These are the Gen9 render-ring HEAD/TAIL/START/CTL and RING_MODE
    // registers per the Kaby Lake PRM render-engine MMIO definitions. The
    // caller has attempted render forcewake first, as required by PRM Vol. 13's
    // force-wake table for the 0x02000-0x026ff render-register range.
    mmio.write32(
        "ring-mode-write",
        RING_MODE,
        masked_bit_disable(GFX_RUN_LIST_ENABLE),
    )?;
    mmio.write32("ring-ctl-disable", RING_CTL, 0)?;
    mmio.write32("ring-head-reset", RING_HEAD, 0)?;
    mmio.write32("ring-tail-reset", RING_TAIL, 0)?;
    mmio.write32("ring-start-write", RING_START, GGTT_RING_BASE as u32)?;
    mmio.write32("ring-ctl-enable", RING_CTL, ring_ctl)?;
    breadcrumb("ring-state-verify");
    if !forcewake.render_acquired {
        serial_println!(
            "[ kernel ] accel-intel: ring-state-verify skipped render_forcewake_ack=0x{:08x}",
            forcewake.render_ack
        );
        crate::log::flush();
        return Ok(());
    }
    let Some(ring_mode_read) = mmio.read32("ring-mode-read", RING_MODE) else {
        return Err(AccelError::InvalidRequest);
    };
    let Some(ring_ctl_read) = mmio.read32("ring-ctl-read", RING_CTL) else {
        return Err(AccelError::InvalidRequest);
    };
    serial_println!(
        "[ kernel ] accel-intel: legacy-ring mode ring_mode=0x{:08x} execlist_enabled={} ring_ctl=0x{:08x} expected_ring_ctl=0x{:08x}",
        ring_mode_read,
        (ring_mode_read & GFX_RUN_LIST_ENABLE) != 0,
        ring_ctl_read,
        ring_ctl
    );
    crate::log::flush();
    if (ring_mode_read & GFX_RUN_LIST_ENABLE) != 0
        || (ring_ctl_read & (RING_CTL_SIZE_MASK | RING_CTL_VALID)) != ring_ctl
    {
        return Err(AccelError::InvalidRequest);
    }
    Ok(())
}

#[derive(Copy, Clone, Default)]
struct ForcewakeState {
    render_ack: u32,
    gt_ack: u32,
    render_acquired: bool,
    gt_acquired: bool,
}

struct ForcewakeGuard {
    mmio: GtMmio,
    state: ForcewakeState,
}

impl ForcewakeGuard {
    fn acquire(mmio: GtMmio) -> Self {
        let guard = Self::acquire_best_effort(mmio);
        if !guard.state.render_acquired {
            serial_println!(
                "[ kernel ] accel-intel: forcewake-acquire-failed render_ack=0x{:08x} gt_ack=0x{:08x} render_ok={} gt_ok={}",
                guard.state.render_ack,
                guard.state.gt_ack,
                guard.state.render_acquired,
                guard.state.gt_acquired
            );
            crate::log::flush();
        }
        guard
    }

    fn acquire_for_status(mmio: GtMmio) -> Self {
        Self::acquire_best_effort(mmio)
    }

    fn acquire_best_effort(mmio: GtMmio) -> Self {
        let (render_acquired, render_ack) = acquire_forcewake_domain(
            mmio,
            "render",
            FORCEWAKE_RENDER_GEN9,
            FORCEWAKE_ACK_RENDER_GEN9,
            "read-FORCEWAKE_ACK_RENDER",
            FORCEWAKE_KERNEL,
        );
        let (gt_acquired, gt_ack) = acquire_forcewake_domain(
            mmio,
            "gt",
            FORCEWAKE_GT_GEN9,
            FORCEWAKE_ACK_GT_GEN9,
            "read-FORCEWAKE_ACK_GT",
            FORCEWAKE_KERNEL,
        );
        let state = ForcewakeState {
            render_ack,
            gt_ack,
            render_acquired,
            gt_acquired,
        };
        LAST_FORCEWAKE_RENDER_ACK.store(render_ack, Ordering::Release);
        LAST_FORCEWAKE_GT_ACK.store(gt_ack, Ordering::Release);
        Self { mmio, state }
    }

    fn state(&self) -> ForcewakeState {
        self.state
    }
}

impl Drop for ForcewakeGuard {
    fn drop(&mut self) {
        if self.state.gt_acquired {
            release_forcewake_domain(
                self.mmio,
                "gt",
                FORCEWAKE_GT_GEN9,
                FORCEWAKE_ACK_GT_GEN9,
                "read-FORCEWAKE_ACK_GT",
                FORCEWAKE_KERNEL,
            );
        }
        if self.state.render_acquired {
            release_forcewake_domain(
                self.mmio,
                "render",
                FORCEWAKE_RENDER_GEN9,
                FORCEWAKE_ACK_RENDER_GEN9,
                "read-FORCEWAKE_ACK_RENDER",
                FORCEWAKE_KERNEL,
            );
        }
    }
}

#[derive(Copy, Clone, Default)]
struct GtStatusSnapshot {
    forcewake_render_ack: u32,
    forcewake_gt_ack: u32,
    ring_head: u32,
    ring_tail: u32,
    ring_ctl: u32,
    ring_mode: u32,
    ring_instdone: u32,
    ring_acthd: u32,
    ring_acthd_udw: u32,
    ring_rpcs: u32,
    ring_fault: u32,
    ring_fault_data0: u32,
    ring_fault_data1: u32,
    gt_core_status: u32,
    sc_instdone: u32,
    sampler_instdone: u32,
    row_instdone: u32,
    td_ctl: u32,
    seqno: u32,
}

fn read_gt_status_guarded(mmio: GtMmio, seqno_ptr: *const u32) -> GtStatusSnapshot {
    let guard = ForcewakeGuard::acquire_for_status(mmio);
    read_gt_status(mmio, seqno_ptr, guard.state())
}

fn read_gt_status(
    mmio: GtMmio,
    seqno_ptr: *const u32,
    forcewake: ForcewakeState,
) -> GtStatusSnapshot {
    let mut snapshot = GtStatusSnapshot {
        forcewake_render_ack: forcewake.render_ack,
        forcewake_gt_ack: forcewake.gt_ack,
        // SAFETY: `seqno_ptr` points into the live seqno DmaBuffer owned by the
        // pending dispatch while this status snapshot is collected.
        seqno: unsafe { read_volatile(seqno_ptr) },
        ..GtStatusSnapshot::default()
    };
    if forcewake.render_acquired {
        snapshot.ring_head = mmio.read32("read-RING_HEAD", RING_HEAD).unwrap_or(0);
        snapshot.ring_tail = mmio.read32("read-RING_TAIL", RING_TAIL).unwrap_or(0);
        snapshot.ring_ctl = mmio.read32("read-RING_CTL", RING_CTL).unwrap_or(0);
        snapshot.ring_mode = mmio.read32("read-RING_MODE", RING_MODE).unwrap_or(0);
        snapshot.ring_instdone = mmio.read32("read-INSTDONE", RING_INSTDONE).unwrap_or(0);
        snapshot.ring_acthd = mmio.read32("read-ACTHD", RING_ACTHD).unwrap_or(0);
        snapshot.ring_acthd_udw = mmio.read32("read-ACTHD_UDW", RING_ACTHD_UDW).unwrap_or(0);
        snapshot.ring_rpcs = mmio
            .read32("read-R_PWR_CLK_STATE", RING_R_PWR_CLK_STATE)
            .unwrap_or(0);
        snapshot.ring_fault = mmio
            .read32("read-RING_FAULT", GEN8_RING_FAULT_REG_RCS)
            .unwrap_or(0);
        // iter7 ISC-5a: capture the faulting address pair alongside the fault reg
        // so a valid fault names *where* it touched, not just the fault type.
        snapshot.ring_fault_data0 = mmio
            .read32("read-FAULT_TLB_DATA0", GEN8_FAULT_TLB_DATA0)
            .unwrap_or(0);
        snapshot.ring_fault_data1 = mmio
            .read32("read-FAULT_TLB_DATA1", GEN8_FAULT_TLB_DATA1)
            .unwrap_or(0);
        // Iter5 ISC-2: widen the view into the EU slice/row so a parked GPGPU
        // thread names its unit. Best-effort under render forcewake; the
        // multicast (MCR-steered) ROW_INSTDONE/TD_CTL return the boot-default
        // steered row, sufficient for "is an EU row parked".
        snapshot.sc_instdone = mmio
            .read32("read-SC_INSTDONE", GEN8_SC_INSTDONE)
            .unwrap_or(0);
        snapshot.sampler_instdone = mmio
            .read32("read-SAMPLER_INSTDONE", GEN7_SAMPLER_INSTDONE)
            .unwrap_or(0);
        snapshot.row_instdone = mmio
            .read32("read-ROW_INSTDONE", GEN8_ROW_INSTDONE)
            .unwrap_or(0);
        snapshot.td_ctl = mmio.read32("read-TD_CTL", GEN8_TD_CTL).unwrap_or(0);
    } else {
        breadcrumb("skip-render-status-forcewake-missing");
    }
    if forcewake.gt_acquired {
        snapshot.gt_core_status = mmio
            .read32("read-GT_CORE_STATUS", GEN6_GT_CORE_STATUS)
            .unwrap_or(0);
    } else {
        breadcrumb("skip-GT_CORE_STATUS-forcewake-missing");
    }
    record_gt_status(snapshot);
    snapshot
}

fn log_gt_status(label: &str, token: DispatchToken, snapshot: GtStatusSnapshot) {
    serial_println!(
        "[ kernel ] accel-intel: {} token={} RING_HEAD=0x{:08x} RING_TAIL=0x{:08x} FORCEWAKE_ACK=0x{:08x} GT_FORCEWAKE_ACK=0x{:08x} RING_CTL=0x{:08x} RING_MODE=0x{:08x} execlist_enabled={} INSTDONE=0x{:08x} ACTHD=0x{:08x}{:08x} R_PWR_CLK_STATE=0x{:08x} RING_FAULT=0x{:08x} GT_CORE_STATUS=0x{:08x} SC_INSTDONE=0x{:08x} SAMPLER_INSTDONE=0x{:08x} ROW_INSTDONE=0x{:08x} TD_CTL=0x{:08x} seqno=0x{:08x}",
        label,
        token.sequence,
        snapshot.ring_head,
        snapshot.ring_tail,
        snapshot.forcewake_render_ack,
        snapshot.forcewake_gt_ack,
        snapshot.ring_ctl,
        snapshot.ring_mode,
        (snapshot.ring_mode & GFX_RUN_LIST_ENABLE) != 0,
        snapshot.ring_instdone,
        snapshot.ring_acthd_udw,
        snapshot.ring_acthd,
        snapshot.ring_rpcs,
        snapshot.ring_fault,
        snapshot.gt_core_status,
        snapshot.sc_instdone,
        snapshot.sampler_instdone,
        snapshot.row_instdone,
        snapshot.td_ctl,
        snapshot.seqno
    );
    crate::log::flush();
    decode_instdone(label, token, snapshot.ring_instdone);
    if snapshot.ring_fault & RING_FAULT_VALID != 0 {
        decode_ring_fault(label, token, snapshot);
    }
}

/// iter7 ISC-5a/5b: on a valid `RING_FAULT`, decode the fault type/source and the
/// faulting GPU virtual address (from the `FAULT_TLB_DATA0/1` pair) plus the
/// GTT-vs-PPGTT select. This makes a repeat of iter6's `0x9e1` self-diagnosing:
/// - addr in 0x0013_xxxx (one of our GGTT buffer pages) -> the payload is right
///   and the bug is the A64 data-port address space (GGTT vs PPGTT);
/// - a garbage/other addr -> the CURBE pointers landed in the wrong GRF.
///
/// Decode fields per i915 `gen8_check_faults`: addr = ((DATA1 & 0xf) << 44) |
/// (DATA0 << 12); DATA1 bit4 = GTT select; RING_FAULT bits[2:1]=type, [10:3]=srcid,
/// [12]=engine GTT select.
fn decode_ring_fault(label: &str, token: DispatchToken, snapshot: GtStatusSnapshot) {
    let fault = snapshot.ring_fault;
    let fault_type = (fault >> 1) & 0x3;
    let srcid = (fault >> 3) & 0xff;
    let fault_addr = (((snapshot.ring_fault_data1 & FAULT_TLB_DATA1_VA_HIGH_MASK) as u64) << 44)
        | ((snapshot.ring_fault_data0 as u64) << 12);
    let gtt_sel = snapshot.ring_fault_data1 & FAULT_TLB_DATA1_GTT_SEL != 0;
    let in_build4a_window = (0x0013_0000..=0x0013_6fff).contains(&fault_addr);
    serial_println!(
        "[ kernel ] accel-intel: ring-fault-decode {} token={} RING_FAULT=0x{:08x} valid=true fault_type={} ({}) srcid=0x{:02x} addr_space={} fault_gpu_va=0x{:012x} data0=0x{:08x} data1=0x{:08x} in_build4a_window={}",
        label,
        token.sequence,
        fault,
        fault_type,
        ring_fault_type_name(fault_type),
        srcid,
        if gtt_sel { "GGTT" } else { "PPGTT" },
        fault_addr,
        snapshot.ring_fault_data0,
        snapshot.ring_fault_data1,
        in_build4a_window
    );
    crate::log::flush();
}

/// Best-effort Gen8/9 `RING_FAULT_REG` fault-type name (bits [2:1]).
fn ring_fault_type_name(fault_type: u32) -> &'static str {
    match fault_type {
        0 => "invalid-PTE/not-present",
        1 => "write-access-violation",
        2 => "privilege-violation",
        _ => "reserved",
    }
}

/// Decode which `RING_INSTDONE` bits are *clear* (a clear bit = that functional
/// unit reports "not done", i.e. busy/parked). The iter4 compute hang sampled
/// `INSTDONE=0xffdeffff`; its busy mask is `!0xffdeffff = 0x00210000`, i.e. bits
/// 16 and 21 — two EU-area units parked past the `GPGPU_WALKER`, versus the
/// near-idle smoke value `0xfffffffe` (busy mask `0x1`, bit 0 = the command
/// streamer alone). Bit→unit names below follow the Gen9 RCS `RING_INSTDONE`
/// hangcheck field map (KBL PRM Vol. 2c); units we cannot name are reported by
/// index so the raw busy mask is never lost.
fn decode_instdone(label: &str, token: DispatchToken, instdone: u32) {
    let busy = !instdone;
    serial_println!(
        "[ kernel ] accel-intel: instdone-decode {} token={} INSTDONE=0x{:08x} busy_mask=0x{:08x} compute_ref_match={}",
        label,
        token.sequence,
        instdone,
        busy,
        busy == 0x0021_0000
    );
    crate::log::flush();
    let mut remaining = busy;
    while remaining != 0 {
        let bit = remaining.trailing_zeros();
        remaining &= remaining - 1;
        serial_println!(
            "[ kernel ] accel-intel: instdone-busy-bit {} token={} bit={} unit={}",
            label,
            token.sequence,
            bit,
            instdone_bit_unit(bit)
        );
        crate::log::flush();
    }
}

/// Best-effort Gen9 RCS `RING_INSTDONE` bit → functional-unit name. The
/// command-streamer bit and the two EU-area bits seen parked in the iter4
/// compute hang are named; all other bits are reported as `eu-area-bitN` so the
/// decode is never silently lossy.
fn instdone_bit_unit(bit: u32) -> &'static str {
    match bit {
        0 => "command-streamer",
        16 => "eu-thread-dispatch(row0)",
        21 => "eu-instruction-fetch(row0)",
        _ => "eu-area-bit",
    }
}

fn record_gt_status(snapshot: GtStatusSnapshot) {
    LAST_FORCEWAKE_RENDER_ACK.store(snapshot.forcewake_render_ack, Ordering::Release);
    LAST_FORCEWAKE_GT_ACK.store(snapshot.forcewake_gt_ack, Ordering::Release);
    LAST_RING_HEAD.store(snapshot.ring_head, Ordering::Release);
    LAST_RING_TAIL.store(snapshot.ring_tail, Ordering::Release);
    LAST_RING_CTL.store(snapshot.ring_ctl, Ordering::Release);
    LAST_RING_MODE.store(snapshot.ring_mode, Ordering::Release);
    LAST_RING_FAULT.store(snapshot.ring_fault, Ordering::Release);
    LAST_FAULT_TLB_DATA0.store(snapshot.ring_fault_data0, Ordering::Release);
    LAST_FAULT_TLB_DATA1.store(snapshot.ring_fault_data1, Ordering::Release);
    LAST_RING_INSTDONE.store(snapshot.ring_instdone, Ordering::Release);
    LAST_GT_CORE_STATUS.store(snapshot.gt_core_status, Ordering::Release);
    LAST_SC_INSTDONE.store(snapshot.sc_instdone, Ordering::Release);
    LAST_SAMPLER_INSTDONE.store(snapshot.sampler_instdone, Ordering::Release);
    LAST_ROW_INSTDONE.store(snapshot.row_instdone, Ordering::Release);
    LAST_TD_CTL.store(snapshot.td_ctl, Ordering::Release);
    LAST_SEQNO_VALUE.store(snapshot.seqno, Ordering::Release);
}

fn acquire_forcewake_domain(
    mmio: GtMmio,
    domain: &str,
    request_reg: usize,
    ack_reg: usize,
    ack_step: &str,
    bit: u32,
) -> (bool, u32) {
    let _ = wait_forcewake_ack(mmio, domain, ack_reg, ack_step, bit, false);
    let write_ok = mmio
        .write32("forcewake-write", request_reg, masked_bit_enable(bit))
        .is_ok();
    let ack = wait_forcewake_ack(mmio, domain, ack_reg, ack_step, bit, true).unwrap_or(0);
    let acquired = write_ok && (ack & bit) == bit;
    if !acquired {
        serial_println!(
            "[ kernel ] accel-intel: forcewake timeout domain={} ack=0x{:08x} write_ok={}",
            domain,
            ack,
            write_ok
        );
        crate::log::flush();
    }
    (acquired, ack)
}

fn release_forcewake_domain(
    mmio: GtMmio,
    domain: &str,
    request_reg: usize,
    ack_reg: usize,
    ack_step: &str,
    bit: u32,
) {
    // Symmetric release for the Gen9 forcewake request acquired above; masked
    // writes preserve unrelated request bits in the register.
    let _ = mmio.write32("forcewake-release", request_reg, masked_bit_disable(bit));
    let _ = wait_forcewake_ack(mmio, domain, ack_reg, ack_step, bit, false);
}

fn wait_forcewake_ack(
    mmio: GtMmio,
    domain: &str,
    ack_reg: usize,
    ack_step: &str,
    bit: u32,
    want_set: bool,
) -> Option<u32> {
    let expected = if want_set { bit } else { 0 };
    let mut value = 0;
    breadcrumb_mmio("forcewake-ack-poll-begin", ack_reg, 4);
    breadcrumb_mmio(ack_step, ack_reg, 4);
    if !mmio.contains(ack_reg, 4) {
        breadcrumb_mmio_skip(ack_step, ack_reg, 4, mmio.mapped_len);
        return None;
    }
    for _ in 0..FORCEWAKE_POLL_LIMIT {
        // SAFETY: The bounds check above proves the ACK register is inside the
        // mapped BAR; this loop avoids per-iteration breadcrumb flushes.
        value = unsafe { mmio.read32_raw_unchecked(ack_reg) };
        if (value & bit) == expected {
            breadcrumb_mmio_value("forcewake-ack-poll-end", ack_reg, value, true);
            return Some(value);
        }
        core::hint::spin_loop();
    }
    serial_println!(
        "[ kernel ] accel-intel: forcewake ack poll timeout domain={} want_set={} ack=0x{:08x}",
        domain,
        want_set,
        value
    );
    crate::log::flush();
    breadcrumb_mmio_value("forcewake-ack-poll-end", ack_reg, value, false);
    Some(value)
}

fn masked_bit_enable(bit: u32) -> u32 {
    (bit << 16) | bit
}

fn masked_bit_disable(bit: u32) -> u32 {
    bit << 16
}

#[derive(Copy, Clone)]
struct InterfaceDescriptorDebug {
    kernel_start: u32,
    thread_group_threads: u32,
    cross_thread_read_grf: u32,
    per_thread_read_grf: u32,
    kernel_ptr_matches: bool,
}

fn write_interface_descriptor(
    state: &mut [u8],
    cross_thread_read_grf: u32,
    per_thread_read_grf: u32,
) -> InterfaceDescriptorDebug {
    // Gen9 INTERFACE_DESCRIPTOR_DATA (32 bytes / 8 DWords; KBL PRM Vol. 2a). The
    // EU fetches the kernel from Instruction Base (0, set by STATE_BASE_ADDRESS) +
    // this Kernel Start Pointer (DW0). Both kernels are stateless A64, so there is
    // NO sampler (DW3) and NO binding table (DW4). iter8's Add sets the two read
    // lengths that split the GPGPU_WALKER indirect-data heap into the thread's GRF
    // (NEO `setGrfInfo`): DW5[31:16] = Constant/Indirect URB Entry Read Length
    // (per-thread local_id → r1..r6) and DW7[7:0] = Cross-Thread Constant Data Read
    // Length (the a/b/o pointers → r7..r9). The no-op passes 0/0.
    state.fill(0);
    let descriptor = InterfaceDescriptorDebug {
        kernel_start: BUILD4A_LAYOUT.kernel_base as u32,
        thread_group_threads: 1,
        cross_thread_read_grf,
        per_thread_read_grf,
        kernel_ptr_matches: BUILD4A_LAYOUT.kernel_base == GGTT_KERNEL_BASE,
    };
    write_le_u32(state, 0, descriptor.kernel_start); // DW0 Kernel Start Pointer
    // DW5 [31:16] Constant/Indirect URB Entry Read Length (per-thread), [15:0] offset = 0.
    write_le_u32(state, 20, per_thread_read_grf << 16);
    write_le_u32(state, 24, descriptor.thread_group_threads); // DW6 [9:0] # threads
    write_le_u32(state, 28, cross_thread_read_grf); // DW7 [7:0] Cross-Thread Read Length (GRF)
    descriptor
}

fn write_blur_interface_descriptor(state: &mut [u8]) -> InterfaceDescriptorDebug {
    state.fill(0);
    let dwords = blur_interface_descriptor_dwords();
    for (index, dword) in dwords.iter().copied().enumerate() {
        write_le_u32(state, index * 4, dword);
    }
    InterfaceDescriptorDebug {
        kernel_start: dwords[0],
        thread_group_threads: dwords[6] & 0x3ff,
        cross_thread_read_grf: dwords[7] & 0xff,
        per_thread_read_grf: dwords[5] >> 16,
        kernel_ptr_matches: u64::from(dwords[0]) == GGTT_KERNEL_BASE,
    }
}

fn blur_execlist_context_id(sequence: u64) -> u32 {
    const MAX_CONTEXT_ID: u64 = (1 << 21) - 1;
    (((sequence.saturating_sub(1)) % MAX_CONTEXT_ID) + 1) as u32
}

fn log_interface_descriptor(descriptor: InterfaceDescriptorDebug) {
    serial_println!(
        "[ kernel ] accel-intel: interface-descriptor kernel_start=0x{:08x} expected_kernel_ggtt=0x{:08x} kernel_ptr_matches={} thread_group_threads={} cross_thread_read_grf={} per_thread_read_grf={} stateless=true(no sampler/binding-table)",
        descriptor.kernel_start,
        GGTT_KERNEL_BASE as u32,
        descriptor.kernel_ptr_matches,
        descriptor.thread_group_threads,
        descriptor.cross_thread_read_grf,
        descriptor.per_thread_read_grf
    );
    crate::log::flush();
}

fn log_ring_batch(mode: &str, token: DispatchToken, ring_offset: usize, command_len: usize) {
    serial_println!(
        "[ kernel ] accel-intel: ring-batch mode={} token={} ring_ggtt=0x{:x} byte_offset=0x{:x} command_len={} tail=0x{:x}",
        mode,
        token.sequence,
        GGTT_RING_BASE,
        ring_offset,
        command_len,
        ring_offset + command_len
    );
    crate::log::flush();
}

fn ggtt_pte_offset(ggtt_addr: u64) -> usize {
    let pte_index = (ggtt_addr / GGTT_PAGE_SIZE) as usize;
    GGTT_PTE_BASE + pte_index * 8
}

fn write_named_ggtt_pte(mmio: GtMmio, label: &str, ggtt_addr: u64, phys: u64) -> AccelResult<()> {
    let offset = ggtt_pte_offset(ggtt_addr);
    let pte = (phys & 0x000f_ffff_ffff_f000) | GGTT_PTE_PRESENT_LLC;
    serial_println!(
        "[ kernel ] accel-intel: ggtt-pte label={} ggtt=0x{:x} phys=0x{:x} flags=0x{:x} pte=0x{:016x} offset=0x{:x}",
        label,
        ggtt_addr,
        phys,
        GGTT_PTE_PRESENT_LLC,
        pte,
        offset
    );
    crate::log::flush();
    mmio.write64("ggtt-pte-write", offset, pte)
}

fn write_named_ggtt_range(
    mmio: GtMmio,
    label: &str,
    ggtt_addr: u64,
    phys: u64,
    len: usize,
) -> AccelResult<()> {
    for page in 0..len.div_ceil(PAGE_SIZE) {
        let offset = (page * PAGE_SIZE) as u64;
        write_named_ggtt_pte(mmio, label, ggtt_addr + offset, phys + offset)?;
    }
    Ok(())
}

/// Write an **invalid** (present=0) GGTT page-table entry, actively unmapping a VA
/// from the global GTT (Phase 7 BUILD-4-1, iter2). The GPU's GGTT is hardware state
/// that persists across a kernel reboot, so a *stale* valid entry from a prior run
/// (e.g. the GGTT `accel_intel_add_bringup` path) would silently catch an A64 access
/// the PPGTT path intends to resolve via PPGTT — masking the self-proving fault. We
/// clear the slot so a data access that did NOT go through PPGTT faults (or hits the
/// GGTT scratch), exposing the GTT/PPGTT `select` bit instead of reading stale frames.
fn clear_named_ggtt_pte(mmio: GtMmio, label: &str, ggtt_addr: u64) -> AccelResult<()> {
    let offset = ggtt_pte_offset(ggtt_addr);
    serial_println!(
        "[ kernel ] accel-intel: ggtt-pte-clear label={} ggtt=0x{:x} pte=0x0000000000000000 offset=0x{:x} (invalidate stale mapping for ppgtt-only buffer)",
        label,
        ggtt_addr,
        offset
    );
    crate::log::flush();
    mmio.write64("ggtt-pte-clear", offset, 0)
}

fn wait_poll_budget(timeout: Duration) -> usize {
    let micros = timeout.as_micros();
    let polls = usize::try_from(micros.saturating_mul(10)).unwrap_or(MAX_WAIT_POLLS);
    polls.clamp(1, MAX_WAIT_POLLS)
}

fn pack_bdf(dev: PciDevice) -> u32 {
    ((dev.bus as u32) << 16) | ((dev.device as u32) << 8) | dev.function as u32
}

/// Read the last-probed iGPU's PCI COMMAND register straight from config
/// space. The I-14(c) refusal discriminator uses this to assert that BUS
/// MASTER is genuinely clear on hardware after a refused probe — evidence
/// about the device, not about our control flow.
#[cfg(feature = "neo-test")]
pub fn debug_command_register() -> u16 {
    let bdf = LAST_BDF.load(Ordering::Acquire);
    pci::command_register_at(
        ((bdf >> 16) & 0xff) as u8,
        ((bdf >> 8) & 0xff) as u8,
        (bdf & 0xff) as u8,
    )
}

fn write_le_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
