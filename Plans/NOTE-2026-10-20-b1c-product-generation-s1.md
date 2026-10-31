<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: B1c — real generation in the product build (S1, 2026-10-20, **revision 2**)

**Asked by:** the PM's B1c trigger (2026-10-20 02:21 UTC) and comment 6281551517 on #71. **PL decision
(2026-10-19):** the 0.1 USB carries an Arthur that thinks in the normal build. **Branch:**
`claude/b1c-product-generation` from `origin/main` @ `2f6f5606`. **Kernel-touching:** yes. It needs the
4-shard gate and a cross-family AAG (it touches syscall ABI validation, buddy order, and a new kernel
stack).

**Revision 2** answers Jarvis's design RETURN, **REFUTE**
(`origin/codex/b1c-design-crosscheck-jarvis` @ `6908321c`,
`Plans/RETURN-2026-10-20-b1c-design-crosscheck-jarvis.md`). The PM accepted all eight findings and ruled
on the choices (trigger of 02:47 UTC). Each finding F*n* maps to Jarvis's "Minimum revised-plan gate" item
*n* below.

**What is already on the branch and stays** (the PM said the vocab-pin commit can stay):
- the vocab pins (`7248e3e4`);
- the `real-lm` feature skeleton (`62188f1f`, `af1139f1`), which the matrix in section 3 corrects;
- the `/boot` vocab enforcement and the vocab-damaged scenario (`cb78c799`);
- the WIP real-LM scenario and `iso --with-model` (`de88a7bd`).

**What is reverted:** `6cc76a92`, which split the **global** thread stack size. The PM's F5 ruling is not
to change the global stack (section 5).

## The facts the revision rests on (code read on `origin/main`; my arithmetic)

- **The forward runs one token at a time.** Prefill loops `state.advance(token)` over the prompt
  (`syscall.rs:2741-2747`). Every activation is `[1 × dim]`. The KV cache is sized for
  `prompt + max_new` up front.
- **No weight is dequantized to f32.** Quantized matrices go to the native Q8_K and Q8_0 kernels
  (`forward_logic.rs:1059-1070`, `qwen_inventory.rs:1062-1073`). Only one 896-float embedding row is
  dequantized per token.
- **The pinned 0.5B has 291 tensors** (f32 121, q4_K 12, q5_0 133, q6_K 12, q8_0 13; 485,452,288 weight
  bytes; `docs/bible.md:288`).
  - 896 is not a multiple of 256, so the embedding and output fall back from k-quants:
    - `token_embd.weight` is Q5_0, 93,592,576 B;
    - `output.weight` is Q8_0, **144,643,072 B**.
  - The per-type sum reproduces 485,452,288 exactly, which is how the types were inferred.
  - neomc stores the blocks raw (`gguf_import.rs:343-371`), and the Qwen2 import requires `output.weight`
    (`:518`).
- **There is no `call_on_stack` helper and no general post-boot kernel mapping API.** Two things exist
  instead:
  - `map_kernel_mmio_page` (`mm/mod.rs:513-571`) walks `KERNEL_PML4` by hand, but only at phys + HHDM;
  - the heap shows how to map a dedicated region past the kernel image (`heap.rs:126-179, 227-239`).
  - The HHDM is huge-page mapped, so it has no room for a 4 KiB guard hole.
- **Nothing assumes `rsp` lies inside the thread's own stack.**
  - Ring-0 preemption pushes onto whatever `rsp` is (`timer.rs:85-99`).
  - `context_switch` saves `rsp` into the thread.
  - `TSS.rsp0` and per-CPU `kernel_rsp` come from the thread's own stack top and are used only on ring-3
    entry (`scheduler.rs:938-944`).
  - No stack walker or stack-range check exists.

## 1. One pre-allocation bounds policy for all three generate ABIs (F1)

There is one pure policy, `generate_logic::check_generate_request(kind, …)`, host-tested. Each syscall arm
calls it first: before any `Vec`, any user-buffer copy, the model lookup, or any model-dependent
allocation. Today the constrained and masked arms copy descriptors and masks before the model lookup; B1c
reorders them so the check comes first.

| ABI | Prompt tokens | New tokens | Other limits (kept) |
|---|---|---|---|
| `SYS_GENERATE` | 1 ≤ p ≤ 2,048 | 1 ≤ n ≤ 256 | — |
| `SYS_GENERATE_CONSTRAINED` | 1 ≤ p ≤ 2,048 (new) | 1 ≤ n ≤ 256 (existing) | ≤ 16 sequences, ≤ 1,024 candidate tokens; ceiling `p + n + 8` |
| `SYS_GENERATE_MASKED` | 1 ≤ p ≤ 2,048 (new) | exactly 1 (effective) | ≤ 262,144 mask bits |

- **The constrained path's O(N²) re-prefill is declared acceptable for B1c, with its cost bounded.** It
  re-runs the full context per token (`syscall.rs:3214-3231`). Each step's KV at the cap is the same
  54 MiB, allocated and freed per step, so the peak is not stacked (section 6). The time grows with
  `n × p`; Arthur's agentic path asks for at most 192.
  - Moving it onto the persistent-KV core is a follow-up, recorded in the PR. It is not in this lane.
- **Allocation-call assertions.** `real_lm_generate_boot` runs under `neo-test-instrumentation` as well.
  It resets `heap::reset_instrumentation_counters()` and asserts **zero `alloc_calls`** across each
  refused request:
  - over-long prompt;
  - too many new tokens;
  - masked with n ≠ 1 (by ABI);
  - constrained over its limits.

  Equal heap before and after is not enough, because a transient allocation that is later freed nets to
  zero.
