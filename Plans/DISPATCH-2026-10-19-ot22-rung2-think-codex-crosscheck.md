<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family check of the OT-22 rung-2 THINK (more wait interests) → Chad (Codex)

**Type:** design review only. **Deliverable:** a pushed RETURN file.

**Why you.** The THINK adds two new wakers to the concurrency core: the present-completion ISR and
`futex_wake`. A different-family lens is required before any rung-2 code is dispatched. Jarvis holds the
rung-1 design review and the CP-60 check, so this goes to a second Codex seat.

## What is under review

`docs/THINK-2026-10-19-ot22-rung2-more-interests.md`, on branch `claude/ot22-next-rung-think` until it merges,
then on `main`. Context:

- `docs/THINK-2026-10-19-ot22-readiness-multiplex.md` (the base THINK);
- the rung-1 code on `claude/ot22-multiplex` (#58): `kernel/src/sched/wait_any.rs` and `wait_any_logic.rs`,
  `sys_wait_any_impl` in `kernel/src/arch/x86_64/syscall.rs`, and `Channel::recv_ready` / `fire_waiters` in
  `kernel/src/ipc/mod.rs`.

The THINK's claims, each to be refuted:

1. **§1.** No shipping build spins in a fence wait: Software presents complete before `fence_wait`'s first
   check. The only spin is the HardwareFlip `fence_wait` yield loop.
2. **§2 (2a).** `fence_wait` can park on the rung-1 wait table, fired by `publish_complete` after its
   `fetch_max(Release)`, without losing a wake. The present ISR gains `WAIT_TABLE` as a second, never-nested
   leaf lock.
3. **§3 (2b).** `WAIT_KIND_FENCE_IDLE` (4), ready when `completed_epoch >= issued_epoch`, is level-triggered
   and race-free.
4. **§4.** Send-ready is unsound here: wait_any receivers are never `RecvBlocked`, so a sender waiting for
   send-ready and a receiver waiting for recv-ready park on each other.
5. **§5 (2c).** `WAIT_KIND_FUTEX` (5):
   - items index a 16-byte extension record passed in syscall args 4/5, read only when a kind-5 item is
     present (byte oracle in §5.1);
   - observers never consume `futex_wake`'s count;
   - keys are tagged by kind.
6. **§8.** The 100 Hz input pull at idle is acceptable on the bare-metal power budget because the BSP's
   periodic PIC tick already wakes it at 100 Hz into a plain `hlt`. The answer flips only with a tickless
   idle or deeper C-states.

## Refute it

1. **Lost wakeups (2a/2b/2c).** For each waker, find an interleaving where the publish (epoch store, user
   word store) and the fire straddle the waiter's register → re-check → park so that the waiter parks with
   nothing left to fire it. Check the §2 mutex-total-order argument against the actual
   `register`/`park`/`fire_*` code.
2. **ISR context.**
   - Can `publish_complete` → `fire_fence` run on a CPU that already holds `WAIT_TABLE`, from any present
     path, the full-ring fallback, or a test seam?
   - Can it deadlock or spin with IF=0 against a holder on another CPU that is itself waiting on something
     the ISR's CPU holds?
3. **The count rule.**
   - Does firing observers after `FUTEX_QUEUE` is dropped change any existing `futex_wake` caller's
     behaviour or return value: `SYS_FUTEX_WAKE`, the `thread_exit` join wake, the rwlock fallback?
   - Is a futex observer on a mutex word (whose fast path skips the wake) only slow, never wrong?
4. **ABI.**
   - Can a rung-1 binary with leftover registers in args 4/5 ever make the kernel read them?
   - Does the extension record's validation (alignment, the `flags == 0` rule, the index bound, `ext_n`) leave
     any unchecked user input?
   - Is the byte oracle right?
5. **FENCE_IDLE.** `issued_epoch` is stored `Relaxed` under `present_serial`; `completed_epoch` is
   `fetch_max(Release)`. Can a scan read a pair that says "idle" while a present is in flight, or "busy"
   forever after everything landed?
6. **Send-ready.** Find a receiver in the tree that parks in `sys_recv` while its sender would need to
   multiplex. If one exists, the §4 verdict is wrong.
7. **Power (§8).**
   - Is the tick truly never stopped or reprogrammed at idle?
   - Is the idle halt truly plain `hlt`?
   - Is the per-tick work estimate off by an order of magnitude anywhere, for example an MMIO access in the
     HID pump that loops?
   - Are the falsifiers F1/F2 sound? Can RDMSR 0x611 #GP on the OptiPlex 7080?
8. Anything else a Claude reviewer would structurally miss.

A finding counts only if it survives your own re-verification; uncertain items are notes. "No findings" must
say what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\chad     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot22-rung2-think-crosscheck-chad   (git fetch origin && git checkout -B codex/ot22-rung2-think-crosscheck-chad origin/main)
```

```
git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-19-ot22-rung2-think-codex-crosscheck.md
#   (until the THINK PR merges: git show origin/claude/ot22-next-rung-think:Plans/DISPATCH-2026-10-19-ot22-rung2-think-codex-crosscheck.md)
git show origin/main:docs/THINK-2026-10-19-ot22-rung2-more-interests.md
#   (until it merges: git show origin/claude/ot22-next-rung-think:docs/THINK-2026-10-19-ot22-rung2-more-interests.md)
git show origin/main:docs/THINK-2026-10-19-ot22-readiness-multiplex.md
git show origin/claude/ot22-multiplex:kernel/src/sched/wait_any.rs
```

No runs are required: this is a design review.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?" questions;
grep still wins for exact symbols. Health-check first with `Invoke-RestMethod http://127.0.0.1:8404/health`,
expecting `{"status":"healthy",...}`. If it is down or degraded, the PL manages it: note that in your RETURN
and continue with direct reads.
Optional self-serve start:
`powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='present fence completion publish_complete vblank ISR'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-19-ot22-rung2-think-codex-crosscheck-chad.md` to
`codex/ot22-rung2-think-crosscheck-chad` and push it. Contents:
- a verdict per claim 1-6 (HOLDS / HOLDS-WITH-CHANGES / FAILS);
- each finding with severity, file:line and an interleaving or repro;
- what you checked for each numbered refute item.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete the
worktree.
