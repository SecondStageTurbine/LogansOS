<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #88): second narrow re-check of the OT-87 installer THINK (revision 3) → Chad

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. A narrow re-check of your relay #87 findings only. If this round still
fails on the same contracts, the next one moves to GPT-6-Astra, per the routing table.

**Type:** review only. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `1c68712b` (PR #149, revision 3 over
`1f313ac6`). §3 (R1) is untouched; R1 is already being built (S2).

## Setup

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot87-installer-think-recheck2-chad
          git fetch origin && git checkout -B codex/ot87-installer-think-recheck2-chad 1c68712b

git show origin/main:Plans/DISPATCH-2026-10-27-ot87-installer-think-recheck2-chad.md   # this file
git show origin/main:Plans/RETURN-2026-10-27-ot87-installer-think-recheck-chad.md      # your relay #87
git diff 1f313ac6 1c68712b -- docs/THINK-2026-10-27-ot87-installer.md
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete input and the wrong result, as in #87.

1. **Durability (#87 items 1 and 4).** Check each of these:
   - every activation step and every W-step is write → flush → read back → boundary check;
   - a device without flush is refused, and a flush error advances no marker;
   - INCOMPLETE is durable before W3, and W3–W5 are flushed before W6;
   - "finish installing" re-runs W3–W6;
   - the oracle models lost and reordered unflushed writes.
   Replay your two power-cut inputs.
2. **Identity (#87 item 2).** Every write goes through a `ClaimedWriter` holding the claimed device. Check that:
   - `media_generation` is checked before submission and after completion;
   - identity and the overlaid first/last-MiB fingerprint are re-checked at every step boundary;
   - the bound on exposure is stated.
   Replay your swap inputs: after the first write, between validation and submission, and a swap the driver doesn't
   report.
3. **The audit (#87 item 5).** The ten harness `write_block` calls and the `virtio_blk.rs` exception are listed, and
   the CI caller check allows them only under their `neo-test` gate.
4. **Reinstall (#87 item 7).** Disks are classed blank, incomplete or installed. Reinstalling over an installed disk
   needs stable identity plus the typed confirmation (R4b), and an unidentified installed disk is refused and stays
   bit-identical.
5. **Anything new** in revision 3 that's wrong.

## Close condition

- Commit `Plans/RETURN-2026-10-27-ot87-installer-think-recheck2-chad.md` on your branch and **push it**, with PASS/FAIL
  per item and an overall verdict. Paste the SHA to the PL.
- No build is needed. Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
