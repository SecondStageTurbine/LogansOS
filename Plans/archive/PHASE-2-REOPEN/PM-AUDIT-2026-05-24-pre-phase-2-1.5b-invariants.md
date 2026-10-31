# PM-AUDIT: pre-Phase-2 audit of Phase 1.5b IPC, scheduler, and capability invariants

- Author: PM (Cerebro/Claude)
- Date: 2026-05-24 (Day 1 of inventory work; this is a substantial partial)
- Status: **PARTIAL.** Inventory column complete. Evidence classification complete.
  Failure-mode analysis and dispositions complete for close_peer-adjacent
  invariants; remaining invariants carry inventory entries + evidence tags
  but failure-mode and disposition sections marked PENDING for follow-up.
- Binding scope: `Plans/PM-AUDIT-SCOPE-2026-05-24-pre-phase-2-1.5b-invariants.md`
  (PL-ratified this session).
- Sources scanned: see §3. ADRs and phase doc fully scanned. Bible articles
  and kernel source doc-comment pass deferred to Day 2 of inventory work.

---

## 1. Sharpened audit framing

The scope doc framed the audit as "what did 1.5b's e2e leak audit miss." After
reading the source surface, the framing sharpens to two simultaneous lenses:

**Lens A — what 1.5b's BUILD-6 leak audit missed under single-CPU.** The
vocabulary of that audit (frame counts, channel counts, process counts) is
blind to by-pid identity violations, exit-code semantics, cross-CPU ordering,
and capability authorization geometry. Scope §3 already characterized this.

**Lens B — what ADR-013's Phase 2 SMP retrofit (sub-commits 1-5, closed
2026-05-10 at commit `e2b3f4f`) failed to preserve, replace, or remove
appropriately.** ADR-013 explicitly inventoried what changes under SMP for
the handoff invariants (Q4/Q5), lock-order (Q6), TLB (Q7), capability tables
(Q8), and reapers (Q9). What it did NOT explicitly inventory was every
1.5b invariant that became newly-reachable under SMP — for example, the
ADR-004 §"Process-exit cleanup" invariant about waking blocked peers with
-EPIPE. Under single-CPU, cross-process IPC parking didn't exist as a
practical race (per BUILD-1's consumer-first amendment, the consumer always
ran first). Under SMP, cross-process parking is the normal case for grant
rendezvous, and the exit-cleanup invariant needed to be re-examined.

