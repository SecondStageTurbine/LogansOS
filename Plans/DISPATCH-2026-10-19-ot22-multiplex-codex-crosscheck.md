<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family design review of OT-22's readiness multiplex → a Codex seat

**Type:** design review only (no code). **Deliverable:** a pushed RETURN file. **Why a Codex seat:** this is
concurrency-core (park, wake, timer expiry, a new lock on the send path). The author is Claude-family, so a
different-family lens is required before any code. The PL assigns the seat (Jarvis / Janus / Chad).

## What is under review

`docs/THINK-2026-10-19-ot22-readiness-multiplex.md` on branch `claude/ot22-multiplex-think`:
- Option A: `sys_wait_any` over at most 8 items with a deadline.
- One wait cell per waiter, which is the CP-14 park lock.
- Channel registration slots fired by `send`/`grant`/`close_peer`.
- Input stays pulled: the waiter pumps HID in its syscall, and the tick wakes input waiters.
- The rung-0 discriminator (`desktop_idle_parks`, with idle-tick accounting fixed) and its falsification legs.

## Refute it

1. **Lost wakeup:** find an interleaving of register → pump/check → cell-lock → park against a concurrent `send`, `close_peer`, tick expiry or process kill in which the waiter sleeps through a ready source, or two CPUs touch the parked thread.
2. **Lock order:** does any existing path take `Channel.state` (an `IrqSafeMutex`) with the scheduler lock held, or the scheduler lock inside the timer handler before the proposed cell walk? Name the inversion.
3. **The tick rescue:** can the design's tests pass while the IPC wake is broken? Is the `no-ipc-fire` control enough?
4. **Token capacity (CP-31):** what happens when `TOKEN_PARKED_SLOTS` is full on the BSP? Is "return EAGAIN without parking" safe for every caller loop?
5. **BSP residency:** is it true that user threads only run on the BSP today? If not, what does an EOI-only IPI (CP-15/21) do to the latency bound?
6. **HID path:** confirm nothing new runs in IRQ context except the bounded timed-list walk, and nothing new takes `DELIVERY`, `PRIMARY` or a device lock outside syscall context (CP-37/55/56).
7. **The alternatives:** is option B or C actually smaller for these four loops than the THINK claims?
8. Anything a Claude reviewer would structurally miss.

A finding counts only if it survives your own re-verification; uncertain items are notes. "No findings" must say
what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\<your seat>     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot22-multiplex-crosscheck-<seat>   (git fetch origin && git checkout -B codex/ot22-multiplex-crosscheck-<seat> origin/claude/ot22-multiplex-think)
```

```
git fetch origin
git show origin/claude/ot22-multiplex-think:Plans/DISPATCH-2026-10-19-ot22-multiplex-codex-crosscheck.md
git show origin/claude/ot22-multiplex-think:docs/THINK-2026-10-19-ot22-readiness-multiplex.md
```

The code to check against:
- `kernel/src/ipc/mod.rs`: send, recv, try_recv, close_peer, and the handoff helpers.
- `kernel/src/sched/scheduler.rs`: `block_current_with_token_and_guard`, `wake_thread_token`, the idle thread.
- `kernel/src/sched/futex.rs`: the timed-list and tick-expiry precedent.
- `kernel/src/atrium/focus.rs`: `DELIVERY`, flush, the SAK latch.
- `kernel/src/drivers/usb/hid_*.rs`: `pump_routed_events`.
- `kernel/src/arch/x86_64/timer.rs`.
- `docs/concurrency-pitfalls.md`: CP-14..16, 20..22, 28, 30..31, 33, 36..37, 55..57.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?"
questions; grep still wins for exact symbols. Health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`, expecting `{"status":"healthy",...}`. If it is down or
degraded, the PL manages it: note that in your RETURN and continue with direct reads.
Optional self-serve start:
`powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='rendezvous recv park wake token'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-19-ot22-multiplex-codex-crosscheck-<seat>.md` to
`codex/ot22-multiplex-crosscheck-<seat>` and push it. Contents:
- a verdict (BUILD-AS-DESIGNED / BUILD-WITH-CHANGES / REDESIGN);
- each finding with its severity, the file:line it cites, and the interleaving or repro;
- what you checked for each numbered item.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete the
worktree.
