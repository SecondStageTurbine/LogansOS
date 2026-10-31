// SPDX-License-Identifier: MPL-2.0
//! OT-34: cross-core deterministic fork-join for the forward matmul.
//!
//! The forward matmul splits its OUTPUT COLUMNS across CPUs; each core runs the
//! identical F4 kernel over a disjoint column range, so the assembled output is
//! byte-identical regardless of core count (I-1 result-invariance under
//! parallelism — see `forward_logic`'s column-partition substrate for why: each
//! output column is an independent dot product, so no cross-core float reduction
//! is ever performed). This module is the CROSS-CORE EXECUTOR.
//!
//! ## Design (obviously-correct, minimal-`unsafe`)
//!
//! - **Per-CPU work slots, not a shared descriptor.** The BSP fills AP `t`'s
//!   slot with that partition's work, then blocks on a completion barrier. Each
//!   slot is written by the BSP only when the previous job on that slot is fully
//!   drained, and read by exactly ONE executor. There is no shared descriptor and
//!   thus no cross-CPU descriptor-reuse race.
//! - **Single-winner claim.** A partition is executed by whoever wins
//!   `CAS(pending: true -> false)` — either the assigned AP (from `ap_poll`) or
//!   the BSP itself (steal-back if the AP never woke). Exactly one writer per
//!   disjoint output range; forward progress is guaranteed even if an AP never
//!   polls (the BSP steals every un-claimed partition).
//! - **Jobs are BSP-serialized.** A non-blocking `PAR_BUSY` try-guard makes a
//!   second concurrent `run_*` fall back to single-threaded rather than clobber a
//!   live job's slots. This is a try-lock, never held across another lock and
//!   never blocked upon, so it adds no ordered lock (I-5) and is not an
//!   AI-governance lock (I-14): the join path touches none of
//!   INFLIGHT/RECENCY/ARENA/reservation/TABLE.
//!
//! ## Kill-switch
//!
//! The whole module is `#[cfg(feature = "threaded-matmul")]` and `THREAD_COUNT`
//! defaults to 1, so an enabled-but-unconfigured build is byte-for-byte
//! single-threaded: `should_parallelize` returns false and `forward_logic` runs
//! the whole-column-range kernel call, exactly as the pre-OT-34 path. A caller
//! opts into parallelism by raising `set_thread_count`, clamped to the online CPU
//! count. Feature OFF removes this module and the `ap_poll` hook entirely.
#![allow(dead_code)] // control API (set/thread_count) is consumed by the OT-34
// determinism scenario + the bare-metal enabler; a plain `--features
// threaded-matmul` build without a driver leaves it unused, same rationale as
// `forward_logic`'s module-level allow.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::forward_logic::{
    ForwardLayerError, MatmulPartitionKind, column_matmul_q8_0_part, column_matmul_q8k_part,
    column_partition,
};
#[cfg(feature = "par-elementwise")]
use super::ops::GqaAttentionParams;
use super::ops::qk_matmul::{BlockQ8K, BlockQ8Zero};
use super::par_matmul_logic::{BarrierAction, barrier_action};
use crate::arch::x86_64::per_cpu;

/// Maximum partitions == the per-CPU table width. Slot `t` is served by CPU `t`
/// (BSP is CPU 0 and always serves partition 0 inline).
const MAX_PARTS: usize = per_cpu::MAX_CPUS;

/// Below this output-column width the fork-join publish + barrier overhead
/// outweighs the parallel win, so the matmul stays single-threaded. Determinism
/// is unaffected by the threshold (single-threaded and partitioned produce
/// identical bytes) — it is a pure performance knob. The FFN gate/up/down
/// (`intermediate_size`, thousands) and the lm_head (vocab, ~150k) — the matmuls
/// worth threading — clear it; the tiny K/V projections (128) stay serial.
const PAR_MIN_COLS: usize = 512;

/// Bounded spin before the BSP gives up waiting on a claimed-but-unfinished
/// partition. A claimed partition is pure compute with no lock, so it always
/// finishes; the bound only prevents an unbounded hang if an AP is lost, in
/// which case the job fails closed (never silently wrong).
const BARRIER_LIMIT: u64 = 8_000_000_000;

/// Requested worker count (number of partitions). Default 1 ⇒ single-threaded
/// even with the feature enabled — the conservative kill-switch default. A caller
/// raises it via [`set_thread_count`]; it is always re-clamped to the online CPU
/// count at use.
static THREAD_COUNT: AtomicUsize = AtomicUsize::new(1);

/// Non-blocking re-entrancy guard: only one fork-join job runs at a time. A
/// second concurrent caller falls back to single-threaded. NOT a governance lock;
/// never blocked upon.
static PAR_BUSY: AtomicBool = AtomicBool::new(false);

/// Spins the BSP waits for an AP to claim its partition before stealing it back.
/// On real hardware an idle-polling AP claims within a few hundred spins, so this
/// grace is never exhausted (zero throughput cost); it only matters if an AP is
/// momentarily absent (then the BSP waits this long, ~sub-millisecond, before
/// doing the work itself). It also gives QEMU's round-robin TCG time to schedule
/// an AP vCPU so the cross-core path is genuinely exercised under the gate.
const STEAL_GRACE_SPINS: u64 = 2_000_000;

