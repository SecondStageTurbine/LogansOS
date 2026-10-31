<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: B1c ruled-change re-check #3 -- Jarvis

**Review type:** cross-family SMP/core code AAG / refutation

**Reviewed head:** `553e105850a44003672f9aa1644f9f6253aeec27`
(`origin/claude/b1c-product-generation`)

**Comparison points:** `e6a75dee23c83cbdb0394528b672e9b0317c8a30` (re-check #2 head) and
`origin/main` (graph-route behavior)

**Verdict: CLOSE the two ruled changes. No new blocking finding.** The lifecycle marker is now limited
to the three generate arms and the inventory arm of `SYS_INFER_RUN`; the graph route is unmarked and
retains `main`'s validation, errno, allocation, and admission ordering. C1 is closed by the cancellation
check after winning `BUSY`, the one-shot pre-forward checks, the `ECANCELED` inventory result, and a
release-race gate that forces the formerly missed interleaving. The KASLR third candidate is arithmetically
sound and its failed-boot layout is pinned by a host KAT.

## Closure status

| Item | Status | Result |
|---|---|---|
| **J3-P** | **CLOSED for B1c** | The B1c-specific premise of J3-P is gone: graph `SYS_INFER_RUN` no longer carries the inference lifecycle marker and is source-equivalent to `main` through its admission call. The underlying pre-existing admission token-park kill leak remains outside this branch by PM ruling and is reviewed separately in PR #76; this RETURN does not claim that generic leak is fixed here. |
| **C1** | **CLOSED** | A waiter checks cancellation immediately after a successful `BUSY` CAS and drops `Held` on cancellation. Inventory infer and masked one-shot forwards check again inside their stack closures. Plain and constrained generation already check before prefill / at the top of the token loop. |
| **J3** | **CLOSED in the ruled B1c scope** | Every killable product caller that can reach the dedicated inference stack is under `inference_syscall`; graph infer is intentionally outside both the stack and marker. The separately tracked generic admission leak does not extend the B1c participant set. |
| **F5** | **CLOSED in the ruled B1c scope** | The scoped stack's participant lifetime, successful-acquire cancellation, owner release, marker coverage, and deterministic race discriminator are coherent. |

## 1. Narrowed lifecycle marker

### Graph route against `main`

- `syscall_dispatch` calls `sys_infer_run_impl` directly for `SYS_INFER_RUN`; only
  `SYS_GENERATE`, `_CONSTRAINED`, and `_MASKED` have an outer `inference_syscall` wrapper
  (`kernel/src/arch/x86_64/syscall.rs:983-1021`).
- `sys_infer_run_impl` preserves `main`'s prefix exactly: nonzero flags return `EINVAL`, then
  `model_lookup`, then `validate_current`, then route classification (`:2829-2850`). For a graph model,
  it calls `infer_run_routed` without a marker (`:2852-2878`).
- On the graph branch, `infer_run_routed` preserves `main`'s precedence: input-count `EINVAL`, output-cap
  `E2BIG`, length conversions/overflow `EINVAL`, input/output pointer `EFAULT`, handle-array allocation
  and copy, tensor lookup errno, graph forward/admission, output-count `EINVAL`, then output-handle
  installation (`:2902-3128`). The extraction of `graph_infer_inputs` is behavior-preserving
  (`:2811-2827`). No graph resource is allocated before the same points as on `main`, and
  `run_graph_infer` still reaches the same admission scheduler (`:4259-4277`).

### Inventory marker boundary

- The inventory branch enters `inference_syscall` immediately after classification and before pointer
  checks, the fixed handle copy, tensor lookup, token admission, or `Vec` creation (`:2847-2869`). The
  only owned local acquired first is the model `Arc`; it is cloned while the syscall thread is Running
  at entry `IF=0`, before any yield/park, so it cannot be force-zombified in that interval.
- `model_lookup` clones existing `Arc`s under process/handle locks; it neither heap-allocates nor parks
  (`:1463-1478,2106-2123`). `validate_current` performs provenance/mount/inode generation reads and
  locks but has no scheduler park or heap allocation (`kernel/src/ai/model.rs:291-294`,
  `kernel/src/fs/mod.rs:211-225`). `is_stage_b_inventory_model` is three immutable field reads
  (`syscall.rs:4255-4257`). Thus no unmarked killable suspension or resource-owning allocation precedes
  the route decision.
- Exact-call search and call-graph tracing found only two `try_run` sites: `real_lm_generate` and
  `on_inference_stack`. Their killable product callers are respectively the marked plain generate arm,
  and the marked constrained/masked arms plus the marked inventory infer branch
  (`:3413-3465,3468-3611,3622-3765,3834-3846,3898-4018`). Direct test-harness calls run as kernel pid 0,
  which `cancel_requested` deliberately excludes. No other killable path reaches `run`/`try_run`
  without the marker.

### Marked-entry probe strength

The measured deltas are meaningful for the routing inside `sys_infer_run_impl`: the supplied inventory
model increments once and the supplied graph model does not (`:2370-2382`). It is not, by itself, a
complete proof of dispatcher coverage because the probe calls `sys_infer_run_impl` directly; it could
still print `infer_graph_marked ret=0` if a future change wrapped the whole `SYS_INFER_RUN` arm outside
that function. Direct inspection of `syscall_dispatch` closes that gap on this head. This is a
non-blocking gate-hardening note, not a defect in the reviewed routing.

## 2. C1 cancellation after winning `BUSY`

### Acquisition and release

- After the successful compare-exchange and owner publication, `Held::acquire` constructs the guard,
  checks `cancel_requested`, increments `CANCELLED_AFTER_WIN` under the test feature, drops the guard,
  and returns `None` (`kernel/src/mm/inference_stack.rs:311-332`). A caller already torn down before this
  point never enters its closure.
- `Held::drop` clears `OWNER_PID`, clears `OWNER_TID`, and only then release-stores `BUSY=false`, all with
  local interrupts masked (`:346-352`). Therefore every cancelled winner releases both the lock and
  owner diagnostics before another acquirer can observe the stack free.
- The post-win load is followed by closure-local checks at both one-shot forwards: inventory infer checks
  before `qwen_inventory::run` (`syscall.rs:2881-2899`), and masked generation checks before either
  `next_logits` implementation (`:3980-3993`). Plain generation checks before prefill (`:3280-3285`),
  and constrained generation checks at the beginning of its loop before its first forward
  (`:3783-3798`). These are cooperative cancellation points; a teardown published after the final check
  is the already documented in-flight-forward case, not the C1 cancelled-waiter-after-win hole.

### `ECANCELED`

The normal inventory route preserves the outer `ECANCELED` from `inventory_infer` instead of converting
it through `infer_errno` (`:3033-3044`), and the syscall boundary exits a torn-down participant rather
than returning any errno to user mode. The preemptible-liveness shadow-forward block still converts an
outer cancellation into `InferenceError::InvalidInput` (`:2998-3027`), but that block is cfg-selected by
`neo-test-inference-preemptible-liveness`, whose declared feature set does not enable `real-lm`; it is
not an inventory caller in the scenario or product configurations. Even in an artificial combined
feature build, the marked boundary exits the torn-down caller, so the converted errno is unobservable
and no forward runs. I do not treat this test-only shadow conversion as a surviving C1 or ABI finding.

### Release-race discriminator

- E is first proved parked while holding the stack. The test then closes `WAITER_GATE_CLOSED`, creates
  F, and requires at least three gate observations, placing F before its next CAS
  (`kernel/src/test_harness.rs:34939-34956`).
- CPU 1 terminates F while the gate remains closed. Only then is E disarmed; the test waits for E to
  return and for `BUSY` to become false before opening the gate (`:34957-34975`). Therefore F's next CAS
  wins the free stack; it cannot take the failed-CAS cancellation branch.
- The gate requires `CANCELLED_AFTER_WIN` to rise by exactly one and stack calls to rise by exactly one
  (E only), plus F's boundary exit (`:34976-34990,35037-35043`). Mentally deleting the post-win check
  makes F enter `try_run_checked`; even though plain generation's own prefill check returns
  `ECANCELED`, `try_run_checked` records the completed stack call, so the call delta becomes two and the
  gate fails. Thus the gate distinguishes C1 rather than merely detecting eventual exit.
- `WAITER_GATE_CLOSED`, `WAITER_GATED`, and `CANCELLED_AFTER_WIN`, their acquire-loop hook, and the entire
  race phase are cfg-gated by `neo-test-inference-kill`; they are absent from product builds
  (`kernel/src/mm/inference_stack.rs:304-309,326-329,384-401`; `test_harness.rs:34777-35055`).

## KASLR inference-stack region

`region_candidates` now returns three checked candidates in preference order: above heap, below heap,
and below image. The shared `below` calculation uses checked subtraction for the gap and complete region,
then rounds downward, so the region end remains at least `GAP` below the relevant lower bound
(`kernel/src/mm/inference_stack_logic.rs:35-58`). `choose_region` still rejects image/heap/HHDM overlap,
noncanonical addresses, integer overflow, and any live mapping. The host KAT reproduces the failed KASLR
layout where the above slot wraps and the below-heap slot overlaps the image, then pins the valid
2 MiB-aligned below-image result `0xffffffffec000000`
(`tools/module-reclaim-logic-tests/src/lib.rs:447-469`).

## Verification

- RAG health: **healthy** (`ollama=true`, `qdrant=true`). Used for semantic discovery, followed by direct
  source reads and exact/call-graph searches.
- `cargo test -p module-reclaim-logic-tests`: **PASS** -- 28 passed, including both region-candidate KATs.
- `cargo check -p neoplutarch-kernel --target x86_64-unknown-none --no-default-features --features product,real-lm,model-boot-module`: **PASS**.
- `cargo check -p neoplutarch-kernel --target x86_64-unknown-none --no-default-features --features neo-test-inference-kill`: **PASS** (warnings only).
- `NEO_SMP=4 cargo xtask neo-test inference_stack_kill inference_stack_kill_control real_lm_generate_boot`:
  **not runnable on this worker**. `LIMINE_DIR` is unset and neither `xorriso` nor
  `qemu-system-x86_64` is installed. The PM runtime gate must supply the boot evidence.

## Merge recommendation

The two ruled B1c changes are ready to merge, subject to the PM's QEMU/runtime gate. Continue treating
the generic admission-park kill leak as PR #76's separate blocker; nothing in this RETURN closes that
underlying main behavior.
