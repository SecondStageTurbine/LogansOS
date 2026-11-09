<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 357 — Chad's review of #90 (Arthur to Q8_0)

Dispatch: `Plans/DISPATCH-2026-11-08-relays-355-358.md`, Section 357, and
`Plans/DISPATCH-2026-11-08-arthur-q8.md`, read from `origin/main`.
Requested routing: GPT-6.1-Sol, high, for the kernel heap policy and model pin.
Reviewed head: `a3cc5f5cd8e45a252ffef14e8207321146488a30` (`claude/arthur-q8`).
RETURN branch: `codex/pr90-review-chad`, based on that head.

**Review: PASS on the five checks below. The required four-shard merge gate is
still pending; this RETURN does not certify it green.** No implementation changes.

- [x] **PASS — source SHA-256 and imported BLAKE3.** Independently fetched the
  [raw LFS pointer at the exact revision](https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/raw/9217f5db79a29953eb74d5343926648285ec7e67/qwen2.5-0.5b-instruct-q8_0.gguf),
  rather than using a summarized response. It declares size `675710816` and oid
  `sha256:ca59ca7f13d0e15a8cfa77bd17e65d24f6844b554a7b6c12e07a5f89ff76844e`.
  Downloaded the GGUF through `/resolve/` at that same revision and ran
  `sha256sum`: exactly that digest, matching `.github/workflows/preview-image.yml:43`.
  Built this head's importer with `cargo build -p neomc --release`, then ran
  `neomc import <gguf> -o <neom> --vocab-output <vocab>`, the same invocation as
  `scripts/preview/build-in-container.sh:18`. The import produced **669,799,936 B**;
  streaming the entire file through Python `blake3` produced
  `1cfd06dc7fe42710f8fab7d337a681ac5527a46e5b2ba6204e929f210055cd4d`,
  exactly `kernel/src/boot_model_logic.rs:39`. The source and revision are
  documented at `kernel/src/boot_model_logic.rs:28`; the module name stays at
  `kernel/src/boot_model_logic.rs:38`. This is an independent local reproduction,
  not reliance on the PR's reported model-pin workflow. `neomc inspect` also
  independently reproduced 291 tensors, 669,763,072 weight bytes and
  `buddy_bytes=1197969408` (1,142.5 MiB). The text vocabulary reproduced its
  unchanged BLAKE3 `e89741214838ae7af6277122c1093e899138884ea3bea4f83ff5671e76c05265`.
  `cargo xtask vocab-bin` also reproduced the 5,010,512-byte NEOVOCB1 and its
  unchanged BLAKE3 `7973d7392444d3faea8d558105c9722c9106483a5a44410fa27fc1c4e6cee015`.

- [x] **PASS — heap arithmetic, default and host oracle.**
  `kernel/src/mm/heap_policy.rs:27` keeps the default **4 MiB**.
  `kernel/src/mm/heap_policy.rs:28` documents
  `ceil(669799936 / 1048576) = 639 MiB`; the old rule's headroom is
  `768 - ceil(485489152 / 1048576) = 768 - 463 = 305 MiB`.
  Thus the file-source tier is `639 + 305 = 944 MiB`
  (`kernel/src/mm/heap_policy.rs:38`) and the boot-module tier is
  `944 - 639 = 305 MiB` (`kernel/src/mm/heap_policy.rs:49`).
  The arithmetic appears in the comments. `kernel/src/mm/heap.rs:57` selects
  this policy from the actual source/model features; unrelated feature floors
  are unchanged. `tools/heap-policy-logic-tests/src/lib.rs:30` checks the
  default and file tier; `tools/heap-policy-logic-tests/src/lib.rs:157` covers
  all 16 source-off flag combinations; `tools/heap-policy-logic-tests/src/lib.rs:172`
  checks the module tier and unchanged larger tiers; the explicit Q8 arithmetic
  oracle is `tools/heap-policy-logic-tests/src/lib.rs:207`. All **10 tests passed**.

- [x] **PASS — staging lifetime and reclaim, with no old-size allocation constant.**
  `kernel/src/mm/module_reclaim_logic.rs:125` derives the reclaim span from
  the actual module endpoints, rounds inward and validates coverage/overlap;
  it does not use a model-size constant. `kernel/src/boot_modules.rs:377` builds
  owned tensors from the lease, then `kernel/src/boot_modules.rs:385` consumes
  the lease to reclaim the module; the mismatch path also reclaims at
  `kernel/src/boot_modules.rs:366`. There is no heap staging vector on that path.
  The file-source path still owns a local vector at
  `kernel/src/arch/x86_64/syscall.rs:3321`, borrows it to build owned tensors at
  `kernel/src/arch/x86_64/syscall.rs:3340`, and drops it on return from
  `model_load_path` (including error returns). Heap deallocation is unchanged
  at `kernel/src/mm/heap.rs:144`.
  **No runtime byte-size constant still assumes 485 MB** in these paths.
  `STAGE_B_05B_NEOM_BYTES` is updated at `tools/xtask/src/main.rs:145`;
  the importer cache name at `tools/xtask/src/main.rs:17794` separates Q8 from
  cached Q4 files. The stack probe uses the new length at
  `kernel/src/test_harness.rs:44961`. The reclaim KAT's `NEOM_05B` is updated
  at `tools/module-reclaim-logic-tests/src/lib.rs:38`: the partial tail is
  1,536 B, with 163,525 whole pages and guaranteed order 16, even after losing
  one extra edge page. All **31 reclaim/pin tests passed**.
  Remaining old-size literals in the heap oracle are explicitly historical
  headroom calculations, not limits on Q8.