/// `matmul-steal-grace-time`: the grace as time since the job was published,
/// so it no longer depends on how long a `PAUSE` takes (up to ~140 cycles on
/// recent Intel cores, which made the 2M-spin grace about 0.1 s per job while
/// an AP was busy). An idle AP polls within microseconds; 50 µs leaves it a
/// wide margin and caps what a busy AP costs a job. Uncalibrated TSC ⇒ the
/// spin grace above.
#[cfg(feature = "matmul-steal-grace-time")]
const STEAL_GRACE_US: u64 = 50;

#[cfg(feature = "matmul-steal-grace-time")]
#[inline]
fn rdtsc() -> u64 {
    // SAFETY: RDTSC is unprivileged and side-effect free.
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// The job's publish stamp, taken after the `pending` Release stores are
/// globally visible (Chad, relay #52 item 4). A Release store orders it
/// against later memory accesses, not against RDTSC, so a bare RDTSC could
/// read the clock before an AP can see the job and start the grace early.
/// SDM vol. 2B, RDTSC: `MFENCE; LFENCE` before it waits for all prior stores
/// to be globally visible and all prior instructions to complete. Once per
/// job; the per-poll reads stay plain (a late read only lengthens the grace).
#[cfg(feature = "matmul-steal-grace-time")]
#[inline]
fn rdtsc_after_stores() -> u64 {
    let lo: u32;
    let hi: u32;
    // SAFETY: fences and RDTSC only; reads the TSC into EDX:EAX, no memory or
    // stack effect.
    unsafe {
        core::arch::asm!(
            "mfence",
            "lfence",
            "rdtsc",
            out("eax") lo,
            out("edx") hi,
            options(nostack, preserves_flags)
        );
    }
    ((hi as u64) << 32) | lo as u64
}

/// Test hook (`neo-test-threaded-matmul-parity-grace-time` only): the AP slot
/// `ap_poll` leaves alone, so the BSP must steal it after the grace; and the
/// smallest elapsed-since-publish value a successful steal was DECIDED on
/// (the value `past_steal_grace` saw, recorded before compute, so a long
/// stolen computation cannot hide an early claim; Chad, relay #53 R6).
/// `u64::MAX` while no steal has happened since the last reset.
#[cfg(feature = "neo-test-threaded-matmul-parity-grace-time")]
static HOLD_SLOT: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "neo-test-threaded-matmul-parity-grace-time")]
static MIN_STEAL_DECISION_CYCLES: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(u64::MAX);

#[cfg(feature = "neo-test-threaded-matmul-parity-grace-time")]
pub fn hold_slot_for_test(slot: usize) {
    HOLD_SLOT.store(slot, Ordering::Relaxed);
}

/// The smallest decision-time elapsed cycles over every successful BSP steal
/// since [`reset_steal_decisions_for_test`] (`u64::MAX`: none).
#[cfg(feature = "neo-test-threaded-matmul-parity-grace-time")]
pub fn min_steal_decision_cycles() -> u64 {
    MIN_STEAL_DECISION_CYCLES.load(Ordering::Relaxed)
}

#[cfg(feature = "neo-test-threaded-matmul-parity-grace-time")]
pub fn reset_steal_decisions_for_test() {
    MIN_STEAL_DECISION_CYCLES.store(u64::MAX, Ordering::Relaxed);
}

/// Test overrides (`neo-test-threaded-matmul-parity-grace-time` only): a fixed
/// TSC rate and grace, so the scenario takes the calibrated path whatever the
/// runner's LAPIC calibration did (hermetic), with a grace far longer than the
/// BSP's own partition under TCG (so a truncated threshold is caught).
#[cfg(feature = "neo-test-threaded-matmul-parity-grace-time")]
static TEST_TSC_HZ: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
#[cfg(feature = "neo-test-threaded-matmul-parity-grace-time")]
static TEST_GRACE_US: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

#[cfg(feature = "neo-test-threaded-matmul-parity-grace-time")]
pub fn set_grace_for_test(tsc_hz: u64, grace_us: u64) {
    TEST_TSC_HZ.store(tsc_hz, Ordering::Relaxed);
    TEST_GRACE_US.store(grace_us, Ordering::Relaxed);
}

/// The TSC rate and grace the barrier uses: the calibrated rate and
/// [`STEAL_GRACE_US`], or the test overrides when set.
#[cfg(feature = "matmul-steal-grace-time")]
#[inline]
fn grace_inputs() -> (u64, u64) {
    #[cfg(feature = "neo-test-threaded-matmul-parity-grace-time")]
    {
        let hz = TEST_TSC_HZ.load(Ordering::Relaxed);
        if hz != 0 {
            // Control (`...-grace-time-control`): the grace times 0, so the
            // BSP steals on its first look and the scenario must FAIL
            // `stole_before_grace`.
            if cfg!(feature = "neo-test-threaded-matmul-parity-grace-time-control") {
                return (hz, 0);
            }
            return (hz, TEST_GRACE_US.load(Ordering::Relaxed));
        }
    }
    (crate::arch::x86_64::lapic_timer::tsc_hz(), STEAL_GRACE_US)
}

