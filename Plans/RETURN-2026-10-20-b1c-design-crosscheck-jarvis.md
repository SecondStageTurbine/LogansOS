<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: B1c product-generation design cross-check (Jarvis)

**Date:** 2026-10-20

**Reviewed plan:** `Plans/NOTE-2026-10-20-b1c-product-generation-s1.md`

**Code authority:** `origin/main` at `2f6f5606`

**Scope:** design review only; no B1c implementation review or runtime gate

**Verdict: REFUTE / bounce the plan before more code.** The product promotion is directionally right, but the plan does not yet define a closed production cfg surface, bounds only one of the three promoted generation ABIs, uses a circular token oracle, and treats two unmeasured global policy changes as established. The 54 MiB KV estimate is correct; the conclusion that the 305 MiB heap is sufficient is not yet proved.

## Findings

### F1 — BLOCKER: the bounds do not cover the product surface the plan promotes

The scope says `real-lm` owns the generate syscalls' real arms, plural (`NOTE:27-29`), but the bounds section constrains only `sys_generate` (`NOTE:34-40`). On `origin/main`:

- `sys_generate_constrained_impl` caps `max_new`, sequence count, and candidate-token count, but has no prompt cap (`kernel/src/arch/x86_64/syscall.rs:3102-3139`). It then allocates/copies sequence descriptors and candidate tokens before model lookup (`:3166-3208`) and re-prefills the entire growing inventory context on every token (`:3214-3231`).
- `sys_generate_masked_impl` has no prompt cap at all (`:3293-3334`), allocates/copies the mask before model lookup (`:3336-3366`), and runs full-context `qwen_inventory::next_logits` (`:3370-3385`).
- The ordinary path currently copies the prompt only after model lookup (`:2990-3001`). A new cap placed with the existing scalar checks at `:2961-2965` can precede heap allocation, but the plan does not pin that placement or share it with the other two ABIs.

**Required amendment:** define one product generation-limit policy used by all three syscall arms, before any `Vec`, user-buffer copy, or model-dependent allocation. For masked generation the effective `max_new` is one. For constrained generation retain the candidate limits, add the same prompt limit, and either move it to the persistent-KV core or explicitly declare the O(N^2) product path acceptable. Test allocation-call deltas, not merely equal heap usage before/after: the allocator has explicit test counters (`kernel/src/mm/heap.rs:54-57,90-106`), while a transient allocation followed by free can leave a net-zero census.

### F2 — BLOCKER: the proposed token discriminator is not independent

The new scenario would compare product `sys_generate` tokens with tokens “the existing ... KAT produces” (`NOTE:76-89`). That KAT calls `generate_inventory_greedy` twice on the same model and prompt, changing only `KvGenMode` (`kernel/src/arch/x86_64/syscall.rs:2863-2921`). The test asserts equality, length, and non-degeneracy, but no committed expected token sequence (`kernel/src/test_harness.rs:7257-7335`). The source itself says shared attention-math bugs remain outside this differential (`syscall.rs:2686-2696`). Calling the same production core through a second syscall route is a route/cfg/copyout oracle, not an independent byte oracle.

**Required amendment:** keep this scenario as a promotion-route discriminator, but give correctness an independently produced fixed token vector (for example, a host reference implementation over the committed tiny inventory model, with the expected bytes committed) and a negative control that proves the route assertion can fail. Do not label equality with the existing KV differential “independent.”

### F3 — HIGH: the cfg promotion is not a closed dependency audit, and test behavior will leak if the obvious gates are widened

The plan's table names the large pieces, but `origin/main` has cross-gate dependencies that require an explicit closure:

- `qwen_inventory` is gated separately in `kernel/src/ai/mod.rs:79-86`; constrained/masked argmax re-exports are separately gated in `kernel/src/ai/generate.rs:15-25`; `generate_errno` and `is_stage_b_inventory_model` have their own feature families in `syscall.rs:3564-3580,3638-3646`.
- `sys_generate_impl` calls `log_forward0_top5` unconditionally (`syscall.rs:3072-3074`), while that helper exists only under `neo-test-arthur-real-brain-boot` (`:3621-3636`). Widening only the real syscall/core gates to `real-lm` therefore fails to compile.
- The per-token `sample step=... tok=...` serial breadcrumb is unconditional inside the real arm (`:3075-3082`), contradicting the plan's promise that breadcrumbs remain on the test feature (`NOTE:30-33`). Constrained generation similarly prints every chosen token (`:3236-3246`).
- `ReprefillBaseline` is not individually test-gated (`syscall.rs:2697-2707`); it is test-only today only because the enclosing types are. Adding `real-lm` to those enclosing cfgs compiles the O(N^2) baseline and its arms into the product even though production does not select it (`:2715-2795`).

