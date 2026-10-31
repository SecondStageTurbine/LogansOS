<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: relay #91 — A9 presents/liveness review → Janus

**MODEL:** GPT-6-Luna. **EFFORT:** high. **Seat:** Janus.

**Reviewed:** `839b893e0240493e6cea12ca8d23f1d403cd1efb` (#151 squash), not current `main`.
**Overall: FAIL.** The pointer window has a reachable cross-CPU boundary race that can misclassify the WM, and the PR omits the required concurrency-pitfall consultation. Request PM **revert-first** for the blocking runtime finding; no implementation fix made here.

## Checklist

1. **The counters — FAIL.** `kernel/src/ai/prefill_perf.rs:158-172,203,228-232,253-255`; dequeue is on the WM path at `kernel/src/atrium/focus.rs:256-270`.
   `begin_generation` clears `GEN_END_TSC` before it clears `PTR_RX`, then samples the input baseline and only stamps the new `GEN_BEGIN_TSC` at the end. With the inference stack's normal `with_interrupts_enabled` call (`kernel/src/mm/inference_stack.rs:254-269,356-361`), CPU 0 can run the WM between those stores. If it dequeues then, `note_pointer_rx` sees the prior nonzero begin and the newly zero end. Depending on whether that increment lands before or after the later `PTR_RX.store(0)`, the event is dropped or carried into a generation whose start stamp has not happened. At close, `end_generation` snapshots driver reports at `:228-230` before publishing the end at `:232`; a report/dequeue in that interval is included in `rx` but omitted from `in`. Atomics prevent lost increments among simultaneous `fetch_add`s; they do not make these multi-counter windows atomic. No extra lock is taken on the ISR path.

2. **The diagnosis it enables — FAIL.** `kernel/src/ai/prefill_perf.rs:228-232,253-255,439-445` permits `in=0 rx=1`: the WM dequeues a pointer record after the report snapshot at `:228-230` but before `GEN_END_TSC` is set at `:232`; `note_pointer_rx` still counts it because end is zero. This is a fourth reading, outside the three documented diagnoses at `:20-22`, and defeats the clean input-versus-WM distinction.

3. **The guard measures what it claims — PASS, with evidence distinction.** `kernel/src/test_harness.rs:38637-38850` measures gaps with `_rdtsc` and calibrated `tsc_hz`, asserts the 250 ms liveness bound, and requires the IF=0 control to exceed it. `kernel/src/mm/inference_stack.rs:356-361` turns interrupts on for the product core; `kernel/src/arch/x86_64/syscall.rs:3720-3740` supplies the IF=0 control. `kernel/src/ai/par_matmul.rs:490-513,520-553` sets the test attention threshold to 1 and forks every attention call; the scenario requests `online_cpu_count()` workers and fails without attention jobs/AP partitions at `kernel/src/test_harness.rs:38700-38840`. The PR body reports liveness `gap_ms=39` and control `gap_ms=29642`; GitHub run `40440467911` independently reports success for all 22 jobs, but its downloadable job log did not expose those serial lines, so the exact timings remain PR-reported rather than independently re-read here.

4. **Production unchanged except overlay field — PASS.** The overlay additions and call sites are `prefill-perf`-gated (`kernel/src/ai/mod.rs:117-120`, `kernel/src/atrium/focus.rs:267-270`); the liveness scenarios and IF=0 helper are `neo-test`-gated (`kernel/Cargo.toml:1009-1022`, `kernel/src/arch/x86_64/syscall.rs:3720-3724`, `kernel/src/test_harness.rs:38637-38646`). The touchpad getter at `kernel/src/drivers/i2c_hid/touchpad.rs:101-104` is passive; it does not alter the driver path.

5. **Prior art — FAIL.** PR #151's body describes IF-on/IF-off preemption and the scenario but contains no `docs/concurrency-pitfalls.md` consultation, applicable CP classes, or discriminators. The required rule is explicit in `CLAUDE.md:828-835` and the relay checklist; the bank's scheduler/IRQ classes include CP-2, CP-3, CP-21, and CP-25 (`docs/concurrency-pitfalls.md:47-80`). The measured IF and scheduling classes needed an applicability/guard table in the PR.

## Gate and close

- Assigned GitHub run `40440467911`: `success`, 22/22 jobs green. Its log did not contain the scenario serial output. PR #151 records the claimed QEMU values above; its original merge check showed QEMU skipped, so these are kept distinct from the later green run.
- RAG health check: healthy (`ollama=true`, `qdrant=true`); used for discovery only. The pinned local source above is authoritative.
- No tests or implementation changes made. This RETURN is the only intended branch change.

**Requested action:** PM revert-first for #151 because the RX/input window can produce a false diagnosis; restore after the instrumentation window is synchronized and the cross-family review is complete.
