<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 366 (#72 code review) and Seat C's next lane (2026-11-09)

Read with `git fetch origin && git show origin/main:<path>`; PR heads with `git fetch origin +pull/<N>/head:refs/pr/<N>`.

## Section 366: Janus, #72 (L1a.1 step 2b: stdio chosen at spawn). **MODEL** GPT-6.1-Sol, high: kernel spawn path, new syscalls, cross-process handles.
Head `576cd862`; PR run 42493229380 is green. The design is THINK 2b revision 2, which Dexter passed in relays 322/325.
§7 of the THINK lists every place the build departs from that revision. Answer each item pass/fail plus `file:line`,
with a concrete input for any fail.
- [ ] The stdio selection is stored on the child before `register`/`try_push_ready`
  (`process.rs` `spawn_user_body!`, `Stdio::for_spawn`): no window where the child runs without it.
- [ ] `sys_spawn_impl`'s stdio checks (step 2 and step 4, `stdio_roles_ok`) refuse every bad combination. Check the
  wrong pipe end, a closed handle, a handle of another kind, and a slot out of range; name the refusal for each.
- [ ] `pipe_lookup`'s `expect` and `buffer_handle_lookup_at` check the `CapId` before using the handle.
- [ ] The fd 0/1 arms in `sys_read_impl`/`sys_write_impl` (`selected_stdio`) copy the selection out before resolving
  it, so a concurrent close gives `EBADF`, not a use-after-free. This is the CP-112 case; check it against the CP table.
- [ ] `SYS_STDIO_INFO` and `SYS_STDIO_TERM_MAP`: the error, all-none and term-identity cases match the THINK. The
  SDK-ABI rows match the kernel's numbers.
- [ ] Endpoint counting: the child's copy is retained, and its exit releases it, with no leak and no early EOF.
- [ ] Kill-switch: with `pipe` off, spawn and console stdio are today's, unchanged.
- [ ] The late-control's witness would fail on the bug it guards. It has only been clippy-checked so far: say whether
  that's acceptable for a control, or name the run it needs.
- [ ] The CP table (CP-14/33/61/67/112): each class is introduced, guarded or N/A, with its discriminator.
RETURN: `Plans/RETURN-2026-11-09-pr72-review-janus.md` on `codex/pr72-review-janus`.

## Seat C: Arthur's drift check on Q8_0 (host-only)
Arthur moves to Q8_0 in #90. The drift harness in #84 measured the old Q4_K_M. Once #84 is ready (Jarvis's re-check,
relay 364), stack a branch on `dexter/quant-drift`, `claude/quant-drift-q8`, that runs the same bars on the shipped
Q8_0 `.neom` from #90:
- Bar A: ours vs llama.cpp on the same Q8_0 file.
- Bar B: Q8_0 vs the same-checkpoint F16.
- Control C, unchanged.
Use the same pinned window and IDs. Report the numbers in a short THINK addendum and re-pin a Q8 KAT next to the Q4 one,
keeping the Q4 KAT as the historical record. Don't change the bars.

Gate: host only. Run the close-gate, scan, and open a PR stacked on #84. The PM routes its review.

## Section 368: Janus, #72 narrow re-check of F1 and F2 (your relay 366). **MODEL** GPT-6.1-Sol, medium. Head `fc964d65`.
New commits: `40f23912` (the kill leg accepts ESRCH; buffers closed), `30f5cfb6` (every setup step names itself on failure),
`fc964d65` (leg 3 waits for an observed fd-0 park before the kill).
- [ ] **F1:** the late control was run at runtime (`neo-test-pipe-stdio-late-control`, with `NEO_SMP=2` too), failed for
  the designed reason, and the normal `pipe_stdio` passed. Cite the runs and the serial lines. Pass/fail.
- [ ] **F2:** leg 3 issues the kill only after an observed park of the target pid on its selected stdin, with a bounded,
  named failure if the park never comes. Pass/fail plus line.
- [ ] Accepting `ESRCH` in the kill leg doesn't hide a real failure (say when `ESRCH` is legitimate here). Pass/fail plus line.
RETURN: `Plans/RETURN-2026-11-09-pr72-recheck-janus.md` on `codex/pr72-recheck-janus`.

## Seat E, next lane: make the size budget stable
#82 showed that image A's kernel `.text` swings by about 64 KB on almost any kernel change (codegen-unit partitioning),
and the ISO by about 1 MB (unstripped debug info). A gate that moves 4% by itself will keep failing innocent PRs. Fix
the measurement, not the ceilings:
1. Measure a `.text` that doesn't depend on codegen-unit layout. Either build the measured kernel with
   `codegen-units=1`, or thin LTO, matching or recording what the product build uses. Show by running both builds that a
   behaviour-free change moves it by less than 1 KB.
