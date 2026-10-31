# PREP — Phase 3 BUILD-2 Evidence Matrix (Prometheus)

**Date:** 2026-05-12
**Author:** Prometheus (Qwen3.6-27B)
**Type:** Evidence matrix and closure-gate shell. Does NOT claim BUILD-2 closure. No implementation authorized yet.
**Authority:** PM decision `Plans/PM-DECISION-2026-05-12-phase-3-build-2-opening.md`

---

## 1. BUILD-2 Closure Claim Template

When all gates pass, the close report will assert:

```
Phase 3 BUILD-2 CLOSED.
Kernel mounts a NeoFS image from virtio-blk and performs crash-consistent
native file create/read/write with typed-inode enforcement, redundant
superblock rotation, per-file fsync intent-log semantics, inode generation
stale-handle rejection, and FileCap rights enforcement.
```

Stop conditions in §8 must all be satisfied before this claim is honest.

---

## 2. Sub-Commit Evidence Slots

Sub-commit boundaries are not yet finalized — Jarvis's observe pass will define them. This matrix defines the evidence slots that any sub-commit plan must fill. Each slot maps to a closure gate.

| Slot ID | Work Area | Expected Sub-Commit | Evidence Required |
|---|---|---|---|
| SC-FMT | `docs/specs/neofs-format.md` + mkfs tooling | Format spec lane | Spec artifact path; mkfs produces deterministic image with reproducible hash |
| SC-ALLOC | Physical block allocator, inode/tree layout | Core substrate | Allocator unit tests; no-free-list-corruption after N alloc/free cycles |
| SC-MOUNT | NeoFS mount from virtio-blk, superblock validation | Mount lane | Superblock parse PASS; redundant copy selection by highest generation + valid checksum |
| SC-FILE | Native file create/read/write, same-boot readback | File I/O lane | Create + write + readback in same boot; byte-level integrity check |
| SC-COW | CoW transaction path, per-file `sys_fsync` intent-log semantics | CoW/commit lane | Write-after-write proves copy-on-write (old block preserved); fsync flushes only target file's intent log |
| SC-TYPE | Typed inode discriminant, executable-mapping refusal | Typed-inode lane | Model inode rejects `mmap(PROT_EXEC)` with `TypeForbidden`; Regular inode allows; filename irrelevant |
| SC-CAP | FileCap rights enforcement, `FileRights(u32)`, GRANT/DERIVE stubs | Capability lane | Rights attenuation test; type-vs-rights error distinction (`TypeForbidden` vs `InsufficientRights`) |
| SC-GEN | Inode generation counter, stale-handle rejection | Generation lane | Retype simulation: increment generation, old FileCap returns `Stale` |
| SC-CRASH | Crash/recovery simulation, redundant superblock rotation | Crash-recovery lane | Kill mid-transaction; remount selects valid superblock; no corruption in committed data |

Optional inputs not yet available — slots reserved for PM consumption after parallel artifacts return:

| Slot ID | Source Artifact | Status |
|---|---|---|
| SC-CEREOFMT | `Plans/RESEARCH-2026-05-12-phase-3-build-2-neofs-format-cerebro.md` | NOT YET FILED — Cerebro format structural challenge pending |
| SC-JARVISPLAN | `Plans/OBSERVE-2026-05-12-phase-3-build-2-neofs-subcommit-plan-jarvis.md` | NOT YET FILED — Jarvis observe pass and sub-commit plan pending |

---

## 3. Runtime Scenario Matrix

Each scenario defines the setup, action, expected evidence, and which slot(s) it satisfies.

### 3.1 mkfs and image creation

| Scenario ID | Description | Setup | Action | Expected Evidence | Satisfies |
|---|---|---|---|---|---|
| S-MKFS-1 | Deterministic mkfs | Clean build, no pre-existing image | Run mkfs tool to create `target/neo-test/neofs.img` with known parameters (block_size=4096, total_blocks=8192) | Image file exists; deterministic SHA-256 hash matches expected value across builds | SC-FMT |
| S-MKFS-2 | mkfs parameter variation | Same tool, different block size | Run mkfs with block_size=512 | Different image hash; superblock reflects new block size field | SC-FMT |

### 3.2 Superblock validation and mount

