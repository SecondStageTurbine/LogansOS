<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-071: Arthur M4 - memory forgetting and total wipe

**Status:** Accepted
**Date:** 2026-07-04
**Dispatch:** `Plans/DISPATCH-2026-07-04-ot17-m4-forgetting-janus.md`
**Prerequisites:** M2 encryption (ADR-065), M3 category delegation/truncate path

## Decision

Arthur's forgetting surface is completed behind the default-off
`arthur-memory-forgetting` feature, which implies the M3 delegation feature. With
the feature off, the M3 path is unchanged.

For `forget <category>`, Arthur now performs:

1. Revoke any outstanding delegated category cap.
2. Burn the category key first: load the old key if present, drop the in-RAM
   key, and durably replace `<category>.key` with a fresh valid key.
3. Read the existing `<category>.mem` length.
4. Overwrite the logical `.mem` object with a fixed `0xa5` pattern, `fsync`, and
   read it back to verify the pattern before returning success.

The post-forget `.mem` object intentionally remains the overwrite pattern until
the next `remember`. Under M4, recall treats that exact pattern as an empty
category, and the next remember rewrites a valid AEAD blob under the fresh key.
This gives the QEMU oracle a direct file-read proof that old ciphertext bytes are
not still exposed through the live NeoFS object.

For `forget everything`, Arthur first burns all category keys (`fact`,
`preference`, `event`, `relationship`) before overwriting any `.mem` object.
Only after every old key is dropped and every `.key` file has been durably
replaced with a fresh key does it overwrite the category files. A crash during
the wipe is therefore ordered toward privacy: once a category key is replaced,
the prior ciphertext for that category is not recoverable through Arthur's key
custody even if later file cleanup is interrupted.

## Proven vs Best-Effort Boundary

Proven by the M4 QEMU oracle:

- delegated category handles are revoked by forget;
- the live logical NeoFS `.mem` object reads back as the `0xa5` overwrite pattern
  after category forget and after total wipe;
- sibling categories survive a category forget until total wipe;
- post-wipe recall is empty, and new memories can be written under fresh keys;
- the feature is default-off and leaves M3 as the fallback.

Not proven: that the overwrite write lands on the same physical disk blocks that
previously held the ciphertext. NeoFS may copy-on-write, allocate a different
block, drain through a write-back cache, or sit on storage that remaps physical
media. Therefore the overwrite is secondary best-effort hygiene. The primary
erasure guarantee remains per-category key burn.

There is still no userland unlink primitive in libneo. M4 does not add a kernel
filesystem primitive; it composes the existing file writes, `fsync`,
`ftruncate`, key burn, and M3 revoke path.

## Arthur Surface

The deterministic Arthur memory command surface now includes:

- `forget <category>` for `fact`, `preference`, `event`, or `relationship`;
- `forget everything`;
- `forget all memory`;
- `forget all memories`;
- `forget everything about me`.

These are explicit mutating memory commands, not model-inferred actions. Any
future constrained NL action layer must classify them as irreversible memory
actions and apply the OT-19/OT-26 confirmation policy at that layer. The substrate
itself remains direct and irreversible by design.

## Verification

The `arthur_memory_forgetting` QEMU scenario is the falsifiable gate. It uses a
preseeded `arthur-memory-forgetting` NeoFS image with opaque existing memory
objects plus valid-length key files, captures the prior bytes, delegates a read
cap, forgets a category, verifies cap revocation and the overwrite pattern,
confirms a sibling category is untouched, then runs total wipe and verifies every
category's logical `.mem` object is overwritten, every category key is replaced,
pattern-backed recall is empty, and fresh writes work after the wipe. M2/M3 keep
the separate proof that normal Arthur writes are real single-blob AEAD records;
M4's gate focuses on the forgetting contract over already-existing memory
objects.

The scenario emits:

`arthur_memory_forgetting=ok forget_endstate=ok wipe_endstate=ok fresh_after_wipe=ok m4_default_off_m3=ok no_new_invariant=ok`
