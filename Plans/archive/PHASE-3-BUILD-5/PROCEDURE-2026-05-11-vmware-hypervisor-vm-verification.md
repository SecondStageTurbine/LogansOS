# PROCEDURE — VMWare Workstation hypervisor-VM verification of Phase 2 BUILD-3

**Date filed:** 2026-05-11
**Author:** PM
**Type:** Manual procedure document for PL direct execution. Not a dispatch — no agent involvement. PL has VMWare Workstation hardware availability; PM provides procedure + verification template; PL runs at convenient cadence.
**Scope:** Hypervisor-VM verification of Phase 2 BUILD-3 SMP correctness on origin/main HEAD `64a92e1`. **Explicitly labeled "hypervisor-VM verification" not "bare-metal verification"** — VMWare Workstation uses VT-x/AMD-V hardware virtualization (substantially closer to bare-metal than QEMU TCG software emulation, but still a hypervisor environment). The Bible Article 13 bare-metal mandate stays formally open for if/when actual bare hardware becomes available.
**Status:** Procedure ready for execution. Estimated 2-4 hours wall-clock including build + boot + scenario sweep + report.

---

## 1. Why hypervisor-VM verification

QEMU TCG (the current standard test environment) is software emulation. VMWare Workstation uses hardware virtualization with VT-x/AMD-V. The two environments differ in:

- **CPU instruction execution.** QEMU TCG dynamically translates guest instructions; VMWare runs them directly on the physical CPU. Bugs in instruction-sequence-specific behavior (memory ordering edge cases, FPU/SSE state transitions, exotic interrupt timings) may surface in VMWare and not QEMU.
- **Memory model.** QEMU TCG's memory model is permissive (effectively sequential consistency); VMWare exposes the real x86_64 TSO memory model. SMP correctness issues that QEMU TCG happens to mask are observable under VMWare.
- **APIC and timing.** QEMU emulates LAPIC; VMWare passes through (mostly). Real TSC, real APIC timer behavior.
- **Performance characteristics.** VMWare runs full kernel speed; QEMU TCG runs ~10-50x slower. This changes timing-sensitive race surfaces.

Phase 2 BUILD-3's SMP correctness work (SC-3B / SC-3C / SC-4) was all verified under QEMU TCG. Hypervisor-VM verification is a valuable additional gate — it catches different bugs than QEMU TCG.

It is NOT bare-metal verification. The Bible Article 13 bare-metal mandate stays open. If a spare laptop or PC becomes available, a bare-metal pass is still wanted.

## 2. Prerequisites

