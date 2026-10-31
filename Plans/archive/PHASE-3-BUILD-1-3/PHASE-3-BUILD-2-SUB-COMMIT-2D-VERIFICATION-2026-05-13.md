# Phase 3 BUILD-2 Sub-Commit 2D Verification

Date: 2026-05-13
Branch: `codex/phase-3-build-2-2d-cow-fsync-recovery`
Base: `d2c8810`
Scope: NeoFS CoW ordering, recovery-safe mount selection, deterministic crash/reboot scenarios, and `SYS_FSYNC = 15` as a BUILD-2 no-op.

The final commit hash is reported by Git for the commit containing this artifact; embedding that object hash in the artifact would make it self-referential.

## New 2D Runtime Gates

Command:

```text
docker compose run --rm neo cargo neo-test phase3_neofs_fsync_noop_write_through
docker compose run --rm neo cargo neo-test phase3_neofs_cow_write_after_write phase3_neofs_crash_mid_overwrite_recovery phase3_neofs_superblock_rotation_recovery phase3_neofs_crash_mid_create_recovery
```

PASS frames:

```text
target/neo-test/phase3_neofs_fsync_noop_write_through.serial.log
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_noop_write_through neofs_fsync=noop path=/neofs/fsync-2d.txt bytes=53 fsync_rc=0 generation_before=3 generation_after=3 write_through=true no_flush_command=true no_intent_log=true selected_superblock=0 generation=1

target/neo-test/phase3_neofs_cow_write_after_write.serial.log
NEO_TEST/1 event=PASS name=phase3_neofs_cow_write_after_write neofs_cow=ok path=/neofs/cow-2d.txt first_bytes=51 second_bytes=51 old_data_block=16395 new_data_block=16399 distinct_blocks=true selected_superblock=0 generation=1

target/neo-test/phase3_neofs_crash_mid_overwrite_recovery.boot1.serial.log
NEO_TEST/1 event=PASS name=phase3_neofs_crash_mid_overwrite_recovery stage=write neofs_recovery=prepared path=/neofs/crash-ow.txt committed=version_a attempted=version_b injection=before_superblock_publish selected_superblock=0 generation=1

target/neo-test/phase3_neofs_crash_mid_overwrite_recovery.boot2.serial.log
NEO_TEST/1 event=PASS name=phase3_neofs_crash_mid_overwrite_recovery stage=readback neofs_recovery=ok path=/neofs/crash-ow.txt recovered=version_a unpublished_version_b_absent=true selected_superblock=0 generation=3 rejected_superblock_mask=0x00

target/neo-test/phase3_neofs_superblock_rotation_recovery.boot1.serial.log
NEO_TEST/1 event=PASS name=phase3_neofs_superblock_rotation_recovery stage=write neofs_superblock_recovery=prepared path=/neofs/sb-rot.txt invalid_slot=2 injection=root_invalid_superblock selected_superblock=0 generation=1

target/neo-test/phase3_neofs_superblock_rotation_recovery.boot2.serial.log
NEO_TEST/1 event=PASS name=phase3_neofs_superblock_rotation_recovery stage=readback neofs_superblock_recovery=ok path=/neofs/sb-rot.txt selected_superblock=0 generation=3 invalid_slot=2 invalid_candidate_rejected=true rejected_superblock_mask=0x04

target/neo-test/phase3_neofs_crash_mid_create_recovery.boot1.serial.log
NEO_TEST/1 event=PASS name=phase3_neofs_crash_mid_create_recovery stage=write neofs_create_recovery=prepared target=/neofs/crash-create.txt marker=/neofs/create-marker-2d.txt injection=before_superblock_publish expected_recovery=target_absent selected_superblock=0 generation=1

target/neo-test/phase3_neofs_crash_mid_create_recovery.boot2.serial.log
NEO_TEST/1 event=PASS name=phase3_neofs_crash_mid_create_recovery stage=readback neofs_create_recovery=ok target=/neofs/crash-create.txt marker=/neofs/create-marker-2d.txt target_absent=true selected_superblock=0 generation=3 rejected_superblock_mask=0x00
```

Crash/recovery outcomes:

- Mid-overwrite injection stops before superblock publish. Reboot selects the last committed root and recovers version A.
- Superblock rotation writes a newer but root-invalid superblock copy in slot 2. Reboot rejects slot 2 and selects the older fully mountable generation.
- Mid-create injection stops before superblock publish. A committed marker distinguishes boot 2, and the target file is absent rather than partially visible.

## Regression Gates

Command:

```text
docker compose run --rm neo cargo neo-test phase3_neofs_create_small_file phase3_neofs_write_readback_same_boot phase3_neofs_allocator_reuse_guard phase3_neofs_reboot_persistence_small_file phase3_neofs_mount_readonly_root phase3_neofs_mount_selects_highest_generation phase3_neofs_mount_rejects_bad_superblock phase3_block_vfs_smoke phase3_virtio_blk_raw_rw_smoke
```

PASS frames were recorded in `target/neo-test/*.serial.log` for all nine requested scenarios, including:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_create_small_file neofs_create=ok create_flag=OPEN_CREATE_REGULAR path=/neofs/create-2c.txt bytes=40 close_reopen_read=ok selected_superblock=0 generation=1 no_directory_create=true no_model_create=true
NEO_TEST/1 event=PASS name=phase3_neofs_write_readback_same_boot neofs_write_readback=ok path=/neofs/direct8.bin bytes=32768 direct_blocks=8 close_reopen_read=ok selected_superblock=0 generation=1 runtime_mutation=true
NEO_TEST/1 event=PASS name=phase3_neofs_allocator_reuse_guard neofs_allocator_reuse_guard=ok first_path=/neofs/alloc-a.bin second_path=/neofs/alloc-b.bin alias_check=distinct_content bump_allocator=true selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_reboot_persistence_small_file stage=readback neofs_reboot_persistence=ok path=/neofs/persist-2c.txt bytes=50 selected_superblock=0 generation=3 image=reused
NEO_TEST/1 event=PASS name=phase3_neofs_mount_readonly_root neofs_mount=ok selected_superblock=0 generation=1 valid_superblocks=3 rejected_superblocks=0 root_kind=Directory hello_read=ok model_kind=Model readonly_write_denied=InsufficientRights runtime_mutation=false
NEO_TEST/1 event=PASS name=phase3_neofs_mount_selects_highest_generation neofs_mount=ok selected_superblock=2 generation=3 highest_generation_selected=true valid_superblocks=3 rejected_superblocks=0 runtime_mutation=false
NEO_TEST/1 event=PASS name=phase3_neofs_mount_rejects_bad_superblock neofs_mount=ok selected_superblock=1 generation=1 corrupt_primary_rejected=true rejected_superblock_mask=0x01 valid_superblocks=2 rejected_superblocks=1 runtime_mutation=false
NEO_TEST/1 event=PASS name=phase3_block_vfs_smoke bdf=0x000300 block_vfs_same_boot=true raw_rw=ok vfs_syscalls=ok stdout_namespace=fd1_serial lba=3 block_size=512 completions_delta=2 descriptor_publish_barriers=2 notification_suppression_barriers=2 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false
NEO_TEST/1 event=PASS name=phase3_virtio_blk_raw_rw_smoke bdf=0x000300 block_size=512 lba=1 completions=2 descriptor_publish_barriers=2 notification_suppression_barriers=2 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false
```

`phase3_aarch64_virtio_barrier_lane` was not rerun because 2D did not modify virtio, barrier, or block-driver files.

The optional SMP sweep was not run; the dispatch hard new-scenario and regression matrix passed.

## Build And Tooling

PASS:

```text
cargo check -p xtask
cargo check -p neofs-mkfs
cargo check -p libneo -p neofs-mkfs
cargo k
cargo k --no-default-features
cargo check -p neoplutarch-kernel --features neo-test-phase3-neofs-fsync-noop-write-through
cargo check -p neoplutarch-kernel --features neo-test-phase3-neofs-cow-write-after-write
cargo check -p neoplutarch-kernel --features neo-test-phase3-neofs-crash-mid-overwrite-recovery
cargo check -p neoplutarch-kernel --features neo-test-phase3-neofs-superblock-rotation-recovery
cargo check -p neoplutarch-kernel --features neo-test-phase3-neofs-crash-mid-create-recovery
git diff --check
git diff --cached --check
```

Scoped rustfmt check was run:

```text
rustfmt --edition 2021 --check kernel/src/arch/x86_64/syscall.rs kernel/src/fs/mod.rs kernel/src/fs/neofs/inode.rs kernel/src/fs/neofs/mount.rs kernel/src/test_harness.rs tools/xtask/src/main.rs userland/libneo/src/lib.rs
```

It reported pre-existing style drift in touched long-lived files, especially `kernel/src/test_harness.rs` and `userland/libneo/src/lib.rs`. No broad rustfmt rewrite was applied to avoid unrelated churn; the 2D-owned formatting nits identified in the NeoFS/debug helper code were adjusted manually. `git diff --check` passed.

## Scope Notes

- `SYS_FSYNC = 15` is documented and wired as a BUILD-2 no-op over synchronous write-through I/O.
- No intent log was added; recovery evidence uses CoW ordering and superblock/root candidate validation only.
- No BUILD-2 2E typed/FileCap generation work was started.
