<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# AUDIT: every doc, note and decision, 2026-11-18

**Asked by:** J.c., during the pause: "go through every document, every note, every decision we have made and
still need to be made, make sure we're not missing anything, then clean it up."
**Done by:** the PM, with seven read-only readers on separate slices. The PM checked each finding it acted on
against `main`'s code or history before changing anything.
**Read at:** `origin/main` `3e84fe20`. **Cleanup commits:** `e46b61e2`, `c3fcd11e`, `5f257998`, `78db5a1f`,
`01728d2f`, and the one that adds this file.

## 1. What was checked

- **Scope:** `docs/` (329 files) and `Plans/` (2,031 files), about 344,000 lines, plus the open PRs, the relay
  queue and the code where a doc made a claim about it.
- **The slices:**
  - the open-threads tracker, every thread against the code;
  - every recorded or pending decision since 2026-10-20;
  - the RETURN and DISPATCH follow-ups for Oct 30 – Nov 7 and for Nov 8 – Nov 18;
  - the summary docs (README, CLAUDE.md, AGENTS.md, INDEX, FEEDBACK, the release ledger, the bible, the invariant
    ledger, the pitfall bank);
  - the draft PRs and the early relays;
  - carried debt in the pre-public documents (before 2026-10-20).

## 2. The overall picture

- **No code landed on a failed review.** Every PR that failed a review was re-checked to PASS before it landed, or
  landed under a recorded ruling by J.c. or the PM. That covers 16 PRs in the Nov 8–18 slice and about 22 in
  Oct 30 – Nov 7.
- **The gap was bookkeeping.** After the move to the public repo, the PM kept the relay queue and the PR comments
  current, but stopped updating:
  - the open-threads register (no commits between Oct 31 and Nov 10);
  - the release ledger (stopped Nov 1);
  - the index (stopped in July);
  - the README's status (stopped Nov 8).

  Decisions made in PR comments were often never written back into the design docs they changed.

## 3. What was fixed

| Area | What changed |
|---|---|
| `docs/OPEN-THREADS.md` | 12 statuses corrected; 17 finished rows archived with their evidence. **OT-59 still read OPEN though it closed in July.** 22 rows added: OT-87, OT-88 and OT-90 (MVP threads with no row) and OT-103 to OT-121 (untracked debt). A blank line that cut the table in half was removed. |
| 15 design docs | A dated status note under each stale "design only" line. Decisions written back: OT-88 D1 superseded by the one-window decision; C3c-2 D1(a)/D2(b); K1b landed; OT-100's F4 ruling; setup Q8 (§5). |
| Trackers | FEEDBACK's table repaired. The relay queue notes every relay that never returned. The invariant ledger says 18 rows, not 16, and has its missing I-17 entry. The pitfall bank gained index rows for CP-105–107 and notes on its gaps. |
| Stale planning docs | Header notes on INDEX (unmaintained since July), the 08-07 queue (superseded), the 11-11 audit and the 10-27 decision (snapshots). |
| Release ledger | A catch-up for Nov 1–18, by PR. |
| The pause | `Plans/PAUSE-2026-11-13.md`: the resume order, by PR and head. The pause had no note. |
| README | Current to Nov 18: what works, the MVP path, OT-100, the browser, the userspace list. |
| Bible | 259 garbled character sequences repaired; the Phase 6 and 7.5 statuses corrected. |
| Draft PRs | #11, #21, #24–#28 and #55 closed. Each had been replaced by a merged PR (#65, #86, #67, #77–#81) and carried a "replaced by" comment. |
| OT-90 THINK | Two wrong thread numbers fixed (login is OT-66; OT-89 was never assigned). |

## 4. Decisions J.c. needs to make

Most urgent first. The first one blocks the MVP.

1. **Editor D6 and D7** (`docs/THINK-2026-10-29-ot67-editor.md` §8). **Blocks E2, the last rung of the MVP
   path.**
   - D6: a file opened without WRITE gets a read-only view, or is refused?
   - D7: is an edit that would push the file past 4 MiB refused whole?
2. **What goes into the shipping image** (OT-120):
   - `net-dial-cap` and `interface-caps`, lane L1.2: dispatched, never built;
   - Arthur's action log (`arthur-actions` / `arthur-action-log`): built, not in the image, and nowhere does it say
     whether that's on purpose.
3. **The Gen9 GPU silicon proof** (OT-110). The M710q isn't available, and the Zenbook isn't Gen9. Re-target it to
   the OptiPlex UHD 630, or retire it?
4. **Arthur write mode** (`docs/THINK-2026-10-30-playbook-gap-map.md` decision 4). Arthur ships read-only today;
   that default is safe.
