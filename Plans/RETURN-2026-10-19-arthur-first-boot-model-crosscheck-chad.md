<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Arthur first-boot model cross-check — Chad

**Date:** 2026-10-19

**Branch:** `codex/arthur-first-boot-model-crosscheck-chad`

**Reviewed design:** `docs/THINK-2026-10-19-arthur-first-boot-model.md` at `4b058002` plus the PL-ratified
`Plans/DECISION-2026-10-19-arthur-first-boot-model.md` on `origin/main`

**RAG:** healthy (`ollama=true`, `qdrant=true`) at review time; used for discovery, then every finding below was
re-read in the source or raw upstream material.

## Verdict

**CHANGES REQUIRED before B1 merges.** The central delivery premise survives: the kernel has xHCI and several USB
class drivers, but no USB mass-storage class driver and no USB-backed `BlockDevice`, so it cannot open a NeoFS
partition on the boot stick after Limine hands over. A Limine boot module remains the smallest offline delivery path.

The ratified copy-then-reclaim shape does **not** survive as written:

1. the module is additive to a fully physically mapped 768 MiB heap, not a replacement for today's heap reservation;
2. Limine's executable/module memmap type is not a page-aligned reclaim grant, and the current single-zone buddy
   cannot accept those frames through `free()`;
3. the proposed tier rule omits separately allocated tensor residency and uses summed `USABLE` memory even though
   the allocator manages only the largest `USABLE` extent; and
4. the bare-metal bank misses a deterministic module-count limit, cannot time pre-kernel loading with kernel
   timestamps, and predicts the wrong NeoFS absence branch for the documented OptiPlex setup.

The PL should amend Decision 1 before B1 merges. Either pull a smaller/lazy heap and an explicit external-region
admission design into B1, or restrict B1 to a measured high-RAM profile and stop claiming memory equivalence or
reclaimability until that work lands. Decision 2 (ship 0.5B first) remains sensible, but its 1792 MiB threshold is not
supported.

## Surviving findings

### C1 — B1 adds about 470.6 MiB to the load-time peak; it does not replace today's file-buffer reservation

**Decision affected:** D1; also the Q2 1792 MiB threshold. **Severity:** blocker.

The current heap is not demand-paged. `heap::init` allocates and maps every page in `HEAP_SIZE` before the allocator
starts (`kernel/src/mm/heap.rs:116-170`). The 0.5B feature fixes that size at 768 MiB
(`kernel/src/mm/heap_policy.rs:24-27,42-55`). Today's NeoFS path fills a 485,489,152-byte `Vec` inside those already
reserved heap frames (`kernel/src/arch/x86_64/syscall.rs:2060-2079`), then constructs the model
(`kernel/src/arch/x86_64/syscall.rs:2131-2150`).

The resident weights are **not** held in that `Vec` or heap. For each validated weight, `Model::from_neom_bytes`
calls `share_or_allocate_weight` (`kernel/src/ai/model.rs:123-177,454-482`), which calls
`Tensor::from_desc_and_bytes`; `HostTensorStorage::allocate_from_bytes` obtains a power-of-two block directly from the
frame buddy (`kernel/src/ai/tensor.rs:176-197,425-436`; `kernel/src/ai/tensor_logic.rs:168-185`). The real importer
inventory records 485,452,288 weight bytes (`docs/bible.md:288`).

B1 explicitly keeps the existing compile-time 0.5B heap and defers runtime heap work to B2. Its lower-bound live set
while the last weights are copied is therefore:

| Component | Bytes | MiB |
|---|---:|---:|
| Eager 0.5B heap | 805,306,368 | 768.000 |
| `.neom` module | 485,489,152 | 462.999 |
| Minimum tensor payload copied outside the heap | 485,452,288 | 462.963 |
| `.vocab` module | 7,990,862 | 7.621 |
| **Lower bound before kernel/base modules/page tables/framebuffer/KV/etc.** | **1,784,238,670** | **1,701.583** |

At the THINK's 1792 MiB threshold, that leaves only 90.417 MiB for everything omitted from the table. The real number
is higher because each tensor allocation rounds independently to a buddy power of two, and because the kernel, four
existing user modules, page tables, framebuffer, user processes, caches and forward state are live. The physical
OptiPlex proof only establishes the path on a machine whose boot log reports 15,976 MiB usable
(`Plans/evidence/ot41-baremetal-arthur-reply-smpfix-2026-07-19.serial.log:19-27,123-150`). It does not establish a
roughly 2 GiB profile.

The module can reduce **heap occupancy** relative to the file path, but not the physical reservation while the heap is
eagerly mapped. A correct B1 close must do one of the following before claiming parity: shrink/lazily back the heap for
the module source; stream into final storage with bounded scratch; adopt the module in place under an approved ARENA
change; or publish a larger measured minimum-RAM requirement. A boot census must distinguish module bytes, mapped
heap capacity, heap used, managed buddy free pages, and tensor block capacity.