/// The steal grace in cycles, or None when the TSC rate is unknown (the spin
/// grace applies).
#[cfg(feature = "matmul-steal-grace-time")]
pub fn steal_grace_cycles() -> Option<u64> {
    match grace_inputs() {
        (0, _) => None,
        (hz, us) => Some(super::par_matmul_logic::steal_grace_cycles(hz, us)),
    }
}

/// Diagnostic counters (reported, NOT gated — a hermetic gate verdicts on the
/// deterministic byte-identity, never on which core happened to run a partition
/// under a nondeterministic scheduler). `ap_partitions_done` counts partitions an
/// AP executed; `bsp_steals` counts partitions the BSP stole back.
static AP_PARTITIONS_DONE: AtomicUsize = AtomicUsize::new(0);
static BSP_STEALS: AtomicUsize = AtomicUsize::new(0);

/// NEGATIVE CONTROL (test-only, `neo-test-threaded-matmul-parity`). When set, the
/// fork-join perturbs each partition's output by a partition-boundary-dependent
/// epsilon, so the assembled result depends on HOW the output columns were
/// partitioned — i.e. on the thread count. This is the exact non-determinism the
/// parity KAT must catch: the single-threaded path (one whole-range partition,
/// `col_start == 0`) stays unperturbed, but any multi-partition run perturbs its
/// `col_start > 0` partitions, so threaded ≠ single. Proves the parity compare is
/// genuinely falsifiable (a "usually matches" reduction would red here). It is
/// `#[cfg]`-gated to the parity-test feature, so it is **never** compiled into a
/// production or non-test build — `run_disjoint` is byte-for-bit unchanged there.
#[cfg(feature = "neo-test-threaded-matmul-parity")]
static PERTURB: AtomicBool = AtomicBool::new(false);

/// Arm/disarm the parity negative control (test-only). See [`PERTURB`].
#[cfg(feature = "neo-test-threaded-matmul-parity")]
pub fn set_perturb(on: bool) {
    PERTURB.store(on, Ordering::Relaxed);
}

/// Partitions executed by an AP since the last [`reset_counters`].
pub fn ap_partitions_done() -> usize {
    AP_PARTITIONS_DONE.load(Ordering::Relaxed)
}

/// Partitions the BSP stole back (the assigned AP never claimed in time).
pub fn bsp_steals() -> usize {
    BSP_STEALS.load(Ordering::Relaxed)
}

/// Zero the diagnostic counters (a measurement harness calls this before a run).
pub fn reset_counters() {
    AP_PARTITIONS_DONE.store(0, Ordering::Relaxed);
    BSP_STEALS.store(0, Ordering::Relaxed);
}

/// What a job computes. Pointers are stored as `usize` so the slot is trivially
/// `Sync`; the executor reconstructs shared slices under the lifetime guarantee
/// that the BSP blocks until the job completes.
#[derive(Copy, Clone)]
enum PartitionJob {
    /// A single-row quantized matmul; the LHS is one quantized-block row, tagged
    /// by block family. Units are output columns.
    Matmul {
        kind: MatmulPartitionKind,
        is_q8k: bool,
        lhs_ptr: usize,
        lhs_len: usize,
        rhs_ptr: usize,
        rhs_len: usize,
        cols: usize,
        shared: usize,
    },
    /// Lever 2: the SwiGLU element-wise step over `len` elements of `gate` and
    /// `up`. Units are elements; each depends only on its own index.
    #[cfg(feature = "par-elementwise")]
    Swiglu {
        gate_ptr: usize,
        up_ptr: usize,
        len: usize,
    },
    /// Lever 2: the cached decode step's attention over the KV cache in place
    /// (`ops::gqa_causal_attention_last_strided_heads`). Units are q heads; each
    /// writes its `head_dim` output values. Partition `t` uses its own `seq_len`
    /// values of the BSP-allocated `scratch` for the softmax weights.
    #[cfg(feature = "par-elementwise")]
    AttnHeads {
        q_ptr: usize,
        q_len: usize,
        key_ptr: usize,
        key_len: usize,
        value_ptr: usize,
        value_len: usize,
        head_stride: usize,
        params: GqaAttentionParams,
        scratch_ptr: usize,
        scratch_len: usize,
    },
}

impl PartitionJob {
    /// Output values per unit.
    fn out_stride(&self) -> usize {
        match *self {
            Self::Matmul { .. } => 1,
            #[cfg(feature = "par-elementwise")]
            Self::Swiglu { .. } => 1,
            #[cfg(feature = "par-elementwise")]
            Self::AttnHeads { params, .. } => params.head_dim as usize,
        }
    }
}

/// One partition: the job, the output base, this partition's index `part`, and
/// its unit range `[col_start, col_end)` (columns for a matmul, elements for
/// SwiGLU, q heads for attention). Unit `u` writes the `out_stride` output
/// values starting at `u * out_stride`.
#[derive(Copy, Clone)]
struct PartitionWork {
    job: PartitionJob,
    out_ptr: usize,
    part: usize,
    col_start: usize,
    col_end: usize,
}

