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

## Section 346: Dexter, #77 (WebRender 0.70 on SWGL). Head `c8a06574`. Rerouted from Codex relay 329 (its #77 part).
Run the **#77** items and the **All four** items of Section 329 in `Plans/DISPATCH-2026-11-08-relays-329-332.md`, for
#77 only. Vendored trees: check `VENDORED.md`, the licences and our two patches, not the upstream contents.
RETURN: `Plans/RETURN-2026-11-08-pr77-review-dexter.md` on `dexter/pr77-review`.

## Section 347: Dexter, #86 (neo-crt answers CLOCK_REALTIME, counted from 1970). Head `0b650a4d`. Small; contained.
The PM picked option (i) of THINK #10 §6.1 (2026-11-08).
- [ ] `CLOCK_REALTIME` returns the process-relative count read from 1970: it advances, and it never claims a real
  date. Pass/fail plus `userland/neo-crt/src/lib.rs` line.
- [ ] `CLOCK_MONOTONIC` is unchanged, including `main`'s `monotonic_epoch()` structure that the conflict touched.
  Pass/fail plus line.
- [ ] `crt_checks.c` would fail on the old code (realtime aborting or not advancing). Pass/fail plus line.
- [ ] std's `SystemTime::now()` is untouched. Pass/fail.
RETURN: `Plans/RETURN-2026-11-08-pr86-review-dexter.md` on `dexter/pr86-review`.

## Section 348: Dexter, #78 (SWGL rasters into a kernel Atrium surface). Head `7fd9eaf5`, stacked on #77. Rerouted from Codex relay 329 (its #78 part).
Run the **#78** items and the **All four** items of Section 329 in `Plans/DISPATCH-2026-11-08-relays-329-332.md`, for
#78 only: `git diff refs/pr/77...refs/pr/78`.
RETURN: `Plans/RETURN-2026-11-08-pr78-review-dexter.md` on `dexter/pr78-review`.

## Section 349: Dexter, #10 (THINK: SpiderMonkey on neo-crt, interpreter-only). Head `c2d5f3cc`. A design read.
The PM decided §6.1 option (i) and §3.3 option (c) (comment on #10, 2026-11-08).
- [ ] §2.2's licence table holds at the pin `servo/mozjs` @ `138c9470`: every linked part is on the allowed list, and
  nothing GPL/LGPL is linked. Check each against the files in a clone. Pass/fail per row.
- [ ] §3.3 option (c) is written so the product build fetches nothing, and the CI generator job fails if its output
  differs from the committed files. Pass/fail plus line.
- [ ] §6.1 option (i) is stated as decided, and C1.3's std behaviour is unchanged. Pass/fail plus line.
- [ ] The trap list (T1 …) each has a discriminator, and the CP table covers the threads the interpreter can start
  (single-threaded must be enforced, not assumed). Pass/fail plus line.
- [ ] The next steps name a gate for each rung. Pass/fail.
RETURN: `Plans/RETURN-2026-11-08-pr10-design-dexter.md` on `dexter/pr10-design`.

## Section 350: Dexter, #79 (the client grants a separate capability). Head `67525f8d`, stacked on #78. Rerouted from Codex relay 329 (its #79 part).
Run the **#79** items and the **All four** items of Section 329 in `Plans/DISPATCH-2026-11-08-relays-329-332.md`, for
#79 only: `git diff refs/pr/78...refs/pr/79`.
RETURN: `Plans/RETURN-2026-11-08-pr79-review-dexter.md` on `dexter/pr79-review`.

## Section 351: Dexter, #88 (neo-crt for SpiderMonkey: libc++ localization, the POSIX gap, stack protector). Head `1d77e6cb`.
Design: C3a THINK §10 (`docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md`). 41 of its 54 files are under `third_party/ot61-crt`.
- [ ] Every newly vendored file under `third_party/ot61-crt` has its licence recorded, and each licence is on
  CLAUDE.md's allowed list. libc++'s `Apache-2.0 WITH LLVM-exception` is fine. Pass/fail per source.
- [ ] Each of the 47 POSIX symbols the link probe listed is either implemented in `neo-crt` or a documented stub that
  fails safely (an error return, never silent success). Name any stub that returns success without doing the work.
- [ ] The stack protector: `__stack_chk_guard` is seeded from the kernel's RNG before any protected function runs,
  and `__stack_chk_fail` stops the process. Pass/fail plus line.
- [ ] Localization stays in the "C" locale: no locale files are read, and nothing fetches. Pass/fail plus line.
- [ ] The smoke checks would fail on the old code for each new piece. Pass/fail plus line.
- [ ] Off by default; other `neo-crt` users build unchanged. Pass/fail.
RETURN: `Plans/RETURN-2026-11-08-pr88-review-dexter.md` on `dexter/pr88-review`.

## Section 352: Dexter, #80 (the browser app's I/O surface). Head `80eb3087`, stacked on #79. Rerouted from Codex relay 329 (its #80 part).
Run the **#80** items and the **All four** items of Section 329 in `Plans/DISPATCH-2026-11-08-relays-329-332.md`, for
#80 only: `git diff refs/pr/79...refs/pr/80`.
RETURN: `Plans/RETURN-2026-11-08-pr80-review-dexter.md` on `dexter/pr80-review`.

## Section 353: Dexter, #89 (THINK: the C3c-1 product build, SpiderMonkey with cc only). Head `103d9122`. Docs only.
- [ ] The product build runs `cc` only, over the frozen sources and `generated/`: no Python, configure, make, bindgen
  or network. Pass/fail plus the THINK line.
- [ ] The drift check covers the bindgen output and the patched files, as well as configure's output. Pass/fail.
- [ ] Every crate in the Rust closure is named with its licence, and each licence is allowed. `dtoa.c` is pending
  J.c.'s licence decision; note it, don't fail on it. Pass/fail per crate.
- [ ] Each step names its gate. Pass/fail.
RETURN: `Plans/RETURN-2026-11-08-pr89-design-dexter.md` on `dexter/pr89-design`.

## Section 354: Dexter, #87 (vendor SpiderMonkey 153.6.0, the network-off generator, the frozen outputs). Head `fd1ec760`.
About 10,000 vendored files: check the records and our own pieces, not upstream's contents. Our pieces are
`.github/workflows/ot61-mozjs-generate.yml`, the generator script, the patches and the vendor cut-list.
- [ ] The vendor tree matches `servo/mozjs` @ `138c9470` except for the recorded cut-list and patches: give the hash
  or diff you checked. The GPL-only files are cut (`third_party/python/yamllint`, `js/src/util/make_unicode.py`,
  `js/src/devtools/rootAnalysis/run_complete`). Pass/fail.
- [ ] The generator job runs with the network off, and fails when its output differs from the committed frozen files.
  Pass/fail plus the workflow line.
- [ ] The workflow takes no secrets, writes nothing to `main`, and runs on a pinned toolchain. Pass/fail plus line.
- [ ] Licence records cover every linked part; `dtoa.c` is flagged as pending J.c. Pass/fail.
- [ ] Nothing in the default product image changes. Pass/fail.
RETURN: `Plans/RETURN-2026-11-08-pr87-review-dexter.md` on `dexter/pr87-review`.
