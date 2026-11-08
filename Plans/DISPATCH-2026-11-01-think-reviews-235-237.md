<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relays 235-237: cross-family design reviews of four THINKs (the always-on relay box)

Same rules as `Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md` (public repo: no personal details; RAG not
available; answer as a plain checklist, PASS or FAIL with the THINK's section or `file:line` and a concrete case for
any FAIL; name the head SHA you read; RETURN on a new branch off `origin/main`, push, reply with the commit SHA; never
push to `main`). These are design reads: check that the plan is sound and complete before code is written. Name what
the plan is missing, not only what it gets wrong.

## Section 235: Dexter, #31 (THINK: OT-58 tiered gate on the free tier)

**MODEL** DeepSeek, high: a design cross-check from a different model family.

`git fetch origin +pull/31/head:pr31 && git diff origin/main...pr31` (head `730b0da0`).

1. The quick tier: is every scenario chosen for the smoke shard justified, and does it cover the failure classes
   that have escaped before (the THINK should cite them)? Name any class a quick-green PR could still carry to `main`.
2. The batch full tier: is a red batch bisected to one PR mechanically, and does `main` stay one action from green
   (the revert-first contract)?
3. The concurrency-core path filter: list kernel paths that touch the SMP core but would miss the filter.
4. The free-tier arithmetic (20 concurrent jobs): does the plan actually keep four seats' quick gates from queuing?
5. Anything that weakens a standing rule in CLAUDE.md (the gate, the AAG, cross-family review) without saying so.

RETURN: `Plans/RETURN-2026-11-01-pr31-tiered-gate-review-dexter.md`, branch `dexter/pr31-review`.

## Section 236: Jarvis, #12 (THINK L1a: the terminal window on the WM)

**MODEL** GPT-6-Luna, **EFFORT** high: one subsystem's design, with an input-security question.

`git fetch origin +pull/12/head:pr12 && git diff origin/main...pr12` (head `d32637b3`).

1. The client protocol between the terminal and its child: framing, flow control, and what happens when either
   side stops reading.
2. The escape-sequence subset: is it enough for kibi and a line-editing shell, and does it have a byte oracle?
   Sequences that must be ignored safely (title set, clipboard, device queries) are named.
3. Input security: no keystroke reaches the terminal's child while the lock screen is up or the window isn't
   focused (the CP-97 lesson); a paste can't inject control sequences unmarked.
4. The kill-switch: the WM is unchanged without the terminal.
5. The prior-art survey (licences and last-commit dates) is present and holds.

RETURN: `Plans/RETURN-2026-11-01-pr12-terminal-think-review-jarvis.md`, branch `codex/pr12-review-jarvis`.

## Section 237: Chad, #9 and #20 (THINKs L3a FAT read-only, L3u USB mass storage)

**MODEL** GPT-6-Luna, **EFFORT** high: two contained subsystem designs.

`git fetch origin +pull/9/head:pr9 +pull/20/head:pr20` (heads `200c4efb`, `65209158`).

1. FAT: the trap-mining bank (long names, FAT12/16/32 detection by cluster count, not the label; dirty bit; bad
   BPB values; cluster-chain loops) each has a discriminator.
2. FAT: read-only is enforced by construction (no write path compiled), and MBR parsing has a hand-laid byte oracle.
3. USB mass storage: the device-quirk bank (capacity off-by-one, READ(10) size limits, devices that stall on unknown
   commands, slow spin-up) each has a discriminator; the DMA path states its I-14 containment.
4. Both: the kill-switch, and the Zenbook rule (no write can reach the internal disk).

RETURN: `Plans/RETURN-2026-11-01-pr9-pr20-storage-thinks-review-chad.md`, branch `codex/pr9-pr20-review-chad`.
