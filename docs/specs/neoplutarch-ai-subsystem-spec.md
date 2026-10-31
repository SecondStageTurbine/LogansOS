# LogansOS AI Subsystem

## Design Specification

---

**Document ID:** ANSPEC-001
**Status:** Draft
**Authority:** Derived from LogansOS Bible, Articles 2, 6, 19–23
**Target phases:** 5 (foundation), 6 (accelerators), 7 (scheduler & multi-tenancy), 8 (inference serving)
**Version:** 0.3.10
**Last revised:** 2026-06-18
**Amendments:** v0.3.0 (2026-05-05) — §3.7 reconciled with ADR-010 two-tier rights model; ANSPEC-002 introduced. v0.3.1 (2026-06-14) — §11.1 added autoregressive generation. v0.3.2 (2026-06-15) — §11.2 added the default-off GGUF importer and BPE tokenizer. v0.3.3 (2026-06-16) — §11.2.1 added the GGUF dequant-to-f32 matmul integration seam. v0.3.4 (2026-06-16) — §11.2.2 added the RoPE/SwiGLU real-weight layer scaffold with attention left unsupported. v0.3.5 (2026-06-16) — §11.2.3 added GQA attention, governed Option A KV cache, and real Qwen top-1 generation. v0.3.6 (2026-06-17) — §11.6 added the Arthur action-surface disposition layer. v0.3.7 (2026-06-17) — ADR-056 recorded the Phase 8 Rule-4 consolidation: shared KV-cache layout factoring and blocking workspace/kernel/no_std hygiene gates, with no behavior change. v0.3.8 (2026-06-17) — §11.2.4 added the default-off Phase 9.1 scalar Q4_K x Q8_K helper and q8_K activation quantizer, with forward wiring deferred. v0.3.9 (2026-06-18) — §11.2.4 records Phase 9.2 forward wiring and Phase 9.3 host-tier AVX2 equality, with the in-kernel AVX2 swap blocked on YMM-state preservation. v0.3.10 (2026-06-18) — §11.7 records ADR-057 CSPRNG 10.1 as an external security prerequisite for later Arthur/W2 randomness consumers; ADR-059 records the Phase 8/9 Rule-4 consolidation.

---

## 0. Document Purpose

This specification defines the AI/ML subsystem of LogansOS: its kernel-resident objects, syscall surface, scheduling model, memory model, accelerator driver interface, and security boundaries. It is the authoritative design reference for Phase 5 through Phase 7 implementation.

It does not specify *how* matrix multiplication is performed on a given accelerator — that is the driver's concern. It specifies what the kernel guarantees about tensors, models, and inference, regardless of where the math runs.

This document is normative. Code that contradicts this spec is wrong; either the code changes or this spec is amended via the standard ADR + bible-amendment process.

---

## 1. Rationale

### 1.1 Why kernel-resident AI

The conventional separation places AI/ML entirely in userspace: a process owns the GPU, links a runtime (CUDA, ROCm, Metal, DirectML, ONNX Runtime), loads a model, and runs inference. The kernel is a passive resource provider. This is the model used by Linux, Windows, and macOS.

This model has three failures that LogansOS rejects:

**1.1.1 Accelerator monopoly.** Userspace ownership of accelerators means one process at a time, or cooperative time-slicing implemented in userspace runtimes that don't trust each other. Multi-tenant inference at the OS level is impossible without a kernel arbitrator.

**1.1.2 Memory waste.** A 70B-parameter model occupies roughly 140 GB at f16. Two processes each loading "the same" model load it twice, because the kernel has no concept of model identity — to the kernel, it's two anonymous mmap regions. Content-deduplication of model weights is the kind of optimization that only the kernel can perform safely.

**1.1.3 Scheduling blindness.** The kernel's scheduler sees a thread blocked on a GPU dispatch as a thread blocked on I/O. It cannot prioritize an interactive inference call over a batch one because it does not know either is happening. Latency-sensitive AI workloads (speech, agent loops, real-time generation) are first-class scheduling problems and the kernel must see them as such.

### 1.2 Why not microkernel-style AI services

A microkernel approach would make the AI subsystem a userspace service that other userspace clients call via IPC. This is rejected because:

- Inference data is large. IPC overhead dominates for tensor-passing.
- The service-process becomes a single point of contention and failure.
- We end up rebuilding a kernel-equivalent inside userspace.

Monolithic kernel placement keeps tensor and model objects directly addressable by inference syscalls without a server hop.

### 1.3 What stays in userspace

Three categories remain firmly in userspace:

- **Model conversion and tooling.** Importing ONNX, GGUF, SafeTensors, etc. into `.neom` format is a userspace job.
- **Application logic.** Prompt construction, token decoding strategies (sampling, beam search), agent orchestration, fine-tuning loops — userspace.
- **Training.** LogansOS's kernel does not provide training primitives in the foreseeable roadmap. Inference is the kernel's concern. Training, if ever supported, is a separate subsystem.

---

## 2. Subsystem Overview

The AI subsystem is composed of three layers and one driver class.

```
+-----------------------------------------------------------+
|                      Userspace                            |
|  applications -- libneo::ai -- syscall stubs              |
+----------------------------+------------------------------+
                             | syscalls 0x100..0x1FF
+----------------------------+------------------------------+
|                  AI Subsystem (kernel)                    |
|                                                           |
|   Layer 2: Inference Contexts (sched/inference.rs)        |
|   Layer 1: Models       (ai/model.rs)                     |
|   Layer 0: Tensors      (ai/tensor.rs)                    |
|                                                           |
|        +-------- accel driver class -----------+          |
|        | accel_sim | virtio-accel | amd | … |          |
|        +------------------------------------- -+          |
+-----------------------------------------------------------+
                             |
+----------------------------+------------------------------+
|                       Hardware                            |
|   CPU SIMD | GPU | NPU | TPU | dedicated AI accelerator   |
+-----------------------------------------------------------+
```

**Layer 0 — Tensors.** The fundamental data primitive. Every quantity that flows through the subsystem is a tensor: weights, activations, inputs, outputs, KV-cache entries.

**Layer 1 — Models.** A loaded computation graph plus its bound weight tensors. Models are the unit of "what to run."

**Layer 2 — Inference contexts.** A scheduled execution of a model with specific inputs producing specific outputs. The unit of "when and where to run it."

**Accel driver class.** The hardware-specific implementation of tensor operations, exposed to the kernel through a uniform trait.

---

## 3. Layer 0 — Tensor Primitive

### 3.1 Object structure

```
struct Tensor {
    id:        TensorId,         // kernel-unique, non-recycled u64
    shape:     Shape,            // up to rank 8
    dtype:     DType,            // enumerated
    device:    DeviceId,         // accelerator or CPU
    storage:   StorageRef,       // refcounted physical region
    flags:     TensorFlags,      // immutable, weight, activation, …
    refcount:  AtomicUsize,
    owner:     Option<ProcessId>, // None for global (kernel-owned, e.g., model weights)
}
```

### 3.2 Shape

`Shape` is a fixed-size struct holding rank (0..=8) and an array of 8 `u64` dimensions. Dimensions beyond `rank` are unused. Rank-0 tensors (scalars) are valid.

The maximum rank of 8 is deliberate. Rank exists for matmul (2), batch-matmul (3), attention (4), and convolution (4–5). Rank 8 leaves substantial headroom without permitting unbounded shape descriptors.

### 3.3 DType

The enumerated dtype set, in priority order of support requirement:

| Dtype | Bits | Required from Phase | Use |
|-------|------|---------------------|-----|
| `f32` | 32 | 5 | Reference precision; CPU baseline |
| `f16` | 16 | 5 | Modern inference default |
| `bf16` | 16 | 5 | Modern training-derived weights |
| `i8` | 8 | 6 | Quantized weights, INT8 inference |
| `u8` | 8 | 5 | Image input, byte buffers |
| `i32` | 32 | 5 | Indices, token ids |
| `bool` | 1 (packed) | 5 | Masks |
| `i4` (packed) | 4 | 7 | Aggressive quantization |
| `f8_e4m3` / `f8_e5m2` | 8 | 7 | Modern accelerator native |

Dtypes are extensible; adding one requires an ADR and a kernel version bump but not a bible amendment.

### 3.4 Device residency

Each tensor lives on exactly one device at a time. `DeviceId` is a kernel-assigned identifier:

- `DeviceId::Cpu` — host RAM, accessible via the kernel's normal memory model.
- `DeviceId::Accel(n)` — accelerator-resident memory; access goes through the driver.

A tensor may be **migrated** between devices via `tensor_migrate()` (see §6.2.4). Migration is explicit; the kernel never silently moves tensors.

A tensor may be **mirrored** in some configurations (CPU-mapped GPU memory, unified memory architectures). Mirroring is a driver capability and is reported via the device descriptor; the abstract tensor model treats mirrored tensors as residing on a virtual `Coherent(devices)` device.

### 3.5 Storage and reference counting