The close_peer cross-process zombify bug (PR #6, fixed in `4dfb13f`) is a
direct hit on Lens B: an exit-cleanup invariant that was implicit in
ADR-004's text held under single-CPU (because the only waiter on your own
side was your own thread, post-handoff) but stopped holding under SMP +
capability grant (because granted recipients could be parked on the donor's
side at the donor's exit time). ADR-013 did not call this out.

Both lenses are productive for the audit. Findings are tagged with which
lens they emerged from.

---

## 2. Disposition of the dangling 2026-05-03 memory reference

PL's working notes from 2026-05-03 carried a reference to "pre-Phase-2 audit;
deviations #10 + #7 + #8" that could not be resolved to specific artifacts.
The scope doc treated this as a soft signal and committed to re-deriving
against current state. With the inventory now built, my read is:

The numbered "deviations" were almost certainly the close-out items now
captured in `docs/phases/PHASE-1.5b.md` §"Course Corrections" and
§"Anti-criteria (deliberate non-deliveries — Phase 2 work)". The phase doc
enumerates eight named deviations; the three referenced ("#10 + #7 + #8")
plausibly correspond to specific entries in that list at the time of
2026-05-03 numbering (which was three weeks ago and is no longer reachable
from current text). The dangling reference is therefore not a missed
artifact — it's a stale pointer into a doc that has since been reorganized.

This audit supersedes the dangling reference. No action needed on the
memory entry beyond PL's noted intent to update.

---

## 3. Sources scanned

| Source | Status | Notes |
|---|---|---|
| `docs/bible.md` Articles 4, 5, 7, 8, 12 | PARTIAL | High-level claims captured from project_files context (TheAltimitNeoBible.md); Article 8.5 sub-clauses pending Day 2 deep-scan |
| `docs/bible.md` Article 6 (AI subsystem) | OUT OF SCOPE | Scope §9 explicitly defers AI subsystem cap discipline |
| `altimitneo-ai-subsystem-spec.md` (ANSPEC-001) §3.7, §10 | PARTIAL | Cap-handle pattern claims captured; AI-specific cap surface deferred per scope §9 |
| `docs/ADR-004-ipc-channels.md` | COMPLETE | 223 lines fully scanned |
| `docs/ADR-005-ring-transition-timer-prologue.md` | DEFERRED | Scheduler-adjacent but tangential to IPC/cap audit; sample if time permits |
| `docs/ADR-006-userspace-heap-and-mmap-region.md` | DEFERRED | Heap-adjacent but tangential; per scope §9 mm invariants are Phase 2 OBSERVE's scope |
| `docs/ADR-007-process-termination-and-process-handles.md` | COMPLETE | 110 lines fully scanned |
| `docs/ADR-010-capability-system-foundation.md` | COMPLETE | 364 lines fully scanned |
| `docs/ADR-011-single-cpu-ipc-handoff-invariants.md` | COMPLETE | 154 lines fully scanned (superseded by ADR-013) |
| `docs/ADR-013-phase-2-smp-retrofit.md` | PARTIAL | 1000 of 1212 lines scanned (Q1-Q9 design complete; migration checklist partial) |
| `docs/ADR-018-bible-8-5-4-preempt-while-holding-clarification.md` | COMPLETE | 73 lines fully scanned |
| `docs/ADR-019-switchmutex-irq-off-by-construction.md` | COMPLETE | 134 lines fully scanned |
| `docs/ADR-020-owner-metadata-production-diagnostic.md` | DEFERRED | Scheduler diagnostic; tangential |
| `docs/ADR-022-fault-recovery-direction-forward-marker.md` | COMPLETE | 78 lines fully scanned (informational, no architectural commitment) |
| `docs/phases/PHASE-1.5b.md` | COMPLETE | 139 lines fully scanned |
| `docs/phases/PHASE-1.5c.md` | DEFERRED | Phase-doc deep-scan pending Day 2 |
| `kernel/src/ipc/mod.rs` | PARTIAL | Read for close_peer fix this session; doc-comment pass pending Day 2 |
| `kernel/src/sched/*.rs` (5 files) | DEFERRED | Doc-comment pass pending Day 2 |

The deferred items are bounded; Day 2 inventory completes them. Findings
that depend on deferred sources are tagged "evidence-pending" rather than
asserted.

---

## 4. Invariant inventory

Each row: identifier, verbatim claim (or close paraphrase where the source
is verbose), source citation, evidence classification, lens (A/B/both).

Evidence classification per scope §4.2: PROVEN / TESTED / CLAIMED.

### 4.1 IPC channel invariants (ADR-004)

| ID | Invariant | Source | Evidence | Lens |
|---|---|---|---|---|
| INV-IPC-1 | Channel handles are anonymous (no name, no global registry); owned by creating process; distributable on send/pass-at-spawn. | ADR-004 §Q1 | TESTED — `sys_chan_create` returns two handles; tests verify both work | A |
| INV-IPC-2 | Pass-at-spawn deterministic ordering ABI: i-th handle in `&[Handle]` lands at child handle index i. | ADR-004 §Q2 | TESTED — neo-init/worker startup relies on this; e2e smoke would fail if violated | A |
| INV-IPC-3 | Send and receive are symmetric: either side can arrive first; second arrival completes both. | ADR-004 §Q3 + load-bearing semantic clarification | TESTED — BUILD-3 verification list items 2-3 explicitly exercise both arrival orders | A |
| INV-IPC-4 | Truncation policy: kernel truncates and returns `bytes_copied = min(send_len, max_len)`. | ADR-004 §"Truncation policy" | TESTED — BUILD-3 verification item 4 | A |
| INV-IPC-5 | Page-translate fault during rendezvous: first arrival (blocker) wakes with -EFAULT; second arrival returns -EFAULT directly; no partial copies. | ADR-004 §Implementation "User→user copy" | CLAIMED — no specific test cited; relies on code review | A |
| INV-IPC-6 | Channel endpoint state machine has exactly: Idle, SendBlocked, RecvBlocked, Closed. | ADR-004 §Implementation "Kernel objects" | TESTED — implicit in all IPC tests | A |
| **INV-IPC-7** | **Process-exit cleanup: for each handle, if peer is SendBlocked or RecvBlocked, yank the parked Box, reset waiter's syscall return to -EPIPE, mark Ready, push to ready queue, mark endpoint Closed.** | ADR-004 §Implementation "Process-exit cleanup" | **CLAIMED pre-PR-#6, TESTED post-PR-#6** — pre-#6 had no test exercising foreign-process waiter at exit time; PR #6 added coverage | **B** |
| INV-IPC-8 | `close_channel_peers` is the shared helper between natural exit (`on_thread_reap`) and forced kill (`process::terminate`). Must stay in lockstep — if one changes, both change. | ADR-004 §Implementation "Process-exit cleanup" amendment | TESTED — both code paths exercised by BUILD-6 e2e (natural exit) and BUILD-5 (forced kill) | A |
| INV-IPC-9 | Killed-recv non-path: killed process's own thread blocked in recv doesn't return — it's gone, not woken. Only the *peer* (still-alive process) blocked on the killed process's channels wakes with -EPIPE. | ADR-007 §"Implementation notes" item 2 (cross-cited with ADR-004) | TESTED — BUILD-6 scenarios | A |
| INV-IPC-10 | Single-CPU handoff (BUILD-1 amendment): completing syscall front-queues blocked peer and immediately yields. Receiver/recipient is the consumer; consumer-first ordering applies regardless of arrival order. | ADR-004 §"Phase 1.5c BUILD-1 amendment: single-CPU handoff" + ADR-011 §INV-1 BUILD-1 amendment | **REPLACED** — superseded by ADR-013 Q4 (wake-on-completer-CPU) under SMP. Test `ipc_heap_kill_smoke` depends on it. | A→B |
| INV-IPC-11 | BUILD-2 amendment: no slow diagnostic syscalls in handoff window between rendezvous return and next blocking syscall. | ADR-004 §"BUILD-2 tightened the test discipline" | TESTED — discipline encoded in test design; not a runtime check | A |
| INV-IPC-12 | BUILD-4 amendment: front-wake grants receiver a bounded handoff quantum, cleared on next syscall entry or block. | ADR-004 §"BUILD-4 production boot tightened it again" + ADR-011 INV-2/3/4 | **REMOVED** — ADR-013 Q5 deletes this mechanism entirely under SMP. | A→B |

### 4.2 Process termination invariants (ADR-007)

| ID | Invariant | Source | Evidence | Lens |
|---|---|---|---|---|
| INV-PROC-1 | ProcessHandle is capability; `sys_kill` requires the handle; no PID-keyed kill path exists. PIDs are not authority-bearing. | ADR-007 §Q1 + Phase 1.5b anti-criterion A1 | TESTED — `sys_kill` impl checks variant; A1 anti-criterion is in BUILD-6 verification | A |
| INV-PROC-2 | Self-kill returns -EINVAL with explicit check at `sys_kill_impl`. | ADR-007 §Implementation note 1 | CLAIMED — explicit guard exists in code; ADR notes runtime test deferred to Phase 4 (requires cap-passing-over-IPC to construct) | A |
| INV-PROC-3 | Forced kill reuses BUILD-3's Drop chain: registry-removed-first → handle-table-drained → Pmap-dropped → frames reclaimed. No new ordering invariants. | ADR-007 §Q3 + Implementation note 5 | TESTED — BUILD-6 e2e leak audit asserts frames_in_use_post_reap == baseline | A |
| INV-PROC-4 | PROCESSES-removed-BEFORE-HandleTable-drop ordering. `debug_assert!` immediately after removal confirms registry no longer has the pid before any handle drop runs. | ADR-007 §Implementation note 3 | TESTED — debug_assert encodes invariant statically | A |
| INV-PROC-5 | Reaper double-cleanup is benign: `terminate` does the channel-close work and pushes to zombies; reaper later calls `on_thread_reap` which finds `PROCESSES.get(&pid) == None` and returns early. No double `close_peer`. | ADR-007 §Implementation note 4 | CLAIMED — relies on early-return guard in `on_thread_reap`; not specifically tested under SMP-introduced race conditions | A→B |
| INV-PROC-6 | Stale ProcessHandle returns -ESRCH (POSIX number -3), not -EINVAL. EINVAL is reserved for "target exists but isn't in killable state." | ADR-007 §Implementation note 6 | TESTED — BUILD-6 unhappy-path #3 (double-kill → ESRCH) | A |
| INV-PROC-7 | Phase 1.5b limitation: `sys_kill` returns -EINVAL if target's thread is not in the ready queue (parked inside an EndpointState). Lifting deferred to Phase 2 SMP work. | ADR-007 §Q3 + Phase 1.5b §"Anti-criteria" | **REPLACED** — ADR-013 Q8.c specifies cross-CPU blocked-thread walk in `kill_thread_by_pid`. Status of implementation needs Day 2 verification against kernel source. | A→B |
| INV-PROC-8 | Phase 4 PID-recycling concern: Phase 1.5b's `NEXT_PID` is monotonic; stale `ProcessHandle` cannot accidentally point at freshly-spawned process. Phase 2+ may add recycling; revocation becomes load-bearing then. | ADR-007 §Implementation note 6 | DEFERRED — not active in Phase 1.5b; flagged for Phase 4 | n/a |

### 4.3 Capability system invariants (ADR-010)

| ID | Invariant | Source | Evidence | Lens |
|---|---|---|---|---|
| INV-CAP-1 | Per-process flat HandleTable + per-process derivation log (BTreeMap<CapId, Option<CapId>>). Cleanup local to process exit. | ADR-010 §Q1 + Implementation note 1 | TESTED — `cap_*` scenarios per Phase 2 BUILD-1 verification | A |
| INV-CAP-2 | CapId is global monotonic AtomicU64; never recycled in Phase 2. | ADR-010 §Q1 + Implementation note 2 | TESTED — by construction | A |
| INV-CAP-3 | Four cap ops: mint (via existing syscalls), derive, grant, revoke. Copy/move deferred. | ADR-010 §Q2 | TESTED — Phase 2 BUILD-1 cap_* scenarios cover each | A |
| INV-CAP-4 | Two-tier rights: uniform `meta: u8` (USE/DERIVE/GRANT/REVOKE) + per-variant `semantic: u8`. | ADR-010 §Q3 | TESTED — implicit in all cap scenarios | A |
| INV-CAP-5 | Bit-subset semantics for derive: `(child & parent) == child` for both meta and semantic bytes. No per-variant logic. | ADR-010 §Q5 + Implementation note 4 | TESTED — `cap_derive_subset` scenario | A |
| **INV-CAP-6** | **Cap survive-exit: caps held by other processes outlive their donor's exit. Invocation against now-dead resource returns appropriate errno (-EPIPE for channels, -ESRCH for processes); the handle stays valid as a handle.** | **ADR-010 §Q4 + Implementation note 5** | **CLAIMED pre-PR-#6, TESTED post-PR-#6** — `cap_survive_exit` scenario per BUILD-1 verification list. Pre-PR-#6 test apparently did not exercise the parked-at-exit-time case that PR #6 fixed. | **B** |
| INV-CAP-7 | Grant rendezvous three-way intersection (Principle 1): `actual = grantor_actual ∩ rights_subset ∩ max_rights`. Kernel never trusts a declaration over a held fact. | ADR-010 §Q2 + Principle 1 | TESTED — `cap_grant_basic` exercises happy path; other grant scenarios exercise rights-clip cases | A |
| INV-CAP-8 | Errors at the causing actor (Principle 3): in multi-actor cap ops, failure errno is reported to the actor whose action caused it; other actor sees -EPIPE. | ADR-010 §Principle 3 | TESTED — `cap_grant_missing_meta` (A gets -EPERM, B sees -EPIPE) | A |
| INV-CAP-9 | Grant is derived-copy not move: donor keeps source cap; recipient gets fresh child CapId. | ADR-010 §"BUILD-1 amendment" item 1 | TESTED — implicit in cap_grant scenarios; donor's source remains usable post-grant | A |
| INV-CAP-10 | Revoke frees handle slots: revoked slot becomes None; subsequent invocation through same userspace index returns -EBADF; slots recyclable by lowest-free-index. | ADR-010 §"BUILD-1 amendment" item 2 | TESTED — `cap_revoke_local`, `cap_revoke_cross_process`, `cap_revoke_descendants_walk` | A |
| INV-CAP-11 | Spawn-time inheritance is derived-copy grants: child receives fresh CapIds with parent links; parent must hold GRANT on each inherited source. | ADR-010 §"BUILD-1 amendment" + spawn-time inheritance clarification | CLAIMED — implementation note says fresh child CapIds enable distinguishing parent/child during revoke; no specific test cited for the revoke-finding-spawn-inheritance descendants case | A→B |
| INV-CAP-12 | CLOSE bit is enforcement: when `sys_close` lands, it MUST gate on `semantic & CLOSE != 0`. Kernel-internal cleanup paths (`on_thread_reap → close_channel_peers`) are exempt by construction. | ADR-010 §Implementation note 8 | DEFERRED — `sys_close` not yet shipped; bit is reserved (Principle 2). Audit relevant when sys_close lands. | A |
| INV-CAP-13 | Kernel-cleanup paths exempt from rights checks: operating on Channel state directly during cleanup; not capability operations. | ADR-010 §Implementation note 8 | CLAIMED — discipline rule; not specifically tested | A |

### 4.4 Single-CPU IPC handoff invariants (ADR-011, superseded)

ADR-011 invariants are listed here because tests that depended on them are
load-bearing for the audit. ADR-013 status column shows whether each was
replaced, removed, or otherwise dispositioned.

| ID | Invariant | ADR-013 disposition | Lens |
|---|---|---|---|
| INV-HANDOFF-1 | Rendezvous-completer yields directly to woken peer at front of ready queue. | **REPLACED** by Q4: wake-on-completer-CPU with consumer-first. Closed 2026-05-10 via SC-3B, commit `e2b3f4f`. | B |
| INV-HANDOFF-2 | Just-woken peer holds bounded preempt-defer quantum (32 ticks). | **REMOVED** by Q5. Sub-commit 2 deletes the mechanism. | B |
| INV-HANDOFF-3 | Deferred quantum cleared on next syscall entry. | **REMOVED** with Q5. | B |
| INV-HANDOFF-4 | Deferred quantum cleared when thread blocks. | **REMOVED** with Q5. | B |

### 4.5 Synchronization invariants (ADR-018/019)

| ID | Invariant | Source | Evidence | Lens |
|---|---|---|---|---|
| INV-SYNC-1 | Hpw discipline: a spinlock acquired in preemptible context, where the same lock may also be acquired in IF=0 context on the same CPU, MUST be acquired with local interrupts disabled by the preemptible-context acquirer (`without_interrupts(...)` is canonical). | ADR-018 (bible 8.5.4) | TESTED — SC-3B Sub-step 5c verification 200/200 PASS at `NEO_SMP=4` on the five affected Channel.state call sites. | B |
| INV-SYNC-2 | IrqSafeMutex provides type-level enforcement of INV-SYNC-1 for mixed-IRQ-context locks. Plain spin::Mutex remains for exclusively-process or exclusively-ISR contexts with SAFETY-comment justification. | ADR-019 | **PENDING** — ADR-019 status is "Implementation status: PENDING — sub-phases A, B, C scheduled." Need Day 2 git log to verify whether implementation has landed since 2026-05-10. | B |
| INV-SYNC-3 | Block-on-switch lock-release pattern (ADR-013 Q6 ManuallyDrop + asm release): per-CPU scheduler lock + Channel mutex both released on the new stack after `context_switch` completes, before new thread's code runs. Up to 4 lock releases supported via the asm wrapper's release list. | ADR-013 §Q6 | TESTED — Closed 2026-05-10 via SC-3B Sub-step 5c; `smp_block_release_smoke` + `smp_cross_cpu_block_release_stress` per Q6 verification. | B |

### 4.6 Phase 1.5b anti-criteria as invariants

The phase doc's anti-criteria are negative-form invariants. They claim
something *is not* a deliverable; future code must preserve the absence.

| ID | Invariant | Source | Evidence | Lens |
|---|---|---|---|---|
| INV-NEG-1 | No POSIX-style ambient authority — PIDs are not authority-bearing. | Phase 1.5b A1, also ADR-007 §Q1 | TESTED — no PID-keyed kill path exists; only ProcessHandle | A |
| INV-NEG-2 | No retrofit cost for Phase 2 — Handle enum IS the Phase 2 capability shape. | Phase 1.5b A2 | TESTED — Phase 2 BUILD-1 extended without churning 1.5b handle code | A |
| INV-NEG-3 | Blocked-thread kill returns -EINVAL (Phase 1.5b deliberate non-delivery). | Phase 1.5b §Anti-criteria | **REPLACED** — ADR-013 Q8.c lifts this restriction under SMP. Day 2 source-side verification of `kill_thread_by_pid` confirms. | A→B |
| INV-NEG-4 | NEXT_PID is monotonic in 1.5b; no PID recycling. | Phase 1.5b §Anti-criteria | TESTED — by construction | n/a |

---

## 5. Findings (close_peer-adjacent invariants, complete)

The audit's seed event (PR #6 / commit `4dfb13f`) most directly implicates
INV-IPC-7 and INV-CAP-6. Findings for these are complete; other findings
PENDING for Day 2.

### Finding F-001 — INV-IPC-7 (process-exit cleanup wakes blocked peer with -EPIPE)

**Pre-PR-#6 status:** CLAIMED (no test exercising foreign-process waiter
at exit time).