2. Measure the ISO with the kernel's debug sections excluded, or stripped as shipped, so debug info doesn't swing it.
3. Re-baseline both ceilings from the new, stable numbers with the file's margins, and keep the reasons.
4. Add a KAT or self-test showing the measurement is stable: build twice with a no-op change and compare.

Gate: full-alone (`tools/xtask`, `.github/workflows`). Close-gate, scan, PR, then post PM:.

## Section 369: Chad, #92 (setup menu S3: the Time zone page and the one confirm). **MODEL** GPT-6-Luna, high: contained userland.
Head `46370757`. The design is `docs/THINK-2026-11-02-setup-menu.md` §5 (also §2.2, §3.4, §4.4).
- [ ] The zone list comes from the attached `Zones` directory, and each entry shows the right id, abbreviation and
  offset. Check two zones by hand against the tz data. Pass/fail plus line.
- [ ] Search: typing filters the list; on this page Space types, and on every other page Space still chooses. Pass/fail.
- [ ] Leaving the page with a zone chosen gives one confirm (`Outcome::Confirm(id)`), the clock shows the new zone's
  time, and declining leaves the old zone untouched. Pass/fail plus line.
- [ ] The zone is written through the S2 store (checked, read back, applied live), and nothing is written when the
  store is volatile. Pass/fail plus line.
- [ ] The scenario's witness would fail on a wrong zone or a missing confirm. Pass/fail plus the xtask line.
- [ ] Off by default; with the feature off, today's image is unchanged. Pass/fail.
RETURN: `Plans/RETURN-2026-11-09-pr92-review-chad.md` on `codex/pr92-review-chad`.

## Section 370: Dexter, #93 (THINK C3c-1 step 4: SpiderMonkey green on target). DeepSeek, high: docs only, light. Head `21b56b16`.
- [ ] The run it cites (42556683335) is green on all four QEMU shards, and `mozjs_smoke` passed in it. Check the job log.
- [ ] xtask requires every `ok(n)` line it claims, so a missing line fails the scenario. Pass/fail plus the xtask line.
- [ ] What the THINK says is still missing for C3c-1 (JIT off, single thread, the product-crates PR) matches the code. Pass/fail.
RETURN: `Plans/RETURN-2026-11-09-pr93-design-dexter.md` on `dexter/pr93-design`.

## Section 371: Janus, #72 F1 only (your relay 368). **MODEL** GPT-6.1-Sol, medium. Head `70fa3e7e`.
Seat B's evidence is on #72: comment 6732593123 and the 05:40 comment.
- The late-control dispatch run 42555777440 failed on shard 4: `timing pipe_stdio 30.1 18.1 12.0 4 FAIL`, with
  `PIPE_STDIO fail step=leg_kill:prompt` and the console leak `neo> [ kernel ] pipe_stdio late-control: window hit`.
- Every CI shard boots with `NEO_SMP=4` (`.github/workflows/ci.yml`).
- The normal `pipe_stdio` at the head is in run 42564447951.

Check:
- [ ] The control fails for the designed reason: the old selection used, so the prompt reached serial, not the pipe. A
  missed window or a generic failure doesn't count. The fact that it surfaces at leg 3 rather than as
  `pipe_stdio_prompt_not_in_pipe` should be explained by the design. Pass/fail plus the log lines.
- [ ] `NEO_SMP=4` covers the multi-CPU requirement your F1 asked for with `NEO_SMP=2`. Pass/fail.
- [ ] The normal `pipe_stdio` passes at `70fa3e7e`. Pass/fail plus the run's timing line.
RETURN: `Plans/RETURN-2026-11-09-pr72-recheck2-janus.md` on `codex/pr72-recheck2-janus`.

## Section 372: Jarvis, #94 (quant drift on Arthur's Q8_0). **MODEL** GPT-6-Luna, medium: a host-only addition to the harness you reviewed (relays 334/364). Head `907cc447`, stacked on #84.
- [ ] The Q8 run uses the same prompts, IDs, window and llama.cpp commit as the Q4 KAT, and `ref/run-q8.sh` refuses
  inputs that aren't the pinned ones. Pass/fail plus line.
- [ ] Splitting our run into four processes gives the same rows as one run: the boot prompt's two copies are identical,
  and the merge preserves order. Pass/fail.
- [ ] Bars A and B are computed unchanged, and the "distinct prefix" figure (96.09%) is reported beside the bar, not
  in place of it. Pass/fail plus line.
- [ ] The THINK addendum corrects the Q8 row: 99.4% on clear positions was llama.cpp's Q8_0, not ours. Pass/fail.
- [ ] The speed note (9.1 vs 3.7 s/token, host scalar) is labelled as a host measurement, not the Zenbook. Pass/fail.
RETURN: `Plans/RETURN-2026-11-09-pr94-review-jarvis.md` on `codex/pr94-review-jarvis`.

