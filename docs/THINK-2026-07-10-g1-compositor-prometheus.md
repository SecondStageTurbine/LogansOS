<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: G1 — the cap-native compositor MVP, concrete buildable design (Prometheus)

**Date:** 2026-07-10
**Author:** Prometheus (Qwen, design lane)
**Source:** `Plans/DISPATCH-2026-07-10-g1-compositor-design-think-prometheus-and-chad.md`
**Branch:** `prometheus/g1-compositor-design-think`
**Status:** DESIGN THINK. Host-only. No code, no QEMU.

---

## 0. Decision in one breath

**G1 is a userland software compositor that composites cap-gated surface buffers onto the kernel framebuffer, using our existing IPC rendezvous channels for control, a new shared-page primitive for zero-copy buffer sharing, and the capability derivation graph for every aspect of surface ownership, input focus, and present fencing.**

The compositor is a single trusted userland process. Clients are untrusted processes that hold attenuated capabilities. The kernel enforces cannot-exceed (Article 6.6 — mechanism only). The compositor decides layout, composition order, and beauty. Kill-switch: `compositor_off` &equiv; the v1 console (`4858ac6`) bit-for-bit.

The architecture is ratified (OT-52, synthesis §4). This THINK makes it **dispatchable**: every verb, every cap mapping, every loop, every seam is specified precisely enough to write the ABI doc and start the smallest-first build.

---

## 1. ISC-1 — The protocol, concrete

### 1.1 Carrier: IPC channels, not new syscalls for control

The compositor-client protocol rides **existing IPC primitives**. Every client process gets a dedicated channel pair at spawn: one end in the compositor, one end in the client. Control messages are `sys_send`/`sys_recv` payloads on this channel. Capabilities (surface handles, buffer caps, focus caps) cross via `sys_grant`/`sys_recv_grant` rendezvous on the same channel.

**Why IPC, not dedicated syscalls:** The kernel stays mechanism-only (Article 6.6). Adding compositor-specific syscalls puts policy logic in ring 0. Our IPC is already capability-gated, synchronous-rendezvous, and cross-pmap-safe. The compositor's control plane is a userland protocol over pipes — exactly the Unix philosophy applied to capabilities.

**What DOES need kernel touch:** One new syscall for shared-page creation (ISC-3). Everything else is userland protocol + existing syscalls.

### 1.2 Message format

Control messages are compact binary records. Each message starts with a 4-byte verb (little-endian u32), followed by verb-specific payload. The wire format is defined in the ABI doc; here is the design:

```
Message {
  verb: u32,          // verb identifier
  seq: u32,           // monotonic sequence number per client (compositor uses for ordering)
  payload: [u8],      // verb-specific, length inferred from channel send
}
```

The rendezvous semantics of `sys_send`/`sys_recv` provide natural backpressure: if the compositor is busy compositing, the client blocks on send. No message queue overflow to manage.

### 1.3 Protocol verbs — the complete table

| Verb | Direction | Payload | Compositor response | Caps required |
|------|-----------|---------|---------------------|---------------|
| `SURFACE_CREATE` (1) | client→comp | `SurfaceCreate { width, height, format, double_buffered: bool }` | `SurfaceCreated { surface_id }` + `sys_grant` of `SurfaceCap` | none (compositor mints) |
| `BUFFER_ATTACH` (2) | client→comp | `BufferAttach { surface_id, buffer_cap_handle, width, height, stride, format }` | `BufferAttached { buffer_id }` | `SurfaceCap.USE` on `surface_id` |
| `BUFFER_DAMAGE` (3) | client→comp | `BufferDamage { buffer_id, rects: [DamageRect] }` where `DamageRect { x, y, w, h }` | ack (implicit in next PRESENT) | `SurfaceCap.USE` on parent surface |
| `PRESENT` (4) | client→comp | `Present { surface_id, fence_handle: Option<u32> }` | `Presented { present_id, timestamp_ticks }` + grants `PresentFenceCap` if fence requested | `SurfaceCap.USE`, active `InputFocusCap` on this surface |
| `FOCUS_REQUEST` (5) | client→comp | `FocusRequest { surface_id, reason: ExplicitUserAction \| AutoBloom }` | `FocusGranted { surface_id }` or `FocusDenied { reason }` + grants/revokes `InputFocusCap` | `SurfaceCap.USE` on `surface_id` |
| `SURFACE_DESTROY` (6) | client→comp | `SurfaceDestroy { surface_id }` | `SurfaceDestroyed` — compositor revokes all derived caps for this surface | `SurfaceCap.REVOKE` on `surface_id` |
| `BUFFER_DETACH` (7) | client→comp | `BufferDetach { buffer_id }` | `BufferDetached` | `SurfaceCap.USE` on parent surface |
| `COMMIT` (8) | client→comp | `Commit { surface_id, position: {x, y}, z_order_hint: i32, visible: bool }` | `Committed` — updates scene graph position/visibility | `SurfaceCap.USE` on `surface_id` |

**Verb flow for a typical frame:**

```
Client renders into its buffer (zero-copy shared page)
  → BUFFER_DAMAGE { rects }     // "I changed these regions"
  → PRESENT { surface_id }      // "composite me now"
  ← Presented { present_id, fence }  // compositor grants fence cap
  // Client holds fence cap; may reuse buffer when fence fires
```

**Verb flow for surface lifecycle:**

```
Client starts
  → SURFACE_CREATE { 800, 600, RGBA8888, true }
  ← SurfaceCreated { surface_id: 1 } + [SurfaceCap granted via sys_grant]
  → sys_map_region(buffer_size)  // client allocates its buffer
  → SHM_ATTACH { surface_id, shm_handle }  // shared page for zero-copy (ISC-3)
  ← ShmAttached
  → COMMIT { surface_id: 1, position: {0,0}, visible: true }
  ← Committed
  // ... render loop ...
  → SURFACE_DESTROY { surface_id: 1 }
  ← SurfaceDestroyed + [all derived caps revoked]
```