impl PartitionWork {
    const EMPTY: Self = Self {
        job: PartitionJob::Matmul {
            kind: MatmulPartitionKind::Q4K,
            is_q8k: true,
            lhs_ptr: 0,
            lhs_len: 0,
            rhs_ptr: 0,
            rhs_len: 0,
            cols: 0,
            shared: 0,
        },
        out_ptr: 0,
        part: 0,
        col_start: 0,
        col_end: 0,
    };
}

/// A per-CPU work slot. The BSP sets `work` then `pending=true` (Release); the
/// assigned AP (or the BSP steal-back) claims it via `CAS(pending: true->false)`,
/// executes, then sets `done=true` (Release). Only the single CAS winner reads
/// `work`, so no lock is needed around it.
struct ApSlot {
    pending: AtomicBool,
    done: AtomicBool,
    work: UnsafeCell<PartitionWork>,
}

impl ApSlot {
    const fn new() -> Self {
        Self {
            pending: AtomicBool::new(false),
            done: AtomicBool::new(false),
            work: UnsafeCell::new(PartitionWork::EMPTY),
        }
    }
}

/// SAFETY: the `UnsafeCell<PartitionWork>` is synchronized by `pending`/`done`
/// release-acquire and the single-winner claim CAS: the BSP writes `work` only
/// before publishing `pending=true`, and only the CAS winner reads it. Jobs are
/// BSP-serialized by `PAR_BUSY`, so no two producers write a slot concurrently.
/// The raw `out`/lhs/rhs pointers `work` carries stay valid for as long as any
/// executor may dereference them because `run_job` never returns (freeing the
/// borrows) while a claimed partition is unfinished — it drains every claimed slot
/// to `done` or fail-stops (see the join loop's F1 discipline). This premise holds
/// on EVERY path, including the hard-limit backstop.
unsafe impl Sync for ApSlot {}

/// Per-CPU work slots. Index 0 is unused (the BSP serves partition 0 inline).
#[allow(clippy::declare_interior_mutable_const)]
static AP_SLOTS: [ApSlot; MAX_PARTS] = [const { ApSlot::new() }; MAX_PARTS];

/// Set the requested worker count (partitions). Clamped to `[1, MAX_PARTS]` here
/// and re-clamped to the online CPU count at use. `n <= 1` ⇒ single-threaded.
pub fn set_thread_count(n: usize) {
    THREAD_COUNT.store(n.clamp(1, MAX_PARTS), Ordering::Relaxed);
}

/// The current requested worker count (unclamped to online CPUs).
pub fn thread_count() -> usize {
    THREAD_COUNT.load(Ordering::Relaxed)
}

/// Effective partitions for a matmul of `cols` output columns: the requested
/// count, clamped to online CPUs and to `cols` (never more partitions than
/// columns).
fn effective_parts(cols: usize) -> usize {
    let want = THREAD_COUNT.load(Ordering::Relaxed).clamp(1, MAX_PARTS);
    let online = crate::arch::x86_64::smp::online_cpu_count().clamp(1, MAX_PARTS);
    want.min(online).min(cols.max(1))
}

/// Whether `forward_logic` should fork this matmul across cores. False keeps the
/// current single-threaded whole-column kernel call (the default and the
/// feature-off behavior).
pub fn should_parallelize(cols: usize) -> bool {
    cols >= PAR_MIN_COLS && effective_parts(cols) > 1
}

/// Fork-join a single-row Q8_K matmul (`Q4K`/`Q6K` RHS) across cores.
pub fn run_matmul_q8k(
    kind: MatmulPartitionKind,
    lhs_q8: &[BlockQ8K],
    rhs: &[u8],
    cols: usize,
    shared: usize,
    out: &mut [f32],
) -> Result<(), ForwardLayerError> {
    run_job(
        PartitionJob::Matmul {
            kind,
            is_q8k: true,
            lhs_ptr: lhs_q8.as_ptr() as usize,
            lhs_len: lhs_q8.len(),
            rhs_ptr: rhs.as_ptr() as usize,
            rhs_len: rhs.len(),
            cols,
            shared,
        },
        cols,
        effective_parts(cols),
        out,
    )
}

/// Fork-join a single-row Q8_0 matmul (`Q5_0`/`Q8_0` RHS) across cores.
pub fn run_matmul_q8_0(
    kind: MatmulPartitionKind,
    lhs_q8: &[BlockQ8Zero],
    rhs: &[u8],
    cols: usize,
    shared: usize,
    out: &mut [f32],
) -> Result<(), ForwardLayerError> {
    run_job(
        PartitionJob::Matmul {
            kind,
            is_q8k: false,
            lhs_ptr: lhs_q8.as_ptr() as usize,
            lhs_len: lhs_q8.len(),
            rhs_ptr: rhs.as_ptr() as usize,
            rhs_len: rhs.len(),
            cols,
            shared,
        },
        cols,
        effective_parts(cols),
        out,
    )
}

