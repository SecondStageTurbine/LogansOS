<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: B1b boot-module model cross-check — Chad

**Date:** 2026-10-20

**Branch:** `codex/b1b-crosscheck-chad`

**Reviewed:** B1b-only delta `edb620bcb6e30d382b62c178a76e82d15540ea68..68762531f3e4fc5d88cb724ffa15c3820cb8bd1d`

**RAG:** healthy (`ollama=true`, `qdrant=true`). I used it for conceptual discovery, then verified the paths below in the checked-out source.

## Verdict

**APPROVE — no merge-blocking findings.**

**C1: CLOSED at the source/design level.** The boot-module path no longer allocates the 485,489,152-byte staging `Vec`; it hashes the Limine bytes in place, builds owned tensor storage from them, and reclaims the module afterward. The 0.5B heap drops from 768 MiB to 305 MiB, preserving the old path's non-staging headroom. Censuses now expose heap capacity/current/high-water, buddy use/free capacity, admitted capacity, and the load/reclaim boundaries. The OptiPlex census remains the acceptance measurement for the advertised 2 GiB floor; it is not a source-level reason to keep C1 open.

The code reading in dispatch item 7 is correct: B1b ships the QEMU/bare-metal acceptance bundle. It does **not** ship a product `iso --with-model`, and a non-test kernel still lacks the real-LM `sys_generate` implementation.

## Dispatch-item verdicts

### 1. Integrity — **PASS**

`sys_model_load` validates the ordinary syscall envelope (path length/pointer/UTF-8, exact flags, current process) before dispatching `/boot/models/` (`kernel/src/arch/x86_64/syscall.rs:2113-2142`). `load_model` maps only the single-component `/boot/models/<file>` namespace, checks that module suffix against the active pin table before `take`, hashes the entire lease with BLAKE3, and calls `Model::from_neom_bytes` only after `Verdict::Match` (`kernel/src/boot_model_logic.rs:50-98`; `kernel/src/boot_modules.rs:257-304`). Product pins contain only the recorded real-0.5B digest; fixture acceptance exists only under `neo-test`.

No checked-out mutator can change the bytes between hash and build: the module has not been admitted to the allocator, the sole lease has tombstoned it, `/boot` refuses consumables, and Limine is no longer active. The build then reads the same lease synchronously.

An unpinned syscall path returns `EPERM` before the take. Registry lookup is suffix-based, so a differently prefixed Limine path ending in the exact pinned `/models/model-qwen2.5-0.5b.neom` suffix can be selected; it still has to be classified consumable and match the pinned full-file digest. That gives no substitution beyond removal/denial already available to someone modifying the unsigned boot medium, and is consistent with the repository's suffix-based Limine naming contract; I do not treat it as a blocker.

Skipping Limine's `#blake2b` suffix is correct for the stated graceful-degradation requirement. Its default mismatch behavior occurs before kernel entry and can panic the bootloader, while the kernel BLAKE3 gate produces Arthur's damaged state. This is accidental-corruption integrity, not authentication of attacker-writable unsigned media.

The real digest is pinned, but the two-fresh-import reproducibility/provenance exercise remains explicitly pending as B1-1 (`kernel/src/boot_model_logic.rs:24-31`; `docs/B1-boot-module-model.md`). That release evidence should remain open; it is not a counterexample to the implemented hash-before-use gate.

### 2. Lifetime — **PASS**

`Model::from_neom_bytes` validates and owns its metadata vectors. For every weight, `Tensor::from_desc_and_bytes` allocates a buddy block and `copy_nonoverlapping`s the weight into it; the model retains arena keys, ids, metadata, names/config/schedule, and a provenance object—not a module slice (`kernel/src/ai/model.rs:95-197`; `kernel/src/ai/tensor.rs:175-197,425-436,500-504`). Partial-construction failures release already-created arena slots before returning.

The content-hash dedup path hashes the weight while the lease is live and either retains an existing owned tensor or allocates/copies a new one. `FileProvenance::boot_module` is an owned synthetic never-stale marker; it contains an empty inode, not a pointer to module data (`kernel/src/fs/mod.rs:196-221`). `lease.reclaim()` runs only after the constructor returns, on both success and format/tensor failure (`kernel/src/boot_modules.rs:300-321`).

`BootFs::lookup` calls registry `find`; consumable `/models/` modules are refused before an inode can capture their bytes. `/boot/vocab/...` remains ordinary and resident (`kernel/src/boot_modules.rs:325-390`).

### 3. C1 / peak memory — **PASS; C1 CLOSED**

The old NeoFS path keeps its full-file `Vec` alive while constructing the model (`kernel/src/arch/x86_64/syscall.rs:2157-2176`). The new path has no corresponding allocation. `HEAP_SIZE_REAL_BRAIN_05B_BOOT_MODULE = (768 - 463) MiB`, selected only when the boot-module source and the 0.5B tier are both active; larger tiers and feature-off builds retain the prior values (`kernel/src/mm/heap_policy.rs:24-46,71-92`). This preserves the prior path's approximately 305 MiB of heap headroom during construction.

