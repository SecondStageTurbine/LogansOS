<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: B1c product-generation code AAG -- Jarvis

**Review type:** cross-family code AAG / refutation

**Reviewed head:** `048182dd8ba76a3a87feb713275c1c1a820b9111` (`origin/claude/b1c-product-generation`)

**Base:** `origin/main`

**Verdict:** **BOUNCE.** F1 and F5 are still open. The other six findings from the design review are closed in the code reviewed here.

## Findings

### J1 -- HIGH -- an inference owner remains shootdown-ready with IF=0, so an unrelated remote shootdown can panic the kernel

The yielding lock fixes the *waiter's* interrupt starvation, not the owner's. A normal syscall enters with IF cleared by SFMASK (`kernel/src/arch/x86_64/syscall.rs:511-527`). The inference-stack module explicitly says the inventory forward does not reopen interrupts (`kernel/src/mm/inference_stack.rs:229-233`), and the short `sti` window exists only inside the failed-CAS loop (`kernel/src/mm/inference_stack.rs:234-246`). Once a caller wins `BUSY`, it executes the complete forward at `kernel/src/mm/inference_stack.rs:210-223` with IF still clear.

That CPU nevertheless remains in `shootdown_ready_cpus_mask()`. A sender intersects its target set with the ready mask, publishes the payload, sends the IPI, and waits (`kernel/src/mm/tlb_shootdown.rs:230-249`). The owner cannot service the IPI while IF is clear. After 50 ms, the sender panics (`kernel/src/mm/tlb_shootdown.rs:378-413`). A real generation is vastly longer than 50 ms, so this is not a narrow timing edge.

Reasoning reproduction:

1. On CPU 0, enter `SYS_GENERATE` (or the inventory arm of `SYS_INFER_RUN`) and pass the bounds checks so the real forward starts.
2. While CPU 0 is in the forward, on CPU 1 remove or downgrade a user PTE -- process reap is one ordinary route -- so `pte_write_fence_and_shootdown()` targets the other ready CPUs.
3. CPU 0 cannot ACK. CPU 1 reaches `tlb_shootdown_timeout` and panics the kernel after 50 ms.

The contention loop's `sti` cannot help because CPU 0 is the holder and never executes that loop again. The current B1c tests do not create a concurrent shootdown while a deliberately stalled inference owns the stack.

Required fix/gate: either (a) make the inference owner interruptible for the entire long operation and keep the foreign-stack timer/context-switch proof, or (b) use `LocalShootdownQuiesce` before the IF-off forward and perform its full local flush before becoming targetable again. Add a two-CPU test that forces a real shootdown while the owner is parked inside an inference-stack closure and proves ACK/EOI progress, readiness restoration, and no timeout. The test should also prove scheduler/timer progress if the chosen design claims preemptibility.

### J2 -- MEDIUM -- the "bounds first" property is not true at all syscall boundaries, and the zero-allocation test bypasses those boundaries

There are three concrete exceptions:

- `SYS_GENERATE_CONSTRAINED` copies the 40-byte user descriptor before `check_generate_request` (`kernel/src/arch/x86_64/syscall.rs:3196-3221`; helper at `:1281-1299`).
- `SYS_GENERATE_MASKED` copies the 24-byte user descriptor before the shared check (`kernel/src/arch/x86_64/syscall.rs:3441-3464`; helper at `:1307-1319`).
- The inventory route of `SYS_INFER_RUN` looks up and validates the model first (`kernel/src/arch/x86_64/syscall.rs:2496-2517`), then allocates/copies `input_handles` and allocates `inputs` (`:2558-2569`). Only after that does `qwen_inventory::run` apply the 2,048-token cap (`kernel/src/ai/qwen_inventory.rs:29-39`). The cap still precedes the prompt copy and KV allocation, but it does not precede every allocation, user copy, or model lookup.

The first two copies are small and stack-backed, so this is not an unbounded-allocation vulnerability. It is nevertheless observable ABI behavior: an over-limit prompt paired with an invalid descriptor pointer returns `EFAULT`, not the promised bounds-first `EINVAL`. The masked arm also intentionally changes `main`'s precedence in the other direction for a valid descriptor: invalid mask limits now return `EINVAL` before bad prompt/output pointers can return `EFAULT`. I found no documented ABI promise that fixes that old precedence, so the masked reorder is not a separate blocker, but the intended precedence needs an explicit syscall test.

