<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Desktop WM fix-forward cross-check (Chad)

**Overall verdict:** ACCEPT (PR #43 is merged as `968d4ca4`; no further fix-forward required for the three reviewed findings).

- **S1-F2 — CLOSED.** No ordinary Rust slice/reference over client-writable pages remains in the desktop or DW-1/DW-2 compositor paths.
- **S2-F2 — CLOSED.** Arthur's bounded damage reaches the WM copy, composition, buffer commit, and translated output rectangle.
- **S3 label-mask finding — CLOSED.** All five S3 runs, including `· owned` and the chip name, are independently bounded at the pixel level in both materials.
- **S2-F1 quick re-check (#42) — CLOSED.** The two canonical IBM TTFs have valid whole-file sfnt checksums and pinned hashes, and every image assembly stages the OFL plus NOTICE.

**Reviewed:** `968d4ca4`, `55d8ccf6`, and their current descendants on `origin/main` at `cfafff09`. The semantic index was healthy (`ollama=true`, `qdrant=true`) and was used for discovery; exact symbols and construction sites were checked directly.

## Findings

No correctness or security finding survived re-verification. One test-strength note remains under item 4; it does not reopen S2-F2 because the production data flow itself is bounded and was checked end to end.

## Dispatch checklist

### 1. S1-F2 — CLOSED

`SharedPixels` stores only a raw `*const AtomicU32`; `snapshot` clips the requested rectangle, computes indices from trusted dimensions, and performs relaxed atomic loads into a caller-owned `&mut [u32]` (`userland/libneo/src/atrium_wm.rs:204-253`). Relaxed ordering is appropriate: the protocol message supplies sequencing for a well-behaved client, while the atomic operation's purpose under a hostile writer is a non-tearing, compiler-visible load, not synchronization with an untrusted store. A volatile load would force an access but would not provide Rust's atomic access semantics.

Both production construction sites satisfy the unsafe contract. The desktop verifies the exact 640×400 tightly-packed descriptor before construction (`userland/neo-init/src/desktop_session.rs:608-637`). DW-1/DW-2 likewise compare the exact 32×32 descriptor before `SharedPixels::new` is reached (`userland/neo-init/src/dw1_desktop_wm.rs:695-773`). The kernel's borrowed mapping pins the backing frames and is page-aligned.

The remaining `from_raw_parts` calls in the desktop family are over WM-owned output/base planes; Arthur's mutable slice is over its own RW mapping. Desktop `Layer` points only to `base`, `glow`, or `client_copy` (`desktop_session.rs:349-380`). DW-1/DW-2 `SceneSurface` points only to stack snapshots (`dw1_desktop_wm.rs:791-936`). A hostile writer can make one snapshot temporally mixed, but pixel values cannot affect dimensions, indices, layer selection, or any surface other than its own.

### 2. S2-F2 — CLOSED

The decoder rejects zero width/height. Registry validation widens each u16 component to u32 before addition, so `x + width` and `y + height` cannot overflow; it refuses anything beyond the created surface before consuming the serial (`userland/libneo/src/atrium_protocol.rs:492-507,706-724`). Channel/provenance lookup happens first and prevents peer commits.

The desktop converts only a registry-approved rect, snapshots it through a second clip, translates the clipped rect by the fixed client origin, composes that translated rect, and commits exactly the returned clipped output rectangle (`userland/neo-init/src/desktop_session.rs:333-335,924-959`). Output dimensions are rejected above u16::MAX at session start. A client that under-reports changes leaves only its own WM-owned copy stale; chrome, focus light, output geometry, and other clients never source from that mapping.

### 3. Protocol — CLOSED

The hand-laid bytes independently match opcode 9, payload length 40 / total length 64, serial at 48..56, and LE u16 x/y/w/h at 56/58/60/62. Empty and truncated/length-invalid requests fail. `OP_COMMIT` remains opcode 3 and full-surface. Only the `desktop-session` Arthur feature selects `OP_COMMIT_DAMAGE`; standalone Arthur keeps `OP_COMMIT`, and DW-1/DW-2 workers are unchanged. `commit_damage_on_channel` uses the same identity/provenance binding as legacy commit, so a client cannot damage-commit a peer.

### 4. Witness strength — code closed; LOW test note

`band=yes` is based on the `written` rectangle returned by the same composition routine, and xtask checks only the first in-loop damage commit. It will catch the ordinary regression of passing the whole client/scene rectangle into `Scene::present`, because `compose_layers_xrgb8888` returns the clipped rectangle it actually rewrote. It is not an independent observation of memory traffic or scanout damage: an implementation that rewrote more pixels but falsely returned the requested band, or a backend that ignored the committed dirty rectangle, could retain the log and pass. Later damage commits are not individually witnessed.

That limitation does not reopen S2-F2: the inspected implementation uses the one returned rectangle for composition, `SYS_BUFFER_COMMIT_DAMAGE`, and the log, and no alternate desktop path expands it. A future performance discriminator could poison pixels outside the band in the WM output and verify they remain untouched, or add backend damage counters.

### 5. Kill-switch and regressions

The added verb is feature-gated by `libneo/atrium-damage`; only desktop-session enables it. Standalone showcase behavior and legacy `OP_COMMIT` remain intact. The copied pixels are identical to the committed source pixels, so scene/readback hashes should not change. DW-1/DW-2 make full local snapshots before their existing composition/oracle paths, preserving their byte oracles. Host scene/oracle suites pass. QEMU gates could not be launched locally because the configured Limine directory is absent (validation below).

### 6. S3 labels — CLOSED

`scripts/desktop/label_oracle.py` imports only the Python exact-area/fontTools oracle and independent NumPy scene renderer; it shares no neo-text or neo-atrium code. I independently re-derived the trust placement: frame x = 674, title x = 692, measured 14 px `Arthur` advance = 40.124, so `· owned` begins at x = 740.124; exact coverage plus the one-pixel window margin yields fixture window `(740, 349, 40, 12)`.

Every pixel inside each tight run window is bounded per channel against exact coverage ±2/255 over the independently rendered background. Full-coverage stems pin the intended primary/secondary colour; shifted or different glyph coverage fails at fixed coordinates. Every pixel outside those five windows is covered by the scene digest, so the former broad-box hole is gone. Pixel-identical Unicode aliases would be visually identical and are not a meaningful counterexample. The host negative controls erase `TITLE_TRUST` and `CHIP_NAME` and fail as intended; generated fixture data reproduced exactly.

Home's shared static bar/chip rendering benefits from the same code path, but the Home scene still masks its dynamic field text/caret. That is acceptable for closing the original S3 five-label finding; the dynamic Home coverage gap is separately recorded in `RETURN-2026-10-18-desktop-ds1a-codex-crosscheck-chad.md`.

### 7. Copy cost / other

The desktop adds one 640×400 WM-owned plane. The first commit snapshots the whole surface; later keystrokes copy and compose only their reported band. Focus/view changes reuse the stable copy rather than rereading client pages. Thus the safety copy moved work into the already-damaged area and reduces normal per-keystroke work instead of moving a full-frame copy into a hotter path. DW-1/DW-2 copy 32×32 surfaces to stack arrays before their infrequent whole-scene oracle compositions. I found no new unbounded work or client-controlled allocation.

## S2-F1 font re-check (#42 / `55d8ccf6`) — CLOSED

The shipped assets now hash to the values pinned in `assets/fonts/README.md`:

- IBM Plex Sans Regular: `975dcda37d80f038dcd143c22e33ca2d97a0cc5a929aace1c749153b0fe1afa5`
- IBM Plex Mono Regular: `7c6fbddca4b700be918f5f6183d9bd4464fa427fe435f0b480d77fe2bb8c5a43`

`shipped_fonts_are_canonical_sfnts` sums every padded big-endian u32 of each complete file and obtains OpenType's `0xB1B0AFBA`, then checks family, style, version, copyright, OFL notice, and OFL URL name records. Its flipped-byte negative control proves the old broken-whole-file-checksum class is detected. The checksum is an sfnt consistency invariant rather than a cryptographic identity proof; the documented SHA-256 values provide the latter, and the checked working-tree files match them.

`assemble_iso_with_limine_kaslr` unconditionally creates `/licenses` and copies `assets/fonts/OFL-IBM-Plex.txt` plus repository `NOTICE` into it (`tools/xtask/src/main.rs:50-56,4210-4215`). The source-file test verifies both are readable/non-empty and that the OFL contains version 1.1 plus Reserved Font Name "Plex". This closes both the invalid reconstructed-font checksum and redistribution-staging parts of S2-F1.

## Validation performed

- `cargo test -p atrium-logic-tests -p neo-atrium` — PASS (95 passed; and 22 passed / 2 ignored).
- `cargo test -p neo-text` — PASS (28 passed, including both font-integrity tests).
- `cargo test -p xtask staged_licence_files_exist` — PASS (1 selected test).
- `python scripts/desktop/label_oracle.py fixture` — generated Rust code/data matched `fixtures/desktop-s3/labels.rs` (comments excluded from comparison because PowerShell decoded the middle dot/em dash differently).
- SHA-256 of both local TTFs matched `assets/fonts/README.md`.
- QEMU desktop gates — NOT RUN: xtask completed the build but stopped because `/home/neo/limine` does not exist and `LIMINE_DIR` is not configured in this Windows worktree.