### C2 — `EXECUTABLE_AND_MODULES` is not a reclaimable allocator zone, and current APIs permit a post-reclaim borrow

**Decision affected:** D1's “returns the module's frames to the allocator.” **Severity:** blocker.

The raw Limine protocol gives type 6 the name `LIMINE_MEMMAP_EXECUTABLE_AND_MODULES`, but says those entries are
illustrative rather than authoritative for locating either object. It guarantees 4096-byte alignment and non-overlap
only for usable and bootloader-reclaimable entries; non-usable entries, explicitly including executable/modules, need
not be aligned and may overlap. Sources:

- `https://raw.githubusercontent.com/limine-bootloader/limine-protocol/trunk/PROTOCOL.md`, “Memory Map Feature”;
- resolved crate constants `limine-0.6.3/src/memmap.rs:4-12` (`MEMMAP_BOOTLOADER_RECLAIMABLE=5`,
  `MEMMAP_EXECUTABLE_AND_MODULES=6`).

Consequently, checking only `type_ == MEMMAP_EXECUTABLE_AND_MODULES` is insufficient. An entry can cover the kernel,
other live modules, partial edge pages, or overlapping descriptions. The 0.5B length itself is not page-aligned
(`485,489,152 % 4096 == 2560`). Reclaim must be driven from the Module feature's exact file range, intersected with the
memmap, and must retain every edge or overlapping page. It also must prove disjointness from the executable and every
other live module.

More directly, the current allocator has no legal operation for this. `BuddyAllocator::new` selects one — and only
one — largest `MEMMAP_USABLE` extent (`kernel/src/mm/frame.rs:127-195`). `free()` requires a frame previously allocated
by that allocator (`kernel/src/mm/frame.rs:340-350`), while its coalescer subtracts the allocator's single
`base_phys` (`kernel/src/mm/frame.rs:301-337`). Passing a module frame to it can underflow the offset/coalescer math,
poison the free lists, and decrement `FRAMES_IN_USE` for a frame that was never counted. B1 needs a checked
**admit-external-region/second-zone** operation with separate accounting; it must not loop over module pages and call
the existing `free()`.

Finally, `MODULE_REQUEST` remains static and `find_module_by_path_suffix` can manufacture a new `'static` slice for
any matching module at any later time (`kernel/src/main.rs:191-206`). That is a live reacquisition capability even if
the load function drops its local slice. Reclaim requires a consuming/tombstoned module registry (and auditing direct
`MODULE_REQUEST` users), so the model module cannot be looked up after ownership transfers. The current comment that
module bytes live for the kernel's entire lifetime is the existing contract; B1 must deliberately replace it rather
than silently violate it.

Reclaim is possible in principle, but only after all three proofs exist: page ownership, allocator admission, and no
remaining/reacquirable borrow. The decision's current type-check wording proves none of them.

### C3 — `heap(T) + R <= U` is not an admission rule for this allocator or model representation

**Decision affected:** D2/B2 tier rule. **Severity:** blocker for B2 and for any B1 minimum-RAM claim.

There are two independent counterexamples.

1. **`R` omits the resident weights.** The THINK describes it as kernel non-heap memory, page tables, processes,
   framebuffer and desktop, but the arena's CPU tensors are also non-heap buddy allocations
   (`kernel/src/ai/tensor.rs:425-469`). For 0.5B their payload alone is 485,452,288 bytes; for larger tiers it grows
   with the model. It is additionally rounded per tensor to a power-of-two page block
   (`kernel/src/ai/tensor_logic.rs:168-185`). A tier-independent 1024 MiB reserve cannot be justified by a census that
   ignores that tier-dependent term. The 1.5B reserve can be consumed by weights alone before the kernel/desktop
   reserve is counted.
2. **`U` is not allocatable capacity.** `mm::init` logs the sum of every `MEMMAP_USABLE` entry as `U`
   (`kernel/src/mm/mod.rs:148-166`), but `BuddyAllocator::new` manages only the largest such entry
   (`kernel/src/mm/frame.rs:127-195`). A synthetic machine with two 1 GiB usable extents has `U=2048 MiB`, so the rule
   selects 0.5B; after the eager 768 MiB heap, the managed 1 GiB zone has at most about 256 MiB left and cannot hold
   485 MiB of weight payload. More starkly, two 1.5 GiB extents give `U=3072 MiB`, so the rule selects 1.5B, but boot
   panics while trying to map its 1792 MiB heap from a largest extent of only 1536 MiB.

