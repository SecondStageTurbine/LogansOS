# PM Disposition - Phase 3 BUILD-4B Close Consumption And Ratification

Date: 2026-05-20
From: PM
Status: ACCEPTED / BUILD-4B CLOSED / PUSH-AND-MERGE AUTHORIZED
Consumed artifact:

```text
Plans/PHASE-3-BUILD-4B-CLOSE-EVIDENCE-2026-05-20.md
```

Commit:

```text
3097126 build-4B: add mount table and directory-capability mount syscalls
```

Diff: 8 files, 1322 insertions, 64 deletions.

## 1. Disposition

BUILD-4B is closed. All dispatch acceptance gates from
`Plans/DISPATCH-2026-05-20-phase-3-build-4B-mount-table-and-mount-syscalls-cerebro.md`
are met. Per dispatch §8, PM consumes the close artifact and authorizes
push and merge to main.

## 2. Gate Verification

PM walked the close artifact against the dispatch contract; every gate
passes:

- Rights/syscall constants (Step 1): `FileRights::MOUNT = 1 << 27` active,
  `MOUNT_RESERVED` alias retained, `KNOWN_BUILD4B = KNOWN_BUILD4A | MOUNT`
  in both kernel and libneo, `SYS_MOUNT = 17`, `SYS_UMOUNT = 18` dispatched.
- Mount-table substrate (Step 2): `MountTable { root, neofs: Option<...> }`
  special case replaced with explicit `MountEntry` table; root testfs and
  NeoFS represented as normal entries; live-handle counter is
  `Arc<AtomicUsize>` per-mount.
- Helper preservation (Step 3): `mount_neofs_from_device0_readonly`/`_readwrite`
  route through the new table; return `VfsError::Busy` on conflict rather
  than force-unmounting. No force-unmount path landed.
- `sys_mount` (Step 4): `MountRequestV1` parser with checked offsets;
  `dir_fd` resolved without premature rights gating; type-first
  (`NotDirectory → ENOTDIR`) before rights-second
  (`InsufficientRights → EACCES`); `EEXIST` on collision.
- `sys_umount` (Step 5): same type-first/rights-second discipline; `EBUSY`
  while `live_handles > 0`; entry removed only after busy check passes.
- All seven required scenarios PASS with literal `NEO_TEST/1 event=PASS`
  frames in close artifact §6.
- All seven anti-regression scenarios PASS including the previously-closed
  stale-fsync and wake-token-abandon scenarios; the mount-table refactor
  did not regress either lane.
- `NEO_SMP=4 cargo neo-test --no-meta` exits 0; end-of-suite scenario
  ordering matches prior closes.
- Forward-trigger conditions for a future two-phase umount refinement
  recorded verbatim in close artifact §10 per PM directive.

## 3. Substrate Confirmation Cross-Check

Cerebro's status-pulse claims from earlier today line up with the close
artifact and with Jarvis's pre-implementation audit:

| Claim | Status-pulse | Close artifact | Jarvis audit |
|---|---|---|---|
| Mount authority directory-FileCap, not ambient path | ✓ | §3 | (out of audit scope but verified) |
| Type-first / rights-second preserved | ✓ | §3 | ✓ |
| `EBUSY` precise via OpenFile-construction/Drop counter | ✓ | §9 | ✓ (counter on OpenFile lifetime, not fd-slot) |
| No FAT32/ext4 | ✓ | §4, §12 | (out of scope) |
| No force-unmount semantics needed | ✓ | §11 helper preservation | (out of scope) |
| No new `SwitchMutex` site | ✓ | §11 | (out of scope) |

This is the alignment that the up-front Jarvis audit was supposed to
produce. The audit-to-implementation handoff worked as designed.

## 4. Accepted Deviations

Three deviations disclosed in close artifact §11; PM accepts each:

**D1.** `MountedFs::root()` returns `Result<Arc<dyn Inode>, VfsError>`
instead of the dispatch's suggested `Arc<dyn Inode>`. Rationale (NeoFS
root snapshot path is fallible; share the error vocabulary with
`lookup`) is sound. Dispatch framing was "suggested shape, not mandatory
type spelling."

**D2.** `cfg(neo-test)`-gated `neofs_typed: Option<Arc<NeoFs>>` shortcut
for debug helpers, avoiding new `Any` machinery. Reasonable; storage is
test-cfg-gated and not load-bearing for the syscall path.

**D3.** Additive `cfg`-feature extensions on `neofs_2c_fail_vfs` and
`require_phase3_virtio_blk_ready` to make them visible under
`neo-test-phase3-mount-table`. This was the FTBFS issue Cerebro flagged
in the mid-lane status pulse; PM confirmed at that time it was not a
stop condition. The fix is additive and does not change behavior under
any existing feature.

None of the deviations affect substrate semantics.

## 5. Warning Count Movement Accepted

