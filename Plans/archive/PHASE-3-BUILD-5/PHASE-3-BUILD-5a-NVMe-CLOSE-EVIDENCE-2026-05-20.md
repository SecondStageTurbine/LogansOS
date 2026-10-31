# Phase 3 BUILD-5a-NVMe Close Evidence

Date: 2026-05-20
Dispatch: `Plans/DISPATCH-2026-05-20-phase-3-build-5a-NVMe-driver-jarvis.md`

## Commits

- Implementation: `eba73ac build-5a-NVMe: add NVMe block-device driver for QEMU NVMe emulation`
- Close evidence: this follow-up close-evidence commit

Implementation diffstat:

```text
 kernel/Cargo.toml                |  10 +
 kernel/src/arch/x86_64/idt.rs    |   8 +-
 kernel/src/drivers/block/mod.rs  |   4 +-
 kernel/src/drivers/block/nvme.rs | 723 +++++++++++++++++++++++++++++++++++++++
 kernel/src/drivers/pci.rs        |  49 ++-
 kernel/src/fs/mod.rs             |   2 +
 kernel/src/test_harness.rs       | 449 +++++++++++++++++++++++-
 tools/xtask/src/main.rs          | 120 +++++++
 8 files changed, 1360 insertions(+), 5 deletions(-)
```

## Container-Canonical Preflight

Command:

```bash
docker compose run --rm neo bash -lc "qemu-system-x86_64 -device help 2>&1 | grep -E '^name .nvme'"
```

Output:

```text
name "nvme", bus PCI, desc "Non-Volatile Memory Express"
name "nvme-ns", bus nvme-bus, desc "Virtual NVMe namespace"
name "nvme-subsys", desc "Virtual NVMe subsystem"
```

Registry check before implementation confirmed the existing block registry API shape remained `register_device` / `get_device`; the NVMe driver plugs into it without replacing the registry.

## Implementation Summary

- Added `kernel/src/drivers/block/nvme.rs` as a QEMU PCI NVMe backend.
- PCI probe matches class `0x01`, subclass `0x08`, prog-if `0x02`.
- Maps BAR0, validates NVM command set support, derives doorbell stride from CAP.DSTRD, and requires 4 KiB page support for this build.
- Creates one admin queue pair and one I/O queue pair; no NCQ-equivalent parallel queueing or multipath.
- Identifies controller and namespace 1, requires 512-byte LBA size, and registers `nvme0n1` at block device index 4.
- Implements synchronous read/write/flush through existing `BlockDevice`.
- Implements PRP-only data transfer: one physically contiguous `DmaBuffer` per request, PRP1/PRP2 for one or two pages, and a one-page PRP list for larger single-buffer requests. SGL and multi-buffer scatter are rejected as unsupported.
- Extends QEMU test wiring so NVMe scenarios attach device4 with `-device nvme,drive=hd4,serial=neoplutarch-nvme`.
- Extends VFS mount source routing only so NeoFS/FAT32 can mount `device4`; `MountedFs` was not modified.

## Interrupts And Ordering

- NVMe uses MSI-X first and falls back to MSI. QEMU NVMe exposed MSI-X in this environment.
- Observed scenario evidence: `msix_configured=true msi_configured=false`.
- Completion is still polled synchronously; the interrupt handler only increments a diagnostic counter and does not advance queue heads.
- Course correction: enabling MSI-X before admin initialization let QEMU deliver an early admin completion interrupt before local APIC initialization, causing an APIC-base assertion. Final code defers MSI-X/MSI enablement until after admin identify and I/O queue creation. This preserves the practical BUILD-5b ordering rule: do not allow storage IRQ delivery before APIC setup can service it.

## Scope And Substrate Validation

- `MountedFs` trait unchanged. It remains declared at `kernel/src/fs/mod.rs:337`.
- Existing FAT32, ext4, NeoFS, AHCI, and virtio-blk drivers were not modified.
- Block-device registry shape unchanged; NVMe uses `block::register_device` like AHCI/virtio.
- No NVMe-specific userspace syscall ABI added.
- No bare-metal quirk table, NVMe-oF, namespace management, SED, SGL, PMR/CMB, Dataset Management, SMART, power management, ZNS, multipath, or hotplug work was added.
- AHCI implementation files were not changed by this build.

