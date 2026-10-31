# DISPATCH - Prometheus BUILD-2 close evidence prep

**Date:** 2026-05-13
**Owner:** Prometheus
**Status:** DISPATCHED
**Mode:** evidence-prep only
**Authority:** BUILD-2 PM decisions through `Plans/PM-DECISION-2026-05-13-phase-3-build-2-2d-consumption-and-2e-authorization.md`

---

## 1. Mission

Prepare the BUILD-2 close-evidence shell while Jarvis implements 2E.

Output:

`Plans/PREP-2026-05-13-phase-3-build-2-close-evidence-prometheus.md`

This is prep only. Do not claim BUILD-2 closure. Do not edit phase docs,
indexes, ADRs, specs, kernel, userland, or tool files.

---

## 2. Inventory Required

Build an inventory table for:

- 2A format/mkfs: commit `f781957`, verification artifact path, accepted status.
- 2B read-only mount: commit `309f28e`, verification artifact path, accepted status.
- 2C create/write: commit `d2c8810`, verification artifact path, accepted status.
- 2D CoW/fsync/recovery: commit `a2f1e49`, verification artifact path, accepted status.
- 2E typed/FileCap: pending Jarvis artifact and commit.

Include the PM decision/dispatch artifacts that authorized each step.

---

## 3. Evidence Matrix To Prepare

Create a close-report skeleton with status slots for:

### Format and mount

- deterministic mkfs image;
- NeoFS format spec;
- read-only mount;
- highest-generation superblock selection;
- corrupted/bad superblock rejection;
- reserved/future type refusal.

### Native file I/O

- create Regular file;
- same-boot write/readback;
- 8-direct-block cap;
- bump allocator no duplicate live blocks;
- two-boot persistence.

### CoW and recovery

- write-after-write CoW with distinct old/new blocks;
- mid-overwrite recovery;
- mid-create recovery;
- invalid newer superblock candidate rejected;
- `SYS_FSYNC = 15` documented no-op over write-through I/O.

### Typed inode and FileCap

- `Model` readable but exec-policy forbidden;
- `Regular` exec-policy success only with rights;
- type-first / rights-second four-corner matrix;
- `TypeForbidden` distinct from `InsufficientRights`;
- FileCap captures inode generation;
- stale FileCap returns distinct stale errno;
- `FileRights` remains `u32` with forward bits representable.

### Regression gates

- 2B/2C/2D scenarios still pass after 2E;
- BUILD-1 block/VFS regressions;
- non-explicit SMP sweep with `NEO_SMP=4`;
- aarch64 barrier lane if relevant, or explicit unchanged-substrate citation;
- build gates: `cargo k`, `cargo k --no-default-features`, `cargo check -p xtask`,
  `cargo check -p neofs-mkfs`, `cargo check -p libneo -p neofs-mkfs`;
- `git diff --check` / cached check;
- scoped rustfmt caveat.

---

## 4. Stop Conditions For Close

List stop conditions that would prevent PM from accepting a BUILD-2 close
report, including:

- 2E artifact missing or not committed;
- full regression evidence missing without explicit PM waiver;
- stale FileCap not checked against current reachable inode generation;
- type and rights errors collapsed;
- `sys_fsync` implemented as an intent log/global transaction/FLUSH path;
- crash recovery accepts checksum-valid but root-invalid superblock;
- any BUILD-3 mechanism appears: CAS, dedup, write-back cache, FLUSH
  negotiation, GC, content-address index;
- any broader POSIX feature appears: readdir, unlink, rename, chmod/chown,
  real file-backed mmap/exec, runtime mkfs;
- global rustfmt described as passing when only scoped rustfmt was run.

---

## 5. Close Report Template

Prepare a 10-section close-report template for PM to fill after Jarvis returns:

1. Status.
2. Commit series and branch.
3. Sub-commit inventory.
4. Capability summary.
5. Runtime evidence.
6. Recovery evidence.
7. Typed/FileCap evidence.
8. Regression/build evidence.
9. Caveats and explicitly deferred work.
10. Closure statement.

The template must include a "BUILD-3 not started" assertion and a "No normative
docs/index updates until post-close dispatch" assertion.

---

## 6. Deliverable Rules

The artifact must clearly say:

- "PREP ONLY";
- "Does NOT claim BUILD-2 closure";
- "2E pending Jarvis return";
- "No normative files edited".

Keep it ready for PM consumption after Jarvis returns with 2E evidence.

