# Dispatch: WEDGE-INV-8 — NMI panic-trip state capture via QEMU monitor

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-08
**Type:** Investigation. Out-of-band observation via a fundamentally different mechanism than WEDGE-INV-5/7's GDB stub. The host-side observer fires when the wedge is detected, NOT continuously while the kernel runs; the kernel-side state-dump handler exists only in stash, not on main. **One small tracked code change required** — a `NEO_QEMU_MONITOR_PORT` env-var gate in `xtask` mirroring the `NEO_QEMU_GDB_PORT` gate from WEDGE-INV-7. Beyond that one commit, substrate is read-only.
**Substrate:** Project HEAD `6edb012` (`xtask: optional QEMU GDB stub for investigation use`). Working tree should be clean on tracked files. Both prior instrumentation stashes remain preserved and are NOT used here:

- `stash@{0}: On main: wedge-investigation-2-instrumentation`
- `stash@{1}: On main: h1-experiment-instrumentation`

A new stash will be created for this dispatch (suggested name: `wedge-investigation-8-nmi-watchdog`). It joins the preserved set.

---

## 0. Standing instructions

1. **Substrate is read-only beyond the one xtask commit.** No kernel-code changes on main. The kernel-side NMI handler lives in stash, NOT on main, regardless of how clean it ends up looking. If during the work it appears the NMI vector wiring is genuinely missing-but-needed for kernel correctness independent of this investigation, raise that observation in the findings report; do not silently fold a kernel-correctness change into this investigation dispatch.
2. **Stashes are not popped or dropped.** Both prior stashes remain. The new NMI-watchdog stash is preserved on completion; the backup script (2026-05-08-refined) will pick it up via `refs/stashes/N`.
3. **Backup runs after the xtask commit lands.** The kernel-side stash code does not require its own backup since it lives in the working tree pre-stash; the stash itself is captured by the next backup after it's created.
4. **Findings reports are honest.** Inconclusive captures are reported as inconclusive. The §6 case verdict accepts "no convergent mechanism" as a valid honest outcome.
5. **Container hygiene as before.** Persistent build container is fine; remove transients on completion.

---

## 1. Goal

Capture per-CPU kernel state at the moment the `POST_SMOKE_SMP_SCENARIO` wedge fires, by sending an NMI from the QEMU host-side monitor to each guest CPU on test timeout, and having an in-stash NMI handler dump the kernel's view of that CPU to serial.

The fundamental difference from WEDGE-INV-5/7: GDB-stub-class observation registers callbacks/breakpoints/single-step machinery that perturbs guest emulation paths even when no client is attached; the QEMU monitor is a separate host-side debug shell that only affects the guest at the moment a command is sent. Sending an NMI is a normal LAPIC operation that the guest already handles in its normal interrupt path. There is no "stub on, but unattended" perturbation analogous to what masked WEDGE-INV-5/7.

The end state is either (a) per-CPU state dumps that converge on a mechanism per the §6 case matrix, or (b) a clean negative result that names the kernel-side stash itself as the masking factor (heisenbug at the stash level, similar to WEDGE-INV-2).

---

## 2. Background

WEDGE-INV-5/7 combined: 0/120 boots with QEMU GDB stub on. Under p=0.0267 (the cargo-no-stub baseline), P[0/120] ≈ 3.9%. Strongly below "luck" threshold. The GDB stub is the masking factor, independent of driver path.

WEDGE-INV-6 confirmed: under direct QEMU without stub, the bug shifts signature (`SMOKE_MID_PHASE` instead of `POST_SMOKE_SMP_SCENARIO`) and rate (0.5% instead of 2.67%). The four-signature timing-shift picture is documented in `Plans/PM-NOTE-2026-05-08-wedge-inv-5-6-outcomes-and-find-mechanism-directive.md`.

The GDB-stub mechanism is closed. The QEMU monitor mechanism is structurally different: it does not register guest callbacks, does not change emulation paths, and only affects the guest at the moment a command is sent.

Why an NMI specifically: it is non-maskable, so it fires even if a CPU is in CLI'd context holding a spinlock (which is exactly the failure mode we suspect). Standard interrupt handlers would not fire if the wedged CPU has interrupts disabled.

