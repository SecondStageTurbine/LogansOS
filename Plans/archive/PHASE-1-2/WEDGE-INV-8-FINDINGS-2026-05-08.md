# WEDGE-INV-8 Findings: NMI Watchdog via QEMU Monitor

**Date:** 2026-05-08  
**Substrate:** `3b3b4db xtask: optional QEMU monitor for investigation use`  
**Kernel instrumentation:** stashed as `stash@{0}: On main: wedge-investigation-8-nmi-watchdog`  
**Scenario:** `smp_per_cpu_scheduler_smoke`  
**CPU count:** `NEO_SMP=4`  
**Driver path:** standard `cargo neo-test smp_per_cpu_scheduler_smoke`  
**Observer:** QEMU HMP monitor over TCP, NMI sent only after serial-silence detection

---

## Summary

WEDGE-INV-8 captured the target wedge without the GDB-stub masking seen in WEDGE-INV-5/7.

Clean results:

| phase | boots | PASS | timeouts | target captures |
|---|---:|---:|---:|---:|
| Preflight, NMI disabled | 30 | 29 | 1 | 0 |
| Main, NMI on silence | 200 | 196 | 4 | 4 |

Verdict: **Case A - Lock spin.**

All four captured target wedges interrupt BSP CPU 0 at the same relocated instruction: `static 0xffffffff8007bc22`, resolving to `core::core_arch::x86::sse2::_mm_pause`, immediately inside `spinning_top::spinlock::RawSpinlock::lock`. In every capture, CPU 0's scheduler lock is observed as held and the NMI snapshot cannot `try_lock` it.

This is a real convergence: the wedge is not an all-idle condition, not a silent fault, and not a missing serial frame. The BSP is actively spinning in a lock acquire path after the SMP smoke workers have been spawned and only partially reaped.

---

## Methodology

Tracked commit:

- `3b3b4db xtask: optional QEMU monitor for investigation use`
- Adds `NEO_QEMU_MONITOR_PORT`, default off.
- Build verification before commit: `docker compose run --rm neo cargo k` passed with the existing 18 kernel warnings.
- Backup after commit: `Z:\AltimitNeo\2026-05-08-195801`, preserving the two pre-existing stashes.

Stash-only kernel instrumentation:

- Existing NMI vector was already wired in `idt.rs` on `NMI_IST_INDEX`; the stash replaces the default fatal NMI handler with the watchdog dump.
- Direct UART output bypasses the serial mutex.
- Scheduler inspection uses nonblocking `try_lock`; it does not spin inside the NMI path.
- Handler halts the CPU after dumping.

Final clean wrapper behavior:

- Uses per-phase/per-run monitor ports to avoid TCP port reuse after killed QEMU instances.
- Uses HMP `cpu N` followed by `nmi`; `nmi 0` is invalid in QEMU HMP.
- Triggers after `min_elapsed_s=30` and `silence_s=5` once in-kernel smoke activity has appeared.

Discarded tooling attempts:

- A fixed monitor port produced invalid `EMPTY rc=1` launches after the first timeout because QEMU could not bind `127.0.0.1:4444`. Those artifacts were discarded and the clean run was restarted with per-run ports.
- The initial monitor command `nmi 0` was rejected by HMP as `extraneous characters at the end of line`; corrected to `cpu N` then `nmi`.
- AP-target probe showed HMP CPU selection changes `info cpus` default CPU, but AP-side `[ NMI ]` frames still did not appear. Final captures therefore include BSP RIP/RSP plus all scheduler queue snapshots as read by BSP, not AP RIP/RSP.

---

## Per-Run Classification

Compact table; full raw table is in `target/verify/WEDGE-INV-8-results.jsonl`.

| phase | run(s) | classification | signature | dump-file |
|---|---|---|---|---|
| preflight | 1-12 | PASS |  |  |
| preflight | 13 | TIMEOUT_IN_KERNEL_SMOKE | SMOKE_COMPLETION | none; NMI disabled |
| preflight | 14-30 | PASS |  |  |
| main | 1-23 | PASS |  |  |
| main | 24 | TIMEOUT_IN_KERNEL_SMOKE | POST_SMOKE_SMP_SCENARIO | `target/verify/WEDGE-INV-8-state-dump-run-24.serial.log` |
| main | 25-40 | PASS |  |  |
| main | 41 | TIMEOUT_IN_KERNEL_SMOKE | POST_SMOKE_SMP_SCENARIO | `target/verify/WEDGE-INV-8-state-dump-run-41.serial.log` |
| main | 42-104 | PASS |  |  |
| main | 105 | TIMEOUT_IN_KERNEL_SMOKE | POST_SMOKE_SMP_SCENARIO | `target/verify/WEDGE-INV-8-state-dump-run-105.serial.log` |
| main | 106-128 | PASS |  |  |
| main | 129 | TIMEOUT_IN_KERNEL_SMOKE | POST_SMOKE_SMP_SCENARIO | `target/verify/WEDGE-INV-8-state-dump-run-129.serial.log` |
| main | 130-200 | PASS |  |  |

Main-loop timing:

- Minimum elapsed: `5.929s`
- Maximum elapsed: `34.503s`
- Mean elapsed: `8.154s`
- NMI captures fired at about `34.4-34.5s`, before xtask's 180s timeout.

---

## Capture Synthesis

All four captured target wedges converge on CPU 0 spinning in a lock acquire path:

| run | relocated BSP RIP | static RIP | resolved site | CPU0 scheduler lock | completed SMP worker reaps |
|---:|---|---|---|---|---|
| 24 | `0xffffffffb68fcc22` | `0xffffffff8007bc22` | `RawSpinlock::lock` / `_mm_pause` | locked, `try_lock_failed` | `smp-hog-1` only |
| 41 | `0xffffffffdf517c22` | `0xffffffff8007bc22` | `RawSpinlock::lock` / `_mm_pause` | locked, `try_lock_failed` | `smp-hog-0`, `smp-hog-1`, `smp-hog-2` |
| 105 | `0xffffffffb56f2c22` | `0xffffffff8007bc22` | `RawSpinlock::lock` / `_mm_pause` | locked, `try_lock_failed` | `smp-hog-0`, `smp-hog-1` |
| 129 | `0xffffffff985ffc22` | `0xffffffff8007bc22` | `RawSpinlock::lock` / `_mm_pause` | locked, `try_lock_failed` | `smp-hog-0`, `smp-hog-1` |

Common scheduler state:

- `zombie_count=0` for all readable per-CPU schedulers at dump time.
- AP schedulers report only their bootstrap threads (`ap-N-main`) as current, with ready and zombie queues empty.
- CPU 0 scheduler inner is locked and unavailable to the NMI snapshot in all target captures.
- Outstanding `smp-hog-*` work is not visible in AP queues. The missing workers differ by run, but the BSP lock-spin site is identical.

Address resolution:

- KASLR slide was removed per dump using the `limine: executable ... virt=...` line.
- All four BSP RIPs normalize to `0xffffffff8007bc22`.
- `addr2line -Cfipe target/x86_64-altimit/debug/kernel 0xffffffff8007bc22` resolves to `_mm_pause`; surrounding `objdump` shows the pause is in `spinning_top::spinlock::RawSpinlock::lock`.

---

## Decision Matrix

Selected: **Case A - Lock spin.**

Reason:

- The BSP is not idle; it is executing `pause` in `RawSpinlock::lock`.
- The site is identical across all four target captures after KASLR normalization.
- The wedge appears after partial SMP worker reaping, not before worker dispatch.
- The visible zombie counts are not contradictory; the unreadable state is specifically behind CPU 0's locked scheduler inner.

Not selected:

- **Case B:** no parked CPU with visible runnable work was captured; AP queues are empty.
- **Case C:** no visible zombie-count inconsistency; the blocked visibility is the locked CPU0 scheduler state.
- **Case D:** no silent fault path appears in the BSP dump.
- **Case E:** CPU 0 is spinning, not idle.
- **Case F:** the four captures converge.
- **Case H:** the initial HMP syntax was broken, but after correction the BSP NMI handler produced dumps. AP NMI delivery remains a limitation, not a total handler failure.

---

## Recommendation

Next move: targeted lock-site identification.

The current NMI dump proves the mechanism family but not the exact lock address, because the `extern "x86-interrupt"` handler does not preserve interrupted GPRs. The missing value is the lock pointer in the interrupted call path, likely in or near `rdi` at `RawSpinlock::lock`.

Recommended next dispatch:

- Add a stash-only assembly NMI entry stub that saves GPRs before calling Rust dump code.
- Dump `rdi`, `rsi`, `rdx`, `rcx`, `r8-r11`, and compare `rdi` against known lock addresses.
- Specifically audit scheduler/reap paths where CPU 0 can hold `PerCpuScheduler.inner` while attempting another spin lock, or can re-enter a scheduler lock acquire after partial worker reaping.

The likely fix is not in the observer. The observer now shows a stable lock-spin mechanism under the original cargo-driven timing profile.

---

## Artifacts

Primary:

- `target/verify/WEDGE-INV-8-results.jsonl`
- `target/verify/WEDGE-INV-8-preflight-1-30.runner.log`
- `target/verify/WEDGE-INV-8-main-1-200.runner.log`
- `target/verify/WEDGE-INV-8-state-dump-run-24.serial.log`
- `target/verify/WEDGE-INV-8-state-dump-run-41.serial.log`
- `target/verify/WEDGE-INV-8-state-dump-run-105.serial.log`
- `target/verify/WEDGE-INV-8-state-dump-run-129.serial.log`

Probe artifacts:

- `target/verify/WEDGE-INV-8-monitor-probe.hmp.log`
- `target/verify/WEDGE-INV-8-monitor-probe.serial.log`
- `target/verify/WEDGE-INV-8-monitor-probe-cpu1.hmp.log`
- `target/verify/WEDGE-INV-8-monitor-probe-cpu1.serial.log`

Scratch tooling:

- `tools/wedge-investigation/wedge_inv8_nmi_watchdog.py`

Stash:

- `stash@{0}: On main: wedge-investigation-8-nmi-watchdog`

---

## Cross-References

- `Plans/DISPATCH-2026-05-08-wedge-investigation-8-nmi-watchdog-cerebro.md`
- `Plans/WEDGE-INV-7-FINDINGS-2026-05-08.md`
- `Plans/WEDGE-INV-5-FINDINGS-2026-05-08.md`
- `Plans/WEDGE-INV-6-FINDINGS-2026-05-08.md`
- `Plans/PM-NOTE-2026-05-08-wedge-inv-5-6-outcomes-and-find-mechanism-directive.md`
