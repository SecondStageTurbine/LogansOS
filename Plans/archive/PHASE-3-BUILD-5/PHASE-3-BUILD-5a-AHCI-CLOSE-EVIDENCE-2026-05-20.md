# Phase 3 BUILD-5a-AHCI Close Evidence

Date: 2026-05-20

Implementation commit: 3da656387c9deda71b06717d7d69873ec02bac6e

## Scope Delivered

BUILD-5a-AHCI landed a QEMU ICH9 AHCI block-device backend. The driver probes PCI mass-storage class 0x01/subclass 0x06/prog-if 0x01, maps BAR5 ABAR MMIO, initializes the first populated SATA port, issues ATA IDENTIFY DEVICE, and supports queue-depth-1 non-NCQ READ DMA EXT, WRITE DMA EXT, and FLUSH CACHE EXT.

The AHCI disk is attached by xtask as device3 behind `-device ich9-ahci,id=ahci0` plus `-device ide-hd,drive=hd3,bus=ahci0.0`. Existing NeoFS and FAT32 mounts compose through the existing `BlockDevice` and `MountedFs` abstractions.

## Container Preflight

Container-canonical QEMU version:

```text
docker compose run --rm neo bash -c 'qemu-system-x86_64 -version'
QEMU emulator version 7.2.22 (Debian 1:7.2+dfsg-7+deb12u18)
Copyright (c) 2003-2022 Fabrice Bellard and the QEMU Project developers
```

Container-canonical AHCI device availability:

```text
docker compose run --rm neo bash -c "qemu-system-x86_64 -device help 2>&1 | grep 'ich9-ahci'"
name "ich9-ahci", bus PCI, alias "ahci"
```

PCI enumeration is generic. `kernel/src/drivers/pci.rs` scans bus/device/function tuples, reads vendor/device/class/subclass/prog-if for every present function, and now dispatches AHCI by PCI class triplet. It is not hard-coded to virtio vendor IDs.

## Diff Stat

```text
3da6563 build-5a-AHCI: add AHCI block-device driver for QEMU ICH9 emulation
 kernel/Cargo.toml                |  10 +
 kernel/src/arch/x86_64/idt.rs    |  10 +-
 kernel/src/drivers/block/ahci.rs | 666 +++++++++++++++++++++++++++++++++++++++
 kernel/src/drivers/block/mod.rs  |   6 +-
 kernel/src/drivers/pci.rs        |  60 +++-
 kernel/src/fs/mod.rs             |  42 ++-
 kernel/src/test_harness.rs       | 433 ++++++++++++++++++++++++-
 tools/xtask/src/main.rs          | 131 +++++++-
 8 files changed, 1336 insertions(+), 22 deletions(-)
```

## Build Gates

```text
docker compose run --rm neo cargo fmt -- --check
PASS

docker compose run --rm neo cargo k
PASS - neoplutarch-kernel generated 52 warnings

docker compose run --rm neo cargo k --no-default-features
PASS - neoplutarch-kernel generated 52 warnings
```

The BUILD-4D warning baseline remained 52. No new unjustified default-build warnings landed.

## New AHCI Scenarios

Literal `NEO_TEST/1 event=PASS` lines:

```text
NEO_TEST/1 event=PASS name=phase3_ahci_block_device_registered registry_index=3 driver=ahci dispatch_read=ok block_devices=4
NEO_TEST/1 event=PASS name=phase3_ahci_controller_detected controller=ich9-ahci bdf=0x000600 abar_mapped=true ports_implemented=0x0000003f msi_configured=true
NEO_TEST/1 event=PASS name=phase3_ahci_fat32_mount_and_read fs=fat32 source=device3 readdir=ok path=/ahci-fat32/hello.txt bytes=18 content_match=true
NEO_TEST/1 event=PASS name=phase3_ahci_multi_sector_io lba=8 sectors=16 bytes=8192 pattern_match=true
NEO_TEST/1 event=PASS name=phase3_ahci_neofs_mount_and_readwrite fs=neofs source=device3 mount=rw initial_read=ok create_write_fsync_readback=ok bytes=38
NEO_TEST/1 event=PASS name=phase3_ahci_port_identify port=0 identify=ok sector_size=512 sectors=32768 msi_configured=true
NEO_TEST/1 event=PASS name=phase3_ahci_read_sector_pattern lba=8 bytes=512 pattern_match=true
NEO_TEST/1 event=PASS name=phase3_ahci_write_sector_pattern lba=32 bytes=512 write=ok flush=ok readback=ok
```

