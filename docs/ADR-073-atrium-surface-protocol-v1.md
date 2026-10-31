<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-073: Atrium Surface Protocol v1

**Status:** Accepted for G1 Slice 2

**Date:** 2026-07-19
**Design of record:** `THINK-2026-07-10-g1-compositor-SYNTHESIS.md` §4

## Decision

Surface control is a fixed-record protocol over existing anonymous IPC
channels. A `SurfaceCap` is the dedicated channel endpoint: object IDs are
namespaced lookup keys, never authority. No control syscall or kernel surface
object is added.

Every v1 request begins with Chad §2.2's 24-byte little-endian `MsgHeaderV1`
(`NMC1`, version, opcode, exact payload length, zero flags, request ID). Every
payload carries `namespace_id`, `surface_id`, and `surface_cap_provenance`.
Import additionally carries `buffer_cap_provenance`; the compositor installs
state only after the explicit offer/ready/`sys_grant`/`sys_recv_grant`/
accepted rendezvous succeeds.

All integers are little-endian. These offsets are the v1 ABI authority:

| Record | Fields and byte offsets | Total bytes |
|---|---|---:|
| Header (every request/reply) | `magic:u32@0`, `version:u16@4`, `opcode:u16@6`, `payload_len:u32@8`, `flags/reserved:u32@12` (= 0), `request_id:u64@16` | 24 |
| Request identity prefix | `namespace_id:u64@24`, `surface_id:u64@32`, `surface_cap_provenance:u64@40` | 48 through prefix |
| `CREATE_SURFACE` | identity prefix; `role:u32@48`, `width:u32@52`, `height:u32@56`, `reserved:u32@60` (= 0), `client_tag:u64@64` | 72 |
| `IMPORT_ATTACH_BUFFER` | identity prefix; `transfer_id:u64@48`, `buffer_cap_provenance:u64@56` | 64 |
| `COMMIT` | identity prefix; `client_serial:u64@48` | 56 |
| `FOCUS_REQUEST` | identity prefix; `reason:u32@48`, `reserved:u32@52` (= 0), `request_serial:u64@56` | 64 |
| `DESTROY_SURFACE` | identity prefix; `final_client_serial:u64@48` | 56 |
| Reply | header with `opcode \| 0x8000`; `status:u32@24`, `reserved:u32@28` (= 0), returned identity `namespace_id:u64@32`, `surface_id:u64@40`, `surface_cap_provenance:u64@48` | 56 |

| Verb | Required authority | Bounded state effect |
|---|---|---|
| `CREATE_SURFACE` | Session channel `USE+SEND+RECV`; returned SurfaceCap is granted on that session | Creates `WORKFIELD_ROOT` or `TEXT`; exactly one full-output root |
| `IMPORT_ATTACH_BUFFER` | SurfaceCap plus transferred BufferCap `USE+MAP_READ+QUERY` | Records one attached buffer provenance token; lifetime/pinning remains Slice 3 |
| `COMMIT` | SurfaceCap provenance matching the surface record | Publishes the pending attachment under a strictly increasing client serial |
| `FOCUS_REQUEST` | SurfaceCap; reason `USER_ACTIVATION` | Calls the compositor's userland policy hook only; no HID route changes in Slice 2 |
| `DESTROY_SURFACE` | SurfaceCap | Marks the record dead and rejects later verbs |

Channel peer death performs the same terminal transition as destroy. Dispatch
uses a fixed eight-slot registry and fixed-size messages (maximum 72 bytes), so
one client can cause only bounded per-message work. It takes no AI-governance
lock and introduces no DMA path. Buffer mapping/lifetime, input delivery, and
present/fence behavior are deliberately unchanged.

## Desktop extension verbs (feature-gated, additive)

Later desktop slices added verbs without changing any v1 record. Each is
compiled only with its libneo feature, has a hand-laid byte oracle in
`tools/atrium-logic-tests`, and is honoured only for the caller's own surface
on that surface's own channel (INV-WM-1).