The existing allocation discriminator does not cover these paths. `run_real_lm_generate_boot` invokes `real_lm_generate` directly for plain generation and invokes the pure checker for constrained/masked requests (`kernel/src/test_harness.rs:34051-34107`); it never calls any of the three syscall implementations and never exercises the inventory `SYS_INFER_RUN` route. It therefore cannot detect the exceptions above.

Required fix/gate: define the achievable contract precisely (for example, register-only bounds before the descriptor copy, then all descriptor-carried bounds before variable-size copies/allocation/model lookup), implement the inventory route without the two temporary vectors before its tensor-length admission, and add syscall-level tests with invalid pointers/handles plus a live heap-allocation counter. If the fixed descriptor read remains an explicit exception, the comments and NOTE must stop saying "before any user copy."

## Prior design findings: closure status

| Prior finding | Verdict | What I checked |
|---|---|---|
| F1 -- one bounds gate, first in every ABI | **STILL OPEN** | Traced all three real syscall arms and the inventory `SYS_INFER_RUN` route. Plain generation checks before pointer validation, lookup, and allocation (`syscall.rs:3042-3094`). Constrained/masked copy their descriptor first; `SYS_INFER_RUN` performs lookup and small allocations first. The shared pure policy has the stated caps (`generate_logic.rs:21-133`), but the bare-metal discriminator does not enter the syscall arms. See J2. |
| F2 -- independent oracle | **CLOSED** | `inventory_ref.py` parses the v2 `.neom` from byte offsets rather than importing kernel/neomc code (`tools/real-lm-oracle/inventory_ref.py:48-78`), runs float64 Qwen math, and has a changed-prompt negative control. Its split-half RoPE (`:85-92`) matches the kernel's independent split-half equations (`kernel/src/ai/ops/mod.rs:296-318`). The graph-model negative route is live at `test_harness.rs:34035-34049`. The minimum committed top-1 margin is 0.010560; it is not a formal rounding proof, but the byte-exact kernel token comparison and the separate llama.cpp/Qwen goldens make a shared silent convention error materially less likely. |
| F3 -- cfg closure and product/off artifacts | **CLOSED** | `real-lm` owns `phase9-real-quant` and `inference-stack`; the old real-brain feature implies it (`kernel/Cargo.toml:685-687`). Product breadcrumbs remain under `neo-test`. `product-elf-check` builds separate `product-real-lm` and `desktop-lean` ELFs in separate target directories and applies both absent and present needles (`tools/xtask/src/main.rs:4972-5096`). I ran it: both variants passed with no leaked/missing needles. |
| F4 -- order 16, admitted zones, free/coalesce/accounting | **CLOSED** | `MAX_ORDER` is 16 only under `real-lm`, otherwise 14 (`kernel/src/mm/frame.rs:56-67`); `NUM_ORDERS` and every list/loop derive from it. The 144,643,072-byte `output.weight` occupies 35,314 pages and rounds to 65,536 pages, so 16 is the minimum (`tools/neomc/src/inspect.rs:104-107`). Primary base trim is exactly up to `2^16-1` pages. Allocation recursion, capped coalescing, admitted-zone ownership, and `FRAMES_IN_USE +=/-= 1 << order` contain no order-14 literal. The reclaimed 463 MiB model span is honestly only guaranteed to contain order 15; tensors are allocated from primary before reclamation (`module_reclaim_logic.rs:222-245`). The exhaustive alignment/tightness KAT and order/accounting host tests passed. This grows one global zone table and one global 64-frame inference stack, not per-CPU state. |
| F5 -- scoped inference stack and SMP/preemption safety | **STILL OPEN** | Verified `call_on_stack`'s SysV alignment: 16-aligned top, saved old RSP plus padding, and callee entry at 8 mod 16; `clobber_abi("C")` covers caller-saved registers and `pop rsp` restores the saved value (`inference_stack.rs:249-296`). Timer entry uses its IST before copying a resumable frame to the interrupted stack; scheduler context state saves RSP; `TSS.rsp0`/`kernel_rsp` continue to name the thread's ordinary stack. The polling lock has no lost wakeup dependency; `PAR_BUSY` is try-only. Faults are fail-stop, so there is no unwind path that silently abandons `BUSY`. Region placement rejects image/heap/HHDM overlap and every already-mapped page, and boot mapping precedes user pmaps (`inference_stack.rs:55-139`). Low-guard overflow normally escalates to #DF/IST and both guards classify correctly; the documented timer-copy/plain-#PF and >64 KiB single-frame limitations are real. The mapping omits the NOTE's `GLOBAL` flag (`:64`), but later pmaps share the kernel PML4, so that is a performance/spec mismatch rather than the blocker. J1 is the blocking SMP failure. |
| F6 -- heap peak proof | **CLOSED** | KV at 2,304 tokens is 56,623,104 bytes. The counting-allocator KAT measured cached attention at exactly `9,220 * S + 3,584` and passed at S=2,304 (`tools/ai-ops-tests/src/lib.rs:16685-16722`). The old logits overlap is included; output logits occur after attention. Threaded matmul uses static per-CPU work slots and writes disjoint slices of the caller-owned output, so it adds no partition-output heap. The full-cap real-model release gate exists at `test_harness.rs:34353-34415`; I did not rerun that bare-metal-only external-model gate in this worktree. |
| F7 -- honest kill switch | **CLOSED** | With `real-lm` off, heap selection uses the old tier, `MAX_ORDER` is 14, the inference stack/qwen inventory path is absent, and generation keeps the old stub/test-feature routing. This is policy/behavior equivalence, not a literal identical ELF hash. The independently built `desktop-lean` ELF passed absence checks for `real_lm_generate`, `qwen_inventory`, `inference_stack`, overflow text, sampling text, and kv-persist text. The product oracle is no longer just the kv-persist equivalence vector: the committed Python result plus changed-prompt and graph-route controls are independently discriminating. |
| F8 -- staging pins and typed composition | **CLOSED** | `xtask iso --with-model` checks both size and BLAKE3 of model and sibling vocab before writing the image (`tools/xtask/src/main.rs:4960-4969`). Product pins exclude the KAT fixture; host pin tests passed. Arthur's `compose` calls the model loader only for `VocabLoad::Loaded`; absent/damaged/unreadable/unparseable vocab cannot consume the model (`userland/neo-arthur/src/model_state_logic.rs:63-104`). The composition host tests passed. A beginner boot without a model produces Arthur's explicit "no model is installed yet" state; a raw generate without a valid model handle fails through `model_from_handle` rather than entering inference. |

