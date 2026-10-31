<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: OT-22 rung-1 re-check after Jarvis F1

**Seat:** Jarvis (Codex-family)

**Reviewed head:** `581a841ec24a6b788e808097889adcc7a7d780c8`

**Verdict:** **MERGE**

No finding survived re-verification. The handle-identity snapshot closes the scan-to-register rebind window, and the exhaustive handle-removal hooks close the post-scan revoke/close window. I found no channel-handle mutation that bypasses those two halves, no reverse lock edge, and no merge-resolution regression in the requested desktop wiring.

## Closure status

- **Rung-1 F1 (HIGH, mutable numeric handle can strand an IPC-only waiter): CLOSED.** `WaitIdentity` snapshots each channel item's wait key and kernel-unique `CapId`; the post-registration scan compares the complete snapshot and retries on change (`kernel/src/arch/x86_64/syscall.rs:4651-4701`, `:4817-4863`). A later removal fires the old key from every channel-handle removal chokepoint (`kernel/src/sched/process.rs:1282-1292`, `:1357-1366`, `:1404-1427`).
- **Design F1 (BLOCKER, stack-owned `Arc<WaitCell>` across force-zombify): CLOSED.** The build uses a static bounded table containing Copy keys, pid, deadline, token and ticket; the parked frame keeps only a Copy `Registration`. Scans drop resolved object `Arc`s before `park` (`kernel/src/sched/wait_any.rs:62-80`, `kernel/src/arch/x86_64/syscall.rs:4703-4749`).
- **Design F2 (HIGH, one per-endpoint slot can overwrite another waiter): CLOSED.** Registrations occupy independent `WAIT_TABLE` slots, and `fire_key` visits every matching entry (`kernel/src/sched/wait_any.rs:141-169`, `:267-299`). The table supports 16 total and 4 per pid; no per-channel slot is overwritten.
- **Design F3 (HIGH, token-table exhaustion panics and leaves registrations): CLOSED.** `park` uses the fallible `try_block_current_with_token_and_guard`; `Err` returns `EAGAIN`, and the registration is then removed (`kernel/src/sched/wait_any.rs:195-226`, `kernel/src/sched/scheduler.rs:1701-1768`).
- **Design F4 (HIGH, no purge identity / teardown ordering): CLOSED.** Entries carry pid plus generation-like ticket; `purge_pid` removes only that pid's registrations, and forced/fault teardown purges before scheduler zombify, with a second purge closing the re-registration race (`kernel/src/sched/wait_any.rs:334-355`, `kernel/src/sched/process.rs:2791-2804`, `:2970-3068`).
- **Design F5 (MEDIUM, claimed lockdep discriminator did not exist): CLOSED as a design defect.** The addendum retracts the nonexistent mechanical-lockdep claim and lists the actual construction edges (`docs/THINK-2026-10-19-ot22-readiness-multiplex.md:199-214`). Mechanical lockdep coverage remains explicitly named future hardening, not an untrue current guarantee.

## Refutation checks

### 1. Two-sided identity/removal argument

I checked all ways a channel handle slot can change what it names.

- `HandleTable.entries` is private. `install`, `install_minted` and `install_excluding` fill only empty slots; they do not replace a live channel entry (`kernel/src/sched/process.rs:1253-1255`, `:1295-1344`).
- The only removals are `remove`, `remove_cap_ids` and `drain`, and each calls `notify_removed` for a removed channel (`kernel/src/sched/process.rs:1357-1366`, `:1404-1427`).
- The only `get_mut` caller mutates a `TcpSocket`'s rights/shutdown state, not a channel variant, side, object or `CapId` (`kernel/src/arch/x86_64/syscall.rs:3860-3893`).
- Derive, grant and spawn mint new `CapId`s and install into empty/new tables. Spawn inheritance cannot mutate a live slot in the waiting process.

Therefore a mutation before the second scan's handle-table read is observed as a different `(key, CapId)` or as `EBADF`; a mutation after that read must pass a removal chokepoint and fire the already-registered old key. The cloned channel `Arc` held briefly by a scan also prevents address reuse during that readiness check.

### 2. Lock order

