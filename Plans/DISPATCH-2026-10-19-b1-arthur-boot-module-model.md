<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: B1 — "boot the USB, Arthur answers": the 0.5B as a boot module, reclaimed after load → S1

**Seat:** S1 (Claude session). **Kernel-touching:** yes, so the 4-shard QEMU gate is mandatory, and the AAG needs a
different-family lens on the frame-reclaim code. **Ends on:** the OptiPlex, a joint PL and coder bare-metal run.
**Ratified by:** `Plans/DECISION-2026-10-19-arthur-first-boot-model.md`.
**Design:** `docs/THINK-2026-10-19-arthur-first-boot-model.md`, the "smallest first build" section and Q1–Q4. Read
it in full first. This dispatch only adds the close conditions.

```
BRANCH: claude/b1-arthur-boot-module   (git fetch origin && git checkout -B claude/b1-arthur-boot-module origin/main)
```

## Scope (from the THINK)

0. **Prerequisites.**
   - The reproducible-import check: the `.neom` built from the raw upstream GGUF is byte-identical across runs, and
     its digest is pinned from that build.
   - The OT-73 hash-in-load-pass: hash during the single load pass and fail closed.
   - Never take an exact byte value or digest through a summarizing fetch. Pull the raw upstream file.
1. `xtask iso --with-model <neom>` stages the `.neom`, the `.vocab` and the licence files as Limine modules.
2. The kernel builds the model from the module bytes through the existing arena path, then reclaims the module
   frames. Reclaim only the expected memmap type, and fail closed on anything else.
3. Arthur's registry gains a boot-module source for the 0.5B, with its digest pinned.
4. The Q4 "absent" and "damaged" messages, plus serial witness lines.
5. The ADR-063 amendment.
6. A model-weights entry in the Bible licence article (`bible.md:387`) and in `NOTICE`.

## Required sections (standing rules)

- **Kill-switch.** The `model-boot-module` feature. Off, the image and the kernel are bit-for-bit today's. A
  regression scenario proves it.
- **Discriminator.** `arthur_boot_module_answers`: no NeoFS disk attached, the module loads, the digest matches, and a
  real reply appears.
  - Controls: a flipped byte must give "damaged" and no reply; a missing module must give "absent".
  - A frame-count witness must prove the reclaim.
- **Byte oracle.** The pinned digest comes from the raw upstream source, plus a hand-checked fixture for any new
  module-header parsing.
- **Pitfall bank.** Frame reclaim and the allocator touch memory and lifetime. Consult `docs/concurrency-pitfalls.md`
  and state which entries apply. CP-60 is the closest.
- **Trap-mining before the bare-metal run.**
  - Limine's module limits and its `#hash` suffix, from the raw `CONFIG.md`.
  - Dell OptiPlex UEFI quirks with large USB reads.
  - xorriso and Limine handling of large files.
- **Failure-mode bank.** F1–F6 from the THINK, with each discriminator captured before it is needed.
- **Runbook.** `docs/runbook-baremetal-arthur-optiplex.md` (read it with
  `git fetch origin && git show origin/main:docs/runbook-baremetal-arthur-optiplex.md`). For this run, detach the SATA
  model disk.

## Close

- Run `scripts/worker-close-gate.sh` and open the PR non-draft.
- Post "PM: ready for gate" with the head SHA. The PM dispatches the 4-shard gate.
- Include a Codex cross-check dispatch (Jarvis or Chad) covering the reclaim and lifetime code.
- **Merge bar:** gate plus AAG. **Acceptance bar:** the OptiPlex run, a milestone and not a merge gate.
- Commit and push at every checkpoint.