PL host system requirements:
- VMWare Workstation Pro or Player installed (any recent version).
- VT-x or AMD-V enabled in host BIOS.
- ~2 GB free disk space (kernel ISO + minimal VM config).
- The NeoPlutarch source tree at `E:\AI Automation\AltimitNeo\` (or wherever the project lives) accessible.

No additional software needed; the existing `cargo iso` build target produces a bootable ISO.

## 3. Procedure

### Step 1 — Verify origin/main HEAD

```powershell
cd 'E:\AI Automation\AltimitNeo'
git fetch origin
git log --oneline origin/main -3
git rev-parse origin/main
```

Expected HEAD at procedure execution time: `64a92e1` (Q9 deferral documentation) or later if subsequent commits have landed. Note the actual HEAD for the verification report.

### Step 2 — Build the kernel ISO

```powershell
docker compose run --rm neo cargo iso
```

The build produces `target/iso/NeoPlutarch.iso` (or similar path; verify in build output).

Expected build time: 2-5 minutes depending on cache state.

Note any warnings or non-fatal output for the verification report.

### Step 3 — Create VMWare VM

VM specifications:
- **Type:** Custom VM
- **OS Type:** Other → Other 64-bit
- **CPU count:** 4 (matches QEMU `-smp 4` baseline; can also test 1 and 8 for triangulation)
- **Memory:** 1024 MB (matches QEMU baseline; can be increased)
- **Disk:** Not strictly needed (kernel doesn't write to disk in Phase 2). Optionally create a 1 GB virtual disk for completeness; kernel will ignore it.
- **Network:** Not needed (Phase 4+ feature).
- **CD/DVD:** Mount the kernel ISO from Step 2 as bootable.
- **Display:** Standard, no acceleration needed (Phase 0 framebuffer suffices).
- **Sound:** Disable.
- **USB controller:** Disable (Phase 4+).
- **Serial port:** Add an output-to-file serial port (point at, e.g., `vmware-serial.log`). Critical — kernel println goes to serial, and the procedure depends on capturing that output.

### Step 4 — Boot the VM, capture serial output

Start the VM. Observe:
- BIOS / UEFI handoff to Limine.
- Limine kernel boot.
- Kernel boot logo on display.
- Kernel boot logs on serial (captured to file per Step 3 config).

**Expected first-boot serial output:**
- Limine boot banner.
- `[ kernel ] NeoPlutarch v...` banner.
- `[ kernel ] arch: BSP CPU online`.
- `[ kernel ] mm: heap...` (allocator init).
- `[ kernel ] traps: BSP IDT loaded`.
- `[ kernel ] per_cpu: cpu_id=0 gs:[24]=0` (BSP GS-base sentinel).
- AP discovery + bring-up: `[ kernel ] sched: AP cpu_id=N online` for each AP.
- `[ kernel ] sched: smp_test PASS` or equivalent multi-CPU smoke output.
- `[ neo-init ] ...` userspace init.
- `[ neo-init ] neo-init exit code 0` for normal exit.
- `[ kernel ] frame: leak audit clean`.

**Indicators of failure:**
- `KERNEL PANIC` (immediate fail; capture full panic backtrace).
- `BUG:` (test-detected assertion failure; capture context).
- Hang for >60 seconds with no progress on serial.
- AP bring-up failure (only BSP reaches `sched:` lines).
- GS-base sentinel mismatch panic.

### Step 5 — Run scenario sweep

The kernel's `cargo neo-test all` target runs all SMP-relevant scenarios. To run from VMWare:

```powershell
# Build the test-harness-enabled ISO.
docker compose run --rm neo cargo iso --features neo-test

# Mount this ISO instead, boot the VM, observe scenario output.
```

**Expected scenarios PASS:**
- `smp_per_cpu_scheduler_smoke` (sub-commit 2)
- `smp_same_cpu_ipc` (sub-commit 2)
- `smp_cross_cpu_ipc_smoke` (sub-commit 3b = SC-3B)
- `smp_cross_cpu_kill_blocked` (sub-commit 3b)
- `smp_cross_cpu_rendezvous_stress` (SC-3B closure gate)
- `q7b_cr3_cache_skip_count` (SC-3C 4a)
- `q7c_concurrent_munmap_smp` (SC-3C 4c)
- `q7c_process_exit_during_thread_run` (SC-3C 4c)
- `q7c_missing_shootdown_detection` (SC-3C 4c D14 negative-feature)
- `q8a_concurrent_install_revoke` (SC-4 5a D16)
- `q8b_grant_vs_exit_race` (SC-4 5b)
- `q8b_exit_state_cas` (SC-4 5b)
- `q8c_blocked_kill_walk` (SC-4 6 D17)
- `q8c_close_peer_my_side_extract` (SC-4 6)
- `q8d_phase_a_coverage` (SC-4 5b D18)
- All cap_* tests (BUILD-1 + SC-3B + SC-4)

Tally PASS/FAIL per scenario. Any FAIL is significant — record fully for triage.

### Step 6 — Specific stress runs

Run these in addition to the standard sweep:

**6.a — 100x `ipc_heap_kill_smoke`:** The sub-commit 2 gate that originally surfaced the Q5 deferred-quantum race. Re-running under VMWare's different memory model + timing may surface different behavior. Expected: 100/100 PASS. Any flake is significant data.

**6.b — 200x `smp_cross_cpu_rendezvous_stress`:** The SC-3B / SC-3C closure stress gate, doubled. Expected: 200/200 PASS.

**6.c — D14 negative-feature scenarios:** Run with `neo-test-missing-shootdown-negative` feature, then with `neo-test-missing-cap-lock-negative` feature. Expected: both DETECT their respective failure modes (gates PASS by detection).

### Step 7 — Frame audit + crash residue checks

After each scenario run completes, observe in serial:
- `frame: leak audit clean` line.
- No `KERNEL PANIC` after init lines.
- No `BUG:` lines after init.
- For test-harness runs: scenario-specific PASS markers.

### Step 8 — Optional: 1-CPU and 8-CPU runs for triangulation

Re-run Steps 4-7 with VM CPU count set to 1 (verifies non-SMP-regression) and 8 (verifies scaling beyond the standard `-smp 4`). Optional but useful for triangulation if any FAIL surfaces in the 4-CPU run.

### Step 9 — File the verification report

Create `Plans/PHASE-2-BUILD-3-VMWARE-HYPERVISOR-VM-VERIFICATION-<date>.md` matching the template in §4 below.

## 4. Verification report template

```markdown
# Phase 2 BUILD-3 Hypervisor-VM Verification (VMWare Workstation)

