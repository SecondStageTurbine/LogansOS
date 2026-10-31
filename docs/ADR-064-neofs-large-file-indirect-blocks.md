<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-064: NeoFS Large Files Through Indirect Blocks

## Status

Accepted for the L2 large-model provisioning substrate.

## Context

ADR-063 established Arthur's production model registry paths, but NeoFS still
stored file payloads in eight direct blocks. That capped `.neom` model payloads
at 32 KiB and left production-sized model files blocked even when they were
generated outside the repository.

The dispatch required additivity: direct file offsets and byte identity must
stay intact, no model or vocab blobs may be committed, and kernel reads must
fail closed on corrupt metadata.

## Decision

NeoFS v1 now keeps the existing direct block array and introduces three indirect
root fields:

- `single_indirect_block` at inode offset 96.
- `double_indirect_block` at inode offset 136.
- `triple_indirect_block` at inode offset 144.

The `content_hash` remains at offset 104, so direct-only inode bytes keep the
legacy field placement. Unused indirect roots must be zero. Required pointer
blocks contain 512 little-endian `u64` block numbers and every pointer is
validated against the mounted filesystem block count before use.

`neofs-mkfs` is the large immutable payload writer. It can stage extra model
files as `Model` inodes and extra sidecars as regular files through
`--extra-file`; xtask also stages an adjacent `.vocab` sidecar when one exists
beside a staged `.neom`. Kernel runtime writes remain small/direct-only.

## Validation

- Host KATs cover hand-laid inode offsets, boundary logical indices 7, 8,
  `8 + 512`, and `8 + 512^2`, direct-only layout regression, large round-trip
  through double-indirect, and corrupt pointer rejection.
- QEMU scenario `neofs_large_file` mounts NeoFS, reads `/neofs/hello.txt`, and
  byte-verifies `/neofs/large-double.bin`, a generated few-MB regular file that
  crosses into double-indirect blocks.

## Consequences

- Production `.neom` and tokenizer sidecar staging no longer depends on a
  Limine module or a checked-in blob.
- Kernel reads remain fail-closed for malformed indirect roots or pointer
  blocks.
- Triple-indirect is defined and parsed now, even though the QEMU fixture only
  needs direct, single, and double-indirect coverage.
