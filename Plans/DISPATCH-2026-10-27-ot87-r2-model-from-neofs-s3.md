<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-87 R2, Arthur's model from the installed NeoFS volume → S3

**Why.** The OT-87 THINK merged (`523e8966`, #149) after five cross-family reads. R2 is the next rung that depends on
nothing in flight. It lets an installed system load Arthur's model from its own disk instead of the USB's boot
modules. That's what makes an installed LogansOS stand on its own (J.c., 2026-10-27: "installed on an actual computer
or VM").

**Spec:** `docs/THINK-2026-10-27-ot87-installer.md` §5 and the R2 row in §8
(`git fetch origin && git show origin/main:docs/THINK-2026-10-27-ot87-installer.md`).

## Scope

1. **A kernel pin check for NeoFS models.** A `/neofs/` path whose name matches a pinned model or vocab is compared
   with the pin using the inode's verified BLAKE3 content hash (the same quantity, so there's no second 485 MB pass).
   A mismatch gives `EBADMSG` (Arthur's "Damaged"). Unpinned names behave as today.
2. **A preload from NeoFS** under `boot-model-preload`: after the boot modules, try `/neofs/<pinned name>`, cache it
   under the same handle path, and build at IF=1 on the inference stack (CP-74's fix carries over).
3. **Arthur's source order:** boot modules first, then `/neofs`. Add the source tag to the model-state line. On an
   installed system, the Absent line says the model is missing from the disk, not "boot the USB".
4. **Kill-switch `model-from-neofs`.** Off gives `.text` identical to today for `desktop`, `product` and image A. Prove
   it with SHA-256s.

## Battery (4-shard gate)

Use R1's VM path once #152 merges (the batch-3 gate), with `data.raw` carrying the model, or the existing NeoFS
scenario machinery:
- the model on NeoFS loads and passes its pin, from a boot disk **without** model modules;
- a wrong-byte model on NeoFS gives Damaged;
- with the boot module present, it wins over NeoFS;
- the kill-switch-off control.

## Rules

- **Concurrency:** the preload runs IF-on at boot. Consult `docs/concurrency-pitfalls.md` (CP-56, CP-74 and others)
  and put the table in the PR.
- **Prior art (CLAUDE.md's new rule):** a short note on how other systems ship a large model or asset on disk with a
  pin. If nothing applies, say "nothing directly comparable" and why.
- Close-gate (`scripts/worker-close-gate.sh`); kill-switch ISC; SPDX headers; no `[skip ci]`.
- You have a standing yes for branch `claude/ot87-r2-model-from-neofs` and a PR. Branch off main **after #152 merges**
  if you need R1's VM path, or now if you use the existing scenarios.
- Cross-family review after the gate: the kernel AI path, GPT-6-Luna high.

## Close

Send the PM the PR number and head SHA.
