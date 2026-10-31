<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: PR #99 stack-probe F1 re-check (Jarvis)

**Verdict: CLOSED. All five checklist items PASS.** The owned, initialized buffer closes my earlier P2 F1 in RETURN `bf85b939`. No new finding in this narrow re-check.

**Reviewed SHA:** `18f8f4a28cbe74106ab16eeeb1fffa9930b489a7`.
**Comparison:** `5255cb0c..18f8f4a2`, limited to the dispatch's F1 scope.
**Branch/worktree:** `codex/zenbook-model-load-freeze-recheck-jarvis`, `E:\AI Automation\workers\jarvis`.
All source line numbers below refer to the reviewed SHA. This does not re-review the other eight items from relay #31.

## 1. F1 closed — PASS

`kernel/src/test_harness.rs:37176` creates a local `Vec<u8>`. `:37177` reserves capacity for 485,489,152 bytes and the error branch cannot return. `:37184` initializes every byte with `write_bytes(..., 0x5a, ...)` before `:37185` sets the vector length. The first input slice is formed only at `:37193`, using safe prefix indexing. Every listed size is at most the initialized length (`:37171`).

The allocation remains owned by this function throughout the loop; its pointer is not published to another thread, ISR, device, or global. The only hash input is a shared borrow of that allocation. Unlike HHDM(1 MiB), it cannot overlap live kernel data or stacks under the allocator's ordinary exclusive-allocation contract. Timer writes therefore cannot mutate this input. The raw write is within reserved capacity, occurs before any input slice, and is followed by no input mutation during hashing. This removes both the unrelated-memory slice and the initialization/immutability defect in F1.

## 2. Same measured path — PASS

`kernel/src/test_harness.rs:37171` retains all five sizes, including the full 485,489,152 bytes. Each iteration forms one full prefix (`:37193`) and makes exactly one `blake3::hash(bytes)` call inside `inference_stack::run_checked` (`:37194`). There are no incremental `Hasher::update` calls or smaller hash chunks. Buffer reservation and initialization occur before entering the measured stack.

Per-size `report.depth`, thread-stack comparison, digest byte and maximum-depth reporting remain at `kernel/src/test_harness.rs:37195` and `:37203`. The unchanged stack wrapper prepares the fill, invokes the closure on the dedicated stack, and scans after returning while still holding `BUSY` (`kernel/src/mm/inference_stack.rs:257`, `:263`, `:272`, `:279`). The unchanged scan measures the lowest byte differing from the fill (`kernel/src/mm/inference_stack_logic.rs:103`). It remains a watermark of written bytes, including wrapper/interrupt activity, rather than total syscall stack use.

## 3. Allocation and feature scope — PASS

`kernel/Cargo.toml:860` adds `real-lm` only to `neo-test-boot-model-load-stack`; the `real-lm` definition is unchanged (`:837`). I inspected the manifest's transitive feature closure, both with and without default features. This scenario enables `real-lm`, `inference-stack` and `xsave-fpu`, but does not enable `model-boot-module`, `boot-model-preload`, or the real-brain scenario that changes thread-stack size. No feature definition depends on `neo-test-boot-model-load-stack`.

Consequently `kernel/src/mm/heap.rs:38` selects the 768 MiB heap: `real-lm=true`, `model-boot-module=false`; the tier is defined at `kernel/src/mm/heap_policy.rs:27`, selected at `:64`, and preserved at `:88`. The 485,489,152-byte buffer leaves 319,817,216 bytes (about 305 MiB) of heap capacity for other allocations. This establishes sufficient configured capacity, not a guarantee against arbitrary prior exhaustion or fragmentation.

If reservation cannot succeed, `try_reserve_exact` takes the explicit failure branch before either raw write or `set_len` (`kernel/src/test_harness.rs:37177`). The heap allocator returns null on allocation failure (`kernel/src/mm/heap.rs:100`), allowing this fallible reservation to return an error. `fail_with_hex` emits a named `NEO_TEST/1 event=FAIL` and calls `debug_exit(EXIT_FAILURE)` (`kernel/src/test_harness.rs:12343`, `:12351`), which writes the QEMU exit port (`:34322`, `:34329`). There is no allocation-error panic or retry/spin loop in this branch. This is the normal scenario failure path; it was inspected, not fault-injected.

The xtask scenario still selects just its named feature (`tools/xtask/src/main.rs:2844`). Builds that do not enable this scenario acquire no new feature dependency from this patch. Its larger heap and `real-lm` code are confined to builds selecting the scenario.

## 4. Wording — PASS

The NOTE explicitly defines the watermark as the deepest byte written below the inference-stack top, includes interrupt frames, and distinguishes it from a stack-pointer trace and total syscall stack use (`Plans/NOTE-2026-10-23-zenbook-model-load-freeze-s1.md:43`). It says a value below 32 KiB does not establish that the whole syscall fits (`:46`). CP-74 makes the same written-byte/whole-syscall distinction (`docs/concurrency-pitfalls.md:3422`).

The NOTE explicitly calls feature-off a rollback with the full-size IF=0/32 KiB exposure still present, and explains that the optimization and WM polling have separate scope (`Plans/NOTE-2026-10-23-zenbook-model-load-freeze-s1.md:76`). CP-74 retains the feature-off path under **What stays exposed**, stating that the in-syscall load remains and connecting it to the long IF=0/thread-stack hazard (`docs/concurrency-pitfalls.md:3445`). Thus neither document presents feature-off as a repaired full-size load.

The NOTE also documents the input-ownership correction and the author's repeated optimization measurements (`Plans/NOTE-2026-10-23-zenbook-model-load-freeze-s1.md:107`). I have not independently reproduced those numbers.

## 5. Nothing else moved in the kernel — PASS

`git diff --name-status 5255cb0c 18f8f4a2 -- kernel/` lists exactly:

- `kernel/Cargo.toml`: the scenario dependency/comment at `:858`.
- `kernel/src/test_harness.rs`: the input-allocation and prefix-borrow changes in `run_boot_model_load_stack` at `:37162`.

Read the complete kernel diff: no production loader, syscall, scheduler, ISR, inference-stack implementation, or heap policy was edited in this comparison. The comparison also contains the requested NOTE/CP-74 updates and two unrelated dispatch documents outside `kernel/`; neither affects this check.

## Method and limits

Read the new dispatch, my original F1 RETURN, the complete requested diff, and the relevant allocator, feature, stack-measurement and scenario-exit source. Inspected feature closure with a read-only manifest script and checked the diff for whitespace errors. Used direct reads and exact searches; no RAG queries during the rebuild. The required `graft ask --source` discovery call was not used as evidence for pinned source behavior.

No builds, tests, QEMU runs, image generation, or Zenbook disk access. No build artifacts were created, so `cargo clean` was unnecessary. Preserved the pre-existing `.gitignore` modification and untracked `.ignore`; only this RETURN is committed.
