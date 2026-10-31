<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: B1-1 — Arthur 0.5B model import reproducibility

**Verdict: REPRODUCIBLE + PIN CONFIRMED.** Two independent `neomc` imports from the pinned upstream GGUF produced byte-identical `.neom` and `.vocab` files. The `.neom` BLAKE3 is the dispatch pin exactly.

## Inputs

- Upstream repository: `Qwen/Qwen2.5-0.5B-Instruct-GGUF`
- Upstream revision: `9217f5db79a29953eb74d5343926648285ec7e67`
- GGUF: `qwen2.5-0.5b-instruct-q4_k_m.gguf`
- GGUF size: `491400032` bytes
- Upstream `lfs.oid`, quoted from the raw tree API JSON: `74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db`
- Local SHA-256: `74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db`
- SHA-256 result: **MATCH**

The API JSON and model bytes were downloaded directly from Hugging Face and all digests were computed from local raw files.

## Tool

- `neomc` source commit: `cfeef9c55036f6f9d280aba5f99cac11eba23153`
- Build: `cargo build -p neomc --release` — exit `0`
- BLAKE3 tool: `b3sum 1.8.7`, installed with `cargo install b3sum --locked`

Each import used a separate `neomc.exe` process and a fresh output directory, `run1` and `run2`.

## Reproducibility

```text
fc.exe /b run1\model-qwen2.5-0.5b.neom run2\model-qwen2.5-0.5b.neom
FC: no differences encountered
exit 0

fc.exe /b run1\model-qwen2.5-0.5b.vocab run2\model-qwen2.5-0.5b.vocab
FC: no differences encountered
exit 0
```

Run 1 and run 2 also independently produced the same sizes and BLAKE3 digests:

| Output | Size (bytes) | BLAKE3 |
|---|---:|---|
| `model-qwen2.5-0.5b.neom` | `485489152` | `a32e4a1ba94a850724c15d840f3ae2c44902d9f1f34d8069bac5fcb37e1ccafe` |
| `model-qwen2.5-0.5b.vocab` | `7990862` | `e89741214838ae7af6277122c1093e899138884ea3bea4f83ff5671e76c05265` |

Expected `.neom` pin from the dispatch:
`a32e4a1ba94a850724c15d840f3ae2c44902d9f1f34d8069bac5fcb37e1ccafe`.
The locally rebuilt `.neom` matches it exactly.

## Licence inventory

The raw tree API JSON at the pinned revision lists:

- `LICENSE`: present, `11343` bytes
- `NOTICE`: absent

The raw downloaded `LICENSE` SHA-256 is
`832dd9e00a68dd83b3c3fb9f5588dad7dcf337a0db50f7d9483f310cd292e92e`.

## Repository observations

- RAG health was `healthy` with both Ollama and Qdrant available.
- At tool source commit `cfeef9c55036f6f9d280aba5f99cac11eba23153`, the dispatch's stated `kernel/src/boot_model_logic.rs` path is absent. An exact repository search finds the full expected BLAKE3 in plans/docs, but not in kernel source. This does not change the byte-reproducibility result or the exact match against the pin oracle supplied by the dispatch, but the stale path/pin-location claim should be reconciled separately.

No GGUF, `.neom`, or `.vocab` artifact is stored in Git.
