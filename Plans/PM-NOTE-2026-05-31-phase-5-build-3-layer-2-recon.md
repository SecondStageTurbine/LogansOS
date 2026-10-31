<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PM-NOTE: Phase 5 BUILD-3 (Layer 2, CPU Inference Backend) — Pre-Design Recon

**Date:** 2026-05-31
**From:** PM/Cerebro
**Status:** RECON ONLY — no design locked in. Feeds ADR-029, which is written
**after** BUILD-2 (Layer 1, model object) merges, since Layer 2 *executes* the
BUILD-2 op-graph and must bind to its final in-memory shape.
**References:** Bible Articles 6.1, 6.5 ("correct and slow is acceptable"), 8.1/8.2
(scheduler / inference class), 20–22 (Tensor / Model / Inference Context); ADR-027
(§D layering); ADR-028 (`.neom` v1, op-graph + opcode set BUILD-3 executes).

---

## 1. Purpose

Use the BUILD-2 build-window to map the surfaces a CPU inference backend hooks
into, and to **stage the design questions** so ADR-029 + the BUILD-3 dispatch
write fast once Layer 1 is real. This note deliberately makes **no** design
decisions — it records what exists, the hard constraints, and the open questions.

## 2. OBSERVE map — what Layer 2 hooks (verified 2026-05-31)

| Surface | State | Ref |
|---|---|---|
| Scheduler | Single round-robin, **no class/priority mechanism** | `sched/scheduler.rs` |
| Kernel-thread spawn | `spawn_owned(name, job: impl KthreadJob) -> KthreadHandle` | `sched/kthread.rs:252` |
| Preemption | Timer ISR @ 100 Hz; preempts every ~10 ms unless `preempt_defer_ticks` set; handoff-defer caps at 32 ticks (~320 ms) | `sched/scheduler.rs:63–64,753–768` |
| Tensor storage | `TensorStorage { frame, order, len }`, buddy-frame backed; `Arc<Tensor>` refcount/drop | `ai/tensor.rs:45–130` |
| Contiguous alloc | `with_frame_alloc(\|a\| a.allocate_blocks(order))`, **max 64 MiB** (MAX_ORDER=14) | `mm/frame.rs:56`, `mm/mod.rs:387` |
| Kernel heap | `Vec`/`Box` available, **4 MiB** total, no per-process accounting | `mm/heap.rs:30` |
| Phys→virt | `phys_to_virt(PhysAddr) -> VirtAddr` | `mm/mod.rs:432` |
| AI syscall range | `0x100..0x17F`; tensors `0x100..0x103`, models `0x110..0x11F` (BUILD-2). Inference free at **`0x120..0x12F`** | `syscall.rs:75` |
| Host op-test pattern | `#[path=...]` import + `#[cfg(test)]`, like `tools/tensor-logic-tests` | `tools/tensor-logic-tests/` |

## 3. Hard constraints (must shape ADR-029)

1. **★ Floating-point state is NOT preserved across context switches.** Boot sets
   CR0.EM=0 / CR4.OSFXSR (FPU/SSE on), but `context_switch` saves only integer
   callee-saved registers — **no FXSAVE/FXRSTOR, no MXCSR/x87 save**
   (`arch/x86_64/context.rs:19–69`, with a standing comment that the routine
   "becomes wrong" the moment the kernel emits float). The kernel currently emits
   **zero** float math (only int/pointer). A CPU inference backend is the first
   float-emitting kernel code — so this constraint is *the* Layer 2 decision.
2. **No inference scheduling class.** Article 8.2 names one; none exists. Every
   kthread competes equally; long compute starves peers unless it defers
   preemption (and the defer cap is ~320 ms).
3. **No mutable-tensor write API.** `Tensor` has a `mutable` flag but no
   `bytes_mut()` / writable-allocate path — intermediates can't be written today.
4. **No device registry / accelerator dispatch.** `Device::Accel(u32)` is a bare
   reserved variant; tensors hardcode `Device::Cpu`.
5. **Buffer ceilings.** 64 MiB max contiguous buddy block; 4 MiB kernel heap.
   Real model intermediates can exceed these — sizing matters.
6. **Syscalls run synchronously in the caller's kernel context** and are
   preempted every 10 ms; there is no async syscall path.

## 4. Staged design questions for ADR-029 (open — not decided here)

**Q1 — Floating-point strategy (the pivotal one; likely needs PL input).**
Options, with trade-offs:
- **(a) soft-float op kernels** — no SSE/AVX; integer/software float math.
  *Sidesteps constraint #1 entirely* (no XMM to save), and Article 6.5
  ("correct and slow is acceptable") explicitly blesses it. Slowest; but Layer 2
  is the *correctness* baseline, with accelerators (Phase 6) for speed.