## Lock Hygiene

New lock sites:

- `kernel/src/drivers/block/nvme.rs:133`:
  `NVME_DEVICES: Lazy<SwitchMutex<Vec<Arc<NvmeNamespaceDevice>>>>`.
  Lock type and IF discipline are documented at the declaration. Mutation happens during PCI probe; the MSI/MSI-X handler intentionally does not take this lock.
- `kernel/src/drivers/block/nvme.rs:362`:
  `NvmeNamespaceDevice.inner: SwitchMutex<NvmeControllerInner>`.
  Lock type and IF discipline are documented at the field. Synchronous submitters hold it while one queue command is in flight; the interrupt handler does not take it.

## Build Gates

Commands:

```bash
docker compose run --rm neo cargo fmt -- --check
docker compose run --rm neo cargo k
docker compose run --rm neo cargo k --no-default-features
```

Results:

- `cargo fmt -- --check`: pass.
- `cargo k`: pass; kernel build emitted the existing 52 warnings.
- `cargo k --no-default-features`: pass; kernel build emitted the existing 52 warnings.

## New NVMe Scenario Evidence

All eight required BUILD-5a-NVMe scenarios passed:

```text
NEO_TEST/1 event=PASS name=phase3_nvme_controller_detected controller=qemu-nvme bdf=0x000700 bar0_mapped=true admin_queue=ready io_queue=ready controller_ready=true msix_configured=true msi_configured=false
NEO_TEST/1 event=PASS name=phase3_nvme_namespace_identify identify_controller=ok identify_namespace=ok nsid=1 lba_format=0 sector_size=512 sectors=32768 doorbell_stride=4
NEO_TEST/1 event=PASS name=phase3_nvme_read_sector_pattern lba=8 bytes=512 pattern_match=true
NEO_TEST/1 event=PASS name=phase3_nvme_write_sector_pattern lba=32 bytes=512 write=ok flush=ok readback=ok
NEO_TEST/1 event=PASS name=phase3_nvme_multi_sector_io lba=8 sectors=16 bytes=8192 pattern_match=true
NEO_TEST/1 event=PASS name=phase3_nvme_block_device_registered registry_index=4 driver=nvme dispatch_read=ok block_devices=5
NEO_TEST/1 event=PASS name=phase3_nvme_fat32_mount_and_read fs=fat32 source=device4 readdir=ok path=/nvme-fat32/hello.txt bytes=18 content_match=true
NEO_TEST/1 event=PASS name=phase3_nvme_neofs_mount_and_readwrite fs=neofs source=device4 mount=rw initial_read=ok create_write_fsync_readback=ok bytes=38
```

## Targeted Anti-Regression Evidence

Focused anti-regression batch exit status was 0. Representative PASS lines:

```text
NEO_TEST/1 event=PASS name=phase3_mount_neofs_device0_readonly mount=neofs fs_type=1 source=device0 target=neofs rc=0
NEO_TEST/1 event=PASS name=phase3_neofs_readdir_root neofs_readdir=root entries=3 order=mkfs cursor=3 eof=0 subdir_entries=0 subdir_eof=0 selected_superblock=0 generation=1 stale_generation_anchor=current_reachable_inode
NEO_TEST/1 event=PASS name=phase3_neofs_filecap_generation_stale path=/neofs/stale-2e.txt stale_cap=ESTALE stale_errno=-116 old_cap_generation=7 refreshed_generation=9 current_inode_generation=11 handle_a_continued=true reachable_generation_checked=true selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_filecap_stale_regression path=/neofs/fsync-3e-stale.txt stale_fsync=ESTALE stale_errno=-116 stale_before_durability=true old_cap_generation=7 refreshed_generation=9 current_inode_generation=9 dirty_blocks_before_stale=7 dirty_blocks_after_stale=7 dirty_blocks_after_refreshed_fsync=0 flush_commands_after_stale_delta=0 cache_flushes=2 selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_fat32_type_import_is_regular neom_kind=Regular model_imports=0 symlink_imports=0 entries=5
NEO_TEST/1 event=PASS name=phase3_ext4_type_import_is_regular neom_kind=Regular xattr_type_ignored=true model_imports=0 symlink_imports=0 entries=7
NEO_TEST/1 event=PASS name=phase3_ahci_controller_detected controller=ich9-ahci bdf=0x000600 abar_mapped=true ports_implemented=0x0000003f msi_configured=true
NEO_TEST/1 event=PASS name=phase3_ahci_port_identify port=0 identify=ok sector_size=512 sectors=32768 msi_configured=true
NEO_TEST/1 event=PASS name=phase3_ahci_read_sector_pattern lba=8 bytes=512 pattern_match=true
NEO_TEST/1 event=PASS name=phase3_ahci_write_sector_pattern lba=32 bytes=512 write=ok flush=ok readback=ok
NEO_TEST/1 event=PASS name=phase3_ahci_multi_sector_io lba=8 sectors=16 bytes=8192 pattern_match=true
NEO_TEST/1 event=PASS name=phase3_ahci_block_device_registered registry_index=3 driver=ahci dispatch_read=ok block_devices=4
NEO_TEST/1 event=PASS name=phase3_ahci_fat32_mount_and_read fs=fat32 source=device3 readdir=ok path=/ahci-fat32/hello.txt bytes=18 content_match=true
NEO_TEST/1 event=PASS name=phase3_ahci_neofs_mount_and_readwrite fs=neofs source=device3 mount=rw initial_read=ok create_write_fsync_readback=ok bytes=38
NEO_TEST/1 event=PASS name=phase3_sched_wake_token_abandon token_cpu=0 token_slot=0 generation=1 late_wake=Gone abandon=zombified_and_reaped
NEO_TEST/1 event=PASS name=phase3_vfs_directory_read_is_eisdir vfs_user_scenario=ok exit_code=0
NEO_TEST/1 event=PASS name=ipc_heap_kill_smoke leak_audit=clean exit_code=0
NEO_TEST/1 event=PASS name=cap_revoke_local cap_scenario=ok exit_code=0
```

## Full No-Meta Gate

Command:

```bash
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Result:

- The detached Docker process completed and the `--rm` container was removed.
- Final captured scenario:

```text
[xtask] neo-test: `phase3_aarch64_virtio_barrier_lane` PASS
```

- NVMe scenarios inside this full run:

```text
[xtask] neo-test: `phase3_nvme_controller_detected` PASS
[xtask] neo-test: `phase3_nvme_namespace_identify` PASS
[xtask] neo-test: `phase3_nvme_read_sector_pattern` PASS
[xtask] neo-test: `phase3_nvme_write_sector_pattern` PASS
[xtask] neo-test: `phase3_nvme_multi_sector_io` PASS
[xtask] neo-test: `phase3_nvme_block_device_registered` PASS
[xtask] neo-test: `phase3_nvme_fat32_mount_and_read` PASS
[xtask] neo-test: `phase3_nvme_neofs_mount_and_readwrite` PASS
```

- Failure scan found no `NEO_TEST/1 event=FAIL`, kernel panic, or panic lines. The only failure-shaped line was the existing retry path:

```text
[xtask] neo-test: `phase3_storage_completion_cross_cpu_wake` attempt 1/2 failed: `phase3_storage_completion_cross_cpu_wake` did not exit via debug success; status=exit status: 35 serial=/workspace/target/neo-test/phase3_storage_completion_cross_cpu_wake.attempt1.serial.log
[xtask] neo-test: `phase3_storage_completion_cross_cpu_wake` PASS after retry attempt 2/2
```

## Stop Conditions

No stop condition remains open.

- QEMU NVMe emulation was present in the container.
- Mounted filesystem abstraction accepted the fourth backend without trait changes.
- Existing filesystem drivers did not need changes.
- Existing AHCI driver did not need changes.
- Block registry accepted NVMe through the same plugin point as AHCI and virtio.