- **Handles.** Direct `SYS_GENERATE` calls from a user driver with an invalid index, a closed or stale
  handle, and a model handle without `USE` must return `EBADF` / `EBADF` / `EPERM`. The code order already
  looks the handle up before copying the prompt; the scenario asserts the errnos. The zero-allocation
  proof applies to the bounds, where it is directly measurable.

## 2. An independent fixed-token oracle (F2)

- **Reference:** `tools/real-lm-oracle/inventory_ref.py`, written in Python and numpy. It shares no code
  with the kernel. It reads the committed tiny inventory model (`kernel/src/ai/fixtures/kv_persist_kat.neom`,
  F32, one layer, vocab 16), runs a textbook Qwen2 step (RMSNorm, RoPE, GQA, SwiGLU, untied output),
  greedy, for prompt `[1, 5, 9, 2]` and 8 new tokens.
- **Committed outputs:**
  - `tools/real-lm-oracle/expected_tokens.txt`, the expected tokens;
  - the logits' argmax margins, so a near-tie that could flip under float reordering is visible.
- **Kernel side:** `real_lm_generate_boot` asserts the product core's tokens equal the committed vector.
- **Negative controls:**
  - the same call with the prompt `[1, 5, 9, 3]` must **differ** from the vector;
  - a graph (non-inventory) model through the same route must return `EINVAL`.

  So both the comparison and the route assertion are shown able to fail.
- **What the existing KV-persist KAT is now:** the persistence differential only (Persistent ≡
  ReprefillBaseline). Its current output `[11, 0, 3, 3, 3, 3, 3, 3]` is **not** cited as independent.
  - If the Python reference disagrees with it, that is a finding, raised before merge rather than
    reconciled by editing the oracle.

## 3. The cfg / symbol ownership matrix (F3)

| Symbol or site | Owner after B1c | Today |
|---|---|---|
| Real `sys_generate_impl`, `sys_generate_constrained_impl`, `sys_generate_masked_impl` arms | `real-lm` | `neo-test-arthur-real-brain-boot` (+ the agentic test features for constrained/masked) |
| The EINVAL stub arms | `not(real-lm)` (and not the agentic or preemptible test features, as today) | `not(real-brain-boot)` |
| `real_lm_generate` (the shared core) | `real-lm` | — (new) |
| `generate_inventory_greedy`, `InventoryLogitsSource` | `any(real-lm, kv-persist KAT)` | `any(real-brain-boot, kv-persist)` |
| `KvGenMode::Persistent` | the same as the enum | — |
| `KvGenMode::ReprefillBaseline`, `PersistentCorruptedKat` and their match arms | **test only**: `any(neo-test-sys-generate-kv-persist-equivalence, neo-test-arthur-real-brain-boot)` | implicit (with the enum) |
| `qwen_inventory` module | `any(real-lm, agentic test features, kv-persist)` | `any(real-brain-boot, agentic, kv-persist)` |
| `generate_logic` re-exports (`constrained_argmax_token`, `masked_argmax_token`), `check_generate_request` | `any(real-lm, agentic test features)` | `any(real-brain-boot, agentic)` |
| `generate_errno`, `is_stage_b_inventory_model` | `any(real-lm, agentic, preemptible)` | `any(real-brain-boot, …)` |
| `forward0_top5`, `log_forward0_top5` | `all(real-lm, neo-test)` (computed only when it is logged) | real-brain or kv-persist |
| Breadcrumbs (`sample step=`, `constrained_sample`, `masked_sample`) | `neo-test` | unconditional inside test-only arms |
| `model_open` / `model_load_resident` / timing / qwen35 oracle prints and statics | real-brain test features (unchanged) | the same |
| Heap tier, `MAX_ORDER` | `real-lm` (section 4) | real-brain test feature |
| Inference stack | `real-lm` (section 5) | — |

**Checks:**
1. **The exact product combination** (`desktop,real-lm,model-boot-module`) builds and passes clippy.
2. **The lean-off builds** (plain `desktop` and default) build and pass clippy.
3. **A host xtask test runs `strings` and `nm` on the release product ELF.** It shows absent:
   - `sample step=`, `forward0 top5`, `constrained_sample`, `masked_sample`, `model_load_resident`;
   - any symbol containing `kv_persist` or `Reprefill`.
4. **On the lean-off ELF** it also shows `qwen_inventory` and `real_lm_generate` absent.

## 4. `MAX_ORDER`: the measured largest tensor, a non-skipping test, and the trim arithmetic (F4)

**Order.**
- The largest tensor is `output.weight` at 144,643,072 B: 35,314 pages, rounded to 65,536 pages
  (`order_for_len`, `tensor_logic.rs:168-184`). That is **order 16**, the minimum sufficient order.
- 15 (128 MiB) is too small; 16 is the same as today's real-brain build.
- **Confirmation:** a host `neomc inspect` subcommand prints each tensor's type, bytes and order. Its
  output on the pinned model (one command on the seat that holds it, as for B1-1) is committed to the PR.
- If it shows the largest tensor at or below 128 MiB, the order drops to 15.

**Where the order-16 block comes from.**
- B1b builds the model **before** the reclaim, so every tensor is allocated from the **primary** zone
  while the module still occupies its own pages. The reclaimed zone never has to supply the order-16
  tensor.
- What the admitted zone can supply is still made explicit:
  - for a span of length L at arbitrary 4 KiB alignment, a block of order *k* is guaranteed iff
    L ≥ 2^(k+1) pages − 1 page;
  - a pure function plus a host KAT proves this;
  - the KAT shows the 463 MiB model span guarantees order 15 but not 16.

**Non-skipping tests.**
- **Primary zone:** in `real_lm_generate_boot`, run under `-m 4096`:
  - allocate one order-16 block (refusing OOM: no PASS on failure);
  - check `FRAMES_IN_USE` rose by 65,536;
  - free it;
  - check full coalescing (the free list at order 16 is restored) and `refused_frees == 0`.
