# PREP — Phase 3 BUILD-2 Close Evidence Shell (Prometheus)

**Date:** 2026-05-13
**Author:** Prometheus (Qwen3.6-27B)
**Type:** PREP ONLY — close-evidence shell. Does NOT claim BUILD-2 closure. 2E pending Jarvis return.
**Authority:** `Plans/PM-DECISION-2026-05-13-phase-3-build-2-2d-consumption-and-2e-authorization.md`

---

## 1. Sub-Commit Inventory

| Sub-commit | Commit SHA | Description | Verification Artifact | PM Decision | Dispatch | Status |
|---|---|---|---|---|---|---|
| 2A | `f781957` | NeoFS v1 format spec + mkfs skeleton | `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2A-VERIFICATION-2026-05-13.md` | `Plans/PM-DECISION-2026-05-13-phase-3-build-2-2a-consumption-and-2b-authorization.md` | `Plans/DISPATCH-2026-05-13-phase-3-build-2-2a-neofs-format-mkfs-jarvis.md` | **ACCEPTED** |
| 2B | `309f28e` | Read-only NeoFS mount from virtio-blk | `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2B-VERIFICATION-2026-05-13.md` | `Plans/PM-DECISION-2026-05-13-phase-3-build-2-2b-consumption-and-2c-authorization.md` (if filed) or covered by 2C consumption | `Plans/DISPATCH-2026-05-13-phase-3-build-2-2b-neofs-readonly-mount-jarvis.md` | **ACCEPTED** |
| 2C | `d2c8810` | NeoFS create/write/readback for Regular files | `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2C-VERIFICATION-2026-05-13.md` | `Plans/PM-DECISION-2026-05-13-phase-3-build-2-2c-consumption-and-2d-authorization.md` | PM chat dispatch (file not present in tree) | **ACCEPTED** |
| 2D | `a2f1e49` | CoW ordering, recovery-safe mount selection, fsync no-op | `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2D-VERIFICATION-2026-05-13.md` | `Plans/PM-DECISION-2026-05-13-phase-3-build-2-2d-consumption-and-2e-authorization.md` | `Plans/DISPATCH-2026-05-13-phase-3-build-2-2d-cow-fsync-recovery-jarvis.md` | **ACCEPTED** |
| 2E | **[PENDING]** | Typed-inode policy activation + FileCap stale-handle rejection | **[PENDING — awaiting Jarvis artifact]** | `Plans/PM-DECISION-2026-05-13-phase-3-build-2-2d-consumption-and-2e-authorization.md` (authorizes 2E) | `Plans/DISPATCH-2026-05-13-phase-3-build-2-2e-typed-inode-filecap-jarvis.md` | **PENDING** |

Branch: `codex/phase-3-build-2-2d-cow-fsync-recovery` (HEAD at `a2f1e49`; 2E commit not yet landed).
Plan consumption decision: `Plans/PM-DECISION-2026-05-13-phase-3-build-2-plan-consumption-and-2a-authorization.md`.

---

## 2. Close Report Template

### Section 1 — Status

```
Phase 3 BUILD-2 [CLOSED / BLOCKED].
Kernel mounts a NeoFS image from virtio-blk and performs crash-consistent
native file create/read/write with typed-inode enforcement, redundant
superblock rotation, CoW ordering, inode-generation stale-handle rejection,
and FileCap rights enforcement.
```

**Status slot:** `[CLOSED]` — filled after 2E artifact accepted by PM.

---

### Section 2 — Commit Series and Branch

| SHA | Sub-commit | Description |
|---|---|---|
| `f781957` | 2A | NeoFS v1 format spec + mkfs skeleton |
| `309f28e` | 2B | Read-only NeoFS mount from virtio-blk |
| `d2c8810` | 2C | NeoFS create/write/readback for Regular files |
| `a2f1e49` | 2D | CoW ordering, recovery-safe mount selection, fsync no-op |
| `[PENDING]` | 2E | Typed-inode policy + FileCap stale-handle rejection |

Branch: `codex/phase-3-build-2-*` (implementation branch series). Base commit from BUILD-1: `7f48c85`.

---

### Section 3 — Sub-Commit Inventory

(Repeats §1 table with final SHAs after 2E lands.)

| Sub-commit | SHA | Verification Artifact | PM Decision | Status |
|---|---|---|---|---|
| 2A | `f781957` | `[path]` | `[path]` | ACCEPTED |
| 2B | `309f28e` | `[path]` | `[path]` | ACCEPTED |
| 2C | `d2c8810` | `[path]` | `[path]` | ACCEPTED |
| 2D | `a2f1e49` | `[path]` | `[path]` | ACCEPTED |
| 2E | `[SHA]` | `[PENDING]` | `[PENDING]` | **[PENDING]** |