**Required amendment:** add a cfg-ownership matrix for every symbol in the real syscall call graph. Separately gate the baseline variants/arms and all breadcrumbs, and add both (a) the exact product feature combination compile and (b) lean `real-lm`-off compile/string/symbol checks. “Helpers” is not specific enough for this seam.

### F4 — HIGH: order 16 is mechanically supported, but availability and necessity are unproved

I found no fixed-order-14 arithmetic in allocation, free, coalescing, admitted-zone routing, or accounting. `NUM_ORDERS` derives from `MAX_ORDER`; splitting and coalescing use the requested order (`kernel/src/mm/frame.rs:67,279-343`); allocation searches primary then admitted zones (`:456-465`); frees route by ownership and subtract `1 << order` pages (`:467-518`); admission decomposes with the live `MAX_ORDER` (`:535-581`). Thus the second zone and `FRAMES_IN_USE` can represent order 16.

That is not enough to make the policy safe:

- Raising the primary-zone alignment from 64 MiB to 256 MiB changes `Zone::from_memmap` trimming (`frame.rs:132-168`). Depending on the largest usable region's base, it can discard up to 192 MiB more boot RAM than order 14. The plan has no minimum-RAM/alignment analysis for this global side effect.
- An admitted module zone uses a base aligned down to the maximum block and only emits naturally aligned blocks wholly inside the reclaimed span (`module_reclaim_logic.rs:169-218`; `frame.rs:565-577`). A 463 MiB span can contain an order-16 block, but is not guaranteed to: guaranteeing a 256 MiB-aligned subspan for arbitrary placement needs almost 512 MiB. Limine placement therefore determines whether the reclaimed zone can satisfy an order-16 tensor.
- B1a's reclaim scenario exercises order 0 and order 8 only (`kernel/src/test_harness.rs:11425-11440,11521-11546`). The generic max-order scenario accepts OOM as PASS (`:11565-11605`), so neither is a non-vacuous proof for order 16 in the admitted zone.
- Tensor storage rounds each tensor to the next buddy order and asks for one contiguous block (`kernel/src/ai/tensor_logic.rs:168-184`; `kernel/src/ai/tensor.rs:425-436`). Main says only that two tensors exceed 64 MiB (`kernel/src/mm/frame.rs:56-66`); it does not establish that either exceeds 128 MiB and therefore needs order 16 rather than 15.

**Required amendment:** record the pinned inventory's largest tensor byte count and derived order; select the minimum sufficient order. Add a non-skipping test that reclaims a realistically placed model span, allocates and frees the required order from that admitted zone, and checks `FRAMES_IN_USE`, refused frees, and full coalescing. Include worst-case primary trim in the RAM floor. Preserve both CP-29 guards during the change: main documents that the wide heap/order-16 boot previously exposed real-silicon AP allocator contention, closed by serialized AP bring-up plus the independent shootdown-ready mask (`docs/concurrency-pitfalls.md:1054-1081`).

### F5 — HIGH: “every kernel thread” understates the 256 KiB blast radius, and the proposed census cannot measure it

`STACK_SIZE` is one global constant (`kernel/src/sched/thread.rs:55-65`). It backs:

- ordinary kernel threads via `Thread::new` (`:205-248`);
- every user thread's kernel/syscall stack via `Thread::new_user` (`:330-362`) and the browser thread variant (`:420-480`);
- one idle thread per CPU, allocated during scheduler install (`kernel/src/sched/scheduler.rs:457-484`).

The bootstrap threads are the exception because `adopt_current` owns no boxed stack (`thread.rs:279-301`). On the real 12-CPU OptiPlex documented by main (`Plans/CLOSE-REPORT-2026-06-04-phase-5.md:40-42`), the production desktop has a hard lower bound of 14 owned stacks while Arthur is running: 12 idle threads, neo-init, and neo-arthur (the desktop spawns Arthur at `userland/neo-init/src/desktop_session.rs:1648-1654`). At 256 KiB that is 3.5 MiB total; compared with 32 KiB, the added reservation is 224 KiB = 56 pages per thread, or 784 pages / 3.0625 MiB for those 14 alone. Any writeback or later worker thread adds another 56-page delta.

The plan says it will measure “frame cost ... in the census” (`NOTE:104-107`), but the entire heap is allocated and mapped frame-by-frame before the allocator starts (`kernel/src/mm/heap.rs:124-179`). Later stack boxes consume already counted heap capacity, so `FRAMES_IN_USE` will not rise per stack. Measure `heap::used_bytes()`/allocator high-water instead.

