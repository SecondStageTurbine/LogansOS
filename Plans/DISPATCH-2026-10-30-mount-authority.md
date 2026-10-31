<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH — a non-delegable mount authority (J.c. decision, 2026-10-30, option A)

**Seat:** A. **Why:** Dexter #187 item 3a (`Plans/RETURN-2026-10-30-login-recheck-dexter.md`).
- In the product image any process can `SYS_OPEN("/", MOUNT)`, then `SYS_UMOUNT(root_fd, "neofs")` whenever no handles
  are open (`fs/mod.rs:768`), then `SYS_MOUNT` a different volume.
- Today that is a lockout and namespace DoS. Once a USB mass-storage driver lands, a foreign volume can carry its own
  `.system/` record and the lock would check it.

**J.c.'s decision:** mounting and unmounting need a dedicated, non-delegable authority, shaped like #224's `system-store`
(`SystemStore` cap: minted only into neo-init, rights without DERIVE/GRANT, refused by derive, grant, spawn and every
IPC transfer).

## Work (one PR off `origin/main`; rebase after #224 merges, since #224 adds `system-store` and you'll share its pattern)

1. **A `MountAuthority` cap** (name it as you see fit), minted once at boot into the process that legitimately mounts:
   neo-init, and the installer flow if it mounts. Check the installer and the boot path, and list every current
   `SYS_MOUNT`/`SYS_UMOUNT` caller. It has no DERIVE or GRANT, and derive, grant, spawn and every IPC lookup refuse it.
2. **`SYS_MOUNT` and `SYS_UMOUNT` require it**, in addition to (or instead of) the directory handle's `MOUNT` right.
   `MOUNT`/`MOUNT_RESERVED` on a plain root open stops being enough; mask it out of what a plain open may request, as
   Dexter suggests.
3. **Kill-switch:** a feature, like `system-store`, in the product image's features with a pairing assert. Off means
   today's behaviour, bit for bit.
4. **Scenario:** a non-authority process's `SYS_UMOUNT` of `neofs` and its `SYS_MOUNT` of a device both get EPERM,
   and the volume is still mounted afterwards. The authority holder can still mount and unmount. Falsify it with the
   check reverted.
5. **Prior-art section** (CLAUDE.md): which CP rows the change touches (it's a capability-graph and syscall-ABI
   change), and the byte or ABI note if the syscall ABI changes.

## Close condition

- `scripts/worker-close-gate.sh` passes; a PR comment starting `RETURN (seat A):` with the head SHA and results.
- Don't merge. The PM gates it, and Dexter does the security re-check (it's Claude-authored security code).
- Commit and push at every checkpoint. No model identifiers.
