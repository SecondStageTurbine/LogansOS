# Phase 3 BUILD-2 Sub-Commit 2E Verification

Date: 2026-05-13
Owner: Jarvis
Branch: `codex/phase-3-build-2-2e-typed-inode-filecap`
Base: `a2f1e49a36c08318a7e21fd41fe54fe6ca8ecafe`
Scope: typed-inode executable policy, `FileRights` execute-bit activation, and FileCap generation stale-handle rejection.

The final commit hash is reported by Git for the commit containing this artifact; embedding that object hash in this file would make the commit self-referential.

## Verdict

`2E PASS / BUILD-2 close pending clean full non-explicit SMP sweep`

Only BUILD-2 sub-commit 2E was implemented. No BUILD-3 work, close report, real mmap/exec, disk exec, retype, GC, CAS, or FileCap grant/derive/revoke semantics were started.

## New 2E Runtime Gates

Commands:

```text
docker compose run --rm neo cargo neo-test phase3_neofs_typed_inode_exec_refusal phase3_neofs_regular_exec_policy_stub phase3_neofs_type_vs_rights_policy_errors phase3_neofs_filecap_generation_stale phase3_neofs_reserved_type_refusal phase3_neofs_filerights_u32_forward_bits
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_neofs_typed_inode_exec_refusal phase3_neofs_regular_exec_policy_stub phase3_neofs_type_vs_rights_policy_errors phase3_neofs_filecap_generation_stale phase3_neofs_reserved_type_refusal phase3_neofs_filerights_u32_forward_bits
```

PASS frames from the final `NEO_SMP=4` run:

```text
target/neo-test/phase3_neofs_typed_inode_exec_refusal.serial.log
NEO_TEST/1 event=PASS name=phase3_neofs_typed_inode_exec_refusal model_read=ok read_bytes=54 forced_execute_cap=true exec_policy=TypeForbidden type_first=true selected_superblock=0 generation=1 no_real_mmap_exec=true

target/neo-test/phase3_neofs_regular_exec_policy_stub.serial.log
NEO_TEST/1 event=PASS name=phase3_neofs_regular_exec_policy_stub path=/neofs/hello.txt regular_execute_policy=ok execute_right=0x00000010 no_actual_mmap=true no_actual_exec=true selected_superblock=0 generation=1

target/neo-test/phase3_neofs_type_vs_rights_policy_errors.serial.log
NEO_TEST/1 event=PASS name=phase3_neofs_type_vs_rights_policy_errors matrix_model_exec=TypeForbidden matrix_model_no_exec=TypeForbidden matrix_regular_no_exec=InsufficientRights matrix_regular_exec=ok type_first=true rights_second=true selected_superblock=0 generation=1 no_real_mmap_exec=true

target/neo-test/phase3_neofs_filecap_generation_stale.serial.log
NEO_TEST/1 event=PASS name=phase3_neofs_filecap_generation_stale path=/neofs/stale-2e.txt stale_cap=ESTALE stale_errno=-116 old_cap_generation=7 refreshed_generation=9 current_inode_generation=11 handle_a_continued=true reachable_generation_checked=true selected_superblock=0 generation=1

target/neo-test/phase3_neofs_reserved_type_refusal.serial.log
NEO_TEST/1 event=PASS name=phase3_neofs_reserved_type_refusal reserved_type_mount_rejected=NoValidSuperblock executable_access=NotFound inert_before_rights=true no_real_mmap_exec=true

target/neo-test/phase3_neofs_filerights_u32_forward_bits.serial.log
NEO_TEST/1 event=PASS name=phase3_neofs_filerights_u32_forward_bits kernel_filerights_size=4 libneo_filerights_size=4 requested_bits=0x0c000010 observed_bits=0x0c000010 retype_bit=0x04000000 open_error=InsufficientRights policy_preserved_high_bits=true selected_superblock=0 generation=1
```

Type-first/rights-second matrix:

- Model + execute right returns `TypeForbidden`.
- Model + no execute right returns `TypeForbidden`.
- Regular + no execute right returns `InsufficientRights`.
- Regular + execute right succeeds.

Stale FileCaps use `ESTALE = -116`. The stale check compares the captured handle generation against the current reachable NeoFS root directory entry generation, not against the old handle's in-memory inode block.

## Regression Gates

Command:

```text
docker compose run --rm neo cargo neo-test phase3_neofs_fsync_noop_write_through phase3_neofs_cow_write_after_write phase3_neofs_crash_mid_overwrite_recovery phase3_neofs_superblock_rotation_recovery phase3_neofs_crash_mid_create_recovery phase3_neofs_create_small_file phase3_neofs_write_readback_same_boot phase3_neofs_allocator_reuse_guard phase3_neofs_reboot_persistence_small_file phase3_neofs_mount_readonly_root phase3_neofs_mount_selects_highest_generation phase3_neofs_mount_rejects_bad_superblock phase3_block_vfs_smoke phase3_virtio_blk_raw_rw_smoke
```