**Date:** YYYY-MM-DD
**Executor:** PL (SecondStageTurbine)
**Origin/main HEAD at verification:** <SHA>
**Verification environment:** VMWare Workstation <version>, host OS <Windows version>, host CPU <model>, host RAM <GB>, VT-x/AMD-V <enabled>.
**Status:** PASS | PASS-WITH-INCIDENTAL | FAIL (with summary)

## 1. Build

- `docker compose run --rm neo cargo iso` — <PASS / FAIL>. Build time: <minutes>. Warnings: <list or "none">.
- `cargo iso --features neo-test` — <PASS / FAIL>. Build time: <minutes>. Warnings: <list or "none">.

## 2. Boot sequence (no test harness)

VM config: 4 vCPU, 1024 MB RAM, ISO mounted.

Observed serial output highlights:
- Limine handoff: <observed timestamps / yes/no>
- Kernel banner: <observed>
- BSP online: <observed>
- AP bring-up: <count of APs that came online>
- neo-init exit code: <code>
- frame: leak audit: <clean / leaks>
- KERNEL PANIC: <none / list>
- BUG:: <none / list>

## 3. Standard scenario sweep (test harness)

| Scenario | PASS | FAIL | Notes |
|---|---|---|---|
| smp_per_cpu_scheduler_smoke | | | |
| smp_same_cpu_ipc | | | |
| smp_cross_cpu_ipc_smoke | | | |
| smp_cross_cpu_kill_blocked | | | |
| smp_cross_cpu_rendezvous_stress | | | |
| q7b_cr3_cache_skip_count | | | |
| q7c_concurrent_munmap_smp | | | |
| q7c_process_exit_during_thread_run | | | |
| q7c_missing_shootdown_detection | | | |
| q8a_concurrent_install_revoke | | | |
| q8b_grant_vs_exit_race | | | |
| q8b_exit_state_cas | | | |
| q8c_blocked_kill_walk | | | |
| q8c_close_peer_my_side_extract | | | |
| q8d_phase_a_coverage | | | |
| (cap_* family — list each individually or summarize) | | | |

## 4. Stress runs

- 100x `ipc_heap_kill_smoke`: <count> PASS / <count> FAIL.
- 200x `smp_cross_cpu_rendezvous_stress`: <count> PASS / <count> FAIL.
- D14 `neo-test-missing-shootdown-negative` detection: <PASS by detection / FAIL>.
- D16 `neo-test-missing-cap-lock-negative` detection: <PASS by detection / FAIL>.

## 5. Triangulation (optional)

- 1-CPU run: <PASS / FAIL>.
- 8-CPU run: <PASS / FAIL>.

