<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: `.neom` config-metadata header keys — L1 importer follow-on (F2 dependency) → Prometheus

**Date:** 2026-06-22
**From:** PM (Claude)
**To:** Prometheus (free after the forward-path THINK; bounded, deterministic, byte-oracle-able host-tool task — the A/B-proven shape). New branch off `origin/main` HEAD (`04897e9` or later): `prometheus/neom-config-metadata-header`.
**Status:** DISPATCHED. **Small, bounded importer extension that unblocks F2 (config-from-`.neom`).** The ratified forward-path design (D2, THINK `cf50ffb`) makes the Qwen2 config data-driven from the `.neom` so the 1.5B (and any variant) runs without recompiling. Tensor *shapes* are already derivable from the `.neom` tensor inventory, but **three scalar metadata values are not derivable from shapes** and must be carried in the `.neom` header: **`layer_count`, `rope_theta`, `rms_norm_eps`.** This build adds them to the importer + the header, with an independent byte-oracle. **Host-tool only — no kernel change.**

## 0. The decision this build makes
Turn "the `.neom` carries tensors + a vocab sidecar but not the 3 non-shape config scalars" into "the `.neom` header carries `layer_count` / `rope_theta` / `rms_norm_eps`, read straight from the source GGUF metadata" — so F2's kernel loader can construct `ModelConfig` entirely from the model object. **This is a `.neom` *format* extension ⇒ the byte-oracle rule applies.**

## 1. References
```
git fetch origin
git show origin/main:tools/neomc/src/gguf_import.rs        # L1 importer: GGUF metadata read + .neom header/tensor-inventory write — where the 3 keys are sourced + written
git show origin/main:docs/ADR-028-neom-format.md           # the .neom on-disk format (header layout) — the authoritative byte spec to extend (confirm the exact ADR number/name in docs/)
git show origin/main:Plans/THINK-2026-06-21-forward-path-real-model-integration.md  # D2 (the 3 keys + why shapes are insufficient); §3
git show origin/main:kernel/src/ai/forward_logic.rs        # the consumer side (F2, not this build): QWEN25_05B_* consts these 3 values replace — layer_count(24), rope_theta, rms_norm_eps
```
**The 3 values come from the GGUF metadata directly** (e.g. `qwen2.block_count`, `qwen2.rope.freq_base`, `qwen2.attention.layer_norm_rms_epsilon` — confirm the exact GGUF KV keys against the source GGUF you imported in L1; **raw-source the key names, do not guess from memory**).

## 2. ISCs
- **ISC-1 — Source the 3 keys from GGUF metadata.** In `gguf_import.rs`, read `layer_count` (u32), `rope_theta` (f32), `rms_norm_eps` (f32) from the GGUF KV metadata (the same metadata block L1 already parses for tensor info). If a key is absent in a given GGUF, fail the import with a clear error (don't silently default — a wrong rope_theta is a silently-wrong model).
- **ISC-2 — Write them into the `.neom` header (the format extension).** Extend the `.neom` header with a fixed, documented layout for the 3 values (the THINK's "~24-byte addition" — exact sizes/offsets are yours to define + document). **Preserve every existing header field's offset** so existing `.neom` files / the kernel's current header parse stay byte-compatible (additive extension — version-bump the header or use a reserved region; state which). Update ADR-028 with the new field layout + a version note.
- **ISC-3 — Byte-oracle (independent, hand-laid — the format rule).** A **hand-laid fixture**: the 3 fields at their exact header offsets with known values (e.g. `layer_count=24`, a known `rope_theta`, a known `rms_norm_eps`), asserted **independent of the writer+reader round-trip** (ADR-028 lesson — a writer+reader built from the same spec agree even when both are off-spec). Include the LE byte encoding of each. Round-trip (import a GGUF → read the 3 values back) is the *complementary* check, not the oracle.
- **ISC-4 — Additivity / equality.** Existing `.neom` files (the L1 0.5B `.neom`) must still validate; the kernel's *current* header parse (which doesn't know the new fields) must not break (additive/reserved-region or version-gated). The vocab sidecar is unchanged. **No new invariant.** **No model/GGUF/vocab blob in git** (the L1/bundled-model discipline) — the importer reads an on-disk GGUF path at build time only.
- **ISC-5 — Standards.** SPDX on any new file. `cargo fmt --all --check` + `cargo clippy --workspace --all-targets -D warnings` via Docker. New `unsafe`: none (host-tool byte I/O in safe Rust).

## 3. Scope guard
The 3 config-metadata keys (source from GGUF + write to `.neom` header + byte-oracle + ADR-028 update) **only**. **Do NOT** change the tensor inventory/quant handling (L1 frozen), the vocab sidecar, the kernel (F2 consumes these — separate build), or anything in the forward path. **No blob in git.** No new invariant.

## 4. Gate
Host KATs: the ISC-3 byte-oracle (3 fields at exact offsets, hand-laid) + the round-trip (import the real 0.5B GGUF → read back `layer_count=24` + the known rope_theta/rms_norm_eps) + the additivity check (the existing 0.5B `.neom` still validates). `fmt`/`clippy -D` green via Docker. **No QEMU (host-tool only).**

## 5. Merge (PM)
**Close-gate: `scripts/worker-close-gate.sh`** (rebase LAST; host-checks; SPDX; self-stat). Green push-CI URL (no QEMU needed — host-tool). PM re-inspects `git show --stat` (**confirm no GGUF/model/vocab blob**) + re-runs the byte-oracle reasoning. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (**OT-28: F2 config-metadata dependency satisfied; F2 unblocked**).

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main`. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-22-neom-config-metadata-header-l1-followon-prometheus.md` (+ `tools/neomc/src/gguf_import.rs`, ADR-028, the forward-path THINK D2). **Branch:** `prometheus/neom-config-metadata-header` off `origin/main`.
- **One-line:** Small **L1 importer follow-on that unblocks F2**: add **3 config scalars not derivable from tensor shapes** — `layer_count` (u32), `rope_theta` (f32), `rms_norm_eps` (f32) — sourced **from the GGUF KV metadata** (raw-source the exact key names, e.g. `qwen2.block_count`/`qwen2.rope.freq_base`/`qwen2.attention.layer_norm_rms_epsilon` — confirm against the source GGUF, don't guess) and written into the **`.neom` header** as a documented, **additive** extension (preserve existing field offsets; version-bump or reserved-region — state which). **Byte-oracle (format rule):** hand-laid fixture of the 3 fields at exact offsets with known LE bytes, asserted independent of round-trip (ADR-028 lesson). Missing key ⇒ fail the import (never silently default — a wrong rope_theta is a silently-wrong model). Existing 0.5B `.neom` must still validate; vocab sidecar unchanged; **no blob in git**; no new `unsafe`/invariant; update ADR-028. **Scope: the 3 keys ONLY — not the tensor inventory, not the kernel (F2 consumes them), not the sidecar.** Gate: host KATs (byte-oracle + GGUF round-trip reading back `layer_count=24` + additivity) — **no QEMU.** **Close-gate: `scripts/worker-close-gate.sh`; RETURN the green push-CI URL; PM re-scans for any blob.**
