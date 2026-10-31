<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Codex re-check #3 of B1c — the narrowed marker and cancel-after-win → Jarvis

**Type:** review only. **Deliverable:** one pushed RETURN.

**Why:** your re-check #2 (`codex/b1c-code-aag-recheck2-jarvis` @ `12cb1af5`, reviewing `e6a75dee`)
closed J1 and J4 (scoped). J3/F5 stayed open on two findings:
- **J3-P (HIGH):** a marked graph `SYS_INFER_RUN` caller could be token-parked by admission and then
  zombified by `zombify_parked_thread_by_pid`.
- **C1 (MEDIUM):** a cancelled waiter could win `BUSY` after the holder released and still run its
  forward.

The PM ruled the fix. The fix is on `claude/b1c-product-generation`, and the PM names the head SHA when
relaying.

**Scope: the two ruled changes only.**
1. The lifecycle marker is narrowed to the inference-stack arms.
2. C1's cancellation after winning `BUSY`, plus its release-race gate.

**Out of scope, by PM ruling.** The pre-existing graph-admission kill leak is `main`'s behaviour:
`zombify_parked_*` ignores the frames of a token-parked graph caller. It goes to S2. `admission.rs` and
`zombify_parked_*` are untouched in this branch. Don't bounce on that leak itself; do bounce if the
graph route here differs from `main`'s.

## What changed

Read the NOTE's last section first: "Jarvis's re-check #2 — BOUNCE on J3-P and C1, and the fix". Then
read CP-65 in `docs/concurrency-pitfalls.md`, which has the marker scope and the release race.

**(1) The narrowed marker** (`kernel/src/arch/x86_64/syscall.rs`):
- `syscall_dispatch` wraps only `SYS_GENERATE`, `_CONSTRAINED` and `_MASKED` in `inference_syscall`.
  `SYS_INFER_RUN` calls `sys_infer_run_impl` directly.
- `sys_infer_run_impl` checks the flags, does `model_lookup` and `validate_current`, and classifies
  `stage_b_inventory`. Only the inventory route is wrapped in `inference_syscall`, before any
  allocation on it. The rest of the old body is `infer_run_routed`.
- The graph route has no marker and no participant count.
- **The check:** `INFERENCE_MARKED_ENTRIES` counts marked entries (test features only). The ABI probe
  makes one call per route: `infer_inventory_marked ret=1`, `infer_graph_marked ret=0`.

**(2) C1:**
- **`kernel/src/mm/inference_stack.rs`:** `Held::acquire` checks `cancel_requested()` right after
  winning `BUSY`. A cancelled winner drops the `Held` (which releases the stack) and returns `None`;
  `CANCELLED_AFTER_WIN` counts it.
- **`syscall.rs`:**
  - `inventory_infer` (the inventory `SYS_INFER_RUN` closure) and the masked closure check
    `inference_cancelled()` before their forward.
  - Inventory infer returns `ECANCELED`, not `EINVAL`.
- **The gate:** a release-race phase in `test_harness.rs` `run_inference_stack_kill`:
  - holder E parks on the stack, and a test gate (`WAITER_GATE_CLOSED`) holds waiter F just before
    its CAS;
  - CPU 1 terminates F; E is released and returns; the gate opens;
  - F's next CAS wins, and the post-win check cancels it.

**Also in this round:**
- **The owner-tuple wording.** The docs now say the owner tid and pid are separate atomics, with
  test-only readers.
- **A KASLR placement fix.** `inference_stack_logic::region_candidates` adds a third slot below the
  kernel image, with a host KAT for the layout that panicked one boot.

Measured locally, at `NEO_SMP=4`:
- **The release race:** `race terminate=0 holder_returned=true cancelled_after_win=1 forwards=1
  waiter_boundary_exit=true`. The one forward is E's.
- **Baselines:** `arcs=1/1 frames_delta=0 served=42 returned=1`.
- **The control** (`inference_stack_kill_control`) still leaks under the pre-J3 rule.
- **`real_lm_generate_boot`:** `infer_inventory_marked ret=1`, `infer_graph_marked ret=0`; all ABI
  cases ok.

## Your job: refute (1) and (2)

1. **The narrowed marker.**
   - Does the graph route of `SYS_INFER_RUN` now behave exactly as on `main`? Diff it against
     `origin/main`, including errno precedence and what is allocated before admission.
   - Is the inventory route marked before its first resource-owning local?
   - Does anything between entry and the route decision (`model_lookup`, `validate_current`) allocate
     or park?
   - Does any other path still reach `inference_stack::run` / `try_run` from a killable context
     without the marker?
   - Does the probe's marked-entry check prove it, or could it pass with the graph route still marked?
2. **C1.**
   - Is there any window left where a torn-down caller enters a forward: after the post-win check, in
     the closures' pre-forward checks, or in the masked one-shot?
   - Is `ECANCELED` handled by every caller of the inventory route?
   - Does a cancelled winner always release `BUSY` and the owner atomics?
   - Does the release-race phase put F in the exact interleaving (terminated, then the holder
     releases, then F's CAS wins)? Could it pass with C1 present? Remove the post-win check mentally:
     F would run its forward and `forwards` would be 2.
   - Are the test gate and its counters cfg-absent from the product build?

A finding bounces the branch only if it survives your own re-verification. "No findings" must say what
you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\jarvis    — yours alone, never another coder's folder, never the repo root
BRANCH:   codex/b1c-code-aag-recheck3-jarvis
          (git fetch origin && git checkout -B codex/b1c-code-aag-recheck3-jarvis origin/claude/b1c-product-generation)
```

```
git fetch origin
git show origin/claude/b1c-product-generation:Plans/DISPATCH-2026-10-20-b1c-codex-aag-recheck3-jarvis.md
git show origin/claude/b1c-product-generation:Plans/NOTE-2026-10-20-b1c-product-generation-s1.md
git diff e6a75dee..origin/claude/b1c-product-generation
```

Optional runs (they need `LIMINE_DIR`, `xorriso` and `qemu-system-x86_64`, which your worker lacked
last time):
- `cargo test -p module-reclaim-logic-tests`
- `NEO_SMP=4 cargo xtask neo-test inference_stack_kill inference_stack_kill_control real_lm_generate_boot`

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?" questions
(faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='SYS_INFER_RUN inventory route inference_syscall marker'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-20-b1c-codex-aag-recheck3-jarvis.md` to your branch and **push it**. It
must contain:
- a verdict on J3-P and C1, and so on J3 and F5 (closed or still open);
- any new finding, with its severity, file:line, and a repro or the reasoning;
- for each item above, what you checked.

Paste the pushed commit SHA in your reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards; never
delete the worktree.