- **Admitted zone:** `boot_module_reclaim` gains the guaranteed maximum order of its probe span, computed
  by the same pure function. It allocates that order from the admitted zone, frees it and checks
  coalescing, and fails on OOM. The B1a probe size is reused, so the image does not grow.

**Worst-case primary trim.**
- `Zone::from_memmap` trims the base **up** to the max-order alignment (`frame.rs:142-147`). It loses up to
  2^order pages − 1 page:
  - 64 MiB − 4 KiB at order 14;
  - **256 MiB − 4 KiB at order 16**, which is **192 MiB more**.
- This term is added to the RAM floor in `docs/B1-boot-module-model.md`. The floor stays 2 GiB when the
  largest usable region is 256 MiB-aligned; it is 2 GiB + 192 MiB in the worst placement. The OptiPlex
  census logs `trimmed_base − region_base` so the real loss is measured.

**CP-29.** Both guards stay: serialized AP bring-up, and the independent shootdown-ready mask
(`docs/concurrency-pitfalls.md` CP-29). Section 5 no longer enlarges idle-thread allocations, so the
per-CPU initialization footprint does not grow.

## 5. A scoped inference stack; the global stack is unchanged (F5, PM ruling)

- **The global stack is unchanged.** `STACK_SIZE` goes back to `main`: 32 KiB, and 256 KiB under the
  real-brain **test** feature exactly as today, so those builds do not move. `6cc76a92` is reverted.
- **The design: one dedicated inference stack**, used for every real-LM generate call:
  - **Size:** 256 KiB, the size the real-brain forward has run with.
  - **Mapping:** its own kernel VA region past the heap end, mapped 4 KiB at a time
    (`PRESENT | WRITABLE | NX | GLOBAL`).
  - **Placement:** the region sits under the top-level kernel PML4 entry that every user pmap already
    shares, so mapping it (not-present → present) needs no shootdown.
  - **Mapping code:** the heap's mapper pattern, generalized to a chosen VA with those flags.
- **The switch.** A small `call_on_stack(top, fn, arg)` (asm: save `rsp`, load the new top, call, restore).
  `real_lm_generate` runs entirely on the inference stack, called from all three syscall arms.
  - It is guarded by a **sleeping** lock (`SwitchMutex`), so a second generate waits rather than sharing
    the stack.
  - Preemption on the inference stack is safe by the facts above: `rsp` is saved per thread, and
    `rsp0` / `kernel_rsp` are untouched.
  - The kernel enforces this for any caller of the public syscalls. It is not an Arthur-only flag.
- **Guard-page decision: yes.**
  - One unmapped 4 KiB page sits below the stack, with another unmapped page above it.
  - An overflow faults on the guard. The CPU cannot push the `#PF` frame onto the same unmapped page, so it
    escalates to `#DF`, which runs on its own IST stack. The `#DF` handler reports
    `inference stack overflow` and halts. That fails closed **before** any other memory is corrupted.
- **Canaries and high-water.**
  - The stack is zeroed before each call, with a 64-byte canary at its lowest mapped bytes.
  - After the call, the canary is checked (panic if hit). The depth is recorded with a `fetch_max` and
    reported by the census as `inference_stack_high_water`.
  - The measured 0.5B depth at the OptiPlex run is recorded. The size is revisited if the margin is below
    2×.