The new removal path is handle table -> `WAIT_TABLE`; `fire_key` drops `WAIT_TABLE` before calling `wake_thread_token`, though the outer handle-table guard remains held until removal returns (`kernel/src/sched/wait_any.rs:267-299`). The park path is `WAIT_TABLE` -> scheduler (`kernel/src/sched/wait_any.rs:197-226`, `kernel/src/sched/scheduler.rs:1709-1768`). I found no scheduler-lock -> `WAIT_TABLE` or scheduler-lock -> handle-table acquisition, and no `WAIT_TABLE` -> handle/channel acquisition.

The revoke walk consistently takes handles before derivations. Its discovery pass may hold both, but fires nothing; its removal pass fires while holding handles and takes derivations only after releasing that handle guard (`kernel/src/sched/process.rs:1769-1830`). No derivations -> handle/`WAIT_TABLE` reverse edge was found. Channel readiness continues to use channel state -> `WAIT_TABLE`, with no new handle-table nesting.

### 3. Identity-retry liveness

The retry can be prolonged by a sibling continually rebinding the same process's slot between scans. That sibling already shares the caller's failure and CPU-accounting domain and can invalidate the wait item directly; it is not a cross-principal liveness regression. An ancestor revoke is finite: removal fires once and the next scan returns `EBADF`. I therefore do not treat the deliberately adversarial same-process churn case as a merge finding.

### 4. Rebind/revoke legs and controls

The rebind hook stops the waiter after its first scan and before registration. The sibling removes the named handle, installs a fresh channel through lowest-free allocation, and records a dedicated failure if the returned index differs (`kernel/src/test_harness.rs:21852-21898`, `:22067-22128`). The new sender uses the newly armed channel B. With the identity comparison disabled, the waiter watches A and reaches the dedicated `wait_any_rebind_stranded` failure after its test deadline.

The revoke leg parks on one channel item with an infinite deadline, checks `ipc_wakes=0`, and requires `EBADF`; disabling handle-removal fires reaches `wait_any_revoke_stranded` (`kernel/src/test_harness.rs:22130-22184`). The ancestor root shares the derived handle's key, so its removal can supply the first wake before the descendant table is cleared; that is a real production wake, not vacuity, and the descendant removal hook is independently present in the exhaustive `remove_cap_ids` loop.

### 5. Merge resolutions and addendum

- `neo-test-desktop-idle` now composes `neo-test-desktop-pointer` rather than a scenario `*-boot` feature, preserving exactly one boot discriminator (`kernel/Cargo.toml:569-606`).
- The consolidated `DESKTOP_PASS` selection includes the common idle feature, so both idle scenarios share the intended pass line without colliding with pointer boot (`kernel/src/test_harness.rs:409-434`, `:2627-2635`).
- `DESKTOP_SCENARIOS` contains both idle scenarios and the table-backed helpers feed script selection, QEMU CPU/memory policy and a host consistency test (`tools/xtask/src/main.rs:517-642`, `:5926-5929`, `:13505-13514`).
- The THINK addendum accurately maps design F1-F5 and rung-1 F1 to the implementation and explicitly records the remaining coverage gaps (`docs/THINK-2026-10-19-ot22-readiness-multiplex.md:199-226`).

## Verification

- RAG health: healthy (`ollama=true`, `qdrant=true`), but the index was stale (`last_indexed=2026-10-17`) and returned no relevant kernel chunks; conclusions use direct source reads.
- `cargo fmt --all -- --check` — **PASS**.
- `cargo test -p xtask --locked` — **PASS**, 32/32.
- `NEO_SMP=4 cargo xtask neo-test wait_any_core` — all userland and kernel scenario builds **PASS** locally; QEMU launch could not run because this Windows environment has no `/home/neo/limine`, after four xtask retries.
- GitHub Actions run `39302053059` — independently checked with `gh`: **SUCCESS** on exact head `581a841e`; all four QEMU shards passed, including `wait_any_core`.
- PR #58's exact-head relay records the three controls at `NEO_SMP=4`: no identity check -> `wait_any_rebind_stranded`; no handle fire -> `wait_any_revoke_stranded`; no IPC fire -> `wait_any_ipc_wake_missing`.