`StorageRef` points to a refcounted physical region. Multiple tensor objects may share a storage region — this is how views (slices, reshapes, transposes that don't require data movement) are represented. The shape/dtype/strides differ; the storage is shared.

When the last tensor referencing a storage drops, the storage is freed (accelerator memory returned to the device's allocator, host memory returned to the frame allocator's tensor pool).

### 3.6 Flags

```
bitflags! {
    struct TensorFlags: u32 {
        const IMMUTABLE      = 1 << 0; // contents fixed for tensor lifetime
        const WEIGHT         = 1 << 1; // belongs to a model's weight set
        const ACTIVATION     = 1 << 2; // intermediate inference value
        const INPUT          = 1 << 3; // model input
        const OUTPUT         = 1 << 4; // model output
        const SHARED         = 1 << 5; // shared across processes
        const DEDUP_CANDIDATE = 1 << 6; // eligible for content-hash dedup
        const PINNED         = 1 << 7; // not migratable, not pageable
    }
}
```

Weight tensors are by convention `IMMUTABLE | WEIGHT | DEDUP_CANDIDATE | PINNED`.

### 3.7 Capabilities and handles

Userspace never holds a raw `TensorId`. It holds a **capability handle** — a per-process integer that the kernel maps to a `(TensorId, rights)` pair. Tensor capability rights follow the two-tier model defined in ADR-010 §Q3: a uniform meta byte (`USE | DERIVE | GRANT | REVOKE`) governs universal capability operations; a per-variant semantic byte governs tensor-specific operations.

Tensor semantic-rights, per ANSPEC-002 §3.2:

- `READ` — may sample tensor contents into userspace (`sys_tensor_read`).
- `WRITE` — may modify tensor contents (`sys_tensor_write`); requires non-`IMMUTABLE` tensor.
- `MIGRATE` — may move the tensor's storage between devices (`sys_tensor_migrate`); requires non-`PINNED` tensor.
- `VIEW` — may create a derived view (`sys_tensor_view`) sharing the same storage.

Reserved-bit semantic-rights (per ADR-010 Principle 2; activated in later phases):

- `PIN`, `DEDUP_HINT` — see ANSPEC-002 §3.2.

A capability handle without `READ` cannot be inspected; without `WRITE` cannot be modified; without `meta::GRANT` cannot be passed to another process. This is the standard capability discipline applied to tensors. Cross-process handle transfer uses the universal `sys_grant` / `sys_recv_grant` from ADR-010 §Q2 with the three-way intersection rule from ADR-010 Principle 1.

The full rights enumeration, attenuation patterns, and view-inheritance rule are specified in ANSPEC-002 §3.

### 3.8 Lifetime and ownership

A tensor is owned by:

- A **process**, if created by that process and not shared. Dropped when the process exits or the last handle is closed.
- A **model**, if it is a weight tensor of a loaded model. Dropped when the model is unloaded and no other model references the same storage (deduplication).
- The **kernel**, if it is a global named tensor (rare; reserved for system models like a default tokenizer if ever shipped — currently none).

There is no garbage collection. Refcounts and explicit drops handle all lifecycle.

---

## 4. Layer 1 — Model Object

### 4.1 The `.neom` format

Models are loaded from `.neom` files. The format is binary, versioned, and documented in `docs/specs/neom-format.md` (Phase 5 deliverable). High-level structure:

```
+------------------+
| header           |  magic, version, endianness, hash
+------------------+
| metadata         |  name, author, license, capabilities required
+------------------+
| graph            |  nodes (ops) and edges (tensor flow)
+------------------+
| weight directory |  offset/length/hash of each weight tensor
+------------------+
| weight data      |  packed tensor storage
+------------------+
```

The graph is a DAG. Nodes are ops drawn from the kernel's supported op set (§4.4). Edges carry tensors typed by shape and dtype.

### 4.2 Native vs imported

`.neom` is the native executable format. Userspace tools (`neo-import`, future)
convert ONNX, SafeTensors, PyTorch checkpoints, and other authoring formats into
`.neom` for normal execution. Phase 8 adds one bounded compatibility exception:
the default-off GGUF importer in §11.2 can parse external quantized-model bytes
into the existing Model object for enumeration and later dequant, but GGUF does
not become native and GGUF graphs are not executed directly. This factoring is
non-negotiable: the native execution path stays `.neom`, and importers must stay
bounded compatibility layers rather than expanding the kernel into an
N-format runtime.

### 4.3 Loading

```
sys_model_load(neom_path: *const u8, path_len: usize, flags: u32) -> Result<ModelHandle>
```

Loading proceeds in phases:

1. **Open and validate.** Read header, check magic and version, verify file-level hash.
2. **Parse graph.** Build the in-kernel graph representation. Reject cycles, unsupported ops, shape mismatches.
3. **Plan placement.** Choose a device or device set for the model based on memory availability and policy hints.
4. **Allocate weight tensors.** For each weight, check the deduplication cache (keyed by content hash). On hit, refcount the existing storage. On miss, allocate fresh storage and copy weights from the file.
5. **Bind.** Construct the `Model` object referencing the graph and weight tensors, return a capability handle.

Failure at any step releases all partial allocations.

### 4.4 Op set

The kernel-supported op set is the union of:

**Tier 1 — required from Phase 5:**

`add`, `sub`, `mul`, `div` (elementwise),
`matmul`, `batched_matmul`,
`reduce_sum`, `reduce_mean`, `reduce_max`,
`softmax`, `layernorm`, `rmsnorm`,
`relu`, `gelu`, `silu`, `tanh`, `sigmoid`,
`reshape`, `transpose`, `slice`, `concat`, `split`,
`embedding_lookup`,
`attention` (scaled dot-product, fused).

**Tier 2 — required from Phase 7:**

`conv2d`, `conv1d`,
`group_norm`, `batch_norm` (inference-mode),
`rope` (rotary position embedding, fused),
`flash_attention` (memory-efficient attention),
`kv_cache_append`, `kv_cache_lookup`,
`quantize_dynamic`, `dequantize`.

**Tier 3 — extensible:**

Additional ops may be added by ADR. The op set is intentionally finite and curated; "any op a model needs" is not the design goal. Models that require unsupported ops fail to load with a clear diagnostic naming the missing op.

### 4.5 Graph representation

Internally, a model's graph is stored as:

```
struct Graph {
    nodes:   Vec<OpNode>,        // topologically sorted
    inputs:  Vec<TensorDescriptor>,  // shape/dtype of expected inputs
    outputs: Vec<TensorDescriptor>,  // shape/dtype of produced outputs
    weights: Vec<TensorHandle>,  // bound weight tensors
}

struct OpNode {
    op:       OpKind,            // discriminant from §4.4
    inputs:   Vec<TensorRef>,    // refs to other nodes' outputs, weights, or graph inputs
    outputs:  Vec<TensorDescriptor>,
    attrs:    OpAttrs,           // op-specific (kernel size, axis, etc.)
}
```

Topological sort happens at load time and is fixed. The kernel does not re-plan graph execution order at runtime.

### 4.6 Sharing

Models are inherently shareable. A model loaded by process A may be used by process B if:

1. A grants a model capability handle to B (via the standard cap-passing IPC, Phase 2).
2. The handle's permissions include `INVOKE`.

The model's weight storage is shared without copy. Inference produces per-call output tensors owned by the *caller*, not the model.

### 4.7 Unloading

A model is unloaded when its last handle is closed. The graph is freed; weight tensors are decrefed. Storages with refcount zero are returned to the device allocator. Storages still referenced (by another model with the same weights, by deduplication) remain.

---

## 5. Layer 2 — Inference Context

### 5.1 The inference call

```
sys_inf_run(
    model:    ModelHandle,
    inputs:   *const InputBinding,
    n_inputs: usize,
    outputs:  *mut OutputBinding,
    n_outputs: usize,
    flags:    InfFlags,
    deadline: Option<Deadline>,
) -> Result<InfStats>
```

`InputBinding` and `OutputBinding` are `(name, tensor_handle)` pairs. The kernel resolves the bindings against the model's declared inputs/outputs.

### 5.2 Synchronous vs asynchronous

By default, `sys_inf_run` is synchronous: the calling thread blocks until the inference completes (or the deadline expires).

Asynchronous mode is selected via `InfFlags::ASYNC`:

```
sys_inf_submit(...) -> Result<InfTicket>
sys_inf_wait(ticket: InfTicket, timeout) -> Result<InfStats>
sys_inf_cancel(ticket: InfTicket) -> Result<()>
```

The ticket is itself a capability and may be passed between processes (one process submits, another awaits — useful for service architectures).

### 5.3 Streaming

Some models produce token-by-token output (autoregressive language models). The streaming API exposes this:

```
sys_inf_stream_open(model, inputs, flags) -> Result<StreamHandle>
sys_inf_stream_next(stream, output_buf, max_tokens) -> Result<TokensProduced>
sys_inf_stream_close(stream) -> Result<()>
```

Streaming is built atop the async machinery: each `next` call is a bounded continuation of the inference.

### 5.4 Deadlines and priority

Each inference call may carry a deadline (absolute wall-clock time) and priority class:

```
enum InfPriority {
    Realtime,    // hard deadline; preempts everything except other Realtime
    Interactive, // soft deadline; preempts Compute and Background
    Compute,     // best-effort throughput
    Background,  // yields readily
}
```

The inference scheduler (§7) honors these. A Realtime inference call missing its deadline is a logged event; the call still completes, but the application is informed via `InfStats`.

### 5.5 Accounting

Every completed inference returns `InfStats`:

```
struct InfStats {
    wall_time_ns:        u64,
    accel_time_ns:       u64,
    cpu_time_ns:         u64,
    peak_memory_bytes:   u64,
    ops_executed:        u64,
    deadline_met:        bool,
    device:              DeviceId,
}
```

These statistics are also rolled up per-process and per-model for system-wide observability.

---

## 6. Syscall Surface

### 6.1 Numbering

The AI subsystem occupies syscall numbers `0x100..0x1FF`. Within that range:

| Range | Use |
|-------|-----|
| `0x100..0x11F` | Tensor operations |
| `0x120..0x13F` | Model operations |
| `0x140..0x15F` | Inference operations |
| `0x160..0x17F` | Device/accelerator queries |
| `0x180..0x1FF` | Reserved for future expansion |

Unused numbers in each band are reserved; they do not return ENOSYS silently — they return `EAINVAL` (AI-subsystem invalid) so that probing the syscall space cannot be confused with absent features.

### 6.2 Tensor syscalls

#### 6.2.1 `sys_tensor_create`

```
sys_tensor_create(shape: *const Shape, dtype: DType, device: DeviceId, flags: TensorFlags)
    -> Result<TensorHandle>
```

Creates a fresh tensor with uninitialized storage on the named device.

#### 6.2.2 `sys_tensor_write`

```
sys_tensor_write(tensor: TensorHandle, src: *const u8, len: usize, offset: usize)
    -> Result<usize>
```

Copies `len` bytes from userspace `src` into the tensor's storage at `offset`. Requires `WRITE` permission and a non-`IMMUTABLE` tensor.

#### 6.2.3 `sys_tensor_read`

```
sys_tensor_read(tensor: TensorHandle, dst: *mut u8, len: usize, offset: usize)
    -> Result<usize>
```

Copies `len` bytes from the tensor's storage at `offset` into userspace `dst`. Requires `READ` permission.

#### 6.2.4 `sys_tensor_migrate`

```
sys_tensor_migrate(tensor: TensorHandle, target: DeviceId) -> Result<()>
```

Moves the tensor's storage to `target`. Allocates new storage on the target device, copies, frees the old. Fails on `PINNED` tensors.

#### 6.2.5 `sys_tensor_view`

```
sys_tensor_view(base: TensorHandle, view_spec: *const ViewSpec) -> Result<TensorHandle>
```

Creates a new tensor handle that references the same storage as `base` but with a different shape/strides/offset. View creation does not allocate or copy. The view inherits permissions from `base`.

#### 6.2.6 `sys_tensor_close`

```
sys_tensor_close(tensor: TensorHandle) -> Result<()>
```

Releases the handle. Decrements the refcount; storage is freed if this was the last reference.

### 6.3 Model syscalls

#### 6.3.1 `sys_model_load`

See §4.3.

#### 6.3.2 `sys_model_info`

```
sys_model_info(model: ModelHandle, info_out: *mut ModelInfo) -> Result<()>
```

Returns metadata: name, version, input/output descriptors, op count, weight memory size, supported devices.

#### 6.3.3 `sys_model_unload`

```
sys_model_unload(model: ModelHandle) -> Result<()>
```

Releases the model handle. Behaves like `sys_tensor_close` for the model's resources.

### 6.3.4 Stack-frame discipline for blocking syscalls

Model and tensor syscalls (`sys_model_load`, `sys_tensor_create`, future inference syscalls) may block on disk I/O (Phase 3+), accelerator memory allocation (Phase 6+), or weight-tensor migration. While a thread is blocked inside a syscall handler, its kernel stack is preserved verbatim. Any data stack-allocated in the handler before the blocking point stays kernel-resident for the entire duration of the block.

**Implementation rule.** Large data structures (model headers, weight descriptors, tensor metadata buffers, intermediate parse state) MUST be heap-allocated or accessed by reference, not stack-allocated. The `EndpointState` pattern from ADR-004 (store the user-buffer pointer and length, not the contents) is the correct shape; this rule extends it to every syscall handler that may block.

**Why.** Variables held across a blocking point cost kernel-stack memory in proportion to block duration, not active work. A handler that stack-allocates a 4KB parse buffer and then blocks for seconds on disk I/O ties up 4KB of kernel stack per blocked thread. Multiplied across many concurrent inferences, this becomes meaningful pressure on the kernel-stack pool. Reference-passing or heap-allocation eliminates the class.

**Anti-pattern to avoid:**

```rust
fn sys_model_load_impl(path: *const u8, len: usize) -> i64 {
    let mut header_buf = [0u8; 4096];           // stack-allocated, held across block
    read_from_disk(&mut header_buf).await_kernel(); // blocks; header_buf still on stack
    parse_header(&header_buf)
}
```

**Correct pattern:**

```rust
fn sys_model_load_impl(path: *const u8, len: usize) -> i64 {
    let header_buf = Box::new([0u8; 4096]);    // heap-allocated; stack holds only the Box
    read_from_disk(header_buf.as_mut_slice()).await_kernel();
    parse_header(header_buf.as_slice())
}
```

The same discipline applies to any future syscall whose handler may block. The principle generalizes: data held across suspension points is paid for in stack memory until the syscall returns; pass references or heap-allocate when the data is large.

This rule is enforced by review, not by the type system. Phase 5+ implementations of these syscalls cite this section in their PR descriptions.

### 6.4 Inference syscalls

See §5.1–5.3.

### 6.5 Device syscalls

#### 6.5.1 `sys_accel_enumerate`

```
sys_accel_enumerate(out: *mut AccelDescriptor, max: usize) -> Result<usize>
```

Lists available accelerators. Returns count written.

#### 6.5.2 `sys_accel_info`

```
sys_accel_info(device: DeviceId, info_out: *mut AccelInfo) -> Result<()>
```

Returns device-specific information: memory size, supported ops, supported dtypes, current load.

---

## 7. Inference Scheduler

### 7.1 Scheduling problem

At any given moment the kernel may have:

- Multiple processes with multiple inference calls in flight or pending.
- Multiple accelerators of varying capability and load.
- Mixed priorities and deadlines.
- Mixed sizes — some calls run in microseconds (small classifier), some in seconds (large LLM generation).

The inference scheduler decides:

1. **Where** each pending op runs (which accelerator).
2. **When** it runs relative to other pending ops.
3. **Whether** to preempt a running op for a higher-priority arrival.

### 7.2 Per-accelerator queues

Each accelerator has a queue of pending op-batches. The scheduler dispatches batches to accelerators by:

- Priority class (realtime > interactive > compute > background).
- Within a class: earliest-deadline-first.
- Tie-break: arrival order (FIFO).

### 7.3 Op vs model granularity

Scheduling happens at op-batch granularity, not whole-model granularity. A long-running model (a 70B LLM forward pass) does not block a small interactive model (a wake-word classifier) — the small model's ops interleave at op boundaries.

This requires that op execution be reasonably short. Ops that would individually take longer than a scheduling quantum (~10ms) are split by the scheduler into sub-ops where the algorithm permits (matmul tiling).

### 7.4 Preemption

True preemption of in-flight accelerator work is hardware-dependent. Where the accelerator supports it (newer GPUs with compute preemption), the scheduler uses it. Where it does not, preemption happens at op-batch boundaries — the next batch goes to the higher-priority work.

This is an explicit limitation. Realtime-priority inference on a non-preemptible accelerator can be delayed by up to one op-batch's worth of time. Drivers report this latency bound; userspace can choose accelerators based on it.

### 7.5 Memory pressure

When accelerator memory is exhausted, the scheduler may:

1. Refuse new model loads on that device until pressure eases.
2. Migrate idle (no in-flight inference) models to a less-loaded device.
3. Evict deduplicated copies, falling back to disk reload on next use.

Active inference calls are never evicted mid-flight; the scheduler waits for completion.

### 7.6 Interaction with the main scheduler

The inference scheduler is a sibling to the main thread scheduler, not a subordinate. They communicate:

- A thread blocked on a synchronous inference call is parked by the main scheduler and woken by the inference scheduler upon completion.
- An async inference call does not park the issuing thread.
- Inference completion may itself trigger thread wake-ups via the standard wait mechanism.

---

## 8. Memory Model

### 8.1 The tensor allocation class

Tensor storage is a distinct allocation class. It is *not* served from the kernel's general slab/buddy allocator. Reasons:

- Tensor allocations are large (KB to GB).
- Tensor allocations have device residency that the general allocator does not understand.
- Tensor allocations benefit from content-hash deduplication that the general allocator should not implement.

The tensor allocator is a per-device subsystem in `kernel/src/ai/alloc.rs`. For CPU-resident tensors, it draws from a reserved fraction of physical RAM (configured at boot). For accelerator-resident tensors, each driver owns its device-memory allocator and exposes a uniform interface.

### 8.2 Deduplication

Weight tensors with `DEDUP_CANDIDATE` are content-hashed (BLAKE3) at load time. The hash is consulted against a per-device deduplication table:

- **Hit:** the existing storage is shared (refcount incremented). No allocation, no copy.
- **Miss:** fresh allocation and copy. The hash is registered in the table.

When refcount drops to zero, the entry is removed from the table.

This means loading the same 70B model twice costs exactly one copy plus one hash table entry. Loading a 70B model and a 70B fine-tune of it costs roughly *one* copy plus the diff (in practice, fine-tunes share most layers — this is a future optimization at the per-tensor level).

### 8.3 Userspace mapping

A capability handle does not by itself map tensor memory into userspace. Userspace reads and writes via `sys_tensor_read`/`sys_tensor_write` syscalls.

For high-throughput cases, a `sys_tensor_map(handle) -> *const u8` syscall maps the storage into the calling process's address space, read-only or read-write per capability permissions. Mapping is permitted only for CPU-resident tensors; accelerator memory is never mapped to userspace (this is a security and portability invariant).

### 8.4 Pinning and migration

`PINNED` tensors are not migratable and not pageable. Weight tensors are typically pinned because in-flight inference depends on stable addresses.

Activations are not pinned by default; they are short-lived and the scheduler may relocate them between op boundaries to consolidate memory.

### 8.5 Out-of-memory behavior

On accelerator OOM, model load fails with `EAINOMEM`. Userspace may retry on a different device or after waiting.

On host OOM during tensor allocation, the kernel does not invoke an OOM-killer in the Linux sense. LogansOS's policy is: the requesting process gets `EAINOMEM`, period. Killing other processes to make room for a new one is a policy nightmare we refuse to inherit.

---

## 9. Accelerator Driver Interface

### 9.1 The `accel` driver class

Drivers in `kernel/src/drivers/accel/` implement the `AccelDriver` trait:

```
trait AccelDriver: Send + Sync {
    fn descriptor(&self) -> AccelDescriptor;
    fn supported_ops(&self) -> &[OpKind];
    fn supported_dtypes(&self) -> &[DType];
    fn alloc(&self, size: usize, align: usize) -> Result<DeviceStorage>;
    fn free(&self, storage: DeviceStorage);
    fn copy_to_device(&self, dst: &DeviceStorage, src: &[u8]) -> Result<()>;
    fn copy_from_device(&self, dst: &mut [u8], src: &DeviceStorage) -> Result<()>;
    fn dispatch(&self, batch: &mut OpBatch) -> Result<DispatchToken>;
    fn poll(&self, token: DispatchToken) -> DispatchStatus;
    fn wait(&self, token: DispatchToken, timeout: Option<Duration>) -> Result<()>;
    fn preemptible(&self) -> bool;
    fn preempt(&self, token: DispatchToken) -> Result<()>;
}
```

`dispatch` may return before asynchronous work is complete. In that case the
caller must keep the `OpBatch` output slice live and unaliased until `poll` or
`wait` reports `Complete` or `Failed`; output bytes are not valid before
completion. `wait(token, None)` completes or fails the operation before
returning.

### 9.2 Required initial drivers

**Phase 6 BUILD-1:** CPU backend (`accel_cpu`) — reference implementation in pure Rust SIMD. Slow but correct. Used for validation and as fallback.

**Phase 6 BUILD-2:** `accel_sim` — an in-kernel reference accelerator on a non-CPU `DeviceId::Accel(n)` with its own device-memory arena and *simulated asynchronous* completion. No hardware or host dependency; validates the logical multi-device + async + cross-device-migration machinery in CI before any transport work. (The `accel` trait's async surface — `poll`/`wait` returning `Pending`→`Complete`, and non-identity `copy_to/from_device` between distinct arenas — is first genuinely exercised here; `accel_cpu` stubbed all of it.)

**Phase 6 BUILD-3:** a **custom virtio-accel device** for QEMU/KVM testing. *(Originally named "virtio-gpu-compute"; no such device exists — QEMU's virtio-gpu is graphics-only.)* We define our own virtio device whose data plane is served by a **vhost-user daemon**, using QEMU's user-creatable `vhost-user-test-device-pci` (arbitrary `virtio-id`/`num_vqs`, **no QEMU fork required**) and reusing the kernel's existing virtio-pci transport (virtio-blk/net). Validates the real hardware transport path — virtqueue / DMA buffers / IRQ-driven completion — in CI. Gets its own substrate ADR with a hand-laid byte oracle for the device protocol (ADR-031; design rationale in `Plans/THINK-2026-06-04-phase-6-build-2-virtio-accel.md`).

**Phase 6 BUILD-4:** one real GPU. Candidate is whichever has the cleanest open documentation at the time (likely an Intel or AMD GPU; NVIDIA is intentionally lower priority due to driver opacity).

**Phase 7+:** additional accelerators by demand and contributor availability.

### 9.3 Driver constraints

Drivers run in kernel mode and must not block indefinitely. Long-running operations must be split into pollable units. Driver code is held to the same Rust standards as the rest of the kernel: `unsafe` blocks justified, no `unwrap()`, error propagation throughout.

Drivers must not allocate from the general kernel heap for tensor storage; they own their device-memory allocator and serve from it.

Drivers must not hold long-lived locks across dispatch boundaries. Per-driver lock granularity is a design concern; the recommended pattern is a per-queue spinlock plus lock-free state for completion polling.

### 9.4 Numerical equivalence and the tolerance-reference model

*(Added by BUILD-4b-2, ADR-032. The Phase-6 goal milestone in §12 calls for an accelerator's output "verified equivalent to the CPU baseline, within numerical tolerance for non-bit-exact dtypes"; this subsection fixes what "equivalent" and "tolerance" mean once a real GPU runs transcendentals. The dispatch referred to this as "§9.5"; §9 had only §9.1–9.3, so it lands here as §9.4.)*

Hardware f32 is **not** bit-identical to the soft-float (`accel_cpu`) backend, especially for transcendentals: the GPU's hardware extended-math `tanh`/`exp`/`rsqrt`, the soft-float polynomial approximations, and host libm are **three independent approximations** of the same true value. Therefore `accel_gpu == accel_cpu` byte-exact is the **wrong** equivalence gate for any op that is not exactly representable.

The equivalence rule is two-tier, and the **reference is host-f32 (libm), the closest-to-true value — never the soft-float backend** (which is itself only an approximation and would make the gate near-vacuous):

1. **Byte-exact (0 ULP)** for integer / data-movement ops (embedding gather, copy/migrate) and exactly-representable arithmetic — including, on a no-FMA accelerator like Gen9, plain `matmul` accumulation. These are held to the bare-metal exact standard.

2. **Bounded tolerance** for transcendental / accumulating ops (`gelu`, `softmax`, `layernorm`, `rmsnorm`):
   - **Op-level:** the accelerator's output is validated against a **host-f32 reference** within a **per-op ≤ 2⁻¹² relative** bound (`|g − h| ≤ 2⁻¹² · max(|h|, 1)`). The reference is host-f32, not `accel_cpu`. The bound must be exercised **non-vacuously** — on inputs where the op's hardware approximation genuinely diverges from libm (e.g. GELU on a spread of inputs, not values where the result rounds identically).
   - **Forward-pass-level:** for a `.neom` forward that mixes accelerator-resident and CPU-fallback ops, the gate is **not** "mixed-device run == pure-CPU run" (both diverge from truth independently). It is that **both** the pure-`accel_cpu` forward **and** the mixed forward land within an **accumulated relative bound B of the host-f32 reference**, where B is derived from the op-depth: `B ≈ d · 2⁻¹²` to first order (d = number of tolerance-bearing ops on the path), rounded up for the O(1) per-op amplification of normalization/softmax stages. B must be stated and justified per model, never picked as a magic number. The soft-float-vs-host regression keeps its established Phase-5 bound.

The CPU-fallback path remains **byte-exact transparent**: when an accelerator returns `Unsupported` for an op, the executor runs it on `accel_cpu`, and the pure-`accel_cpu` forward must reproduce the byte-exact baseline oracle (the fallback never changes results, only the device).

### 9.5 Per-principal accelerator accounting (the governance measurement floor)

*(Added by Phase 7 BUILD-1, ADR-033. The first governance subsection. **Measurement only — no policy.** You cannot arbitrate what you cannot measure; this fixes *what is measured* and *who it is attributed to* so later builds — admission control, quota, reservation, isolation — have a floor to stand on. Those builds make *decisions on the numbers*; §9.5 makes *no decision*.)*

The kernel maintains a per-`(Device::Accel(n), principal)` accounting record, updated from the common `AccelDriver`/residency paths so every backend (`accel_cpu`, `accel_sim`, `virtio_accel`, `accel_intel`) is covered uniformly — it is device-agnostic, hanging off the trait boundary, not any one backend.

**What is measured**, per `(device, principal)`:
- **`resident_bytes`** — bytes currently resident on the device for this principal. `+len` on `DeviceStorage` allocation / migrate-onto-device, `−len` on free/drop / migrate-off. **Deterministic — asserted exactly.** Captured at the `DeviceStorage` boundary (the residency unit every backend shares), with the owning principal recorded at alloc time so the matching free decrements the same principal regardless of the freeing context. Saturating at 0 (never underflows).
- **`dispatch_count`** — completed `AccelDriver` dispatches attributed to this principal on this device. **Deterministic — asserted exactly.** Recorded by the common submit→completion wrapper the inference executor routes through.
- **`dispatch_time`** — accumulated wall-clock (100 Hz timer ticks) from submit to completion. **Real but non-deterministic** — logged in the governance trace, asserted only as monotonic / non-negative, **never exact**.

**The principal** is the **`ProcessId`** that owns the dispatched work — for inference, the process that invoked the inference syscall (the executor runs synchronously in the caller's thread, so the running thread's pid *is* the principal). Kernel-internal dispatch with no userspace caller (regression scenarios, internal oracles) attributes to the **system principal** — the `KERNEL_PID` sentinel — and accounting **never panics on "no process."** An explicit scoped principal channel exists for threading a principal that is not the running thread's (and for tests). **The capability-principal mapping** (attributing to the capability/tenant that owns a *shared* tensor, not just the invoking process) is a labelled **BUILD-3 seam** — `ProcessId` is the accounting unit at this layer; no new authority primitive is introduced.

**Observability:** the record is dumpable as a governance trace — one `accel-acct: principal=… device=… resident_bytes=… dispatches=… time_ticks=…` line per `(device, principal)`. The table is bounded by the live-principal set (a principal's entries are dropped on teardown). Accounting itself must not leak: after a principal frees/migrates-off all its device residency, its `resident_bytes` returns to 0 (mirrors the migration `allocs == frees` discipline).

This subsection is the *scale that measures*, not the *hand that arbitrates* — admission, quota, reservation, deferral, sharing, and isolation are explicitly out of scope here and land in later Phase-7 builds.

### 9.6 Admission-control inference scheduling (the first cross-tenant decision)

*(Added by Phase 7 BUILD-2, ADR-034. The first **enforceable** governance decision. §9.5 made the kernel *measure*; §9.6 makes it *decide* — when two mutually-untrusting principals contend for one accelerator, the kernel admits one and **defers** the other, by policy the applications did not request. That deferral is structurally impossible above the kernel: a per-process userspace runtime sees only its own work and can never legitimately defer *another* principal.)*

**Resource shape — admission control, NOT preemption.** Accelerators run to completion; the kernel decides *who runs next* when the device is busy or a principal is over its share — it does **not** interrupt a running dispatch. So: a per-`Device::Accel(n)` **request queue**, **run-to-completion**, **temporal isolation** (one tenant's work runs at a time per device). No timeslicing, no preemption, no mid-dispatch migration, no spatial concurrency. This is the block-layer/batch-I/O shape, not the CPU-scheduler shape.

**The schedulable unit** is the **inference request**, tagged `{principal (ProcessId), priority, submit_time, reservation}`. Per-request principal attribution travels with the request through the dispatch path (the §9.5 in-flight-principal mechanism, per device) — not a single global — so concurrent requests from different principals on different devices are each accounted to *their* principal.

**Policy inputs** (minimal — enough to defer one principal for another, no more; no fair-share rotation, aging, work-stealing):
- **priority** — a request priority; higher admits first.
- **per-principal quota** — derived from the **§9.5 accounting table** (a per-principal dispatch budget per device); an over-quota principal yields to an under-quota one when the device frees. The scheduler reads §9.5 — it does **not** keep a second usage counter (measure and decide stay separate concerns sharing one source of truth).
- **reservation** — a principal may hold a device reservation that defers others while active.

**Decision semantics.** On an arrival while the device is busy or reserved **by another principal**, the request is **deferred** (a principal's own reservation/running never blocks its own request). When the device frees, the next request is chosen by `priority → under-quota → FIFO`. The decision is emitted as a governance trace whose load-bearing clause is *"deferred **because of** another principal's claim"* — e.g. `accel-sched: infer principal=PID12 device=… DEFERRED — PID7 reservation active`, paired with `… ADMITTED` on admission. No starvation: the deferred request runs once the device frees in policy order.

**Live-path enforcement — `sys_inf_run` routes through the scheduler (BUILD-2L, ADR-034).** The decision is not advisory: the `sys_inf_run` syscall submits every live inference to the per-device queue and is bound by the admit/defer verdict. The contract on the live path is **block-on-defer / wake-on-admit**:

- **Admit ⇒ run now.** When the accelerator is free the caller is admitted immediately and proceeds with no parking and no added latency — the single-tenant fast path is the common case and pays nothing for the governance machinery.
- **Defer ⇒ park (no busy-wait).** A deferred caller's thread is **blocked** (parked off the run queue via the kernel's existing wake-token blocking primitive), not spun. It consumes no CPU while it waits. When the holder completes, the scheduler admits the next request **and wakes its parked caller**; the woken caller then runs to completion and, on finishing, admits/wakes the next in turn. Run-to-completion is enforced structurally: a device stays attributed to the admitted principal from admission until that caller's completion.
- **No lost wakeups, no deadlock.** The defer decision and the park are performed under a single acquisition of the queue lock — the pending request carrying the caller's wake token is installed while the lock is held and released atomically with the context switch, so a concurrent completion can never wake a token that is not yet stored. A parked caller holds **no** lock the admitter needs (the queue lock is released on the park switch); the lock order is one-way — queue → accounting table (quota read) and queue → scheduler (wake) — never inverted.

Since the live path shares the §9.5 in-flight-principal attribution, the principal the scheduler arbitrates over and the principal the accounting charges are identically the same — scheduling and measurement cannot disagree. (BUILD-2L adds **no new ABI**: `sys_inf_run` is unchanged at the syscall boundary; the priority/reservation policy inputs above are not yet surfaced to userspace, so every live request shares one default priority and contention resolves by run-to-completion then FIFO. A usage/priority query ABI is a later build and triggers the byte-oracle rule when it lands.)

**Deferred — not in §9.6:**
- **Preemption / timeslicing** — never; run-to-completion is the shape.
- **Multi-device placement** (which accelerator, not just which-next-on-one) — a later build; §9.6 is single-device admission.
- **Spatial concurrency** (two tenants at once on one device with enforced boundaries) — that is the per-context isolation / Intel PPGTT lift (BUILD-4). §9.6 is **temporal**: one at a time, the scheduler picks order.

This subsection is the hand that arbitrates *order* — not the hand that interrupts, shares, or isolates.

### 9.7 Capability-mediated tensor sharing + content-hash dedup (global-view ownership)

*(Added by Phase 7 BUILD-3, ADR-035. The third governance capability. §9.5 made the kernel **measure**, §9.6 made it **arbitrate**; §9.7 makes it **own the global view** — it sees, and enforces, that multiple principals reference **one physical copy** of an immutable tensor. This is a fact only the kernel can know: a per-process userspace runtime sees only its own handles and would re-copy. It is the clearest "kernel does what a library structurally can't.")*

**Two paths to one physical copy.** Both produce the ownership trace `tensor <id> shared RO {PID..} — 1 physical copy`:

- **Explicit** — a principal *grants* a read-only tensor capability to another principal over the existing Phase-2 capability machinery (derive reduced rights → grant via capability IPC → revoke through the derivation graph). The grantee gets an RO handle to the *same physical storage*, not a copy.
- **Implicit (dedup)** — when a principal loads a model whose immutable weight bytes (BLAKE3 content hash) match an already-resident immutable tensor, the kernel **shares the existing physical copy** (refcount++) instead of allocating a second.

**The refcount is the existing `Arc<Tensor>`.** Cross-process references are cloned Arcs over one physical allocation — *not a new allocator*. N capability handles → 1 physical copy; the last handle dropped frees the storage. The kernel adds three layers on top: **visibility** (the reader set — derived by scanning capability tables, so revoke/drop need no bespoke hook), **dedup** (since BUILD-O2/ADR-044: a content-hash → `SlotKey` table — the arena's generation check (I-13) is the liveness probe, candidacy pins nothing, and a dedup hit shares the SLOT: N models, one slot, one resting `Arc`. **The share survives movement**: the one copy can be demoted and re-fetched by the policy layer without breaking the share, because every sharer resolves through the same slot — gated by `accel_dedup_movement`. Idle deduped weights are therefore claimable: the production-default `sys_model_load` path is fully under §9.9–§9.12 governance), and the **accounting rule** below.

**Read-only only (this build).** Shared/deduped tensors are immutable to sharers — model weights, the canonical case, are already frozen snapshots. **RW-shared (mutable coherent sharing) is out of scope** — a separate hard coherence problem, deferred.

**Rights enforced at the op boundary — not by convention.** A tensor capability carries a semantic `WRITE` right (`tensor_rights::WRITE`); a read-only share is a capability **without** it. A mutating op (`sys_tensor_write`) issued by an RO grantee is **denied** (`EPERM`) at the syscall/op boundary — enforced by the capability, independent of (and in addition to) the tensor object's own `mutable` flag. So a grantee with an RO cap to an otherwise-*mutable* tensor still cannot write it: the right, not the object, gates the mutation. Revocation through the derivation graph removes a grantee's access (and thus its place in the reader set); revoke and last-reference free interact cleanly — no use-after-free (the `Arc` keeps the storage alive for surviving references), no leak (the last drop frees and deregisters).

**Accounting must not double-count (the §9.5 amendment).** A shared/deduped tensor's bytes are **one physical copy**, so they are counted in §9.5 **exactly once — attributed to the owning/first principal** — at the point the tensor enters the sharing registry; every other referencing principal is a **reader**, contributing **zero** additional resident bytes. Whatever the reader count, total `resident_bytes` summed across principals equals the physical bytes resident — sharing never inflates residency. The owner's attribution is released once, when the last reference drops and the physical copy is freed. (Shared CPU-resident tensor residency is attributed on the CPU device; before BUILD-3, plain CPU tensors carried no §9.5 residency — sharing is the governance event that makes the copy *accounted*.)

**No new ABI.** Explicit sharing reuses the Phase-2 capability-grant IPC unchanged. The only new syscall is `sys_tensor_write` (the mutating op boundary the `WRITE` right guards); its arguments are scalar (handle, offset, user pointer, length) with no struct layout, so the byte-oracle rule does not apply.

**Deferred — not in §9.7:**
- **RW / coherent shared tensors** — a separate coherence problem; never in this build.
- **Concurrent *device-side* RO-vs-RW enforcement at the hardware boundary** — for device-resident shared tensors the *handle* is shared and access is kernel-mediated (accelerator memory is never user-mapped), but enforcing concurrent isolation at the device is the **per-context PPGTT lift (BUILD-4)**. §9.7 is CPU-resident first; this is the noted seam.
- **A usage/ownership query syscall** (a tenant reading the shared-reader set) — a later build; its struct ABI triggers the byte-oracle rule when it lands.

This subsection is the hand that recognises *one copy behind many references* — global-view ownership. The hardware-boundary isolation it leaves open — a shared RO tensor being hardware-unwritable by the grantee, not merely policy-unwritable — was **closed by the BUILD-4X arc (2026-06-08, c48d5d5):** the grantee's per-context PPGTT maps the shared tensor read-only, so a device-side write faults. See §9.8 (spatial isolation: ENFORCEMENT-PROVEN).

---

### 9.8 Enforcement fidelity (what is proven, what is pending, what is bounded)

*(Added 2026-06-07, after Phase 7 BUILD-1/2/2L/3. This subsection states — honestly and precisely — how strongly the kernel can **enforce** the governance model of §9.5–§9.7 against adversarial principals, on the current Intel Gen9 accelerator. Governance being **exercised** (§9.5–§9.7) is distinct from governance being **adversarially enforced**; this fixes the difference so the project neither over- nor under-claims.)*

Status:

```
Execution sovereignty:   proven (Phase 6 — kernel owns the compute path).
Governance sovereignty:  demonstrated (BUILD-1/2/2L/3) — the kernel measures,
                         arbitrates, and owns the cross-tenant view, using a
                         global view no per-process userspace runtime has.
Adversarial enforcement (the fidelity question):
  • Entry / accounting / reservations / tensor authority (syscall/API level):
        UNBYPASSABLE. There are no userspace BARs (Article 2.3); the only path to
        the accelerator is sys_inf_run, which routes through the scheduler. A
        tenant cannot jump the admission queue, and an RO grantee's
        sys_tensor_write is denied EPERM at the op boundary.
  • Spatial device-memory isolation:  **PROVEN on silicon (BUILD-4X arc,
        closed 2026-06-08, c48d5d5).** A second tenant's device-side (EU A64)
        access to the first tenant's tensor page-faults, and an RO-shared tensor
        faults on write — both enforced by per-context PPGTT page tables built by
        the kernel and loaded via GuC-free execlist/LRC submission. The path:
        BUILD-4-1 falsified legacy-ring PPGTT (the engine binds PPGTT to the LRC);
        a 5-lane deep-research pass specified the execlist mechanism; 4X-1 ran a
        context with its PPGTT active, 4X-2 proved tensor data resolves through it
        (`select=PPGTT`), 4X-3 proved the adversarial boundary (cross-tenant fault
        + RW-on-RO fault, with a positive control). So the kernel's tensor
        authority is now enforced *by the accelerator's own page tables*, not by
        an op-boundary check a tenant could race. *(Originally read "PENDING —
        requires the execlist/LRC mechanism"; closed by the 4X arc.)* See the
        detailed line below.
        GFX_MODE bit7), and the Gen7 PP_DIR_BASE aliasing path — were correctly
        programmed and *all three* read select=GGTT (fault @0x130000) — the
        render engine binds PPGTT to the Logical Ring Context, so without an
        execlist/LRC context holding the PPGTT descriptor the engine has no
        per-process address space. So spatial isolation here is achievable only
        via an execlist/LRC bring-up (the deferred larger lift), not the
        legacy-ring path. **UPDATE 2026-06-08 — execlist/PPGTT now ENGAGES on
        silicon (BUILD-4X-1, 7626f62):** a GuC-free context submitted via ELSP
        runs on the UHD 630 with the context's PPGTT as the active address space
        (the inverse of the legacy-ring select=GGTT dead end) — so the mechanism
        is proven. **4X-2 PASSED (e78cc79):** the Add retired `[4,6]` through the
        execlist context with its data buffers cleared from GGTT — so the EU's
        A64 tensor data resolved through the per-context PPGTT (`select=PPGTT`),
        the *positive* boundary proven. **4X-3 PASSED (c48d5d5) — the enforcement
        claim is closed:** with two tenants in two per-context PML4s, tenant B's
        device-side A64 access to tenant A's tensor **page-faulted** (select=PPGTT,
        A's VA, B's PML4 active), while B's access to its *own* tensor succeeded
        (the positive control proving the fault is isolation, not breakage), and a
        write to an RO-mapped tensor **faulted** while a read succeeded. So the
        spatial-isolation line is now **ENFORCEMENT-PROVEN** — "B cannot reach A"
        is silicon-demonstrated, the kernel's tensor authority enforced by the
        page tables. This also closes §9.7's device-resident-shared-tensor seam:
        a shared RO tensor (BUILD-3) is now hardware-unwritable by the grantee,
        not merely policy-unwritable. The BUILD-4-1 PPGTT page-table machinery
        (PML4→PT, the RW/RO PTE byte-oracle, the PDP-root capture) is reused by
        the execlist path — the LRC points the engine at it.
  • Temporal fairness (device monopolization):  ADMISSION-CONTROLLED, NOT
        PREEMPTIVE — bounded by hardware. Accelerators run to completion and
        Gen9 compute does not preempt, so the kernel governs *order* but cannot
        interrupt a tenant holding the device with a long-running kernel. This
        gap is fundamental to the hardware, not an implementation gap, and is
        NOT closed by PPGTT.
```

The accurate claim the project may make today:

> The kernel has **unbypassable** control over accelerator **entry, accounting, reservations, and tensor authority at the syscall/API level**, and governs AI resources across mutually-untrusting tenants using a global view a userspace runtime cannot have. **Device-memory isolation becomes adversarially strong only once PPGTT-backed per-context address spaces land (BUILD-4); runtime monopolization remains bounded by hardware preemption limits.**

The claim it may **not** yet make: *"the kernel fully isolates adversarial tenants on the accelerator."* That crosses from governing-by-API-convention to governing-by-silicon-enforced-boundary, and is gated on BUILD-4-2. Enforcement fidelity is therefore **asymmetric and per-device**: strong and achievable on spatial isolation, inherently weaker on temporal fairness — and the kernel must report, per accelerator, which fidelity it can actually deliver (a serialize-only device offers temporal isolation; a PPGTT/SR-IOV device offers spatial).

### 9.9 Residency-aware op placement (the kernel decides where an op runs)

*(Added by Phase 7 BUILD-P1, ADR-036. §9.5–§9.8 built the governance **mechanisms** — measure, arbitrate, own, isolate. §9.9 opens the **policy** layer: the kernel does not merely *execute* a placement a caller named, it **decides** placement from a global view, and **logs the reasoning**. Every placement before this was explicit — a caller/executor said "run this op on driver R." The leap is that the executor now consults an in-kernel policy that, from inputs only the kernel sees globally, chooses *where the op runs* and emits a decision trace. The first policy decision attacks **migration thrash**, which §3's correctness-for-performance thesis depends on: a mixed forward pass that bounces a tensor device→host→device on every op can be net-negative — the accelerator makes it slower — so residency-aware placement is correctness, not a nicety.)*

**The placeable unit** is the **op** (where it runs); tensor placement follows from op outputs (an op's result is made resident on the device it ran on, so the next op sees its operand in place). The policy is **pure and deterministic** (host-testable) and reads — never recomputes — the registry and §9.5 accounting as inputs.

**Inputs (only the kernel sees these globally):**
- **Operand residency** — the `DeviceId` of each operand tensor (§9.7 residency). Host-class residency (`Device::Cpu`) folds into the host-memory fallback (accel_cpu): same memory, no migration between them.
- **Device coverage** — which registered accelerators' `supported_ops` cover this op.
- **Per-principal quota** — read from the **§9.5 accounting table** (the same `dispatch_count ≥ quota` predicate §9.6 uses — measure and decide stay one source of truth, no second counter).

**The rule (minimal, first build):**
1. All operands resident on one covering, under-quota accelerator `D` → **place on `D`** (zero migration). Reason `operands-resident`.
2. Operands split across devices → place on the device holding the **most operand bytes** (migration minimized); deterministic lowest-`DeviceId` tiebreak. Reason `min-migration`.
3. No accelerator covers the op, or operands are host-resident with no accelerator benefit → **host fallback** (accel_cpu). Reason `cpu-fallback`.
4. The residency-preferred accelerator is **over the principal's quota** (§9.5/§9.6) → spill to the host fallback. Reason `over-quota`.

The host fallback is always selectable (the safety floor — the policy never returns "nowhere to run"), so the decision is total and a **strict improvement** over fixed dispatch: when no accelerator holds operands or covers the op it reproduces today's behavior exactly (host fallback, `Device::Cpu` outputs, zero migration).

**The result-invariance guarantee (load-bearing).** Placement changes *where* an op runs, **never its result.** A forward pass's output is **byte-identical / within-tolerance** regardless of the placement chosen — this is the §9.4 numerical-equivalence guarantee extended from "any backend computes the same bytes" to "routing-by-policy computes the same bytes as routing-by-default." A placement that changes output is a bug, not an optimization.

**The decision trace (the governance line).** One line per placement decision — the reasoning the application did not request:
```
placement: op=<kind> principal=PID<n> → device=Accel(<n>)|cpu reason=<operands-resident|min-migration|cpu-fallback|over-quota> migrated=<bytes>
```
The success metric is observable: a forward pass whose operands reside on an accelerator runs every op there with `migrated=0`, where a fixed-non-resident-device baseline incurs a migration per op — the policy *measurably* avoids the round-trips, output unchanged.

**Deferred — not in §9.9:**
- **Eviction** — when a device is at memory quota and a new tensor needs residency, deciding which resident tensor to evict (LRU from §9.5). The classic resource-management decision; **BUILD-P2** (the `eviction:` trace) — **now §9.10**.
- **Proactive / cost-aware migration** — *moving* tensors in anticipation based on a transfer-cost-vs-compute-benefit model. §9.9 chooses *where to run* to minimize the migration a dispatch implies; it does **not** yet move tensors proactively. **BUILD-P3** (the `residency:` retain/migrate trace).
- **Concurrency** — the placement decision is per op in a synchronous pass; cross-device parallel placement is later policy.

This subsection is where the trace **starts to read like resource management, not dispatch** — the kernel reasoning about placement is the moment the system stops looking like an inference engine and starts looking like an OS managing a new class of resource. The mechanisms (§9.5–§9.8) made it possible; this makes it visible.

### 9.10 Eviction under device-memory pressure (the kernel makes room)

*(Added by Phase 7 BUILD-P2, ADR-037. §9.9 decides where an op runs but assumes the chosen device has room. §9.10 handles the consequence: when placement wants residency on an accelerator that is **out of room**, the kernel decides **which resident tensor leaves** — the tensor-residency analogue of OS page eviction, and the first decision about a *scarce* resource. Before this, a full device arena simply failed the allocation; now the kernel makes room, falls back gracefully, and logs the reasoning.)*

**The one new input: last-use recency (§9.5 extension).** §9.5 measures resident bytes and dispatch counts; LRU needs *when a resident tensor was last used*. A monotonic table tick is recorded per tensor — on device allocation, on migrate-onto-device, and whenever an op touches the tensor (operand read or output write) — keyed `tensor_id → (device, tick)`: a tensor is resident on exactly one device, and the entry dies on migrate-off or drop, so the table is bounded by the live device-resident set. Recency is **measurement**, not decision: the table lives with §9.5 accounting; the policy only reads it.

**The policy (pure, deterministic, host-tested).** On a placement that needs `N` bytes on device `D` with capacity `C` and §9.5-resident total `R`:
1. `R + N ≤ C` → no eviction (the §9.9 path, bit for bit). Devices that declare no capacity never evict.
2. Else select victims **LRU-first among `D`'s evictable resident tensors** (ascending last-use tick, deterministic lowest-`tensor_id` tiebreak), accumulating until the freed bytes make room. Reason `lru`.
3. **Pin protection (required).** The operands and output of the op being placed are never evictable — you cannot evict what you are about to use; evicting a needed tensor is corruption, not a tradeoff.
4. **No partial eviction.** Sufficiency is precomputed: the policy returns either a victim set that *does* make room, or the CPU-fallback signal with **zero victims**. Migrating tensors off-device without ending up with room is thrash with no benefit ("do not loop").
5. If nothing evictable can make room → **place the op on the host fallback** (accel_cpu). Reason `no-room-cpu-fallback`. The pass never fails on pressure.

**Evictability is read, not enforced.** A tensor is evictable iff the executing pass holds the *only* reference to it (a uniquely-held intermediate). The op's own operands are mechanically protected (the executing node holds live references), and externally-held tensors — model weights, caller inputs, §9.7-shared/deduped tensors — can never be victims: another holder keeps the storage alive, so migrating them could not free device memory anyway. Pin protection therefore needs **zero new mechanism**; the explicit pin set is belt-and-suspenders on top.

**What `N` includes.** For arena-backed devices (accel_sim), a dispatch transiently stages its inputs and output in the *same* arena that holds resident tensors, so the room check covers the resident output **plus** the transient staging (Σ operand bytes + output bytes). A check that ignored staging would pass and then exhaust the arena mid-dispatch. As a backstop for what the model cannot see (SMP-concurrent allocators, alignment padding, op scratch beyond the staging model), a placed op whose device allocation or dispatch still reports out-of-memory **reroutes to the host fallback** instead of failing — totality holds even when the arithmetic is beaten by a race.

**Eviction is migration, and migration is the existing mechanism.** A victim is migrated to CPU through the existing tensor-migration path (storage replaced, device storage freed, §9.5 residency moved) — no parallel residency mechanism, no new bookkeeping. **Result-invariance (load-bearing):** eviction changes *where a tensor lives*, never its bytes — an evicted tensor re-staged later reads back byte-identical, and **the forward pass's output is byte-identical regardless of eviction**. A wrong result or an evicted-needed tensor is a bug.

**The decision trace (the governance line).** One line per victim, and one for the no-room fallback:
```
eviction: tensor=<id> device=Accel(<n>) → cpu reason=lru (resident=<R>/<C>, need=<N>)
eviction: device=Accel(<n>) reason=no-room-cpu-fallback (resident=<R>/<C>, need=<N>, evictable=<E>)
```
The `R ≤ C` invariant is observable post-hoc: §9.5 tracks each device's residency **high-water mark**, so a pressured pass can prove residency never exceeded capacity throughout, not merely that it ended below it.

**Deferred — not in §9.10:**
- **Proactive / cost-aware migration** — eviction *reacts* to pressure; anticipating need (retain because it will be used; migrate ahead of need when the cost pays off) is **BUILD-P3** (the `residency:` retain/migrate trace) — **now §9.11**.
- **Eviction to another accelerator** — victims migrate to CPU only; device-to-device spill is later policy.
- **Working-set / frequency heuristics** — LRU only; smarter victim selection is later policy.

With §9.9 + §9.10 the kernel both *places* work and *makes room* for it — the two decisions that, together, constitute managing a scarce device resource under pressure.

### 9.11 Proactive residency — cost-aware retention and headroom banking (the kernel anticipates)

*(Added by Phase 7 BUILD-P3 (Path A), ADR-038, on the ratified design in `Plans/THINK-2026-06-10-phase-7-build-P3-cost-aware-migration.md`. §9.9/§9.10 are reactive — they decide when a request has already arrived or pressure has already hit. §9.11 is the first decision the kernel makes about an access that has **not happened yet**: refusing a net-negative eviction because reuse is imminent, and demoting a dead tensor early to bank headroom for the next placement. The kernel stops merely reacting to pressure and starts anticipating it — the last reactive→proactive step the policy layer can take without concurrency.)*

**The reuse signal is structural, not statistical.** From the op sequence the pass is already executing, a value is **predicted-hot** iff an op within the lookahead window `W` consumes it (`W = 1` — "needed by the next op" — is the always-on default), and a value is **dead to the pass** iff no remaining op consumes it and it is not a declared model output. No learned model, no access-counter sampling: the dependency graph the pass holds is the entire input, which is what makes the prediction trustworthy enough to act on. Cost is measured in **bytes only** — the only unit the kernel can assert without a timing oracle.

**The two decisions (synchronous, inside the pass — Path A):**
1. **Retain (`residency: … RETAINED reason=reuse-soon`).** At the §9.10 eviction consult, a predicted-hot tensor loses eviction priority even when it is the LRU coldest — evicting it merely forces a re-migration one op later (net-negative). A colder *non-hot* victim of sufficient size is chosen instead. If retention leaves no way to make room, the decision **falls through to §9.10 exactly** (LRU among all evictable, predicted-hot included, then the no-room CPU fallback) — retention is an optimization, never a correctness gate.
2. **Cold-demote (`residency: … → cpu reason=cold-demote`).** At each op boundary (never after the final op — headroom banks for a *next* placement, and none exists), if residency has crossed the headroom threshold `R > C − H`, the coldest **dead-to-the-pass** tensor is migrated to CPU, banking room so the next placement avoids an eviction round-trip entirely. At most one demotion per seam.

**The headroom target `H` is one placement's worth:** the largest placement-need (`output + transient staging`, §9.10) any op in this pass has presented so far. Banking less can never prevent the next eviction round-trip; banking more holds device memory idle for no predicted benefit. The backward max is monotone, so the threshold never chatters the way a per-op forward estimate would.

**The thrash guard is the eligibility rule, and the window asymmetry is deliberate.** Retention uses the narrow `W`-window because its failure mode is benign: retain too few and the decision falls through to plain LRU — still correct. Demotion uses the conservative full-remaining-scan (dead-to-the-pass, output-protected) because its failure mode is exactly the thrash this section exists to eliminate: a `W`-window demote eligibility provably moves a tensor the pass still needs (a tensor reused three ops out is invisible at `W = 1`). A proactive move whose own predicted consequence is being undone is refused, not weighed. Optimistic where wrong is cheap; pessimistic where wrong is fatal.

**Strict superset, all the way down.** With `W = 0` (or on the legacy fixed-driver path) every proactive decision is disabled and the behavior is **§9.10 bit for bit** — asserted by the host suite (the no-prediction decision equals the §9.10 selector across its case matrix) and pinned by the reactive gate scenario, which runs the `W = 0` entry point. Result-invariance is unchanged: retention and demotion change *where* a tensor lives, never its bytes; the forward-pass output is byte-identical regardless of every proactive decision.

**Movement stays synchronous and single-mutator — concurrency is P4.** Every retain/demote executes inside the executor pass, which remains the only mutator of its value table; the device remains run-to-completion, one request at a time. §9.11 added the proactive *decision* without relaxing the single-mutator invariant (re-documented at the eviction seam). The async residency daemon — prefetch/promotion concurrent with dispatch, on the device-level migration-reservation primitive — is **BUILD-P4**, the build that relaxes and re-proves that invariant.

**The measured claim (the gates):** on a reuse workload, §9.10 evicts the very tensor the next op needs while §9.11 retains it and evicts a colder one (`accel_residency_retain`); on the §9.10 pressure workload itself, headroom banking takes **zero** eviction round-trips where the reactive path took two — strictly lower, same bytes out (`accel_residency_demote`).

**Deferred — not in §9.11:**
- **Prefetch / promotion** (migrate toward a device ahead of need) — requires coordination with eviction under concurrency; **BUILD-P4** — **landed in §9.12 (the daemon's publish-only prefetch)**.
- **Cross-tenant anticipation** — the pass can only see its own op sequence; the §9.12 daemon acts only on tenant-published predictions (no cross-tenant speculation, by design).
- **Sub-tensor block migration, timing/bandwidth cost models, learned predictors** — later policy, if ever; the kernel asserts only what it can measure.

### 9.12 The migration reservation (concurrent residency — the mechanism)

*(Added by Phase 7 BUILD-P4a, ADR-039, on the ratified design in `Plans/THINK-2026-06-10-phase-7-build-P4-concurrent-residency.md`. §9.9–§9.11 are policy over a single-mutator executor pass. §9.12 is a **mechanism** build: the primitive that makes it safe for a future mover (the P4b daemon) to migrate tensors concurrently with dispatch. No daemon ships here — the deliverable is the primitive plus its race proof: an N≥50 NEO_SMP stress gate in which every plausible race mode is self-diagnosing.)*

**The shape (the `PG_isolated` analogue).** A tensor eligible for background movement lives in a `TensorSlot`. A mover's **claim** — under the slot's lock — applies the exact §9.10 evictability gate (`strong == 1 && weak == 0`, not already reserved; a contended claim is **skipped, never blocked** — the `MIGRATE_SYNC_LIGHT` lesson) and then **takes the tensor out of the slot**, leaving a `Reserved` marker keyed by tensor id. The copy runs with **no locks held**: the mover owns the only reference (protection by possession — `Arc::get_mut` cannot fail), so the container is never monopolized. An accessor that hits the marker **parks** — its wake token installed while the reservation lock is held and released atomically with the context switch (no lost-wakeup window) — and **re-resolves from scratch** on wake (the fault-replay analogue). The **commit** restores the migrated tensor, removes the reservation, and wakes the waiters; rollback is the identical motion with the unmigrated tensor. From any observer's view the storage swap and the §9.5 updates are atomic, because every observer is parked-or-parking until the commit.

**The soundness lemma (refcount-preserving — zero new `unsafe`).** References are minted only from an owning container or via `Weak::upgrade`. The gate's `weak == 0` makes any `Weak`-bearing tensor unclaimable (no upgrade can race a claim); taking the `Arc` out of the slot means the container cannot mint references for the duration; and an accessor that skips the protocol entirely can only make the gate see `strong > 1` and the claim skip — it can never produce a use-after-migrate. Protection comes from the reference counts, as it has since §9.10; the reservation adds **arbitration** (two movers cannot claim the same tensor — the second sees `Reserved` and skips) and **parking** (accessors wait for the commit instead of spinning).

**The lock order (discriminator #3).** One documented global order, asserted by a debug-build lockdep in the `SwitchMutex` wrapper that panics *at the offending acquire*: `ADMISSION_QUEUES(10) → TENSOR_SLOT(15) → RESERVATION(20) → ACCT_INFLIGHT(22) → ACCT_TABLE(25) → ACCT_RECENCY(28) → scheduler` (levels occupy the lockdep's 1..=31 bitmask domain). Tracking is per-thread (a preempted holder cannot false-positive another thread); unordered locks are exempt; the table degrades to skip when full — a best-effort diagnostic, never a correctness gate.

**The single-mutator invariant, relaxed and re-proven.** The §9.10/§9.11 coherence note now reads: a concurrent mover may migrate a tensor via the reservation — safely, because a referenced tensor is never claimable and an arriving accessor parks-and-re-resolves. The synchronous P1–P3 paths are untouched: with no concurrent mover the reservation table is empty and behavior is §9.11 bit for bit.

**The discriminators (instrumented in-build, capture-before-you-need-it):** (#1) a per-tensor **storage generation counter**, bumped on every swap — an accessor asserts its read saw one generation, and the gate requires the park/retry path to have actually fired; (#2) a **parked gauge** asserted zero at quiesce, with the mover's bounded retry budget as the watchdog; (#3) the lockdep above; (#4) per-reservation **owner uniqueness** with double-claims counted and structurally refused; (#5) the **per-migration §9.5 delta** asserted at the commit boundary (source −bytes / target +bytes) plus balance-to-zero at teardown. (#7) every concurrent read is **byte-identical** to the pattern, N iterations under NEO_SMP — one divergent read fails the gate.

**The daemon (Phase 7 BUILD-P4b, ADR-040).** The primitive's first consumer: a dedicated kernel thread (the kswapd/TPP model) that, concurrently with live dispatch, **bg-demotes** cold managed tensors and **prefetches** published-hot ones — so the executor's critical path stops paying for reclaim.

- **The surface is the managed-slot registry:** tensors a tenant parks in registered `TensorSlot`s. The executor's value table stays private and synchronous-only (the §9.12 lemma keeps it that way); the daemon claims-or-skips through the reservation and can never touch what a pass holds.
- **Watermarks, per device:** `low = H_dev` (the device's noted max placement-need, fed by every placement consult — the §9.11 headroom figure, now per-device) and `high = min(2·H_dev, C)`. The daemon wakes below low (kicked by the placement path), reclaims coldest-first until high, and sleeps — hysteresis prevents boundary oscillation, and on tiny arenas where `2H ≥ C` the band degenerates to drain-all-cold (documented, still oscillation-free because prefetch fires only on an explicit publish). **Monotonic-latch caveat (ADR-040):** `H_dev` only grows; one outsized placement permanently raises the low watermark — bounded by C, policy-quality-only; a decaying estimate is the named follow-on.
- **Prefetch, behind the thrash guard as a hard gate:** only explicitly published-hot, CPU-resident managed tensors; only when free bytes stay ≥ low *after* the transfer (a prefetch must never cause the reclaim it exists to avoid). A prefetched tensor whose prediction is withdrawn before use is a counted **misprediction**, rolled back byte-preservingly by the daemon; consumption (`mark_used`) makes its later demotion ordinary cold reclaim.
- **Strictly additive:** the synchronous §9.10/§9.11 placement path is the always-complete direct-reclaim backstop. Daemon dormant (the default) is bit-for-bit §9.11; awake, it can make placements cheaper, never make one fail. Daemon-vs-backstop victim races are safe by construction — both go through the claim; the loser skips.
- **Teardown is drain, never kill:** stop completes the cycle (every reservation ends Resident, every parked waiter woken by the commit/rollback that ends its reservation) and only then exits — asserted by the quiesce hooks.
- **The traces:** `residency: device=Accel(n) → cpu reason=bg-demote (daemon, resident=<R>/<C> low=<L>)`, `residency: cpu → device=Accel(n) reason=prefetch (daemon, headroom ok, bytes=<B>)`, `residency: device=Accel(n) → cpu reason=mispredict-rollback (daemon, prediction withdrawn)`.
- **The measured claim:** on the multi-tenant pressure workload, the same pass that pays ≥1 critical-path reclaim (ensure_room evictions + no-room fallbacks; the §9.11 boundary seams are reported separately as proactive housekeeping) with the daemon paused pays **zero** with it awake — strictly lower, byte-identical, every iteration of the NEO_SMP stress loop.

**Ownership: the slot arena (BUILD-O1a, ADR-042).** The daemon's append-only registry generalized into a generational slot arena: the kernel holds the unique resting `Arc` per slot; holders keep a `SlotKey {index, generation}`; *use* mints a short-lived reference through the §9.12 reservation gate (`mint` = generation check, then `guarded_clone` — the fd-table `fget`/`inc_not_zero` discipline); `free` drains via the take protocol and bumps the generation, so a stale key always fails clean (invariant I-13 — never the new occupant). With the slot as sole owner at rest, the I-4 claim predicate (`strong==1 && weak==0`) now means exactly "no use in flight" for arena-owned tensors. Race-proven (mint vs claim vs free, N=60 at NEO_SMP=4) with the generation-mismatch, slot-balance, and byte-tag discriminators armed. The full ownership-model section (handles/models on `SlotKey`) lands with BUILD-O1b.

**Deferred — not in §9.12:**
- **Wiring production tensor surfaces** (syscall-held tensors, model caches) into managed slots — the arena mechanism landed in BUILD-O1a (ADR-042); the `Handle::Tensor`/`Model::weights` conversion is BUILD-O1b.
- **Device-side concurrent DMA** (the Intel copy engine) — a later hardware build; kernel-side coherence on accel_sim is the scope here (Q3).
- **Wiring every kernel reference site** through `guarded_clone` — the audited entry points exist; the daemon's surfaces use them. Un-instrumented sites degrade gracefully by the lemma.

### 9.13 The ownership model — handles name, the kernel owns (BUILD-O1a/O1b)

The converged shape of three traditions (DRM GEM/TTM two-counter ownership,
the fd table's mint-on-use, slotmap generational indices — ADR-042 §prior
art): **a capability handle names a tensor; the slot arena owns it.**

- **The arena (§9.12 ownership note, BUILD-O1a):** `SlotKey {index,
  generation}`; the slot holds the only resting `Arc`; `mint` =
  generation-check + `guarded_clone` (parks across migration, fails clean on
  a drained or stale slot — invariant I-13); `free` = drain-then-bump;
  `retain`/`release` = the GEM `handle_count` analogue, free-on-last.
- **The surfaces (BUILD-O1b):** `Handle::Tensor` = `{slot, meta, cap_id,
  rights}` — `meta` caches immutable identity so metadata paths never mint;
  table membership carries the handle count (`HandleTable` install retains,
  remove/revoke/drain release — the chokepoints are exhaustive because the
  table's entries are private). `Model::weights` = `Vec<SlotKey>`, alloc'd
  at load, released by the model's drop. Transient `Handle` copies carry no
  ownership; a stale copy fails the generation check (the revoke race is
  now strictly safer: the revoke wins).
- **Use mints, and minting IS the pin (I-9 unification):** `sys_inf_run`
  operands and the executor's weight loads mint for exactly the pass; query
  /write/share-registration paths mint and drop inside the call. While
  minted, `strong ≥ 2` ⇒ unclaimable — production tensors are pinned by the
  same mechanism as pass intermediates. The `sys_tensor_query` logical
  refcount is the arena handle count (= live table entries — the pre-O1b
  figure at every quiescent point).
- **The payoff (gate `accel_model_residency`):** a loaded model's idle
  weights are prefetched when published-hot, pass-pinned while inferring,
  demoted by the daemon when idle under another tenant's pressure, and
  byte-identical on every re-run — placed, evicted, retained, prefetched,
  moved in the background, capability-secured.

**Deferred — not in §9.13:**
- **Dedup candidacy vs claimability — CLOSED by BUILD-O2 (ADR-044):** the
  §9.7 registry now keys slots, candidacy pins nothing, and the
  `accel_model_residency` payoff gate runs through the default
  dedup-enabled load. See §9.7.
- **The serving surface** — unblocked by O1, its own line.

---

## 10. Security

### 10.1 The threat model

We assume:

- Userspace processes are mutually untrusted. A process cannot inspect or affect another process's tensors or models without an explicit shared capability.
- Models are untrusted data. A loaded model cannot escape its sandbox; it cannot read process memory outside its declared inputs or write outside its declared outputs.
- Accelerator firmware is partially trusted. We cannot prevent a compromised GPU firmware from misbehaving, but we minimize what it can affect (no DMA outside its allocated memory; IOMMU-enforced where available).

### 10.2 Capability discipline

Every AI-subsystem object — tensor, model, inference ticket, stream — is referenced by a capability handle. The kernel maintains a per-process capability table. Handles are not pointers; they are indices into a kernel-private table.

Capabilities are passed between processes through the standard cap-passing mechanism (Phase 2 deliverable). They are not forgeable; they cannot be guessed (the indices are local to each process and have no global meaning).

### 10.3 Model integrity

Models loaded from disk carry a content hash in their header. The kernel verifies the hash on load. If the file changes between load and re-load, the hash mismatches and the load fails (or, with policy hint `ALLOW_HASH_MISMATCH`, succeeds with a warning logged).

### 10.4 Side channels

Accelerator timing side channels are real. A process running inference on a shared GPU can in principle observe timing variations from another process's concurrent inference. We do not solve this in Phase 6. In Phase 7+, the inference scheduler offers a `EXCLUSIVE_DEVICE` flag for security-sensitive inference; when set, the device is not shared with other tenants for the duration.

### 10.5 Model-as-attack-vector

A maliciously crafted `.neom` could attempt to:

- **Trigger graph parser bugs.** Mitigation: extensive parser fuzzing in CI from Phase 5 onward.
- **Allocate excessive memory.** Mitigation: per-process model memory quotas.
- **Run forever.** Mitigation: per-inference deadlines enforced by the scheduler.
- **Exfiltrate data.** A model can only output to its declared outputs. Inputs are explicit. There is no "inference call also reads /etc/passwd" path because the kernel does not give the model access to anything beyond its bound tensors.

### 10.6 DMA isolation (Phase 7.5, I-14(c) — ADR-045, ADR-050)

A connectivity driver's DMA must never reach governance or tensor memory (invariant I-14(c), `docs/complexity-governance.md`). Two threats, two defenses:

- **(c1) — our errant descriptor programming (GUARDED, 7.5-0):** connectivity DMA buffers allocate only from a pre-registered **fenced arena** (`DmaBuffer::alloc_in_arena`); the descriptor-programming boundary accepts only a `FencedDmaAddr` — a witness type whose single constructor is the lock-free `dma_arena::fence(phys, len)` bounds check (pure-tier, total: zero-length/unset/overflow are explicit refusals; uninitialized fails CLOSED). A driver that skips validation cannot type-check a device programming call — the same construction-proof as `mint` (I-4) and `SlotKey` (I-13). Existing DMA users (GPU/NVMe/AHCI/virtio) stay outside the arena, untouched. Gate: `dma_arena_fence` (in-arena reaches the device; every out-of-arena address, including live kernel memory, refused; 0 escapes).
- **(c2) — a misbehaving device (PROVEN, 7.5-V — QEMU + real silicon; ADR-050):** a device that DMAs on its own initiative is contained by the IOMMU (Intel VT-d, DMA-remapping only — no interrupt remapping). Each connectivity device is placed in a translating **fence domain** whose second-level page tables map ONLY the §10.6 arena's physical range; the proven substrate (GPU/NVMe/AHCI/virtio) goes into **identity domains** (GPA==HPA, 1 GiB super-pages — the firmware RMRRs honored inherently) so it is byte-identical to VT-d-off (no regression). A fenced device's out-of-arena DMA physically cannot complete — the IOMMU faults, and the DMAR fault-recording registers capture the source-id + faulting address on a path that takes **no AI-governance lock and is bounded per drain** (I-14(a)/(b) preserved on the fault path; the VT-d lock is a leaf `SwitchMutex`). Kill-switch: the `vtd` feature OFF ⇒ none of it compiles ⇒ bit-for-bit pre-VT-d; `disable()` clears Translation-Enable at runtime. Gate: `vtd_fence` on QEMU `intel-iommu` — DMAR parsed (V1), TES set (V3), the substrate green under VT-d-on (no regression), a fenced NVMe's out-of-arena submission-queue DMA raises a DMAR fault with the matching source-id at an out-of-arena address (V5, the (c2) proof), governance memory intact, kill-switch clears TES (**Stage A PASS, 2026-06-13**). Stage B (OptiPlex 7080 / Comet Lake, blind, 2026-06-13) PASSED on real silicon: the fenced NVMe (sid 0x0100) DMA'd just below its arena and Comet Lake's IOMMU faulted it (governance intact, kill-switch cleared TES) — I-14(c2) fully proven, the Phase-7.5 pre-mandate discharged.

### 10.7 USB host (Phase 7.5 BUILD-A — ADR-046)

The kernel's USB host is an xHCI driver + minimal core under the full I-14 posture: every DMA structure from the §10.6 fenced arena, every programmed address a `FencedDmaAddr` (the single-gate property as construction — `PhysAddr` absent from the programming API), the ISR burst-capped and isolated from every AI-governance lock (the USB lock band orders above all AI levels; inversion panics in the lockdep), and governance proven byte-identical under an interrupt storm. BUILD-A's gate is enumeration (VID/PID/class + descriptor parse, QEMU `usb-kbd`+`usb-storage`, then the OptiPlex root ports blind per the F1–F7 discriminator bank); the class-driver seam is consumed by 7.5-B (CDC-ECM → `NetDevice`).

### 10.8 USB Ethernet (Phase 7.5 BUILD-B — ADR-047)

The first class driver on the §10.7 seam: CDC-ECM, implementing the EXISTING `NetDevice` trait — the whole Phase-4 stack (DHCP/ARP/DNS/TCP/TLS) runs over USB with zero new networking code. The claim contract: `probe` is a pure descriptor match (an ECM function found BY CONTENT across all configurations — dual-config RNDIS+ECM devices bind on the ECM one with no RNDIS knowledge anywhere); `bind` keeps the slot alive for the driver's lifetime, while a declined device takes BUILD-A's teardown bit-for-bit (the kill-switch: `cdc_ecm` disabled ⇒ every offer declines ⇒ the system IS BUILD-A). The bind sequence is the ECM 1.2 protocol with its classic trap made explicit: Configure Endpoint (interrupt-IN + the alt-setting-1 bulk pair, both CSZ layouts) → SET_CONFIGURATION → **SET_INTERFACE(data, alt 1)** (alt 0 has no endpoints) → SET_ETHERNET_PACKET_FILTER (STALL tolerated, breadcrumbed) → post fenced streaming buffers. Containment extends I-14 to the streaming path: per-endpoint completion queues are routed by (slot, DCI) in the bounded ISR drain and hard-capped (`EP_QUEUE_CAP`, drops counted — an RX flood degrades USB, never governance), the device lock orders above all AI levels (`USB_DEVICE`), and every bulk/interrupt buffer address is a `FencedDmaAddr` (0 refusals asserted at every gate). Link state rides the interrupt endpoint's NetworkConnection notifications with a breadcrumbed assume-up fallback (silent endpoints are common). The gate is the wire: a REAL DHCP DORA + DNS round-trip over the bulk pipes — slirp answers on QEMU, the PL's LAN answers on the OptiPlex (blind, per the E1–E7 bank). *Silicon close (BUILD-B Stage B, 2026-06-13, ADR-047): the wire gate passed on an AX88179 over CDC-ECM (real LAN lease + DNS round-trip), after correcting two of our own bugs the blind loop first mis-attributed to the chip — a wrong native-ASIX TX header corrupting ECM raw-frame TX (the chip transmits fine; the spec-pure raw-frame confirm reversed the false "dead hardware" verdict before a needless native-driver build), and a latent Phase-4 DHCP defect (all-zero BOOTP `chaddr` + clear broadcast flag, masked by QEMU slirp until bare metal; fixed in `transmit_client_message`). The AX88179's native MAC TX path (PHY power-cycle + `link_reset` TX-FIFO handshake, in vendor config) is enabled before the ECM switch.*

### 10.9 WiFi security / supplicant crypto (Phase 7.5 BUILD-W0 + W0b — ADR-051)

The WiFi link needs a WPA2/WPA3 supplicant, and the supplicant's hardest, most error-prone part is **software** crypto — fully testable before any radio exists. BUILD-W0 (+ W0b) is that part: a self-contained, kernel-free WPA2/WPA3 **key-management + EAPOL + CCMP + SAE library** (`net/wpa/{kdf,eapol,ccmp,sae}.rs`), the crypto core the W2 on-air supplicant will drive. It computes keys and encrypted frames; it touches no hardware, no MLME state machine, and no kernel glue (W1/W2). **The one hard rule (Article 15.7 / the kTLS-`chacha20poly1305` precedent): never hand-roll AES/SHA/HMAC/ECC** — every primitive is a vetted RustCrypto crate (`aes`, `ccm`, `cmac`, `sha1`, `sha2`, `hmac`, `pbkdf2`, `p256`, all `no_std`); this module writes only the WiFi-specific protocol layers on top. **Proof discipline (I-12, the byte-oracle applied to crypto): every layer is asserted against *published* IEEE 802.11 / RFC known-answer vectors in the host suite (`tools/ai-ops-tests`), NEVER round-trip** — a self-consistent encrypt/decrypt that is both wrong still passes, so only published answers prove correctness. The WPA2 key hierarchy and data plane (**PROVEN, 2026-06-13 — 11/11 host KATs green**): the **PMK** = `PBKDF2-HMAC-SHA1(passphrase, SSID, 4096, 256)` (ISC-1, two published PMK vectors); the **802.11 PRF** over HMAC-SHA1 and the **PTK/GTK** expansion (ISC-2) with the classic **Min/Max** ordering of the two addresses and two nonces made explicit and pinned both transitively (the frame-2 MIC) and directly (the published CCMP TK); the **EAPOL-Key MIC** (ISC-3) computed over the frame with the MIC field zeroed (the zero-before-compute trap), algorithm selected by key-descriptor version (v2 = HMAC-SHA1-128, v3 = AES-CMAC) — both KAT'd; **AES-CCMP** (ISC-4) = AES-CCM (M=8, L=2) via the `ccm` crate driven exactly as kTLS drives its AEAD, where the crypto core is pinned by the RFC 3610 packet vectors and the 802.11-specific construction (the `priority‖A2‖PN`-big-endian nonce and the masked-FC/SeqCtrl/QoS AAD) is pinned by a hand-laid independent byte oracle (the repo's "format/ABI byte oracle" rule); the **PMKID** = `Truncate-128(HMAC-SHA1(PMK, "PMK Name"‖AA‖SPA))` (ISC-5), pinned end-to-end against the canonical published example, with the 4-way-handshake key-confirmation being `eapol::verify_mic` over the derived KCK. **WPA3 SAE** (the dragonfly handshake over P-256 group 19, **LANDED in W0b — `net/wpa/sae.rs`, H2E only**) computes the password element and the commit/confirm key schedule on the `p256` crate (its RFC 9380 SSWU `MapToCurve`/`FromOkm`) + `hkdf`/`hmac`/`sha2` — no hand-rolled field math, SSWU, or HKDF; the legacy hunting-and-pecking PWE (with its timing side-channel) is deliberately not implemented. The pipeline: **PT** (MAC-independent) = `SSWU(u1) + SSWU(u2)` where `u_i = HKDF-Expand(HKDF-Extract(SSID, password‖id), "SAE Hash to Element u{i} P{i}", 48) mod p`; **PWE** = `val · PT` with `val = (HKDF-Extract(0, MAX‖MIN macs) mod (q−1)) + 1` (the MAC-sort trap); **commit** = `scalar=(rand+mask) mod q`, `element=−(mask·PWE)` (the inverse trap), serialized `group‖scalar‖x‖y`; **keys** = `keyseed=HMAC-SHA256(0,k)`, `KCK‖PMK=KDF(keyseed,"SAE KCK and PMK",(scalar+peer-scalar) mod q,64)`, `PMKID` = that context's first 128 bits; **confirm** = `HMAC-SHA256(KCK, …)`. Proven against the PM-verified cross-source vectors (hostap `common_module_tests.c` §J.10 + iwd `unit/test-sae.c`): the full PT pipeline and the PWE point are KAT'd directly (the PWE byte-identical across both sources, on-curve). A documented subtlety: hostap's §J.10 *handshake* (commit/KCK/PMK/PMKID) derives its PWE by hunting-and-pecking, so — since the commit-element/key-schedule/confirm math is PWE-agnostic and we implement H2E only — those stages are KAT'd by recovering the handshake PWE algebraically from the published commit (`PWE = mask⁻¹·(−element)`, pure published-byte algebra, no hunting-and-pecking) against the published KCK/PMK/PMKID oracle; the H2E PWE itself is the directly-KAT'd one. **Kill-switch:** the whole module compiles only under the `wifi` feature (default-off ⇒ the crypto deps are not pulled and the kernel is bit-for-bit pre-WiFi). **I-14:** no new containment vector — this library moves no memory and runs in the host tier; the firmware adversary (W1's MT7601U blob) is contained by USB itself (device-initiated DMA is impossible; the xHCI controller mediates all memory movement and already sits in VT-d's §10.6 fence domain, 7.5-V).

### 10.10 WiFi driver — MT7601U USB SoftMAC (scan) (Phase 7.5 BUILD-W1 — ADR-054)

The WiFi arc meeting silicon: the second class driver on the §10.7 seam, bringing the MT7601U USB dongle (`148f:7601`) **up to and including scan** — enumerate on the 7.5-A/B core, load firmware, read eFUSE, bring up MAC/PHY, and **list real 2.4 GHz BSSIDs**. Scope is the driver up to scan only; the MLME auth/assoc/4-way state machine, the `NetDevice` glue, CCMP on-air, and AP/mesh/P2P are **W2** (which joins §10.9's proven crypto to W1's frames). **The byte-oracle spine (I-12):** the byte-layout-defining logic is split into pure, kernel-free cores (`drivers/usb/mt7601u/{fw_logic,mcu_logic,eeprom_logic,rx_logic}.rs`) path-`mod`'d into `tools/ai-ops-tests` and asserted against **hand-laid byte fixtures computed independently of the parser** (the byte-oracle rule — a writer+reader pair from one spec agree even when both are wrong); four oracles are green — the `mt76_fw_header` parse + ILM/DLM/IVB partition + the FCE TXINFO/chunk framing (ISC-2), the MCU DMA command-wrap + RANDOM_WRITE/BURST_WRITE payloads + response seq/`CMD_DONE` decode (ISC-3), the `MT_EFUSE_CTRL` kick-word + the usage-map **HARD-FAIL** (the blank-clone abort, ported faithfully) + the EEPROM field decode yielding ver `0x0d` + MAC `xx:xx:xx:xx:xx:xx` (ISC-4), and the RX DMA segment walk + 28-byte RXWI strip + beacon/probe-response→BSSID/SSID extraction (ISC-5). The device layer (`mt7601u/mod.rs`) only adds the USB transfers around these cores; the bring-up sequence (register access via USB vendor control, ASIC-ready/PLL, firmware FCE-DMA upload in 14,336-byte chunks via vendor req `0x42` + IVB control upload, the MCU in-band command/response interface, eFUSE read, MAC/BBP initvals, PHY/RF init, `mac_start`, and the channel-hopping scan) is a faithful port of the Linux `mt7601u` driver (the FINDINGS §1 file:line reference; every register constant pulled from RAW upstream source, never a summarizing fetch). **I-14 (restated, no new guard):** the bulk/event path takes no AI-governance lock and adds only a device-state leaf lock at `USB_DEVICE` (a); RX drains in bounded passes, `EP_QUEUE_CAP` (b); every DMA buffer is a `FencedDmaAddr` and the xHCI controller mediates all device DMA inside VT-d's §10.6 fence domain — the firmware blob cannot DMA arbitrary host memory (c). **Kill-switch:** the whole driver + the per-channel PHY tables compile only under the `mt7601u` feature (default-off ⇒ bit-for-bit today, verified). **OT-27a (cold firmware self-upload, ADR-054 amended):** NeoPlutarch now uploads the MT7601U firmware *itself* from a cold device — `fce_preload_setup`'s `vendor_reset` → `msleep(5)` → `COM_REG0==1` poll, bounded 3-attempt retry (the THINK §5 P2 recovery) — so booting NeoPlutarch alone (no Linux warm-load) reaches WiFi; the `upload_logic` fault classifier buckets a failed cold boot into endpoint-stall / MCU-no-boot / wrong-variant / blob-missing-corrupt (the byte-progress + COM_REG0-progression + chip-id breadcrumbs) so the blind Stage-B is conclusive. The 45,412-byte firmware blob is **no longer `include_bytes!`'d into the kernel and is NOT committed to git** — it is staged as a **Limine boot module** (`/modules/mt7601u-fw`, third-party "Redistributable", recorded in NOTICE) only when present on disk, and looked up at runtime via `firmware_blob()`, **fail-closed** if absent — the same out-of-band, not-in-git discipline as the AI model blob (a fresh checkout compiles the WiFi kernel without it). **Blind-hardware discriminators (the standing rule):** each stage emits a self-localizing breadcrumb keyed to the known-good ASIC rev `0x76010001` / MAC rev `0x76010500` / EEPROM ver `0x0d` / MAC `xx:xx:xx:xx:xx:xx`, captured into statics the Stage-B scenario prints (`stage=` enumerated→asic→firmware→mcu→efuse→mac/bbp→phy→mac-start→scanned, + the read-back revs/ver/MAC + the BSSID count), so a failure self-diagnoses in one run. **Gate:** Stage A is the host byte-oracles (QEMU has no MT7601U model — stated honestly); Stage B is the conclusive bare-metal scan on the OptiPlex with the dongle in hand.

---

### 10.11 WiFi supplicant — MLME + 4-way + CCMP + NetDevice (Phase 7.5 BUILD-W2 — ADR-057)

The WiFi arc's capstone: the OS **joins a real network**. W2 takes W1's scan-only driver to *associated + encrypted data* via an **in-kernel minimal supplicant** — mac80211 leaves the 4-way handshake to a userspace supplicant (wpa_supplicant), and NeoPlutarch has none, so W2 owns both the 802.11 MLME state machine **and** the handshake logic, wiring §10.9's proven crypto over W1's frames. Flow: scan (W1) → open-auth → assoc → (WPA2) 4-way handshake → CCMP keys installed → Connected → the link is registered as a Phase-4 `NetDevice` (the §10.7 seam, the ADR-047 CDC-ECM precedent) so the existing IPv4/DHCP/TCP stack runs over WiFi. **The byte-oracle spine (I-12):** 133 host tests in `tools/ai-ops-tests`, each against hand-laid fixtures independent of the producer — the MLME mgmt frames (open-auth/RSN-IE/assoc-req + parse offsets + the state machine, also cross-checked by an independent Forge derivation), the TX descriptor (TXWI + DMA TXINFO wrap), the EAPOL-over-data encap, the EAPOL-Key M2 builder, AES Key Unwrap (RFC 3394 §4.1), the WCID key-install encoding, CCMP PN/replay, and the 802.3↔802.11 conversion — joined to §10.9's already-KAT'd PMK/PTK/GTK/EAPOL-MIC/AES-CCM/SAE. **Key decisions:** CCMP is **hardware-offloaded** (the firmware encrypts/decrypts once the pairwise key is written to the WCID hardware key table via plain register writes, not MCU commands), with W0's AES-CCM the byte-oracle'd correctness reference and a software replay window enforced; the GTK from handshake M3 is AES-Key-Wrapped, so W2 added `wpa::keywrap::aes_unwrap` (W0 had the KEK but not the unwrap); every 802.11 constant/layout was pulled from RAW `include/linux/ieee80211.h`/`nl80211.h` (never a summarizing fetch), which corrected four wrong values in the prep FINDINGS (`WLAN_AUTH_OPEN`, `WLAN_AUTH_SAE`, `EID_EXT_SUPP_RATES`, the RSN PSK AKM type) that would have silently broken association. **I-14 (restated, no new guard):** the MLME/TX/RX path takes no AI-governance lock (a); the supplicant's poll loops are bounded (b); all TX/RX/key DMA stays in the fenced arena behind VT-d, re-asserted `fence_refusals=0` on Stage-B (c). **Kill-switch:** the whole supplicant + `WifiNetDevice` compile only under the `wifi`/`mt7601u` feature (default-off ⇒ bit-for-bit, verified). **Blind-hardware discriminators:** the assoc scenario dumps `assoc_stage` (auth→assoc), `handshake` (M1→M4), `last_status` (reject vs silence), `connected`, and `fence_refusals` every run, so a blind failure self-localizes. **OT-2 (CLOSED — W2 is now a security boundary):** the SNonce is a CSPRNG draw from the kernel ChaCha20 DRBG — `wifi` now pulls `kernel-rng` and `gen_snonce` → `crate::rng::fill_random`, so the PTK depends on cryptographically-random material (the counter survives only as an explicitly-insecure no-`kernel-rng` bring-up fallback). KAT'd: the 4-way handshake derives a correct, *distinct* PTK from a random non-counter SNonce + the M2 MIC round-trips and is PTK-specific (host `wpa_ot2_ptk_snonce_agnostic_kat`), while the CSPRNG-ness of the source rides the existing kernel-rng self-test. **Gate:** Stage A is the host byte-oracles (QEMU has no MT7601U model); Stage B is the conclusive bare-metal run on the OptiPlex, sequenced **slice A (open AP: scan→auth→assoc→DHCP) then slice B (WPA2-PSK: + the 4-way+CCMP)** — the "confirm the simple path first" discipline, since slice A green leaves only the already-KAT'd crypto delta for the WPA2 close. The Stage-B target AP is the config-swap in `mt7601u::configured_target`.

---

### 10.12 Interface identity — runtime InterfaceId registry + `/net/if/<name>` capability (Phase 7.5 CB-3 — ADR-068)

The connectivity lane's identity layer: interfaces become **runtime-named, hotplug-aware, capability-addressable** objects above `NetDevice`, where before there was only "interface 0" and compile-time `&'static str` names. Four parts, all behind a **two-layer default-off kill-switch** so nothing is observable until enabled. **(1) The `InterfaceId` registry (`net/identity.rs`):** a leaf-locked table populated from the generic `register_device` (no driver edits), giving each device a runtime `id`, a runtime name (`eth0`/`wlan0`, class inferred from the existing identity strings), and a hotplug-tracked `LinkState` refreshed from the generic `NetDevice::link_state()` in `net::tick` — the **single source of truth** for "is `wlan0` up?", with the binding invariant *registry index i ≡ ipv4/ipv6 interface_id i ≡ get_device(i)*. **(2) Multi-interface `select_source` (`net/interface.rs`):** the RFC-6724 source pick extended across **all UP interfaces** (the lowest-index UP interface with a usable source for the family), and the egress sites' hardcoded `get_device(0)` / `submit_or_queue(0,…)` replaced by `egress_interface_for_source(src)` — the interface that owns the chosen source. **(3) The `/net/if/<name>` capability (`fs/mod.rs`):** a synthetic `NetIfFs` mounted root-child `net` whose `lookup("if/<name>")` yields a small `InterfaceInode` (reads `<name> <link>`); opening it via the **unchanged** `sys_open` mints a `Handle::File` that enters the I-15 derivation/grant graph exactly like any file cap — **zero new `Handle` kind, zero new syscall, zero change to `derive_handle`/`revoke_handle`** (a new `Handle` variant was rejected as strictly more surface for no benefit). **(4) The two-layer kill-switch (`net/identity_logic.rs`):** `interface-identity` (default-off) gates all of the above — off ⇒ the cross-interface pick is the constant `0`, the registry is inert, `/net` is never mounted, so the stack is **bit-for-bit 7.5-C2** (index semantics, ambient `get_device(0)`, the `&'static str` names), an I-2-form collapse to the proven core; `interface-caps` (default-off, depends on `interface-identity`) is a **second** sub-switch gating whether egress *requires* an interface cap, so naming lands before access is gated. **Proof discipline:** the disabled-equivalence is the Rule-1 artifact — `tools/interface-identity-logic-tests` sweeps **every** `(up, has_source)` configuration of four interfaces and asserts the disabled pick is interface 0 (the I-2 baseline), plus a contrapositive (eth0 down ⇒ eth1 wins); the QEMU `interface_identity_select` (ISC-1/2/3, two virtio-net devices) and `interface_cap_derive_revoke` (ISC-4, neo-init opens `/net/if/eth0` → derive → revoke → descendant dies) scenarios prove the ON behavior at NEO_SMP=4. **I-14 (no new guard):** the registry has no ISR path and takes no AI-governance lock — (a) lock isolation N/A by construction; it holds names/ids/link-state, not buffers — (c) memory isolation N/A by construction (no DMA); the leaf registry lock cannot stall governance — (b) inherited. **No new invariant** (the registry is metadata; the cap is I-15). Scope is identity only: the mt7601u/xHCI driver and the DHCP/wlan0 RX path are untouched. Deferred (flagged): full cross-interface RFC-6724 longest-prefix-match, IPv6 RA-per-interface population, and administrative up/down via the cap.

---

## 11. Inference Serving and Observability

### 11.1 Autoregressive generation (Phase 8 BUILD-8.1, ADR-053)

The inference-serving layer wraps the existing governed forward pass. A
generation call provides token IDs, a sampling policy, a stop condition, and a
loaded model. For each generated token, the kernel rebuilds the model input from
the current token context, calls `exec::run`, reads logits for the current last
position, samples the next token, appends it, and repeats until EOS or
`max_new_tokens`.

BUILD-8.1 is correctness-first: it recomputes the full context every step. There
is no KV cache in this layer; the KV cache is the follow-on optimization and is
proven against this output. There is also no tokenizer and no quantization here:
the interface is token IDs in, token IDs out, over f32 logits.

`.neom` v1 has static IO shapes, and `exec::run` validates tensors exactly
against those shapes. Generation therefore uses a bounded static context window:
the model's single `I32` rank-1 input declares the window length, the current
tokens occupy the prefix, and remaining slots are filled with a configured
`pad_token_id`. Rank-2 logits are sampled at row `current_context_len - 1`;
rank-1 logits are treated as the already-selected vocab vector. A request that
would exceed the declared context window fails closed.

Sampling order is fixed:

1. Scale logits by `1 / temperature`.
2. Apply top-k if configured.
3. Apply top-p if configured.
4. Sample from the renormalized categorical distribution.

Greedy sampling is the argmax path and breaks ties by the lowest token id.
Non-greedy sampling uses PCG32 XSH-RR, seeded explicitly per generation call, as
a deterministic non-cryptographic PRNG. The reference algorithm and published
seed vector are pinned by ADR-053.

Gate: host KATs for PCG32, sampler modes, and the pure decode loop; plus the
QEMU `ai_generate_decode` scenario over a hand-laid `.neom` whose greedy sequence
is `[2, 3, 4]` from prompt `[1]`.

Anti-scope: tokenizer, KV cache, quantization, beam search, speculative decode,
dynamic-shape `.neom`, and executor hot-path restructuring.

### 11.2 GGUF importer and BPE tokenizer (Phase 8 BUILD-8.2, ADR-055)

`.neom` remains the native executable model format. GGUF is accepted only through
a default-off importer path that loads external quantized-model bytes into the
existing Model object for enumeration and later dequant work. The importer does
not make GGUF native, does not write a `.neom` v2 quant format, and does not
execute a GGUF graph.

The GGUF importer parses GGUF v3:

- Header: magic `GGUF`, version 3, signed tensor count, signed metadata count.
- Metadata: typed key/value entries, including arrays and strings, with
  `general.alignment` selecting the tensor-data alignment when present.
- Tensor table: name, rank, GGML dimensions, GGML dtype, and data offset. Tensor
  data offsets are relative to the aligned data section and are validated as a
  sequential padded layout.
- Quantized blocks: `block_q4_K` is parsed as 144 bytes per 256 values
  (`d`, `dmin`, 12-byte `scales`, 128-byte `qs`); `block_q8_0` is parsed as 34
  bytes per 32 values (`d`, 32-byte signed `qs`). BUILD-8.2 only extracts these
  typed buffers. Dequantization is deferred to Phase 8.3.

Imported GGUF tensors are stored byte-for-byte as immutable byte tensors while
their GGUF shape/type/block metadata is retained separately for enumeration and
future dequant. `.neom` graph execution remains the only native execution path.

The BPE tokenizer is byte-level and table-driven. It consumes raw-source vocab
and merge tables, applies byte-level pre-tokenization, runs the merge loop by
rank, emits token IDs, and detokenizes by reversing byte-level tokens. The
Phase-8.2 KAT pins Qwen2.5 raw tokenizer assets against:

```
"Hello, world!\n" -> [9707, 11, 1879, 4894]
```

This corrects the earlier gap-analysis draft IDs, which do not match the
raw-source Qwen2.5 tokenizer bytes.

Gate: host byte-oracles for GGUF header/metadata/tensor-info, field-exact Q4_K
and Q8_0 block extraction, the raw-source tokenizer KAT, and an environment
gated real-model load of `Qwen2.5-0.5B-Q4_K_M.gguf` that enumerates tensor
names/shapes/dtypes.

Anti-scope for BUILD-8.2: dequantization, RoPE, SiLU/SwiGLU, attention, KV
cache, forward execution, quantized matmul, and adopting GGUF as native.

#### 11.2.1 GGUF dequant-to-f32 matmul seam (Phase 8 BUILD-8.3)

BUILD-8.3 is the bounded integration of the §11.2 importer and the Q4_K/Q8_0
dequant helpers into the existing f32 op path. A named imported GGUF tensor can
be selected from the retained tensor metadata, its byte payload can be minted
from the existing Model-owned immutable U8 tensor storage, and supported
quantized formats are materialized as an ordinary f32 buffer:

- `Q4_K`: iterate 144-byte blocks, using the raw-source llama.cpp
  `get_scale_min_k4` packing and `y = d * scale * q - dmin * min`.
- `Q8_0`: iterate 34-byte blocks and apply `y = f16(d) * int8(qs[i])`.

The resulting f32 data is consumed by existing op kernels such as `Matmul`.
This is not native GGUF execution and not quantized matmul; it is the first-light
correctness path that proves real imported weights can compute through the
already-governed f32 machinery.

Gate: host known-answer tests against a synthetic Q4_K/Q8_0 tensor path plus an
environment-gated real `Qwen2.5-0.5B-Q4_K_M.gguf` check. The real check pins
`blk.11.ffn_down.weight` dequantization against an independent raw-source
llama.cpp reference and asserts an existing f32 matmul over a real-weight slab.

Anti-scope for BUILD-8.3: RoPE/SiLU/SwiGLU forward-graph wiring, GQA attention,
KV cache, quantized matmul, `.neom` v2, GGUF-native graph execution, QEMU
scenarios, new invariants, and new `unsafe`.

#### 11.2.2 Qwen layer RoPE/SwiGLU scaffold (Phase 8 BUILD-8.4)

BUILD-8.4 wires the §11.2.1 dequant-to-f32 seam into a default-off Qwen2 layer
scaffold. The scaffold consumes real Qwen2.5-0.5B GGUF tensor shapes after
supported weights have been materialized as ordinary f32 buffers:

- `attn_norm.weight` `[896]`
- split `attn_q.weight` `[896, 896]`, `attn_k.weight` `[896, 128]`, and
  `attn_v.weight` `[896, 128]`
- `ffn_norm.weight` `[896]`
- `ffn_gate.weight` and `ffn_up.weight` `[896, 4864]`
- `ffn_down.weight` `[4864, 896]`

The attention-side scaffold is:

```
x -> RMSNorm -> Q/K/V matmuls -> RoPE(Q/K)
```

RoPE uses Qwen2.5's `theta = 1e6` frequency scale and the llama.cpp NEOX
split-half pair convention. The scaffold returns projected Q/K/V and rotated
Q/K for the future attention implementation; it does not compute attention and
does not mutate KV-cache state.

The FFN-side scaffold is:

```
x -> RMSNorm -> gate/up matmuls -> SwiGLU -> down matmul -> residual add
```

This uses the existing RMSNorm, matmul, add, and BUILD-8.4a SwiGLU kernels. No
quantized matmul is introduced; GGUF remains an importer format and `.neom`
remains the executable model format.

The dequant seam supports the real tensor formats needed by the Qwen2.5
Q4_K_M artifact used for this gate, including GGML `Q5_0`'s 22-byte blocks
(`d`, 32 high-bit flags, and 16 packed low-nibble bytes). `Q5_0` materializes
to f32 before matmul just like the existing Q4_K/Q8_0 paths.

Gate: host I-12 float-oracles. With `NEO_QWEN25_GGUF` set, the real-weight FFN
block and real projected-Q/K RoPE path are checked against independent
raw-source llama.cpp-derived references over the same GGUF tensor bytes. A
synthetic scaffold test verifies the explicit unsupported-attention seam.

Anti-scope for BUILD-8.4: GQA attention, KV cache, full generation on real
weights, quantized matmul, `.neom` v2, GGUF-native graph execution, new
invariants, and new `unsafe`.

#### 11.2.3 GQA attention and governed KV cache generation (Phase 8 BUILD-8.5)

BUILD-8.5 completes the default-off real-weight Qwen2.5-0.5B forward path by
feeding BUILD-8.4's RoPE'd Q/K/V seam into causal GQA attention, then running
the full decoder stack through final RMSNorm, output projection, and the
BUILD-8.1 sampler. The implemented real-model shape is 24 layers, hidden size
896, intermediate size 4864, 14 Q heads, 2 KV heads, grouping factor 7, and
Qwen2.5 `theta = 1e6` RoPE. The real GGUF's attention Q/K/V bias tensors and
`attention.layer_norm_rms_epsilon = 1e-6` metadata are part of the forward
contract.

The KV cache is the ratified Option A governed tensor design. At generation
start, each layer pre-reserves fixed-capacity f32 K and V tensors sized for the
selected max context. Those tensors are registered through the existing arena
and tensor-governance path at pre-reserve time, so the arena, reservation
accounting, I-9 production tensor rule, and I-13 liveness rule all continue to
see fixed-size tensors. The cache's `used_len` is internal bookkeeping and does
not require a new invariant.

RoPE is applied at cache write: K is rotated before it is appended, and cache
reads return the already-rotated active prefix. The read path never re-applies
RoPE. `kv_cache_enabled = false` is the kill switch and selects the uncached
recompute-all path; the cache is not read or written by attention in that mode.

The real GGUF is still an importer input, not a native executable graph. All
supported quantized weights materialize as f32 before existing matmul ops, and
BUILD-8.5 adds the GGML `Q6_K` dequant path only because the real
Qwen2.5-Q4_K_M artifact needs it. Quantized matmul remains out of scope.

Gate: host I-12 KATs assert cached decode equals uncached recompute-all within
the f32 tolerance, assert cache position/full-bound behavior, and assert the
kill switch is equivalent to the uncached path. The real-generation oracle is
raw-sourced from local llama.cpp commit
`5f04dc7ac3f40271f60105574be0f617887195bb`: prompt `Hello` token `9707`
greedily produces top-1 token `271` (`"\n\n"`). No WebFetch-derived token IDs
are authoritative.

Anti-scope for BUILD-8.5: active KV eviction/prefetch, segment eviction,
multi-stream cache state, quantized matmul, `.neom` v2, GGUF-native graph
execution, new invariants, and new `unsafe`.

ADR-056 is the Rule-4 consolidation pointer for this section. It factors the
shared KV-cache storage convention into one helper used by both the host KAT
cache and the governed arena cache, and promotes the workspace/kernel/no_std
CI gates to blocking. The BUILD-8.5 behavior and anti-scope above are unchanged.

#### 11.2.4 Quantized Q4_K x Q8_K matmul helper (Phase 9, ADR-058)

BUILD-9.1 adds a standalone scalar Q4_K x Q8_K helper under `ai/ops/`. It
quantizes f32 activations into llama.cpp-compatible `block_q8_K` blocks
(256 signed quants plus 16 per-16-element sums), then computes scalar integer
dots against raw `block_q4_K` weight blocks using the existing 144-byte Q4_K
layout, 6-bit scale/min depacking, i32 accumulators, and the q8_K `bsums`
min-correction.

The kernel compiles this helper only when `phase9-quantized-matmul` is enabled;
the feature is default-off. In this build no forward path, GGUF importer,
executor op kind, tensor ABI dtype, decode loop, or sampler behavior changes.
The helper is host-KAT'd through the existing `ai-ops-tests` path-module
surface.

BUILD-9.2 wires the helper into `forward_logic` without promoting quantized
matmul to a graph `OpKind`. Qwen2 layer weights carry the existing f32 RHS
slices and optional raw Q4_K RHS bytes. With `phase9-quantized-matmul` off, the
selector calls the existing f32 `matmul_vec()` path. With the feature on and a
raw Q4_K sidecar present, `matmul_vec_quantized()` quantizes the f32 activation
to Q8_K blocks and calls the scalar Q4_K x Q8_K matrix helper. Non-Q4_K tensors
continue to use the f32 fallback.

Gate: the q8_K quantizer has a hand-laid exact byte oracle for `d`, quants, and
`bsums`; the scalar Q4_K x Q8_K dot is checked against a raw-source
transcription of llama.cpp commit `74ade52741203e5c8f81eaf06a96cb1cfe15f2a3`
`ggml_vec_dot_q4_K_q8_K_generic` on the published cosine-data generator shape
with normalized error `<= 0.02`.

BUILD-9.3 adds a host-tier AVX2 selector for the same op. It uses
`_mm256_maddubs_epi16` for unsigned-Q4-by-signed-Q8 adjacent-pair products,
widens through `_mm256_madd_epi16` into i32 lanes, accumulates across the eight
Q4_K sub-blocks, and horizontally reduces to the same scalar `scaled_dot`.
Runtime selection requires CPUID AVX/AVX2, OSXSAVE, and XCR0 XMM/YMM enablement;
otherwise the selector falls back to scalar.

The AVX2 gate is bit-identical equality against the scalar oracle, not a
tolerance. The AVX2 path is not used by the in-kernel forward pass in this
slice: the kernel does not yet preserve YMM high halves across context
switches/IRQs/syscalls, so an in-kernel AVX2 swap is blocked by OT-23 until a
separate XSAVE/XRSTOR state-preservation slice lands. Scalar remains the
in-kernel path.

Anti-scope for BUILD-9.1: forward wiring, dual raw+f32 weight storage, AVX2,
GPU kernels, `.neom` v2, GGUF-native graph execution, new invariants, and new
`unsafe`.

Anti-scope for BUILD-9.2: AVX2, latency measurement, raw-only weight storage,
new executor op kinds, GPU kernels, `.neom` v2, GGUF-native graph execution,
new invariants, and new `unsafe`.

Anti-scope for BUILD-9.3: latency measurement, raw-only weight storage, GPU
kernels, `.neom` v2, GGUF-native graph execution, in-kernel AVX2 forward swap,
and the YMM-state preservation fix.

### 11.3 Per-process AI accounting

The kernel tracks per-process:

- Tensor storage in use (bytes, by device).
- Models loaded (count, total weight bytes).
- Inference calls completed (count, by priority class).
- Total accelerator time consumed (per device).
- Deadline misses (count).

These are exposed via `sys_proc_stats(pid, AI)` (Phase 5+).

### 11.4 System-wide AI events

The kernel emits structured AI events as line-oriented `key=value` records to the
kernel log. The event taxonomy is part of ANSPEC-001's versioned surface: new
events, new fields on existing events, or removal of fields require an ADR and a
spec-version bump. The wire format is LogansOS-native; we do not adopt
syslog(3) RFC 5424, journald binary format, OpenTelemetry, or Linux audit
record formats.

#### 11.4.1 Field-type contract

Every field has a type and, where applicable, a unit. Types are spec-stable
within a major spec version.

| Field      | Type           | Unit / Encoding                                          |
|------------|----------------|----------------------------------------------------------|
| `pid`      | `ProcessId`    | unsigned 64-bit, decimal                                 |
| `device`   | `DeviceId`     | `cpu` or `accel:<n>` where `<n>` is decimal              |
| `name`     | string         | quoted, escaped per the standard kernel-log escape rules |
| `hash`     | BLAKE3 digest  | 64-char lowercase hex (per §8.2)                         |
| `bytes`    | unsigned 64    | bytes, decimal                                           |
| `wall_ns`  | unsigned 64    | nanoseconds, decimal                                     |
| `accel_ns` | unsigned 64    | nanoseconds, decimal                                     |
| `cpu_ns`   | unsigned 64    | nanoseconds, decimal                                     |
| `missed_by_ns` | unsigned 64 | nanoseconds, decimal                                    |
| `refcount` | unsigned 64    | decimal                                                  |
| `requested`| unsigned 64    | bytes, decimal                                           |
| `available`| unsigned 64    | bytes, decimal                                           |
| `model`    | `ModelHandle`  | unsigned 64, decimal (kernel-internal handle id)         |
| `ticket`   | `InfTicket`    | unsigned 64, decimal                                     |
| `reason`   | enum           | one of an event-specific enumeration; see per-event spec |

Fields not listed here are not part of the v0.1.0 contract; consumers must
ignore unrecognized fields rather than treat them as errors. Producers must
not reuse a field name with a different type across events.

#### 11.4.2 Event taxonomy

Each event has a fixed name, a fixed severity, and a fixed required-field set.
Severity is one of `INFO`, `WARN`, `ERROR`. The kernel emits all events at or
above the configured severity floor (default `INFO`).

**Lifecycle events (severity `INFO`):**

```
[ ai ] model_load           pid=N name="…" hash=… bytes=… device=…
[ ai ] model_unload         pid=N name="…" reason=…
[ ai ] inf_run              pid=N model=… deadline_met=<bool> wall_ns=… accel_ns=… cpu_ns=…
[ ai ] inf_cancel           pid=N model=… ticket=… reason=…
[ ai ] dedup_hit            hash=… refcount=…
[ ai ] dedup_miss           hash=… bytes=…
```

**Soft-failure events (severity `WARN`):**

```
[ ai ] inf_deadline         pid=N model=… missed_by_ns=…
```

**Hard-failure events (severity `ERROR`):**

```
[ ai ] model_load_failed    pid=N name="…" reason=…
[ ai ] oom                  device=… requested=… available=…
```

#### 11.4.3 Per-event `reason` enumerations

`reason` field values are spec-stable enumerations, not free-form strings.
The v0.1.0 set:

- `model_unload.reason`: `process_exit` | `explicit_unload` | `model_eviction` | `dedup_refcount_zero`
- `inf_cancel.reason`: `caller_request` | `deadline_expired` | `process_exit` | `device_lost`
- `model_load_failed.reason`: `hash_mismatch` | `unsupported_op` | `shape_invalid` | `dtype_invalid` | `cycle_in_graph` | `quota_exceeded` | `io_error`

Adding a value to a `reason` enumeration is a minor-version change. Removing a
value is a major-version change.

#### 11.4.4 Consumer expectations

These events are consumed by:

- A userspace AI-aware system monitor (Phase 8 deliverable).
- The per-process accounting paths in §11.3 (events feed the rollups).
- Future tracing infrastructure (§11.5).

Log consumers must tolerate event reordering across CPUs (events are emitted
from whichever CPU services the originating syscall) and must not assume
monotonic timestamps without an explicit timestamp field. v0.1.0 events do not
carry timestamps; consumers correlate against the kernel log's surrounding
timestamp record.

#### 11.4.5 Open questions

These are deferred to a future minor revision; naming them here so they don't
get rediscovered:

- **Runtime emission control.** The default severity floor is `INFO`. Whether
  this is per-event-class, per-process, or globally configurable is unsettled.
  Phase 8 monitor work is the natural venue for resolving this, since the
  monitor is the first consumer that knows what control surface it needs.
- **Timestamp inclusion.** Whether events should carry their own monotonic
  timestamp rather than relying on the kernel log's surrounding record. Adding
  a timestamp is cheap; the question is whether v0.1.0 consumers actually need
  it or whether kernel-log correlation suffices.
- **Sampling.** A high-throughput inference workload (1000+ `inf_run`/sec) will
  flood the log. Whether to add per-event-class sampling (1-in-N emission) is
  unsettled.

### 11.5 Tracing hooks

A future-phase tracing system (likely Phase 7) instruments op dispatch with timestamps. This lets userspace tools profile inference performance without invasive instrumentation. The tracing format is our own; we are not adopting Linux's perf or eBPF.

### 11.6 Arthur action-surface disposition layer (Phase 8 Arthur Slice 2)

Arthur's action surface is a userspace disposition layer, not a kernel AI
primitive. The model may propose a typed action, but `libneo::action` checks an
explicit userland grant set before invoking the existing syscall sequence for
that action. On missing authority it returns a structured `Denied` result and
does not issue the syscall.

The v1 catalog covers file read/write/list, process spawn/kill/inspect/wait,
TCP dial/send/recv, inference run, and model load. Each action maps to existing
syscalls and existing kernel capability checks. File write uses the already
present `SYS_WRITE` FileHandle path; no syscall extension is required.

This layer is defense-in-depth and caller UX: the kernel remains the
enforcement boundary, while the wrapper gives Arthur a clean "model proposes,
capability disposes" refusal before the syscall. The surface is additive and
unused unless a userspace caller invokes it.

### 11.7 Kernel CSPRNG prerequisite (ADR-057 / OT-2)

The kernel CSPRNG is not an AI op and does not change inference execution, but
it is a security prerequisite for later Arthur and WiFi consumers. ADR-057
records BUILD-10.1: the default-off `kernel-rng` entropy and ChaCha20 DRBG core
with published-anchor KATs. That slice proves the DRBG core only.

The consumer surfaces are intentionally later. **OT-2 is now CLOSED:** the
kernel-internal `fill_random` API exists (10.3) and W2's counter SNonce has been
replaced by a CSPRNG draw (`wifi` pulls `kernel-rng`, `gen_snonce` →
`fill_random`), so the 4-way handshake derives its PTK from cryptographically-
random material — W2 is a security boundary (§10.11). Arthur userland encryption
needs the userland `SYS_GETRANDOM`
surface, so it depends on the later userland randomness slice, not merely the
10.1 DRBG core.

### 11.8 Arthur grant store (OT-18 Slice E1 — ADR-060)

The §11.6 disposition layer checks a grant set the model cannot widen; the grant
*store* is where that set comes from across sessions. `libneo::grant_store` defines
`.arthur/grants.json` — a versioned, human-readable JSON record of the user's
durable coarse grants (D2's six resource-class scopes; `scope`/`resource`/`rights`/
`granted_by`, the last being the OT-20 provenance seam) — plus a no-alloc, zero-copy
loader that re-derives Arthur's in-process `Grant` set at startup. **The authority
boundary is unchanged and explicit (Article 29.3): the kernel capability Arthur holds
is the enforcement boundary by construction; `grants.json` is persistence/UX/audit,
never authority.** The in-process `Grant` set is the userspace policy mirror only — a
`Grant` present with no backing kernel cap still fails `EPERM` at the syscall, proven
by the host boundary test (`grant_in_set_without_kernel_cap_still_eperm`), not asserted.
**D7 least-privilege default:** first boot / no store / corrupt store ⇒ `FileRead` on
`.arthur/` only (read its own grants + OT-17 memories), never widened past `.arthur/`
(the D5 guard — OT-19 reversibility unbuilt). **Kill-switch `arthur-grants`:** disabled
⇒ no durable grants (spawn-inherited only) ≡ pre-E1 bit-for-bit; the store cannot leak
in when off. **Byte-oracle discipline (I-12):** the format is pinned by a hand-authored
`grants.json` fixture asserted field-by-field against the loader (ADR-060's worked
example), never a writer→reader round-trip; v1 is restricted JSON (no string escapes,
unknown keys rejected) and any malformed store degrades to the D7 default — no crash,
no widening. E1 is the host-coverable grant *infrastructure* (format + loader + D7 +
kill-switch + boundary); the kernel-cap re-derivation at spawn (`derive_handle` +
`ipc::grant`) and the neo-arthur startup-load wiring are the kernel-touching follow-on
with the mandatory `arthur_grants_persist` QEMU gate, and E2 (on-miss re-request) / E3
(audit + revoke) / E4 (memory scopes) remain later slices.

---

## 12. Phasing and Milestones

### 12.1 Phase 5 — Foundation

**Goal milestone:** A statically-linked test binary can load a small `.neom` file (a tiny MLP), run it on the CPU backend, and produce correct output verified bit-exactly against a reference.

Deliverables:

- `kernel/src/ai/tensor.rs` — Layer 0 implemented.
- `kernel/src/ai/model.rs` — Layer 1 implemented (graph parsing, weight binding).
- `kernel/src/ai/alloc.rs` — tensor allocator (CPU-only).
- `kernel/src/ai/syscall.rs` — Tensor and Model syscalls.
- `kernel/src/drivers/accel/cpu.rs` — Tier 1 ops on CPU.
- `userland/libneo/src/ai.rs` — userspace bindings.
- `userland/neo-import/` — minimal ONNX→`.neom` for one architecture (a small MLP suffices for Phase 5 verification).
- `docs/specs/neom-format.md` — finalized format spec.

Anti-scope:
- No accelerators beyond CPU.
- No async inference.
- No streaming.
- No deduplication (added in Phase 7).
- No multi-process model sharing.

### 12.2 Phase 6 — Accelerators

**Goal milestone:** The same `.neom` runs on the custom virtio-accel device in a QEMU VM, and on one real GPU on bare metal, with output verified equivalent to the CPU baseline (within numerical tolerance for non-bit-exact dtypes).

Deliverables:

- `kernel/src/drivers/accel/virtio_gpu.rs`.
- `kernel/src/drivers/accel/<real_gpu>.rs`.
- `sys_accel_enumerate` and `sys_accel_info`.
- `sys_tensor_migrate` between CPU and accelerator.
- Async inference syscalls.

Anti-scope:
- Inference scheduling is naive (FIFO per device). The real scheduler is Phase 7.
- No deduplication yet.

### 12.3 Phase 7 — Scheduler & Multi-tenancy

**Goal milestone:** Two processes simultaneously serving inference from different models on the same accelerator, with a third process running a realtime-priority small model that consistently meets its 10ms deadline despite the contention.

Deliverables:

- `kernel/src/sched/inference.rs` — the inference scheduler proper.
- Priority classes and deadlines.
- Op-level preemption where the driver supports it.
- Deduplication (`DEDUP_CANDIDATE` flag honored).
- Model sharing across processes via cap-passing.
- Streaming inference syscalls.
- KV-cache and Tier 2 ops.

Anti-scope:
- Training. Still not in scope, ever, for this spec.
- Distributed inference across machines.

---

## 13. Open Questions

These are deferred to ADRs as they arise; they are not blockers for Phase 5.

- **Q1.** How is model signing handled for trusted-source models? (Likely: optional signature in `.neom` header verified against a kernel keyring; policy decides whether unsigned models load.)
- **Q2.** How are op extensions versioned? (Likely: each op carries a version integer; models declare required versions; kernel rejects unsupported.)
- **Q3.** How does the accelerator scheduler interact with thermal/power events? (Out of scope until we have hardware where it matters.)
- **Q4.** Do we expose a "lower-level" graph-build API to userspace, or only `.neom` loading? (Currently: only `.neom`. Programmatic graph construction would expand attack surface. ADR if reconsidered.)
- **Q5.** Quantization ops at runtime — how do we balance flexibility with op-set finitude? (Likely: `quantize_dynamic` and `dequantize` are kernel ops; specific schemes are op attributes.)

---

## 14. Glossary

- **Accelerator** — Any non-CPU compute device usable for tensor operations: GPU, NPU, TPU, FPGA accelerators, etc.
- **Capability handle** — A per-process integer mapped by the kernel to an underlying object plus permissions. The userspace-visible reference to AI subsystem objects.
- **Dedup** — Deduplication of tensor storage by content hash.
- **`.neom`** — LogansOS native model format.
- **Op** — A single tensor operation drawn from the kernel's enumerated op set.
- **Op batch** — A contiguous sequence of ops dispatched together to an accelerator.
- **Tensor** — The kernel's fundamental AI data primitive (§3).
- **Tier 1/2/3** — Op support priority bands (§4.4).

---

## 15. References

- LogansOS Bible v1.0.4, Articles 2, 6, 19–23.
- Phase 1.4 Plan (`docs/phases/PHASE-1.4.md`) — establishes ELF loading and userspace conventions used by Phase 5.
- BLAKE3 spec — chosen content-hash algorithm.
- Limine Boot Protocol spec — module loading conventions for kernel-bundled files.

---

**Document end.**

*Amendments to this spec follow the standard ADR + bible-amendment process. Material changes to the syscall surface require a major-version bump.*