| Scenario ID | Description | Setup | Action | Expected Evidence | Satisfies |
|---|---|---|---|---|---|
| S-MNT-1 | Valid superblock mount | Image from S-MKFS-1 attached as virtio-blk disk | Kernel boots, mounts NeoFS root | Serial log: `neofs_mount=ok superblock_gen=1 checksum=valid root_inode=<addr>` | SC-MOUNT |
| S-MNT-2 | Redundant superblock selection | Image with two valid superblock copies at different generations (gen=1 at slot A, gen=2 at slot B) | Kernel boots, mounts NeoFS root | Mount selects gen=2 copy; serial log: `selected_superblock=slot_B generation=2` | SC-MOUNT |
| S-MNT-3 | Corrupted superblock rejection | Image with invalid checksum in primary superblock | Kernel boots, attempts mount | Mount rejects primary, falls back to valid redundant copy (or fails cleanly if all copies corrupted) | SC-MOUNT |

### 3.3 Native file I/O

| Scenario ID | Description | Setup | Action | Expected Evidence | Satisfies |
|---|---|---|---|---|---|
| S-FILE-1 | Native file create | Mounted NeoFS from S-MNT-1 | `sys_create(dir_cap, "testfile", Regular, initial_rights)` → returns new FileCap | New inode allocated; generation=1; type=Regular in serial log | SC-FILE, SC-TYPE |
| S-FILE-2 | Native file write | FileCap from S-FILE-1 with WRITE right | `sys_write(file_cap, buffer, len)` where buffer is known pattern | Write completes; CoW allocates new block (old block preserved if overwritten) | SC-FILE, SC-COW |
| S-FILE-3 | Same-boot readback | File written in S-FILE-2 | `sys_read(file_cap, out_buf, len)` | Byte-level match between written pattern and readback; `data_integrity=ok` in serial log | SC-FILE |
| S-FILE-4 | Two-boot persistence readback | Disk image from S-FILE-2 boot preserved | Second QEMU boot reuses same disk image; mount NeoFS; open file; read | Readback matches pattern written in first boot; `persistence=ok` in serial log | SC-FILE |

### 3.4 CoW and fsync semantics

| Scenario ID | Description | Setup | Action | Expected Evidence | Satisfies |
|---|---|---|---|---|---|
| S-COW-1 | Write-after-write proves CoW | File with data in block N | Second write to same logical offset | New block M allocated; old block N still present in allocator (not overwritten); inode tree updated to point to M | SC-COW |
| S-COW-2 | Per-file fsync intent-log semantics | Two files A and B; write to both | `sys_fsync` on file A only | Only file A's intent log entries flushed; file B's data may still be in volatile state. Serial log distinguishes which file's log was flushed | SC-COW |

### 3.5 Typed inode enforcement

| Scenario ID | Description | Setup | Action | Expected Evidence | Satisfies |
|---|---|---|---|---|---|
| S-TYPE-1 | Model inode blocks exec | FileCap for Model-typed inode | `mmap(PROT_EXEC)` or equivalent exec attempt | Return `TypeForbidden`; serial log: `type=Model op=mmap_exec result=TypeForbidden` | SC-TYPE |
| S-TYPE-2 | Regular inode allows exec | FileCap for Regular-typed inode with MMAP_X right | `mmap(PROT_EXEC)` | Success; VMA created with PROT_EXEC | SC-TYPE |
| S-TYPE-3 | Filename does not imply type | Model inode named `legit.neom` | `mmap(PROT_EXEC)` | `TypeForbidden` — `.neom` suffix does not make it Regular | SC-TYPE |
| S-TYPE-4 | No suffix does not imply Regular | Regular inode named `binary` (no extension) | `mmap(PROT_EXEC)` with MMAP_X right | Success — lack of `.neom` does not make it Model | SC-TYPE |
| S-TYPE-5 | Model allows read | FileCap for Model-typed inode with READ right | `sys_read` or `mmap(PROT_READ)` | Success — gate is specifically on exec, not readability | SC-TYPE |

### 3.6 FileCap rights enforcement