- **Accounting.**
  - The inference stack is **not** heap memory: 64 frames from the buddy allocator at first use, counted
    in `FRAMES_IN_USE`.
  - Heap effects are measured with `heap::used_bytes()` and the `kernel-observability` high-water, never
    with `FRAMES_IN_USE` (the heap's frames are counted up front).
- **12-CPU boot check.** `real_lm_generate_boot` and `desktop_session_boot` built with `real-lm` also run
  at `NEO_SMP=12` locally, the OptiPlex's CPU count. The results go in the PR (CP-29 lens).

## 6. The heap peak: a formal budget; the real peak is the release gate (F6)

**Peak live kernel heap during one 0.5B generate at the 2,304-token cap** (code-derived, decode is the
worst point):

| Term | Bytes | Live |
|---|---|---|
| KV cache: 24 layers × K, V × 128 × 4 B × 2,304 | 56,623,104 (54.0 MiB) | the whole call |
| Previous logits (old `Vec` kept until the new one exists) | 607,744 (0.58 MiB) | across steps |
| Attention, one layer: the full query `[14 × S × 64]` f32, active K and V, the `bytes_from_f32` copies of all three, softmax weights | 9,220 × S = 21,242,880 at S = 2,304 (20.26 MiB) | inside one layer, freed after |
| Layer seam, norms, `lhs_q8`, FFN gate/up/SwiGLU copies | < 0.2 MiB | inside one layer |
| New logits at the output stage (not concurrent with attention) | 607,744 | output stage |
| **Peak (decode)** | **≈ 74.9 MiB** | |
| **Peak (prefill, last prompt token)** | ≈ 72.6 MiB | |

The terms above leave out two things:
- **`threaded-matmul` scratch.** `desktop` enables it. Its fork-join adds per-partition descriptors only;
  the partition outputs are disjoint slices of the caller's buffer. B1c traces it and adds its bytes to
  this table before merge.
- **The rest of the kernel heap:** the boot baseline plus the desktop's own use. It is measured: the
  `desktop_session_boot` census at the product feature set gives `heap_used` before Arthur loads.

**The budget:** `305 MiB ≥ baseline + 74.9 MiB + threaded scratch`, with a required margin of ≥ 2× on
(74.9 + scratch). The PR records the numbers. If the margin fails, the tier is raised before merge.

**The shape is proven, not only computed.** A host allocation-shape KAT (a counting allocator in
`ai-ops-tests`) runs the kernel's `gqa_causal_attention_last` at the 0.5B dimensions with S = 2,304
(no weights are needed for attention). It asserts the live peak equals 9,220 × S ± 1 page.

**User space is counted separately** in the total-RAM census:
- Arthur's 16 MiB vocab read buffer;
- the parsed vocab (measured);
- the resident 7,990,862-byte vocab module.

**The release gate (OptiPlex, not merge):** a bare-metal step loads the real 0.5B and runs one generate at
the full cap (2,048-token prompt, 256 new). It records the `kernel-observability` heap high-water, the
inference-stack high-water and the timing. It is a separate `real_lm_peak_baremetal` bundle, because
Arthur never sends a 2,048-token prompt. The 305 MiB tier is confirmed or raised from that number.

## 7. The kill-switch, worded honestly (F7)

- `real-lm` off is **policy- and behaviour-equivalent** to today's build, not "bit-for-bit".
- The evidence:
  - the host selector KATs (heap; `MAX_ORDER` and the stack stay outside `real-lm` except as listed in
    the matrix);
  - the existing desktop scenarios unchanged and green;
  - a runtime guard: a plain-`desktop` kernel answers `SYS_GENERATE` with `EINVAL` (asserted in a
    `desktop` scenario's off-path driver call);
  - the negative `strings`/`nm` checks from section 3.
- No reproducible-binary comparison is claimed.

## 8. xtask verifies digests before staging; the vocab and model states compose (F8, PM ruling)

**xtask.**
- It gains a host `blake3` dependency, with `default-features = false`. With the default features,
  workspace feature unification turns on `std` for the kernel's `blake3`, which breaks the `no_std`
  kernel. B1b hit exactly this.
- `iso --with-model` checks the length **and** the BLAKE3 of both the `.neom` and the `.vocab` against
  the kernel's pins before staging. The pins come from `kernel/src/boot_model_logic.rs`, mounted by
  `#[path]`, so there is one source of truth.
- A mismatch refuses to write the image and names the file, the expected digest and the actual digest.

**Arthur.** The vocab boolean is replaced by a typed outcome:
- `VocabLoad::{Loaded, Absent, Damaged, Unreadable(errno), Unparseable}`;
- it composes with the model through a pure `model_state_logic::compose(vocab, model_load)`.

| Vocab | Model | State |
|---|---|---|
| `Absent` | (not taken) | absent |
| `Damaged` (EBADMSG) | (not taken) | **damaged** |
| `Unreadable` / `Unparseable` | (not taken) | load failed |
| `Loaded` | ENOENT / EBADMSG / other / ok | absent / damaged / load failed / ready |

- "A bad vocab means the model is not consumed" holds by construction: the model load is only attempted on
  `Loaded`.
- Both the console path and the desktop path call `compose`. The desktop no longer short-circuits to the
  generic notice.
- A host KAT covers every row.
- The scenarios cover:
  - absent;
  - damaged model;
  - damaged vocab (exists);
  - load failed: a truncated `.neom` that matches no pin is "damaged". A real load failure needs a pinned
    but corrupt-structured model, so it is KAT-only, with that stated.

## Discriminator summary (QEMU, hermetic)

**`real_lm_generate_boot`** (`real-lm` + `model-boot-module`, **without** the real-brain test feature,
with `neo-test-instrumentation` and `kernel-observability`) proves:
- the boot-module load of the tiny inventory LM (pinned);
- the product core on the inference stack;
- tokens equal to the **Python** reference, plus both negative controls;
- zero allocation calls on refused requests;
- order-16 primary allocation, free and coalesce;
- the census inference-stack high-water and canary;
- `NEO_SMP=12` locally.

**The other scenarios:**
- `arthur_boot_module_{answers,damaged,absent,vocab_damaged}` (B1b plus B1c) re-run with typed composition.
- `boot_module_reclaim` gains the admitted-zone max-order allocation.
- The existing real-brain, agentic, kv-persist and desktop scenarios run unchanged as the regression.
- The `SYS_GENERATE → EINVAL` guard runs on plain `desktop`.

**Acceptance:** the OptiPlex boots the `iso --with-model` image, and Arthur answers in the desktop. The
`real_lm_peak_baremetal` bundle records the real peak. Both are release gates, not merge gates.

## AAG

- A Claude lens.
- A Codex lens on the built branch: Jarvis, who refuted the plan, or Chad. It covers the inference
  stack and the switch, the bounds ordering, and the `MAX_ORDER` tests.
- CP-29 and the pitfall bank are consulted. A new CP entry covers the scoped-stack hazard (a foreign stack
  under preemption, and a sleeping lock held across a long syscall).

## Order of work (after the PM's clearance)

1. Revert `6cc76a92`; complete the cfg matrix (section 3) with the product and lean-off builds.
2. The bounds policy for the three ABIs, with zero-allocation tests (section 1).
3. The inference stack: mapping, guard, `call_on_stack`, lock, canary and high-water (section 5).
4. The Python reference and the committed token vector; finish `real_lm_generate_boot` (section 2).
5. The `MAX_ORDER` evidence: `neomc inspect`, the admitted-order KAT, the primary and admitted
   allocations (section 4).
6. The heap budget table with measured baselines; the allocation-shape KAT (section 6).
7. xtask BLAKE3 plus the typed vocab/model composition (section 8); the docs.
8. Close-gate, the 12-CPU run, AAG, Codex dispatch, PR, "PM: ready for gate".

## As built (after the PM's clearance, 03:33 UTC) — what changed from revision 2, and the measured numbers

**Where the build differs from the plan, and why:**
- **The lock is a yielding lock, not `SwitchMutex`.** `SwitchMutex` turned out to be a spin lock. A
  spinner held across a seconds-long, preemptible forward would burn its CPU and could strand behind a
  preempted holder. `inference_stack::run` uses `compare_exchange` and `yield_now()` on contention
  instead, so a waiter gives its CPU away. CP-63 records it. (The holder was *not* preemptible in the
  first build: see "Jarvis's code AAG" below.)
- **The inference stack is mapped at boot, not at first use.** It is mapped right after the heap, with
  the same mapper, before any user pmap or AP exists. No page table changes at run time, so there is no
  post-boot mapping API to write and no shootdown question. Cost: 64 frames, always, under `real-lm`.
- **`inference-stack` is its own feature**, implied by `real-lm`. The overflow scenario boots without
  the real-LM heap tier.
- **The zero-allocation proof** runs at the shared core that `SYS_GENERATE` uses (`real_lm_generate`):
  over-long prompt, too many new tokens, and zero new tokens. The constrained and masked limits are
  checked through `check_generate_request` itself.
  - Their syscall arms read only a fixed-size descriptor into a stack array (`copy_user_array`) before
    calling the policy. Nothing allocates before the check, by code order.
  - Exercising those two arms from the kernel would need a user driver. That is left to the AAG to
    confirm.
- **The handle errnos** are asserted on `model_from_handle`, the classification factored out of
  `model_lookup` that every arm uses:
  - a missing or closed index is `EBADF`;
  - a non-model handle is `EBADF`;
  - a model handle missing the semantic or the meta `USE` right is `EPERM`.

  A closed index and a never-allocated one are the same `None` here; the table's own close and
  generation semantics are covered elsewhere.
- **The cfg matrix's clippy bar.** On `main`, `desktop` already carries 79 dead-code clippy findings
  (atrium, `qk_matmul`, `forward_perf`; CI does not clippy that feature set), and `product` is clean. The
  bar used is therefore: the product combination (`desktop,real-lm,model-boot-module`) adds no finding
  over `desktop`'s baseline on `main`.
  - The GDN (Qwen3.5) state is built only by the OT-16 walker. It is marked
    `allow(dead_code)` outside the walker, as the real-brain test builds on `main` already carried it.
  - The ELF checks (section 3, checks 3–4) are `cargo xtask product-elf-check`. It builds the product
    and the lean `desktop` kernels into their own target dirs and scans each whole ELF (strings, symbol
    table and debug info), with positive controls that must be found. Result: PASS, nothing leaked
    either way.
  - The section 7 runtime `EINVAL` guard on a plain-`desktop` kernel is carried by that scan instead
    of a user driver: the lean ELF contains no `real_lm_generate`, `qwen_inventory` or
    `inference_stack`, and its `sys_generate_impl` is the `EINVAL` stub.

**Measured:**
- **Python oracle** (`tools/real-lm-oracle/expected_tokens.txt`):
  - `[1,5,9,2]` gives `11,0,3,3,3,3,3,3`, which equals the kernel's tokens;
  - the control `[1,5,9,3]` gives `4,11,3,3,3,3,3,3`, which also matches the kernel;
  - the minimum top-1/top-2 margin is 0.0106.

  The persistence KAT's earlier output agrees with an independent reference: no finding.
- **Inference stack:**
  - the 40-frame control used 171,672 of 262,144 bytes with the canary intact;
  - the 100-frame call reached `#DF` on the low guard, `cr2` inside it;
  - the tiny inventory LM's forward used 20,488 bytes.

  The 0.5B depth is an OptiPlex measurement (`CENSUS … inference_stack_high_water=`).
- **`MAX_ORDER`:**
  - an order-16 block came from the primary zone (+65,536 frames), freed, and coalesced (the free-list
    count was restored);
  - the admitted probe span (1,536 pages) supplied its guaranteed order 9.
- **Attention heap shape** (host, counting allocator, 0.5B geometry, S = 2,304): the peak is
  21,246,464 B = 9,220 × S + 3,584 (the output row). Section 6's per-position term holds.
- **`threaded-matmul` scratch: 0 heap bytes.** Partition descriptors live in the static `AP_SLOTS`;
  every partition writes a disjoint slice of the caller's output (`par_matmul.rs` `run_disjoint`); the
  partition kernels allocate nothing.

**The heap budget (section 6), with the measured terms:**

| Term | Bytes | Source |
|---|---|---|
| Boot baseline plus Arthur, before the model | 1,853,608 used, high-water 1,857,799 (≈ 1.8 MiB) | `arthur_boot_module_answers` census, this branch |
| Peak of one 0.5B generate at the 2,304-token cap | ≈ 74.9 MiB: KV 56,623,104 + attention 21,246,464 (measured, host KAT) + logits 2 × 607,744 + < 0.2 MiB per-layer copies | code-derived; the attention term is measured |
| `threaded-matmul` scratch | 0 | traced: static slots, disjoint output slices |
| **Required:** 305 MiB ≥ baseline + peak + scratch, margin ≥ 2× on (peak + scratch) | 305 ≥ 1.8 + 74.9 + 0, so (305 − 1.8) / 74.9 = **4.05×** | |

- The desktop session's own kernel-heap use is not in the QEMU census: `desktop_session_boot` has no
  `model-boot-module` census. The 2× margin still holds as long as it stays under
  305 − 1.8 − 2 × 74.9 ≈ 153 MiB.
- The OptiPlex census (`heap_used` at `arthur_boot_module_start`, `heap_high_water` at the end) measures
  it on the product image.
- The bare-metal `real_lm_peak_baremetal` bundle (the 2,048 + 256 generate) stays the release gate for
  the 305 MiB tier.

## AAG (Claude family, three lenses, read-only) — outcome and what changed

- **Concurrency / IST lens: no surviving finding.** Notes acted on:
  - The inventory forward runs at `IF=0`. A contended `acquire()` now opens a short interrupt window
    each round, so a waiting CPU still takes ticks and IPIs.
  - The kernel emits no stack probes, so the low guard is now 64 KiB of unmapped address space.
  - An underflow into the high guard, and a timer frame copy on a nearly full stack, halt as a plain
    `#PF`. This is documented.
  - Force-kill while holding the lock stays a recorded residual (CP-63): no path reaches it today.
- **ABI / memory lens: one finding, fixed (F-A, high).** With `real-lm` on, `SYS_INFER_RUN`'s inventory
  route ran the forward on the 32 KiB thread stack with no prompt cap.
  - Both call sites now go through `on_inference_stack`.
  - `qwen_inventory::run` refuses more than 2,048 tokens before it copies anything.
- **Completeness lens: no blocker.** Fixed:
  - The B1 doc's tensor footprint ignored buddy rounding: ≈ 620–855 MiB, not ≈ 463 MiB.
    `neomc inspect` now prints the exact `buddy_bytes`, and the floor text and falsifier were updated.
  - A stale staging line in the doc.
  - The oracle docstring overclaimed independence: head size and dims order are inputs from outside
    ADR-028.
- **Recorded limits, not fixed here:**
  - The F2 oracle covers neither the quantized kernels nor a GQA group above 1 (the fixture is F32 and
    2:2).
  - The scenario adds `phase9-data-driven-config`, which the product does not have.
  - In CI, the constrained and masked arms never run on the inference stack: the agentic fixtures take
    the graph path.
  - CI runs neither `inventory_ref.py --check` nor `product-elf-check`.
  - No test runs two generates at once.
  - The trim log line is unconditional, so it appears in every build.
- **Cross-family lens:** Jarvis (Codex), `Plans/DISPATCH-2026-10-20-b1c-product-generation-codex-aag.md`.

## Jarvis's code AAG (cross-family) — BOUNCE on J1 and J2, and the fix

Jarvis closed F2, F3, F4, F6, F7 and F8 at `048182dd`. F1 and F5 stayed open:
`codex/b1c-code-aag-jarvis`, `Plans/RETURN-2026-10-20-b1c-product-generation-codex-aag-jarvis.md`.
The PM ruled option (a) for J1.

**J1 (HIGH): the inference-stack owner ran its whole forward at `IF=0` while shootdown-ready.**
- **The failure.** A syscall enters at `IF=0`, and the forward never reopened interrupts. The owner's CPU
  stayed in the shootdown-ready mask but could not ACK, so any other CPU's shootdown (a reap, an unmap)
  panicked the kernel after 50 ms. The earlier fix had only given *waiters* an interrupt window. On the
  BSP, the same `IF=0` stopped the only timer for the whole generation.
- **The fix, option (a).**
  - `inference_stack::run` runs its closure under `with_interrupts_enabled`; the caller's `IF` is
    restored before the call returns to its own stack. On the BSP the owner is now preemptible.
  - Preemption on the inference stack was already shown safe. The timer enters on its IST and copies
    its frame to the interrupted stack. A context switch saves `rsp`, and the FPU state:
    `inference-stack` now implies `xsave-fpu`, so the AVX2 matmul's YMM uppers survive (FXSAVE alone
    would lose them). `TSS.rsp0` and `kernel_rsp` come from the thread's own stack. The heap lock is
    IRQ-safe.
  - APs have no timer (PIC IRQ0 goes to the BSP only), so an owner on an AP is interruptible (IPIs) but
    not preempted.
- **The force-kill premise, re-checked.** It was wrong. CP-63 said force-termination acts only at the
  return to ring 3. In fact `kill_thread_by_pid` zombifies any *Ready* thread, and a preempted owner is
  Ready mid-syscall; zombifying it would leave the lock held forever. The non-killable-owner design:
  - `run` records the owner (thread id and pid) with the lock. A drop guard releases both together.
  - `kill_thread_by_pid` skips the owner (`owner_thread`).
  - `terminate` still tears the process down; `owned_by_pid` satisfies its accounting assertion.
  - The owner exits itself at the syscall boundary: `after_inference` wraps the four inference
    syscalls and calls `sched::exit()` if the process is gone or exiting. By then the arm has returned,
    so every local is dropped and the stack is released.
- **The discriminator.** `inference_stack_shootdown` (`NEO_SMP=4`):
  - **The setup.** The owner parks on the BSP inside the closure, entered at `IF=0`. CPU 1 sends 16 real
    shootdowns at the BSP, and a co-runner waits on the BSP's ready queue.
  - **Measured.** `sent=16 ipis_received=16 timeouts=0 max_ack_cycles=946615 ticks_while_parked=4
    corunner_runs=43808 if_restored_off=true ready_after=true canary=true`.
  - **The control.** `inference_stack_shootdown_control` builds the pre-fix owner, which keeps the
    caller's `IF`. It panics as J1 predicted: `tlb_shootdown_timeout sender_cpu=1 ...
    elapsed_cycles=500002889`.
- **Also changed.**
  - `inference_stack_overflow`'s control asserts the owner runs with interrupts on.
  - Its deliberate overflow masks interrupts for itself alone, because a timer copy near the guard
    would fault as a plain `#PF` rather than reach `#DF`.
- **CP-64** is the new pitfall: a long `IF=0` kernel section while shootdown-ready.

**J2 (MEDIUM): "bounds first" was not literally true at the syscall boundaries.**
- **The contract, stated precisely** (`generate_logic::check_generate_request`'s doc):
  1. Register-carried bounds come first. For `SYS_GENERATE` that is all of them; for the constrained and
     masked arms it is the prompt length (`check_prompt_len`, before the descriptor copy).
  2. The only user access before the full policy is the fixed-size descriptor, read into a stack array.
  3. Then the full policy runs, before any variable-size user copy, heap allocation or model lookup.
- **`SYS_INFER_RUN`'s inventory route**, in order:
  1. flags (register);
  2. the model lookup, which classifies the route and allocates nothing;
  3. the input count (register);
  4. the one handle, a fixed 4-byte read into a stack array;
  5. the tensor lookup, which allocates nothing;
  6. `qwen_inventory::admit_input`: an I32 vector of 1..=2,048 tokens.

  Only then is `inputs` built. The two temporary `Vec`s are gone from this route.
- **The test, through the real syscall arms.** `real_lm_generate_boot` runs 16 cases on CPU 1. The probe
  thread belongs to a registered user process with one mapped page, whose CR3 is active. Each case
  records its return value and its heap allocation calls:
  - all 14 refusals return their errno with **0** allocations;
  - the two admitted calls allocate (`plain_ok` 610, `infer_ok` 494), so the counter is live.
- **What the cases pin.**
  - Over-limit requests with bad pointers or a bad handle return `EINVAL`, which proves bounds come
    first.
  - The same requests within limits return `EFAULT` or `EBADF`, which proves the later checks are live.
  - A bad constrained or masked descriptor returns `EFAULT`.
  - The masked arm's malformed-mask `EINVAL` precedes a null-output `EFAULT`. This precedence changed
    from `main`; no documented ABI depended on the old order.
  - `SYS_INFER_RUN` with a 2,049-token tensor returns `EINVAL` with 0 allocations, and the same call with
    a 4-token tensor succeeds.
- **The comment and NOTE wording** no longer says "before any user copy".

**The F5 note: `GLOBAL`.** The inference-stack mapping now carries `GLOBAL`, as section 5 promised.

**Still not covered (unchanged):**
- no test runs two generates at once;
- no runtime test kills a preempted owner. The skip and the boundary exit are by code and by CP-63's
  audit; a user-process kill-during-generate scenario would need a ring-3 driver.

**Rebased onto #75 (`c074037e`); the product image gets the intro.** At the PM's ask, `xtask iso
--with-model` now builds the full desktop with the first-boot intro, as `run-desktop --intro` does:
- neo-init gets `desktop-intro`, the worker gets the Lens, and Arthur gets the desktop-full roles plus
  `arthur-boot-module`;
- the kernel adds `atrium-pointer-forward`, so it is `desktop,atrium-pointer-forward,real-lm,model-boot-module`.

The CI clippy arm and `product-elf-check` use the same set.

## Jarvis's re-check — BOUNCE on J3 and J4, and the fix

Jarvis re-checked `056f89fe` (`codex/b1c-code-aag-recheck-jarvis`,
`Plans/RETURN-2026-10-20-b1c-codex-aag-recheck-jarvis.md`). J2 and F1 are closed, and so is the
original shootdown deadlock. J1/F5 stayed open on two new findings in the kill design. The PM ruled the
shape of the fix: one per-thread inference-lifecycle marker.

**J3 (HIGH): a waiter was killable.**
- **The failure.** Only the stack's holder was skipped by force-kill. A waiter that lost `BUSY` yields
  as an ordinary Ready thread while its kernel stack owns `Arc<Model>`, prompt `Vec`s and tensor `Arc`s.
  A zombie reap frees that stack without running destructors (CP-30's rule), so the model leaked.
- **The fix.**
  - **The marker.** `syscall::inference_syscall` wraps the four inference syscalls (the dispatcher
    calls it, and so does the scenario). At the arm's entry, before any allocation, model clone or
    `Vec`, it sets `Thread::inference_lifecycle` and bumps the process's `inference_participants`.
  - **The kill path.** `kill_thread_by_pid` skips every marked thread (`position_by_pid_killable`).
    `terminate`'s accounting counts participants.
  - **Cancellation is cooperative** (`inference_stack::cancel_requested`, a torn-down non-kernel
    process):
    - a waiter gives up in the `BUSY` retry loop (`try_run` gives `None`, so the arm returns
      `ECANCELED`) and never runs the forward;
    - the holder stops before prefill and between tokens (`generate_inventory_greedy`,
      `constrained_loop`);
    - every local drops as the arm returns, and the thread exits at the boundary.

**J4 (MEDIUM): the marker ended before the boundary decision.**
- **The boundary.** `inference_syscall` takes the exit decision while the thread is still marked,
  after the arm has returned. It clears the marker and participant count only then, and a torn-down
  caller exits there.
- **The stack's ownership** (`BUSY` plus the owner tid/pid) stays separate from the marker. All three
  are published and cleared with interrupts masked. (Re-check #2 narrowed this wording: they are
  separate atomics, not one state, and only tests read the tid/pid.)
- **Residual, pre-existing.** After the marker clears, a thread on the syscall return path holds
  nothing, but a remote terminate can still find it Running and unqueued. Every syscall's return path
  has that window today. CP-65 records it.

**The gate: `inference_stack_kill`** (`NEO_SMP>=2`, 4 GiB).
- **The setup.**
  - Two registered processes. Each caller thread is its process's own, with its pid and its address
    space (`Thread::attach_process_for_test`), and runs `SYS_GENERATE` through `inference_syscall`.
  - Holder A parks on the inference stack, on the BSP with interrupts on, so it can be preempted.
  - Waiter B arrives, loses the stack, and yields: 3 yields are observed.
  - CPU 1 terminates B, then A. Each time the BSP holds its CPU with preemption disabled and interrupts
    on, so both callers sit Ready in the queue during the terminate.
- **Measured:**
  - `waiter terminate=0 boundary_exit=true cancelled=1 forward_ran=false holder_still=true`;
  - `holder terminate=0 boundary_exit=true left_park_on_cancel=true busy_after=false owner_after=None`;
  - `baselines arcs=1/1 frames_delta=0 served=42 returned=0`.

  The heap ends below its baseline. That is the killer thread, created before the baseline and reaped
  after; the check is "no growth".
- **The control.** `inference_stack_kill_control` builds the pre-J3 rule (skip only the holder): the
  waiter is zombified, never exits at the boundary, and its model `Arc` (`arcs=2/1`) and 5 frames leak.
- **CP-65** is the new pitfall: force-killing a suspended thread whose kernel stack owns `Arc`s.

**Still not covered:** two *concurrent forwards* (the second caller here waits and is cancelled); a
kill driven by a real ring-3 process (the callers are kernel threads attributed to user processes).

## Jarvis's re-check #2 — BOUNCE on J3-P and C1, and the fix

Jarvis re-checked `e6a75dee` (`codex/b1c-code-aag-recheck2-jarvis` @ `12cb1af5`). J1 and J4 are closed
(J4 scoped to this lane). J3/F5 stayed open on two findings. The PM ruled the fix and sent the
pre-existing graph-admission kill leak to S2: `admission.rs` and `zombify_parked_*` are untouched here.

**J3-P (HIGH): the marker covered the graph route of `SYS_INFER_RUN`.**
- **The failure.** `inference_syscall` wrapped the whole of `SYS_INFER_RUN`. Its graph route never
  touches the inference stack. Under accelerator contention, admission token-parks the caller, and
  `zombify_parked_thread_by_pid` ignores the marker. A terminate then zombified a *marked* thread: its
  `Arc`s leaked, and `inference_participants` was never decremented.
- **What the ruling splits off.** The `Arc` leak on a token-parked graph caller is `main`'s own
  behaviour, and it goes to S2. This lane's part is that the marker must not reach that route.
- **The fix.**
  - The dispatcher wraps only `SYS_GENERATE`, `_CONSTRAINED` and `_MASKED`. `SYS_INFER_RUN` calls
    `sys_infer_run_impl` directly.
  - `sys_infer_run_impl` checks the flags, looks the model up, validates it and classifies the route;
    none of that allocates. Only the inventory route is wrapped in `inference_syscall`, before any
    allocation on it. The rest of the body is `infer_run_routed`.
  - The graph route is as on `main`: no marker, no participant count.
- **The check.** The ABI probe counts marked entries (`INFERENCE_MARKED_ENTRIES`) across one call per
  route: `infer_inventory_marked ret=1` and `infer_graph_marked ret=0`.

**C1 (MEDIUM): a waiter that won `BUSY` after its teardown ran the forward.**
- **The failure.** A waiter checked for cancellation only after a failed CAS. If the holder released
  between the teardown and the waiter's next CAS, the waiter won and ran a whole forward for a dead
  process.
- **The fix.**
  - `Held::acquire` checks `cancel_requested()` right after winning `BUSY`, before the closure. A
    cancelled winner releases the stack and returns `None`.
  - The inventory-infer and masked closures check for cancellation before their forward, as the
    generate paths already did.
  - Inventory infer returns `ECANCELED`, not `EINVAL`.
- **The discriminator: the release race,** a new phase of `inference_stack_kill`.
  - Holder E parks on the stack. A test gate holds waiter F just before its CAS.
  - CPU 1 terminates F while it waits. E is then released and returns, and the gate opens.
  - F's next CAS wins, and the post-win check cancels it.
  - Measured: `race terminate=0 holder_returned=true cancelled_after_win=1 forwards=1
    waiter_boundary_exit=true`. The one forward is E's; F ran none.
- **Baselines after all phases:** `arcs=1/1 frames_delta=0 served=42 returned=1`.

**Owner-tuple wording.** The docs now say the owner tid and pid are separate atomics, published and
cleared with interrupts masked, and read only by tests. No safety decision reads them.

**Found while running the gate: a KASLR boot panic.** One `inference_stack_kill` boot panicked with
`inference stack: no unmapped region beside the kernel heap`. KASLR had placed the image at
`0xffffffffec3f0000`; the 305 MiB heap sat directly above it and ran to `0xfffffffffff00000`. The slot
above the heap wraps the address space, and the slot below the heap lands in the image.
- **The fix.** `inference_stack_logic::region_candidates` adds a third slot below the kernel image.
- **The test.** The host KAT `region_candidates_kaslr_ceiling` pins that exact layout.
- This is a boot-time placement bug, present since the inference stack landed, and not a scenario flake.

**Found while running the gate: the release race's own sequencing at 12 CPUs.**
- **The failure.** At `NEO_SMP=12` on a 4-core TCG host, holder E's forward outlasted the 2,000-tick
  (20 s) wait. The gate opened while E still held `BUSY`. F then cancelled in the retry loop instead of
  after a win (`cancelled_after_win=0 forwards=0`), and the phase failed.
- **No product defect.** F ran no forward in any attempt; the test had opened the gate too early.
- **The fix.** The phase waits up to 12,000 ticks for E to return, and fails with its own reason
  (`race_holder_never_returned`) if it doesn't. Measured afterwards: three straight first-attempt
  passes at `NEO_SMP=12`.

**Regression (serial, one QEMU at a time):**
- **`NEO_SMP=4`, all PASS:**
  - `inference_stack_{kill,kill_control,overflow,shootdown,shootdown_control}`
  - `real_lm_generate_boot`
  - `arthur_real_brain_boot`
  - `sys_generate_kv_persist_equivalence`
  - `arthur_agentic_actions`
  - `arthur_boot_module_{answers,damaged,absent,vocab_damaged}`
  - `boot_module_reclaim`
  - `desktop_session_boot`
  - `desktop_intro_boot`
- **`NEO_SMP=12`, all PASS:**
  - `inference_stack_kill`: three first-attempt passes after the fix
  - `inference_stack_shootdown`
  - `real_lm_generate_boot`
  - `desktop_session_boot` with `real-lm`