## 6. Incidental observations

(Anything noted during verification that's not strictly a PASS/FAIL: log line oddities, timing differences vs QEMU, performance observations, etc.)

## 7. Conclusion

(One paragraph: overall verdict; comparison to QEMU TCG baseline; any items needing PM triage.)
```

## 5. What constitutes a PASS

Hypervisor-VM verification PASSes if:

- Build succeeds on both ISO variants.
- Boot sequence completes cleanly (frame audit clean; no panic; no BUG; no hang).
- Standard scenario sweep all PASS.
- Stress runs 100/100 + 200/200 PASS (or within 1 flake which gets surfaced for analysis).
- D14 + D16 negative-feature gates detect their failure modes.

If all of the above hold, Phase 2 BUILD-3 hypervisor-VM verification is complete. The Bible Article 13 bare-metal mandate stays open (separate workstream when hardware becomes available); the hypervisor-VM pass is a useful intermediate gate.

## 6. What constitutes a FAIL

Any of:

- Build failure.
- Boot hang / panic / BUG.
- Standard scenario sweep showing FAIL not previously seen on QEMU TCG.
- Stress run flake rate > 1% (i.e., more than 1/100 ipc_heap_kill_smoke flakes, or more than 2/200 smp_cross_cpu_rendezvous_stress flakes).
- Negative-feature gate failing to detect its failure mode.

On FAIL: PL surfaces the verification report. PM triages — may be a new SMP correctness issue surfaced by VMWare's different memory model / timing characteristics, or may be a hypervisor-config issue (insufficient resources, misconfigured serial, etc.).

## 7. Likely failure modes and triage hints

If FAIL surfaces, common categories:

**Build failure** — likely cause: Docker / cargo / network. Re-run; if persistent, surface to PM with build log.

**Hang at AP bring-up** — VMWare's APIC / SIPI handling differs from QEMU. Check serial log for which AP didn't come online; PM triages whether it's a real kernel bug or a config issue. The kernel-rename bundle's sub-commit 1b (AP bring-up) is the relevant code surface.

**Hang at first SMP scenario** — possible memory-model issue. SC-3B Q6 ManuallyDrop + asm release was identified as the highest-correctness-risk work in BUILD-3. If a hang surfaces here, the `context_switch_with_release` asm wrapper at `kernel/src/sched/scheduler.rs` is the first suspect.

**Sporadic FAIL on stress runs** — possible race-condition surface that QEMU TCG masked. Note exact scenario + frequency; PM triages.

**ipc_heap_kill_smoke flake** — would re-surface the Q5 issue that the deferred-quantum-restore amendment supposedly closed. Significant if it recurs; PM dispatches Cerebro for memory-ordering review if so.

## 8. Cross-references

**Phase 2 BUILD-3 closure context:**
- `Plans/CLOSE-REPORT-2026-05-11-sc-3c.md`
- `Plans/CLOSE-REPORT-2026-05-11-sc-4.md`
- `Plans/Q9-EXPLORATION-2026-05-11.md` §5.1 (this verification scheduled non-blocking).

**Bible mandate:**
- Article 13: cross-cutting verification "Bare-metal multi-CPU verification at BUILD-3 close." Hypervisor-VM is partial coverage; bare-metal stays open.

**Test scenarios anchored:**
- `Plans/DISPATCH-2026-05-11-sc-3c-plan-stage-jarvis.md` — q7b/q7c scenarios.
- `Plans/DISPATCH-2026-05-11-sc-4-plan-stage-jarvis.md` — q8a/b/c/d scenarios.
- ADR-013 §Q5 amendment — ipc_heap_kill_smoke gate semantics.

---

**End of procedure document.**

*Manual procedure for PL direct execution. Hypervisor-VM verification of Phase 2 BUILD-3 SMP correctness; bare-metal mandate stays open. Estimated 2-4 hours wall-clock. Verification report template provided. PM triages any FAIL.*