---

### Section 4 — Capability Summary

BUILD-2 delivers these capabilities over BUILD-1 substrate:

1. **NeoFS v1 format** — On-disk spec (`docs/specs/neofs-format.md`), host-side mkfs tool producing deterministic images with three redundant superblock copies, BLAKE3 checksums, typed inode discriminants (Regular, Directory, Model, reserved future types).
2. **Read-only mount** — Kernel NeoFS parser validates superblock magic, checksum, version, block size, inode layout, root pointer bounds, generation, child type matching, and content hashes. Scans three superblock offsets, selects highest valid generation. Rejects corrupted candidates.
3. **Native file create/write/readback** — `sys_open` extended with `OPEN_CREATE_REGULAR`. Narrow `/neofs/<name>` Regular-file creation. Writes up to eight direct blocks via monotonic bump allocator. Same-boot readback and two-boot persistence verified.
4. **CoW ordering and crash recovery** — Write path writes new data/inode/root before publishing superblocks. Mount selection validates reachable root before accepting candidate superblock. Newer invalid candidates rejected in favor of older fully mountable generation. Deterministic crash scenarios: mid-overwrite recovery (committed version survives), mid-create recovery (incomplete create absent), superblock rotation with root-invalid newer copy rejected.
5. **`SYS_FSYNC = 15` no-op** — Wired as documented no-op over synchronous write-through I/O. No intent log, journal, write-back cache, or FLUSH negotiation introduced.
6. **Typed-inode policy + FileCap stale-handle rejection** — **[2E PENDING]** Model inodes readable but exec-policy forbidden; Regular inodes pass exec-policy only with rights; type-first/rights-second dispatch with distinct `TypeForbidden` vs `InsufficientRights`; FileCap captures inode generation at issue time; stale FileCap returns distinct `ESTALE = -116`.

---

### Section 5 — Runtime Evidence

#### 5.1 Format and mkfs (2A)

| Evidence Slot | Expected | Actual (from 2A verification) | Status |
|---|---|---|---|
| Deterministic mkfs image | Same SHA-256 across runs | `dd845cb0e9a4065e3d548274f4259da948ee4762d8335eb620ddad55175b1c85` — identical on two runs | PASS |
| NeoFS format spec exists | `docs/specs/neofs-format.md` produced | File present; superblock, inode, directory entry layouts defined | PASS |

#### 5.2 Mount (2B)

| Evidence Slot | Expected | Actual (from 2B verification) | Status |
|---|---|---|---|
| Read-only mount from virtio-blk | `neofs_mount=ok` in serial log | `phase3_neofs_mount_readonly_root`: `neofs_mount=ok selected_superblock=0 generation=1 valid_superblocks=3 root_kind=Directory hello_read=ok model_kind=Model readonly_write_denied=InsufficientRights` | PASS |
| Highest-generation superblock selection | Mount selects gen=3 copy at slot 2 | `phase3_neofs_mount_selects_highest_generation`: `selected_superblock=2 generation=3 highest_generation_selected=true valid_superblocks=3` | PASS |
| Corrupted/bad superblock rejection | Primary rejected, fallback to valid copy | `phase3_neofs_mount_rejects_bad_superblock`: `corrupt_primary_rejected=true rejected_superblock_mask=0x01 valid_superblocks=2 rejected_superblocks=1` | PASS |

#### 5.3 Native file I/O (2C)

| Evidence Slot | Expected | Actual (from 2C verification) | Status |
|---|---|---|---|
| Create Regular file | New inode, generation=1, type=Regular | `phase3_neofs_create_small_file`: `neofs_create=ok create_flag=OPEN_CREATE_REGULAR path=/neofs/create-2c.txt bytes=40 close_reopen_read=ok generation=1 no_directory_create=true no_model_create=true` | PASS |
| Same-boot write/readback | Byte-level integrity match | `phase3_neofs_write_readback_same_boot`: `neofs_write_readback=ok path=/neofs/direct8.bin bytes=32768 direct_blocks=8 close_reopen_read=ok` | PASS |
| 8-direct-block cap | File limited to 8 direct blocks | `direct_blocks=8`, `bytes=32768` (8 × 4096) | PASS |
| Bump allocator no duplicate live blocks | Two files get distinct blocks, no aliasing | `phase3_neofs_allocator_reuse_guard`: `alias_check=distinct_content bump_allocator=true` | PASS |
| Two-boot persistence | Data survives QEMU reboot on same image | `phase3_neofs_reboot_persistence_small_file`: boot 1 `stage=write`, boot 2 `stage=readback image=reused selected_superblock=0 generation=4` — both PASS | PASS |