Cerebro disclosed the 19-warning reduction (72 → 53) in §5 as a natural
consequence of the refactor scope: dead-code warnings on conditional
branches that became unreachable when `MountTable { root, neofs }` was
replaced. PM finds the disclosure adequate per the standing rule that
warning reductions are not violations of the "no new unjustified
warnings" stop condition, provided their source is named.

## 6. Documentation Hygiene Items

Two minor inconsistencies in the close artifact that do not affect
closure but should be cleaned up in a follow-up pass:

a. Close artifact §1 marks status as "IN PROGRESS — substrate landed,
   all 7 required scenarios PASS, anti-regression in flight" but
   downstream sections (§7, §8, §14) show all anti-regressions and the
   final NEO_SMP=4 gate PASS. The header status is stale.

b. Diff-stat discrepancy: close artifact §1 shows "1117 insertions"
   while the commit landed at "1322 insertions" per Cerebro's status
   table. The 205-line difference likely reflects test/evidence content
   added between §1 capture and final commit. Reconcile in a follow-up.

PM does not block closure on these. They are post-hoc cleanup, not
substrate issues. Filing as a small Cerebro or Prometheus follow-up
task during a quiet window.

## 7. Authorized Forward Lanes

### 7.1 Push and merge

Per dispatch §8, push and merge of commit `3097126` to main is now
authorized. Standard git posture from current handoff applies: local
main is ahead of `origin/main` and behind by README-only cherry-picks;
do not rebase or rewrite without explicit PL guidance on reconciling
the evidence trail.

### 7.2 ANSPEC-002 v0.1.1 Open Question Q6

With BUILD-4B closed, the cascade-revocation × mount-interaction
question (ANSPEC-002 v0.1.1 §16.3, Q6) is now unblocked. The mount-
table semantics, EBUSY discipline, and live-handle accounting model
are visible in the implementation, which is what Q6 was waiting on.

PM will surface Q6 to PL for decision in a separate disposition.
Specifically: when a mounted filesystem containing AI-object storage
is unmounted, are outstanding `ModelCap`s/`TensorCap`s treated as
live handles (forcing EBUSY) or as cascade-revocation targets
(allowing unmount by invalidating caps)?

### 7.3 Bible-amendment triage packet

PM previously committed to surfacing the Theseus-style fault recovery
candidate (and the optional Article 23.4 substrate-identity-vs-
narrative-identity framing) to PL after BUILD-4B closes. This is the
natural moment. PM will assemble the packet as a separate artifact.

### 7.4 BUILD-4C (FAT32 read-only)

Per ADR-021 and the BUILD-4 prereq audit, BUILD-4C is the next
authorized Phase 3 lane. Agent assignment per current roster:
heavy-coding work goes to Jarvis (Codex). BUILD-4C requires `mtools`
in the dev container which the audit confirmed is present.

PM does not authorize BUILD-4C in this disposition. Authorization will
be a separate dispatch after PL acknowledges BUILD-4B closure and
confirms whether the bible-amendment packet or BUILD-4C goes first.

### 7.5 Follow-up tasks (small, low-priority)

- Documentation hygiene cleanup on close artifact §1 (stale status,
  diff-stat reconciliation). Cerebro or Prometheus, opportunistic.
- Bible-citation sweep across ANSPEC-001/ANSPEC-002/ADR-* for stale
  "v1.0.4" or earlier references; correct to current v1.6.0. Article
  addresses still verify; only version numbers are stale. Prometheus,
  opportunistic.

## 8. Phase 3 Status

After BUILD-4B closes:

- BUILD-1, BUILD-2, BUILD-3, BUILD-4A, BUILD-4B: closed.
- BUILD-4C (FAT32 r/o), BUILD-4D (ext4 r/o), BUILD-4E (Phase 3
  integration close): not yet started.
- BUILD-5 (AHCI/NVMe/bare-metal): not yet started. This is the bare-
  metal validation lane that Article 16.3 requires before Phase 3
  closes. PL hardware-target call (likely a ThinkPad T-series per
  current parallel discussion) will inform the dispatch.

Phase 3 is roughly two-thirds closed by BUILD count.

## 9. PM Posture

Three concurrent lanes have closed today:

- Cerebro: BUILD-4B (this disposition).
- Jarvis: FD-teardown interleaving audit (already consumed via
  `Plans/PM-DISPOSITION-2026-05-20-phase-3-build-4B-fd-teardown-audit-consumption.md`).
- Prometheus: ANSPEC-002 v0.1.1 amendment (already consumed and
  ratified via
  `Plans/PM-DISPOSITION-2026-05-20-anspec-002-v011-amendment-ratification.md`).

The day's work is consumable. PM stands ready for PL acknowledgment and
the post-closure decisions (Q6 resolution, bible-amendment triage,
BUILD-4C authorization, bare-metal target selection).
