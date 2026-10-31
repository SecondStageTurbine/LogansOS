<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: BUILD-LO — the latency oracle (the §4 performance-falsifiability obligation) → Jarvis

**Date:** 2026-06-14
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `f32f7d8`: `codex/build-lo-latency-oracle-jarvis`)
**Status:** DISPATCHED. **Host + QEMU only — NO silicon, NO dongle. This discharges the standing §4 commitment** (`docs/complexity-governance.md` §4, PL-ratified 2026-06-10): build the hot-path latency oracle, take the **first measured numbers** against the budget, and turn cost into a falsifiable axis the way the byte-oracle made correctness one.
**Why now:** the WiFi arc's remaining work is hardware (W1 needs the `148f:7601` dongle, ~2 days out). §4 was explicitly scheduled "after the Phase 7.5 connectivity lane," it adds no AI-hot-path indirection, and it's fully host/QEMU-testable — the right build to drive while the dongle ships. It's also load-bearing for the JARVIS interactive layer (VISION doc §4): "designed-for, not measured" is acceptable for a batch governor, not a conversation.

## 0. The decision this build makes
The project has made **correctness** falsifiable (the byte-oracle, I-1) and **maintainability** falsifiable (the invariant ledger). **Cost is the one axis never instrumented.** This build ships the **cost counterpart to the byte-oracle**: a micro-benchmark harness that measures the hot-path orchestration operations in isolation and reports them against the §4 budget — so a governance change that blows the tax is a *defect surfaced at a gate*, not a slowdown discovered in production. The deliverable is the **instrument + the first reading + the recorded discipline**, not a one-off number.

## 1. The hot-path operations to measure (tree-verified symbols — do NOT re-derive)
Per §4's table, each measured in isolation and asserted against its budget:
| Op | Symbol (verified on `main`) | Budget (§4 — confirm or adjust to the measured number) |
|---|---|---|
| `mint` / `guarded_clone` | `kernel/src/ai/arena.rs:263` `mint(SlotKey)`, `arena.rs:85` + `kernel/src/ai/reservation.rs:419` `guarded_clone` | O(1): a small constant of atomics + generation compare + **one uncontended `ARENA` critical section**; **zero allocation, no syscall, no park** on the uncontended path |
| Placement decision | `kernel/src/ai/placement_policy.rs:74` `decide(&[DeviceCandidate], u64)` | pure function, **O(devices)**, no I/O, no lock beyond the §9.5 snapshot |
| Accounting | `kernel/src/ai/sharing_logic.rs:33/46` `attributed_resident_bytes` / `total_resident_bytes` | pure, O(principals), no tensor-byte touch |
| `sys_tensor_query` (metadata) | `kernel/src/arch/x86_64/syscall.rs:930` `sys_tensor_query_impl(h, out, out_len)` | **never mints, never touches tensor bytes**; cost **independent of tensor size** |
| Capability check | the cap-derivation walk on the tensor-handle syscall path (pin the exact symbol from the `SYS_TENSOR_*` handlers in `syscall.rs`) | O(derivation depth), bounded; no tensor-data touch |
| **Per-op orchestration total** | placement + mint + accounting wrapped around **one dispatched CPU-fallback op** (an `ops/` kernel, e.g. a matmul, via the `accel-sim` path) | **the load-bearing budget:** the tax is a small constant *fraction of the op's own compute* — amortizable, i.e. negligible on any real op |

