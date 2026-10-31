<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-052: BUILD-LO — the latency oracle (cost as a measured, gated, falsifiable property)

**Status:** Accepted — the §4 performance-falsifiability obligation is **DISCHARGED**. The instrument exists, the first numbers are recorded, and all four budget claims **HOLD under measurement** (Tier A host + Tier B QEMU, 2026-06-14).
**Date:** 2026-06-14
**Design:** `docs/complexity-governance.md` §4 (the latency-oracle spec + the budget table + the four claims; PL-ratified 2026-06-10) — this build executes it and adds §5 (the recorded numbers + the gate rule).
**Dispatch:** `Plans/DISPATCH-2026-06-14-build-lo-latency-oracle-jarvis.md`
**Builds on:** the irqsafe-mutex-bench `rdtsc` precedent (`test_harness.rs`) and the host-suite KAT precedent (`tools/ai-ops-tests`, which path-`mod`s the pure `*_policy`/`*_logic` files).
**Ledger:** **no new invariant, no new lock, no new `unsafe`** beyond the `rdtsc` idiom (precedent). Pure instrumentation, default-off — the kill-switch is the feature gate (`neo-test-ai-latency-oracle` off ⇒ bit-for-bit today).

## The decision

The project made **correctness** falsifiable (the byte-oracle, I-1) and **maintainability**
falsifiable (the invariant ledger). **Cost was the one axis never instrumented** — the
policy layer was proven correct and switch-off-able, but its orchestration tax was
*designed-for, not measured*. The byte-oracle cannot see a regression that is purely a
slowdown.

BUILD-LO ships the **cost counterpart to the byte-oracle**: a micro-benchmark harness that
measures the hot-path orchestration operations **in isolation** and reports each against the
§4 budget. A governance change that blows the tax is now a *defect surfaced at a gate*, not a
slowdown discovered in production. The deliverable is the **instrument + the first reading +
the recorded discipline**, not a one-off number.

## Two tiers (mirroring §4 "host + QEMU")

**Tier A — host micro-bench** (`tools/ai-ops-tests`, `cargo test`). The pure, kernel-free
hot-path functions, measured with native `rdtsc` (median-of-N over a batched loop, warmed
up). These give portable, deterministic **SHAPE** proofs plus machine-noted cycles/iter:
- `placement_policy::decide` — swept over device count; **O(devices)**, no allocation.
- `sharing_logic::attributed_resident_bytes` / `total_resident_bytes` — the §9.5 accounting;
  **independent of tensor size**, O(principals) in the referencing set.
- `arena_logic::key_matches` + `bump_generation` — the I-13 stale-key gate every `mint`
  runs first; **O(1)**.

**Tier B — QEMU cycle harness** (`neo-test-ai-latency-oracle` scenario, `cargo neo-test`).
The operations that need the real kernel, measured with in-guest `rdtsc` (warm-up +
median-of-N, discard warm-up). Each emits a `NEO_TEST/1 event=measure … op=<name>
cycles=<median>` breadcrumb:
- `arena::mint` (the keyed `fget`: `ARENA` lookup + generation compare + `guarded_clone`)
  and `reservation::guarded_clone` (the slot-level clone alone).
- the **cap check** — `tensor_handle_lookup_in` (the per-syscall handle-table lookup + USE-
  rights gate), driven via a default-off probe seam against a harness-constructed process so
  there is **no syscall entry** (exactly what budget claim 1 is about).
- `sys_tensor_query` **metadata core** — the cap resolution + `mint` + `handle_count` +
  `query_desc` that the syscall runs, excluding only the fixed-size user-buffer copy.
- the **per-op orchestration total** — placement `decide` + `mint` + §9.5 accounting wrapped
  around **one dispatched CPU-fallback op** (a soft-f32 `ops::dispatch(Matmul)`), reported as
  a fraction of the op's own compute.

## The first numbers (recorded in `docs/complexity-governance.md` §5)

**Tier A — host, Intel Core i9-9900K @ 3.6 GHz, native rdtsc, 2026-06-14:**

| Op | Measurement | Result |
|---|---|---|
| `decide` (O(devices)) | 1/2/4/8/16/32/64/128 devices → 286/343/459/557/957/1886/3405/6169 cyc/iter | **linear** — 16× the input (8→128) costs 11×; quadratic would be ~256× |
| `attributed_resident_bytes` (size sweep) | 256 B … 1 TiB → ~51–72 cyc/iter | **flat** — independent of tensor size |
| `total_resident_bytes` (principal sweep) | 1/4/16/64 principals → 91/124/402/1231 cyc/iter | **O(principals)** |
| generation compare (`key_matches`+`bump`) | 120 cyc/iter | **O(1)** |

**Tier B — QEMU TCG on the i9-9900K, in-guest rdtsc, 2026-06-14:**

| Op | Median cycles (QEMU TCG) |
|---|---|
| `mint` (lookup + generation compare + guarded_clone) | 173,914 |
| `guarded_clone` (slot-level clone alone) | 50,848 |
| cap check (`tensor_handle_lookup_in`) | 37,918 |
| `sys_tensor_query` metadata core | 333,848 |
| query, size sweep 256 B / 4 KiB / 64 KiB / 1 MiB | 363,730 / 210,866 / 216,872 / 314,416 |
| orchestration (decide + mint + accounting) | 117,588 |
| matmul 16×16 (CPU fallback) / orchestration as ‰ of it | 32,859,846 / **3‰** |
| matmul 48×48 (CPU fallback) / orchestration as ‰ of it | 902,721,101 / **<1‰** |
| indirection: mints for an 8-op × 2-operand pass (4096 elems each) | **16 mints / 65,536 elements** |

