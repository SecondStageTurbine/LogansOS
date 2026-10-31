<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Arthur memory M1 — directory + typed record format + ephemeral fallback → Janus

**Date:** 2026-06-18
**From:** PM (Claude)
**To:** Janus (built Slice 1 fd routing, Slice 4 action surface, the 8b input path, and `arthur_console` — owns the userland/Arthur I/O lane). New branch off `origin/main` HEAD: `codex/phase-8-arthur-memory-m1-janus`.
**Status:** DISPATCHED. **The first slice of Arthur's persistent memory** — the build that gives Arthur an actual durable memory of the user. The ratified OT-17 design's M1 (D1 + D2 + D6): a memory directory, the typed record format, and the fail-safe fallback. **Plaintext, Arthur-only, no forgetting** — encryption (M2) is blocked on the userland randomness slice, delegation (M3) and erasure (M4) are later.

## 0. The decision this build makes
Arthur can **persist and reload memories across sessions** — durable, typed, human-readable records in NeoFS, with a fail-safe that degrades cleanly to today's ephemeral Arthur when anything is wrong. **No encryption, no cap delegation to other processes, no erasure** — those are M2/M3/M4. Single pillar (OT-22): the substrate + format + fallback only.

## 1. Authoritative design — RATIFIED (read first)
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-18-arthur-memory-substrate-design.md   # D1 (where), D2 (format), D6 (fail-safe), §10 ratified
git show origin/main:docs/OPEN-THREADS.md                                        # OT-17 RATIFIED; the M1->M4 slicing; OT-22 (one pillar at a time)
```
Reuse: NeoFS file ops + the `Handle::FileDir` capability; Slice-1 spawn handle inheritance (`neo-shell`/`neo-init` → `neo-arthur`); the `neo-arthur` conversation loop (Slice 3) that this memory feeds.

## 2. ISCs
- **ISC-1 — The memory directory + typed record format (D1 + D2).** `.arthur/memory/<category>.mem` (one file per category) in NeoFS. Records are **line-delimited**, format **`version:type:timestamp:content`** (D2): four `type`s — `fact`, `preference`, `event`, `relationship`; a `version` field for forward-compat. **Plaintext** (no encryption — M2). Read path: Arthur loads its `.mem` files at startup, parses records, makes them available to the conversation loop. Write path: append a record. **Format byte-oracle (CLAUDE.md "Format/ABI specs require an independent byte oracle"):** ship a **hand-laid record example asserted byte-for-byte** against the parser/writer — *not just round-trip* (a writer+reader from one spec agree even when both are wrong; the `.neom` v1 lesson). Host-KAT the parse/serialize.
- **ISC-2 — The fail-safe fallback (D6 — the load-bearing ISC).** **Every** failure mode degrades to today's ephemeral OD-6 sliding window, **never crashes, never leaks**: missing dir/file → ephemeral (or create-empty); a corrupt file / malformed record / bad version → skip the bad record (or the file) and continue, **never panic**; a read that would surface another principal's data → refuse (Arthur reads only its own memory dir). Host-test this **exhaustively** — each failure mode → the ephemeral path, asserted. This is the privacy/robustness guarantee; it's the heart of M1.
- **ISC-3 — Arthur holds a `Handle::FileDir` cap to its memory dir (least-privilege, Arthur-only).** Arthur is granted a capability for `.arthur/memory/` **only** (at spawn, via the existing handle-inheritance path — `neo-shell`/`neo-init` derives it). **No delegation to other processes** (that's M3). This is the D7 least-privilege floor applied to memory: Arthur reaches its own memory and nothing else.
- **ISC-4 — Kill-switch + standards.** Feature **`arthur-memory` default-off ⇒ Arthur is the current ephemeral sliding-window (OD-6), bit-for-bit** — the existing `arthur_converse` behavior is unchanged (the I-2 regression). Host-testable record-format + fallback logic in a `*_logic` split (the established pattern). No new invariant; no new `unsafe`. `fmt`/`clippy --workspace --all-targets -D warnings` via Docker (kernel clippy blocking). ANSPEC §11.x (Arthur memory) or a short note; fold the format into the OT-17 THINK's record-format section as the as-built reference.

## 3. Gate
- **Host:** the record-format KAT (parse/serialize + the hand-laid byte-oracle, exact) + the **exhaustive fail-safe tests** (every corrupt/missing/malformed case → ephemeral, never panic) green; builds on+off the feature.
- **QEMU scenario `arthur_memory`:** Arthur writes a memory in one session → a fresh Arthur in a later session **reads it back** (persistence across the session boundary); **feature-off ⇒ no persistence** (ephemeral, the kill-switch). At **NEO_SMP=4**. Plus a corrupt-memory-file case → Arthur boots ephemeral, no crash.
- **Kernel-touching (the scenario + any spawn-cap plumbing) ⇒ pre-merge `workflow_dispatch` QEMU gate green** (all 4 shards) + clippy via Docker.

## 4. Scope guard
The directory + typed record format + the fail-safe fallback + Arthur's own memory-dir cap **only**. **Do NOT** add encryption (M2 — blocked on the userland `SYS_GETRANDOM` slice anyway), cap delegation to other processes (M3), erasure/forgetting (M4), grant ergonomics (OT-18), or a query/index layer (the THINK rejected the DB approach — minimal-sufficient). Plaintext, Arthur-only, append+read. No new invariant/`unsafe`.

## 5. Merge (PM)
**Close-gate: run `scripts/worker-close-gate.sh`** (rebase-freshness vs current `main`, the exact CI host-check set on the rebased tree, SPDX, self-stat). The two hard rules: **"green" without a CI URL is not done** (paste the green push-CI URL **and** the `workflow_dispatch` QEMU URL — kernel-touching), and the PM re-inspects your commit's own `git show --stat`. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (Article 13 — **Arthur's first durable memory**). Next: **M3** (cap delegation — Arthur derives USE-only memory caps for grantees; needs no CSPRNG) or **M2** (encryption — when the userland `SYS_GETRANDOM` slice lands). M1/M3 are CSPRNG-independent; M2 waits on 10.3.

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main` and confirmed. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Janus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-18-arthur-memory-m1-janus.md`. **Branch:** `codex/phase-8-arthur-memory-m1-janus` off `origin/main`.
- **One-line:** Build **Arthur memory M1** — the first ratified OT-17 slice: a NeoFS memory directory (`.arthur/memory/<category>.mem`), the typed line-delimited record format **`version:type:timestamp:content`** (types: fact/preference/event/relationship), and the **fail-safe fallback**. Arthur loads memories at startup + appends; **plaintext** (encryption is M2, blocked on the userland randomness slice), **Arthur-only** (a `Handle::FileDir` cap for its own dir, granted at spawn — no delegation, that's M3), **no forgetting** (M4). **The load-bearing ISC is the fail-safe (D6):** every failure mode (missing dir, corrupt file, malformed record, bad version) degrades to today's ephemeral OD-6 window — **never crash, never leak another principal's data** — host-tested exhaustively. **Format byte-oracle:** a hand-laid record example asserted byte-for-byte (not just round-trip — the `.neom` lesson). Kill-switch: `arthur-memory` default-off ≡ today's ephemeral Arthur bit-for-bit. Gate: the format KAT + exhaustive fail-safe tests + the `arthur_memory` QEMU scenario (write → fresh-session read-back; feature-off ⇒ no persistence; corrupt-file → ephemeral, no crash) at NEO_SMP=4 + **pre-merge QEMU gate** + clippy via Docker. **Scope: directory + format + fallback + own-dir cap ONLY — NO encryption/delegation/erasure/query-layer.** **Close-gate: run `scripts/worker-close-gate.sh`; RETURN pastes the green push-CI + QEMU URLs.**