### 1.4 Kernel object vs userland compositor state

**Kernel objects (new handle types):**
- `Handle::Shm` — shared-page region, created by new `SYS_SHM_CREATE` syscall (ISC-3). The kernel tracks the physical frame and which Pmaps hold a mapping. This is the ONLY new kernel object G1 requires.
- All five cap kinds (SurfaceCap, BufferCap, InputFocusCap, PresentFenceCap, DecorationCap) are **conventions over existing caps** (ISC-2). No new kernel cap types.

**Userland compositor state:**
- Scene graph: per-surface position, z-order, visibility, attached buffers, damage state
- Surface registry: maps `surface_id` to owning client PID, channel handle, SurfaceCap CapId
- Focus state: which surface currently holds InputFocusCap
- Damage accumulator: union of all damage rects across visible surfaces since last composite
- Frame pacing state: last present timestamp, vsync target (ISC-4)

The kernel knows nothing about surfaces, composition order, or frame pacing. It enforces that a process cannot read/write a shared page it doesn't hold a cap for, and cannot receive input unless the compositor granted it the focus cap. Policy lives in the compositor.

### 1.5 The kernel's one new syscall: `SYS_SHM_CREATE`

Signature: `sys_shm_create(size: u64) -> (handle_idx: u32, map_va: u64)`

Behavior:
1. Allocates `size` bytes worth of physical frames from the buddy allocator.
2. Maps them into the caller's Pmap at `map_va` (R+W+NX, using `Pmap::allocate_user_region` + `Pmap::map_user`).
3. Installs `Handle::Shm(handle_idx)` in the caller's handle table, pointing to the allocated frame list.
4. Returns `(handle_idx, map_va)` packed in i64.

Sharing with another process:
- Grantor calls `sys_grant(channel_h, shm_handle, CapRights { USE })` — this sends the Shm handle to the peer.
- Peer calls `sys_recv_grant(channel_h)` — receives the handle, kernel maps the SAME physical frames into the peer's Pmap at a new VA.
- The kernel tracks reference count on the shared frame list; unmaps when all mappings are dropped.

This is the zero-copy primitive. The client writes pixels into its mapping; the compositor reads from its mapping of the same physical pages. No copy, no dma-buf dance, no FD passing.

**Alternative considered — tensor sharing reuse:** The existing `SYS_TENSOR_SHARE` + `SYS_GRANT` model already does cross-process buffer sharing via the AI arena. However, it's tied to the inference DMA arena and has tensor-specific lifecycle (write-once-read-many semantics for model weights). A compositor needs bidirectional R+W sharing with fine-grained damage tracking. `Handle::Shm` is the general-purpose primitive; tensor sharing remains inference-specific.

---

## 2. ISC-2 — The cap taxonomy made real

### 2.1 Mapping onto I-15

The five cap kinds are **conventions over existing handles**. No new kernel cap types are needed. The convention is: each cap kind is a `Handle::Channel` with a reserved semantic-right encoding that identifies its role, combined with the underlying resource handle (Shm for buffers, Process for input routing).

**Actually — simpler and more robust:** Each cap kind IS the underlying resource handle, with a **specific CapRights subset** that defines what operations are permitted. The "cap kind" is a userland convention: the compositor treats certain handles as SurfaceCaps, BufferCaps, etc., based on how they were created and what rights they carry.

Here's the concrete mapping:

### 2.2 SurfaceCap — a channel handle with surface semantics

**What it is:** The handle the client holds to represent ownership of a surface. It IS the IPC channel between client and compositor for that surface's control messages.

**Kernel representation:** `Handle::Channel` with semantic rights redefined:
- Semantic bit 0 (`SURFACE_USE`) — may send protocol verbs on this channel
- Semantic bit 1 (`SURFACE_REVOKE`) — may destroy the surface

**Derivation rules:**
- **Minted by:** The compositor, in response to `SURFACE_CREATE`. The compositor creates the channel, keeps end A, grants end B to the client via `sys_grant` on the client's main channel.
- **Attenuation:** A client CAN derive a child SurfaceCap with only `SURFACE_USE` (no revoke) — useful for delegating surface rendering to a sub-component without letting it destroy the surface.
- **Revocation:** When the compositor revokes its end of the channel (or the client dies), `Channel::close_peer()` fires `EPIPE` on the other end. All protocol verbs fail. The surface is dead.

**I-15 invariant:** SurfaceCap rights &subseteq; { meta: USE|DERIVE|GRANT|REVOKE, semantic: SURFACE_USE|SURFACE_REVOKE }. A SurfaceCap is always a channel — you can't have a surface without the communication path.

### 2.3 BufferCap — a Shm handle with buffer semantics

**What it is:** The handle to a shared-page buffer that the client renders into and the compositor reads from.

**Kernel representation:** `Handle::Shm` with CapRights:
- Meta USE — may read/write the mapped pages
- Meta GRANT — may re-grant the buffer (for buffer pooling, double-buffering handoff)
- Semantic bit 0 (`BUFFER_WRITE`) — may write pixels (client-side)
- Semantic bit 1 (`BUFFER_READ`) — may read pixels (compositor-side)