/// Lever 2: fork-join the SwiGLU element-wise step across cores, split into
/// contiguous element ranges. `ops::swiglu_f32` per range; no value crosses a
/// range boundary, so the bytes are the single-threaded call's at any count.
/// The caller checks [`should_parallelize`] on the element count.
#[cfg(feature = "par-elementwise")]
pub fn run_swiglu(gate: &[f32], up: &[f32], out: &mut [f32]) -> Result<(), ForwardLayerError> {
    if gate.len() != up.len() || out.len() != gate.len() {
        return Err(ForwardLayerError::Shape);
    }
    run_job(
        PartitionJob::Swiglu {
            gate_ptr: gate.as_ptr() as usize,
            up_ptr: up.as_ptr() as usize,
            len: gate.len(),
        },
        gate.len(),
        effective_parts(gate.len()),
        out,
    )
}

/// Below this much attention work (`n_q_heads * seq_len * head_dim`, one
/// multiply-add each for the scores and again for the outputs) the cached decode
/// step stays on one core. A pure performance knob, like [`PAR_MIN_COLS`]: any
/// split gives the same bytes. From A8 (Zenbook, 2026-10-26): a fork-join costs
/// about 44 us of barrier per job (`bar=10%` over 4,210 jobs), and after lever 1c
/// the step costs about 8 us per position at 0.5B, so a 4-way split pays from
/// about 16 positions: 14 x 16 x 64 = 14,336, rounded up.
#[cfg(all(
    feature = "par-elementwise",
    not(feature = "neo-test-par-attention-every-step")
))]
const PAR_MIN_ATTN_WORK: usize = 16_384;

/// Test only: every cached decode step forks (given more than one part), so the
/// forward scenarios' short contexts run the split against their oracles.
#[cfg(feature = "neo-test-par-attention-every-step")]
const PAR_MIN_ATTN_WORK: usize = 1;

/// Test only: forked attention calls since boot, so the forward scenarios show the
/// split ran.
#[cfg(feature = "neo-test-par-attention-every-step")]
static ATTN_JOBS: AtomicUsize = AtomicUsize::new(0);

/// Forked attention calls since boot (test only).
#[cfg(feature = "neo-test-par-attention-every-step")]
pub fn attention_jobs() -> usize {
    ATTN_JOBS.load(Ordering::Relaxed)
}

/// Whether the cached decode step's attention should fork across cores.
#[cfg(feature = "par-elementwise")]
pub fn should_parallelize_attention(params: GqaAttentionParams) -> bool {
    let work = (params.n_q_heads as usize)
        .saturating_mul(params.seq_len as usize)
        .saturating_mul(params.head_dim as usize);
    work >= PAR_MIN_ATTN_WORK && effective_parts(params.n_q_heads as usize) > 1
}

/// Lever 2: fork-join the cached decode step's attention across cores by q-head
/// range (`ops::gqa_causal_attention_last_strided_heads`). Each head's output and
/// summation orders are its own, so the bytes are the single-call op's at any
/// count. The BSP allocates every partition's softmax scratch up front, so a
/// partition allocates nothing.
#[cfg(feature = "par-elementwise")]
pub fn run_attention(
    q_last: &[f32],
    key: &[f32],
    value: &[f32],
    head_stride: usize,
    params: GqaAttentionParams,
    out: &mut [f32],
) -> Result<(), ForwardLayerError> {
    let n_q_heads = usize::try_from(params.n_q_heads).map_err(|_| ForwardLayerError::Shape)?;
    let seq_len = usize::try_from(params.seq_len).map_err(|_| ForwardLayerError::Shape)?;
    let parts = effective_parts(n_q_heads);
    #[cfg(feature = "neo-test-par-attention-every-step")]
    ATTN_JOBS.fetch_add(1, Ordering::Relaxed);
    let mut scratch =
        alloc::vec![0u32; parts.checked_mul(seq_len).ok_or(ForwardLayerError::Shape)?];
    run_job(
        PartitionJob::AttnHeads {
            q_ptr: q_last.as_ptr() as usize,
            q_len: q_last.len(),
            key_ptr: key.as_ptr() as usize,
            key_len: key.len(),
            value_ptr: value.as_ptr() as usize,
            value_len: value.len(),
            head_stride,
            params,
            scratch_ptr: scratch.as_mut_ptr() as usize,
            scratch_len: scratch.len(),
        },
        n_q_heads,
        parts,
        out,
    )
}