- [x] **PASS — VM/scenario RAM and size budget.** No RAM increase is necessary:
  the shipped preview uses `4096M` (`scripts/preview/preview.py:64`), the README's
  `vm-run` uses `4096M` (`tools/xtask/src/main.rs:24733`), and the 0.5B real-model,
  boot-module, installed-model, compositor and load-stack scenarios already use
  `4096M` (`tools/xtask/src/main.rs:15532`, `tools/xtask/src/main.rs:15572`).
  The independent tensor footprint makes the boot-module peak
  `305 + 639 + 1142.5 + 7.6 = 2094.1 MiB`, about 2.05 GiB before the base
  system. `docs/B1-boot-module-model.md:84` updates that calculation and recommends
  3 GiB; the 2 GiB target no longer qualifies. The Zenbook's 32 GB and M710q's
  16 GB exceed it (hardware records: `docs/runbook-baremetal-zenbook-s14.md:18`
  and `docs/runbook-baremetal-m710q-gen9-eu.md:11`). The OptiPlex's RAM is not
  specified in its runbook, so no capacity measurement is claimed for it.
  `tools/size-budget.toml:23` changes only the model-size comment; **no ceiling
  changes**, correctly, because that ISO budget explicitly excludes the model
  and vocabulary. The model-bearing preview's
  [build/package/boot-check run](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42526540075)
  succeeded on the reviewed head; publishing was skipped.

- [x] **PASS — affected output pins remain valid; no re-pinned text.** The
  0.5B QEMU checks at `tools/xtask/src/main.rs:21336` require generation
  breadcrumbs and, with a real artifact, reject rendered control tokens;
  they do not pin the literal chat reply. Their fixture-only reply ID is
  independent of the Q8 model. However, contrary to the PR body's statement
  that only Qwen3.5 has a golden output, **a 0.5B golden-token KAT does exist**:
  `tools/ai-ops-tests/src/lib.rs:7400`, with the assertion at
  `tools/ai-ops-tests/src/lib.rs:7426` and token constant at
  `tools/ai-ops-tests/src/lib.rs:15500`. Independently ran it against the
  downloaded Q8 GGUF, with `phase9-real-quant,phase9-quantized-matmul` enabled:
  **1 passed, not skipped**. Prompt `[9707]` (`Hello`) still generates `[271]`
  (`\n\n`), and the enabled test also checks the f32 reference path agrees.
  Thus no output pin needed changing and there is no new text to quote in the PR.
  The unrelated Qwen3.5 32-token scenario at `tools/xtask/src/main.rs:21689`
  uses another model; it was not rerun locally. Its artifact-absent skip at
  `tools/xtask/src/main.rs:10851` must not be called real-model coverage.
  Q4-specific intermediate-weight/dequantization oracles remain Q4 records;
  they are not output pins for the shipped Q8 model. The PR explicitly leaves
  #84's drift KAT as Q4, with a Q8 re-pin after #84 lands.

Independent commands/results:

```text
cargo test -p heap-policy-logic-tests -p module-reclaim-logic-tests -p xtask
  heap-policy: 10 passed
  module-reclaim: 31 passed
  xtask: 199 passed, 1 ignored

NEO_QWEN25_GGUF=<downloaded Q8 GGUF> cargo test --release -p ai-ops-tests \
  --features phase9-real-quant,phase9-quantized-matmul \
  forward_isc6_real_qwen_cached_top1_matches_llama_cpp_when_available -- --nocapture
  1 passed, 316 filtered out; 16.59 seconds
```

Merge evidence and limits: the exact head's
[four-shard PR CI run](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42526538913)
was still `in_progress`, all four shards running, when checked during this review.
Host/format/lint, product/feature build checks, product RNG/XSAVE boots and decoder
fuzz had succeeded. **Require the final green run before merging**; neither the
green preview boot nor the host KAT substitutes for it. The PR reports its own
worker-close-gate pass; this review did not rerun that full implementation gate.
The green preview checks boot to login, not a real-model chat response or a
bare-metal RAM census.

Non-blocking corrections for the PR author: acknowledge the exercised 0.5B golden
KAT in the PR body. Old size/quantization labels remain in
`userland/neo-arthur/src/main.rs:282` (`QWEN_05B_ALIASES` still accepts the Q4
spelling), `userland/neo-arthur/src/main.rs:642` and
`tools/xtask/src/main.rs:21187` (the matching `model-485MB-plus-compositor`
diagnostic), and `tools/xtask/src/main.rs:17826` (an error message). These do not
allocate or validate against the old size. `docs/B1-boot-module-model.md:80`
also retains old tensor-description prose alongside its new measured footprint.

Environment: the local RAG health endpoint refused the connection; used direct
file reads and exact-symbol searches. Pre-push personal-details scan: PASS for
personal paths, LAN addresses, MAC addresses and email addresses in this RETURN.
`git diff --cached --check`: PASS. Existing unrelated untracked content is excluded
from the commit.