---

## 3. Tooling

### 3.1 The `xtask` env-var gate (the one tracked code change)

Mirror the `NEO_QEMU_GDB_PORT` pattern from `6edb012`. In `tools/xtask/src/main.rs` (or wherever QEMU args are constructed), add:

```rust
if let Ok(port) = std::env::var("NEO_QEMU_MONITOR_PORT") {
    if !port.is_empty() {
        qemu_args.push("-monitor".to_string());
        qemu_args.push(format!("tcp:127.0.0.1:{},server,nowait", port));
    }
}
```

Constraints:

- Default behavior unchanged when `NEO_QEMU_MONITOR_PORT` is unset or empty.
- TCP-based monitor (not stdio), so the wrapper can connect/disconnect at will.
- `server,nowait` so the monitor is listening but QEMU doesn't block waiting for a client.
- Single focused commit: `xtask: optional QEMU monitor for investigation use`.
- The commit lands BEFORE the §4 loops run.
- Backup runs after.

### 3.2 Kernel-side NMI watchdog (stash only)

Cerebro designs the NMI handler. Required behavior:

- **Wired up to vector 2 (NMI)** in the IDT for all CPUs, including APs.
- **Per-CPU dump on entry**: writes a marker line to serial (e.g., `[ NMI ] cpu=N`), then dumps:
  - Saved RIP, RSP, RFLAGS from the interrupt frame.
  - The current thread's ID, name, state, and which run queue it's associated with.
  - The per-CPU run queue's runnable count and any zombie/exiting threads on this CPU.
  - The global zombie list state (`zombie_count`, head pointer if applicable). Single read; do not acquire locks (the wedge may be lock-related).
  - A short backtrace (e.g., 10 frames) walked from the saved RBP.
- **Serial output via direct port writes**, NOT through any locked logging facility. The wedge target may be in the logging path; locked output would deadlock.
- **Per-CPU output ordering**: the NMI is delivered per CPU by the wrapper, sequentially with a brief delay. The handler does not attempt cross-CPU coordination; if a second NMI arrives while one is being handled on another CPU, the serial output will interleave at line boundaries (acceptable; the wrapper can demux by `cpu=N` markers).
- **Terminate after dump**: end with `cli; hlt` so the CPU stays put and doesn't continue masking other CPUs' dumps. Do NOT call `panic!()` (which itself uses locked logging).
- **Stash discipline**: the entire change lives in one stash entry. Suggested name: `wedge-investigation-8-nmi-watchdog`.

NMI vector wiring: if vector 2 is not currently set up in the IDT, the stash includes that wiring. If it's already set up to a different handler, the stash redirects it to the watchdog dump. Document the choice in the findings report.

### 3.3 Host-side wrapper

