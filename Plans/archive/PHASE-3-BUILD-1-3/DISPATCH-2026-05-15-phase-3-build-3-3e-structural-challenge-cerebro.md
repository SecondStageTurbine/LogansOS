# Dispatch - Phase 3 BUILD-3E Structural Challenge

Date: 2026-05-15
Agent: Cerebro
Status: AUTHORIZED SIDEcar
Authority:
- `Plans/PM-DISPOSITION-2026-05-15-phase-3-build-3-3d-consumption-and-3e-authorization.md`
- `Plans/DISPATCH-2026-05-15-phase-3-build-3-3e-durable-fsync-jarvis.md`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3D-VERIFICATION-2026-05-14.md`

---

## 1. Mission

Run a structural challenge of BUILD-3E before Jarvis closes durable NeoFS
`sys_fsync`.

This is not implementation. Do not edit kernel code. Produce a challenge note
that Jarvis and PM can consume while 3E is in flight.

## 2. Questions To Answer

Focus on the actual 3D substrate and 3E dispatch. Answer these questions:

1. Does a mount-wide cache drain satisfy 3E durability semantics, or does the
   current VFS/FileCap shape require per-file dirty attribution before fsync can
   be honestly called durable?

2. What is the narrowest correct ordering for `sys_fsync(fd)` over the 3D cache:
   data, inode/root/free-map metadata, superblock copies, FLUSH, and return?

3. Where can fsync race the background `neofs-writeback` worker, and what
   locking/pause discipline is required to avoid torn publication?

4. Where can fsync race GC, especially with dirty unpublished blocks and
   free-map/live-set selection?

5. Can stale FileCaps reach fsync after CoW generation changes? Confirm the
   check must compare against current reachable inode generation, not the old
   inode block.

6. What does clean-file fsync need to prove so it avoids spurious FLUSH without
   weakening dirty fsync?

7. What crash/reboot scenarios are sufficient to distinguish:
   - fsync'd data survives;
   - unfsync'd dirty data may be lost;
   - old committed state remains mountable and not torn?

8. Does the 3D virtio completion-ordering fix create any new evidence burden
   for 3E's FLUSH command/completion checks?

## 3. Required Output

Create:

```text
Plans/RESEARCH-2026-05-15-phase-3-build-3-3e-durable-fsync-cerebro.md
```

The note must include:

- recommended fsync implementation posture;
- must-fix hazards before 3E can close;
- acceptable simplifications for 3E;
- unacceptable shortcuts;
- required scenario/evidence matrix;
- explicit advice on mount-wide drain vs per-file drain;
- any blocker recommendation if durable fsync cannot be honestly implemented
  on top of the 3D substrate.

## 4. Boundaries

Do not start CAS/dedup, AHCI/NVMe, foreign filesystem work, rootfs pivot,
mmap/exec, broad POSIX semantics, BUILD-4, or implementation patches.