/// Execute units `[col_start, col_end)` into `out_sub` (length ==
/// `col_end - col_start`), reconstructing the job's shared input slices from its
/// pointers.
///
/// SAFETY: the caller guarantees the job's input pointers/lengths name live,
/// immutable slices for the whole job (the BSP blocks on the barrier until every
/// partition is done), and that `out_sub` is the caller's exclusive, disjoint
/// slice for exactly these units.
unsafe fn execute(work: &PartitionWork, out_sub: &mut [f32]) -> Result<(), ForwardLayerError> {
    match work.job {
        PartitionJob::Matmul {
            kind,
            is_q8k,
            lhs_ptr,
            lhs_len,
            rhs_ptr,
            rhs_len,
            cols,
            shared,
        } => {
            // SAFETY: pointers/lens are live for the job duration (see fn contract).
            let rhs = unsafe { core::slice::from_raw_parts(rhs_ptr as *const u8, rhs_len) };
            if is_q8k {
                // SAFETY: BlockQ8K LHS produced by the caller's `quantize_row_q8_k`.
                let lhs =
                    unsafe { core::slice::from_raw_parts(lhs_ptr as *const BlockQ8K, lhs_len) };
                column_matmul_q8k_part(
                    kind,
                    lhs,
                    rhs,
                    cols,
                    shared,
                    work.col_start,
                    work.col_end,
                    out_sub,
                )
            } else {
                // SAFETY: BlockQ8Zero LHS produced by the caller's `quantize_row_q8_0`.
                let lhs =
                    unsafe { core::slice::from_raw_parts(lhs_ptr as *const BlockQ8Zero, lhs_len) };
                column_matmul_q8_0_part(
                    kind,
                    lhs,
                    rhs,
                    cols,
                    shared,
                    work.col_start,
                    work.col_end,
                    out_sub,
                )
            }
        }
        #[cfg(feature = "par-elementwise")]
        PartitionJob::Swiglu {
            gate_ptr,
            up_ptr,
            len,
        } => {
            if work.col_end > len {
                return Err(ForwardLayerError::Shape);
            }
            // SAFETY: `gate`/`up` are the caller's live `len`-element slices for the
            // job duration (see fn contract); this partition reads only its range.
            let gate = unsafe { core::slice::from_raw_parts(gate_ptr as *const f32, len) };
            let up = unsafe { core::slice::from_raw_parts(up_ptr as *const f32, len) };
            let range = work.col_start..work.col_end;
            Ok(super::ops::swiglu_f32(
                &gate[range.clone()],
                &up[range],
                out_sub,
            )?)
        }
        #[cfg(feature = "par-elementwise")]
        PartitionJob::AttnHeads {
            q_ptr,
            q_len,
            key_ptr,
            key_len,
            value_ptr,
            value_len,
            head_stride,
            params,
            scratch_ptr,
            scratch_len,
        } => {
            let seq_len = params.seq_len as usize;
            let scratch_start = work
                .part
                .checked_mul(seq_len)
                .ok_or(ForwardLayerError::Shape)?;
            if scratch_start
                .checked_add(seq_len)
                .is_none_or(|end| end > scratch_len)
            {
                return Err(ForwardLayerError::Shape);
            }
            // SAFETY: the query and the cache's K/V storage are the caller's live
            // slices for the job duration (see fn contract), read only.
            let q_last = unsafe { core::slice::from_raw_parts(q_ptr as *const f32, q_len) };
            let key = unsafe { core::slice::from_raw_parts(key_ptr as *const f32, key_len) };
            let value = unsafe { core::slice::from_raw_parts(value_ptr as *const f32, value_len) };
            // SAFETY: partition `part`'s `seq_len` scratch values, checked in bounds
            // above; partitions have distinct `part`, so no two alias, and the BSP
            // holds the scratch allocation until every partition is done.
            let weights = unsafe {
                core::slice::from_raw_parts_mut(
                    (scratch_ptr as *mut u32).add(scratch_start),
                    seq_len,
                )
            };
            Ok(super::ops::gqa_causal_attention_last_strided_heads(
                q_last,
                key,
                value,
                head_stride,
                params,
                work.col_start..work.col_end,
                weights,
                out_sub,
            )?)
        }
    }
}

/// Compute a partition's disjoint output slice `out_base[col_start..col_end)` and
/// run it. `out_base` is the raw base pointer of the full output; the caller
/// guarantees each partition's `[col_start,col_end)` is disjoint from every other
/// live partition, so the derived `&mut` never aliases another.
///
/// SAFETY: see [`execute`]; additionally `out_base.add(col_start)..+len` must be
/// in-bounds of the output and disjoint from all other partitions in flight.
unsafe fn run_disjoint(work: &PartitionWork, out_base: *mut f32) -> Result<(), ForwardLayerError> {
    if work.col_start >= work.col_end {
        return Ok(());
    }
    let stride = work.job.out_stride();
    let len = (work.col_end - work.col_start) * stride;
    // SAFETY: disjoint, in-bounds sub-slice for this partition's units only.
    let out_sub =
        unsafe { core::slice::from_raw_parts_mut(out_base.add(work.col_start * stride), len) };
    let r = unsafe { execute(work, out_sub) };
    // NEGATIVE CONTROL (test-only): make the result depend on the partition
    // boundary so a multi-partition (threaded) run diverges from the single whole-
    // range run. `col_start == 0` (single-threaded, and partition 0) is untouched;
    // `col_start > 0` partitions get a boundary-keyed epsilon. Vanishes entirely in
    // non-test builds (the production `run_disjoint` is byte-for-bit unchanged).
    #[cfg(feature = "neo-test-threaded-matmul-parity")]
    if r.is_ok() && work.col_start > 0 && PERTURB.load(Ordering::Relaxed) {
        // A single low-mantissa-bit flip on every `col_start > 0` column: a
        // guaranteed but *sub-ULP-subtle* divergence — invisible to a greedy
        // argmax, caught ONLY by the byte-exact parity compare. That is the whole
        // point: it models a reduction that "usually matches," and proves the KAT
        // reds on exactly that (a "close enough" parity check would pass here).
        for v in out_sub.iter_mut() {
            *v = f32::from_bits(v.to_bits() ^ 1);
        }
    }
    r
}

