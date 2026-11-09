<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 371, PR #72 F1 only — Janus

Dispatch: `Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md`, Section 371, fetched from `origin/main` at `101483f77`.
Reviewed head: `70fa3e7eef61d184b69857b30f4251cda311418b`, independently fetched as `refs/pr/72`.
RETURN branch: `codex/pr72-recheck2-janus`, based on that main. Source citations refer to the reviewed head.

**Verdict: PASS on all three Section 371 checks; close F1.** This supersedes relay 368's outstanding F1 evidence requirement. F2 and ESRCH are outside this re-check and retain their previous PASS dispositions.

## 1. PASS — observed old-selection use, not a missed window or generic failure

[Late-control run 42555777440](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42555777440), head `d691eaffe7fc6ec79a019681f635957ea40738a8`, completed with failure. Its [shard-4 job](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42555777440/job/128961217878) failed. Independently downloaded its `neo-test-serial-logs` artifact (ID `13650572050`) and `neo-test-timings-shard-4` artifact (ID `13650059213`), rather than relying only on [comment 6732593123](https://github.com/SecondStageTurbine/LogansOS/pull/72#issuecomment-6732593123) and the [05:40 correction](https://github.com/SecondStageTurbine/LogansOS/pull/72#issuecomment-6733154145).

`pipe_stdio.attempt2.serial.log` contains:

```text
119: PIPE_STDIO round prompt=true reply=true
122: PIPE_STDIO eof exit=2 out_eof=true
130: [ neo-shell ] ready
131: neo> [ kernel ] pipe_stdio late-control: window hit
137: PIPE_STDIO fail step=leg_kill:prompt
138: PIPE_STDIO fail leg=kill
148: NEO_TEST/1 event=FAIL name=pipe_stdio reason=pipe_stdio code=11
```

All four attempt logs contain the hit, leaked prompt, named leg-3 failure and final code 11. None contains `pipe_stdio_late_window_not_hit`. The timing artifact's `timings.tsv:92` records `pipe_stdio 30.1 18.1 12.0 4 FAIL`, corresponding to:

```text
timing pipe_stdio 30.1 18.1 12.0 4 FAIL
```

The control branch differs from `fc964d65` only by selecting `neo-test-pipe-stdio-late-control` instead of `neo-test-pipe-stdio` in xtask. The requested head differs from `fc964d65` only in `tools/size-budget.toml`; the control therefore exercises the same implementation with the late-store fault enabled.

**Why leg 3 is the designed failure:** the control initially supplies NONE (`kernel/src/arch/x86_64/syscall.rs:2137`), waits for the child's first syscall, prints the hit and only then stores the real selection (`:2327`–`:2357`). A first syscall is not necessarily a prompt write. Legs 1/2 passed in this execution; the fresh shell in leg 3 emitted its banner and prompt while the old console selection was still active. The leaked `neo> ` is direct output evidence beyond the recorder's hit. Leg 3 reads that shell's fresh stdout pipe and returns `leg_kill:prompt` if the prompt never arrives (`userland/neo-init/src/pipe_stdio_test.rs:182`–`:192`), before its park/kill checks.

THINK §4 (`docs/THINK-2026-11-08-l1a1-stdio-selection.md:329`–`:341`) predicts the same fault at leg 1, whose failure string is `pipe_stdio_prompt_not_in_pipe` (`pipe_stdio_test.rs:152`–`:155`). Its prediction of the leg/string is too specific for the first-syscall recorder; the actual leg-3 message identifies the same missing piped prompt. The scenario terminates with code 11 before xtask's success-sequence/quiet-prefix checks (`tools/xtask/src/main.rs:19851`–`:19871`, `:22808`–`:22834`), explaining why the quiet-prefix diagnostic does not appear. This is an observed wrong destination for output, not a generic timeout accepted without a serial witness. No claim that leg 1 alone reliably discriminates the fault is needed.

## 2. PASS — four CPUs satisfy the multi-CPU requirement

The downloaded control's `pipe_stdio.attempt2.serial.log:48` independently confirms the boot configuration: `[ kernel ] smp: Limine reports 4 cpu(s); bsp_lapic_id=0`.

`.github/workflows/ci.yml:578` passes `-e NEO_SMP=4` to every QEMU shard, including the control's shard 4. xtask passes `smp_count()` to QEMU's `-smp` (`tools/xtask/src/main.rs:15266`–`:15267`); that function reads `NEO_SMP` (`:25032`–`:25033`). Four CPUs cover the multi-CPU requirement for which relay 366/368 requested two. A separate exact-two-CPU run is unnecessary under Section 371. This corrects relay 368's overly literal two-CPU requirement; it does not claim a two-CPU trial occurred or prove parent and child execute on different CPUs.

## 3. PASS — normal pipe_stdio at 70fa3e7e

[Normal run 42564447951](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42564447951) reports the exact reviewed head in the Actions API. Its [shard-4 job 128988034951](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42564447951/job/128988034951) completed successfully. Independently downloaded `neo-test-timings-shard-4` (artifact ID `13652933311`); `timings.tsv:92` records:

```text
scenario    wall_s qemu_s build_s qemu_runs result
pipe_stdio  12.6   3.8    8.8     1         pass
```

The corresponding timing line is:

```text
timing pipe_stdio 12.6 3.8 8.8 1 pass
```

This is the timing artifact's measured result, not an earlier-head result or an invented serial success line. The overall run was still in progress (shard 2) when checked; the requested `pipe_stdio` result on completed shard 4 is already final. No claim of a completed green full run is made.

## Validation

Checked the exact-head source, control-branch diff, GitHub comments, run/job metadata, control serial and both timing artifacts. Retrieved archives through `nightly.link` because GitHub's unauthenticated log/archive endpoints rejected access; all three downloaded ZIP SHA-256 digests match the digests independently returned by GitHub's artifact API. No new runtime trial or implementation change was needed. Only this RETURN is committed.
