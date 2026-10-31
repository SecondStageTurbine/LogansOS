# HANDOFF: Phase 2 OptiPlex hardware sweep to Jarvis

- **Date:** 2026-05-25
- **From:** PM
- **To:** Jarvis
- **Repo:** `github.com/SecondStageTurbine/NeoPlutarch`
- **Workspace:** `E:\AI Automation\AltimitNeo`
- **Current pushed HEAD:** `2f1aa14c308640484749a17e05b66c2e0c548e96` (`plans: close F-006 bare-metal reverify`)
- **Status:** F-006 closed; Phase 2 hardware scenario sweep remains open
- **Mode:** execution-support / PM handoff; do not make code changes without a follow-up dispatch

---

## 0. Minimum read set

Read these before acting:

1. `Plans/CLOSE-REPORT-2026-05-25-f006-bare-metal-hhdm-coverage.md`
2. `Plans/DISPATCH-2026-05-25-phase-2-hardware-optiplex-7080-verification.md`
3. `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md`
4. `Plans/CLOSE-REPORT-2026-05-25-phase-2-build-subphase-4-formal-close-pre-hardware.md`

Do not restart the F-006 production boot loop. That acceptance gate is already closed by `2f1aa14`.

---

## 1. Current state

- F-006 implementation landed at `59443de`.
- PM accepted it for bare-metal reverify at `43209d3`.
- PL completed 10 consecutive OptiPlex 7080 F-006 reverify boots.
- PM filed and pushed the F-006 close package at `2f1aa14`.
- Evidence files for the 10 F-006 boots are committed under `Plans/evidence/`.
- Local working tree was checked after an interrupted PM command: clean against `origin/main` except pre-existing untracked `ai_digest.py`.
- No Neo build/test container was left running by the interrupted PM command.

One PM mistake to be aware of: PM briefly started `docker compose run --rm neo cargo neo-test --build-only smp_per_cpu_scheduler_smoke`, then PL stopped it. Treat that as aborted setup, not evidence.

---

## 2. Goal

Move Phase 2 bare-metal verification forward after F-006 closure.

The remaining hardware lane is the targeted scenario sweep on the Dell OptiPlex 7080:

1. targeted pass at `>=4` logical CPUs;
2. targeted pass at full logical CPU count;
3. optional full non-ignored sweep if the hardware window allows;
4. hardware perf captures.

Jarvis should support the lane by preparing exact commands, media paths, evidence manifests, and blocker/disposition notes. Physical boot execution still requires PL at the OptiPlex.

---

## 3. Immediate issue: 4-CPU pass ambiguity

Before building or asking PL to boot scenario media, resolve this procedure/tooling mismatch:

- The dispatch requires a targeted pass at `>=4` logical CPUs.
- The procedure says the 12-logical-CPU OptiPlex should run that pass with the kernel CPU-count limit at 4.
- Current source inspection suggests `NEO_SMP` is wired to QEMU `-smp` only in `tools/xtask/src/main.rs`.
- The current scenario ISO builder appears to copy `boot/limine.conf` as-is and does not encode a physical CPU limit into the ISO.
- `kernel/src/arch/x86_64/smp.rs` currently brings up all Limine-reported APs up to `per_cpu::MAX_CPUS`; no obvious boot-arg CPU limiter was found by PM.

Jarvis action:

1. Confirm whether a physical scenario ISO can actually limit the OptiPlex to 4 CPUs with existing tooling.
2. If yes, document the exact command/path and proceed.
3. If no, file a short PM disposition/blocker note, for example:
   `Plans/PM-DISPOSITION-2026-05-25-phase2-hardware-4cpu-pass-ambiguity.md`
4. Do not silently label a full-count 12-CPU boot as the `>=4` CPU pass.
5. Do not implement a CPU-limit feature unless PM/PL issues a separate code-bearing dispatch.

Recommended disposition options if no existing 4-CPU mechanism exists:

- run full-count hardware scenario sweep now, explicitly defer the 4-CPU hardware pass until physical CPU-limit tooling exists;
- or dispatch a narrow implementation task to add a boot-time CPU limit for scenario ISO media, then rebuild scenario media and run both passes.

---

## 4. Required scenario set

The dispatch requires these 43 scenarios at both required CPU-count tiers:

```text
smp_per_cpu_scheduler_smoke
smp_same_cpu_ipc
smp_block_release_smoke
smp_send_blocks_first_then_completes
smp_recv_blocks_first_then_completes
smp_grant_cross_cpu
smp_cross_cpu_migration_reap
smp_cross_cpu_rendezvous_stress
smp_concurrent_reap_after_terminate
q7b_cr3_cache_skip_count
phase2_perf_proxy_baseline
q7c_concurrent_munmap_smp
q7c_process_exit_during_thread_run
q7c_missing_shootdown_detection
q7c_shootdown_timeout_panic
ipc_rendezvous_translate_fault
q8a_concurrent_install_revoke
q8b_grant_vs_exit_race
q8b_exit_state_cas
q8c_blocked_kill_walk
q8c_close_peer_my_side_extract
q8d_phase_a_coverage
cap_mint_and_use
cap_derive_subset
cap_derive_rights_violation
cap_derive_missing_meta
cap_grant_basic
cap_grant_missing_meta
cap_revoke_local
cap_revoke_cross_process
cap_revoke_descendants_walk
cap_revoke_spawn_inheritance_descendants
cap_survive_exit
sys_wait_blocking
sys_wait_wnohang
sys_wait_already_exited
sys_wait_multiple_waiters
sys_inspect_running
sys_inspect_exited
sys_inspect_rights_gate
sched_ready_queue_capacity_pressure
irqsafe_mutex_if_restore
irqsafe_mutex_bench
```

