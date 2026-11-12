<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Seat E, F0 — NeoFS directory operations (2026-11-11)

**Seat:** Seat E (cloud). **Branch:** `claude/f0-neofs-dir-ops` off `origin/main`.
**Design:** `docs/THINK-2026-10-29-ot88-app-model.md` §4.2 (F0) and §7 (the rung row), as approved.
**Why now:** `Plans/AUDIT-2026-11-11-mvp-app-chain.md` (#105). F0 is the one kernel rung on the critical path to
"Files opens a file in the editor and saves it". Its dependency, K1b's scoped directory caps, is landed, and it
conflicts with nothing in flight.

## Build (§4.2, as written)

- `mkdir_at(dir, name)`, `unlink_at(dir, name)` (a file, or an **empty** folder), `rename_at(dir, name, dir2, name2)`
  (within one volume, with **no replace**: renaming over an existing name is refused), and `stat_at(dir, name)`.
  Each is bounded by the rights on the directory handle it's given (WRITE, or LOOKUP for `stat_at`).
- Each operation is **one commit** through #180's commit barrier: the superblock is written last, and freed blocks
  aren't reused until the commit is durable.
- **Kill-switch `neofs-dir-ops`:** off means none of the four calls exists, and today's syscall table is unchanged.
- **Not in F0:** timestamps, folders over 64 entries, names over 47 bytes, and files over 32 KiB written through the
  ordinary path.

## Oracles and gate

- **Byte oracle:** a hand-laid directory block before and after each operation, independent of the writer and
  reader (the format rule).
- **Crash cuts:** a scenario cuts power after each write of each operation. The tree must be either before or after,
  never in between, the way #180's scenarios do it.
- **Rights:** a handle without WRITE can't delete, make a folder or rename. A handle without LOOKUP can't stat.
- The **4-shard gate** (full-alone) and `scripts/worker-close-gate.sh`.

## Kernel-touching: the standing rules

- **Concurrency pitfall bank** (`docs/concurrency-pitfalls.md`): run it over the NeoFS commit path and the handle
  rights. State per class whether the change introduces it, guards it or leaves it alone, and name the discriminator.
- **AAG** with a **cross-family lens** (Codex, GPT-6.1-Sol high), because it touches the NeoFS commit path. Rights
  enforcement in Claude-authored code also gets a **Muse security read**.
- **I-14:** no AI-governance lock is taken on these paths.

## Coordination

- #100 also adds a syscall; expect a textual merge at the syscall table.
- OT-100 R3 (the persistence mount) comes later and may meet F0 at mount time. Note anything R3 should know.

## Close

Commit and push at every checkpoint. Post PM: with the 4-shard run. The PM queues the reviews.
