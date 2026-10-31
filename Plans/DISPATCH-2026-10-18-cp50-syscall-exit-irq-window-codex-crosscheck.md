<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of CP-50 (an interrupt between the syscall exit's `swapgs` and `iretq`) → Chad (Codex)

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file. **Why a Codex seat:** CLAUDE.md
requires a different-model-family lens on any interrupt-core merge. The author and the AAG lenses on this
change are all Claude-family, so their agreement is weak evidence. Any Codex seat (Jarvis/Janus) can take it
instead; Chad is named because Janus holds the CP-46 cross-check (PR #26).

## What is under review

Branch `claude/beautiful-carson-m4oyo5`, on `main` @ `2779c20b` (after #26):
`git diff origin/main...origin/claude/beautiful-carson-m4oyo5`. The pitfall-bank entry is CP-50 in
`docs/concurrency-pitfalls.md` (it closes CP-44's "maskable interrupt at `syscall_entry`'s exit" residual).

- **The defect.** Some syscalls return with IF=1: `spawn_user` (`kernel/src/sched/process.rs`) ends with
  `LocalShootdownQuiesce::resume()` (`kernel/src/mm/tlb_shootdown.rs`), which calls `enable_interrupts()`, and
  `SYS_SPAWN` returns straight through the exit. Both `syscall_entry` variants
  (`kernel/src/arch/x86_64/syscall.rs`) ended `… mov r11, [rsp+16]; swapgs; iretq` with no `cli`. A timer tick
  or IPI taken after the `swapgs` arrived at CPL 0 with the user GS live; `swapgs_irq_entry` (`idt.rs`) and the
  timer stub (`timer.rs`) swap only on a saved CS.RPL of 3, so the body's `gs:[24]` read faulted at `0x18` and
  recursed (CP-44).
- **The fix.** `cli` immediately before the exit `swapgs` in both variants; `iretq` restores the user's IF from
  the frame. Two global labels, `syscall_exit_swapgs` and `syscall_exit_iretq`, mark the window (no code).
- **The audit** (CP-50's table): the timer stub's and `swapgs_irq_entry`'s ring-3 exits and the
  `enter_userspace*` trampolines (`context.rs`) all reach their `swapgs` with IF=0, because every
  `context_switch` runs with IF=0 and every IF-setter a syscall or interrupt body can reach restores the state
  it found, except `resume()`.
- **The discriminator.** `syscall_exit_irq_window`, run under `-icount shift=0,sleep=off` (xtask
  `scenario_qemu_icount`). A ring-3 probe calls the test-only `SYS_TEST_IRQ_WINDOW` (`0xF02`) for counts
  1..=4096, twice; each call starts a one-shot LAPIC timer (`apic::start_oneshot_timer_for_test`) on test vector
  `0xE1`, routed through the real `swapgs_irq_entry`, and returns with IF=1. The body (`test_harness.rs`,
  module `syscall_exit_irq_window`) gets the interrupted frame from a naked shim (`lea rsi, [rbp + 136]`),
  records RIP/CS and whether `IA32_GS_BASE` points at a `PER_CPU` slot. It fails on any CPL-0 landing at
  either label or on the user GS, and requires a landing on the `cli` followed (next count) by one deferred to
  the probe's return address. Measured locally on QEMU 8.2.2: with only the `cli` removed it fails every
  attempt at `NEO_SMP=4` and `=1` (landings on `swapgs`, and on `iretq` with the user GS); with the fix it
  passes.
- **The Claude AAG.** The fix + IF-audit lens found no surviving defect; all six audit claims held. Its notes
  are folded into CP-50's residuals: the `SYS_SPAWN` tail before the `cli` still runs with IF=1, a faulting
  `iretq` (non-canonical user RIP) still takes `#GP` on the user GS, and NMI/`#MC` remain. The
  discriminator-validity lens found no false pass. Its points are fixed in `c7ca94dd`: the scenario checks
  the `cli` byte (`0xFA`) and requires exact landings on three or more boundaries of the exit block instead of
  a "next count lands at the user return" step that late delivery could satisfy, and the no-`cli` control was
  re-run on QEMU 10.2.3 built from CI's tag (fails at `NEO_SMP=4` and `=1`; the fixed build passes).

## Your job: refute it

Default to suspicious. Specifically try to break:

1. **The fix.** Is there any path from a syscall exit to `iretq` that still has IF=1 after the `cli` (e.g. a
   fault on `iretq` itself: what does the `#GP` path do on the user GS)? Is placing the `cli` after the GP pops
   and the FPU restore (rather than right after `call syscall_dispatch`) safe — can a preempting tick in the
   pops migrate the thread to another CPU mid-exit, and does anything after the pops depend on the CPU?
2. **The audit.** Find a `context_switch` / `switch_context*` caller that can run with IF=1, a path that resumes
   a thread into code that enables interrupts unconditionally before it returns into an interrupt stub's exit,
   an interrupt body that context-switches, or a syscall that can return with IF=1 other than `SYS_SPAWN`.
   Grep `enable_interrupts`, `interrupts::enable`, `sti`, `with_interrupts_enabled`, `IrqSafeMutex`.
3. **The discriminator.** Can it pass with the window still open (the "`cli` = `syscall_exit_swapgs - 1`"
   assumption, slot bookkeeping, the `rbp + 136` frame offset vs the `swapgs_irq_entry` push count, the icount
   granularity claim at `NEO_SMP=4`)? Can it flake on CI's QEMU 10.x or under `-icount` at `NEO_SMP=4`? Does
   the body touch anything GS-relative when it runs on the user GS?
4. **Production impact.** Is everything test-only correctly cfg-gated? Do the two unconditional global labels
   cause any problem (symbolization, a duplicate-symbol risk between the cfg-exclusive asm blocks)?
5. **Anything a Claude reviewer would structurally miss** — in particular the SDM/APM semantics of `cli`,
   `swapgs` and `iretq` here, and NMI/`#MC`/`#DB` in the window (documented residual, not fixed).

A finding bounces the change only if it survives your own re-verification; uncertain items go in the RETURN as
notes. "No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\chad     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/cp50-crosscheck-chad        (git fetch origin && git checkout -B codex/cp50-crosscheck-chad origin/claude/beautiful-carson-m4oyo5)
```

Read this dispatch and the code from the branch (it is not on `main`):

```
git fetch origin
git show origin/claude/beautiful-carson-m4oyo5:Plans/DISPATCH-2026-10-18-cp50-syscall-exit-irq-window-codex-crosscheck.md
git diff origin/main...origin/claude/beautiful-carson-m4oyo5
```

Building/running is optional. With `NEO_SMP=4`, `cargo xtask neo-test syscall_exit_irq_window` should PASS.
For the control, delete only the two `    cli` lines before `syscall_exit_swapgs:` in
`kernel/src/arch/x86_64/syscall.rs` (keep the labels); the scenario should then FAIL with
`reason=irq_inside_syscall_exit_window` and a `first_user_gs_rip` equal to the printed `iretq` address. Set
`NEO_TEST_ATTEMPTS=1` to skip the retries.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='syscall exit swapgs iretq interrupt flag'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-cp50-codex-crosscheck-chad.md` to `codex/cp50-crosscheck-chad` and **push
it**. A review delivered only in chat does not exist. Contents: verdict (MERGE / MERGE-WITH-NITS / BOUNCE),
each finding with severity + file:line + repro or reasoning, and for each numbered item above what you
checked. Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane; if you built, run `cargo clean` in your worktree afterwards. Never
delete the worktree itself.
