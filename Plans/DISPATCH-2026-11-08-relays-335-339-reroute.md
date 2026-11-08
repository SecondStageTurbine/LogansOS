<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relays 335–339 (2026-11-08): Codex reviews rerouted; #73/#82 re-check

**Why.** No Codex RETURN has come back since 2026-11-03. The Codex relays queued this morning (315–321, 324,
328–332) have produced nothing, while DeepSeek relays through the same runner return within the hour. Until Codex
runs again, the reviews that block a landing go to Dexter (DeepSeek), which is cross-family for this Claude-authored
code. The rest stay queued for Codex.

Each section below names an earlier section to run **as written**, with the head given here. Read with
`git fetch origin && git show origin/main:<path>`; PR heads with `git fetch origin +pull/<N>/head:refs/pr/<N>`.
Answer pass/fail plus `file:line`, and push the RETURN to the branch named. If a Codex RETURN for the same item turns
up later, the PM banks both.

## Section 335: Dexter, #14 (Shut down / Restart). Head `ec3de81c`.
Run Sections 317 and 324 of `Plans/DISPATCH-2026-11-08-relays-315-321.md` in one pass. 317 is the Enter-only
confirm check; 324 is test commit `3d55c856`. Jarvis's 11-03 review is `Plans/RETURN-2026-11-03-pr14-power-menu-review-jarvis.md`.
RETURN: `Plans/RETURN-2026-11-08-pr14-check-dexter.md` on `dexter/pr14-check`.

## Section 336: Dexter, #70 item 5 and #58 re-checks. Heads `1c036bf2` (#70) and `9a8f014e` (#58).
Run Sections 315 and 316 of `Plans/DISPATCH-2026-11-08-relays-315-321.md`. The earlier FAILs are
`Plans/RETURN-2026-11-03-pr70-setup-s1-review-janus.md` (item 5) and `Plans/RETURN-2026-11-03-pr58-recheck-jarvis.md`
(the cross-window oracle). Write one RETURN per PR.
RETURN: `Plans/RETURN-2026-11-08-pr70-recheck-dexter.md` and `Plans/RETURN-2026-11-08-pr58-recheck-dexter.md` on
`dexter/pr70-pr58-recheck`.

## Section 337: Dexter, #83 (batch-plan stderr). Head `9187ff19`.
Run Section 332 of `Plans/DISPATCH-2026-11-08-relays-329-332.md`.
RETURN: `Plans/RETURN-2026-11-08-pr83-review-dexter.md` on `dexter/pr83-review`.

## Section 338: Dexter, #73 and #82 re-check of item 5 (your relay 326). Heads `4b2628c0` (#73) and `cc04384f`/`c30975a3` (#82).
- [ ] The `-off` leg now compiles the index out entirely: no `neofs-cache-index` cfg is active in that build, and
  `check_index` doesn't run. Pass/fail plus the `kernel/Cargo.toml` and `cache.rs` lines.
- [ ] The two timings now compare the same image and workload, with the index the only difference. Pass/fail.
- [ ] THINK §9's conclusion follows from the new numbers, and the leg is named for what it is. Pass/fail plus line.
- [ ] The PR #73 body no longer recommends the BTreeMap. Pass/fail.
RETURN: `Plans/RETURN-2026-11-08-pr73-recheck2-dexter.md` on `dexter/pr73-recheck2`.

## Section 339: Prometheus, #76 (the file-viewer prior-art survey). Qwen. Head `6be22efa`.
Run Section 321 of `Plans/DISPATCH-2026-11-08-relays-315-321.md`. It's a licence-and-date check, which fits the
research lane. For each project the THINK names, fetch the licence file and the latest commit with `git` (`git ls-remote`, or a
shallow clone), not a summarizing fetch, and give the SHA you checked.
RETURN: `Plans/RETURN-2026-11-08-pr76-survey-review-prometheus.md` on `prometheus/pr76-survey-review`.

## Section 340: Dexter, #71 (setup menu S2) first review. Head `50ca36e2`. Rerouted from Codex relay 328.
Run Section 328 of `Plans/DISPATCH-2026-11-08-relays-325-328.md` as written.
RETURN: `Plans/RETURN-2026-11-08-pr71-review-dexter.md` on `dexter/pr71-review`.

## Section 341: Dexter, #82 (NeoFS cache index build) review. Head `c30975a3`. Rerouted from Codex relay 331.
Run Section 331 of `Plans/DISPATCH-2026-11-08-relays-329-332.md` as written, at the new head (the `-off` leg now
compiles the index out). You wrote the design reviews (319, 326), so also confirm the build matches the THINK at
`4b2628c0`. Note: you are the same family as your own design review, so the cross-family check on the design came from
your read of a Claude-authored THINK; that still holds for this build.
RETURN: `Plans/RETURN-2026-11-08-pr82-review-dexter.md` on `dexter/pr82-review`.

## Section 344: Dexter, #75 (key destructors run at most PTHREAD_DESTRUCTOR_ITERATIONS). Head `efb63b9d`. Rerouted from Codex relay 318.
Run Section 318 of `Plans/DISPATCH-2026-11-08-relays-315-321.md` as written.
RETURN: `Plans/RETURN-2026-11-08-pr75-review-dexter.md` on `dexter/pr75-review`.