## Section 373: Chad, #92 re-check of the confirm (your relay 369). **MODEL** GPT-6.1-Sol, medium. Head `60c1e1e5`.
The fix is `21272805` ("the confirm is transactional").
- [ ] Declining the confirm, or a refused or failed write, leaves the old zone and RTC mode in place, and the clock
  shows the old zone. Pass/fail plus line.
- [ ] A scenario leg declines and checks that the stored id and the clock are unchanged. Pass/fail plus the xtask line.
- [ ] Your other five items still hold. Pass/fail.
RETURN: `Plans/RETURN-2026-11-09-pr92-recheck-chad.md` on `codex/pr92-recheck-chad`.

## Section 374: Jarvis, #94 re-check of item 1 (your relay 372). **MODEL** GPT-6.1-Sol, medium. Head `572fb6ff`.
- [ ] `ref/run-q8.sh` refuses prompt IDs that differ from the pinned set, and any llama.cpp commit other than
  `a2b30183`. Pass/fail plus line, and an input that each refusal catches.
- [ ] The split-run TSVs (or their hashes) are committed, so the merge check is reproducible. Pass/fail.
RETURN: `Plans/RETURN-2026-11-09-pr94-recheck-jarvis.md` on `codex/pr94-recheck-jarvis`.

## Section 375: Dexter, #95 (your relay-341 notes on #82, folded in). DeepSeek, high: docs plus one comment, light. Head `5fe0eb13`.
- [ ] Each of your seven notes is answered, and the only code change is a comment. Pass/fail per note.
RETURN: `Plans/RETURN-2026-11-09-pr95-notes-dexter.md` on `dexter/pr95-notes`.

## Section 376: Chad, #96 (size-budget: measure a stable image A). **MODEL** GPT-6-Luna, high: build tooling and CI.
Head `697ad91c`. The problem is in the PR body and #82 (a 6-line no-op moved `.text` by +64,384 B and the ISO by ~1 MB).
- [ ] The measured kernel is built with `codegen-units=1` (or the PR's stated equivalent), and the measurement
  matches what it claims: the product's own image A, or a stated, fixed proxy for it. Pass/fail plus line.
- [ ] The ISO measurement excludes the kernel's debug sections (or measures the stripped kernel), so debug info can't
  swing it. Pass/fail plus line.
- [ ] `--stability` shows that a behaviour-free change moves the number by less than the stated tolerance. Check the
  run it cites. Pass/fail.
- [ ] The new ceilings come from the new measurement plus the file's margins, with reasons recorded. Pass/fail.
- [ ] Nothing changes the product image itself; only the measurement changes. Pass/fail plus line.
RETURN: `Plans/RETURN-2026-11-09-pr96-review-chad.md` on `codex/pr96-review-chad`.

## Section 377: Chad, #92 re-check of the all-or-nothing confirm (your relay 373). **MODEL** GPT-6.1-Sol, medium. Head `5dd0e4e9`.
The fix is `3cebc991`.
- [ ] A failed or refused settings write leaves the stored zone, the RTC mode and the live clock all unchanged:
  nothing is applied live before the write is read back, and a partial write is rolled back. Pass/fail plus line.
- [ ] A test forces the write to fail and checks all three are unchanged. Pass/fail plus line.
RETURN: `Plans/RETURN-2026-11-09-pr92-recheck2-chad.md` on `codex/pr92-recheck2-chad`.

## Section 378: Chad, #96 re-check (your relay 376). **MODEL** GPT-6.1-Sol, medium. Head `0eea4614`.
**PM decision on your item 5:** `codegen-units=1` for the product kernel is accepted. It changes the shipped image, but
deliberately: it's the standard release setting, it gives deterministic codegen, and the full gate ran with it. #96 should
say so in its PR body and in `tools/size-budget.toml`'s reason.
- [ ] The ceilings are re-baselined from #96's own CI measurement plus the file's margins (`1a09cd3b`), with reasons.
  Pass/fail plus line.
- [ ] The PR and the TOML state that the product kernel now builds with `codegen-units=1`. Pass/fail.
RETURN: `Plans/RETURN-2026-11-09-pr96-recheck-chad.md` on `codex/pr96-recheck-chad`.

## Section 379: Chad, #92 re-check against the PM's rule (your relay 377). **MODEL** GPT-6.1-Sol, medium. Head `1609c752`.
**The rule (PM, on #92):** the durable record is the single source of truth. After any error in the confirm, re-read
the store and apply exactly what it holds (zone, RTC mode, clock), and show the user an error. The fix is `d4671046`.
- [ ] Every error path in the confirm ends in one "reconcile from store" step, so the live state equals the re-read
  record. Pass/fail plus line.
- [ ] Your two scenarios (bytes land but fsync errors; refusal plus failed rollback) each have a test asserting
  live == re-read record. Pass/fail plus line.
- [ ] The user sees an error on any failure, and the THINK's confirm section states the rule. Pass/fail.
RETURN: `Plans/RETURN-2026-11-09-pr92-recheck3-chad.md` on `codex/pr92-recheck3-chad`.