Thus two machines with the same `U` may correctly get different outcomes because their extent layouts differ; the
THINK lists that as a falsifier when it is actually a property of the current allocator. A sound rule must either make
the buddy multi-zone first, or admit on **managed allocatable pages after mandatory boot reservations**, with a
tier-specific measured term for rounded resident tensors, heap, module-at-peak, KV/scratch and safety margin. The host
KAT must include fragmented memmaps, not only scalar `U`.

### C4 — The F1–F6 bank misses one deterministic pre-kernel limit and two discriminators do not match the run

**Decision affected:** B1 OptiPlex acceptance. **Severity:** high.

1. **Missing F7: module-count overflow.** The kernel has a fixed eight-entry early-boot extent array and panics when
   Limine reports more (`kernel/src/main.rs:136,575-595`). The normal config already has four modules
   (`boot/limine.conf:25-39`), and xtask may inject MT7601U firmware and the std-real smoke module
   (`tools/xtask/src/main.rs:4630-4682`). Model + vocab therefore leave zero or two slots depending on the build. The
   B1 dispatch literally says to stage `.neom`, `.vocab` **and licence files as Limine modules**; that can exceed eight
   before model code runs. Add an xtask manifest-count hard failure, raise/prove the bounded kernel capacity if needed,
   and log module count/path/size before the array is populated. Do not rely on a later model breadcrumb.
2. **F2 cannot measure what it claims.** Limine has completed every module read before the first kernel instruction.
   Timestamps from `kernel entry` to `module seen` measure kernel work, not USB/firmware/Limine load time. A timeout in
   that interval yields no kernel timestamp at all. Capture externally timestamped serial/screen transitions from
   power-on through Limine handoff (or a Limine build that emits timed loader progress) and record the wall-clock
   watchdog boundary before the run.
3. **F6 predicts the wrong branch on the documented OptiPlex.** Detaching the SATA NeoFS disk does not make
   `device_count()==0`; the internal NVMe remains registered. `find_neofs_block_device` will probe it and return
   `NoValidSuperblock`, not `NoBlockDevice` (`kernel/src/fs/mod.rs:675-712`). Both branches continue, but the expected
   breadcrumb must be `boot-neofs: no valid NeoFS superblock; boot continues`
   (`kernel/src/fs/mod.rs:768-801`) for the runbook's current NVMe layout.

The existing F3 should also be split: “wrong memmap type” and “allocator rejected/corrupted by an external zone” are
different failures. The pre-run proof for the latter is a host KAT over unaligned/overlapping module ranges and a
multi-zone/admission allocator test, not only a before/after frame counter.

## Dispatch item verdicts and checks performed

### 1. USB claim — **UPHELD**

Checked `kernel/src/drivers/block/`, `kernel/src/drivers/usb/`, xHCI enumeration/binding, block registration, NeoFS
device discovery, Limine requests, and firmware interfaces. The only block drivers are AHCI, NVMe and virtio
(`kernel/src/drivers/block/mod.rs:10-16,109-121`). USB registers HID, CDC-ECM and optional MT7601U class drivers, but
no class-08/SCSI/Bulk-Only driver (`kernel/src/drivers/usb/mod.rs:37-53,237-250`). An unclaimed xHCI device is torn down
(`kernel/src/drivers/usb/xhci.rs:1466-1500`), so enumeration is not a storage path. No post-handoff UEFI file/block
service is retained. A Limine buffer can carry a named boot file, but it is not an arbitrary post-boot read path.

### 2. A1 copy then reclaim — **REFUTED AS WRITTEN**

Checked current file-buffer lifetime, tensor copies, eager heap mapping, boot module extents/HHDM, Limine's raw memmap
contract, buddy bootstrap/free/coalescing/accounting, and all module lookup sites. Findings C1 and C2 must be resolved.
A2 can remain deferred only if A1 gains a smaller/lazy heap plus safe external-zone admission; otherwise some memory
work the THINK assigns to B2 is a B1 prerequisite.

### 3. Tier rule — **REFUTED**

Checked heap policy, heap mapping order, the exact 0.5B import inventory, tensor storage allocation/rounding, memmap
census and buddy bootstrap. Finding C3 gives concrete counterexample profiles. The 1024 MiB reserve and all three
published thresholds are unproven.

### 4. Integrity — **DESIGN SOUND WITH A REQUIRED GATE; Limine suffix is optional defense-in-depth**

Checked the actual importer/writer. `neomc` walks the GGUF tensor vector in file order
(`tools/neomc/src/gguf_import.rs:149-208`), the builder stores ordered `Vec`s and emits into a zero-filled buffer in
that order (`tools/neomc/src/writer.rs:261-332,391-406`), and the tokenizer sidecar writes tokens/merges in indexed
vector order (`tools/neomc/src/gguf_import.rs:1040-1068`). No timestamp, randomness, unordered output map or
uninitialized padding is on this path. I found no nondeterminism.

