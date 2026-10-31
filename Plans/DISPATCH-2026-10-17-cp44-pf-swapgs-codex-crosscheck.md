<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of CP-44 (`page_fault` per-CPU reads on the user GS) → Janus (Codex)

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file. **Why a Codex seat:** CLAUDE.md
requires a different-model-family lens on any interrupt-core merge. The author and the AAG lenses on this
change are all Claude-family, so their agreement is weak evidence. Janus already reviewed the CP-38/39/40 entry
paths this change builds on, so one pass is cheapest. Any Codex seat (Jarvis/Chad) can take it instead.

## What is under review

PR #23, branch `claude/tender-thompson-2u1err`, rebuilt on `main` after #15/#18/#20/#21/#22 merged:
`git diff origin/main...origin/claude/tender-thompson-2u1err`.

- **The defect.** `page_fault` (`kernel/src/arch/x86_64/idt.rs`) sits behind CP-40's `exception_entry` shim,
  which clears AC/DF but does not swap GS, so a ring-3 fault arrives on the user GS base (0). Its first real call
  was `record_page_fault`. With `kernel-observability` on, that reads `gs:[24]` through `current_cpu_id()`. The
  read hits `0x18` and raises a nested `#PF`, which repeats the read until the stack guard and `#DF`. Measured
  under `-d int`: one `cpl=3` guard fault, then 226 nested `cpl=0` faults with `CR2=0x18`, `GS base=0`, then
  `#DF`. The only `swapgs` was inside the `browser-runtime` fault-kill block, after the observability call.
- **The fix.** `swapgs` first thing in `page_fault`, keyed on the saved CS.RPL (`sf.code_segment.0 & 3 == 3`,
  the same predicate as the IRQ stubs' `test [rsp+8], 3`), for every build. It runs ahead of CP-40's test-only
  AC counters, which read only the frame and RFLAGS. The `browser-runtime` block keeps only CP-41's
  `load_kernel_fp_control`. Nothing swaps back: the claim is that `page_fault` never returns.
- **The discriminator.** `threads_guard_fault_observability` = the `threads_guard_fault` battery built with
  `kernel-observability`. CP-40's AC counters are widened to it, so its worker's AC=1 guard fault also runs
  CP-40's ring-3 AC leg. A hook in `handle_page_fault` (`kernel/src/test_harness.rs`, `note_ring3_pf_trace`)
  asserts two things for a ring-3 fault: the `IA32_GS_BASE` MSR points at a `PER_CPU` slot, and the trace ring's
  newest page-fault event for this `cr2` carries that slot's index as its cpu id (`latest_trace_cpu_id`,
  `kernel/src/observability.rs`). The index comes from the MSR, the traced id from `gs:[24]`. The scenario then
  requires at least one such fault. With the swap reverted it hits `#DF` and times out; with it, it passes.
- **Found on the way, not fixed here** (recorded in CP-44's text):
  - a note that callees of error-code exception handlers run with RSP misaligned by 8, with a concrete `#GP` on
    the kill path via `purge_pid` → `recompute_earliest` (`movaps [rsp+0x90]`) when the process had a timed
    futex waiter. The hook is scalar-only for that reason. A later PR;
  - `syscall_entry`'s exit runs `swapgs; iretq` with no `cli`, while `spawn_user` returns with IF=1;
  - the fatal handlers never swap, which matters in bare-metal test builds.

## Your job: refute it

Default to suspicious. Specifically try to break:

1. **Never returns.** Is there any cfg combination (`neo-test` on/off, `browser-runtime` on/off, any scenario
   feature) where control reaches the end of `page_fault` and `iretq`s to ring 3 on the kernel GS? Walk
   `handle_page_fault` and every function it calls, `fault_kill_current_user`, `unexpected_trap`.
2. **The predicate.** CS.RPL for the swap, `USER_MODE` for the fault-kill. Can they disagree in a way that
   matters? Can `#PF` be delivered at CPL 0 while the user GS is live (between an exit `swapgs` and its `iretq`,
   in an exception shim, or at the `swapgs` that opens `syscall_entry`)? Can NMI/`#MC` nesting before the swap
   cause a double swap or no swap?
3. **The switch-away.** After the swap, KERNEL_GS_BASE holds the user value, and the fault-kill switches threads
   for good. Is the next thread's GS state (kernel mode and its next return to ring 3) identical to what the
   removed `browser-runtime` swap produced? Does `load_kernel_fp_control` still run where CP-41 needs it?
4. **Sibling holes.** Any other entry path that reads per-CPU state via GS before establishing the kernel GS.
   In particular, confirm or refute the `syscall_entry` exit window (no `cli` before `swapgs; iretq`,
   `spawn_user` returns with IF=1), and say whether any other exit (the timer stub after an `on_tick` switch,
   the IRQ stub, the trampolines) can reach its `swapgs` with IF=1. Would moving the swap into the CP-40 shim,
   keyed on each vector's frame layout, be the better home?
5. **Discriminator.** Can it pass vacuously, or fail spuriously under `NEO_SMP=4` (concurrent trace-ring
   writers, ring wrap)? Is `latest_trace_cpu_id`'s seq/wrap check sound? Does widening CP-40's AC counters to
   the new feature change what `threads_guard_fault` itself asserts?
6. **The alignment note.** Do you reproduce the misalignment reading (`page_fault` prologue: ten pushes +
   `sub rsp, 0x278`, error-code frame at a 16-aligned E), and the `recompute_earliest` instance? Is the
   proposed repair sound? Anything a Claude reviewer would structurally miss?

A finding bounces the change only if it survives your own re-verification; uncertain items go in the RETURN as
notes. "No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/cp44-crosscheck-janus        (git fetch origin && git checkout -B codex/cp44-crosscheck-janus origin/claude/tender-thompson-2u1err)
```

Read this dispatch and the code from the branch (it is not on `main`):

```
git fetch origin
git show origin/claude/tender-thompson-2u1err:Plans/DISPATCH-2026-10-17-cp44-pf-swapgs-codex-crosscheck.md
git diff origin/main...origin/claude/tender-thompson-2u1err
```

Building/running is optional. `cargo xtask neo-test threads_guard_fault_observability` should PASS. For the
control, revert only the swap: delete the `if sf.code_segment.0 & 3 == 3 { … swapgs … }` block at the top of
`page_fault`, and put `unsafe { core::arch::asm!("swapgs", options(nostack, preserves_flags)) };` back as the
first statement of the `#[cfg(feature = "browser-runtime")]` block. Do not revert the whole `idt.rs` diff: the
cfg widening of CP-40's AC counters must stay, or the scenario no longer compiles. The control should print
`DOUBLE_FAULT (#DF)` where `threads_guard_fault` prints `ring-3 #PF … fault-killing process`, then time out at
180 s. Set `NEO_TEST_ATTEMPTS=1` to skip the retries.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='swapgs exception handler per-CPU GS base'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-17-cp44-codex-crosscheck-janus.md` to `codex/cp44-crosscheck-janus` and **push
it**. A review delivered only in chat does not exist. Contents: verdict (MERGE / MERGE-WITH-NITS / BOUNCE),
each finding with severity + file:line + repro or reasoning, and for each numbered item above what you
checked. Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane; if you built, run `cargo clean` in your worktree afterwards. Never
delete the worktree itself.