| Scenario ID | Description | Setup | Action | Expected Evidence | Satisfies |
|---|---|---|---|---|---|
| S-CAP-1 | Type allows, cap denies | Regular inode; FileCap WITHOUT MMAP_X right | `mmap(PROT_EXEC)` | Return `InsufficientRights` (not `TypeForbidden`) | SC-CAP |
| S-CAP-2 | Cap allows, type denies | Model inode; FileCap WITH MMAP_X right | `mmap(PROT_EXEC)` | Return `TypeForbidden` (not `InsufficientRights`) | SC-CAP |
| S-CAP-3 | Both allow | Regular inode; FileCap with MMAP_X right | `mmap(PROT_EXEC)` | Success | SC-CAP |
| S-CAP-4 | Rights attenuation through openat | DirCap with CREATE right limited to READ\|WRITE | `sys_create` requesting READ\|WRITE\|MMAP_X | New FileCap rights = requested ∩ parent = READ\|WRITE; MMAP_X not silently dropped — explicit `InsufficientRights` if caller expected it | SC-CAP |
| S-CAP-5 | FileRights is u32 | Any FileCap | Inspect rights word width | Rights field is 32 bits, not 16; reserved bits available for RETYPE (1<<16), MOUNT_HERE, etc. | SC-CAP |

### 3.7 Inode generation and stale-handle rejection

| Scenario ID | Description | Setup | Action | Expected Evidence | Satisfies |
|---|---|---|---|---|---|
| S-GEN-1 | Generation initialized to 1 | New inode created via `sys_create` | Read inode generation field | Generation = 1 | SC-GEN |
| S-GEN-2 | Stale handle after generation increment | FileCap with gen=1; simulate retype by incrementing inode gen to 2 | Any syscall on old FileCap | Return `Stale`; serial log: `cap_gen=1 inode_gen=2 result=Stale` | SC-GEN |

---

## 4. Regression Gates Inherited from BUILD-1

These gates must still pass after every BUILD-2 sub-commit to prove NeoFS work does not regress the block/VFS substrate.

| Gate ID | Scenario | Source | Required Evidence |
|---|---|---|---|
| R-BLK-1 | `phase3_virtio_blk_raw_rw_smoke` | BUILD-1 SC-1B | F_FLUSH offered/not negotiated; descriptor publish barriers=2; notification suppression barriers=2 |
| R-BLK-2 | `phase3_block_vfs_smoke` | BUILD-1 SC-1D | Virtio-blk attaches; raw block R/W + VFS syscalls pass in same boot |
| R-BLK-3 | `phase3_storage_completion_cross_cpu_wake` | BUILD-1 SC-1D | Real virtio used-ring completion path; ThreadWakeToken wake path |
| R-BLK-4 | `phase3_build1_reboot_persistence_smoke` | BUILD-1 SC-1D | Two-boot reuse of raw disk image; write + readback |
| R-BLK-5 | `phase3_virtio_blk_abandoned_io_no_leak` | BUILD-1 SC-1B | `retained_dma=3`, `live_before=3`, `live_after=3` |
| R-VFS-1 | VFS stdout namespace rule | BUILD-1 SC-1C | `sys_write(1, ...)` routes to serial; fd=1 not consumed by VFS open |
| R-SMP-1 | `smp_cross_cpu_rendezvous_stress` at SMP=4 | Phase 2 anti-regression | `iterations=1000`, `send_first=500`, `recv_first=500`, `cpus=4` |
| R-AARCH64-1 | `phase3_aarch64_virtio_barrier_lane` | BUILD-1 aarch64 lane | `runtime=aarch64`, `compile_only=false`, `publish_descriptors=2`, `before_notification_check=2` |
| R-BUILD-1 | `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta` | BUILD-1 full sweep | Full non-explicit SMP suite PASS |

---

## 5. Crash/Recovery Gates

| Gate ID | Description | Setup | Action | Expected Evidence |
|---|---|---|---|---|
| C-CRASH-1 | Kill mid-transaction, remount recovers committed data | NeoFS image with file A written and fsynced; then start write to file B, kill QEMU before B completes | Remount NeoFS from same image | File A readable and intact (committed); file B either fully absent or fully present — no torn state. Serial log: `crash_recovery=ok committed_survive=A uncommitted_discarded=B` |
| C-CRASH-2 | Redundant superblock rotation survives crash | Two valid superblock copies at gen=N and gen=N+1; kill after writing new data but before rotating superblocks | Remount | Mount selects highest-generation superblock with valid checksum (gen=N+1). Data committed before kill is recoverable through that superblock's root inode pointer. |
| C-CRASH-3 | Corrupted superblock during crash | Kill mid-superblock-write so one copy has invalid checksum | Remount | Mount rejects corrupted copy by checksum, selects valid redundant copy. No silent acceptance of bad superblock. |

