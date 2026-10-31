<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of CP-44 (`syscall_entry` rcx/r11 reload) → Chad (Codex)

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file. **Why a Codex seat:** this
is security-relevant entry-path asm, and everything on PR #21 so far (the author and every local control) is
Claude-family, so agreement among them is weak evidence. The seat is a suggestion; any Codex-family seat
(Jarvis/Janus/Chad) fits.

**Relay timing:** PR #21 is serialized after PR #18 (both edit the `syscall_entry` stubs) and will be
rebased onto `origin/main` once #18 merges. Relay this after that rebase, so the review covers the rebased
head. The rebased SHA is in the PR's latest push.

## What is under review

PR #21, branch `claude/zen-tesla-yk5dkx`: review the whole branch diff
(`git diff origin/main...origin/claude/zen-tesla-yk5dkx`).

- `kernel/src/arch/x86_64/syscall.rs`: SYSCALL overwrites `rcx` (return RIP) and `r11` (RFLAGS). Both
  `syscall_entry` variants (`xsave-fpu` and non-`xsave-fpu`) copied them into the IRETQ frame and restored
  every other GP register from the save area, but never gave `rcx`/`r11` a value again before `iretq`, so
  ring 3 received kernel scratch after every syscall (kernel heap pointers on `SYS_CHAN_CREATE`, which give
  the KASLR slide). Both variants now do `mov rcx, [rsp]` / `mov r11, [rsp + 16]` after the GP pops, right
  before `swapgs; iretq`: what SYSRET would leave. The return stays IRETQ.
- Same file: test-only `SYS_TEST_DIRTY_SCRATCH` (`0xF01`), `cfg(feature = "neo-test-syscall-return-clean-regs")`,
  which leaves a kernel text address in both registers.
- New scenario `syscall_return_clean_regs` (`kernel/src/test_harness.rs`, module `syscall_return_clean_regs`):
  a ring-3 probe makes five syscalls (unassigned number, `SYS_WRITE`, `SYS_CHAN_CREATE`, `SYS_SCHED_YIELD`,
  `SYS_TEST_DIRTY_SCRATCH`) with RFLAGS `0xAD7` and dumps `rcx`/`r11`/`rax` and its own return RIP after each.
- `docs/concurrency-pitfalls.md` CP-44 has the evidence: before the fix every attempt fails with
  `leaked=enosys:rcx,write:rcx,chan_create:rcx,sched_yield:rcx,dirty_scratch:rcx+r11`; an rcx-only half-fix
  fails `leaked=dirty_scratch:r11`; after the fix it passes (qemu64 SMP 1/4, `-cpu max`, and the
  non-`xsave-fpu` build locally).

## Your job: refute it

Default to suspicious. Specifically try to break:

1. **Frame offsets after the rebase.** In both variants, after the last `pop rbx`, is RSP exactly at the IRETQ
   frame (`[rsp]` = RIP, `[rsp+16]` = RFLAGS)? PR #15 adds an x87/SSE save and
   `pushfq; and qword ptr [rsp], -263169; popfq` to interrupt stubs, and PR #18 adds `ldmxcsr` after the FPU
   save in `syscall_entry`: does anything that landed before this rebase change the stack layout at the
   epilogue, or leave a push unbalanced?
2. **Other ways back to ring 3.** Any path that returns to ring 3 from a syscall without this epilogue
   (`SYS_EXIT`, `SYS_THREAD_EXIT`, a kill or fault while in the syscall body, a context switch that resumes a
   different thread's user context). Any path that rewrites the IRETQ frame before return: is it acceptable
   that `rcx`/`r11` then follow the rewritten RIP/RFLAGS?
3. **SYSRET equivalence.** Is `r11 = frame RFLAGS` always the RFLAGS IRETQ actually restores (RF, reserved
   bit 1, anything the kernel might modify in the frame)? Does it matter that SYSRET would mask r11 with
   `0x3C7FD7`, while IRETQ uses the frame value?
4. **The epilogue window.** IF state between the pops and `iretq`; whether placing the loads before `swapgs`
   matters for any interrupt/NMI/#MC interleaving. Nothing new should be introduced: confirm.
5. **The test-only syscall.** Can `SYS_TEST_DIRTY_SCRATCH` reach any build that isn't this scenario's? Is the
   claim "the dirty-scratch leg is what makes the r11 half non-vacuous" right? Specifically, does the
   `asm!`'s `rcx`/`r11` value really survive to the epilogue on the unfixed code, or did the half-fix control
   pass for another reason?
6. **Coverage.** The gate runs this scenario on qemu64 only (the `xsave-fpu` build's FXSAVE fallback). The
   asm under test is identical under XSAVE, but the non-`xsave-fpu` variant never runs in CI (PR #19 only
   *assembles* it). The probe runs on whichever CPU picks it up. Which of these gaps matter?
7. **What else ring 3 can still see after a syscall.** Every GP register should now be its own saved value,
   `rax`, or a SYSRET-defined value. Check that claim. MXCSR is PR #18's and x87 FIP/FDP on pre-Zen2 AMD is
   in PR #17's residuals; flag anything else.
8. **Anything else** a Claude reviewer would structurally miss.

A finding bounces the PR only if it survives your own re-verification; uncertain items go in the RETURN as
notes. "No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/cp44-crosscheck-chad         (git fetch origin && git checkout -B codex/cp44-crosscheck-chad origin/claude/zen-tesla-yk5dkx)
```

Read this dispatch and the code from the PR branch (it is not on `main`):

```
git fetch origin
git show origin/claude/zen-tesla-yk5dkx:Plans/DISPATCH-2026-10-17-cp44-syscall-return-codex-crosscheck.md
git diff origin/main...origin/claude/zen-tesla-yk5dkx
```

Building/running is optional: `cargo xtask neo-test syscall_return_clean_regs` should PASS. Deleting the two
`mov rcx`/`mov r11` lines from the `xsave-fpu` variant should make it FAIL with all five legs leaking `rcx`
and `dirty_scratch` also leaking `r11`. Deleting only the `mov r11` line should FAIL with
`leaked=dirty_scratch:r11`.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='syscall entry iretq return path register restore'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-17-cp44-codex-crosscheck-chad.md` to `codex/cp44-crosscheck-chad` and
**push it**. A review delivered only in chat does not exist. Contents: verdict (MERGE / MERGE-WITH-NITS /
BOUNCE), each finding with severity + file:line + repro or reasoning, and for each numbered item above what
you checked. Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane; if you built, run `cargo clean` in your worktree afterwards. Never
delete the worktree itself.

## Addendum — rebased and renumbered (2026-10-17)

PR #21 is rebased onto `main` @ `d9e179ab` (after #15, #18 and #20). The pitfall is now **CP-43**: `main`'s
CP-42 is the `stac`/`clac` entry. This file and the RETURN path above keep their `cp44` names so the paths
already relayed still work. Review the rebased head of `claude/zen-tesla-yk5dkx`. `syscall_entry` now also
carries #18's `ldmxcsr [rip + KERNEL_DEFAULT_MXCSR]` after the FPU save in both variants (item 1).