/// Fork-join `job` over `cols` units, split `parts` ways (the caller's
/// [`effective_parts`]), into `out` (`out_stride` values per unit, so
/// `out.len() == cols * out_stride`).
fn run_job(
    job: PartitionJob,
    cols: usize,
    parts: usize,
    out: &mut [f32],
) -> Result<(), ForwardLayerError> {
    if cols.checked_mul(job.out_stride()) != Some(out.len()) {
        return Err(ForwardLayerError::Shape);
    }
    let out_ptr = out.as_mut_ptr();

    let mk = |part: usize, col_start: usize, col_end: usize| PartitionWork {
        job,
        out_ptr: out_ptr as usize,
        part,
        col_start,
        col_end,
    };

    // Single-threaded fallback: whole-column range inline (bit-for-bit the
    // pre-OT-34 kernel call). Also taken if a concurrent job holds PAR_BUSY.
    if parts <= 1
        || PAR_BUSY
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        let work = mk(0, 0, cols);
        // SAFETY: whole output, single writer (this thread), inputs live here.
        return unsafe { run_disjoint(&work, out_ptr) };
    }

    #[cfg(feature = "freeze-watchdog")]
    crate::watchdog::note_par_job();
    // ---- publish AP partitions 1..parts ----
    for (t, slot) in AP_SLOTS.iter().enumerate().take(parts).skip(1) {
        let (start, end) = column_partition(cols, parts, t);
        slot.done.store(false, Ordering::Relaxed);
        // SAFETY: this slot's previous job is fully drained (we cleared pending
        // and observed done at the end of the previous run_job), and jobs are
        // serialized by PAR_BUSY, so no concurrent producer/consumer touches it.
        unsafe {
            *slot.work.get() = mk(t, start, end);
        }
        slot.pending.store(true, Ordering::Release);
    }
    #[cfg(feature = "matmul-steal-grace-time")]
    let published = rdtsc_after_stores();

    // ---- BSP executes partition 0 (gives APs time to claim theirs) ----
    let mut result: Result<(), ForwardLayerError> = {
        let (s0, e0) = column_partition(cols, parts, 0);
        let work = mk(0, s0, e0);
        #[cfg(feature = "prefill-perf")]
        let stamp = crate::ai::prefill_perf::partition_begin();
        // SAFETY: partition 0's columns are disjoint from all AP partitions.
        let r = unsafe { run_disjoint(&work, out_ptr) };
        #[cfg(feature = "prefill-perf")]
        crate::ai::prefill_perf::partition_end(stamp);
        r
    };

    #[cfg(feature = "prefill-perf")]
    let perf_t0 = crate::ai::prefill_perf::tsc();
    // ---- barrier + steal-back for partitions 1..parts ----
    //
    // F1 (AAG bounce) fail-STOP discipline: `run_job` MUST NOT return while a
    // partition an AP has *claimed* is still unfinished — that would drop the
    // `&mut out` borrow, let the caller free/reuse `out`, and leave the live
    // executor writing freed memory (UAF). So the loop only ever exits a slot via
    // `done` (drained) or `Halt` (panic). The hard limit may skip/steal ONLY an
    // *unclaimed* slot; a claimed-but-wedged slot halts. Because the loop drains
    // every claimed slot before proceeding, `run_job` never returns with a
    // live executor outstanding — which also closes the cascade (no abandoned AP
    // can survive into a later job to trip its barrier with a stale `done`).
    for slot in AP_SLOTS.iter().take(parts).skip(1) {
        // The slot's index, from its address in `AP_SLOTS`.
        #[cfg(feature = "freeze-watchdog")]
        crate::watchdog::note_par_wait(
            ((slot as *const _ as usize - AP_SLOTS.as_ptr() as usize)
                / core::mem::size_of_val(slot)) as u32,
        );
        let mut spins: u64 = 0;
        loop {
            let done = slot.done.load(Ordering::Acquire);
            let pending = slot.pending.load(Ordering::Acquire);
            #[cfg(all(
                feature = "matmul-steal-grace-time",
                not(feature = "neo-test-threaded-matmul-parity-grace-time")
            ))]
            let past_grace = {
                let (hz, grace_us) = grace_inputs();
                super::par_matmul_logic::past_steal_grace(
                    rdtsc().wrapping_sub(published),
                    hz,
                    spins,
                    grace_us,
                    STEAL_GRACE_SPINS,
                )
            };
            // The grace scenario: the same decision, keeping the elapsed time
            // it was made on, which a successful steal records.
            #[cfg(feature = "neo-test-threaded-matmul-parity-grace-time")]
            let elapsed = rdtsc().wrapping_sub(published);
            #[cfg(feature = "neo-test-threaded-matmul-parity-grace-time")]
            let past_grace = {
                let (hz, grace_us) = grace_inputs();
                super::par_matmul_logic::past_steal_grace(
                    elapsed,
                    hz,
                    spins,
                    grace_us,
                    STEAL_GRACE_SPINS,
                )
            };
            #[cfg(not(feature = "matmul-steal-grace-time"))]
            let past_grace = spins >= STEAL_GRACE_SPINS;
            match barrier_action(done, pending, past_grace, spins >= BARRIER_LIMIT) {
                BarrierAction::Done => break,
                BarrierAction::Steal => {
                    // The slot is unclaimed and past the grace window. Win the claim
                    // and compute it inline. If an AP claims it first, the CAS fails
                    // and we loop — it is now a *claimed* slot (Spin/Halt), never
                    // re-stolen (single-winner). The grace gives an idle AP time to
                    // claim on real hardware, and lets TCG schedule an AP vCPU.
                    if slot
                        .pending
                        .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        // The decision this claim was made on, before compute.
                        #[cfg(feature = "neo-test-threaded-matmul-parity-grace-time")]
                        MIN_STEAL_DECISION_CYCLES.fetch_min(elapsed, Ordering::Relaxed);
                        // SAFETY: we won the single claim for this slot; `work` is
                        // stable, and no other executor writes this slot's columns.
                        let work = unsafe { *slot.work.get() };
                        #[cfg(feature = "prefill-perf")]
                        let stamp = crate::ai::prefill_perf::partition_begin();
                        let r = unsafe { run_disjoint(&work, out_ptr) };
                        #[cfg(feature = "prefill-perf")]
                        crate::ai::prefill_perf::partition_end(stamp);
                        if r.is_err() {
                            result = r;
                        }
                        BSP_STEALS.fetch_add(1, Ordering::Relaxed);
                        slot.done.store(true, Ordering::Release);
                        break;
                    }
                }
                BarrierAction::Halt => {
                    // F1 guard: do NOT rewire this arm to a `break`/return — see
                    // par_matmul_logic.rs. The host KAT proves the *policy* (Halt,
                    // never Steal/return) but cannot see this kernel-side wiring;
                    // a return here reopens the UAF the F1 fix closed.
                    // FAIL-STOP. A *claimed* slot (an AP won its claim CAS, so
                    // `pending == false`) is still not `done` after BARRIER_LIMIT
                    // (8e9) BSP spins: an executor wedged mid pure-compute. This is
                    // an impossible ring-0 state — the AP idle loop runs with
                    // interrupts ENABLED, so a bounded ISR delay between claim and
                    // `done` is a legal interleaving, but a multi-second stall on a
                    // microsecond partition is not. We must NOT `break`/return: that
                    // frees `out` under the still-live executor (UAF) and a late
                    // `done` store could trip a later job's barrier (cascade).
                    // Halting keeps `out` on the frozen stack — nothing is freed
                    // under the wedged executor, and no subsequent job runs.
                    panic!(
                        "par_matmul: AP partition wedged past BARRIER_LIMIT (claimed but not \
                         retired) — fail-stop to avoid freeing shared output under a live executor"
                    );
                }
                BarrierAction::Spin => {}
            }
            core::hint::spin_loop();
            spins += 1;
        }
        // Quiescent for the next job: `pending` is already false for a slot the AP
        // claimed or the BSP stole; this is a belt-and-suspenders clear.
        slot.pending.store(false, Ordering::Relaxed);
    }

    #[cfg(feature = "freeze-watchdog")]
    crate::watchdog::note_par_wait(0);
    #[cfg(feature = "prefill-perf")]
    crate::ai::prefill_perf::par_job(crate::ai::prefill_perf::tsc().wrapping_sub(perf_t0));
    PAR_BUSY.store(false, Ordering::Release);
    result
}

