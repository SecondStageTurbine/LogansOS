<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE (relays #121–#124) → Chad: re-checks of your own FAILs, plus one test fix

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. This is a bundle, so one model throughout (CLAUDE.md bundle rule).

All four merged after batch 12b (run `40787440872`, green). Review each squash on `main`:
`git show <sha> --stat` and `git show <sha>`. One pushed RETURN per relay, each a plain checklist (pass or fail, the
enforcing `file:line`, a concrete input for any fail):
`Plans/RETURN-2026-10-29-<slug>-recheck-chad.md`.

## #121: #173 `36fe4784`, the A9-C2 wheel fix (your #104 F1/F2). Slug `a9-wheel`

1. **F1.** A kind-5 forwarded wheel on a full key-filled queue is dropped (`DroppedIncomingMotion`), and key 4 stays
   at the head. Your exact KAT is in the tree.
2. **F2.** `RecordRing::mint_report` mints the wheel record first and delivers each record before minting the next,
   so a loss lands RESYNC on the motion/press record.
   - **Check S1's KAT adaptation.** S1 says your hand-rolled mint order hard-coded the old order and couldn't pass
     whatever the kernel did. The press is now tag 66, the wheel record 65 goes first, and `pointer_grab::decide` is
     asserted (`cancel_grab=true`).
   - Does the adapted KAT still test your scenario: a stale grab plus a lost release, then a new press?
   - Does it fail with the motion record minted first?
3. **The kill-switch.** `atrium-pointer-wheel` off gives the same `.text`.

## #122: #175 `ee223760`, the A9-C scrollback fix (your #100 item 1). Slug `a9-scrollback`

1. `write_byte` moves a held `view_top` up to `first_kept` as lines are evicted. Your sequence (512 lines, PgUp to 0,
   +10 held, PgDn) gives 21, and the KAT is in the tree.
2. No other path leaves `view_top < first_kept`: scroll, follow, resize, `settle`.

## #123: #176 `2fe2b399`, OT-87 review fixes C (your #107, #110 and #95). Slug `ot87-fixes-c`

1. **R4.2.** `checked_add` for `dirent_offset + 32`, plus the three other spots S3 found (the overlap test, the
   slot-map length, `check_part`'s range). Do the KATs, including the `0xffffffe0` entry, reject them?
2. **NFS-W1.**
   - Every error after the reservation gives the run back (`release_run`), and a failed commit sets `commit_failed`,
     after which `publish_superblock` refuses.
   - The writeback pause is an RAII guard.
   - The QEMU leg reuses the run after an injected error: `next_free_block` 18711 against the run 18708..18719.
   - Are any error paths left?
3. **R2.** A separate `vocab_source`, and `reported_source` names the vocab only when it failed. Does the mixed-case
   KAT check the label?

## #124: #178 `7bd9a9a0`, the `virtio_accel_identity_roundtrip` test race fix (the PM's). Slug `virtio-accel-test`

A light, test-only check.
1. Does output written by the IRQ between `poll() → Pending` and the test's read still pass, and does output with no
   IRQ completion counted still fail?
2. Is the 1,000,000-iteration spin bound sound? `poll_used` copies at `virtio_accel.rs:767` and counts at `:781`.
3. Is any discriminator weakened?

```
WORKTREE: E:\AI Automation\workers\chad   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/review-bundle-2026-10-29-chad
          git fetch origin && git checkout -B codex/review-bundle-2026-10-29-chad origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-review-bundle-chad.md
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push all four RETURNs. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