---

## 6. Typed-Inode/FileCap Gates

These are the specific ADR-021 and Cerebro-ratified gates that must pass for BUILD-2 closure.

| Gate ID | Requirement | Evidence |
|---|---|---|
| T-TYPE-1 | Inode type discriminant exists in on-disk inode format | `neofs-format.md` specifies type field; mkfs emits it; mount reads it |
| T-TYPE-2 | Model type reserved and enforced | `InodeType::Model` defined; `INODE_TYPE_POLICY[Model]` excludes MMAP_X, EXECUTE, WRITE |
| T-TYPE-3 | Type-first, rights-second dispatch | VFS entry points check inode type policy before FileCap rights; `TypeForbidden` returned before `InsufficientRights` check |
| T-TYPE-4 | Typed inode discriminant is versioned enum tag | 1-byte field with reserved range for future types (Tensor, InferenceContext); no format break needed |
| T-CAP-1 | FileCap stores inode generation at issue time | FileCap struct includes `gen: u32`; set to current inode generation on `sys_create`/`sys_open` |
| T-CAP-2 | FileRights is `u32` with reserved bits | Rights word is 32 bits; RETYPE (1<<16) and future bits reserved per Cerebro recommendation |
| T-CAP-3 | Type vs rights errors are distinguishable | `NeoError::TypeForbidden` ≠ `NeoError::InsufficientRights`; audit log can tell which gate failed |
| T-FORWARD-1 | Five BUILD-2 forward-compat invariants for retype (BUILD-5) | Generation counter in inode (init=1); generation in FileCap; versioned discriminant enum; 16 reserved audit bytes in inode metadata; RETYPE bit (1<<16) in FileRights |

---

## 7. Anti-Scope Gates

These assertions prove BUILD-2 did not creep into later BUILD territory or introduce prohibited mechanisms. Each must be verifiable by code inspection or test output.

| Gate ID | Assertion | Verification Method |
|---|---|---|
| A-CAS | No content-addressing (CAS) introduced | No hash-as-address lookup path in inode tree; no content-hash index structure; no bloom filter |
| A-DEDUP | No deduplication mechanism | No reference-counted DDT; no duplicate block detection at allocation time |
| A-CACHE | No buffer-cache write-back | No page cache / dirty-buffer tracking; all writes go through virtio-blk directly |
| A-KTHREAD | No kthread primitive introduced | No `kernel/src/sched/kthread.rs` or equivalent; no background GC thread |
| A-AHCI | No AHCI driver | No `kernel/src/drivers/storage/ahci.rs` or PCI class 0x0106 probe |
| A-NVME | No NVMe driver | No `kernel/src/drivers/storage/nvme.rs` or PCI class 0x0108 probe |
| A-FOREIGN | No foreign FS (FAT32/ext4) support | No FAT32 parser; no ext4 superblock reader; no foreign-FS mount path |
| A-ROOTFS | No rootfs pivot | Limine modules remain the earliest root; NeoFS is mounted as additional filesystem, not replacing module-based userspace |
| A-CAS-GC | No mark-sweep or background GC | No garbage collection pass; no unreachable-block reclamation |

---

## 8. Stop Conditions

Do NOT claim BUILD-2 closed if any of the following hold:

- `docs/specs/neofs-format.md` not produced and PM-consumed;
- mkfs tool does not produce a deterministic image with reproducible hash;
- superblock validation fails to select highest-generation copy with valid checksum;
- NeoFS mount from virtio-blk fails or produces no usable root inode;
- native file create/read/write same-boot readback fails byte-level integrity check;
- two-boot persistence readback fails (data not durable across QEMU reboot);
- CoW write-after-write does not preserve old block (overwrite instead of copy);
- `sys_fsync` flushes global transaction instead of per-file intent log;
- Model inode allows `mmap(PROT_EXEC)` (type gate bypassed);
- Regular inode with MMAP_X right is rejected by type gate (false positive);
- `TypeForbidden` and `InsufficientRights` are not distinguishable error codes;
- FileCap generation check does not return `Stale` when inode generation is incremented;
- crash/recovery simulation shows torn state in committed data;
- corrupted superblock is silently accepted without checksum validation;
- any regression gate from §4 fails;
- any anti-scope gate from §7 is violated (CAS, dedup, buffer-cache, kthreads, AHCI, NVMe, foreign FS, rootfs pivot, or GC introduced);
- Cerebro format structural challenge (`SC-CEREOFMT`) has not been consumed by PM;
- Jarvis sub-commit plan (`SC-JARVISPLAN`) has not been ratified by PM.

---

## 9. Formatting Caveat (inherited from BUILD-1)

`rustfmt` is installed for the pinned nightly, but global `cargo fmt --check` fails on broad pre-existing formatting drift across the repo. Scoped rustfmt for new BUILD-2 source should pass. Global formatting is a non-blocking style debt carried forward from BUILD-1 (§7 of `Plans/CLOSE-REPORT-2026-05-12-phase-3-build-1.md`).

---

## 10. Forward Markers

### 10.1 Formatting/mechanical cleanup (optional)
Broad pre-existing format drift remains. A dedicated mechanical lane could address this separately from BUILD-2 correctness work.

### 10.2 BUILD-3 CAS + buffer cache + FLUSH (measurement-gated)
ADR-021 §BUILD-3 defines content-hash index, mark-sweep GC, virtio-blk FLUSH negotiation, and buffer-cache write-back. Entry gate requires measured dedup ratio justification or PM explicit authorization. Not scoped under BUILD-2.

### 10.3 BUILD-4 foreign FS read-only
FAT32/ext4 read-only, `sys_mount`/`sys_umount`, directory enumeration, cursor-based `sys_readdir`, broader POSIX-ish syscall surface. Default `noexec` on all foreign-FS mounts. Not scoped under BUILD-2.

### 10.4 BUILD-5 AHCI + NVMe + bare-metal validation
SATA AHCI driver, NVMe driver, real-hardware boot/storage verification. Not scoped under BUILD-2.

### 10.5 Docs/index/phase updates
Updating `docs/phases/PHASE-3.md` with BUILD-2 section, `Plans/INDEX.md` entries, and merging to `origin/main` require follow-on PM instruction after BUILD-2 closes.

---

## 11. Cross-References

| Artifact | Purpose |
|---|---|
| `Plans/DISPATCH-2026-05-12-phase-3-build-2-evidence-matrix-prometheus.md` | This dispatch mandate |
| `Plans/PM-DECISION-2026-05-12-phase-3-build-2-opening.md` | PM decision opening BUILD-2 PLAN/SPEC lane |
| `Plans/CLOSE-REPORT-2026-05-12-phase-3-build-1.md` | BUILD-1 close report (regression gate source) |
| `docs/ADR-021-phase-3-design-substrate.md` | Canonical Phase 3 design substrate with BUILD-2 scope and gates |
| `docs/phases/PHASE-3.md` | Phase 3 phase doc (BUILD-1 closed, BUILD-2 not started) |
| `Plans/RESEARCH-2026-05-12-phase-3-vfs-typed-inode-capability-cerebro.md` | Cerebro VFS/FileCap prior art research (typed inode, FileRights, error codes) |
| `Plans/RESEARCH-2026-05-12-phase-3-build-2-neofs-format-cerebro.md` | **NOT YET FILED** — Cerebro format structural challenge (slot reserved) |
| `Plans/OBSERVE-2026-05-12-phase-3-build-2-neofs-subcommit-plan-jarvis.md` | **NOT YET FILED** — Jarvis observe pass and sub-commit plan (slot reserved) |

---

**End of evidence matrix.**

*This is a PREP artifact — evidence slots and structure only. Does NOT claim BUILD-2 closure. No normative files were edited. No BUILD-2 implementation is authorized until PM consumes all three parallel artifacts (Cerebro format challenge, Jarvis observe plan, this evidence matrix).*