5. **The 1.0 lanes that wait on you** (OT-121):
   - the update signing root and slot model (L3);
   - the compatibility strategy (L6: a Linux personality, or a hypervisor room);
   - sleep/wake D1–D3 (`docs/THINK-2026-10-30-l7.3-s0ix-sleep-wake.md`).
6. **The browser's trust store** (OT-111): Mozilla `certdata.txt` or `webpki-roots`, and what to do about
   `rustls-rustcrypto` being unmaintained. Blocks the browser's real-site battery, which is 1.0.
7. **aarch64** (OT-112): keep the bible's promise, or re-word it.
8. **OT-82**: close it, since image A answers it now, or keep the original question open.
9. **Long-waiting:** OT-1 (licence questions Q1–Q3) and OT-48 (patents). Both have waited since June.
10. **CLAUDE.md**, whose text the PM won't change without your OK:
    - The seat counts disagree: line 7 says "eight CLI coders", line 31 says six, and Sage is listed as both
      folded into Dexter and a separate seat.
    - It says "four" model families, but Muse makes five.
    - The cloud Seats A–I that do most of the work aren't described.
    - The pitfall-bank count says 83; it is 112.
    - Write-guard citations "#85/#88" are private-repo numbers, which now point at unrelated public PRs.
    - The RAG section still says "NeoPlutarch".
    - AGENTS.md, which the Codex seats read, carries none of the process rules.

    Say the word and the PM will make these edits.
11. **Branches to delete.** The PM's delete was refused before:
    - `claude/zenbook-timing-release`;
    - `claude/l1a3-terminal`, `claude/l1a3-terminal-next`, `claude/l1a3-terminal-tickdiag-control`.

## 5. Decisions the PM made in this audit

- **Setup menu Q8:** a time-zone change after the wizard's confirm takes effect at the next boot. This keeps the
  time-zone lane's write-once contract. The delta re-confirm can come with Home's `timezone` verb, with its own
  concurrency KAT.
- **Time zones:** the history limitation stands as accepted, since the lane was built to it.
- **ACPI/AML Fix 2 limits and the Fix 3 String-target store:** accepted as built. They were reviewed and merged to
  the recommendation.

**For the PM at resume, not decided here:**
- **#102 (OT-tick-1).** The recommendation is to rebase it and land it as a test-only discriminator, with the bound
  stated as N runnable threads × the tick. Armed discriminators stay.
- **NeoFS cache index Q2:** run some installer scenarios with `neofs-cache-evict`. Fold it into the consolidation
  (OT-119).
- **The kibi (L1b) and coreutils (L1d) survey decisions:** seven and six of them. They're off the MVP path, and
  they don't need J.c. until those lanes are picked.
- **The file-viewer decisions 1–3:** off the MVP path.

## 6. What was not changed, and why

- **Code:** small stale strings and comments (OT-119 e) and the unpinned `rustup` bootstraps (OT-119 f). A code
  change needs its own gate, so these go into the consolidation build.
- **The design docs' contents beyond status notes:** for example the setup THINK's "keyboard=1" wording (#71 note 3)
  and the SWGL THINK's crate count. These are small and listed in OT-119.
- **CLAUDE.md and AGENTS.md:** see §4 item 10.
- **INDEX.md:** marked unmaintained, not rebuilt. Rebuilding it for 1,300 files would cost more than it returns;
  OPEN-THREADS, the ledger catch-up and this file are the maps now.
- **Private-repo PR numbers** in older docs: marked where the README and the invariant ledger cite them. Others
  remain in history docs, where they're accurate for their time.

## 7. Process lessons

1. **Keep the register in the same commit as the work.** The register, the ledger and the README went stale when
   the work moved to PR comments. From resume, every landing note also updates the thread's OPEN-THREADS row.
   Every decision made in a PR comment is written back into the doc it changes, in the same loop.
2. **The Astra escalation rule wasn't applied in October.** Several PRs (#16, #13, #15, #60, #6) went three or four
   rounds on GPT-6.1-Sol. The rule (CLAUDE.md) moves a lane to GPT-6-Astra after two. It was first applied on
   Nov 12.
3. **PM rulings that closed a reviewer's FAIL without the reviewer's re-check:**
   - #23, the trap table;
   - #53, docs only;
   - #111, A2's two dispatch-overreach FAILs.

   Each is recorded. For #111, a one-line confirmation from Jarvis before it lands is cheap and worth having.
4. **Work landed with a gate and no review RETURN:**
   - #19, #32, #34, #44, #48, #49, #50, #63 and #37 (host-only or tooling);
   - #112 (the C3c-2 THINK), merged before its design read, which never returned (relay 409).

   None is kernel-touching, so this is within the rules. It should be stated when it happens.
5. **Relays that come back empty** need a re-run, as relay 387 got and 409 didn't. The relay queue now lists every
   one that never returned.