Weight storage is buddy-backed, not heap-backed, and its per-tensor power-of-two rounding is included in `frames_in_use`, not merely the 463 MiB payload estimate. Generation's governed KV tensors are also buddy-backed; heap transients are logits and forward scratch. At Arthur's 512-token prompt plus 32 generated tokens, the 0.5B KV payload is about 13 MiB and logits about 0.6 MiB; neither challenges a 305 MiB heap. The host policy KAT proves only the staging tier changes.

The run-plan's payload table is approximate; the authoritative peak is the maximum real census at `model_before_take`, `model_built`, and `model_reclaimed`, combining `frames_in_use × 4096` with the still-live module bytes. `tensor_block_capacity_pages` reports currently free buddy capacity across primary/admitted zones, while the `frames_in_use` delta captures allocated rounded tensor blocks (`kernel/src/mm/mod.rs:650-678`). The OptiPlex can validate the real artifact, real memmap fragmentation, and the stated 1.5 GiB falsifier; QEMU's 4,144-byte fixture cannot.

### 4. ABI — **PASS**

`EBADMSG` is `-74` in the kernel, libneo, and Arthur's state logic (`kernel/src/errno.rs:69-72`; `userland/libneo/src/lib.rs:276-277`; `userland/neo-arthur/src/model_state_logic.rs:12-31`). Missing/tombstoned maps to `ENOENT=-2`; other load errors map to `LoadFailed`.

The boot branch deliberately bypasses VFS open/read-rights checks because it is not a file: it accepts only a compile-time-pinned module after the same syscall path/flag/process validation. It does not bypass model format or size validation: `validate_neom` requires `total_size == bytes.len()` and `bytes.len() <= NEOM_MAX_FILE_SIZE` before weight construction (`kernel/src/ai/model_logic.rs:361-385,1085-1091`).

### 5. Kill-switch — **PASS**

With `model-boot-module` off, `boot_model_logic`, the registry consumers, `/boot` mount, syscall branch, census additions, and heap-source reduction are absent; the NeoFS branch remains in place. The heap-policy equality KAT exhausts feature combinations and proves source-off equals the old policy.

With `arthur-boot-module` off, the old `/neofs/model-qwen2.5-0.5b.neom` and `/neofs/...vocab` constants, preload-fails-exit behavior, and generic desktop no-model response remain. `arthur_real_brain_boot` still selects the NeoFS path. The new Arthur feature composes the old real-brain feature only for the three B1b acceptance scenarios.

### 6. Failure bank — **PASS**

F1 captures either pre-kernel Limine/screen failure or the complete early `BOOTMOD` census. F2 is explicitly external timing from boot selection to first serial output. F3 is correctly split into a named plan/admission refusal and a separate post-admission lifetime/corruption failure. F4 has expected/got digest and state. F5 logs a census even when `from_neom_bytes` returns `Err`, then reclaims and emits the typed model error. F6 asserts the boot-module scenario skips NeoFS and documents the OptiPlex NVMe `NoValidSuperblock` branch. F7 is the B1a xtask/kernel double gate.

Only the OptiPlex run can exercise the firmware's 485 MB USB read, the real physical memmap/admission shape, real BLAKE3/load duration, real per-tensor buddy fragmentation, and hardware peak. The QEMU fixture proves ordering, states, messages, and discriminators—not magnitude.

### 7. Product question — **CONFIRMED / SCOPE GAP IS HONEST**

The real `sys_generate_impl` is compiled under `neo-test-arthur-real-brain-boot`; the non-test implementation remains the stub. xtask wires the real/fixture boot module only into `arthur_boot_module_{answers,damaged,absent}`, and `docs/B1-boot-module-model.md` explicitly says no product `iso --with-model` is built. Therefore this PR is acceptance-path delivery, not a shipping first-boot product image. A later product slice must move real generation out of the test feature and choose the corresponding product heap/MAX_ORDER feature contract before advertising that image.

## Message and scenario checks

Arthur preloads once, records the exact errno, and degrades instead of exiting. `ENOENT` produces the absent sentence; `EBADMSG` produces the damaged sentence; every subsequent console line receives the same named response (`userland/neo-arthur/src/main.rs:512-525,620-630,2514-2569`). The serial oracle requires each source/census/state breadcrumb and forbids contradictory states. The damaged path cannot reach model construction; the absent path cannot take a module.

## Commands run

- `cargo test -p heap-policy-logic-tests -p arthur-logic-tests` — **PASS** (9 + 73 tests).
- `cargo test -p module-reclaim-logic-tests --lib` — **PASS** (19 tests, including regenerating and hashing the fixture). The first sandboxed aggregate run was denied while the fixture generator wrote its temp directory; the identical package passed outside that filesystem restriction.
- `cargo test -p xtask` — **PASS** (38 tests).
- `cargo check -p neoplutarch-kernel -Zjson-target-spec --features neo-test-arthur-boot-module-answers` — **PASS** (warnings only, all outside the B1b delta).
- `git diff --check origin/claude/b1-arthur-boot-module...HEAD` — one non-functional documentation hygiene warning: an extra blank line at EOF in `docs/ADR-063-arthur-boot-model-provisioning.md:103`.

The user-owned `.gitignore` modification and untracked `.ignore` were left untouched.