| Verb (opcode, feature) | Payload after the identity prefix | Total bytes | Bounded state effect |
|---|---|---:|---|
| `YIELD_FOCUS` (7, `atrium-yield`, DS-1a) | `yield_serial:u64@48` (≠ 0) | 56 | The WM takes the keyboard back to Home; the client is held |
| `LAUNCH` (8, `atrium-launch`, DS-1b) | `module_id:u32@48` (≠ 0), `reserved:u32@52` (= 0), `launch_token:u64@56` (≠ 0) | 64 | The session broker judges an allowlisted launch (ADVISORY until OT-66) |
| `COMMIT_DAMAGE` (9, `atrium-damage`, S2-F2 fix-forward) | `client_serial:u64@48`, `x:u16@56`, `y:u16@58`, `width:u16@60`, `height:u16@62` (w, h ≠ 0) | 64 | `COMMIT`, plus the changed rect in surface coordinates; refused (`BAD_FIELD`, nothing committed) unless the rect lies inside the surface as created. A compositor that copies client pixels (the desktop WM) copies and presents only that rect; `COMMIT` still means the whole surface |

### DI-1b resize: the configure mailbox and the `CONFIGURE` ack (`atrium-configure`)

DW-2's `CONFIGURE` (6, `dw2-scene-policy`: `width:u32@48`, `height:u32@52`,
`configure_serial:u64@56`, 64 bytes; size only, no position) is what a desktop
resize tells the client. It cannot be *sent* to the client: IPC is a synchronous
rendezvous with no non-blocking send, so a WM-initiated send parks until the
client receives, and deadlocks against a client parked in its own `COMMIT`
(CP-57). So:

- **Publish.** The WM owns a 4x1 XRGB8888 buffer, the *configure mailbox*, granted
  to the client at session setup with `USE | MAP_READ | QUERY` (read-only; the
  client maps it with the compositor role). Four little-endian `u32` words:
  `seq`, `width`, `height`, `0`. A seqlock: the WM stores `seq = 2·serial − 1`,
  then the size, then `seq = 2·serial`; a reader takes the size only when it read
  the same even, non-zero `seq` before and after. Worked example: serial 2 of a
  320x200 client is the words `[4, 320, 200, 0]`.
- **Ack.** Idle, the client reads the mailbox. On a new serial it lays itself out
  at that size (inside its buffer, which a resizable client creates at the largest
  size it accepts) and sends `CONFIGURE` with the same size and serial on its
  surface channel. The ack means "my buffer now holds this size's picture". The WM
  honours it only if it equals the configure it published (a client cannot size
  itself); otherwise the session ends. It then shows the picture and replies.
- **Damage bound.** With `dw2-scene-policy` compiled in, a `COMMIT_DAMAGE` rect is
  bounded by the size the surface last acknowledged (the created size until
  then), not by the created size.
- **One at a time.** The WM publishes nothing new, and moves nothing, while a
  configure is outstanding.

The client is never told its position: the mailbox holds a size, and pointer
coordinates stay window-relative and clamped to the surface (a press inside the
client while a configure is outstanding is consumed, so no grab outlives the
size it was clamped to). One residual channel is inherent to edge clamping: a
right or bottom drag stopped at the workfield edge yields a size that encodes
the frame's distance from that edge, from which a client that knows the screen
size can infer its origin.

## Compatibility and kill switch

The protocol is compiled only with `atrium-compositor-g1`; the existing runtime
gate remains required. With either layer off, the v1 console route and armed
pixel digest are unchanged. Session namespaces plus per-channel cap provenance
make OT-22 multi-principal separation an additive registry/bootstrap change,
not a wire redesign.

## Verification

Host KATs compare each verb's encoder and decoder independently with hand-laid
bytes, including both role tags, malformed envelopes, cap-provenance failures,
second-root rejection, explicit destroy, and peer-death cleanup. The
`atrium_surface_protocol` QEMU scenario runs the complete sequence across real
IPC channels and transfers a real SurfaceCap plus attenuated Rung-0 BufferCap.

## Slice 3 — cross-process buffer lifetime + the Qubes-C3 relocated-surface proof

Slice 2 records an attach *provenance token* but explicitly defers the buffer's
cross-process lifetime and mapping mechanics. Slice 3 makes the buffer's
guarantees hold across the client→compositor process boundary and proves the
relocated attack surface airtight. **No new kernel syscall is added:** the
cross-process import composes existing primitives — `SYS_GRANT` of a
`Handle::Buffer` over the surface channel, then role-1 `SYS_BUFFER_MAP` (hardware
RO+NX) in the compositor. The client keeps its RW+NX mapping; both point at the
same borrowed frames (the zero-copy dual-PTE — the Qubes-C3 win, no grant-table
negotiation).

