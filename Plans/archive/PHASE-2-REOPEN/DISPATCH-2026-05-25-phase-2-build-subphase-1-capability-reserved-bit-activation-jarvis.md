# DISPATCH: Phase 2 BUILD subphase 1 - capability reserved-bit activation

- From: PM
- To: Jarvis/Codex
- Date: 2026-05-25
- Lane: Kernel implementation + tests
- Status: READY FOR PL ROUTING
- Ratification gate: `Plans/PL-RATIFICATION-2026-05-25-phase-2-plan.md`
- PLAN source: `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md` (`e6a4cdf`)
- PM acceptance: `Plans/PM-DISPOSITION-2026-05-25-phase-2-plan-substance-close.md` (`b7281d0`)

---

## 1. Mission

Implement Phase 2 BUILD subphase 1: capability reserved-bit activation.

The milestone is narrow:

- `sys_wait` is implemented and rights-gated by WAIT on a `ProcessHandle`.
- `sys_inspect` is implemented and rights-gated by INSPECT on a `ProcessHandle`.
- `sys_close` for process/channel capability close remains deferred; do not activate the CLOSE bit in this subphase.
- The exact audit (b) scenario `INV-CAP-11` is added or explicitly confirmed as already covered under the exact scenario name `cap_revoke_spawn_inheritance_descendants`.

This dispatch is implementation work only. It does not reopen THINK v0.2 or PLAN choices.

## 2. Authoritative Inputs

Read these before editing:

- `Plans/PL-RATIFICATION-2026-05-25-phase-2-plan.md` - PL ratification of this four-subphase PLAN.
- `Plans/PM-DISPOSITION-2026-05-25-phase-2-plan-substance-close.md` section 2.2 - downstream BUILD clarifications.
- `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md` - subphase 1 scope, gates, anti-scope.
- `Plans/PHASE-2-THINK-v0.2-2026-05-24.md` - ratified answered dispositions for Q1, Q2, and Q3.
- `Plans/PM-DISPOSITION-2026-05-24-phase-2-think-v0.2-substance-close.md` - PM acceptance of THINK substance.

Likely source surfaces:

- `kernel/src/arch/x86_64/syscall.rs`
- `kernel/src/sched/process.rs`
- `kernel/src/sched/scheduler.rs`
- `userland/libneo/src/lib.rs`
- `userland/neo-init/src/main.rs`
- `kernel/src/test_harness.rs`
- `tools/xtask/src/main.rs`

Use live source as truth. The list above is a routing map, not a substitute for reading the implementation.

## 3. Required Implementation

### 3.1 Syscall ABI

Add kernel and libneo syscall numbers for:

- `SYS_WAIT = 19`
- `SYS_INSPECT = 20`

Add dispatcher arms and libneo wrappers. Keep existing file `sys_close` behavior untouched; do not introduce a process/channel/capability close syscall in this subphase.

For `sys_wait`, include a flags argument so the ratified WNOHANG-style polling path is available. The return convention must be unambiguous: a running WNOHANG result must not collide with any valid process exit code. If the implementation uses an errno such as `-EAGAIN` for "still running", add/document that errno path consistently in kernel and libneo. If a different encoding is chosen, document it in the close report and prove it cannot collide with all `i32` exit codes.

For `sys_inspect`, return a fixed value-type snapshot only. A copied-out `#[repr(C)]` struct is acceptable if that best fits the existing syscall/user-copy style. The snapshot may contain scalar state such as process state, exit code if available, thread count under D13, handle/channel counts, and a bounded process name. It must not contain `Handle`, `CapId`, pointers, cap inventory identities, or any value that can become authority.

### 3.2 Rights And Handle Semantics

Do not reuse the current kill-only lookup path blindly. WAIT and INSPECT need distinct semantic-right checks on `ProcessHandle`:

- wrong handle type -> `-EBADF`
- missing WAIT/INSPECT semantic right -> `-EPERM`
- stale or absent target process/exit record -> `-ESRCH` or the repo's closest existing stale-target convention

The meta right remains USE. The semantic right is operation-specific:

- `sys_wait` requires WAIT
- `sys_inspect` requires INSPECT
- existing `sys_kill` continues to require KILL

### 3.3 Wait Behavior