PASS frames were recorded in `target/neo-test/*.serial.log` for every requested regression, including:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_noop_write_through neofs_fsync=noop path=/neofs/fsync-2d.txt bytes=53 fsync_rc=0 generation_before=3 generation_after=3 write_through=true no_flush_command=true no_intent_log=true selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_cow_write_after_write neofs_cow=ok path=/neofs/cow-2d.txt first_bytes=51 second_bytes=51 old_data_block=16395 new_data_block=16399 distinct_blocks=true selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_crash_mid_overwrite_recovery stage=readback neofs_recovery=ok path=/neofs/crash-ow.txt recovered=version_a unpublished_version_b_absent=true selected_superblock=0 generation=3 rejected_superblock_mask=0x00
NEO_TEST/1 event=PASS name=phase3_neofs_superblock_rotation_recovery stage=readback neofs_superblock_recovery=ok path=/neofs/sb-rot.txt selected_superblock=0 generation=3 invalid_slot=2 invalid_candidate_rejected=true rejected_superblock_mask=0x04
NEO_TEST/1 event=PASS name=phase3_neofs_crash_mid_create_recovery stage=readback neofs_create_recovery=ok target=/neofs/crash-create.txt marker=/neofs/create-marker-2d.txt target_absent=true selected_superblock=0 generation=3 rejected_superblock_mask=0x00
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

`phase3_aarch64_virtio_barrier_lane` was not rerun because 2E did not touch virtio, barrier, or block-driver files.

## SMP Sweep

Command attempted:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Result: the sweep did not complete. It stopped before the NeoFS 2E scenarios at `phase3_virtio_blk_abandoned_io_no_leak` with:

```text
NEO_TEST/1 event=FAIL name=phase3_virtio_blk_abandoned_io_no_leak reason=virtio_blk_abandoned_io_dma_live_leak before=3 after=6
```

Isolation checks:

```text
Base a2f1e49 targeted rerun under NEO_SMP=4:
NEO_TEST/1 event=PASS name=phase3_virtio_blk_abandoned_io_no_leak bdf=0x000300 retained_dma=3 live_before=3 live_after=3 late_wake=2 completions_delta=1 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false

2E branch targeted rerun under NEO_SMP=4:
NEO_TEST/1 event=PASS name=phase3_virtio_blk_abandoned_io_no_leak bdf=0x000300 retained_dma=3 live_before=3 live_after=3 late_wake=2 completions_delta=1 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false
```

The full sweep is therefore not claimed as closed in this artifact.

## Build And Tooling

PASS after final source edit:

```text
cargo check -p xtask
cargo check -p neofs-mkfs
cargo check -p libneo -p neofs-mkfs
cargo k
cargo k --no-default-features
cargo check -Zjson-target-spec --package neoplutarch-kernel --bin kernel --features "neo-test,neo-test-phase3-neofs-typed-inode-exec-refusal"
cargo check -Zjson-target-spec --package neoplutarch-kernel --bin kernel --features "neo-test,neo-test-phase3-neofs-regular-exec-policy-stub"
cargo check -Zjson-target-spec --package neoplutarch-kernel --bin kernel --features "neo-test,neo-test-phase3-neofs-type-vs-rights-policy-errors"
cargo check -Zjson-target-spec --package neoplutarch-kernel --bin kernel --features "neo-test,neo-test-phase3-neofs-filecap-generation-stale"
cargo check -Zjson-target-spec --package neoplutarch-kernel --bin kernel --features "neo-test,neo-test-phase3-neofs-reserved-type-refusal"
cargo check -Zjson-target-spec --package neoplutarch-kernel --bin kernel --features "neo-test,neo-test-phase3-neofs-filerights-u32-forward-bits"
git diff --check
git diff --cached --check
```

Scoped rustfmt was applied to every Rust file changed by 2E:

```text
rustfmt --edition 2024 kernel/src/arch/x86_64/syscall.rs kernel/src/errno.rs kernel/src/fs/mod.rs kernel/src/fs/neofs/inode.rs kernel/src/fs/neofs/mount.rs kernel/src/test_harness.rs tools/neofs-mkfs/src/format.rs tools/neofs-mkfs/src/main.rs tools/xtask/src/main.rs userland/libneo/src/lib.rs
```

The build still reports inherited unused/dead-code warnings; no new warning cleanup sweep was performed.
