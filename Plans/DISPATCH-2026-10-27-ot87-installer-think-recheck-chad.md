<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #87): narrow re-check of your relay #86 findings on the OT-87 installer THINK → Chad

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. A narrow re-check of your own findings (relay #86: items 2, 3, 5, 6, 7 and
8). S3 revised the THINK in one commit.

**Type:** review only. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `1f313ac6` (PR #149, branch
`claude/ot87-installer-think`; revision 2 over `f1cec9c2`).

J.c.'s decisions since relay #86 are folded into the THINK, so check them too:
- Install offers **"Erase disk and install"**, with a typed confirmation, an identity-bound one-shot claim, and a Muse
  review;
- **Try opens an installed LogansOS disk read-only.**

## Setup

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot87-installer-think-recheck-chad
          git fetch origin && git checkout -B codex/ot87-installer-think-recheck-chad 1f313ac6

git show origin/main:Plans/DISPATCH-2026-10-27-ot87-installer-think-recheck-chad.md   # this file
git show origin/main:Plans/RETURN-2026-10-27-ot87-installer-think-review-chad.md      # your relay #86
git diff f1cec9c2 1f313ac6 -- docs/THINK-2026-10-27-ot87-installer.md
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, say what's still wrong.

1. **Item 7, A/B.** Deployments stay in NeoFS per OT-51, and the ESP holds per-slot boot copies written by a slot
   writer (§7.1). Activation commits by flipping one sector. Check three things:
   - rollback and interrupted activation are covered;
   - the slot writer touches only preallocated contiguous slot files, the dirent FileSize and the commit sector;
   - the ESP budget (§7.2) matches the image A sizes.
2. **Items 2 and 5, identity.** A registry generation plus driver identity plus a first- and last-MiB fingerprint is
   revalidated before every write, and fails closed. Erase is refused when there's no stable identity. Do the tests
   cover reorder, removal/reinsert, unavailable identity, same-tuple replacement, the boot disk and protected disks?
   Does anything still let a write reach a disk other than the confirmed one?
3. **Item 5, GPT.** A fresh GPT is computed for the target's own sector size (§4.4). Every header and entry field is
   listed, with CRC-E, CRC-H1 and CRC-H2 named separately. The byte oracle per geometry is independent of the writer.
4. **Item 8, interruption and the rest.**
   - The W0–W6 state machine with the INCOMPLETE attribute (§4.7) leaves every interrupted disk recognisable and
     finishable;
   - the payload is a NEOINST1 Limine module with a compiled-in pin (§4.3), and pulling the USB mid-install is
     harmless;
   - disk selection works by keyboard and touchpad (§4.5);
   - a 2 GiB minimum is checked before any write (§4.6).
5. **Item 3, Try's write surface.** §6.2's audit is complete: every `write_block` caller is listed and each is safe
   in Try. Spot-check it against the code.
6. **J.c.'s decisions.** Check both:
   - erase is behind the typed confirmation and the claim, and the Windows-GPT default stays opaque for everything else;
   - which layer stops the Zenbook's internal NVMe if VMD is ever driven.
7. **Anything new** the revision introduced that's wrong.

## Close condition

- Commit `Plans/RETURN-2026-10-27-ot87-installer-think-recheck-chad.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- No build is needed. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
