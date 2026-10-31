<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-031: Phase 6 BUILD-3 — virtio-accel Device Protocol (substrate + byte oracle)

| | |
|---|---|
| **Status** | Accepted — Phase 6 BUILD-3 (virtio-accel) shipped; Phase 6 closed 2026-06-07 (`Plans/CLOSE-REPORT-2026-06-07-phase-6.md`). *(Status corrected 2026-06-17: was "Proposed", stale post-close.)* |
| **Date** | 2026-06-05 |
| **Authority** | PM/Cerebro + PL |
| **Related** | ADR-030 (Phase 6 substrate), ANSPEC-001 §9 (AccelDriver trait), `Plans/THINK-2026-06-04-phase-6-build-2-virtio-accel.md` (why vhost-user, why no QEMU fork); bible Articles 2.2/2.3, 6.4, 7.5, 9.1; CLAUDE.md "Format/ABI specs require an independent byte oracle" |

## Context

BUILD-2 (`accel_sim`) proved the *logical* multi-device + async + migration machinery in-kernel. BUILD-3 is the **hardware-transport de-risk**: a real virtio device whose data plane crosses an address-space boundary — virtqueue submission, DMA buffers, IRQ-driven completion — exercised in CI **before** a real GPU (BUILD-4) is ever the first contact with accelerator DMA.

Per the THINK pass: there is no off-the-shelf "virtio-gpu-compute" device. We define **our own** virtio device (`virtio-accel`) and present it in CI via QEMU's user-creatable **`vhost-user-test-device-pci`** (arbitrary `virtio-id`/`num_vqs`, **no QEMU fork**), backed by a host **vhost-user daemon** we ship. The guest driver reuses the kernel's existing virtio-pci/virtqueue transport (virtio-blk Phase 3, virtio-net Phase 4) and `DmaBuffer` (`drivers/block/dma.rs`).

This ADR fixes the **on-wire op-batch protocol** and ships the mandatory **hand-laid byte oracle** (§7). The protocol is the format; round-trip tests between our own driver and our own daemon are *not* sufficient (both could implement it wrong and still agree — the `.neom` weight_off bug precedent).

## Decisions

### D1 — Transport: virtio-pci, single virtqueue, vhost-user in CI
`virtio-accel` is a modern virtio-pci device. **One virtqueue** ("opq"): the driver posts an op as a descriptor chain on the avail ring; the device completes it on the used ring and raises the queue interrupt. CI presents it via `vhost-user-test-device-pci,virtio-id=$VIRTIO_ID_ACCEL,num_vqs=1,chardev=...`. `VIRTIO_ID_ACCEL = 41`; this value is accepted by QEMU 10.2.3's user-creatable vhost-user device and is unique within Neo's guest driver match table.

### D2 — Op = one descriptor chain, readable-then-writable
Each op submission is a chain (virtio requires all device-readable descriptors before device-writable):

1. **`VirtioAccelReqHeader`** (device-readable) — op kind, input count.
2. **`VirtioAccelTensorDesc × (input_count + 1)`** (device-readable) — one per input, then one for the output (dtype/rank/dims/byte_len).
3. **input buffers** (device-readable) — `input_count` raw tensor byte buffers, in order.
4. **output buffer** (device-writable) — `output_desc.byte_len` bytes the device fills.
5. **`VirtioAccelStatus`** (device-writable) — completion status + actual output length.

Segments 1–2 (the descriptor block) MAY share one descriptor; buffers 3/4/5 are their own descriptors (they are DMA buffers). The oracle asserts byte content, independent of descriptor splitting.

### D3 — Fixed little-endian layouts (all multi-byte fields LE)

```
VirtioAccelReqHeader            (16 bytes)
  0x00  op_kind        u32      // OpKind ABI: Identity=0 … Add=1 … Transpose=11
  0x04  input_count    u32      // 0..=max_inputs
  0x08  flags          u32      // v1: MUST be 0
  0x0C  reserved       u32      // v1: MUST be 0

VirtioAccelTensorDesc           (80 bytes)
  0x00  dtype          u32      // wire dtype code (D4)
  0x04  rank           u32      // 0..=8
  0x08  dims[8]        u64[8]   // row-major; dims[rank..] MUST be 0
  0x48  byte_len       u64      // length of this tensor's data buffer

VirtioAccelStatus               (16 bytes)   // device-writable
  0x00  status         u32      // 0 = OK; nonzero = error code (D6)
  0x04  reserved       u32      // device MUST write 0
  0x08  output_len     u64      // bytes actually written to the output buffer

VirtioAccelConfig               (virtio config space, device-read-only)
  0x00  supported_ops      u32  // bit i set ⇒ OpKind i supported
  0x04  supported_dtypes   u32  // bit d set ⇒ wire dtype d supported
  0x08  max_inputs         u32
  0x0C  queue_size         u32
  0x10  max_tensor_bytes   u64
```

The descriptor block for `input_count = N` is `16 + (N+1)*80` bytes: header, then N input descs, then the output desc (the output desc is the **last** `VirtioAccelTensorDesc`).

### D4 — Wire dtype codes (match ANSPEC §3.3 order)
`F32=0, F16=1, BF16=2, I8=3, U8=4, I32=5, BOOL=6, I4=7, F8_E4M3=8, F8_E5M2=9`. The kernel maps its `Dtype` to/from these at the driver boundary (the wire codes are the contract; the in-kernel enum discriminants are NOT the wire format). **v1 daemon supports F32 and I32 only** (the BUILD-1/2 op set); others → `status = UNSUPPORTED_DTYPE`.