## 2. Two tiers (mirrors §4 "host + QEMU")
- **Tier A — host micro-bench (the pure, kernel-free functions).** `decide`, `attributed_resident_bytes`/`total_resident_bytes`, and the slot/generation compare are already in the host-testable tier (the `*_policy`/`*_logic` pattern). Measure them on the host (`tools/ai-ops-tests` sibling or a `[[bench]]`/iteration-timed test) with **iteration counts + median-of-N**, and prove the *shape* budgets structurally: sweep the device-candidate count and show `decide` scales O(devices); these give portable, deterministic, machine-independent numbers.
- **Tier B — QEMU cycle harness (the operations that need the real kernel).** `mint`/`guarded_clone` (real `ARENA` lock + `Arc` atomics + generation compare), the cap check, `sys_tensor_query` (the real metadata path), and the **per-op total**. Use the established **NEO_TEST bench-scenario precedent** — a `neo-test-ai-latency-oracle` feature → `SCENARIO = "ai_latency_oracle"` in `test_harness.rs` (the `phase2-perf-proxy-baseline` / `irqsafe-mutex-bench` pattern), **`rdtsc`** for cycle counts (precedent: `tlb_shootdown.rs:403`, the existing mutex bench), **warm-up + median-of-N** to control TCG/KVM jitter. Emit one breadcrumb per op: `NEO_TEST/1 event=measure name=ai_latency_oracle op=<name> cycles=<median> n=<samples>`.