That is source evidence, not the prerequisite's byte proof. B1 still must import the **same pinned raw GGUF twice** in
fresh output directories/tool invocations, compare both `.neom` and `.vocab` byte-for-byte, and pin the importer
version plus upstream commit/LFS SHA-256. Hashing every module byte during the copy/load and comparing against an
immutable pinned BLAKE3 before publishing or executing the model is sufficient to detect accidental damage; Limine's
suffix is an earlier optional check, not a correctness dependency. Raw Limine `CONFIG.md` specifies that suffix as
**BLAKE2b**, not BLAKE3 (`https://raw.githubusercontent.com/limine-bootloader/limine/v9.x/CONFIG.md`, “Paths”), so B1
must generate the right algorithm if it enables it. Neither check authenticates user-modifiable unsigned media when
an attacker can replace both payload and expected digest; the decision promises damage detection, not Secure Boot.

### 5. Licences — **UPHELD FOR 0.1; one 3B note**

Queried the raw upstream repository inventories at pinned heads:

- `Qwen/Qwen2.5-0.5B-Instruct-GGUF@9217f5db79a29953eb74d5343926648285ec7e67`: Apache-2.0, `LICENSE`, no `NOTICE`;
- `Qwen/Qwen2.5-1.5B-Instruct-GGUF@91cad51170dc346986eccefdc2dd33a9da36ead9`: Apache-2.0, `LICENSE`, no `NOTICE`;
- `Qwen/Qwen2.5-3B-Instruct-GGUF@7dabda4d13d513e3e842b20f0d435c732f172cbe`: Qwen Research License,
  `LICENSE`, no separate `NOTICE`/use-policy file.

Apache-2.0 §4(d) therefore adds no upstream NOTICE file for the two 0.1 candidates; staging the full Apache-2.0 text
and a project model-weights NOTICE section is the right shape. `/licenses/` is consistent with the image's existing
font practice (`tools/xtask/src/main.rs:50-55,4684-4691`).

If 3B is reconsidered, its pinned license §3(c) requires this exact attribution inside a distributed **Notice text
file**: `Qwen is licensed under the Qwen RESEARCH LICENSE AGREEMENT, Copyright (c) Alibaba Cloud. All Rights
Reserved.` Arthur's `/model` display is additive and cannot substitute for that file. Its §4 “Rules of use” is part of
the license itself; there is no separate upstream use-policy artifact at the pinned GGUF head. This does not affect D3
because 3B is excluded from 0.1.

### 6. Failure bank — **REFUTED / INCOMPLETE**

Checked F1–F6 against early boot order, module capacity, the runbook's device layout, NeoFS error classification and
what each proposed breadcrumb can temporally observe. Finding C4 must be folded into the run plan before the first
OptiPlex iteration.

### 7. PL decisions — **D1 amendment required; D2 threshold withdrawn; D3–D5 survive**

- **D1:** boot-module delivery survives; “then returns the module's frames” and “no worse than today” do not. Amend
  with C1/C2's prerequisites and a supported RAM profile before B1 merges.
- **D2:** 0.5B-first survives. Withdraw the 1792 MiB fit claim and scalar-`U` rule pending C3.
- **D3:** survives; 3B stays out of 0.1.
- **D4:** survives for accidental-corruption detection if the digest gate runs before model publication/use and the
  two-run reproducibility prerequisite passes.
- **D5:** survives; USB-only persistence remains honestly absent.

## Required B1 amendments / close evidence

1. Record the supported first-boot RAM profile. If it includes about-2-GiB machines, replace the eager 768 MiB heap
   before B1; a 16-GiB-only OptiPlex pass cannot prove the lower tier.
2. Add a checked module-range planner: exact Module-feature range, type-6 containment, page alignment, executable and
   other-module overlap exclusion, retained edge pages, overflow checks.
3. Add explicit external-region/second-zone allocator admission with separate free/used accounting; never call the
   present `BuddyAllocator::free` on Limine-owned frames.
4. Make model-module lookup consumable/tombstoned and prove no direct or indirect `'static` slice can be obtained
   after reclaim. Cover success and every partial-load/hash-failure path.
5. Replace the reclaim witness with capacity accounting that sees newly admitted pages and cannot underflow
   `FRAMES_IN_USE`; test allocate/use/free/reallocate from the admitted range.
6. Gate the staged module count against `MAX_BOOT_MODULE_EXTENTS`, including optional firmware/test modules and the
   final decision on whether licence files are ordinary ISO files or Limine modules.
7. Repair F2/F6 and add F7 as described in C4.
8. Preserve the real-artifact two-import equality test and pin both upstream and importer inputs in the RETURN.

No code was changed and no build artifacts were produced; `cargo clean` was not needed.