### D5 — Async completion is interrupt-driven (the real de-risk)
`dispatch` builds the chain over `DmaBuffer`s, places the head on the avail ring, and notifies the device — then returns a `DispatchToken` carrying the **used-ring head index**. The device processes asynchronously; on completion it writes output+status, advances the used ring, and raises the opq interrupt. The kernel's virtio IRQ handler walks new used entries and marks the matching token complete (a per-token completion flag + the parsed `VirtioAccelStatus`). `poll` reads that flag; `wait` parks/spins until set. **This is where the BUILD-2 lazy simulation is replaced by genuine hardware-style async** — `wait()`'s contract (drive to completion or fail) is honored against a real used-ring/IRQ, and `preemptible() == false` still.

### D6 — Device memory = `DmaBuffer`; errors via status
`DeviceStorage` for `virtio-accel` wraps a `DmaBuffer` (guest-physical; the daemon maps guest RAM via vhost-user). `copy_to/from_device` are host↔DMA memcpys; a tensor migrated to `Accel(virtio)` lives in a `DmaBuffer` and rides the chain zero-copy. For ops on CPU-resident tensors the driver uses transient bounce `DmaBuffer`s and copies the result into the CPU-resident output (outputs stay CPU-resident, as in BUILD-1/2). Status codes: `OK=0, UNSUPPORTED_OP=1, UNSUPPORTED_DTYPE=2, BAD_REQUEST=3, SHAPE_MISMATCH=4, INTERNAL=5` → mapped to `AccelError` in the driver.

### D7 — The vhost-user daemon (`tools/virtio-accel-host`)
A Rust host binary implementing the vhost-user protocol + this device. For each chain it validates the header/descs, runs the op (reusing reference soft-float math or a host-native equivalent), writes the output buffer + status, and completes the descriptor. CI launches it (creating the socket) **before** QEMU and tears it down after. It carries the **same byte oracle** (§7) as an independent assertion — daemon and kernel each validate the hand-computed bytes separately.

## Anti-scope (BUILD-3 must NOT do)
Multi-queue; preemption; >1-deep pipelining beyond the queue depth; zero-copy beyond `DmaBuffer`s; f16/bf16/quantized dtypes (v1 = F32/I32); the inference scheduler (Phase 7); graphics/DRM. No `.neom`/op-math changes.

## §7 — Authoritative worked byte example (the oracle)

**Op:** `Add`, inputs `a=[1.0, 2.0]` and `b=[3.0, 4.0]` (f32, shape `[2]`), output `c=[4.0, 6.0]` (f32, shape `[2]`). Hand-computed, asserted independently in the kernel driver tests AND the daemon tests.

**(1) `VirtioAccelReqHeader`** (16 bytes):
```
01 00 00 00   op_kind     = 1   (Add)
02 00 00 00   input_count = 2
00 00 00 00   flags       = 0
00 00 00 00   reserved    = 0
```

**(2) `VirtioAccelTensorDesc` for input0** (80 bytes) — input1 and the output desc are **byte-identical** to this (same dtype/rank/dims/len):
```
00 00 00 00                included dtype = 0 (F32)
01 00 00 00                rank  = 1
02 00 00 00 00 00 00 00     dims[0] = 2
00*56                      dims[1..8] = 0   (56 zero bytes)
08 00 00 00 00 00 00 00     byte_len = 8     (2 × f32)
```
So the device-readable descriptor block = `16 + 3*80 = 256` bytes: header ‖ desc(in0) ‖ desc(in1) ‖ desc(out), the three descs identical.

**(3) input0 buffer** (8 bytes), `a=[1.0, 2.0]`:
```
00 00 80 3F   1.0f32 (0x3F800000)
00 00 00 40   2.0f32 (0x40000000)
```
**(3) input1 buffer** (8 bytes), `b=[3.0, 4.0]`:
```
00 00 40 40   3.0f32 (0x40400000)
00 00 80 40   4.0f32 (0x40800000)
```

**(4) output buffer** (8 bytes) — device writes `c=[4.0, 6.0]`:
```
00 00 80 40   4.0f32 (0x40800000)
00 00 C0 40   6.0f32 (0x40C00000)
```

**(5) `VirtioAccelStatus`** (16 bytes) — device writes:
```
00 00 00 00   status     = 0 (OK)
00 00 00 00   reserved   = 0
08 00 00 00 00 00 00 00   output_len = 8
```

Any implementation that produces different bytes for this exact op is off-spec, regardless of whether driver and daemon agree with each other.

## Consequences
- BUILD-3 reuses the virtio-pci/virtqueue/`DmaBuffer` transport, so the new surface is the protocol + the daemon + IRQ completion — bounded and CI-testable.
- The op-batch header/desc/status are on-wire formats ⇒ the §7 oracle is a **required deliverable** in both the kernel and the daemon.
- CI gains a daemon process + socket lifecycle (launch-before-QEMU); this is new CI plumbing to get right.

## Open questions for the PLAN
1. **Track split?** Daemon (`tools/virtio-accel-host`) and guest driver may be parallel tracks (cf. Phase 5 BUILD-2's kernel+tool split), meeting at the §7 oracle.
2. **Shared virtqueue module** — factor a reusable split-virtqueue helper out of virtio-net/blk, or copy the pattern? (Coherence vs. coupling — decide in PLAN.)
3. **`wait()` blocking** — park the thread on the token vs. bounded spin under QEMU TCG (cf. the TLB-shootdown timeout flake; avoid a tight unyielding spin).
4. **CI ordering/teardown** — xtask launches the daemon + socket before QEMU, reaps after; failure modes if the daemon dies mid-scenario.
5. **`virtio-id` final value** — resolved at BUILD: `VIRTIO_ID_ACCEL = 41`.