**Derivation rules:**
- **Minted by:** The client, via `sys_shm_create()`. The client creates the buffer, then grants it to the compositor with `BUFFER_READ` only (the compositor gets a read-only derivation).
- **Attenuation:** Client derives `BUFFER_READ`-only child → grants to compositor. Compositor can never scribble on the client's buffer. Client retains `BUFFER_WRITE`.
- **Double-buffering:** Client creates two Shm regions, holds WRITE caps for both. Attaches one as "front" (compositor has READ). After present, swaps: detaches old front, attaches new front. Compositor's READ cap follows the attach/detach protocol.
- **Revocation:** Revoking the client's WRITE cap mid-frame is safe — the compositor's READ cap on the same physical pages remains valid (derivation graph: compositor's READ is a child of the original Shm, revoked WRITE is a sibling derivation). The compositor finishes reading the frame it already has.

**I-15 invariant:** A BufferCap's rights &subseteq; { meta: USE|GRANT, semantic: BUFFER_WRITE | BUFFER_READ }. Client holds WRITE, compositor holds READ. They are sibling derivations from the same Shm root — neither can revoke the other (REVOKE requires the root handle).

### 2.4 InputFocusCap — a process handle with input semantics

**What it is:** The capability that authorizes a process to receive input events for the focused surface.

**Kernel representation:** `Handle::Process` pointing to the client PID, with semantic rights:
- Semantic bit 0 (`INPUT_RECV`) — may receive input events from the compositor

**Derivation rules:**
- **Minted by:** The compositor, in response to `FOCUS_REQUEST`. The compositor already holds a `Handle::Process` for every spawned client (from `sys_spawn`). It derives an attenuated child with only `INPUT_RECV` and grants it via `sys_grant`.
- **Focus transfer:** When focus moves from surface A (client X) to surface B (client Y):
  1. Compositor revokes client X's InputFocusCap (walks derivation graph, removes all INPUT_RECV caps for X)
  2. Compositor derives new InputFocusCap for client Y, grants via `sys_grant`
  3. Between steps 1 and 2, NO process holds INPUT_RECV — input events are silently dropped. This is correct: focus transfer is atomic from the input system's view.
- **Revocation mid-frame:** Revoking INPUT_RECV while a key event is in flight is harmless — the compositor's `sys_send` on the client's channel returns `EPIPE` if the client dropped the channel, or succeeds and the client ignores it if it doesn't expect input (the protocol defines which surfaces consume input).

**I-15 invariant:** InputFocusCap rights &subseteq; { meta: USE, semantic: INPUT_RECV }. Only the focused surface's owner holds this cap. The compositor enforces single-focus: at most one process holds INPUT_RECV at any time.

### 2.5 PresentFenceCap — a channel handle with fence semantics

**What it is:** A one-shot capability that fires when the compositor has finished reading the presented buffer, signaling the client may reuse it.

**Kernel representation:** `Handle::Channel` (single-message fence channel). The compositor creates it, grants to client on `PRESENT`. When the compositor finishes compositing this frame, it sends a single byte on the fence channel. The client's `sys_recv` unblocks.

**Derivation rules:**
- **Minted by:** The compositor, in response to `PRESENT` when the client requested a fence.
- **One-shot:** After the compositor sends the fire signal, both ends drop the channel. The cap is consumed.
- **Buffer reuse rule:** A client MAY reuse a buffer ONLY after receiving on its PresentFenceCap for that buffer's present. This is enforced by protocol discipline (userland), not kernel — but the kernel enforces that the compositor cannot forge a fence cap for a buffer it doesn't hold READ for.

**I-15 invariant:** PresentFenceCap rights &subseteq; { meta: USE, semantic: 0 }. It's a channel that carries exactly one message. After recv, it's closed.

### 2.6 DecorationCap — compositor-owned chrome handle

**What it is:** Authorizes drawing compositor-owned chrome (borders, title bars, Cap Halos) around a surface.

**Kernel representation:** NOT granted to clients. This is a **compositor-internal convention**: the compositor tracks which surfaces have decorations enabled and renders chrome during composition. No kernel cap needed — decorations are composited by the compositor, not drawn by the client.

**Absent-by-default:** CaptureCap is similarly absent. Screen capture requires explicit per-surface consent (the surface owner grants a temporary READ derivation of its BufferCap to the capture service). No ambient capture capability exists.

### 2.7 Cap taxonomy summary table

| Cap kind | Kernel handle type | Who mints | Client rights | Compositor rights | Revocation effect |
|----------|-------------------|-----------|---------------|-------------------|-------------------|
| SurfaceCap | Channel | Compositor | SURFACE_USE, optional SURFACE_REVOKE | Peer channel end | Surface dead, all derived caps go |
| BufferCap | Shm | Client | BUFFER_WRITE (own buffer) | BUFFER_READ (granted derivation) | Writer stops; reader finishes current frame |
| InputFocusCap | Process | Compositor | INPUT_RECV | Handle to client PID | Input drops for that client |
| PresentFenceCap | Channel (one-shot) | Compositor | RECV (unblocks on fire) | SEND (fires when done) | Consumed after fire |
| DecorationCap | N/A (compositor internal) | Compositor | — | — | N/A |
| CaptureCap | — | — | — | — | **Absent by design** |

---

## 3. ISC-3 — Buffers, zero-copy, damage, composition loop

### 3.1 Buffer allocation path

```
Client:
  1. shm_handle = sys_shm_create(width * height * 4)  // RGBA8888
     // Kernel allocates physical frames, maps into client Pmap, returns handle + VA
  2. buffer_ptr = shm_va  // client has a pointer it can render into
  3. Derive read-only: read_cap = sys_derive(shm_handle, CapRights { USE, BUFFER_READ })
  4. Grant to compositor: sys_grant(compositor_channel, read_cap, CapRights { USE, BUFFER_READ })
     Compositor: sys_recv_grant() → receives read_cap, kernel maps SAME physical frames
  5. BUFFER_ATTACH { surface_id, buffer_cap: read_cap_handle, ... }
     // Compositor now has read access to the same pixels
```

The compositor holds a READ derivation. The client holds the root (WRITE). They map the same physical pages. Zero copy.

### 3.2 Memory safety — client scribble corrupts only its own pixels

**The invariant:** A client can only write into Shm regions it created (or was granted WRITE for). The compositor only holds READ derivations. Therefore:
- Client X writing its buffer can only corrupt pixels in X's buffer.
- The compositor reading buffers during composite can only READ — it cannot scribble.
- If client X crashes while writing, the worst case is corrupted pixels in X's own buffer region. The compositor's next composite reads garbage from X's buffer but composes it harmlessly into the frame. Other surfaces are unaffected.

**Double-buffering discipline:** The client owns two buffers (front/back). Only the front buffer's READ cap is held by the compositor. The client renders into back (compositor can't see it). On present, the client swaps: tells compositor to read from back (now front), starts rendering into old front (now back). The compositor never reads a buffer the client is actively writing.