**The rights model (pure logic, host-KAT'd in `atrium/logic.rs`).** The
compositor role is a **strict read-only sibling** of the client:
`COMPOSITOR_ROLE = MAP_READ | QUERY | PRESENT` — never `MAP_WRITE`, never
`COMMIT`. The grant attenuation is the byte-wise `source & offered & max_rights`
intersection (`granted_semantic`), which can only *remove* bits: no combination
of a client offer and a recipient ceiling ever re-introduces `MAP_WRITE`
(swept exhaustively over `0..0x1F³`). `is_readonly_compositor_import` pins the
predicate the QEMU proof enforces at the hardware PTE.

**The C3 relocated-surface proof (`atrium_slice3` QEMU scenario).** The client
(neo-worker) paints a known pattern, commits, grants the read-only cap to the
compositor (neo-init), then **exits**. The compositor waits for peer death
(EPIPE) — the happens-before edge — then maps the client's now-orphaned frames
RO+NX and composites from them. Proven, each falsifiable:

- **Lifetime holds across processes (no UAF).** The buffer's frames outlive the
  client process: the compositor reads the client's exact pixels and composites
  them to the framebuffer *after* the client has exited. The Arc pins held by the
  compositor's cap + its borrowed mapping keep the frames live; the client's
  `Pmap::Drop` (PTE removal + TLB shootdown + pin release) frees nothing while a
  mapping remains.
- **Compositor genuinely cannot write (C3 #1).** The compositor's final act is a
  write to its RO import; it **faults** (`#PF`, user-mode write
  protection-violation), verified in the fault handler — not a silent
  corruption.
- **No third principal can map (C3 #2).** There is no ambient buffer namespace: a
  handle that is not a granted buffer cap (a channel handle, an unallocated
  index) cannot be mapped as a buffer, and the granted compositor cap cannot be
  upgraded to a writable mapping (role-0 map → `EPERM`).
- **ABA-safe reuse.** Buffer cap-ids are globally monotonic (never reused), so a
  stale client handle can never alias a later buffer's frames.

**Kill switch.** Cross-process-import-off ≡ the single-process (Slice-1) buffer
behavior bit-for-bit: Slice 3 adds no new syscall and no change to the
single-process path, so the existing kill-switch equality (compositor-off ≡ the
v1 console, and every atrium syscall `ENOSYS` with the runtime switch off)
covers the off-state unchanged.

**I-14 fault containment.** The import/map/teardown path takes **no**
AI-governance lock and issues **no** DMA; a client that never tears down leaks
only its own buffer's frames (reclaimed on the last unmap — the Arc pin bounds
it). Slice 3 introduces **no new DMA path**.

### I-14(c) scanout-DMA seam (implemented by OT-63 Slice 3)

When the GPU scans out directly from a `SharedPixelBuffer` (OT-63 Rung ≥ 1), that
device DMA must route through `dma_arena::fence` (I-14 c1) under IOMMU
containment (I-14 c2). Slice 3 owns the buffer's **frames**, not the scanout DMA.
OT-63 Slice 3 resolves the allocation seam additively: when scanout fencing is
enabled before buffer creation, `SharedPixelBuffer::create` obtains one
contiguous `DmaBuffer::alloc_in_arena` span and retains its existing `frames()`
view for mappings and CPU composition. The flip-arm boundary calls
`dma_arena::fence(phys, page_rounded_len)` and the pending ring carries the
resulting private-constructor `FencedDmaAddr`. A general-allocator buffer or any
other out-of-arena span cannot mint that witness, so the flip is not queued and
the already-composited present completes synchronously via Software degradation.

This is the c1 software-programming guard, not the c2 device guard. Moving the
real Gen9 iGPU from its identity VT-d domain into a translated arena-only domain,
including RMRR/aperture reconciliation, remains Slice 5 bare-metal. Likewise,
the real `PLANE_SURF` byte layout and vblank IRQ remain Slice 4–5; Slice 3 adds no
new wire/register layout and therefore needs no new byte oracle.
