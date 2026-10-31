<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: `.neom` layer-schedule section — the draft byte-spec + hand-laid oracle (B1 prep) → Prometheus

**Date:** 2026-07-05
**From:** PM (Claude)
**To:** Prometheus (research / byte-spec / host-only lane). **Branch:** `prometheus/neom-schedule-bytespec` off `origin/main` HEAD (`c9ad583` or later).
**Status:** DISPATCHED — **the B1-prep deliverable the OT-16 ratification calls for.** The PL ratified OT-16 (2026-07-05, `docs/THINK-2026-07-05-ot16-forward-graph-design.md`): layer-kind schedule, `.neom` format-version bump **with an ADR-028-family byte oracle when B1 starts**, Phase-9 sequencing. This dispatch pre-bakes that oracle so B1 is a straight lift whenever it opens: a **DRAFT byte-level spec of the `.neom` schedule section, with the authoritative hand-laid worked example as its normative core.** **Scope: a spec/THINK doc, host-only — NO code, NO format change lands now, no QEMU. Output: `docs/THINK-2026-07-05-neom-schedule-section-bytespec.md`.** B1 lifts this draft into the numbered ADR when it lands the code.

## 0. Why the worked example IS the deliverable (the ADR-028 lesson)
ADR-028's v1 text was internally contradictory about `weight_off` (absolute vs blob-relative); writer and reader agreed with each other while both off-spec, every round-trip test passed, and the discrepancy surfaced two builds later as a four-file correction. The standing rule since: **a byte-layout spec ships a hand-computed byte example asserted against, independent of any writer/reader code** — the prose serves the example, not the reverse. Your spec's normative core is the worked example; ambiguity that survives the example is a spec bug.

## 1. What to spec (ISCs)
- **ISC-1 — Establish the CURRENT format first.** Read the real `.neom` header code (the importer/writer in `tools/` and the kernel loader) and state the current version number and header layout as-implemented (the F2 build added v2 header config metadata — verify against source, not memory). The schedule section is the NEXT version, **additive and version-gated**: a current-version `.neom` with no schedule section must keep loading exactly as today (the D3 kill-switch story depends on this).
- **ISC-2 — The schedule-section byte layout.** Full byte-level spec: section offset/discovery, endianness (match the existing header convention), field widths, `LayerSchedule { version, layer_count, layers[] }`, the `LayerKind` enum encoding (**explicit stable discriminant values** with reserved space for future kinds — an enum that renumbers is ABI drift), the tagged-union `LayerConfig` encoding (`Qwen2LayerConfig` | `GDNLayerConfig` — field-by-field, incl. the GDN state-shape parameters), alignment/padding rules, and the bounds every field must satisfy.
- **ISC-3 — The hand-laid worked examples (the oracle).** TWO, every byte in hex, computed by hand:
  (a) a **2-layer Qwen2-only schedule** — the exact fixture B1's walker-parity build will assert its writer/reader against;
  (b) a **4-layer hybrid** (Qwen2/GDN interleaved) — the fixture B2/B3 inherit.
  Per the byte-oracle-acquisition rule: hand-computed from your own spec, cross-checked field-by-field — never pulled through a summarizing fetch.
- **ISC-4 — Validation mapping.** Map each of the ratified THINK's **7 load-time validation invariants** (§2.2) to the exact bytes/fields it checks and the reject error it raises — so B1's validator is a transcription, not a design task.
- **ISC-5 — The GGUF→schedule mapping (B3 prep, real-source rule).** From the **actual** llama.cpp (`LLM_ARCH_QWEN35`) and HF `transformers` Qwen 3.5 config source: which config/GGUF metadata keys determine the layer interleave (full-attention vs linear-attention per layer — explicit layer-type list? an interval field?), and the mapping rule into `LayerKind[]`. Cite the exact source locations. This is the same read-the-real-source discipline as your Q1 pass — no summarizing fetches.
- **ISC-6 — Open items, named not resolved.** Anything the draft can't fix without code-side confirmation (e.g. a padding choice the loader's alignment dictates) goes in an explicit OPEN-ITEMS section for B1 — a named hole beats a silent guess.

## 2. Scope guard
**Draft spec only.** No code, no `.neom` writer/reader change, no version bump lands now — B1 lands the code + the numbered ADR by lifting this draft. Do not renumber/restructure existing header fields (additive section only). If ISC-1 reveals the current format differs from what the ratified THINK assumed, flag it prominently — that's exactly the drift this dispatch exists to catch early.

## 3. Gate + close
- **Docs-only ⇒ no CI run will exist for the branch** (pushes touching only `docs/`/`Plans/`/`.md` are paths-ignored) — that is correct and expected, not a miss. The bar for this RETURN: **close-gate `scripts/worker-close-gate.sh` run locally (host set + SPDX + self-stat), and the RETURN says "docs-only, no CI triggered" explicitly.** If you touch ANYTHING outside docs/Plans, the branch becomes CI-visible and the green push-CI URL becomes mandatory in the RETURN.
- RETURN: the spec path + the two worked examples' summary (layer counts + total byte sizes) + the ISC-5 mapping citations + the OPEN-ITEMS list.
- **Disk reclaim** after merge.

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-05-neom-schedule-bytespec-prometheus.md` (+ the ratified `docs/THINK-2026-07-05-ot16-forward-graph-design.md`, ADR-028 for the worked-example precedent). **Branch:** `prometheus/neom-schedule-bytespec` off `origin/main` HEAD.
- **One-line:** **Draft the `.neom` layer-schedule section byte-spec with the hand-laid oracle — the B1-prep the OT-16 ratification requires.** Establish the CURRENT header format from real source first (version, layout — flag any drift from the THINK's assumptions); spec the additive, version-gated schedule section byte-by-byte (stable `LayerKind` discriminants + reserved space, tagged-union `LayerConfig` incl. GDN state shape, alignment, bounds); **hand-compute TWO worked examples in hex** — a 2-layer Qwen2-only (B1's parity fixture) and a 4-layer hybrid (B2/B3's) — the example is the normative core (the ADR-028 lesson); map the THINK's 7 validation invariants to exact bytes/errors; and from REAL llama.cpp `LLM_ARCH_QWEN35` + HF source, spec which config keys drive the layer interleave and the mapping rule (cite locations, no summarizing fetches). Name open items, don't guess. **Draft only — no code, no format change; B1 lifts it into the numbered ADR.** Docs-only ⇒ no CI run exists, say so explicitly in the RETURN; run the close-gate host set locally. RETURN the spec + example sizes + citations + open items.
