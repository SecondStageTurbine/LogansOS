<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 368, PR #72 F1/F2 re-check — Janus

Dispatch: `Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md`, Section 368, fetched from `origin/main` at `e3d39dbdb`.
Reviewed head: `fc964d6515bfc8c18ea19a2be58e4d8bab1dc925`, independently fetched as `refs/pr/72`.
RETURN branch: `codex/pr72-recheck-janus`, based on that main. Source line citations below are at the reviewed PR head.

**Verdict: REQUEST CHANGES / evidence outstanding. F1 FAIL; F2 PASS; ESRCH acceptance PASS.** This is the requested narrow re-check, with no production-code changes. F1 fails the evidence requirement; it is not a claim that the running control has produced the wrong result.

## F1 — FAIL: runtime expected-failure evidence is still incomplete

Independently queried GitHub's public Actions run and job APIs on 2026-11-09 at approximately 04:16 UTC:

| Run | Exact head | Observed status |
|---|---|---|
| [Normal PR run 42555771453](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42555771453) | `fc964d6515bfc8c18ea19a2be58e4d8bab1dc925` | In progress; all four QEMU shard jobs in progress, with no final scenario result verified. |
| [Late-control run 42555777440](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42555777440) | `d691eaffe7fc6ec79a019681f635957ea40738a8` | In progress; all four QEMU shard jobs in progress, with no designed failure or serial witness verified. |

The fetched throwaway control branch differs from `fc964d65` only at `tools/xtask/src/main.rs:5702`: `pipe_stdio`'s feature changes from `neo-test-pipe-stdio` to `neo-test-pipe-stdio-late-control`. This is a valid way to select the runtime control without modifying the implementation being tested. Starting that run does not establish its outcome.

**Serial evidence at the reviewed head: not yet verified for either run.** Both runs currently have no uploaded artifacts according to their Actions artifact-list APIs. I have no observed `pipe_stdio_prompt_not_in_pipe`, leaked shell banner/prompt, late-window-hit evidence, or normal final `PIPE_STDIO pass ...` line from these runs to cite. Source strings are not runtime observations. Their Build Checks jobs have already failed at **Size budgets — image A .text + ISO (blocking)**; that failure is not the control's designed failure and cannot close F1.

For historical context only, the author's [completed-run report for 42493229380](https://github.com/SecondStageTurbine/LogansOS/pull/72#issuecomment-6724264881), at the earlier `576cd862` head, quotes:

```text
PIPE_STDIO pass legs=round,eof,kill,stale,refusals,info,shell_child,term,unselected
timing pipe_stdio 9.6 2.7 6.9 1 pass
```

Those are author-reported earlier-head lines, not independently recovered serial from `fc964d65`; that run predates the new park witness. The [author's latest report](https://github.com/SecondStageTurbine/LogansOS/pull/72#issuecomment-6731424064) explicitly says the control is being run and `NEO_SMP=2` is not covered. Independently, `.github/workflows/ci.yml:578` hard-codes `NEO_SMP=4` for these QEMU jobs. Four CPUs do not satisfy the specifically requested two-CPU trial.

To close F1, retain completed runtime serial and exit status for the late-control at this implementation, including `NEO_SMP=2`, and the normal passing `pipe_stdio`. Verify an observed old-selection-use failure (`pipe_stdio_prompt_not_in_pipe` with console leakage), rather than a missed window, generic timeout, or build-budget red. If the banner/quiet-prefix path is what detects the bug, reconcile its actual diagnostic with the THINK as requested in relay 366. No local QEMU runtime trial was executed in this re-check.

## F2 — PASS: an observed selected-stdin park precedes kill

`userland/neo-init/src/pipe_stdio_test.rs:194` calls `wait_parked(c.in_wr)` after observing the prompt. A false result returns the named `leg_kill:park_not_seen` at `:196`; `sys_kill(c.process)` is reached only at `:198`. `wait_parked` (`:275`) polls test syscall `0xF0B`, accepts exactly one reader token (`:280`), and rejects an error or elapsed `WAIT_CYCLES` (`:283`). The bound is `1 << 35` TSC cycles (`:24`), not an assumed fixed wall-clock duration. `bail` emits `PIPE_STDIO fail step=leg_kill:park_not_seen` (`:291`). Missing prompt separately names `leg_kill:prompt` (`:192`).

The syscall resolves the caller's pipe handle and counts its read-side park tokens (`kernel/src/arch/x86_64/syscall.rs:1437`); it is gated to `neo-test-pipe-stdio` (`:1436`). `PipeObject::park` installs the read token through the scheduler's guarded blocking operation (`kernel/src/ipc/pipe.rs:319`, `:321`); `parked_for_test` reads the list under the same pipe-state lock (`:343`). This observes a token park, rather than inferring it from the earlier prompt write.

**Target identity is established by the fixture, not a pid argument to the test syscall.** `spawn_on_pipes` creates fresh pipes and passes the stdin read end only to this shell (`pipe_stdio_test.rs:130`, `:132`); it closes the terminal's read end (`:135`) and retains only the stdin writer and stdout reader. No other child or thread is given this fresh stdin read end. Before this observation no stdin bytes are written and its writer stays open, so the sole read waiter is the target shell reading its selected fd 0. The selected fd-0 path blocks on that pipe (`syscall.rs:7343`, `:7346`). A nonblocking fd-0 regression therefore cannot satisfy this witness.

`parked_for_test` counts occupied slots and can count stale tokens after a killed thread (`pipe.rs:340`). That limitation does not invalidate this specific pre-kill observation: the fresh pipe has no prior killed reader, and this shell is its sole reader. This is a source-level PASS on the requested discriminator; the new head's normal runtime result remains outstanding under F1.

## ESRCH — PASS: legitimate for wait after successful kill, never accepted for kill itself

The relaxed result is **`SYS_WAIT`**, at `pipe_stdio_test.rs:201`–`:202`: `Ok(_) | Err(ESRCH)`. The final condition still requires `parked`, `kill == 0`, `reaped`, stdout EOF and stdin EPIPE (`:218`). An ESRCH from `SYS_KILL`, an invalid target, a leaked reader or a leaked writer cannot pass simply because wait accepts ESRCH (`:198`, `:203`, `:204`).

ESRCH is legitimate when the successfully killed process completes without a retained natural exit code. `finish_kill` waits for teardown and the last live thread (`kernel/src/sched/process.rs:2747`), completes waiters with `None` (`:2756`), and publishes EXITED (`:2759`). A later wait finding neither a live/dying process nor a completed exit code returns ESRCH (`:4256`, `:4262`); a waiter may also receive the kill's ESRCH completion (`:4439`, `:4448`). The acceptance matches those kernel semantics and preserves the endpoint and successful-kill assertions. It does not establish the missing F1 runtime evidence.

## Validation and disposition

Read the three named commits and exact-head source, compared the control branch, and checked live Actions metadata and the author's reports. No implementation edits, new runtime trials, CI dispatches, or external messages were made. This RETURN is the only tracked change.

Close F2 and the ESRCH concern. Keep F1 open until the normal and expected-failure serial evidence, including the explicit two-CPU control, is supplied and checked. Do not infer a completed runtime PASS/FAIL from the current in-progress jobs or the unrelated size-budget failure.