## Additional F5 checks

- **Force-kill/fault release:** `BUSY` is released only after the closure returns. Kernel faults and panics are fail-stop, so they do not leave a live kernel with the lock held. I found no remote process-kill path that asynchronously unwinds a currently executing kernel frame. If J1 is fixed by making the owner preemptible/cancellable rather than by shootdown quiescence, this premise must be revisited and an RAII/non-killable-owner design is safer.
- **Empty ready queue:** a contending IF-off caller calls `yield_now`; if nothing is runnable it returns and the following explicit interrupt window still admits ticks/IPIs. This is polling rather than a sleeping lock, but no wakeup can be lost.
- **Guard behavior:** ordinary downward exhaustion faults in the 64 KiB low guard and cannot build the #PF frame, so #DF uses its IST. A timer-IST copy near the bottom can instead fault while already on IST and halt as plain #PF, which the module documents. With stack probes disabled, one frame larger than 64 KiB can jump the guard; the reviewed forward has no such frame, but the guard is not a universal proof for arbitrary future closures.
- **Pmap sharing:** inference mapping occurs during `mm::init`, before the kernel-half PML4 is captured/copied into later pmaps. Runtime mapping/unmapping is absent.

## Verification performed

- `cargo test -p ai-ops-tests -p module-reclaim-logic-tests -p arthur-logic-tests -p neomc -p xtask` -- **PASS** (249 + 21 + 76 + 26 + 17 + 27 + 1 + 42 tests, plus doc-tests).
- `cargo xtask product-elf-check` -- **PASS** for `product-real-lm` and `desktop-lean`; no leaked or missing needles.
- `python -I tools/real-lm-oracle/inventory_ref.py --check` -- **NOT RUNNABLE in this worker**: Python lacks `numpy` (`ModuleNotFoundError`). I reviewed the script and committed vector directly; this is an environment limitation, not a reason to accept or reject the branch.
- Bare-metal `real_lm_generate_boot`, `inference_stack_overflow`, `boot_module_reclaim`, and external-model `real_lm_peak` -- not rerun locally.

RAG health returned healthy, but `/stats` reported `last_indexed` as 2026-10-17 and its B1c search results were stale/irrelevant. I treated RAG as discovery only and tied the conclusions above to source at reviewed head `048182dd`.

## Merge gate

Do not merge this head. Close J1 with a two-CPU shootdown-during-inference discriminator. Close J2 by making the attainable ordering contract explicit, moving the checks accordingly, and testing the actual syscall paths (including inventory `SYS_INFER_RUN`) with live allocation and errno-precedence controls.