---

### Section 6 — Recovery Evidence (2D)

| Evidence Slot | Expected | Actual (from 2D verification) | Status |
|---|---|---|---|
| Write-after-write CoW with distinct old/new blocks | Old block preserved, new block allocated | `phase3_neofs_cow_write_after_write`: `neofs_cow=ok old_data_block=16395 new_data_block=16399 distinct_blocks=true` | PASS |
| Mid-overwrite recovery | Committed version A survives; unpublished version B absent | Boot 1: `injection=before_superblock_publish`; Boot 2: `recovered=version_a unpublished_version_b_absent=true selected_superblock=0 generation=3` | PASS |
| Mid-create recovery | Incomplete create absent on reboot; committed marker present | Boot 1: `injection=before_superblock_publish`; Boot 2: `target_absent=true marker=/neofs/create-marker-2d.txt generation=3` | PASS |
| Invalid newer superblock candidate rejected | Slot 2 (root-invalid) rejected; older gen=3 selected | Boot 2: `invalid_candidate_rejected=true rejected_superblock_mask=0x04 selected_superblock=0 generation=3` | PASS |
| `SYS_FSYNC = 15` no-op | Returns success, no intent log or FLUSH | `phase3_neofs_fsync_noop_write_through`: `neofs_fsync=noop fsync_rc=0 write_through=true no_flush_command=true no_intent_log=true generation_before=3 generation_after=3` | PASS |

---

### Section 7 — Typed Inode and FileCap Evidence (2E)

**Status: [PENDING — awaiting Jarvis 2E artifact]**

| Evidence Slot | Expected Outcome | Status |
|---|---|---|
| Model readable but exec-policy forbidden | `mmap(PROT_EXEC)` on Model inode → `TypeForbidden`; `sys_read`/`mmap(PROT_READ)` → success | **[PENDING]** |
| Regular exec-policy success only with rights | `mmap(PROT_EXEC)` on Regular + MMAP_X right → success; without right → `InsufficientRights` | **[PENDING]** |
| Type-first / rights-second four-corner matrix | (type allows, cap allows) → OK; (type allows, cap denies) → InsufficientRights; (type denies, cap allows) → TypeForbidden; (both deny) → deterministic error | **[PENDING]** |
| `TypeForbidden` distinct from `InsufficientRights` | Distinguishable syscall errno codes; audit log can tell which gate failed | **[PENDING]** |
| FileCap captures inode generation at issue time | FileCap struct includes `gen: u32`; set to current inode generation on create/open | **[PENDING]** |
| Stale FileCap returns distinct stale errno | Mutate through handle A (refreshes gen); handle B still has old gen → returns `ESTALE = -116` | **[PENDING]** |
| `FileRights` remains `u32` with forward bits | Rights word is 32 bits; RETYPE (1<<16) and future bits representable | **[PENDING]** |

---

### Section 8 — Regression and Build Evidence

#### 8.1 Regression gates after 2D (last verified lane before 2E)

| Gate | Scenario | Source | Status (from 2D verification) |
|---|---|---|---|
| R-2B-1 | `phase3_neofs_mount_readonly_root` | 2B | PASS |
| R-2B-2 | `phase3_neofs_mount_selects_highest_generation` | 2B | PASS |
| R-2B-3 | `phase3_neofs_mount_rejects_bad_superblock` | 2B | PASS |
| R-2C-1 | `phase3_neofs_create_small_file` | 2C | PASS |
| R-2C-2 | `phase3_neofs_write_readback_same_boot` | 2C | PASS |
| R-2C-3 | `phase3_neofs_allocator_reuse_guard` | 2C | PASS |
| R-2C-4 | `phase3_neofs_reboot_persistence_small_file` | 2C | PASS |
| R-BLK-1 | `phase3_block_vfs_smoke` | BUILD-1 | PASS (F_FLUSH offered/not negotiated intact) |
| R-BLK-2 | `phase3_virtio_blk_raw_rw_smoke` | BUILD-1 | PASS |

**2E regression requirement:** After 2E lands, the full matrix above must rerun and pass. Additionally:

| Gate | Scenario | Requirement | Status |
|---|---|---|---|
| R-SMP-1 | `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta` | Full non-explicit SMP sweep PASS | **[PENDING 2E]** |
| R-AARCH64-1 | `phase3_aarch64_virtio_barrier_lane` or explicit unchanged-substrate citation | aarch64 barrier lane rerun OR citation that 2E did not modify virtio/barrier/block-driver files | **[PENDING 2E]** |

#### 8.2 Build gates

| Gate | Command | Status (from latest verification) |
|---|---|---|
| B1 | `cargo k` | PASS (2D) |
| B2 | `cargo k --no-default-features` | PASS (2D) |
| B3 | `cargo check -p xtask` | PASS (2D) |
| B4 | `cargo check -p neofs-mkfs` | PASS (2D) |
| B5 | `cargo check -p libneo -p neofs-mkfs` | PASS (2D) |
| B6 | `git diff --check` / `git diff --cached --check` | PASS (2D) |
| B7 | Scoped rustfmt for new/touched files | Caveat — pre-existing drift in long-lived shared files (`test_harness.rs`, `libneo/src/lib.rs`); scoped check on 2E-owned files must pass | **[PENDING 2E]** |

---

### Section 9 — Caveats and Explicitly Deferred Work

#### 9.1 Scoped rustfmt caveat (inherited from BUILD-1)

`rustfmt` is installed for the pinned nightly, but global `cargo fmt --check` fails on broad pre-existing formatting drift across the repo. Scoped rustfmt for new BUILD-2 source passes; pre-existing drift in long-lived shared files is a non-blocking style debt. Global formatting is not described as PASS.

#### 9.2 `SYS_FSYNC = 15` is a no-op

Over synchronous write-through I/O, `sys_fsync` returns success without any intent log, journal flush, or FLUSH command. This is the correct BUILD-2 behavior per ADR-021 Q7: fsync semantics are per-file intent-log flush in BUILD-3+ with FLUSH negotiation. BUILD-2 documents the no-op to prevent callers from assuming durability guarantees that don't exist yet.

#### 9.3 No real mmap/exec implementation

Typed-inode policy checks in 2E use a narrow kernel/VFS helper or test stub. Actual file-backed `mmap`, ELF loading from NeoFS, and process execution from disk are not implemented. The gate proves the type-first/rights-second dispatch logic and error distinction, not full executable-mapping infrastructure.

#### 9.4 Bump allocator limitations

The mount-owned bump allocator is monotonic — no free-space reuse or GC. This is intentional BUILD-2 scope; a proper allocator/GC is BUILD-3+ work.

#### 9.5 Explicitly deferred work

| Deferred Item | Target BUILD | Rationale |
|---|---|---|
| CAS content-addressing + dedup | BUILD-3 (measurement-gated) | ADR-021 Q8: correctness first, optimization when measured |
| Buffer-cache write-back + FLUSH negotiation | BUILD-3 | ADR-021 §BUILD-3 |
| Mark-sweep GC | BUILD-3 | Requires CAS substrate |
| kthread primitive | BUILD-3 prerequisite | Background GC needs kernel threads |
| FAT32/ext4 read-only | BUILD-4 | Foreign FS import, default `noexec` |
| `sys_mount`/`sys_umount`, readdir, unlink, rename | BUILD-4 | Broader POSIX-ish surface |
| SATA AHCI driver | BUILD-5 | Bible Article 9.9 |
| NVMe driver | BUILD-5 | Bible Article 9.9 |
| Bare-metal storage validation | BUILD-5 | Phase 3 closure requirement |
| Retype operation | BUILD-5 / Phase 5 | Requires generation counter infrastructure (laid in BUILD-2) |

**BUILD-3 not started.** No CAS, dedup, write-back cache, FLUSH negotiation, GC, or content-address index was introduced by any BUILD-2 sub-commit.

**No normative docs/index updates until post-close dispatch.** Phase docs, `Plans/INDEX.md`, ADRs, and Bible files are not edited by this close report.

---

### Section 10 — Closure Statement (template)

```
Phase 3 BUILD-2 [CLOSED].

NeoFS native v1 is mounted from virtio-blk with crash-consistent file
create/read/write. CoW ordering, redundant superblock rotation, and
mount-time root validation provide crash recovery without intent log or
journal. Typed-inode enforcement rejects Model executable mapping via
type-first/rights-second dispatch. FileCap generation checks reject stale
handles with ESTALE. All BUILD-1 regression gates pass. No BUILD-3
mechanism (CAS, dedup, write-back cache, FLUSH negotiation, GC) was
introduced.

No normative docs or index updates were edited by this report.
BUILD-3 is not started.
```

