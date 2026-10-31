<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# CLOSE REPORT: TLB-shootdown timeout flake

**Branch:** `codex/tlb-shootdown-timeout-flake-jarvis` (merged ff-only to `main` as `5a9c878`)
**Dispatch:** `Plans/DISPATCH-2026-06-02-tlb-shootdown-timeout-flake-jarvis.md`
**Supersedes task in:** `Plans/DISPATCH-2026-06-01-ci-sched-capacity-pressure-jarvis.md` (SUPERSEDED)
**Design contract updated:** `docs/ADR-013-phase-2-smp-retrofit.md`, `docs/phases/PHASE-2.md` (SC-3C entry)
**Status:** ✅ CLOSED — fixed, CI green on `main`, bare-metal verified on real hardware.

## Summary

`main` CI's QEMU Integration Tests job had been intermittently red (~50–60% per run) with a kernel panic at `kernel/src/mm/tlb_shootdown.rs:236`: `tlb_shootdown_timeout pending=1 targets=0x0e`. Root cause: the sender's TLB-shootdown ACK deadline was a single **1 ms** TSC budget with **no IPI re-send**. Under QEMU/TCG SMP, the sender busy-spins (`PAUSE`, which does not yield a vCPU under TCG round-robin), so the very AP being waited on could be starved of execution long enough to miss the 1 ms window — one straggler AP, non-deterministically. The fix replaces the blind budget with a **1 ms intermediate re-poke + 50 ms bounded final panic**, and adds instrumentation that pins the ACK-latency margin in-test.

## Root-cause arc (recorded because the misdiagnosis is the lesson)

1. **First diagnosis (wrong):** the sched-capacity dispatch read a *truncated* CI log, never saw the per-scenario serial frame, and decoded exit status 35 as the scenario's own `debug_exit(EXIT_FAILURE)` — concluding a `sched_ready_queue_capacity_pressure` *scheduler functional* failure.
2. **Correction (`8a28c88`):** serial-log capture (added to CI) showed the real frame was `reason=kernel_panic` from `tlb_shootdown.rs:236`, cross-cutting (hit `sys_inspect_running`, `sys_wait_already_exited` — never the sched scenario), and flaky. Re-dispatched as a focused TLB-shootdown timeout task.
3. **Fix:** principled re-poke + bounded panic, not a constant bump.

**Lesson:** a persistent flake doesn't just cost time — it *hides real bugs behind it* (see "Latent bug flushed out" below). Always capture the actual serial frame before theorizing from a truncated stdout.

## The fix

`kernel/src/mm/tlb_shootdown.rs`:
- `SHOOTDOWN_TIMEOUT_MS = 1` → **`SHOOTDOWN_RETRY_AFTER_MS = 1`** + **`SHOOTDOWN_FINAL_TIMEOUT_MS = 50`**.
- After the 1 ms window, `repoke_pending_targets()` re-sends the IPI **once** to only those targets whose per-CPU slot still holds our payload pointer (i.e. genuinely not-yet-serviced; mid-handler targets are correctly skipped). Panic only fires at 50 ms if ACKs are still pending, now with rich diagnostics (`unacked_slots`, `repoked`, `elapsed_cycles`, `timeout_cycles`).
- Per-CPU instrumentation added: send-TSC, last/max ACK cycles, retry-IPI count, timeout count.

## Validation

| Gate | Result |
|---|---|
| **QEMU statistical clearance** | 20/20 consecutive full `cargo neo-test` runs under `NEO_SMP=4`, **zero** non-intentional `tlb_shootdown_timeout`; `non_intentional_timeout_hits=0` |
| **ACK-latency margin** | worst measured synthetic ACK latency 1,309,226 cycles ≈ **130.9 µs** vs the 50 ms / 500,000,000-cycle budget ≈ **382× headroom** — and pinned in-test (`q7c` fails if `max_ack_cycles >= final_timeout_cycles`) |
| **Negative scenario** | `q7c_shootdown_timeout_panic` still forces the timeout and passes only when the expected panic fires (real-stuck-CPU detection not blinded) |
| **Host-checks** | `cargo fmt --all --check`, `cargo test --workspace --exclude neoplutarch-kernel`, `cargo clippy --workspace --exclude neoplutarch-kernel -- -D warnings` — all exit 0 |
| **CI on `main`** | green (host-checks + SPDX + QEMU Integration Tests) — first clean QEMU column since the BUILD-4 merge |
| **Bare-metal smoke** | green (see below) |

### Bare-metal evidence (Option A — observational, AHCI exemplar)

Run on a physical OptiPlex (12 logical CPUs) via `phase3_ahci_neofs_mount_and_readwrite --bare-metal-mode` on `2daa1de`, serial captured on COM3 @ 38400:

- **AP bring-up:** PASS — 11 `AP ... ready` lines + `smp: started 11 AP(s)` (real cross-CPU shootdowns exercised at 12 CPUs — 3× the `NEO_SMP=4` concurrency CI ran).
- **TLB panic scan:** PASS — 0 hits for `tlb_shootdown` / `KERNEL PANIC`.
- **NEO_LOG readback:** PASS — 9200 bytes extracted, scenario PASS:
  `NEO_TEST/1 event=PASS name=phase3_ahci_neofs_mount_and_readwrite fs=neofs source=device0 mount=rw initial_read=ok create_write_fsync_readback=ok bytes=30`

Evidence files (on the worker tree, `AltimitNeo-ci-sched-capacity-jarvis/target/bare-metal-smoke/tlb-shootdown-ahci-2daa1de/`):
- `serial-com3-38400-20260603-203547.log`
- `tlb-smoke-extracted.log` — sha256 `cf4d71a74934c54bd2b12b1f656c9931bbd5904993549254902c618ecda353b4`

## Latent bug flushed out

Getting the first-ever clean end-to-end QEMU run exposed a BUILD-4 defect the flake had been masking: `infer_transformer_forward` had a **swapped opcode ABI** (`OP_SOFTMAX`/`OP_GELU`) and **missing affine LayerNorm/RMSNorm** (learnable γ/β). Both were fixed in the same commit (`ai/exec.rs`, `ai/ops/mod.rs`) and are now accuracy-gated. BUILD-4's QEMU job had never once run clean, so this was never verified until now.

## Policy of record (ADR alignment)

`ADR-013` and `PHASE-2.md`'s SC-3C entry updated to describe the **1 ms re-poke + 50 ms bounded panic** protocol, and to record **"blind timeout bump" as a rejected alternative** with rationale — so the code and the design docs agree on why the constant is 50 ms with a re-poke rather than a larger naked budget.

## Deferred (not blocking — recorded so it isn't lost)

**Option B:** wire `q7c_shootdown_synthetic_round_trip` (+ the `q7c_shootdown_timeout_panic` negative) into `bare_metal_log_sink_target_feature` / `prepare_bare_metal_storage_disks` so `--bare-metal-mode` captures `max_ack_cycles` on real hardware directly (today `--bare-metal-mode` only supports the AHCI/NVMe NeoFS exemplars). ~half-day of xtask plumbing; only needed if the hardware ACK-latency *number* is ever wanted on the record.

## Disposition

The flake is closed on **QEMU and real hardware**. `main` is fully green. Bare-metal box returned to storage.