**Failure modes not detected by 1.5b's BUILD-6 leak audit:**

1. **Cross-process zombify via grant.** Under SMP with ADR-010's grant
   rendezvous, init can park on worker's side of a granted data channel.
   When worker exits, `close_peer(worker's_side, exiting_pid=worker)` finds
   init parked there. The pre-PR-#6 `extracted_my` branch in `close_peer`
   was an optimization for the case where the EXITING process's own thread
   was parked on its own side (zombify directly rather than wake-with-EPIPE,
   since the thread is already mid-cleanup). Pre-PR-#6, that branch fired
   for ANY parked waiter, regardless of pid. Init got zombified; init's
   `sys_exit` never ran; kernel fell back to exit code 255. *Result: a
   non-exiting process is terminated by another process's cleanup, with
   the wrong exit semantics.*
2. **Leak-audit blindness.** Init's process did exit (just by the wrong
   path), so process count rebalanced. Frames freed deterministically via
   the Drop chain. e2e leak audit assertion `frames_in_use == baseline`
   passed. The bug was invisible to the audit's vocabulary.

**Post-PR-#6 status:** TESTED. The fix threads `exiting_pid` through
`close_peer` and only zombifies when `waiter.pid() == exiting_pid`;
foreign-process waiters are woken with -EPIPE per the invariant.