## Targeted Anti-Regressions

Literal `NEO_TEST/1 event=PASS` lines:

```text
NEO_TEST/1 event=PASS name=phase3_mount_neofs_device0_readonly mount=neofs fs_type=1 source=device0 target=neofs rc=0
NEO_TEST/1 event=PASS name=phase3_neofs_readdir_root neofs_readdir=root entries=3 order=mkfs cursor=3 eof=0 subdir_entries=0 subdir_eof=0 selected_superblock=0 generation=1 stale_generation_anchor=current_reachable_inode
NEO_TEST/1 event=PASS name=phase3_neofs_filecap_generation_stale path=/neofs/stale-2e.txt stale_cap=ESTALE stale_errno=-116 old_cap_generation=7 refreshed_generation=9 current_inode_generation=11 handle_a_continued=true reachable_generation_checked=true selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_filecap_stale_regression path=/neofs/fsync-3e-stale.txt stale_fsync=ESTALE stale_errno=-116 stale_before_durability=true old_cap_generation=7 refreshed_generation=9 current_inode_generation=9 dirty_blocks_before_stale=7 dirty_blocks_after_stale=7 dirty_blocks_after_refreshed_fsync=0 flush_commands_after_stale_delta=0 cache_flushes=2 selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_fat32_mount_readonly mount=fat32 fs_type=3 source=device1 target=fat32 readonly=true
NEO_TEST/1 event=PASS name=phase3_fat32_readdir_root entries=5 hello=Regular readme=Regular subdir=Directory lfn=Regular neom=Regular
NEO_TEST/1 event=PASS name=phase3_fat32_open_read_regular_file path=/fat32/hello.txt bytes=18 content_match=true
NEO_TEST/1 event=PASS name=phase3_fat32_lfn_lookup lfn_lookup=case_preserving uppercase_lookup=case_insensitive
NEO_TEST/1 event=PASS name=phase3_fat32_type_import_is_regular neom_kind=Regular model_imports=0 symlink_imports=0 entries=5
NEO_TEST/1 event=PASS name=phase3_ext4_mount_readonly mount=ext4 fs_type=4 source=device2 target=ext4 readonly=true
NEO_TEST/1 event=PASS name=phase3_ext4_readdir_root entries=7 hello=Regular readme=Regular subdir=Directory large=Regular neom=Regular symlink_skipped=true
NEO_TEST/1 event=PASS name=phase3_ext4_extent_chain_read path=/ext4/large.bin bytes=1048576 extents=50 pattern_match=true
NEO_TEST/1 event=PASS name=phase3_ext4_type_import_is_regular neom_kind=Regular xattr_type_ignored=true model_imports=0 symlink_imports=0 entries=7
NEO_TEST/1 event=PASS name=phase3_sched_wake_token_abandon token_cpu=0 token_slot=0 generation=1 late_wake=Gone abandon=zombified_and_reaped
NEO_TEST/1 event=PASS name=phase3_vfs_directory_read_is_eisdir vfs_user_scenario=ok exit_code=0
NEO_TEST/1 event=PASS name=ipc_heap_kill_smoke leak_audit=clean exit_code=0
NEO_TEST/1 event=PASS name=cap_revoke_local cap_scenario=ok exit_code=0
```

