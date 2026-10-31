# Phase 3 BUILD-2 Sub-Commit 2A Verification

Date: 2026-05-13
Owner: Jarvis
Branch: `codex/phase-3-build-2-2a-neofs-format-mkfs`
Dispatch: `Plans/DISPATCH-2026-05-13-phase-3-build-2-2a-neofs-format-mkfs-jarvis.md`
Commit: working tree at artifact creation; final sub-commit contains this file.

## Verdict

`PASS`

BUILD-2 sub-commit 2A is limited to the NeoFS v1 format specification and a host-side mkfs skeleton. No kernel mount path, runtime disk mutation, or 2B/2C/2D/2E work is included.

## Files Changed

- `Cargo.toml`
- `Cargo.lock`
- `docs/specs/neofs-format.md`
- `tools/neofs-mkfs/Cargo.toml`
- `tools/neofs-mkfs/src/main.rs`
- `tools/neofs-mkfs/src/format.rs`
- `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2A-VERIFICATION-2026-05-13.md`

## Format Evidence

- Superblock copies are emitted at offsets 0, 1 MiB, and 64 MiB.
- The default sparse image size is 128 MiB, large enough for all superblock copies and the seed tree.
- Superblocks use `b"NEOFSv1\0"`, format version 1, block size log2 12, 512-byte layout, zeroed reserved fields, and BLAKE3 checksum over bytes 0..479.
- Inodes use `b"NEOFSINO"`, 256-byte layout, eight direct block pointers, zero indirect pointer, zeroed reserved fields, logical-content BLAKE3 hash, and inode checksum over bytes 0..223.
- Directory entries use 64-byte records with NUL-terminated UTF-8 names capped at 47 bytes.
- Seed contents are:
  - root directory inode
  - `hello.txt` regular file inode
  - `model.neom` model inode
  - `subdir` directory inode
- The host verifier checks superblock checksums, highest-generation superblock selection, root inode hash, child inode type matches, content hashes, zero indirect pointer, and zero reserved fields.
- The tool rejects non-4096 block sizes, images too small for the superblock policy, unsupported inode type IDs, unsupported non-model `model.neom` kind IDs, and filenames longer than 47 bytes plus NUL.

## Host Sanity Evidence

Command, first run:

```text
cargo run -p neofs-mkfs -- sanity --output-dir target\neofs-mkfs-sanity
```

Result:

```text
NEOFS_MKFS_SANITY/1 status=PASS output_dir=target\neofs-mkfs-sanity image_size=134217728 hash=dd845cb0e9a4065e3d548274f4259da948ee4762d8335eb620ddad55175b1c85 deterministic=true superblock_copies=3 root_kind=Directory entries=hello.txt:Regular,model.neom:Model,subdir:Directory model_kind=Model indirect_zero=true reserved_zero=true rejects=ok
```

Command, second run:

```text
cargo run -p neofs-mkfs -- sanity --output-dir target\neofs-mkfs-sanity
```

Result:

```text
NEOFS_MKFS_SANITY/1 status=PASS output_dir=target\neofs-mkfs-sanity image_size=134217728 hash=dd845cb0e9a4065e3d548274f4259da948ee4762d8335eb620ddad55175b1c85 deterministic=true superblock_copies=3 root_kind=Directory entries=hello.txt:Regular,model.neom:Model,subdir:Directory model_kind=Model indirect_zero=true reserved_zero=true rejects=ok
```

The repeated hash proves deterministic output for the 128 MiB default image:

```text
dd845cb0e9a4065e3d548274f4259da948ee4762d8335eb620ddad55175b1c85
```

## Build and Style Checks

- `cargo fmt -p neofs-mkfs --check`: PASS.
- `cargo check -p neofs-mkfs`: PASS.
- `git diff --check`: PASS, with Windows LF/CRLF notices for `Cargo.toml` and `Cargo.lock` only.
- `cargo k`: PASS, with existing kernel warnings only.
- `cargo k --no-default-features`: PASS, with existing kernel warnings only.
- `cargo check -p xtask`: not applicable; `tools/xtask/src/main.rs` was not edited.

## Dependency Note

`tools/neofs-mkfs` adds the host-side `blake3` dependency. `Cargo.toml` requests `blake3 = "1.8.2"` and `Cargo.lock` resolves the semver-compatible `1.8.5` package plus its host build dependencies.

## Final Status

BUILD-2 2A PASS. Scope remains format spec plus host-side mkfs skeleton only.