**Disposition: (a) immediate kernel fix — COMPLETE.** Shipped in commit
`4dfb13f` (PR #6). No further action required by the audit.

**Lens-B observation:** ADR-013's SMP retrofit did not explicitly call out
INV-IPC-7 as needing SMP-aware review. Q4 (handoff under SMP) addressed
the wake strategy at rendezvous; Q9 (per-CPU reapers) addressed the reaper
distribution. Neither walked the exit-cleanup path's interaction with
foreign-process waiters that could only exist post-Phase-2-BUILD-1 (when
grant rendezvous made cross-process parking real). This is a process gap in
the SMP retrofit ADR's coverage, not a code gap (the code is now correct).

### Finding F-002 — INV-CAP-6 (cap survive-exit)

**Pre-PR-#6 status:** CLAIMED. Per ADR-010 BUILD-1 verification list, a
`cap_survive_exit` scenario existed. Per the close report's analysis, this
scenario most plausibly exercised the post-exit invocation case — A grants
to B; A exits; B invokes on the granted handle; expect -EPIPE or -ESRCH.
The pre-PR-#6 close_peer bug zombified B *at A's exit time*, not at B's
invocation time, so the scenario as designed would not have caught it. B
never got to the invocation step because B was already zombie.

This is the "test passes for the wrong reason" failure mode: the assertion
held (B's invocation never returned a wrong errno because B's invocation
never ran) but for the wrong reason (B was wrongly terminated).

**Post-PR-#6 status:** TESTED. The fix preserves cap survive-exit per the
ADR-010 Q4 invariant: cross-process cap holders survive donor exit;
invocation returns -EPIPE (channel) or -ESRCH (process).

**Disposition: (a) immediate kernel fix — COMPLETE.** Same fix as F-001
(single PR addresses both invariants).

**Lens-B observation:** ADR-010 BUILD-1's verification list shape ("scenario
N exercises the named behavior") is vulnerable to the "passes for the wrong
reason" failure mode when the negative space of "behavior X happens" is
larger than the test's explicit positive assertion. The audit's discipline
recommendation: cap-related tests should add an explicit "and B is not
zombified" or "and B's sys_exit ran" assertion when the test depends on
B's continued execution. Two-sided assertions (positive + negative space)
are stronger than one-sided.

### Finding F-003 — INV-IPC-8 (close_channel_peers shared between natural exit and forced kill)

**Pre-PR-#6 status:** TESTED for the IPC-state correctness path
(both code paths exercised by BUILD-5 forced kill + BUILD-6 natural exit).
NOT tested for the cross-process-waiter behavior — both callers entered
the buggy `extracted_my` branch with the same blind spot.

The lockstep invariant (ADR-004 §"Process-exit cleanup" amendment) is
preserved by PR #6: both callers receive the fix simultaneously because
the fix is in the shared helper, not in either caller.

**Post-PR-#6 status:** TESTED for both IPC-state and cross-process-waiter
behavior on both callers via PR #6's test coverage. Lockstep holds.

**Disposition: PASS.** Invariant is now correctly enforced. The audit
notes that INV-IPC-8 was structurally sound — extracting the shared
helper meant that fixing the bug fixed both callers simultaneously, which
is exactly the failure-mode the invariant was designed to prevent.

### Finding F-004 — INV-PROC-5 (reaper double-cleanup is benign) under SMP

**Pre-SMP status:** CLAIMED. The early-return guard in `on_thread_reap`
relies on `PROCESSES.get(&pid)` returning None after `terminate` has run.

**Under SMP question:** With Q8.a's RwLock on PROCESSES and Q9's per-CPU
reapers, can two CPUs concurrently observe `PROCESSES.get(&pid) == Some(...)`
between the read and the eventual cleanup? ADR-013 Q9 addresses the
"is this the last thread" race by adding `live_thread_count: AtomicU32`
and double-checking under write-lock. That covers the deregistration
ordering. It does NOT explicitly address the `on_thread_reap` early-return
guard under SMP.

**Failure mode:** If `terminate` removes from PROCESSES on CPU0, then a
concurrent reaper on CPU1 calls `on_thread_reap(pid)` before CPU0's cleanup
completes its full handle-drain, CPU1 would see `PROCESSES.get(&pid) == None`
and return early. But CPU1's call is on a zombie produced BY `terminate`'s
push — meaning CPU0 already drained handles before pushing. So the
ordering should hold.

But this is a CLAIMED invariant under SMP, not a TESTED one. The
ordering depends on `terminate`'s internal sequencing: remove from
PROCESSES first, then drain handles, then push to zombies. If this
sequencing is reordered by future refactoring, the invariant breaks
silently.

**Disposition: PARTIAL — file a test recommendation.** Recommend a test
in the next BUILD-N test work item that explicitly exercises
"terminate on CPU0 + concurrent on_thread_reap on CPU1 + verify no
double `close_peer`." Today's CI ran clean across the full suite per
PR #6's verification, suggesting the invariant holds empirically, but
no specific assertion exists.

Disposition target per scope §6: **(b) BUILD-N test work** in the next
available BUILD subphase. Recommended test name: `smp_concurrent_reap_after_terminate`.

---

## 6. Findings (remaining invariants — PENDING)

The following invariants have inventory entries and evidence classification
but failure-mode analysis and disposition are deferred to Day 2 of the
inventory work:

| Invariant | Reason for deferral |
|---|---|
| INV-IPC-5 (page-translate fault in rendezvous) | CLAIMED; needs source-side check of fault-injection coverage |
| INV-IPC-11 / INV-IPC-12 | REPLACED/REMOVED under SMP; needs verification that current `ipc_heap_kill_smoke` still PASSes after Q5's deferred-quantum removal |
| INV-PROC-2 (self-kill -EINVAL) | CLAIMED with explicit deferred-test note; future Phase 4 concern, not 1.5b audit-blocking |
| INV-PROC-5 under SMP | PARTIAL finding F-004 above |
| INV-PROC-7 (Blocked-target kill -EINVAL) → SMP cross-CPU walk | REPLACED by ADR-013 Q8.c; needs source-side verification that `kill_thread_by_pid` implements the walk |
| INV-CAP-11 (spawn-inheritance derived-copy descendants in revoke walk) | CLAIMED with no specific test cited; needs Day 2 review of test suite for descendants coverage |
| INV-CAP-12 (CLOSE bit enforcement) | DEFERRED until `sys_close` ships |
| INV-CAP-13 (kernel-cleanup paths exempt from rights checks) | CLAIMED discipline rule; needs source-side verification that kernel cleanup paths do not pass through `rights_check` |
| INV-SYNC-2 (IrqSafeMutex implementation) | PENDING — needs Day 2 git log to verify implementation status |
| INV-NEG-3 (Blocked-thread kill lifted under SMP) | Needs Day 2 source-side verification |

Each PENDING entry will resolve to PASS / PARTIAL / DEFECT after Day 2's
source-side scan. The audit deliverable is not complete until each has a
finding label.

---

## 7. Net findings so far

| Disposition | Count | Notes |
|---|---|---|
| PASS | 1 | F-003 (INV-IPC-8 lockstep) |
| PARTIAL — (b) BUILD-N test work | 1 | F-004 (INV-PROC-5 under SMP) |
| DEFECT — (a) immediate fix, COMPLETE | 2 | F-001 (INV-IPC-7), F-002 (INV-CAP-6) — both fixed in PR #6 |
| PENDING Day 2 | 10 | per §6 |

**No DEFECT findings are open.** PR #6 closed the two known defects. All
PARTIAL findings reachable from current inventory disposition to
**(b) BUILD-N test work** — no bible amendments or Phase 2 ADRs required
from the audit's seed-event-adjacent findings.

This is the load-bearing result for Phase 2 PLAN: the audit's seed event
does not block Phase 2 PLAN start. PARTIAL findings can be absorbed into
Phase 2's own BUILD work as test additions. The audit's remaining PENDING
findings may surface additional dispositions; Phase 2 PLAN should not start
until the audit is fully closed, per the scope doc §10.

---

## 8. Day 2 work plan

Estimated 4-8 hours of focused PM time:

1. **Bible Article 8 deep scan** — sub-clauses 8.5.1 through 8.5.5,
   especially 8.5.4 as amended by ADR-018. Verify alignment with INV-SYNC-1.
2. **Bible Article 12 deep scan** — capability discipline mandates;
   verify INV-CAP-* row coverage is complete.
3. **`docs/phases/PHASE-1.5c.md`** — capture any 1.5c-specific invariants
   that augment 1.5b (especially BUILD-1/2/4 handoff amendments already
   reflected in ADR-004/011 but possibly with additional phase-doc detail).
4. **Kernel source doc-comment pass** — `kernel/src/ipc/mod.rs`,
   `kernel/src/sched/scheduler.rs`, `kernel/src/sched/process.rs`,
   `kernel/src/sched/thread.rs`. Extract any module- or function-level
   invariant claims not captured in ADRs.
5. **Git log verification** for INV-SYNC-2 (IrqSafeMutex implementation
   status) and INV-PROC-7 (cross-CPU blocked-thread kill walk).
6. **Failure-mode analysis** for the 10 PENDING invariants.
7. **Final findings + dispositions**.
8. **PM-DISPOSITION artifact** summarizing the audit's net findings and
   listing blocking items (if any) for Phase 2 PLAN.

---

## 9. References

- `Plans/PM-AUDIT-SCOPE-2026-05-24-pre-phase-2-1.5b-invariants.md` —
  binding scope.
- `Plans/CLOSE-REPORT-2026-05-24-ci-caching-and-cap-survive-exit.md` —
  seed event close report (PR #6).
- Commit `4dfb13f` — PR #6, the close_peer pid-aware fix.
- Commit `e2b3f4f` — SC-3B Sub-step 5c, ADR-013 Q4/Q6/Q7 closure.
- Commit `e9675c4` — Phase 1.5b close.
- ADR-004, ADR-007, ADR-010, ADR-011, ADR-013, ADR-018, ADR-019, ADR-022
  per §3 sources scanned.
- Phase 1.5b doc.
