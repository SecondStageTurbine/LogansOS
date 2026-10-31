<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Fix CI red — QEMU scenario `sched_ready_queue_capacity_pressure` → Jarvis

**Date:** 2026-06-01
**From:** PM/Cerebro
**To:** Jarvis (worker tree, feature branch — e.g. `prometheus/ci-sched-capacity-fix` or your choice)
**Status:** SUPERSEDED (2026-06-02) — the worker task moved to [`DISPATCH-2026-06-02-tlb-shootdown-timeout-flake-jarvis.md`](DISPATCH-2026-06-02-tlb-shootdown-timeout-flake-jarvis.md). This dispatch's 2026-06-02 root-cause correction (below) reidentified the failure as a cross-scenario TLB-shootdown timeout panic, not a scheduler functional failure; the focused fix is dispatched there. Kept for history.
**Priority:** High — this is the only thing keeping `main` CI red after the SPDX fix.

> **⚠️ 2026-06-02 ROOT-CAUSE CORRECTION — read [the update at the bottom](#update-2026-06-02--root-cause-corrected-tlb-shootdown-timeout-panic-not-a-scheduler-functional-failure) FIRST.** The original diagnosis below (sections "The failure" / "Root-cause narrowing") concluded the scenario's `_ipc_smoke()` workload exits nonzero — i.e. a scheduler ready-queue *functional* failure. **That was wrong.** Serial-log capture (now in CI) shows the real cause is a **kernel panic in TLB shootdown** (`kernel/src/mm/tlb_shootdown.rs:236`), and the failure is **cross-run flaky (~50–60%)**, not deterministic. The sections below are kept for history; the update supersedes them.

---

## Background: the red column was TWO stacked failures

CI on `main` had been red for an unbroken run of commits (Actions #97 → #104). Investigation (2026-06-01) found **two independent causes**, either of which alone fails the whole run:

1. **`spdx-lint` job** — `kernel/src/net/tcp.rs` was missing its mandated SPDX header (first line was the `//!` doc comment) since `438cae2` (phase-4 build-4a). This failed deterministically on *every* push regardless of content, **masking** cause #2. **FIXED** in `a2d824b` (header added; verified the `spdx-lint` and `host-checks` jobs both go green on the live run).

2. **`neo-test` (QEMU Integration Tests) job** — scenario **`sched_ready_queue_capacity_pressure`** fails its in-kernel assertion. This is **your task.** It was invisible until the SPDX fix landed because spdx-lint failed first on every run.

`host-checks` (fmt / clippy / test `--workspace`) is green and was never a factor.

## The failure — exact evidence

From run `26779142016` (head `a2d824b`), job "QEMU Integration Tests", step "Run QEMU integration tests":

```
[xtask] neo-test: launching QEMU for `sched_ready_queue_capacity_pressure`
--- DISK_SNAPSHOT 20:23:16 ---  /dev/root 72G 45G 28G 63% /   (disk NOT the issue)
xtask: `sched_ready_queue_capacity_pressure` failed after 4 attempts;
       did not exit via debug success; status=exit status: 35
##[error]Process completed with exit code 1.
```

- **Deterministic:** all 4 retry attempts failed identically. Not flaky, not disk-exhaustion (disk at 63%, the container built fine, ran ~10 min not a 22-min kill).
- **Exit decode:** xtask uses `qemu_debug_exit_status(code) = (code<<1)|1`. Observed `35` ⇒ `code = 17 = 0x11 = EXIT_FAILURE`. The kernel **reached a deliberate failure path and called `debug_exit(EXIT_FAILURE)`** — it did not hang or triple-fault.

## Root-cause narrowing already done (start here)

The scenario body is `kernel/src/test_harness.rs:1145`:

```rust
#[cfg(feature = "neo-test-sched-ready-queue-capacity-pressure")]
{
    let exit_code = _ipc_smoke();
    if exit_code == 0 {
        pass("sched_capacity_scenario=ok exit_code=0");   // pass() is `-> !`, exits SUCCESS
    }
    serial_println!(
        "NEO_TEST/1 event=FAIL name={} reason=sched_capacity_scenario_exit_nonzero code={}",
        SCENARIO, exit_code
    );
    debug_exit(EXIT_FAILURE);
}
```

- **Harness logic verified correct.** `pass()` (`test_harness.rs:4694`) is `-> !` and calls `debug_exit(EXIT_SUCCESS)`, so the `FAIL` path is reached *only* when `_ipc_smoke()` returns nonzero. The pattern is byte-identical to the sibling `sys-wait` / `sys-inspect` scenarios (`test_harness.rs:1132`) which pass. So this is **not** a fall-through / missing-return bug in the harness.
- **Therefore the failure is functional:** the user-side workload driven by `_ipc_smoke()` under the `sched-ready-queue-capacity-pressure` user feature (xtask entry: `tools/xtask/src/main.rs:430`, `user_feature: Some("sched-ready-queue-capacity-pressure")`) **exits nonzero** under ready-queue capacity pressure. Either the scheduler's ready-queue-under-pressure behavior is wrong, or the scenario's expectation is.

## What I did NOT determine (do this first)

- **The actual nonzero `code=N`** — the `reason=sched_capacity_scenario_exit_nonzero code=N` line lives in the serial log (`target/neo-test/sched_ready_queue_capacity_pressure.attempt4.serial.log`), not the truncated job stdout I read. **Pull that serial frame first** — the specific code tells you which assertion in the user workload tripped.
- **Regression vs. long-standing.** I could not tell whether the QEMU job started failing recently or was failing all along behind the SPDX mask (determining it needs per-run job data across older runs). Worth a quick `git log` on the scheduler ready-queue code + the scenario/user feature to see if a recent change correlates. The scenario plumbing was last touched around `806756f` (BUILD-3) / `5456d23`.

## Repro (Docker works in-container; daemon must be running)

```bash
# full suite (as CI runs it):
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test
# faster: build + run just this scenario's feature, then inspect its serial log under target/neo-test/
```

## Close gate (standard)

- `git fetch origin && git rebase origin/main` before close (current `main` tip is `a2d824b`).
- The `sched_ready_queue_capacity_pressure` scenario must **exit via debug SUCCESS** in QEMU (all attempts), and the full `cargo neo-test` suite must pass.
- Host-checks green (`cargo fmt --all --check`, `cargo test --workspace --exclude neoplutarch-kernel`, `cargo clippy --workspace --exclude neoplutarch-kernel -- -D warnings`).
- **If the fix is to the scheduler:** add/keep a scenario assertion that pins the corrected ready-queue-under-pressure behavior so it can't silently regress.
- **If the conclusion is that the *expectation* was wrong** (scenario asserts something the design never promised): bring that back to PM with evidence before weakening/removing the scenario — don't unilaterally relax a test to make CI green (cf. the BUILD-4 bounce).
- Push to your **feature branch, not `main`.**

---

## UPDATE 2026-06-02 — root cause corrected: TLB-shootdown timeout panic, not a scheduler functional failure {#update-2026-06-02--root-cause-corrected-tlb-shootdown-timeout-panic-not-a-scheduler-functional-failure}

The original diagnosis above was built on a **truncated** job log: the CI stdout was cut off before the per-scenario serial frame, so the `code=N` line was never read, and the exit decode (`35 → code 17 → EXIT_FAILURE`) was *assumed* to be the scenario's own `debug_exit(EXIT_FAILURE)` after `_ipc_smoke()` returned nonzero. With serial logs now captured, that assumption is disproven.

### What actually happens

The scenario triggers a **kernel panic**, and the panic handler is what calls `debug_exit(EXIT_FAILURE)` (hence the same exit status 35 — `code 17` — that the original dispatch mis-attributed to the scenario workload). The serial frame's reason is `kernel_panic`, **not** `sched_capacity_scenario_exit_nonzero`:

```
================================================
!!  KERNEL PANIC  !!
panicked at kernel/src/mm/tlb_shootdown.rs:236:13:
tlb_shootdown_timeout pending=1 targets=0x000000000000000e
================================================
NEO_TEST/1 event=FAIL name=sched_ready_queue_capacity_pressure reason=kernel_panic
```

- **`targets=0x0e`** = bits 1,2,3 set ⇒ the shootdown was sent to **CPUs 1, 2, 3** (BSP=0 excluded, correct).
- **`pending=1`** ⇒ exactly **one** of those three APs never decremented `payload.pending` before the TSC deadline in `wait_for_acks_or_panic`.
- It fires **mid kill-storm**: the scenario spawns and `sys_kill`s dozens of `worker` procs (pids 47–52 visible immediately before the panic), each address-space teardown issuing a TLB shootdown. The serial trace shows a dense run of `proc: terminated 'worker' … via sys_kill` / `pmap: dropped — freed N frames + 1 PML4` right up to the panic.

### Flakiness — corrected

The original "deterministic, all 4 attempts failed identically, **not flaky**" was true *within one run* but wrong *across runs*:

- **Within a failing boot:** panics **5/5** (`attempt1`–`attempt4` + the final serial.log all show the identical panic).
- **Across runs:** **~50–60%** of runs trip it. Evidence — `main` `workflow_dispatch` batches on identical kernel bytes:
  - runs `26797418255 / 420576 / 423173 / 425507 / 427646 / 429870` → 2 pass, 3 fail, 1 cancelled.
  - runs `26822585498 / 589438 / 600037 / 604479` → 3 fail, 1 pass.
- **Duration signature:** failing runs die at **~13–15 min**; clean runs complete the full suite in **~41–45 min**. A green run exercises *every* scenario after `sched_*`, so the bug is purely whether the boot reaches the kill-storm state badly enough to lose an ack.

This pattern (near-deterministic once in the bad state, ~50% whether it gets there) argues for a **real software stall on a target CPU**, not pure host-scheduler jitter — though host oversubscription on the GHA runner may be what tips a marginal timeout over the edge.

### Evidence is now captured automatically

CI commit **`1a05baf`** ("ci: capture neo-test serial logs on failure", on `main`) added two `if: failure()` steps to the `neo-test` job: it dumps `target/neo-test/*.serial.log` (tail 240, under `::group::` folds) into the job log and uploads them as the **`neo-test-serial-logs`** artifact (14-day retention). Example: run `26822585498`, artifact ID `7359246930` (98 serial logs). Pull the `sched_ready_queue_capacity_pressure.*.serial.log` files from any failed `main` run to see the full pre-panic trace.

### The code (start here)

All in `kernel/src/mm/tlb_shootdown.rs`:

- **`wait_for_acks_or_panic`** (panic site, line 236) — sender spins on `payload.pending` against a `timeout_cycles()` TSC deadline; panics if any ack is outstanding when it expires.
- **`handle_shootdown_ipi`** (receiver) — swaps its `pc.pending_shootdown` slot to null, `invalidate_local(range)`, then `payload.pending.fetch_sub(1, Release)`. If this never runs on a target (IRQs masked / CPU not servicing), the sender times out.
- **`publish_payload_to_targets`** — note the sibling `tlb_shootdown_slot_busy` panic: if a target's slot is *already* occupied, two shootdowns overlap on one CPU. Worth checking whether the kill-storm produces overlapping shootdowns.
- **`shootdown_ready_cpus_mask`** / `per_cpu::tlb_shootdown_ready(cpu_id)` — defines who is targeted. A CPU marked "ready" that can't actually service IPIs (e.g. parked with IRQs off) would be silently included and never ack.
- **`timeout_cycles()`** — verify how the cycle budget is derived; if it assumes a TSC frequency that's wrong/uncalibrated under QEMU/KVM, the *effective* timeout is far shorter than intended.

### Hypotheses to test (in rough priority)

1. **IRQ-off window on a target during teardown.** A target CPU is itself inside a critical section with interrupts disabled (its own `pmap` drop / shootdown / lock held `cli`) longer than the timeout, so it can't take the incoming IPI. Under the kill-storm many CPUs tear down simultaneously → cross-waiting. **Most likely given the 5/5-once-in-state pattern.**
2. **Mis-calibrated `timeout_cycles()`.** TSC-frequency assumption wrong under virtualization ⇒ deadline far too tight. Would explain host-sensitivity.
3. **Host vCPU deschedule.** The oversubscribed GHA runner deschedules a guest vCPU past the timeout. Real, but should be the *tip* not the whole cause; don't "fix" by just inflating the timeout without ruling out #1.

### Revised close gate (supersedes the rebase tip below — current `main` is `1a05baf`, includes the serial-capture step)

- `git fetch origin && git rebase origin/main` before close.
- `sched_ready_queue_capacity_pressure` must exit via debug **SUCCESS** in QEMU across **multiple** runs (the cross-run flakiness means a single green is not sufficient evidence — run the scenario in a loop, e.g. ≥10× green, or the full suite ≥3× green).
- Host-checks green.
- If the fix changes the shootdown protocol or timeout, keep an assertion/scenario that pins the corrected behavior so it can't silently regress.
- Do **not** simply enlarge the timeout to paper over hypothesis #1 without first ruling it out with the serial evidence.