**Formal property:** `∀ client C, ∀ buffer B: C holds WRITE(B) ∧ compositor holds READ(B') where B' is derived from B ∧ WRITE ∩ READ = ∅ (sibling derivations, not parent-child) ⇒ C's writes cannot invalidate compositor's concurrent reads on the same physical page IF double-buffering discipline is observed.`

### 3.3 Damage tracking

Per-surface, the client reports damage rects via `BUFFER_DAMAGE`:

```
BufferDamage {
  buffer_id: u32,
  rects: [DamageRect; MAX_RECTS],   // MAX_RECTS = 16
  rect_count: u8,
}
DamageRect { x: u16, y: u16, w: u16, h: u16 }
```

The compositor maintains a **global damage accumulator** — the union of all damage rects across all visible surfaces, transformed by each surface's position. Before compositing, the compositor clips to the accumulated damage region.

**Damage rect coalescing:** Adjacent rects are merged (standard rectangle union). If the accumulated damage exceeds 60% of the framebuffer, the compositor does a full repaint (avoids partial-composite overhead for large updates).

**G1 slice-1 simplification:** First slice does FULL repaint every frame. Damage tracking is added in slice-2. The rationale: proving one surface composited correctly is the hard part; optimizing for N surfaces with incremental redraw is a separate concern.

### 3.4 The software composition loop

```
Compositor main loop:
  tick_start = now()
  
  // Phase 1: Collect presents (non-blocking recv on all client channels)
  for each client_channel:
    while msg = try_recv(client_channel):
      dispatch(msg)  // handle PRESENT, DAMAGE, COMMIT, etc.
  
  // Phase 2: Composite
  if there are pending presents OR first_frame:
    fb_guard = acquire_framebuffer_lock()  // existing fb::Mutex
    
    if damage_mode:
      for each damage_rect in accumulated_damage:
        composite_region(damage_rect)
    else:
      composite_full()
    
    release_framebuffer_lock(fb_guard)
    
  // Phase 3: Fire fences
  for each fired_fence:
    send_on_channel(fence.channel, [0x01])  // one-shot fire
    drop(fence)  // channel closes
  
  // Phase 4: Pace to vsync target (ISC-4)
  elapsed = now() - tick_start
  sleep_until(vsync_target - elapsed)
  
  clear_damage_accumulator()
```

**`composite_region(rect)`** — for each visible surface (back-to-front z-order):
1. Clip surface to damage rect
2. If clipped region is empty, skip this surface
3. Blit surface buffer → framebuffer region using `fb::blit_rgba()` (existing primitive, alpha-blended)
4. Render DecorationCap chrome (Cap Halo borders, etc.) around the surface's visible edges

**Composition is a blit.** No GPU, no shader, no Mesa. The compositor reads RGBA8888 from the client's shared buffer and alpha-blends it into the framebuffer at the surface's position. `fb::blit_rgba()` already does this (per-pixel alpha composite with the existing palette).

**Lock contention:** The framebuffer lock (`fb::Mutex`) is held only during the blit phase. Message collection, fence firing, and pacing are lock-free. The lock is the same `spin::Mutex<Option<FbState>>` that guards all framebuffer access — G1 doesn't change its concurrency model.

---

## 4. ISC-4 — Present pacing + honest liveness

### 4.1 Vsync without display hardware timing

**The problem:** On bare metal, vsync is the display controller's vertical blank interval. QEMU doesn't expose this. We need frame pacing that works on both.

**Solution — software vsync with measured floor:**

```
VsyncTarget {
  refresh_hz: u32,           // configured: 60 (default), 120 (if known)
  frame_budget_ms: u32,      // 1000 / refresh_hz (16.67ms @ 60Hz, 8.33ms @ 120Hz)
  measured_composite_ms: f64, // moving average of actual composite time
  jitter_margin_ms: f64,     // 10% of frame_budget
}
```

The compositor measures its own composite time (Phase 2 above). If `measured_composite + jitter_margin > frame_budget`, it drops the target to the next lower refresh rate. This prevents frame piling: the compositor never starts a frame it can't finish before the next vsync.

**Bare metal (future):** When the display controller exposes vblank IRQs (Phase-6 GPU lane), the compositor switches to hardware vsync. The software pacing is the floor that works everywhere.

### 4.2 PresentFenceCap semantics — when may a client reuse a buffer?

A client may reuse a buffer **if and only if** it has received on the PresentFenceCap for that buffer's most recent present. The protocol enforces this:

```
Client presents buffer A with fence request:
  → PRESENT { surface_id, fence: true }
  ← Presented { present_id, fence_channel_granted }
  
  // Client renders next frame into buffer B (double-buffer)
  // Client MUST NOT touch buffer A until fence fires
  
  recv(fence_channel)  // blocks until compositor fires
  // NOW client may reuse buffer A
```

The compositor fires the fence **after** the blit of that buffer completes (Phase 3 above). The client's recv unblocks. Double-buffering means the client is always rendering into the non-presented buffer.

### 4.3 Honest liveness — present events are measured, never theater

The compositor exposes a **liveness channel** — a special read-only surface that the Atrium's Liveness Thread reads. Present events are real timestamps:

```
LivenessEvent {
  kind: PresentComplete | CompositeStart | CompositeEnd | FrameDropped,
  timestamp_ticks: u64,    // real tick count, never synthetic
  duration_ms: u32,        // for CompositeStart→End pairs
  surface_id: Option<u32>, // which surface, if applicable
}
```

These events are written to a ring buffer that the Liveness Thread surface reads. No fake frame callbacks. If the compositor missed a frame (composite took longer than vsync budget), it emits `FrameDropped` with the actual duration — the Liveness Thread shows the red segment, not a green lie.

**Tie to the measured Liveness Thread:** The v1 console already brackets `sys_infer_run` with real "forward pass running" / "complete tokens=N" (OT-26). G1 extends this: composite start/end are real measured events. The Liveness Thread's PresentComplete segment lights only when a frame is actually blitted to the framebuffer.

---

## 5. ISC-5 — Input routing = the cap check

### 5.1 The unified input-security and input-routing code path

The ratified line: **focus IS a capability check.** Input events route to the focused surface's owner through the same mechanism that enforces "you can't receive input unless you have the focus cap."

**Current seed:** The USB-HID keyboard driver (`hid_keyboard.rs`) deposits decoded bytes into a pending queue. `try_read_byte()` drains completions and returns the next byte. Currently, fd=0 reads go to whoever calls it (neo-shell or Arthur).

**G1 input path:**

```
USB-HID ISR → completes report → decoder → bytes deposited in pending queue
  ↓
Compositor's input_poll() loop:
  byte = hid_keyboard::try_read_byte()?
  
  // The cap check: who holds InputFocusCap?
  focused_client = self.focus_registry.get_focused()  // PID + channel handle
  
  if focused_client.is_some():
    // Route the event — this IS the cap check:
    // only the process with InputFocusCap has a channel we'll send to
    sys_send(focused_client.channel, input_event_payload)
  else:
    // No focused surface — event goes to neo> (deterministic rail)
    deposit_to_neo_prompt(byte)
```

**Why X11's global-snooping is impossible by construction:**
- Input events are `sys_send` on a specific channel. Only the two endpoints of that channel can read the data.
- The compositor decides which channel to send to based on focus. A client cannot intercept another client's input because it doesn't hold that channel.
- There is no "listen to all keypresses" primitive. The kernel doesn't offer a global input FD. Input flows from HID → compositor → focused channel. End of story.
- Even if the compositor is compromised, it can only route input — it cannot forge a cap check. The kernel enforces that `sys_send` on a channel only delivers to that channel's peer.

### 5.2 Focus transfer protocol

```
User clicks/-tabs to surface B (owned by client Y):
  → Compositor receives focus-change event (from pointer driver or keyboard Tab)
  
  Compositor:
    1. Old focused = client X with surface A
    2. Revoke X's InputFocusCap:
       sys_revoke(X's_input_focus_cap_root)
       // Walks derivation graph, removes all INPUT_RECV for X
    3. Gap: no process holds INPUT_RECV (input silently drops during transfer)
    4. Derive new InputFocusCap for Y:
       new_cap = derive(Y_process_handle, CapRights { USE, INPUT_RECV })
    5. Grant to Y:
       sys_grant(Y_channel, new_cap, CapRights { USE, INPUT_RECV })
       Y: sys_recv_grant() → receives InputFocusCap
    6. Focus is now on Y's surface B
```

**Focus request from client:** A client can also request focus via `FOCUS_REQUEST` verb. The compositor grants it if the surface is visible and not occluded. This is how a Workfield blooms and grabs focus.

### 5.3 Input event payload

```
InputEvent {
  kind: KeyDown | KeyUp | Char,
  keycode: u16,       // HID usage code
  modifiers: u8,      // shift=1, ctrl=2, alt=4, cmd=8
  timestamp_ticks: u64,
}
```

Sent as a `sys_send` payload on the focused client's channel. The client's protocol handler dispatches it to the focused surface. If the client doesn't handle the key (e.g., passes it through), it sends it back to the compositor, which may route it to `neo>` or the next surface.

---

## 6. ISC-6 — The seams, honest

### 6(a). OT-22 multi-principal: G1 ships single-principal, protocol is multi-principal-ready

**What changes when OT-22 opens:**

1. **Per-principal surface namespaces:** Each principal gets a surface namespace prefix. `surface_id` becomes `(principal_id, local_id)`. The compositor's surface registry is already a hash map — adding a principal key is a one-line change.

2. **Cap provenance on input events:** When principal A's surface receives input routed through principal B's compositor, the input event carries the originating principal's CapId. The receiver can verify: "this input came from the owner's keyboard, not from another principal's automation." Today (single-principal), all input is from the owner. OT-22 adds provenance.

3. **Cross-principal surface visibility:** A surface's `COMMIT` gains a `visible_to: [PrincipalId]` field. Today it's implicit (all surfaces visible to the one principal). OT-22 makes it explicit.

4. **Compositor per-principal or shared?** The compositor is a single process today. OT-22 can either: (a) run one compositor per principal (simplest, strongest isolation), or (b) run one compositor with per-principal surface namespaces (more complex, shares frame pacing). **Recommendation: one compositor, per-principal namespaces.** The cap system already isolates — principal A can't hold caps for principal B's surfaces.

**Nothing needs redesign.** The protocol verbs carry surface_id (which becomes namespaced), caps enforce isolation, and the composition loop iterates visible surfaces regardless of principal.

### 6(b). G0 → G1 migration: the Workfield becomes a composited surface

Chad's G0 spec names its regions like future surfaces: Intent surface, Folder surface, Candidate surface, Action surface, Grant surface, Undo Shadow, Command Graph surface, Liveness Thread.