I found no 32 KiB assumption in the context-switch boundary: TSS.rsp0 and per-CPU rsp use the selected thread's actual slice top (`thread.rs:311-320`; `scheduler.rs:936-946`). IST stacks are separate 20 KiB static per-CPU objects (`kernel/src/arch/x86_64/gdt.rs:15-20,30-56,88-103`). I also found no guard mapping around boxed kernel stacks; they are zeroed heap buffers. So the selector does not break TSS/IST arithmetic, but it pays an 8x tax for unrelated user and idle threads without adding overflow containment.

**Required amendment:** prefer a dedicated inference syscall/worker stack, or a separately justified large-stack class for generate-capable threads, over changing the global thread ABI. An Arthur-only stack flag is not sufficient by itself because any process may call the syscall; the kernel must switch stacks or enforce the corresponding capability/policy. Whichever design wins needs measured stack high-water/canaries, a guard-page decision, heap-used accounting at the real desktop thread count, and a 12-CPU boot check. This is also a CP-29 lens: larger idle allocations widen per-CPU initialization even though serialization prevents AP-vs-AP allocation concurrency.

### F6 — HIGH: 54 MiB KV is correct; “therefore it fits 305 MiB” is unsupported

For the 0.5B configuration, main pins 24 layers, 2 KV heads, and head dimension 64 (`kernel/src/ai/forward_logic.rs:28-34`). Each layer allocates both key and value as `f32[kv_len * max_context]` (`:330-350`). At 2,304 tokens the exact allocation is:

`24 * 2 (K+V) * 2 * 64 * 2,304 * 4 = 56,623,104 bytes = 54 MiB`.

`InventoryDecodeState::new` does reject a zero/oversized context before constructing the cache (`kernel/src/ai/qwen_inventory.rs:98-117`), so that part of the plan checks out.

The fit claim still lacks the other live allocations. The attention path materializes the active key/value and a sequence-wide query (`forward_logic.rs:757-759`); at 2,304 tokens the query alone is 7.875 MiB, and `gqa_attention_last` converts query/key/value to additional byte vectors before allocating output (`:2080-2121`). Other normalization, quantization, matmul, and FFN helpers also allocate scratch. Main's own B1 document says the observed 1,857,799-byte heap high-water came from the 4,144-byte fixture and “is not a 0.5B measurement” (`docs/B1-boot-module-model.md:67-73`).

**Required amendment:** make the 305 MiB claim conditional until a real 0.5B run records peak kernel-heap use at the worst accepted 2,304-token context, including forward scratch, kernel stacks, and other kernel-heap consumers, with an explicit safety margin. Account for Arthur's user-space vocab separately in the total-RAM census. If running a 2,304-token forward is impractical before merge, add an allocation-shape KAT/formal peak budget and retain the real-model acceptance as the release gate; do not cite KV alone as the heap proof.

### F7 — MEDIUM: the kill-switch tests prove selected values and behavior, not bit-for-bit identity

Host selector KATs plus existing desktop scenarios are good requirements (`NOTE:67-74`). Main already host-tests that boot-module-off preserves the prior heap policy for every feature combination (`tools/heap-policy-logic-tests/src/lib.rs:156-165`). But equality of three constants and a green QEMU scenario does not prove a bit-for-bit identical ELF. This matters because the promotion touches shared cfg families and logging call sites (F3).

**Required amendment:** either narrow the claim to “policy- and behavior-equivalent” or add a reproducible artifact comparison for the exact plain-`desktop` feature set, plus negative symbol/string checks showing inventory generation and its breadcrumbs are absent. Keep the runtime `SYS_GENERATE -> EINVAL` guard as a separate behavioral proof.

### F8 — HIGH: the product-image and beginner failure-state design has two holes

First, the `xtask` requirement contradicts itself: it says the command refuses an unpinned length **or digest**, then says xtask has no BLAKE3 and checks only length (`NOTE:46-54`). Main's xtask is deliberately dependency-free (`tools/xtask/Cargo.toml:11-13`) and the B1b staging path copies/writes the supplied model and vocab without hashing (`tools/xtask/src/main.rs:4390-4401,4942-4960`). Kernel-time model hashing is valuable fail-closed defense, but it is not “refuse before writing the image.” A beginner should not spend a boot discovering that the builder knowingly accepted bad bytes.