## 3. ISCs
- **ISC-1 — Tier-A host bench.** `decide` (sweep device count → show O(devices) + record cycles/iter), accounting (record), generation-compare (record). Assertions: `decide` does no allocation and its time grows linearly (not super-linearly) in device count; accounting is independent of tensor *size*. Deterministic, machine-noted.
- **ISC-2 — Tier-B QEMU cycle harness.** The `neo-test-ai-latency-oracle` scenario measuring mint/guarded_clone, cap check, `sys_tensor_query`, and the per-op total in TSC cycles (warm-up + median-of-N), each emitted as a `measure` breadcrumb. **Trap: TSC under virtualization is noisy and not constant-rate** — warm up, take the median, discard the first sample, and **state the measurement context** (QEMU TCG/KVM, host CPU) in the output; do not present a virtualized cycle count as a silicon number.
- **ISC-3 — The four §4 budget verdicts**, each reported hold/violate with its number:
  1. **No hot-path syscall** for in-kernel residency ops — mint/claim resolve in-kernel (structural: assert the path makes no syscall entry) + the cycle number.
  2. **Metadata-cheap** — `sys_tensor_query` cycles **flat across a tensor-size sweep** (the I-4 cached-meta property: handles name slots, don't carry bytes).
  3. **Amortizable per-op tax** — orchestration cycles **as a ratio** of the dispatched op's own compute cycles (the load-bearing verdict — report the fraction).
  4. **Indirection paid once** — a counter proving `mint` fires **per-operand, not per-element** across a representative forward pass (per-*use*, never per-weight) — a count, not a timing.
- **ISC-4 — Record + establish the discipline.** Add a **§5 performance-row table** to `docs/complexity-governance.md` with the **first measured numbers** (Tier-A host cycles/iter + Tier-B QEMU median cycles, machine + QEMU mode noted), flip the §4 "designed-for, not measured" caveats to **"measured (QEMU/<host>, <date>)"** for the ops now instrumented (leaving a **banked bare-metal re-read** as a one-line follow-on that rides the next OptiPlex boot — QEMU cycles are not silicon), confirm-or-adjust each budget to the real number, and state the **gate rule**: *a future policy/substrate build that adds hot-path indirection measures its delta against this table, the way the kill-switch proves correctness-equivalence.* Open **ADR-052** (the latency oracle: the instrument, the first reading, the budget discipline).

## 4. Gate (host + QEMU — no silicon)
- **Tier A (host):** the micro-bench runs, records numbers, and its shape-assertions pass (O(devices), size-independence); clippy; fmt.
- **Tier B (QEMU):** the `ai_latency_oracle` scenario boots, emits the per-op `measure` breadcrumbs + the four verdicts, and the **existing test family stays green** (the oracle is additive instrumentation, default-off). NEO_SMP=4 sweep unaffected.
- **Recorded:** the §5 table + ADR-052 carry the first numbers; the four budget claims reported hold/violate against measurement.

## 5. Complexity-governance posture (state in the dispatch RETURN)
Pure **instrumentation**, not policy: a host bench + a `neo-test-ai-latency-oracle`-gated QEMU scenario, **default-off**, adding **zero hot-path indirection**, **no new invariant**, **no new lock**, and **no new `unsafe`** beyond the `rdtsc` idiom (precedent: the existing mutex bench + `tlb_shootdown.rs`). Kill-switch trivially holds (feature-off ⇒ bit-for-bit today). Article 15.7 honored — this is the *committed* §4 obligation, the machinery is earned by the falsifiability axis it instruments. Consolidation cadence untouched (no feature; it's the cost gate).

## 6. Scope guard
The **measurement instrument + first reading + the recorded discipline** only. **Do NOT** optimize any hot path in this build (if a number blows its budget, **report it as a finding** — measuring is the job; fixing is a follow-on); **do NOT** add hot-path indirection; **do NOT** present QEMU TSC as a silicon number (state the context, bank the bare-metal re-read); **do NOT** invent budgets — confirm or adjust §4's against the measurement and show the number. If a hot-path op can't be isolated without refactoring shipped code, BLOCKER-to-PM rather than churning a proven path for the bench.

## 7. Merge (PM, on gate PASS)
Close-gate: rebase onto current `main`, re-run Tier-A + the `ai_latency_oracle` scenario + the family, push. PM `pm-merge-check` → `--ff-only` → `main`; ledger the §4/§5 update + ADR-052 + the audit-log row. With it, **cost joins correctness and maintainability as a measured, gated, falsifiable property** — and every future hot-path build measures its tax instead of asserting it. Then back to the WiFi arc when the dongle lands (W1).

---

## Relay — what to tell Jarvis
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-14-build-lo-latency-oracle-jarvis.md`
- **Design anchor (on `main`):** `docs/complexity-governance.md` **§4** (the latency oracle spec + the budget table + the four claims) — this build executes it.
- **Read them from `main`** (your tree is on a feature branch — a local file-open won't find them):
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-14-build-lo-latency-oracle-jarvis.md
  git show origin/main:docs/complexity-governance.md   # read §4
  ```
- **Branch:** `codex/build-lo-latency-oracle-jarvis` off `origin/main` `f32f7d8`.
- **One-line:** Build the **latency oracle** — the cost counterpart to the byte-oracle, discharging the committed §4 obligation. **Tier A (host):** micro-bench the pure hot-path functions (`placement_policy::decide` @ `placement_policy.rs:74`, `sharing_logic::attributed_resident_bytes`/`total_resident_bytes`, generation-compare) — iteration timing, show `decide` is O(devices), size-independence. **Tier B (QEMU):** a `neo-test-ai-latency-oracle` NEO_TEST scenario (the `phase2-perf-proxy-baseline`/`irqsafe-mutex-bench` + `rdtsc` precedent) measuring `arena::mint`/`reservation::guarded_clone`, the cap check, `sys_tensor_query_impl` (@ `syscall.rs:930`), and the **per-op orchestration total** around one `accel-sim` CPU-fallback op — TSC cycles, warm-up + median-of-N, emit `NEO_TEST/1 event=measure name=ai_latency_oracle op=<name> cycles=<n>`. Report the **four §4 budget verdicts** (no hot-path syscall / metadata-cheap-size-independent / amortizable per-op tax as a ratio / indirection-paid-once via a mint-per-operand counter). Record the **first numbers** in a new **§5 perf-row table** in `complexity-governance.md`, flip the instrumented ops from "designed-for" to "measured (QEMU/<host>)", bank a bare-metal re-read, open **ADR-052**. Pure instrumentation: default-off, zero hot-path indirection, no new invariant/lock, `unsafe` only the `rdtsc` idiom. **Don't optimize** — if a number blows budget, report it as a finding. `[skip ci]`-adjacent is NOT appropriate here (Tier B is a real QEMU scenario — let the family run). Next: back to W1 when the dongle lands.