## Final Close Gate

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
PASS - command exited 0
```

## AHCI Implementation Summary

PCI enumeration:

- `StorageKind::AhciForward` now probes AHCI controllers instead of only logging a forward marker.
- The match is by PCI storage class/subclass/prog-if, not by virtio IDs.
- QEMU q35 also exposes an ICH9 AHCI function at `00:1f.2`; BUILD-5a scope is one populated controller/port, so the driver keeps the first successful device-backed controller and ignores later no-disk controllers.

HBA and port initialization:

- BAR5 ABAR is mapped with the existing MMIO page mapping helper.
- PCI memory space and bus mastering are enabled.
- GHC.AE is set before port initialization.
- Ports are enumerated from HBA PI.
- Candidate ports require SSTS DET=present, IPM=active, and SATA ATA signature.
- Port initialization stops CMD.ST/FRE, waits for CR/FR to clear, clears PxSERR/PxIS, programs PxCLB and PxFB, restarts FRE/ST, runs IDENTIFY DEVICE, then enables PxIE/GHC.IE if MSI configured.

Command flow:

- Queue depth is one command slot.
- Command list, FIS receive area, and command table are allocated through the existing `DmaBuffer` allocator.
- PRDT supports up to eight DMA buffers.
- ATA commands implemented: IDENTIFY DEVICE 0xec, READ DMA EXT 0x25, WRITE DMA EXT 0x35, FLUSH CACHE EXT 0xea.
- 48-bit LBA fields are populated unconditionally for DMA EXT commands.

## MSI / IRQ Disposition

MSI is used. `pci::configure_msi_fixed_vector` programs the AHCI MSI capability to fixed LAPIC destination address `0xfee00000` and vector `0xf0`, and `kernel/src/arch/x86_64/idt.rs` installs an AHCI MSI handler at vector 240.

The interrupt handler acknowledges HBA and port interrupt status and ends the LAPIC interrupt. Command submission remains synchronous and queue-depth-1: it waits on PxCI/PxIS/PxTFD rather than adding a scheduler wake-token rendezvous for AHCI. This was a deliberate BUILD-5a simplification because the public `BlockDevice` API remains synchronous and all required composition tests pass through that API. A future NCQ or asynchronous block API would be the trigger for a fuller AHCI completion queue/waiter design.

No legacy IRQ path landed.

## Block Registry Disposition

No `MountedFs` change was required. No filesystem driver change was required.

The existing block registry was already generic (`Arc<dyn BlockDevice>` behind `drivers::block::register_device/get_device`), so a `VIRTIO_DEVICES` rename was not needed in this build. AHCI ports register through the same registry as virtio-blk and appear as device index 3 in the AHCI scenarios.

Filesystem source routing changed only at the VFS mount-dispatch layer:

- NeoFS fs_type 1/2 accepts `device3` in addition to device0.
- FAT32 fs_type 3 accepts `device3` in addition to device1.
- ext4 fs_type 4 remains device2 only.

The VFS `OPEN_CREATE_REGULAR` gate was also generalized from a hard-coded `/neofs/...` prefix to "any mounted root child"; this is what allows NeoFS mounted as `/ahci-neofs` to create files while preserving the old bare-root `Invalid` behavior. This is not a `MountedFs` trait change and not a NeoFS driver change.

## Lock Hygiene

New lock site: `kernel/src/drivers/block/ahci.rs` `AHCI_DEVICES`.

- Lock type: `SwitchMutex<Vec<Arc<AhciDevice>>>`.
- IF discipline location: declaration-site comment above `AHCI_DEVICES`.
- Invariant: mutations happen during PCI probe before scenario I/O; MSI handler may read the list with IF=0 after interrupt-gate entry; handler never takes a per-port lock.

New lock site: `kernel/src/drivers/block/ahci.rs` `AhciDevice.inner`.

- Lock type: `SwitchMutex<AhciPortInner>`.
- IF discipline location: declaration-site comment above `inner`.
- Invariant: submitters may hold with IF=1 while one non-NCQ command is in flight; MSI handler does not take the lock and only acknowledges immutable MMIO register locations.

No new lock site was introduced in filesystem drivers or the `MountedFs` abstraction.

## Explicit Confirmations

AHCI implementation did not require modifications to the `MountedFs` trait.

AHCI implementation did not require modifications to existing FAT32/ext4/NeoFS drivers.

No NVMe code landed.

No bare-metal-specific code landed.

No NCQ, Port Multiplier, hot-plug, TRIM, SMART, power-management, ATAPI, or legacy PATA support landed.

No AHCI-specific userspace syscall ABI landed.

No BUILD-3F CAS/dedup work landed.

## Deviations / Course Corrections

- QEMU q35 exposes two ICH9 AHCI functions in this harness shape: the explicitly attached `00:06.0` controller with the test disk and the built-in `00:1f.2` controller without the BUILD-5a disk. The probe snapshot initially got overwritten by the later no-disk function; the driver now preserves the first successful populated controller, matching BUILD-5a single-controller scope.
- The NeoFS-on-AHCI scenario exposed a VFS name coupling: CREATE_REGULAR accepted only `/neofs/...`. The fix generalized VFS create routing to any mounted root child, enabling `/ahci-neofs/...` without touching the NeoFS driver or `MountedFs` trait.
- MSI routing is installed and the handler acknowledges AHCI interrupts, but command completion remains synchronous polling over the AHCI command issue register for BUILD-5a queue-depth-1 operation.

## Stop Conditions

No stop condition remained open at close.
