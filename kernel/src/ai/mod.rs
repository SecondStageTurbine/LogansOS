// SPDX-License-Identifier: MPL-2.0
//! The AI subsystem — tensors, models, inference, and the residency policy
//! arc (ANSPEC-001 §9). This module list IS the map (BUILD-C1, the
//! fresh-reader contract): read it top-to-bottom and you have the layering.
//!
//! **Tier 1 — pure, kernel-free, host-testable policy** (path-`mod`'d into
//! `tools/ai-ops-tests`; the cheap-to-maintain tier where decisions live):
//! - `tensor_logic` / `model_logic` — shape/dtype/layout arithmetic, `.neom` graph rules
//! - `gguf_logic` / `gguf_dequant_logic` / `tokenizer_logic` — default-off GGUF
//!   importer, dequant seam, and BPE parsing
//! - `forward_logic` — default-off Qwen layer scaffold: dequant-fed projections,
//!   RoPE, cache-aware GQA attention, and SwiGLU FFN wiring
//! - `kv_cache` — default-off governed pre-reserved KV cache arena slots
//! - `ops` — the CPU op kernels (the §9.4 numerical reference)
//! - `admission_policy` (in `admission`) — §9.6 admit/defer/next-choice rules
//! - `sharing_logic` (in `sharing`) — §9.7 count-once attribution
//! - `placement_policy` (in `placement`) — §9.9 where-an-op-runs
//! - `evict_policy` (in `evict`) — §9.10/§9.11 LRU victims, retention, demotion
//! - `predictor` — §9.11 structural reuse lookahead (hot/future-use sets)
//! - `reservation_logic` (in `reservation`) — §9.12 claim truth table
//! - `watermark` (in `residency_daemon`) — §9.12 hysteresis, decay, prefetch gate
//! - `arena_logic` (in `arena`) — §9.12 generations, key match, free list,
//!   handle counts (the I-13 arithmetic)
//!
//! **Tier 2 — kernel glue (thin, mechanism-reusing):**
//! - `tensor` — the capability-backed tensor object; storage, migration
//!   (THE movement mechanism — everything above reuses it), generation counter
//! - `model` — validated `.neom` model objects
//! - `admission` — §9.6 queues + park/wake (temporal isolation, I-10)
//! - `sharing` — §9.7 grant/dedup registry (dedup keys SLOTS since BUILD-O2
//!   — candidacy pins nothing; the share survives movement)
//! - `placement` — §9.9 glue: snapshot residency/coverage/quota → decide → trace
//! - `evict` — §9.10/§9.11 glue: candidates from the pass's value table,
//!   eviction + boundary demote seams, the single-mutator coherence note (I-3)
//! - `exec` / `exec_ops` / `exec_dispatch` — the forward-pass driver, per-op
//!   shape rules, and the place/room/dispatch glue for one op
//!
//! **Tier 3 — concurrency (§9.12, on the proven primitive):**
//! - `reservation` — the migration reservation: claim-by-possession, park,
//!   atomic commit (I-3/I-4/I-11); `TensorSlot` is the shared surface
//! - `arena` — the generational slot arena (BUILD-O1a, ADR-042): the kernel
//!   as unique owner at rest; `SlotKey` names, `mint` is our `fget`, free
//!   bumps the generation (I-13: stale handles fail safe); owns `ManagedSlot`
//! - `residency_daemon` — the kswapd-model mover: per-device watermarks,
//!   bg-demote, publish-only prefetch, drain teardown; never load-bearing;
//!   walks the arena's live slots
//!
//! Decision logic goes in Tier 1 unless it physically cannot (governance
//! Rule 3); Tier 2 stays thin; Tier 3 consumes — never reimplements — the
//! reservation. The invariant ledger lives in `docs/complexity-governance.md`.

mod exec_dispatch;
mod exec_ops;
#[cfg(all(feature = "gguf", feature = "ai-generate"))]
pub mod forward_logic;
#[cfg(feature = "ai-generate")]
pub(crate) mod generate_logic;
#[cfg(feature = "gguf")]
pub mod gguf_dequant_logic;
#[cfg(feature = "gguf")]
pub mod gguf_logic;
#[cfg(all(feature = "gguf", feature = "ai-generate"))]
pub mod kv_cache;
#[cfg(all(feature = "gguf", feature = "ai-generate"))]
mod kv_layout;
mod model_logic;
pub mod ops;
#[cfg(feature = "decode-repetition-guard")]
pub(crate) mod repetition_logic;
// OT-34: cross-core deterministic fork-join for the forward matmul. Gated on
// `threaded-matmul` (which implies `phase9-real-quant` ⇒ `gguf`+`ai-generate`, so
// `forward_logic`'s column-partition substrate is present). Feature off removes
// the module and the `ap_poll` idle-loop hook entirely (kill-switch).
#[cfg(feature = "threaded-matmul")]
pub mod par_matmul;
// OT-34 F1: the pure fork-join barrier-decision policy (host-testable). Kept
// separate from the kernel executor so a host KAT can prove the fail-stop rule
// (claimed+unfinished+past-limit ⇒ Halt, never a return-that-frees-`out`).
#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic",
    feature = "neo-test-sys-generate-kv-persist-equivalence"
))]
pub mod decode_logic;
#[cfg(feature = "threaded-matmul")]
pub mod par_matmul_logic;
#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic",
    feature = "neo-test-sys-generate-kv-persist-equivalence"
))]
pub mod qwen_inventory;
mod tensor_logic;
#[cfg(feature = "gguf")]
pub mod tokenizer_logic;
#[cfg(feature = "gguf")]
pub mod vocab_bin_logic;

pub mod admission;
pub mod arena;
/// Arthur answer-latency breakdown (prefill/decode, layers/lm_head, matvec,
/// fork-join barrier). Diagnosis instrumentation; default off.
#[cfg(feature = "prefill-perf")]
pub mod commit_slot_logic;
pub mod evict;
pub mod exec;
/// Arthur perf lane — honest TSC wall-clock for the showcase forward (OT-7).
#[cfg(feature = "forward-perf")]
pub mod forward_perf;
#[cfg(feature = "ai-generate")]
pub mod generate;
pub mod model;
pub mod placement;
#[cfg(feature = "prefill-perf")]
pub(crate) mod pointer_window_logic;
pub mod predictor;
#[cfg(feature = "prefill-perf")]
pub mod prefill_perf;
#[cfg(feature = "prefill-perf")]
pub mod prefill_perf_logic;
pub mod reservation;
pub mod residency_daemon;
pub mod sharing;
pub mod tensor;

pub fn init() {
    crate::drivers::accel::init();
}
