# Phase 3 BUILD-2 Sub-Commit 2B Verification

Date: 2026-05-13
Owner: Jarvis
Branch: codex/phase-3-build-2-2b-neofs-readonly-mount
Base: f781957
Dispatch: Plans/DISPATCH-2026-05-13-phase-3-build-2-2b-neofs-readonly-mount-jarvis.md

## Verdict

PASS. Sub-commit 2B implements a read-only NeoFS mount from a mkfs-created virtio-blk image and passes all three targeted mount scenarios in real QEMU x86_64 via the Docker dev container.

No runtime NeoFS mutation, file create/write, 2C, 2D, or 2E work was performed.

## Implementation Summary

- Added kernel NeoFS read-only parser/mounter under `kernel/src/fs/neofs/`.
- Added `/neofs` VFS routing while preserving the BUILD-1 testfs root namespace.
- Validates NeoFS v1 superblock magic, checksum, version, block size, inode size, reserved fields, root pointer bounds, generation, inode checksum, direct-block bounds, directory entries, child type matching, and file content hashes.
- Scans the three 2A superblock copy offsets and selects the highest valid generation.
- Reads file data through `BlockDevice::read_block`; no NeoFS kernel write path or `write_block` use was added under `kernel/src/fs/neofs`.
- Returns `InsufficientRights` for NeoFS writes through `NeoInode::write_at`.
- Treats `Model` as a readable VFS inode kind for 2B, while leaving symlink/native mutation semantics out of scope.
- Added host-only mkfs variants for targeted verification:
  - `normal`
  - `highest-generation`
  - `corrupt-primary`
- Added xtask scenario wiring to generate the selected mkfs image before QEMU boot.

## Targeted Runtime Scenarios

Local host QEMU is blocked because Limine is not installed at the expected path:

```text
cargo neo-test phase3_neofs_mount_readonly_root
FAIL: Limine directory /home/neo/limine does not exist. Set $LIMINE_DIR or build the dev container.
```

Runtime verification was therefore run in the Docker dev container.

### Read-Only Root

Command:

```text
docker compose run --rm neo cargo neo-test phase3_neofs_mount_readonly_root
```

Host image:

```text
variant=normal hash=dd845cb0e9a4065e3d548274f4259da948ee4762d8335eb620ddad55175b1c85
```

Serial evidence:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_mount_readonly_root neofs_mount=ok selected_superblock=0 generation=1 valid_superblocks=3 rejected_superblocks=0 root_kind=Directory hello_read=ok model_kind=Model readonly_write_denied=InsufficientRights runtime_mutation=false
```

### Highest Generation Selection

Command:

```text
docker compose run --rm neo cargo neo-test phase3_neofs_mount_selects_highest_generation
```

Host image:

```text
variant=highest-generation hash=c753e70da8271cb0acb7429e080802747874eda197b904286fda183eece11ade
```

Serial evidence:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_mount_selects_highest_generation neofs_mount=ok selected_superblock=2 generation=3 highest_generation_selected=true valid_superblocks=3 rejected_superblocks=0 runtime_mutation=false
```

### Bad Primary Rejection

Command:

```text
docker compose run --rm neo cargo neo-test phase3_neofs_mount_rejects_bad_superblock
```

Host image:

```text
variant=corrupt-primary hash=e4253a62912889f759f0ee7836cddc0a3f2a7181cb51783033aaf127a7c73433
```

Serial evidence:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_mount_rejects_bad_superblock neofs_mount=ok selected_superblock=1 generation=1 corrupt_primary_rejected=true rejected_superblock_mask=0x01 valid_superblocks=2 rejected_superblocks=1 runtime_mutation=false
```

## Regression Evidence

Commands:

```text
docker compose run --rm neo cargo neo-test phase3_block_vfs_smoke
docker compose run --rm neo cargo neo-test phase3_virtio_blk_raw_rw_smoke
```

Serial evidence:

```text
NEO_TEST/1 event=PASS name=phase3_block_vfs_smoke bdf=0x000300 block_vfs_same_boot=true raw_rw=ok vfs_syscalls=ok stdout_namespace=fd1_serial lba=3 block_size=512 completions_delta=2 descriptor_publish_barriers=2 notification_suppression_barriers=2 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false
NEO_TEST/1 event=PASS name=phase3_virtio_blk_raw_rw_smoke bdf=0x000300 block_size=512 lba=1 completions=2 descriptor_publish_barriers=2 notification_suppression_barriers=2 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false
```

## Build And Tooling Evidence

All commands passed:

```text
cargo k
cargo k --no-default-features
cargo check -p xtask
cargo check -p neofs-mkfs
cargo check -p neoplutarch-kernel --features neo-test-phase3-neofs-mount-readonly-root
cargo check -p neoplutarch-kernel --features neo-test-phase3-neofs-mount-selects-highest-generation
cargo check -p neoplutarch-kernel --features neo-test-phase3-neofs-mount-rejects-bad-superblock
rustfmt --check kernel\src\fs\mod.rs kernel\src\fs\neofs\mod.rs kernel\src\fs\neofs\format.rs kernel\src\fs\neofs\inode.rs kernel\src\fs\neofs\mount.rs tools\xtask\src\main.rs tools\neofs-mkfs\src\main.rs tools\neofs-mkfs\src\format.rs
cargo run -p neofs-mkfs -- sanity --output-dir target\neofs-mkfs-sanity
git diff --check
```

Host mkfs sanity output:

```text
NEOFS_MKFS_SANITY/1 status=PASS output_dir=target\neofs-mkfs-sanity image_size=134217728 hash=dd845cb0e9a4065e3d548274f4259da948ee4762d8335eb620ddad55175b1c85 deterministic=true superblock_copies=3 root_kind=Directory entries=hello.txt:Regular,model.neom:Model,subdir:Directory model_kind=Model indirect_zero=true reserved_zero=true rejects=ok
```

Rust warnings remain in the existing warning-heavy kernel build; none of the required build or runtime commands failed.

## Scope Guard

- No NeoFS runtime disk mutation.
- No NeoFS file create/write.
- No kernel-side mkfs.
- No mount namespaces.
- No readdir syscall.
- No mmap/exec.
- No symlink or hard-link semantics.
- No 2C/2D/2E work started.
