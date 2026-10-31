# CLOSE-REPORT — Phase 3 (NeoFS, Block Layer, Real Disk I/O) — Formal Close

**Date:** 2026-05-28
**Phase:** 3
**Status:** **CLOSED**
**Authority:** PM (Cerebro / Claude), on PL instruction 2026-05-28 following BUILD-5b bare-metal green
**Close ritual:** Per practiced precedent (Phase 2 close `ce2cba3`): a `plans: close phase N` commit landing the closeout artifacts + INDEX update. No history squash, no phase tag (neither is observed in repo practice; the only tag present is `backup-tag-pre-consolidation-2026-05-21`).

## Phase goal (Bible Article 13 / Part II Articles 9–10)

NeoFS, block layer, real disk I/O. Concretely: a generic block-device substrate; the native NeoFS filesystem (format, mount, COW, fsync/recovery, GC); VFS file/mount syscalls with typed inodes and capability gating; read-only foreign-FS interop (FAT32, ext4); real storage drivers (virtio-blk, AHCI, NVMe); and verification on real hardware.

## Build ledger — all closed

| Build | Delivered | Close artifact |
|-------|-----------|----------------|
| BUILD-1 | virtio-blk, VFS file syscalls, scheduler wake-token, integration closure; aarch64 barrier-lane feasibility | `CLOSE-REPORT-2026-05-12-phase-3-build-1.md` |
| BUILD-2 | NeoFS format + mkfs, read-only mount, COW + fsync + recovery, typed-inode file capabilities | `CLOSE-REPORT-2026-05-14-phase-3-build-2.md` |
| BUILD-3 | kthread lifecycle, virtio flush, NeoFS GC/free-space, writeback cache, durable fsync; full SMP sweep | `CLOSE-REPORT-2026-05-16-phase-3-build-3.md` |
| BUILD-4 (+4A–4D) | SMP liveness fixes (virtio-blk completion-worker deadlock; wake-token lock deadlock), directory/readdir, mount table + `SYS_MOUNT`/`SYS_UMOUNT` + live-handle `EBUSY`, FAT32 read-only, ext4 read-only | `PM-DISPOSITION-2026-05-20-phase-3-build-4-formal-closure-at-build-4D.md` |
| BUILD-5a | AHCI driver, NVMe driver | `PHASE-3-BUILD-5a-AHCI-CLOSE-EVIDENCE-2026-05-20.md`, `PHASE-3-BUILD-5a-NVMe-CLOSE-EVIDENCE-2026-05-20.md` |
| BUILD-5b | Bare-metal storage validation on Dell OptiPlex 7080 (AHCI + NVMe lanes, NeoFS mount/RW/fsync on real disk, log-sink extraction) | `PHASE-3-BUILD-5b-CLOSE-EVIDENCE-2026-05-28.md` |

## Verification (Bible Article 13 / 16.3)

- **QEMU:** all builds verified in QEMU across the development cycle; BUILD-5b added single-scenario `--bare-metal-mode` QEMU coverage for the AHCI/NVMe log-sink path (`87a49d1`).
- **Bare metal (capstone):** Dell OptiPlex 7080 Micro (Service Tag D4SHKD3). AHCI lane green (Samsung 860 EVO SATA): NeoFS create/write/fsync/readback, 9202-byte clean log extraction. NVMe lane green (Intel NVMe scenario disk + Samsung AHCI log sink): NeoFS create/write/fsync/readback (`bytes=30`), `NEO_LOG_v1` at `0x10000000` with valid header, 9191-byte clean extraction, no `.rodata` leak. Evidence: `PHASE-3-BUILD-5b-CLOSE-EVIDENCE-2026-05-28.md` and the `evidence/phase3-build5b-optiplex-7080-20260528-*` files.

## Notable incidents resolved at close (BUILD-5b campaign)

1. **APIC-base-not-initialized race** — storage probe enabled MSI/MSI-X before BSP APIC init. Fixed `72a0a41` (init BSP APIC before `block::init`). Bare-metal proven.
2. **AHCI log-sink DMA defect** — log-sink writes landed at the wrong LBA and over-DMA'd kernel `.rodata` to disk (confidentiality leak). Root cause: missing release publication fence before the AHCI `PX_CI` doorbell, so a real HBA fetched stale/unpublished command-table state (the in-memory FIS LBA and bounded PRDT were correct; they simply weren't published). Fixed `2fdb7cb` (release fence before doorbell, acquire on completion, matching the NVMe discipline). Bare-metal proven — the defect was invisible in QEMU, so bare metal was the arbiter. Full review trail: the two Round dispositions + the Round-2 HANDOFF (2026-05-28).

## Anti-scope respected

No training primitives, no AI subsystem work (Phase 5+), no rootfs pivot beyond mount-table scope, no foreign-FS write support, no broad POSIX expansion, no distributed/SSI storage. Storage stack is inference-agnostic substrate, consistent with the Bible.

## Deviations from plan

- BUILD-4 carried substantial unplanned SMP-liveness stabilization (two deadlocks root-caused and fixed) before its sub-builds could close — documented in the BUILD-4 stale-fsync findings series.
- BUILD-5b required two on-hardware bug fixes (APIC ordering, AHCI publish barrier) not anticipated at BUILD-5a close; both are now closed with bare-metal evidence.

## Follow-ups (NOT performed by this close; flagged for PL)

1. **Bible ledger amendment (PL-tier, Article 27):** `docs/bible.md` is stale — it lists Phase 2 as "Planned next" and Phase 3 with no status. An amendment should mark Phase 2 and Phase 3 complete and Phase 4 as next, with a version bump. Recommend folding Phase 2 + Phase 3 ledger updates into one amendment.
2. **Push:** the closeout commit stack is local-only pending explicit PL push authorization.
3. **Phase 4 (network stack)** is next per the ledger; THINK v0.2 already exists (`PHASE-4-THINK-v0.2-2026-05-22-...`). Opening Phase 4 PLAN is a separate PL decision.

---

**Phase 3 CLOSED.** The storage stack — block layer, NeoFS, VFS, foreign-FS interop, AHCI/NVMe drivers — is complete and verified on real hardware. Phases 1–3 (foundation) are done; the AI-native phases (5–7) remain the project's reason for being, with Phase 4 (network) intervening.
