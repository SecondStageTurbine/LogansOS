<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Codex re-check #2 of B1c — the J3/J4 kill-design fix → Jarvis

**Type:** review only. **Deliverable:** one pushed RETURN.

**Why:** your re-check (`codex/b1c-code-aag-recheck-jarvis` @ `204b0aac`, reviewing `056f89fe`) closed
J2 and F1, and the original shootdown deadlock. You bounced the kill design on two findings:
- **J3 (HIGH):** a waiter for the inference stack was killable, and its kernel stack's `Arc`s leaked.
- **J4 (MEDIUM):** the owner marker ended before the boundary exit decision, and the owner tuple was not
  one coherent state.

The PM ruled the shape of the fix: one per-thread inference-lifecycle marker. The fix is on
`claude/b1c-product-generation`, and the PM names the head SHA when relaying. This re-check covers J3,
J4, and anything the fix broke. J1's shootdown half, J2 and F1 stay closed unless the fix regressed
them.

## What changed

Read the NOTE's last section first: "Jarvis's re-check — BOUNCE on J3 and J4, and the fix". Then
CP-65, and CP-63's force-kill paragraph, in `docs/concurrency-pitfalls.md`.

- **The lifecycle marker.** In `kernel/src/arch/x86_64/syscall.rs`, `inference_syscall` replaces
  `after_inference`. It wraps the four inference arms in `syscall_dispatch` and:
  - sets `Thread::inference_lifecycle` (`sched/thread.rs`, via
    `scheduler::set_current_inference_lifecycle`) and bumps `Process::inference_participants`
    (`sched/process.rs`) before the arm runs;
  - takes the exit decision (`torn_down`) while still marked;
  - clears both, drops its process `Arc`, then exits if torn down.
- **The kill path.**
  - `scheduler.rs`: `kill_thread_by_pid` uses `position_by_pid_killable`, which skips every marked
    thread.
  - `process.rs`: `terminate`'s accounting assertions count `inference_participants > 0` as handled.
- **Cooperative cancellation.**
  - `kernel/src/mm/inference_stack.rs`:
    - `cancel_requested` (a torn-down, non-kernel process);
    - `Held::acquire` returns `None` from its retry loop on a teardown;
    - `try_run` / `try_run_checked`; `run` / `run_checked` keep their contract for kernel callers.
  - `syscall.rs`:
    - `on_inference_stack(cancelled, f)`;
    - `real_lm_generate` uses `try_run`, which gives `ECANCELED`;
    - `generate_inventory_greedy` checks before prefill and every token, and `constrained_loop` checks
      every token (`inference_cancelled`).
- **Coherent ownership.** `Held` publishes and clears `BUSY` plus `OWNER_TID` / `OWNER_PID` inside
  `without_interrupts`. `owner_thread` / `owner_pid` are test-only readers now. `owned_by_pid` is gone.
- **The gate.** `test_harness.rs`, `run_inference_stack_kill`:
  - scenarios `inference_stack_kill` and `inference_stack_kill_control`;
  - `Thread::attach_process_for_test` gives a caller thread its process's pid and pmap;
  - the control's kill rule is `position_by_pid_killable` under `neo-test-inference-kill-control`.

Measured locally, at `NEO_SMP=4`:

- **The waiter:** `terminate=0 boundary_exit=true cancelled=1 forward_ran=false holder_still=true`.
- **The holder:** `terminate=0 boundary_exit=true left_park_on_cancel=true busy_after=false
  owner_after=None`.
- **Baselines:** `arcs=1/1 frames_delta=0 served=42 returned=0`.
- **The control:** `arcs=2/1 frames_delta=5`, with no boundary exit for the zombified waiter.

## Your job: refute the fix

1. **Coverage of the marker.**
   - Is every path from a syscall into an inference forward (all four arms) marked before its first
     resource-owning local? Are there other entries into `inference_stack::run` / `try_run` from a
     killable context?
   - Can a marked thread be zombified by any other path: `zombify_parked_thread_by_pid`, a
     channel-parked extract, fault kill, a browser-runtime multi-thread terminate loop?
2. **Cancellation.**
   - Does a waiter always notice a teardown, given that `torn_down` is read without a lock?
   - Can the holder hang without reaching a token boundary: a long prefill, a single huge masked
     forward, `par_matmul`'s barrier?
   - Is `ECANCELED` safe for every caller?
   - Are kernel-thread callers (pid 0) ever cancelled?
3. **The boundary (J4).**
   - Is the window between the exit decision and marker-clear, and after it, sound?
   - Is the residual named in CP-65 (a Running thread on its syscall return path) honestly
     pre-existing and not made worse?
4. **The owner tuple.** Is masked publish/clear enough? Does anything still rely on `OWNER_*` for a
   safety decision?
5. **The gate.**
   - Does `inference_stack_kill` really put both A and B in a Ready queue at the moment CPU 1's
     terminate runs? (The BSP holds its CPU with `disable_preemption`, interrupts on.)
   - Does it prove destructor recovery: the model `Arc` count, frames, no heap growth?
   - Could it pass with J3 present? The control shows the leak.
   - Is the kernel-thread-attributed-to-a-process stand-in fair, against a ring-3 caller?
6. **Regressions.**
   - With `inference-stack` off, is the kernel unchanged?
   - Do the overflow, shootdown, real-LM and agentic paths still hold?

A finding bounces the branch only if it survives your own re-verification. "No findings" must say what
you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\jarvis    — yours alone, never another coder's folder, never the repo root
BRANCH:   codex/b1c-code-aag-recheck2-jarvis
          (git fetch origin && git checkout -B codex/b1c-code-aag-recheck2-jarvis origin/claude/b1c-product-generation)
```

```
git fetch origin
git show origin/claude/b1c-product-generation:Plans/DISPATCH-2026-10-20-b1c-codex-aag-recheck2-jarvis.md
git show origin/claude/b1c-product-generation:Plans/NOTE-2026-10-20-b1c-product-generation-s1.md
git diff 056f89fe..origin/claude/b1c-product-generation
```

Optional runs (they need `LIMINE_DIR`; last time your worker lacked `/home/neo/limine`):
- `NEO_SMP=4 cargo xtask neo-test inference_stack_kill inference_stack_kill_control`

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?" questions
(faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='kill_thread_by_pid zombify ready thread inference lifecycle'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-20-b1c-codex-aag-recheck2-jarvis.md` to your branch and **push it**. It
must contain:
- a verdict on J3 and J4 (closed or still open);
- any new finding, with its severity, file:line, and a repro or the reasoning;
- for each item above, what you checked.

Paste the pushed commit SHA in your reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards; never
delete the worktree.
