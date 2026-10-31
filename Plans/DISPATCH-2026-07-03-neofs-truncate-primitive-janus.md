<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: NeoFS `truncate` primitive — unblock OT-17 M3 + generalize A2/A3 reversibility → Janus

**Date:** 2026-07-03
**From:** PM (Claude)
**To:** Janus (main-tier — built A1/A2/A3). New branch off `origin/main` HEAD **once the OT-8/OT-9/A3 batch lands (`08fcb95`)** — confirm `origin/main == 08fcb95` before branching: `codex/neofs-truncate-janus`.
**Status:** DISPATCHED — a **high-leverage `fs`/NeoFS primitive** that unblocks two stuck things: **OT-17 M3** (Arthur's memory cap-delegation is flagged "needs a NeoFS truncate primitive") **and** it **generalizes A2/A3 reversibility** — today a reversible write only works when the new content is the *exact same length* as the old, purely because there's no truncate (the A2 equal-length constraint). **Deliberately routed to a NON-agentic surface** so Oracle owns the Arthur-agentic layer cleanly for the yolo-posture build. **Non-conflicting** with WiFi, `mm/slab` (OT-8), observability (OT-9), and the agentic posture. **Kernel-touching ⇒ 4-shard QEMU + AAG.**

## 0. What this build is
A `truncate(path/handle, new_len)` operation in NeoFS: shrink a file to `new_len` (freeing the released blocks back to the allocator) and grow-with-zero-fill where the ABI calls for it. It's the missing fs primitive that (1) lets OT-17 M3 delegate/rewrite memory objects cleanly, and (2) lets A2/A3 undo restore a file to *any* prior length, not just an equal-length overwrite. You built A2/A3, so you know exactly the reversibility limitation this removes.

## 1. References
```
git fetch origin
git show origin/main:kernel/src/fs/neofs/inode.rs       # NeoInode read/write/block-map — where truncate frees/zeroes blocks; byte_len already lives here
git show origin/main:kernel/src/fs/mod.rs               # the Inode trait — add truncate to the VFS surface
git show origin/main:docs/ADR-064-neofs-large-files.md  # the indirect-block map truncate must walk/free (direct/single/double/triple)
git show origin/main:docs/OPEN-THREADS.md               # OT-17 M3 ("needs a NeoFS truncate primitive, flagged") + the A2/A3 equal-length reversibility note
# the A2/A3 undo path (userland/neo-arthur) — the eventual consumer that generalizes once truncate exists
```

## 2. ISCs
- **ISC-1 — The VFS `truncate` on the `Inode` trait + NeoInode impl.** Add `truncate(&self, new_len) -> Result<…>` to the fs trait (default = `Unsupported`, like `byte_len`), and implement it on `NeoInode`: update `file_size`, and **free the released blocks** back to the allocator walking the direct/single/double/triple indirect map (ADR-064) — no leaked blocks, no double-free. Grow-with-zero-fill if the ABI includes grow.
- **ISC-2 — Block-map correctness (the byte-oracle-grade part).** A hand-laid fixture: a file spanning into double/triple-indirect, truncated to various lengths (0, mid-direct, mid-single-indirect, exact block boundary, +1), asserting the surviving bytes are exact, the freed blocks return to the free count, and re-reading past the new end fails/zeroes per the ABI. Independent of the writer (the CLAUDE.md byte-oracle discipline for a format/layout change).
- **ISC-3 — The syscall/ABI surface** (if truncate is exposed to userland — `sys_ftruncate`-class): capability-gated (a WRITE cap required; I-15), bounds-checked. If M3/A2 consume it in-kernel only for now, state that and defer the syscall.
- **ISC-4 — Kill-switch / equality.** truncate behind a default-off feature (or additive-so-inert): **absent/off ⇒ the fs is bit-for-bit today** (no existing path calls truncate until a consumer opts in). No behavior change to existing read/write/`byte_len`.

## 3. Standing-rule sections
- **Kill-switch (Rule 1):** additive primitive — inert until a consumer calls it; prove the existing fs paths are unchanged.
- **Invariant budget (Rule 2):** **no new invariant** (an fs operation, not a cross-cutting guarantee). I-13 (stale-handle/generation) and the NeoFS on-disk structural facts must hold — a truncate must not orphan or double-free a block or corrupt the indirect map.
- **Byte-oracle:** ISC-2 is the required hand-laid fixture (a block-layout change ⇒ an independent byte oracle, per CLAUDE.md).
- **I-14:** N/A by construction (fs metadata + block-free; no driver ISR/DMA; the NeoFS lock is a leaf).
- **Scope guard:** the truncate primitive + its oracle + (optional) syscall ONLY. **Do NOT:** touch the Arthur-agentic surface (Oracle's yolo build) — this build *enables* the reversibility generalization but does NOT rewire A2/A3's undo (that's a small follow-on once truncate lands); touch WiFi/`mm-slab`/observability/forward; or change existing read/write semantics.

## 4. Gate + close
- **Kernel-touching ⇒ mandatory 4-shard QEMU `NEO_SMP=4` + AAG.** A scenario exercising truncate across the indirect-block boundaries + the free-count assertion + the cap-gate (if syscall). Push-CI green (host byte-oracle fixture). **Green without the CI run URLs is not done.**
- **Close-gate `scripts/worker-close-gate.sh`** (rebase; re-run; host set; SPDX; self-stat).
- **RETURN** with the QEMU shard URLs + SHA, the byte-oracle fixture pass (the truncate-across-indirect cases + free-count), the kill-switch/inert-equality, and a note that OT-17 M3 + the A2/A3 equal-length-reversibility generalization are now unblocked (follow-ons). **Disk reclaim** after merge.

---
## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-03-neofs-truncate-primitive-janus.md` (+ `fs/neofs/inode.rs`, `fs/mod.rs`, ADR-064, the OT-17 M3 + A2/A3-reversibility notes). **Branch:** `codex/neofs-truncate-janus` off `origin/main` **after the batch lands (`08fcb95`)**.
- **One-line:** **Build a NeoFS `truncate` primitive** — the missing fs op that unblocks OT-17 M3 memory-delegation AND generalizes the A2/A3 undo you built (today reversible-write needs equal length only because there's no truncate). Add `truncate(new_len)` to the `Inode` trait (default Unsupported) + the NeoInode impl that **frees the released blocks** walking the direct/single/double/triple indirect map (ADR-064 — no leak, no double-free). **Byte-oracle** the truncate across indirect boundaries (surviving bytes exact, free-count returns, past-end reads per ABI). Cap-gate the syscall if exposed (I-15 WRITE cap). **Kill-switch:** additive/inert — existing fs bit-for-bit today. **No new invariant; fs surface only — do NOT rewire A2/A3 undo (small follow-on) or touch the agentic/WiFi/mm-slab/observability surfaces.** **Gate:** 4-shard QEMU + AAG; rebase before close. RETURN shard URLs + SHA + the byte-oracle pass + the inert-equality.
