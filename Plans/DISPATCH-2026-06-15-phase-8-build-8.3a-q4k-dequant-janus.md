<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 8 BUILD-8.3a — the standalone Q4_K dequant kernel, byte-oracle-KAT'd → Janus

**Date:** 2026-06-15
**From:** PM (Claude)
**To:** Janus (new branch off `origin/main` HEAD: `codex/phase-8-build-8.3a-q4k-dequant-janus`).
**Status:** DISPATCHED. **Host-tier only — a pure dequant function, no model, no QEMU, no silicon.** The last dependency-light Phase-8 slice (after this, the remaining builds need 8.2). Same clean-slice pattern as 8.4a/8.5a: build the **Q4_K → f32 dequant math** standalone + byte-oracle-KAT'd now, parallel + non-colliding with Jarvis's 8.2.
**Why it doesn't get in the way:** 8.2 (Jarvis) parses the **GGUF container** and *extracts* quantized blocks; 8.3a (you) takes a **raw Q4_K block (`&[u8]`) → `[f32; 256]`** — the dequant arithmetic. **Operate on raw bytes so you define no shared type with 8.2** (the integration build, 8.3, wires 8.2's extracted block-bytes → your dequant → the existing f32 matmul). Lives in `ai/ops/` (or a small `ai/quant.rs`) + `ai-ops-tests` — disjoint from 8.2's `gguf`/`tokenizer`; only `ai-ops-tests/src/lib.rs` overlaps (additive, trivial rebase reconcile, as in 8.4a/8.5a).

## 0. The decision this build makes
Prove the **Q4_K dequant arithmetic** in isolation against a hand-laid block, so 8.3 (the f32 inference path) is just *wiring* 8.2's extracted blocks → this dequant → the f32 matmul. This is the byte-oracle for the quant format — exactly where exact-byte fidelity matters most (the 72→144 catch is why).

## 1. Reference (on `main`) — RAW-SOURCE the layout + the math
```
git fetch origin
git show origin/main:Plans/RESEARCH-2026-06-14-phase-8-real-model-gap-analysis-FINDINGS.md   # §3 block_q4_K layout (CORRECTED: 144 B/256), dequant ref
```
**`block_q4_K` = 144 bytes / 256 values:** `d` (f16, 2 B) + `dmin` (f16, 2 B) + `scales[12]` (the packed 6-bit per-sub-block scales+mins) + `qs[128]` (256 × 4-bit quants). 8 sub-blocks of 32. Dequant per value: `y = d · sc − dmin · m` where `(sc, m)` are the sub-block's 6-bit scale/min unpacked from `scales[12]`, and the 4-bit `q` selects within. **Pull the exact unpack + dequant from raw `ggml-quants.c` `dequantize_row_q4_K` (the gap-analysis §3 / `:1471-1493`) — never a summarizing fetch; the q4_K=144 slip is precisely why.**

## 2. ISCs — byte-oracle KAT (I-12, NOT round-trip)
- **ISC-1 — `dequantize_q4_K(&[u8] /* 144 */) -> [f32; 256]`.** Parse `d`/`dmin` (f16), unpack the 12 scale-bytes into the 8 `(scale, min)` 6-bit pairs, expand the 128 `qs` nibbles, apply `y = d·sc − dmin·m`. **KAT: a hand-laid 144-byte block** (chosen `d`/`dmin`/`scales`/`qs`) → the **independently-computed** expected `f32[256]` (hand/Python from the dequant formula, or a raw-source reference vector — *not* your own code round-tripped). This is the format byte-oracle.
- **ISC-2 — The 6-bit scale/min unpack (the trap).** The `scales[12]` packing of 8×(6-bit scale + 6-bit min) is the fiddly bit (the K-quant `get_scale_min_k4` shuffle). KAT it explicitly: a fixed `scales[12]` → the 8 expected `(sc, m)` pairs, asserted, so a wrong-shuffle bug fails here not silently in production.
- **ISC-3 — Standards.** A standalone fn (a plain `*_logic` fn is fine — it's invoked at weight-load, not in the op graph, so **no `OpKind`/exec_ops/accel_sim arms needed** unless you choose to make it one); host KAT (raw-source/hand-computed, never round-trip); **no GGUF container parsing (8.2), no importer/matmul wiring (8.3-integration), no `unsafe`, no new invariant**; operate on `&[u8]` (define no `block_q4_K` type that would collide with 8.2). ANSPEC §11.x note; fold the ADR into 8.3's.

## 3. Gate (host only)
Host KATs green — the hand-laid-block → f32[256] oracle + the scale/min-unpack oracle (each vs an independent computation, not round-trip); the existing op family unaffected (107/107 + the new ones); `fmt`/`clippy -D warnings`; builds on+off. **No QEMU.**

## 4. Scope guard
The **Q4_K dequant fn + its byte-oracle KATs only.** **Do NOT** build the GGUF container parser / tokenizer (8.2 — Jarvis), wire into the importer or the matmul/forward pass (8.3-integration), define a `block_q4_K` struct (operate on `&[u8]` — disjoint from 8.2), implement quantized-matmul (Phase 9), or touch `generate.rs`/W1. Stay in `ops/`(or `quant.rs`) + `ai-ops-tests`. **Raw-source the unpack + dequant math + the layout — never WebFetch the exact bytes.** No new invariant/`unsafe`.

## 5. Merge (PM)
Close-gate: rebase onto current `main` (reconcile the additive `ai-ops-tests/src/lib.rs`), re-run the dequant KATs + the family, push (CI host gate). PM `pm-merge-check` → `--ff-only` → ledger. **After 8.3a, the dependency-light slices are exhausted** — 8.3-integration and 8.5-integration both need 8.2's importer, so Janus then **holds for 8.2** (Jarvis, in flight). With 8.4a + 8.5a + 8.3a banked, the integration builds are pure wiring of proven, byte-verified pieces.

---

## Relay — what to tell Janus
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-15-phase-8-build-8.3a-q4k-dequant-janus.md`
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-15-phase-8-build-8.3a-q4k-dequant-janus.md
  git show origin/main:Plans/RESEARCH-2026-06-14-phase-8-real-model-gap-analysis-FINDINGS.md   # §3 q4_K layout (144 B) + dequant
  ```
- **Branch:** `codex/phase-8-build-8.3a-q4k-dequant-janus` off `origin/main` HEAD (has 8.4a + 8.5a; disjoint from Jarvis's 8.2 except additive `ai-ops-tests`).
- **One-line:** Build the **standalone Q4_K dequant** (the 8.4a/8.5a pattern): `dequantize_q4_K(&[u8] 144 B) -> [f32; 256]` — parse `d`/`dmin` (f16) + unpack the 12 scale-bytes into 8×(6-bit scale,min) + expand 128 nibbles, `y = d·sc − dmin·m`. **Raw-source the unpack + dequant math from `ggml-quants.c dequantize_row_q4_K`** (never WebFetch — the q4_K 72→144 catch is why). Byte-oracle KATs (I-12, not round-trip): a **hand-laid 144-byte block → independently-computed f32[256]**, plus the **6-bit scale/min unpack** (the `get_scale_min_k4` shuffle trap). **Operate on `&[u8]`** so you define no `block_q4_K` type that collides with 8.2; **no container parser, no importer/matmul wiring** (that's 8.2 / 8.3-integration). Plain fn (no OpKind/exec arms needed), host-only, no new invariant/`unsafe`. fmt/clippy clean, push (let CI run). This is the **last** clean parallel slice — after it, hold for 8.2.
