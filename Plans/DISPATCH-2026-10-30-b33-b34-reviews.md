<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Cross-family pre-merge reviews for batches 33 and 34 (run by the PC relay runner)

Each section is one relay. Fetch the PR with `git fetch origin +pull/<N>/head:pr<N>` and review
`git diff origin/main...pr<N>`, plus the review it answers (`git show origin/main:<path>`). Answer as a plain checklist:
for each item, PASS or FAIL, the enforcing `file:line`, and a concrete input for any FAIL.

**Delivery:** commit the RETURN file named in your section on a new branch named after it (for example
`codex/<slug>-<coder>` or `dexter/<slug>`), off `origin/main`. Push the branch and reply with the commit SHA. Never push
to main.

## Section 195: Dexter, security review of #240 (mount authority, head `6f2ef641`)
The task is `Plans/DISPATCH-2026-10-30-mount-authority.md`; the finding it answers is
`Plans/RETURN-2026-10-30-login-recheck-dexter.md` item 3a.
1. Can any process other than the authority holder mount or unmount, on any path to `do_mount`/`do_umount`? Include
   any MOUNT right still obtainable from a plain open.
2. Can the authority be derived, granted, inherited through spawn, or sent over IPC?
3. Is it in image A (feature plus pairing assert), and does the legitimate mount path (boot, installer) still work?
4. Does the scenario's refused unmount leave the volume mounted, and does its control fail?
RETURN: `Plans/RETURN-2026-10-30-mount-authority-review-dexter.md`.

## Section 196: Dexter, security re-check of #243 (login follow-ups, head `7947a189`)
Your #187: 3b (wait_until at `show`), G1 (reset retry), qualifiers A (root `/.system`) and B (more guard probes).
1. Is each one CLOSED? Give the `file:line` for each.
2. Does the root `.system` match avoid over-matching (`.systemX`, `/x/.system`)?
3. Can the reset retry loop forever?
RETURN: `Plans/RETURN-2026-10-30-login-followups-recheck-dexter.md`.

## Section 197: Janus, re-check of #242 (head `daedccf9`)
**MODEL** GPT-6.1-Sol, **EFFORT** medium: a narrow re-check of your #188.
Your `Plans/RETURN-2026-10-30-k1a0-recheck2-janus.md`, items 2, 3 and 5:
- **Item 2:** does on_thread_reap's forced-exit return still hand a possibly-last Arc to the deferral?
- **Item 3:** can ProcessAnchor's Arc release no longer come after its count publication?
- **Item 5:** is there an F1d-specific control, and is F1f's check tightened?
- **Anything new.**
RETURN: `Plans/RETURN-2026-10-30-k1a0-recheck3-janus.md`.

## Section 198: Chad, re-check of #239 (head `5c3e7f98`)
**MODEL** GPT-6.1-Sol, **EFFORT** medium: a narrow re-check of your #183.
Your `Plans/RETURN-2026-10-29-b24-recheck-chad.md`:
- **#229.1a:** is the first allocation charged exactly, with no double charge on growth?
- **#229.3:** do the new install-path legs (minted, excluding, padding, refusal, backlog) each fail without the code
  they cover?
- **#230.3:** is RACE_CLOSE now IRQ-safe?
RETURN: `Plans/RETURN-2026-10-30-b24-recheck2-chad.md`.

## Section 199: Chad, review of #246 (OT-90 P3a clean shutdown, head `821f2e69`)
**MODEL** GPT-6-Luna, **EFFORT** high: one contained subsystem.
1. Before power-off or restart, what is flushed (NeoFS cache and writeback), in what order, and with what bound?
2. Can a straggler process or a stuck device wait block shutdown forever? Is there a forced fallback, and does its
   leg fail without it?
3. Does any path write a disk the write guard forbids?
4. Is `power-clean` in image A, or a feature only? Off = today's behaviour?
RETURN: `Plans/RETURN-2026-10-30-ot90-p3a-review-chad.md`.