- **(b) FXSAVE/FXRSTOR in `context_switch`** — kernel-wide architecture change;
  makes hardware float safe everywhere, at a per-switch cost on the hot path.
- **(c) FP save/restore scoped to the inference region** + preemption handling
  inside it — narrower than (b), but intricate (preemption mid-op still corrupts).
- **(d) preempt-defer around op kernels** — fragile; a big matmul exceeds the
  ~320 ms defer cap and starves the system.
*Leaning question to resolve in ADR-029:* does Layer 2 take **(a) soft-float** as
the lean correctness-first baseline (deferring hardware-float/FXSAVE to a later
perf pass), or pay for **(b)** now? This is the highest-leverage decision and the
one most worth a PL call, since (b) touches the whole kernel.

**Q2 — Execution model.** `inf_run` (Article 22.1) returns a result, implying
**synchronous in the caller's context**. For Layer 2 (single-tenant, no class),
synchronous is simplest and matches the signature; the kthread/async path and the
8.2 inference class are Phase 7 (multi-tenant serving). *Question:* confirm
synchronous-blocking for Layer 2; defer scheduled/async inference to Phase 7.

**Q3 — Op-kernel dispatch.** OpCode → kernel function. A `match` on the ADR-028
opcode enum is leanest (no device registry yet, per constraint #4); a
function-pointer table or `DeviceOps` trait is accelerator-ready but premature.
*Question:* `match` now vs. a dispatch table.

**Q4 — Intermediate/activation tensors.** Layer 2 must **add a writable-tensor
API** to `tensor.rs` (`allocate_writable(shape,dtype)` + a `mutable`-guarded
`bytes_mut()`), which BUILD-1 deferred. Lifetime options: (a) `Arc<Tensor>`
intermediates dropped as each consumer completes (reuses BUILD-1 refcount, lower
peak memory, more churn); (b) a per-`inf_run` scratch arena freed wholesale at
call end (simpler lifetime, higher peak). *Question:* per-tensor Arc vs.
per-call arena. Note: this is also where BUILD-1's deferred immutability
*enforcement* finally gets a mutator to guard against (weights immutable, only
activation tensors writable).

**Q5 — `inf_run` ABI.** Article 21.3: "inference produces per-call output tensors
owned by the calling process." So the kernel **allocates outputs, installs them as
TensorCaps, returns handles** (rather than the caller pre-allocating). Inputs are
arrays of tensor handles. *Question:* exact marshalling (handle arrays + counts +
flags), and whether outputs are kernel-allocated-and-returned (favored) vs.
caller-provided.

**Q6 — Inference vs. Article 8.2 scheduling class.** 8.2 wants inference to be a
scheduling class; none exists and building the multi-class scheduler is a large
Phase 7 effort. *Question (near-certain answer, but flag it):* Layer 2 runs
inference **best-effort** with no scheduling class; the 8.2 class + multi-tenant
fairness is **Phase 7**. ADR-029 should state this boundary explicitly so we don't
half-build a scheduler.

**Q7 — Numerical correctness & testing.** Mirror `tools/tensor-logic-tests`: a
host `tools/cpu-ops-tests` crate testing matmul/add/relu/softmax/layernorm against
known values. *Question:* test oracle source — hand-computed small cases
(zero deps, fully deterministic) vs. a dev-only reference crate (ndarray/nalgebra)
for cross-check. Determinism/reproducibility is a requirement either way.

**Q8 — Syscall + Bible.** Inference sub-block **`0x120..0x12F`**; needs an Article
6.1 range note (as models will get) and realizes Article 22 (Inference Context).
Minor; bundle the Bible touch with the BUILD-3 opening.

## 5. Anti-scope to carry into ADR-029 / the dispatch

- No accelerator drivers / `Device::Accel` backing (Phase 6).
- No multi-tenant inference scheduler or 8.2 scheduling class (Phase 7).
- No governance/LogitPort.
- No new model formats (executes ADR-028 `.neom` graphs only).

## 6. Sequencing

Hold ADR-029 + the BUILD-3 dispatch until **BUILD-2 (Layer 1) merges** — Layer 2
binds to the merged `Model`/op-graph types, and designing against an in-flight
shape invites rework. When Layer 1 lands, ADR-029 resolves Q1–Q8 (Q1 ideally with
a PL call) and the dispatch follows. This note is the OBSERVE bank so that step is
fast.