Cerebro writes (or extends from WEDGE-INV-5's `wedge_inv5_capture.py`) a wrapper that:

1. Removes prior source serial logs.
2. Launches the test via `cargo neo-test` with `NEO_QEMU_MONITOR_PORT=4444` (or another free port) set.
3. Monitors serial output. If serial has been silent for ≥ N seconds AND the test hasn't completed AND elapsed time > M seconds (calibrate so this fires on the wedge but not on normal boot phases), trigger the NMI sequence:
   - Open a TCP socket to `127.0.0.1:4444` (the QEMU monitor).
   - Send `nmi 0\n` (BSP). Wait ~500ms for the dump to complete.
   - Send `nmi 1\n`, wait ~500ms.
   - Send `nmi 2\n`, wait ~500ms.
   - Send `nmi 3\n`, wait ~500ms.
   - Close socket.
   - Wait an additional 1–2 seconds for any trailing serial output, then kill QEMU.
4. Records the test's serial log (which now contains the NMI dump) to `target/verify/WEDGE-INV-8-state-dump-run-N.serial.log`.
5. On normal PASS / kernel panic with output: records, kills, moves on.

Calibration: the wedge produces silence; calibrate the silence-detection thresholds based on observed normal-boot timing. WEDGE-INV-3+4 logs are reference data.

Wrapper stays in `tools/wedge-investigation/` as scratch (untracked).

---

## 4. Methodology

### 4.1 Pre-flight: 30 boots, stash applied, NMI-on-timeout disabled

Apply the NMI-watchdog stash. Run 30 cargo-driven boots of `smp_per_cpu_scheduler_smoke` at `NEO_SMP=4` with `NEO_QEMU_MONITOR_PORT=4444` set BUT WITHOUT the wrapper triggering NMIs on timeout. We're testing whether the stash code perturbs timing enough to mask the bug.

Classify per the WEDGE-INV-4 §4.2 + `POST_SMOKE_SMP_SCENARIO` distinction.

Expected: at least 1 `POST_SMOKE_SMP_SCENARIO` timeout. (At p=0.0267, P[≥1 in 30] ≈ 56%.)

- **If pre-flight produces ≥1 in-kernel timeout:** proceed to §4.2.
- **If pre-flight produces 0 in-kernel timeouts in 30:** extend to 60. If 0/60: the stash itself perturbs timing (heisenbug at the stash level, like WEDGE-INV-2's instrumented smoke paths). **Stop.** Don't run the main loop. Document the result, ping for direction. The next dispatch likely pivots to Path D (always-on watchdog) or to a code-review-only audit dispatch. Do not silently push through.

### 4.2 Main loop: 200 boots, NMI-on-timeout enabled

200 cargo-driven boots, stash applied, wrapper triggers NMI sequence on serial-silence detection per §3.3.

Expected: ~5 in-kernel timeouts at the historical 2.67% rate, each producing an NMI dump.

Pre-flight timeouts (without NMI trigger) also produce serial logs that can be analyzed for context, even without the NMI dumps.

### 4.3 Runtime budget

Cargo-driven, similar to WEDGE-INV-7: ~30s/boot at steady state. 230 boots ≈ 2 hours. NMI-trigger wait time on each timeout adds ~5s; negligible.

---

## 5. State dump contents

The NMI handler's output per CPU should answer: *what is this CPU doing at this instant, and is there a runnable thread it should be running but isn't?*

**Required (must be present in every successful dump):**

1. CPU index (e.g., `cpu=N` marker on the first line).
2. Saved RIP, RSP, RFLAGS from the interrupt frame.
3. Current thread on this CPU: thread ID, name, state, which run queue.
4. This CPU's run queue: runnable count, presence of zombie/exiting threads.
5. Global zombie state: `zombie_count`, head pointer (single read, no lock).
6. `smp-hog-{0..3}` thread states if identifiable: which are alive, zombie, reaped.

**Best-effort (include if reachable; don't fail the dump if not):**

7. Spinlock states for any locks the handler can identify by symbol.
8. Short backtrace (e.g., 10 frames) walked from the saved RBP. If the stack is corrupt or the frame chain breaks, log what's reachable and stop.
9. Local APIC ID (cross-check against the `cpu=N` marker).

The dump is plain text. The wrapper concatenates per-CPU dumps into one log file; the findings report's job is to synthesize across them.

---

## 6. Decision matrix

After the main loop completes, classify the cross-dump pattern. Pick exactly one and name diverging runs explicitly.

### Case A — Lock spin
One or more CPUs spinning at the same address inside a known lock acquire path. **Next move:** ADR or PM-NOTE naming the lock site, then a fix dispatch.

### Case B — Missing wakeup / parked CPU
One or more CPUs halted (idle/hlt) while a runnable thread exists. **Next move:** dispatch to instrument or audit the wakeup paths.

### Case C — Reap inconsistency
The zombie list, `zombie_count`, and per-CPU current-thread state are mutually inconsistent across captures. **Next move:** PM convergence note re-opening the H1-broader family, dispatch for targeted reap-side audit.

### Case D — Unexpected fault / silent panic
A CPU is in a fault path that didn't reach the serial console pre-NMI. **Next move:** dispatch to fix serial-during-fault, then re-run.

### Case E — All CPUs idle, work outstanding
No CPU is doing useful work but `smp-hog-{1..3}` are unaccounted for. **Next move:** dispatch to instrument scenario worker spawn + scheduler enqueue.

### Case F — No convergent pattern
Captures don't agree across runs. **Next move:** project lead decides whether to widen the dump, increase sample size, or pivot. Do not fabricate convergence.

### Case G — Pre-flight stop (stash perturbs)
0/60 in-kernel timeouts in pre-flight. The stash itself masks. **Next move:** pivot — likely Path D (always-on watchdog with even smaller hot-path footprint), or pivot to code-review-only audit, or PL-directed alternative.

### Case H — NMI-trigger fires but no dump output
The wrapper successfully sent NMIs but the serial log has no `[ NMI ]` markers. The NMI vector isn't wired correctly, or the handler aborted before output. **Next move:** Cerebro debugs the handler in stash; report what was tried and what the failure mode was.

### Case I — NMI dumps captured but wedge not yet wedged
The wrapper triggered on a false-positive silence (e.g., normal boot phase that has long quiet periods). The dumps show normal in-progress kernel state. **Next move:** recalibrate the silence-detection thresholds, re-run.

---

## 7. Output artifacts

1. **State dumps**: `target/verify/WEDGE-INV-8-state-dump-run-N.serial.log` per timeout captured.
2. **Per-run classification table**: in the findings report.
3. **Findings report**: `Plans/WEDGE-INV-8-FINDINGS-2026-05-08.md`.
4. **Tooling artifacts**: in `tools/wedge-investigation/` as scratch (untracked).

The findings report must NOT inline full state dumps; cross-reference them.

---

## 8. Commit policy

**One permitted commit:** the `xtask` `NEO_QEMU_MONITOR_PORT` gate.

- Single, focused message: `xtask: optional QEMU monitor for investigation use`.
- Default OFF.
- Minimal scope.
- Lands BEFORE the §4 loops run.
- Backup runs after.

The kernel-side NMI watchdog goes into a STASH, not a commit. Stash is preserved across the dispatch.

If during the work you encounter what looks like a kernel-correctness gap (e.g., NMI vector legitimately not wired up and that's a bug independent of this investigation), surface the observation in the findings report; do not fold a separate kernel commit into this dispatch.

---

## 9. Stop conditions

Stop and ping for direction in any of these cases:

- **§4.1 pre-flight produces 0/60 in-kernel timeouts.** Stash masks; don't run main loop.
- **The NMI handler crashes or triple-faults the guest.** Tooling problem; surface, don't power through.
- **A novel signature appears.** Capture, classify, ping; don't bury under the loop.
- **A kernel panic appears in any boot's serial log** with content we haven't seen before. Higher-priority signal than timeouts.
- **The wrapper's silence-detection produces high false-positive rate** (Case I in §6, repeating). Recalibrate, don't ship noisy data.
- **Per-boot runtime materially diverges** from expected (>3× the WEDGE-INV-7 baseline).

In any stop condition, write what you have to the findings report so it survives the chat boundary.

---

## 10. Cross-references

- `Plans/DISPATCH-2026-05-08-wedge-investigation-7-cargo-stub-cerebro.md` — the §3.1.3 path for the env-var-gate pattern.
- `Plans/WEDGE-INV-7-FINDINGS-2026-05-08.md` — the 0/60 result that closed GDB-stub-class observation.
- `Plans/WEDGE-INV-5-FINDINGS-2026-05-08.md`, `Plans/WEDGE-INV-6-FINDINGS-2026-05-08.md` — context for the four-signature timing-shift picture.
- `Plans/PM-NOTE-2026-05-08-wedge-inv-5-6-outcomes-and-find-mechanism-directive.md` — project-state framing including PL's "find the mechanism" directive.
- `Plans/PM-NOTE-2026-05-08-wedge-inv-4-case-iii-verdict.md` — the four-signature picture.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the active commitment this investigation continues to inform.
- `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md` §5.1 — canonical classifier.
- `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md` — H1 narrow refutation; the lock-free zombie_count short-circuit is the prime candidate for Case A or Case C.

---

**End of dispatch.**

*Different observation mechanism than WEDGE-INV-5/7. The QEMU monitor is host-side debug shell, not guest emulation perturbation. NMI is non-maskable, fires through CLI'd context. If this still masks, we know the heisenbug is at a deeper level than observation choice and a code-review-only audit is the next move.*
