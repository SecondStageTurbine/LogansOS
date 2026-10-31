<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: OT-90 shutdown THINK re-check (relay #118)

**MODEL:** GPT-6-Luna. **EFFORT:** medium. **Seat:** Janus.
**Reviewed:** `docs/THINK-2026-10-27-ot90-shutdown-restart.md` at `15379e82` (revision 2, PR #153).
**Overall: FAIL.** Item 2 still has a concrete false-accept in the proposed byte scanner. Items 4 and 8 pass this narrow re-check.

## Checklist

1. **Item 2, the `\\_S5` decode — FAIL.** The revised rule requires a unique byte sequence, `NameOp`, `PackageOp`, and a covering-scope check (THINK:143-164). The covering list includes Scope, Method, If, Else, While, Device, Processor, PowerResource, and ThermalZone, but omits `BufferOp` and other AML containers (THINK:148-162). Since the scan treats every byte as a candidate, AML-looking bytes in a Buffer payload can be mistaken for a root definition.

   Concrete input: a DSDT containing `Name (DUMY, Buffer (0x0A) { 0x08, 0x5F, 0x53, 0x35, 0x5F, 0x12, 0x04, 0x02, 0x00, 0x00 })`, with no actual `_S5` Name. The payload contains the exact accepted pattern `NameOp _S5_ PackageOp (2, Zero, Zero)`. The described checks see one `_S5_`, the preceding `NameOp`, the following two-element package, and no recognized covering scope; they can therefore accept this non-definition as `\\_S5` and derive SLP_TYP 0. This violates the requirement to prove the namespace object is the root `\\_S5`.

   KAT coverage also does not list an `_S5` reached through an SSDT, despite the rule spanning DSDT and all SSDTs (THINK:131, 143; planned KATs at :405-410). Add the Buffer-payload case and at least one SSDT case, and ensure the scanner rejects data-contained lookalikes or use a parser that establishes the actual Name definition and path. The positive evidence is only a Python prototype; the Rust implementation is future P1 work (THINK:166-178, 503).

   I spot-checked the captured DSDT bytes in two fixtures: Zenbook has one `_S5_` at offset `0x9a47` followed by `PackageOp` bytes encoding 7/0; M75q has one at `0x3aec` encoding 5/0. These support those two corpus examples, not the scanner’s behavior for other AML layouts.

2. **Item 4, durability — PASS.** K2 now freezes writes, uses a shutdown-only drain, stops the worker, and reports unclean on drain timeout or write/flush error (THINK:246, 253-273). This follows the referenced #180 publication order: data/inode/directory/free-map writes, barrier FLUSH, superblock publication, then a second FLUSH; an unsuccessful first barrier prevents superblock writes (origin `claude/neofs-commit-barrier:docs/specs/neofs-format.md` §3.4, lines 105-118). Deferred reuse waits until every present superblock copy has the newer generation after a completed FLUSH (§3.5, lines 121-123). The THINK correctly says P3 needs #180 merged (THINK:505).

3. **Item 8, prior art — PASS.** The date convention now explicitly says UTC committer dates (THINK:358-360), and Redox is corrected to `2026-09-26 (19:58:20Z)` (THINK:366), matching the dispatch’s required timestamp.

## Review method

Read the relay #118 dispatch, prior RETURN #93, the revision diff, and the specified #180 NeoFS format document. RAG health check at `http://127.0.0.1:8404/health` failed with a connection error; review continued by direct file reads. No build or tests were run; this is a documentation re-check.
