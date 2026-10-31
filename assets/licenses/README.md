<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Model-weight licences (B1b)

Images that carry a model as a boot module (the `arthur_boot_module_*` bundles,
and the OptiPlex B1 run) also carry that model's licence under `/licenses/`.
The weights are third-party content. They are not LogansOS source and not
MPL-2.0; `NOTICE` (section MODEL WEIGHTS) lists them.

| File | Covers | Source | sha256 |
|---|---|---|---|
| `Apache-2.0.txt` | Qwen2.5-0.5B-Instruct (and 1.5B) weights, Apache License 2.0 | `https://www.apache.org/licenses/LICENSE-2.0.txt`, fetched raw with `curl` (2026-10-20), byte-for-byte | `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30` |

**Checked by B1-1** (`Plans/RETURN-2026-10-20-b1-1-model-import-janus.md`, upstream revision
`9217f5db79a29953eb74d5343926648285ec7e67`, read from the raw tree API):

1. The upstream repository has **no `NOTICE`**, so Apache-2.0 §4(d) adds nothing.
2. The upstream `LICENSE` is 11,343 bytes, sha256
   `832dd9e00a68dd83b3c3fb9f5588dad7dcf337a0db50f7d9483f310cd292e92e`. The apache.org text here is
   11,358 bytes: the same licence, but **not the same bytes**. Shipping upstream's exact file is a
   follow-up for a seat that can reach Hugging Face (this session cannot). When it lands, it is
   committed as `Qwen2.5-LICENSE`, and `MODEL_LICENSE_FILES` in xtask points at it.