/// Called by each AP from its idle bootstrap loop. If a fork-join job assigned a
/// partition to this CPU, claim and execute it exactly once. A no-op when no job
/// is pending for this CPU (the common case) — a single relaxed-then-acquire load.
pub fn ap_poll(cpu_id: u32) {
    let t = cpu_id as usize;
    if t == 0 || t >= MAX_PARTS {
        return;
    }
    #[cfg(feature = "neo-test-threaded-matmul-parity-grace-time")]
    if HOLD_SLOT.load(Ordering::Relaxed) == t {
        return;
    }
    let slot = &AP_SLOTS[t];
    if !slot.pending.load(Ordering::Acquire) {
        return;
    }
    // Claim exactly once. If the BSP already stole it, the CAS fails and we skip.
    if slot
        .pending
        .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    // SAFETY: we are the sole claim winner; `work` was published before `pending`
    // (Release) and is stable until we set `done`. Inputs (`out`, lhs, rhs) are
    // live on EVERY path because `run_job` never returns while a claimed
    // partition is unfinished: its join loop drains every claimed slot to `done`
    // or, on the impossible wedge, fail-stops (halts) — it never `break`s/returns
    // out of the barrier with this slot claimed-but-not-`done` (the F1 fix). So the
    // `&mut out` borrow that backs `out_ptr` outlives this write. Our output slice
    // is this partition's disjoint column range.
    let work = unsafe { *slot.work.get() };
    let out_base = work.out_ptr as *mut f32;
    #[cfg(feature = "prefill-perf")]
    let stamp = crate::ai::prefill_perf::partition_begin();
    let _ = unsafe { run_disjoint(&work, out_base) };
    #[cfg(feature = "prefill-perf")]
    crate::ai::prefill_perf::partition_end(stamp);
    AP_PARTITIONS_DONE.fetch_add(1, Ordering::Relaxed);
    slot.done.store(true, Ordering::Release);
}