---

## 3. Stop Conditions for Close

Do NOT claim BUILD-2 closed if any of the following hold:

**2E artifact:**
- 2E verification artifact missing, not committed, or not accepted by PM;
- 2E commit SHA not yet available.

**Typed-inode/FileCap gates:**
- Stale FileCap not checked against current reachable inode generation (only stale in-memory snapshot);
- `TypeForbidden` and `InsufficientRights` collapsed to the same error code;
- Model inode passes exec-policy check;
- Regular inode without execute right passes exec-policy check;
- `FileRights` width pressure reintroduces 16-bit rights word.

**Fsync semantics:**
- `sys_fsync` implemented as an intent log, global transaction flush, or FLUSH command path (must remain no-op in BUILD-2).

**Crash recovery:**
- Crash recovery accepts a checksum-valid but root-invalid superblock candidate;
- Mid-overwrite recovery shows torn state (neither version A nor B fully visible);
- Mid-create recovery shows partially visible incomplete file.

**Regression evidence:**
- Full 2B/2C/2D/BUILD-1 regression matrix missing after 2E without explicit PM waiver;
- Non-explicit SMP sweep (`NEO_SMP=4 --no-meta`) fails after 2E.

**Anti-scope — BUILD-3 mechanisms must not appear:**
- CAS content-addressing index;
- Deduplication mechanism (refcounted DDT, duplicate block detection);
- Buffer-cache write-back or dirty-buffer tracking;
- virtio-blk FLUSH negotiation (`F_FLUSH` negotiated or `F_CONFIG_WCE` negotiated);
- Mark-sweep GC or background garbage collection;
- kthread primitive.

**Anti-scope — broader POSIX features must not appear:**
- `sys_readdir`, `sys_unlink`, `sys_rename`;
- `chmod`/`chown`;
- Real file-backed `mmap` or exec subsystem (2E stub only);
- Runtime mkfs from within the kernel.

**Formatting:**
- Global rustfmt described as passing when only scoped rustfmt was run.

---

## 4. Cross-References

| Artifact | Purpose |
|---|---|
| `Plans/DISPATCH-2026-05-13-phase-3-build-2-close-evidence-prep-prometheus.md` | This dispatch mandate |
| `Plans/PM-DECISION-2026-05-13-phase-3-build-2-2d-consumption-and-2e-authorization.md` | PM decision accepting 2D, authorizing 2E |
| `Plans/PM-DECISION-2026-05-13-phase-3-build-2-2c-consumption-and-2d-authorization.md` | PM decision accepting 2C, authorizing 2D |
| `Plans/PM-DECISION-2026-05-13-phase-3-build-2-2a-consumption-and-2b-authorization.md` | PM decision accepting 2A, authorizing 2B |
| `Plans/PM-DECISION-2026-05-13-phase-3-build-2-plan-consumption-and-2a-authorization.md` | PM decision consuming plan artifacts, authorizing 2A |
| `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2A-VERIFICATION-2026-05-13.md` | 2A verification — format spec + mkfs |
| `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2B-VERIFICATION-2026-05-13.md` | 2B verification — read-only mount |
| `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2C-VERIFICATION-2026-05-13.md` | 2C verification — create/write/readback |
| `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2D-VERIFICATION-2026-05-13.md` | 2D verification — CoW/recovery/fsync no-op |
| `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2E-VERIFICATION-*.md` | **[PENDING]** — 2E verification artifact (awaiting Jarvis) |
| `Plans/DISPATCH-2026-05-13-phase-3-build-2-2e-typed-inode-filecap-jarvis.md` | 2E dispatch for Jarvis |
| `Plans/PREP-2026-05-12-phase-3-build-2-evidence-matrix-prometheus.md` | Pre-implementation evidence matrix (superseded by this close-evidence shell) |
| `Plans/CLOSE-REPORT-2026-05-12-phase-3-build-1.md` | BUILD-1 close report (regression gate source) |
| `docs/ADR-021-phase-3-design-substrate.md` | Canonical Phase 3 design substrate with BUILD-2 scope and gates |
| `docs/specs/neofs-format.md` | NeoFS v1 on-disk format specification (produced by 2A) |

---

**End of close-evidence shell.**

*PREP ONLY. Does NOT claim BUILD-2 closure. 2E pending Jarvis return. No normative files edited.*