**Migration path:**
1. G0 draws these regions on the raw framebuffer using `fb::draw_string()`, `fb::fill_rect()`, `fb::blit_rgba()`.
2. G1 wraps each G0 region into a surface buffer:
   - G0's render function writes into a Shm buffer instead of the framebuffer directly
   - The compositor composites all G0 surfaces onto the framebuffer
   - The G0 code changes: replace `fb::draw_*` calls with `buffer_draw_*` calls (same API, different target)
3. The G0 kill-switch (`atrium_g0=off`) becomes the G1 kill-switch (`compositor_off=off`): both revert to the v1 console bit-for-bit.

**Minimal rework:** G0's layout model (bands on 8px grid, full-width regions) maps directly to compositor surface positions. The `COMMIT` verb sets each surface's `(x, y)` and `visible` flag. G0's "draw the Workfield" becomes "composite the Workfield's surfaces."

### 6(c). The text engine: T0 text surface is a client buffer

Prometheus's text-engine THINK (T0) specifies FreeType CPU/hinted type blitted via `blit_rgba()`. In G1:

- The text renderer is a userland process (or a library linked into the compositor)
- It renders glyphs into a Shm buffer (the text surface's buffer)
- The compositor composites the text surface like any other surface
- Zero-copy: the text renderer writes directly into the Shm; the compositor reads it

**Clean fit.** The text engine produces RGBA8888 pixels in a shared buffer. The compositor doesn't care that they're text — it's just another surface to composite. The T0/T1/T2/T3 progression is transparent to the compositor.

### 6(d). I-14 fault containment enumeration (required)

The compositor is userland. Here's what a compositor fault can and cannot touch:

| Fault | Containment | Proof |
|-------|-------------|-------|
| **Compositor hangs (infinite loop)** | Compositor holds no AI-governance lock. The framebuffer lock is a spinlock held only during blit (microseconds). If the compositor hangs in its message loop, it never acquires the fb lock → other fb writers (console, boot diagnostics) can still write. Arthur's inference path is unaffected. | Compositor's fb lock acquisition is bounded to Phase 2 of the loop (the blit). Message collection, fence firing, pacing are lock-free. |
| **Compositor crashes (segfault)** | Userland crash → kernel reaps process → drops all handles → `Channel::close_peer()` fires EPIPE on all client channels → clients detect compositor death → fall back to direct framebuffer writes (G0 mode) or exit. No kernel panic. | Process exit path drains handle table, closes peer channels, drops Pmap. Standard userland crash containment. |
| **Runaway client (infinite render loop)** | Client writes only its own Shm buffer (memory safety, ISC-3). Compositor reads the buffer during composite — if the client is still writing, the compositor might read torn pixels, but this is visually harmless (garbage in that surface's region). No kernel impact. | BufferCap isolation: client WRITE and compositor READ are sibling derivations on the same Shm. Client can't write outside its buffer. |
| **Client crashes mid-frame** | Kernel reaps client → closes channels → compositor gets EPIPE → removes surface from scene graph → next composite skips dead surface. The framebuffer is not corrupted — the dead surface's region shows whatever was last composited (or the background clear). | Channel close-peer on process exit. Compositor's message loop handles EPIPE by removing the surface. |
| **Framebuffer write path** | The compositor writes the framebuffer via `fb::blit_rgba()` under the existing spinlock. This is the same path G0 and the console use. No new framebuffer access pattern. No governance lock on any composite path. | Uses existing `fb::Mutex`. Same concurrency model as today. |
| **AI governance** | The compositor holds no AI-governance locks. Inference syscalls (`SYS_INFER_RUN`, etc.) are independent. The compositor's frame deadline is governed by the same QoS machinery (OT-15/Phase-7) that governs inference — but they are independent deadlines, not locked together. | Compositor doesn't call inference syscalls. Arthur calls inference syscalls. They share the CPU-QoS substrate but don't hold each other's locks. |

**I-14 summary:** The compositor is a userland process that blits pixels. It cannot stall governance, corrupt memory outside its caps, or panic the kernel. A compositor fault degrades to "no composition" — surfaces stop updating, but the system runs. Kill-switch off &equiv; no compositor, raw console.

---

## 7. ISC-7 — Kill-switch + build decomposition + forks

### 7.1 Kill-switch: compositor-off &equiv; raw console bit-for-bit

**Mechanism:** Feature flag `compositor`. When off:
- The compositor process is not spawned at boot
- All framebuffer writes go through the existing console path (`fb::write_bytes()`, scrolling text console)
- G0, if built, also respects `atrium_g0=off` and falls through to the v1 console
- The framebuffer module is unchanged: same `init()`, same primitives, same lock

**Proof shape:**
```
compositor_off && atrium_g0_off == Janus v1 console (4858ac6) bit-for-bit:
  - Boot sequence identical (boot_frame.rs → console init → Arthur greeting)
  - Framebuffer writes: only console.rs write_bytes path active
  - No compositor process exists → no channel creation, no Shm allocation
  - Input: HID keyboard → fd=0 → neo>/Arthur (current path, untouched)
  - No new syscalls active (SYS_SHM_CREATE exists but is never called)
```

The kill-switch is a spawn-guard: if the flag is off, the compositor binary is never `sys_spawn`ed. Everything else is the current system.

### 7.2 Smallest-first build decomposition

**Slice 1 — One surface, one client, composite-to-fb (the proof slice):**
- Kernel: `SYS_SHM_CREATE` syscall + `Handle::Shm` variant + grant/receive maps shared frames into peer Pmap
- Userland: Minimal compositor process that:
  - Spawns one trivial client
  - Client creates a Shm buffer, fills it with a solid color
  - Compositor reads the buffer, blits to framebuffer via `fb::blit_rgba()`
- Gate: QEMU scenario — boot, compositor runs, colored rectangle appears on framebuffer, init continues
- **This is the smallest thing that proves zero-copy composition works.**

**Slice 2 — Protocol verbs + scene graph:**
- Compositor implements SURFACE_CREATE, BUFFER_ATTACH, COMMIT, PRESENT, SURFACE_DESTROY
- Client can create/position/destroy surfaces
- Scene graph: hash map of surface_id → position, z-order, buffer, visible
- Full repaint every frame (no damage tracking yet)
- Gate: Two surfaces composited at different positions, correct z-order

**Slice 3 — Damage tracking + double-buffering:**
- BUFFER_DAMAGE verb, damage accumulator, coalescing
- Client double-buffers: two Shm regions, present swaps front/back
- PresentFenceCap: one-shot fence channel, fires after blit
- Gate: Client animates a moving rectangle; compositor only redraws damage region (verify with composite-time measurement)

**Slice 4 — Input focus = cap check:**
- InputFocusCap: derive/grant/revoke on process handle
- Compositor routes HID keyboard input to focused client's channel
- FOCUS_REQUEST verb, focus transfer protocol
- Gate: Two clients, Tab switches focus, keys only arrive at focused client

**Slice 5 — Frame pacing + liveness:**
- Software vsync with measured composite time
- PresentFenceCap timing: client reuses buffer only after fence fires
- Liveness event ring buffer: PresentComplete, CompositeStart/End, FrameDropped
- Gate: Compositor maintains steady frame rate under load; liveness events match measured composite time

**Slice 6 — G0 Workfield as composited surface:**
- G0's "clean a folder" Workfield renders into Shm surfaces
- Each G0 region (Intent, Candidate, Action, Grant, Undo, Liveness) is a separate surface
- Compositor composites the Workfield layout
- Gate: G0 Workfield works identically under compositor as on raw framebuffer

**Slice 7 — Kill-switch + I-14 verification:**
- `compositor_off` flag: proves bit-for-bit equality with v1 console
- Fault injection: kill compositor → clients detect EPIPE → graceful degradation
- Gate: Close-gate scenarios for each fault mode (ISC-6d)

### 7.3 Honest forks + recommendations

| Fork | Options | Recommendation | Reason |
|------|---------|----------------|--------|
| **Carrier: syscalls vs IPC** | Dedicated compositor syscalls vs IPC channels for control | **IPC channels** | Kernel stays mechanism-only. IPC is already cap-gated, rendezvous, cross-pmap-safe. Only `SYS_SHM_CREATE` needs kernel touch. |
| **Shared memory: new Shm handle vs tensor reuse** | `Handle::Shm` (new) vs repurpose `Handle::Tensor` sharing | **New `Handle::Shm`** | Tensor sharing is inference-specific (DMA arena, write-once semantics). Compositor needs general-purpose R+W shared pages with fine-grained lifecycle. |
| **Scene graph granularity** | Per-pixel retained graph vs per-surface positions | **Per-surface positions** | G1 is an MVP. The compositor tracks surface_id → (x, y, z, visible, buffer). No internal surface geometry. That's G3+. |
| **Damage tracking: slice-1 or later?** | Damage rects from slice 1 vs full-repaint slice 1, damage in slice 3 | **Full-repaint slice 1** | Proving one surface composited correctly is the hard part. Damage optimization is orthogonal. Add it when N > 1 surface. |
| **Buffer format** | RGBA8888 (32-bpp) vs RGB565 (16-bpp) vs XRGB8888 | **RGBA8888** | Matches framebuffer format (32-bpp BGRA, existing `blit_rgba()` does alpha composite). No conversion needed. 16-bpp saves memory but adds conversion cost. |
| **Input event encoding** | Binary struct vs HID-pass-through | **Binary struct with HID usage codes** | Abstracts the input device. The compositor can later add pointer/touch without changing the wire format. HID pass-through locks us to keyboard. |
| **Compositor process model** | Single compositor process vs per-principal compositor | **Single compositor** (OT-22 adds namespaces) | Simpler for G1. OT-22's per-principal isolation is cap-enforced, not process-enforced. One compositor with namespaced surfaces is sufficient. |

---

## 8. Protocol verb table (complete reference)

```
VERB_SURFACE_CREATE    = 1   // client→comp: create a surface
VERB_SURFACE_CREATED   = 10  // comp→client: surface created, SurfaceCap granted
VERB_BUFFER_ATTACH     = 2   // client→comp: attach a buffer to a surface
VERB_BUFFER_ATTACHED   = 20  // comp→client: buffer attached
VERB_BUFFER_DAMAGE     = 3   // client→comp: report damage rects
VERB_PRESENT           = 4   // client→comp: present current buffer
VERB_PRESENTED         = 40  // comp→client: present complete, fence granted
VERB_FOCUS_REQUEST     = 5   // client→comp: request input focus
VERB_FOCUS_GRANTED     = 50  // comp→client: focus granted, InputFocusCap sent
VERB_FOCUS_DENIED      = 51  // comp→client: focus denied
VERB_SURFACE_DESTROY   = 6   // client→comp: destroy a surface
VERB_SURFACE_DESTROYED = 60  // comp→client: surface destroyed, caps revoked
VERB_BUFFER_DETACH     = 7   // client→comp: detach a buffer
VERB_BUFFER_DETACHED   = 70  // comp→client: buffer detached
VERB_COMMIT            = 8   // client→comp: update surface position/visibility
VERB_COMMITTED         = 80  // comp→client: commit acknowledged

// Compositor-initiated (sent on client's SurfaceCap channel):
VERB_FOCUS_LOST        = 90  // comp→client: you lost focus (InputFocusCap revoked)
VERB_INPUT_EVENT       = 91  // comp→client: input event for your focused surface
VERB_FRAME_TARGET      = 92  // comp→client: current vsync target (for client pacing)
```

---

## 9. Cap taxonomy mapping (complete reference)

```
// CapRights encoding for compositor caps (semantic byte):
// SurfaceCap (Channel handle):
SURFACE_USE    = 1 << 0   // may send protocol verbs
SURFACE_REVOKE = 1 << 1   // may destroy the surface

// BufferCap (Shm handle):
BUFFER_WRITE   = 1 << 0   // may write pixels
BUFFER_READ    = 1 << 1   // may read pixels

// InputFocusCap (Process handle):
INPUT_RECV     = 1 << 0   // may receive input events

// PresentFenceCap (Channel handle, one-shot):
// No semantic rights — it's a channel that carries one message

// Meta rights (standard I-15):
META_USE    = 1 << 0
META_DERIVE = 1 << 1
META_GRANT  = 1 << 2
META_REVOKE = 1 << 3
```

---

## 10. Sources read

**Ratified design documents:**
- `docs/THINK-2026-07-08-gui-vision-SYNTHESIS.md` — ratified paradigm, §4 compositor decision, §5 build spine G0-G7
- `docs/OPEN-THREADS.md` — OT-52 (compositor architecture, cap taxonomy, ratified), OT-22 (multi-principal frontier), OT-18 (grant ergonomics), OT-19 (reversibility), OT-26 (agentic actions)
- `docs/THINK-2026-07-10-atrium-g0-buildable-spec.md` (Chad) — G0 spec, cap-surface table, framebuffer constraints, kill-switch
- `docs/THINK-2026-07-10-atrium-design-language.md` (Cerebro) — design language, Cap Halo materials, motion rules
- `docs/THINK-2026-07-10-atrium-text-engine-research.md` (Prometheus) — T0 text engine, cap-surface buffer fit

**Kernel implementation seams:**
- `kernel/src/drivers/fb/mod.rs` — framebuffer primitives: `blit_rgba()`, `fill_rect()`, `draw_glyph()`, `draw_string()`, spinlock guard, 32-bpp BGRA
- `kernel/src/drivers/fb/console.rs` — current console sink, `write_bytes()`, cursor, scroll
- `kernel/src/ipc/mod.rs` — rendezvous channels, `sys_chan_create/send/recv/grant/recv_grant`, `cross_pmap_copy()`, `GrantPayload`
- `kernel/src/sched/process.rs` — `Handle` enum (8 variants), `HandleTable`, `CapId`, `CapRights`, `derive_handle()`, `revoke_handle()`, `complete_grant()`, `PROCESSES` registry, spawn with handle inheritance
- `kernel/src/sched/file_cap_logic.rs` — file cap mapping into CapRights, I-15 unification
- `kernel/src/arch/x86_64/syscall.rs` — syscall table, dispatch, 24-0xFF available
- `kernel/src/mm/pmap.rs` — per-process page tables, `map_user()`, `translate()`, `allocate_user_region()`, W^X enforcement
- `kernel/src/mm/mod.rs` — buddy allocator
- `kernel/src/drivers/usb/hid_keyboard.rs` — HID keyboard driver, `try_read_byte()`, report decoding, pending queue
- `kernel/src/drivers/dma_arena.rs` — DMA arena, `FencedDmaAddr`

**External prior art (firsthand, OT-1 discipline):**
- Wayland protocol — compositor-owns-final-composition, client-allocated buffers, per-surface input focus, explicit damage, presentation feedback (lifted as primitives, not imported as stack)
- X11 — global input snooping as the anti-pattern (our caps forbid it by construction)

---

## 11. Close-gate

**Docs-only branch.** No code, no QEMU. `scripts/worker-close-gate.sh` host set locally — skip-with-note on docs-only branch (no source files touched, no CI legs apply).

**RETURN:**
- THINK path: `docs/THINK-2026-07-10-g1-compositor-prometheus.md`
- ISC-1: Protocol — IPC channels as carrier, 18 verbs defined with payloads/directions/caps required, kernel object (Handle::Shm) vs userland state (scene graph, surface registry, focus state, damage accumulator, frame pacing)
- ISC-2: Cap taxonomy — all five caps mapped onto existing handles (Channel, Shm, Process) with specific CapRights subsets; derivation/attenuation/revocation rules per kind; I-15 invariants stated; CaptureCap absent-by-default, DecorationCap compositor-internal
- ISC-3: Buffers — `SYS_SHM_CREATE` syscall spec, zero-copy via shared physical frames, client WRITE / compositor READ as sibling derivations, double-buffering discipline, damage tracking (deferred to slice-3), software composition loop using `fb::blit_rgba()`, memory-safety property formalized
- ISC-4: Present pacing — software vsync with measured composite time, adaptive frame rate, PresentFenceCap one-shot channel semantics, honest liveness events (measured timestamps, no fake callbacks)
- ISC-5: Input routing — focus IS the cap check, one code path; HID → compositor → focused client's channel; X11 snooping impossible by construction; focus transfer protocol with atomic revoke-then-grant
- ISC-6: Seams — OT-22 multi-principal-ready (namespaced surface_ids, cap provenance on input, per-principal visibility; no redesign needed); G0→G1 migration (G0 regions become Shm surfaces, minimal render-target swap); T0 text engine fits as client buffer; I-14 fault containment enumerated (6 fault modes, all contained)
- ISC-7: Kill-switch — spawn-guard, bit-for-bit equality proof shape; 7-slice build decomposition (one-surface proof → protocol → damage → input → pacing → G0 integration → kill-switch/I-14); 7 forks with recommendations
- Protocol verb table: 18 verbs with directions and semantics
- Cap taxonomy mapping: complete CapRights encoding reference

**Diff verified:** RETURN matches the committed artifact at `docs/THINK-2026-07-10-g1-compositor-prometheus.md`.

*Design THINK. No code. Host-only. Independent — did not read Chad's draft.*