Second, the proposed damaged-vocab state does not compose with current Arthur state. `try_load_sidecar` returns only `bool` and collapses open/read/parse failures (`userland/neo-arthur/src/main.rs:2573-2629`). The desktop expression short-circuits model preload when vocab is false (`:519-525`), while `boot_model_state` starts as `Ready` and is only updated by `preload_boot_model` (`:2503-2538,2552-2570`). Without a designed vocab-result-to-state path, a damaged vocab can avoid taking the model as intended but still fall through to the generic notice rather than the promised damaged sentence.

The normal Arthur ordering itself is sound: it loads vocab, preloads the model, and only enters the desktop session afterward (`main.rs:493-540`); `model()` lazily obtains or loads a handle before generation (`:2638-2663`). For a direct syscall made without a valid loaded model, `model_lookup` returns `EBADF` (or `EPERM` for a model handle without USE rights) before prompt copying (`kernel/src/arch/x86_64/syscall.rs:2059-2073,2990-2999`). This should nevertheless be a product test, including stale/revoked handles, because `SYS_GENERATE` is public and not Arthur-specific.

**Required amendment:** hash and length-check both `.neom` and `.vocab` in xtask before staging (adding a host BLAKE3 dependency is preferable to weakening the requirement). Replace the vocab boolean with a typed state/error that composes deterministically with model state, preserving the short-circuit “bad vocab means model not consumed.” Add beginner-visible absent/damaged/load-failed cases and direct generate tests for invalid, stale, and no-USE handles with zero allocation.

## Requested six-point check ledger

| Requested check | What I checked on `origin/main` | Result |
|---|---|---|
| 1. Promotion completeness | All real syscall cfgs, inventory module/re-exports, errno/model-route helpers, top-5 helper, serial breadcrumbs, KV baseline | **Finding:** cfg closure incomplete; one obvious widening fails compile, and logs/baseline leak unless separately gated (F3). |
| 2. `MAX_ORDER` 14 -> 16 | Primary-zone construction, split/coalesce/free, admitted zone, CP-60/CP-62 shape, `FRAMES_IN_USE`, B1a and max-order tests | Core arithmetic is order-generic; **findings:** extra primary trim, order-16 availability in the reclaimed zone, and non-vacuous proof are missing (F4). |
| 3. 256 KiB stacks | Kernel/user/browser constructors, idle threads, bootstrap exception, TSS.rsp0, per-CPU rsp, IST, heap mapping, real 12-CPU count | TSS/IST do not assume 32 KiB; **finding:** blast radius and accounting are wrong, no guard exists, and a scoped inference stack is safer (F5). |
| 4. Bounds and memory | Validation order in all three ABIs, cache constructor, 0.5B dimensions, KV allocation formula, forward scratch, B1 census docs | KV is exactly 54 MiB and context validation precedes cache allocation; **findings:** two ABIs remain unbounded and 305 MiB fit is unmeasured (F1, F6). |
| 5. Kill-switch and oracle | Heap selector KAT, proposed order/stack selectors, off stub, KV-persist KAT implementation and assertions | Selector/off behavior is testable; **findings:** “bit-for-bit” is unproved and the token oracle is circular (F2, F7). |
| 6. Beginner product image | Vocab boot FS/read path, model state composition, desktop preload order, model handle lookup, xtask staging | Normal Arthur preloads/degrades; **findings:** builder digest promise is contradictory, vocab damage state is not wired by the plan, and direct pre-load/stale-handle behavior needs a product test (F8). |

## Minimum revised-plan gate

Do not call B1c design-closed until the revised note contains all of these:

1. one pre-allocation bound policy for ordinary, constrained, and masked generation, with allocation-call assertions;
2. an explicit cfg/symbol ownership matrix and clean product/off builds;
3. an independent fixed-token oracle, while retaining the existing KAT only as a persistence differential;
4. the pinned model's measured maximum tensor order, a non-skipping admitted-zone allocation test at that order, and worst-case alignment/RAM arithmetic;
5. a scoped stack design or a measured justification for the global selector, using heap high-water rather than `FRAMES_IN_USE`;
6. a real-model or defensible formal peak showing 305 MiB covers KV + scratch + kernel stacks and other kernel-heap use with margin, plus separate total-RAM accounting for the user-space vocab;
7. honest kill-switch wording or a reproducible binary comparison;
8. pre-staging model/vocab hash verification and a typed vocab/model failure-state composition.

## Method boundary

The local RAG health endpoint was healthy and it was used for semantic discovery. Its index timestamp predated `origin/main`'s reviewed commit, so no load-bearing conclusion relies on a RAG snippet. Every claim above was checked against raw `git show origin/main:<path>` / exact-symbol search. No build or hardware run was performed because this relay is the pre-code design review.