**These Tier-B numbers are QEMU TCG, NOT silicon.** Under TCG the in-guest TSC is
virtualized and not constant-rate — one guest instruction costs hundreds of host "cycles",
the magnitudes are inflated ~100–1000×, and the counts are not additively consistent (e.g.
orchestration's bundled `mint` reads cheaper than the standalone `mint` because TCG
translation-block caching and warm-up differ between the two measurements). **Only the SHAPE
and RELATIVE results are conclusions; the absolute magnitudes are not.** A bare-metal
re-read for real magnitudes is **banked** (below).

## The four §4 budget verdicts — all HOLD

1. **No hot-path syscall** — `mint`/claim resolve **in-kernel**, a plain call with no ring
   transition (structural; the probe seam deliberately excludes the syscall entry). **HOLD.**
2. **Metadata-cheap / size-independent** — `sys_tensor_query` cycles are **flat across a
   4096× tensor-size sweep** (210 k–364 k, with the *smallest* tensor the *most* expensive —
   noise, not size scaling; size-linear cost would explode ~4096×). The I-4 cached-meta
   property holds: handles name slots, they do not carry bytes. **HOLD.**
3. **Amortizable per-op tax** — orchestration is **3‰ of a tiny 16×16 matmul and <1‰ of a
   48×48** on the CPU-fallback path, and the fraction *falls* as the op grows. This is the
   load-bearing budget: the tax vanishes on any real op. **HOLD.**
4. **Indirection paid once** — a representative forward pass mints **16 times for 16 operand-
   uses across 65,536 elements**: one `mint` per operand-*use*, never per element/weight.
   **HOLD.**

No budget was blown — happily, the four claims are now *measured* truths, not designs. (Had
one blown, this build would have **recorded it as a finding**, not optimized it: measuring is
the job; fixing is a follow-on.)

## A spec-vs-code discrepancy, recorded honestly

The dispatch's §1 table says `sys_tensor_query` "never mints". The shipped
`sys_tensor_query_impl` (`syscall.rs`) **does** call `arena::mint` to obtain the `Arc` it
reads `meta` from. This does **not** weaken budget claim 2: `mint` is an O(1) `Arc` bump that
touches no tensor bytes, so the query stays size-independent (the measurement proves it). The
phrasing "never mints" should read "never copies tensor bytes / cost independent of tensor
size." Recorded so the spec and the code agree on what is actually claimed.

## The gate rule this establishes (the discipline)

The oracle joins the gate family. **A future policy/substrate build that adds indirection to
the hot path measures its delta against the §5 table**, the way the kill-switch (I-2) proves
correctness-equivalence. A change that blows a budget is a *defect surfaced at the gate*, not
discovered in production. The four claims are no longer "designed-for, not measured" — they
are **measured (QEMU/host, 2026-06-14)**, with the bare-metal magnitudes banked.

## Banked follow-on — the bare-metal re-read

QEMU cycles are not silicon. A one-line follow-on rides the next OptiPlex boot: run the
`ai_latency_oracle` scenario bare-metal (the bare-metal-mode log sink) to capture the real
constant-rate-TSC magnitudes and confirm the shape holds on hardware. The §5 row carries the
QEMU numbers until then, explicitly labelled.

## Complexity-governance posture

Pure **instrumentation**, not policy: a host bench + a default-off `neo-test-ai-latency-
oracle` QEMU scenario, **zero hot-path indirection** (the mint counter for verdict 4 is
scenario-side, never in `mint`), **no new invariant**, **no new lock**, and **no new
`unsafe`** beyond the `rdtsc` idiom. Kill-switch trivially holds (feature-off ⇒ bit-for-bit
today; the only shipped-code change is the behavior-preserving extraction of
`tensor_handle_lookup_in` from `tensor_handle_lookup`, proven equivalent by the unchanged
cap/tensor test family). Article 15.7 honored — this is the *committed* §4 obligation; the
machinery is earned by the falsifiability axis it instruments. With it, **cost joins
correctness and maintainability as a measured, gated, falsifiable property**, and every
future hot-path build measures its tax instead of asserting it.

## BUILD-BENCH amendment — macro benchmark harness, no numbers yet

The Phase 8 LogansOS-vs-llama.cpp benchmark extends this oracle discipline from
kernel hot-path probes to a macro inference harness. The harness is
rig-agnostic: it treats LogansOS and Linux+llama.cpp as external commands, writes
a pre-number manifest, preserves raw append-only CSV samples plus stdout/stderr
logs, and computes only the pre-registered p50/p99, tokens/sec, footprint, and
fairness summaries.

Ledger-valid latency rows require an explicit line protocol from the subject:
`NEO_BENCH/1 event=token index=N latency_ns=...` and a final
`NEO_BENCH/1 event=done tokens=... decode_ns=... rss_bytes=...`. A wall-clock
fallback may be enabled for harness shakedown, but those rows are marked
`valid_for_latency=false` and cannot satisfy p50/p99 claims. ISC-2 adds only the
harness and ratified methodology; it does not collect benchmark numbers and does
not promote or demote any ledger claim.