Implement the ratified behavior:

- Blocking wait parks the caller until the target exits.
- WNOHANG-style wait returns immediately with either the exit result or a non-colliding "still running" result.
- Waiting after the target already exited returns the cached exit code and consumes the completion slot.
- Multiple waiters are first-consumer-wins. The first waiter receives the cached exit code; later waiters against the same completed target observe the stale/no-result condition, expected as `-ESRCH`.

The blocking path must not busy-poll. It should compose with the existing scheduler blocking/wake machinery, avoid heap allocation under scheduler locks, and avoid missed wakeups between checking target state and parking the caller.

### 3.4 Inspect Behavior

Implement thin inspection:

- Running target returns a Running state and live scalar counts.
- Exited target returns Exited state and the cached exit code if still available.
- Missing INSPECT right returns `-EPERM`.
- The returned snapshot is observational only and must preserve Bible Article 12.2 no-ambient-authority discipline.

Do not expose cap identities. A count is observable; identities are not.

### 3.5 INV-CAP-11

PM clarification section 2.2 is binding here.

Add or explicitly confirm the exact audit (b) scenario:

- `cap_revoke_spawn_inheritance_descendants`

Expected property: revoke of a parent's root cap reaches a grandchild cap inherited through `sys_spawn` derived-copy inheritance. If an existing scenario is truly equivalent, the close report must name it, explain the spawn-inheritance chain it covers, and state whether a rename/alias was added so the audit chain can find `cap_revoke_spawn_inheritance_descendants` by exact name.

## 4. Required Tests

Add or wire scenarios covering:

- `sys_wait_blocking`
- `sys_wait_wnohang`
- `sys_wait_already_exited`
- `sys_wait_multiple_waiters`
- `sys_inspect_running`
- `sys_inspect_exited`
- `sys_inspect_rights_gate`
- `cap_revoke_spawn_inheritance_descendants`

Use the existing NEO_TEST PASS/FAIL conventions and the existing feature/scenario routing in `neo-init`, `test_harness`, and `xtask`.

## 5. Acceptance Gates

Minimum gates before close report:

- `cargo k` PASS.
- All new wait/inspect scenarios PASS.
- Exact `cap_revoke_spawn_inheritance_descendants` scenario PASS, or exact-equivalent evidence is filed as described in section 3.5.
- Full `cap_*` regression suite PASS at NEO_SMP=4.
- No `KERNEL PANIC`, no `BUG:`, and no new frame leak regression in the exercised scenarios.

If the local host cannot run the usual cargo path directly, use the repo's standard container path and record the exact command. Do not substitute compile-only evidence for scenario evidence unless blocked, and if blocked, name the blocker exactly.

## 6. Anti-Scope

Do not do any of the following in this subphase:

- activate `sys_close` for process/channel/capability close
- claim full FM-D14 structural retirement through handle-presence-gated slot lifetime
- implement PID recycling or stale handle revocation beyond this subphase's wait/inspect behavior
- start ADR-016 A2 scheduler ring work
- start ADR-019 B/C lock migration
- add the tiered perf harness or bare-metal procedure execution
- fold in NVMe FLUSH durability diagnosis
- broaden into unrelated cap, scheduler, IPC, or filesystem refactors

## 7. Close Report

File a close report under `Plans/` after implementation.

The close report must include:

- commit id(s)
- files changed
- syscall ABI chosen for WNOHANG/still-running and inspect snapshot
- exact rights/error behavior
- exact `INV-CAP-11` handling
- verification commands and PASS/FAIL results
- known blockers or caveats
- explicit statement that `sys_close` remains deferred

## 8. Tell-Jarvis Text

Jarvis - Phase 2 BUILD subphase 1 is ratified and ready. Please implement capability reserved-bit activation per `Plans/DISPATCH-2026-05-25-phase-2-build-subphase-1-capability-reserved-bit-activation-jarvis.md`. Scope is `sys_wait`, `sys_inspect`, and exact `INV-CAP-11` handling only; keep `sys_close` deferred. Read the PL PLAN ratification and PM disposition section 2.2 first, then implement and verify the listed scenarios. Close with a Plans close report including ABI choices, rights/error behavior, test evidence, and the explicit `sys_close` deferral statement.