Expected-panic exception:

- `q7c_shootdown_timeout_panic` may show `KERNEL PANIC` only if the same log emits `event=PASS expected_panic=tlb_shootdown_timeout`.

---

## 5. Scenario media build shape

For each scenario, the documented build command is:

```powershell
cd "E:\AI Automation\AltimitNeo"
docker compose run --rm neo cargo neo-test --build-only <scenario>
```

Expected ISO path:

```text
target/neo-test/vmware/<scenario>/<scenario>.iso
```

Record SHA256 for every scenario ISO:

```powershell
Get-FileHash -Algorithm SHA256 "target\neo-test\vmware\<scenario>\<scenario>.iso"
```

Do not use `--bare-metal-mode` for this Phase 2 scenario set. That mode is reserved for the Phase 3 AHCI/NVMe storage exemplars.

---

## 6. PL interaction style

PL asked not to be firehosed during manual boot loops. Give one concrete next boot at a time:

1. scenario name;
2. ISO path to flash;
3. SHA256 to record;
4. PuTTY log path to use;
5. exact pass marker to wait for.

Evidence path pattern:

```text
Plans/evidence/phase2-bare-metal-optiplex-7080-<YYYYMMDD-HHMMSS>-<cpu-count>-<scenario>.serial.log
```

Known working serial settings:

```text
COM3 / 38400 / 8N1 / PuTTY "All session output"
```

Each scenario log must be captured before power-on and continue until:

- `NEO_TEST/1 event=PASS name=<scenario>`; or
- `NEO_TEST/1 event=FAIL name=<scenario>`; or
- an unexpected panic, BUG, PAGE_FAULT, frame-audit issue, AP bring-up issue, or hang.

---

## 7. Stop conditions

Stop and file evidence immediately on:

- unexpected `KERNEL PANIC`;
- unexpected `BUG:`;
- `PAGE_FAULT`;
- `event=FAIL`;
- frame leak or dirty-frame audit failure;
- GS-base sentinel mismatch;
- AP bring-up failure;
- boot hang after serial progress.

The only panic exception is `q7c_shootdown_timeout_panic`, and only when paired with the explicit expected-panic PASS sentinel.

---

## 8. Final deliverables expected from Jarvis

At minimum:

1. disposition of the 4-CPU physical-pass ambiguity;
2. scenario ISO manifest with paths and SHA256 hashes for the first execution batch;
3. PL-facing one-step-at-a-time instructions for the first scenario boot;
4. no source changes unless separately dispatched.

After PL returns logs, Jarvis or PM should package:

```text
Plans/PHASE-2-BARE-METAL-OPTIPLEX-7080-VERIFICATION-2026-05-25.md
```

That report must include scenario tables, serial-log scan output, expected-panic handling, perf captures if run, and a narrow conclusion. Do not claim Phase 2 formal hardware close until the required rows are actually satisfied or explicitly dispositioned.

---

## 9. Tell-Jarvis text

Jarvis - F-006 is closed and pushed at `2f1aa14`; do not rerun the F-006 boot loop. Take over Phase 2 OptiPlex hardware sweep support. Read `Plans/CLOSE-REPORT-2026-05-25-f006-bare-metal-hhdm-coverage.md`, `Plans/DISPATCH-2026-05-25-phase-2-hardware-optiplex-7080-verification.md`, and `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md`. First resolve the 4-CPU physical-pass ambiguity: PM saw `NEO_SMP` wired to QEMU only and no obvious ISO/kernel boot CPU limiter, so do not label a 12-CPU boot as the `>=4` pass unless existing tooling proves that is valid. If no mechanism exists, file `Plans/PM-DISPOSITION-2026-05-25-phase2-hardware-4cpu-pass-ambiguity.md` with options; do not implement without a follow-up dispatch. Once that is dispositioned, build scenario ISOs one at a time with `docker compose run --rm neo cargo neo-test --build-only <scenario>`, record SHA256, and give PL one boot instruction at a time using COM3 / 38400 / 8N1 and the evidence path pattern in this handoff. Stop on any unexpected panic, PAGE_FAULT, `event=FAIL`, frame-audit issue, AP issue, or hang. Expected-panic exception only for `q7c_shootdown_timeout_panic` with the explicit expected-panic PASS sentinel.

---

**End of handoff.**
