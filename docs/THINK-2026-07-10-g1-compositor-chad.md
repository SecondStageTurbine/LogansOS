<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: G1 - the cap-native compositor MVP, made buildable (Chad)

**Date:** 2026-07-10
**Author:** Chad (Codex)
**Source:** `Plans/DISPATCH-2026-07-10-g1-compositor-design-think-prometheus-and-chad.md`
**Branch:** `codex/g1-compositor-design-think-chad`
**Status:** Independent design THINK. Host-only. No code, no QEMU.

---

## 0. Decision in one breath

Build G1 as a **userland compositor over the existing synchronous IPC and
capability graph**, with surface policy entirely in ring 3 and four small kernel
mechanisms underneath it:

1. `BufferCap`: a new shared-page handle kind. The client maps it read/write;
   the compositor receives an attenuated read-only child through `sys_grant`.
2. `InputFocusCap`: a new input-route handle kind. The compositor chooses which
   object is active; the kernel's one input path permits only that cap's holder
   to receive HID/serial input.
3. `PresentFenceCap`: a new one-shot fence handle kind. The kernel completes it
   only after the bounded framebuffer copy is finished, or terminates it with an
   explicit discard/fault outcome.
4. `FramebufferOutputCap`: a supporting device cap, held only by the compositor,
   which gates a clipped, stride-aware copy into the current Limine framebuffer.

`SurfaceCap` should **not** be a new kernel object. It is a dedicated IPC channel
endpoint whose peer is bound to exactly one userland `Surface` record. The
endpoint is already an unforgeable, derivable, grantable, revocable kernel cap;
making a second kernel `Surface` object would either be enforcement theater or
pull scene/layout policy into the kernel. `DecorationCap` is a compositor-owned
surface role over the same mechanism. `CaptureCap` has no constructor, protocol
verb, or inherited default in G1.

The first visible G1 is one client, one full-screen Workfield surface, one or two
client buffers, buffer-coordinate damage, immediate software pacing with at
most one executing and one pending present, and truthful completion feedback.
There is **no claim of vsync** on either QEMU or current bare metal: the kernel
can prove that framebuffer stores completed, not that photons changed at a
vertical retrace.

This lifts the useful Wayland primitives - pending state plus atomic commit,
client-owned buffers, damage in buffer coordinates, release only after the
compositor stops reading, and explicit presented/discarded outcomes - without
lifting Wayland's object graph, wire ABI, fd-passing model, shell roles, or
Linux graphics stack.

---

## 1. The live substrate constrains the design

The design is based on the current tree, not an assumed future kernel:

- `kernel/src/sched/process.rs` has one flat per-process handle table, uniform
  `{USE, DERIVE, GRANT, REVOKE}` meta-rights, variant-specific eight-bit
  semantic rights, a cross-process derivation graph, and bit-subset enforcement.
  New resource caps belong as `Handle` variants.
- `kernel/src/ipc/mod.rs` provides anonymous two-ended channels, synchronous
  send/receive rendezvous, and synchronous derived-copy cap grant/receive.
  It has no asynchronous port, wait-many, or atomic message-plus-cap operation.
- `kernel/src/mm/pmap.rs::allocate_user_region` creates private RW/NX pages.
  `Pmap::Drop` currently assumes it owns every user leaf frame and frees it. A
  shared pixel buffer therefore cannot be faked by mapping the same frame into
  two Pmaps; shared mapping ownership must be made explicit first.
- `kernel/src/drivers/fb/mod.rs` is a 32-bpp Limine linear framebuffer with one
  `FB` spinlock and software clear/plot/fill/alpha-blit primitives. There is no
  userland framebuffer cap, double buffer, display clock, page flip, or vblank.
- `kernel/src/drivers/fb/console.rs` and `sys_write(1, ...)` are the shipped raw
  console path. They must remain the exact compositor-off path.
- `kernel/src/drivers/usb/hid_keyboard.rs` decodes boot keyboard reports into a
  bounded byte queue, but `sys_read(0, ...)` is still a global fd-0 special case.
  Any process can currently invoke that path; it is not yet a per-surface cap.
- The nominal timer is 100 Hz (`timer::TICKS`). It is adequate for measured
  coarse timestamps and scheduling, but cannot substantiate 120 Hz or hardware
  vsync claims.

Two consequences are load-bearing:

1. **IPC is the compositor protocol carrier, not the pixel carrier.** Fixed
   messages describe state; `BufferCap` grants share the pages.
2. **G1 is genuinely kernel-touching.** Shared mapping ownership, framebuffer
   output authority, present fences, and focus routing are real prerequisites,
   not details that can be hidden in a userland library.

---

## 2. ISC-1 - The protocol, concrete

### 2.1 Carrier and bootstrap

Use existing anonymous IPC channels.

At boot, `neo-init` creates a compositor session channel, spawns the compositor
with one endpoint plus attenuated `FramebufferOutputCap` authority, and passes
the peer endpoint to the one G1 Atrium client. There is no global display-server
name and no ambient connect syscall.

The session channel is a `SessionCap`. `CREATE_SURFACE` causes the compositor to
create another channel pair. It retains the server endpoint, binds that endpoint
to one `Surface` record, and grants the client endpoint over the session channel.
That client endpoint is the `SurfaceCap`. Every surface verb travels over it, so
a guessed integer can never name another surface.

G1 has one session and one surface. The protocol still uses session-local IDs,
versioned fixed records, and explicit principal provenance so OT-22 adds clients
without changing the wire verbs.

### 2.2 Wire envelope

All messages are little-endian, fixed-layout records no larger than 1024 bytes:

```text
MsgHeaderV1 {
  magic:       u32 = 0x31434d4e,   // "NMC1" in little-endian bytes
  version:     u16 = 1,
  opcode:      u16,
  payload_len: u32,
  flags:       u32,
  request_id:  u64,
}
```

Rules:

- `payload_len` must exactly equal the opcode's v1 size; trailing or truncated
  bytes are `BAD_LENGTH`.
- Unknown mandatory opcode or non-zero reserved field is `BAD_VERSION` or
  `BAD_FIELD`, never silently ignored.
- Every request gets exactly one reply with the same `request_id`, except the
  terminal half of an explicit cap-transfer sequence.
- Object IDs are opaque, non-zero, monotonically allocated within one session.
  They are lookup keys after the channel cap check, never authority by themselves.
- The future build owes a hand-laid I-12 byte fixture for the header and one
  create/import/commit/present exchange. A writer-reader round trip is not an
  oracle.

### 2.3 Capability transfer on today's IPC

Today's IPC cannot attach a cap to a byte message atomically. Use one explicit,
bounded sequence everywhere rather than inventing implicit ordering:

```text
1. sender -> receiver: CAP_OFFER { request_id, transfer_id, kind, offered_rights }
2. receiver -> sender: CAP_READY { request_id, transfer_id, max_rights }
3. sender:   sys_grant(channel, source_cap, offered_rights)
4. receiver: sys_recv_grant(channel, max_rights)
5. receiver -> sender: CAP_ACCEPTED { transfer_id, object_id } or CAP_REJECTED
```

The receiver installs no compositor state until step 4 succeeds. If either peer
dies, the rendezvous returns `EPIPE`; the pending offer is discarded. Only one
cap transfer may be active per G1 surface channel, which matches the one-client
scope and prevents operation-type confusion in `EndpointState`.

`CREATE_SURFACE` uses the same sequence to transfer, in fixed order, the new
`SurfaceCap` then its receive-only `InputFocusCap`. `IMPORT_BUFFER` transfers one
read-only BufferCap child to the compositor. `PRESENT` transfers one wait-only
PresentFenceCap child to the client.

### 2.4 Kernel objects versus userland objects

| State/object | Owner | Why |
|---|---|---|
| Session, surface namespace, `Surface`, pending/committed state, role, bounds, scene order, imported-buffer table, damage region, focus policy, present queue | Userland compositor | Layout, roles, batching, beauty, and focus choice are policy. |
| `SharedPixelBuffer` pages + immutable descriptor | Kernel (`Handle::Buffer`) | Cross-Pmap memory ownership and read/write authority need kernel enforcement. |
| Active input-route object | Kernel (`Handle::InputFocus`) | The global fd-0 path must become non-ambient; only the active cap may receive. |
| One-shot present completion | Kernel (`Handle::PresentFence`) | Buffer reuse and measured completion must survive faults and cannot be a fake callback. |
| Linear framebuffer output authority | Kernel (`Handle::FramebufferOutput`) | A process must not gain ambient access to display memory; copies need bounds validation. |
| Workfield records, T0 glyph layout, Cap Halos/materials | Client userland | They are content, not compositor or kernel policy. |

### 2.5 Protocol verb table

Proposed names are ABI-document-ready; the build ADR assigns numeric opcodes.

| Verb | Inputs | Output | Authority / validation | State effect |
|---|---|---|---|---|
| `HELLO` | min/max version, client feature bits | selected v1, output geometry, limits, pacing mode | `SessionCap: USE+SEND+RECV` | None. Reject incompatible major version. |
| `CREATE_SURFACE` | role=`WORKFIELD_ROOT` or `TEXT`, requested width/height, client tag | configured bounds, `SurfaceId`, then SurfaceCap + InputFocusCap grants | SessionCap; compositor clamps to output and quota | Creates a dormant userland surface. G1 accepts exactly one full-screen root. |
| `IMPORT_BUFFER` | transfer id | `BufferId`, immutable `BufferDescV1` echoed from kernel query | SurfaceCap plus grant of `BufferCap: MAP_READ+QUERY`; desc/size/format must match surface | Compositor maps the kernel-owned pages read-only and records a session-local ID. |
| `DROP_BUFFER` | BufferId | `OK` or `BUSY` | SurfaceCap; caller session must own import; no queued/in-flight commit may reference it | Drops compositor mapping/handle after all fences are terminal. |
| `ATTACH_BUFFER` | BufferId, x=0, y=0 | `OK` | SurfaceCap; imported, live, correctly sized buffer | Sets pending buffer only. G1 forbids transforms and non-zero offset. |
| `DAMAGE_BUFFER` | 1..32 `{x,y,w,h}` rects in buffer coordinates | normalized rect count or error | SurfaceCap; checked arithmetic, clip to buffer, empty rect ignored | Unions into pending damage; no framebuffer write yet. |
| `COMMIT` | client serial | compositor commit serial, accepted/no-op | SurfaceCap; pending buffer exists; client serial monotonic; buffer not already in flight for this surface | Atomically freezes pending attach+damage into one immutable committed state and clears pending state. It is not scheduled yet. |
| `PRESENT` | commit serial, feedback requested=true | present id, then wait-only PresentFenceCap grant | SurfaceCap; commit belongs to session/surface and is not already presented/terminal | Associates a one-shot fence and queues the frozen commit. It does not create or modify surface state. |
| `FOCUS_REQUEST` | surface-local reason=`USER_ACTIVATION`, request serial | granted/denied + active focus generation | SurfaceCap; surface visible, InputFocus root live, compositor policy allows | On grant, compositor invokes `sys_input_focus_activate`; kernel atomically transfers the active route. |
| `DESTROY_SURFACE` | final client serial | `OK` | SurfaceCap | Marks record dead, rejects later verbs, discards queued commits, terminates fences, deactivates/revokes focus, releases buffer imports. |

Protocol errors are a small stable enum: `BAD_VERSION`, `BAD_LENGTH`,
`BAD_STATE`, `STALE_OBJECT`, `WRONG_OWNER`, `CAP_REQUIRED`, `LIMIT`, `BUSY`,
and `SURFACE_GONE`. Kernel cap failures retain their real errno (`EBADF`,
`EPERM`, `EPIPE`, `EFAULT`, `ENOMEM`). Do not flatten the two domains.

### 2.6 The surface state machine

```text
Created(no content)
  ATTACH + DAMAGE* -> Pending
  COMMIT           -> Committed(commit N), pending cleared
  PRESENT(N)       -> Queued(commit N, fence F)
  compositor       -> Executing(N) -> Composed | NoDamage | Discarded | Fault
  DESTROY          -> Dead
```

There is one unpresented committed state, one executing present, and one queued
present. A new `COMMIT` may replace an unpresented state without a fence. A newer
queued present may supersede an older queued present; the older fence terminates as
`DISCARDED_SUPERSEDED`. An executing commit is bounded and completes. A client
never has to infer whether it may reuse a buffer.

---

## 3. ISC-2 - The cap taxonomy made real

### 3.1 Mapping

| Ratified cap | Backing in G1 | New kernel kind? | G1 holders and meaning |
|---|---|---|---|
| `SurfaceCap` | Dedicated `Handle::Channel` endpoint bound by the compositor to one Surface record | **No** | Compositor retains the root/source endpoint; client receives a `USE+SEND+RECV` leaf. Possession is authority to issue verbs for that one record. |
| `BufferCap` | `Handle::Buffer { Arc<SharedPixelBuffer>, cap_id, rights }` | **Yes** | Client root owns/maps pixels; compositor holds only an attenuated read/query child. |
| `InputFocusCap` | `Handle::InputFocus { Arc<InputFocus>, cap_id, rights }` | **Yes** | Compositor root may activate/deactivate and revoke; client leaf may receive/query only while active. |
| `PresentFenceCap` | `Handle::PresentFence { Arc<PresentFence>, cap_id, rights }` | **Yes** | Compositor signal side is consumed by present/discard; client wait side is consumed by wait. |
| `DecorationCap` | `SurfaceCap` role=`DECORATION`, minted and retained by compositor | **No** | No client receives one in G1. The role is reserved in v1; client-created decoration is rejected. |
| `CaptureCap` | No object, constructor, verb, or default inheritance | **Absent** | A BufferCap only exposes its own pages; FramebufferOutputCap has no read/capture right. Future capture requires a separate ratified cap and protocol. |

Supporting device authority:

| Cap | Backing | Holder |
|---|---|---|
| `FramebufferOutputCap` | `Handle::FramebufferOutput` over the boot framebuffer | `neo-init` root; compositor gets `QUERY+PRESENT` child only after readiness. |

### 3.2 Semantic-right vocabularies

Uniform meta-rights remain exactly ADR-010's `USE | DERIVE | GRANT | REVOKE`.
Proposed independent semantic bits:

```text
buffer_rights:
  MAP_READ  = 1<<0
  MAP_WRITE = 1<<1
  QUERY     = 1<<2

input_focus_rights:
  RECEIVE    = 1<<0
  ACTIVATE   = 1<<1
  DEACTIVATE = 1<<2
  QUERY      = 1<<3

present_fence_rights:
  WAIT       = 1<<0
  COMPLETE   = 1<<1
  DISCARD    = 1<<2
  QUERY      = 1<<3

framebuffer_output_rights:
  QUERY   = 1<<0
  PRESENT = 1<<1
  RELEASE = 1<<2
```

Every active bit gates a named syscall. There are no decorative rights. Future
bits are reserved only when a concrete later operation is named in the ABI ADR.

### 3.3 Mint, derive, grant, attenuate, revoke

The generic rule for every new variant is the existing I-15 shape:

```text
child.meta     & !parent.meta     == 0
child.semantic & !parent.semantic == 0
```

`derive_handle`, three-way-intersection grant, cross-process revoke, and spawn
inheritance stay kind-uniform. Each new `Handle::with_cap` arm changes only the
rights, never object identity or immutable metadata. The future build owes an
exhaustive child-subset host sweep per new rights vocabulary.

Concrete derivation rules:

- **SurfaceCap:** the compositor calls `sys_chan_create`; the kernel mints both
  channel roots. The client receives `USE + SEND|RECV`, with no DERIVE, GRANT,
  or REVOKE in G1. Raw delegation of the same endpoint is deliberately blocked;
  future surface export asks the compositor to mint a new endpoint so concurrent
  holders never share one current rendezvous side.
- **BufferCap:** `sys_buffer_create(desc)` root-mints full meta-rights and
  `MAP_READ|MAP_WRITE|QUERY`. Yes, the client may attenuate it. The compositor
  accepts at most `USE + MAP_READ|QUERY`; three-way intersection makes a client
  request for compositor write access harmless. Revoking the client root removes
  imported descendants. An already executing bounded framebuffer syscall may
  finish using its pinned Arc; no later present can start.
- **InputFocusCap:** the compositor creates one object per surface, retains
  `ACTIVATE|DEACTIVATE|QUERY` plus full meta-rights, and grants the client a
  no-meta leaf with `USE+RECEIVE|QUERY`. The client cannot self-focus or delegate.
- **PresentFenceCap:** the compositor root-mints the object, derives/grants a
  `USE+WAIT` leaf, and retains `USE+COMPLETE|DISCARD`. Both operations are
  one-shot and consume their role handle. A client cannot signal its own fence.
- **DecorationCap:** compositor-created SurfaceCap role only. A client can request
  decoration policy later, but cannot draw compositor chrome or derive the role.
- **CaptureCap:** no root exists, so derivation cannot manufacture one. Neither
  BufferCap MAP_READ nor output PRESENT implies capture.

### 3.4 Revoking focus during a frame

Focus and pixels are independent objects. Revoking the compositor's focus root:

1. atomically removes that object from the active-input slot;
2. wakes a blocked `sys_input_recv` with `FOCUS_REVOKED`/`EBADF`;
3. discards bytes not yet delivered to userland;
4. removes all derived client focus handles through the existing graph.

It does **not** cancel or corrupt the current framebuffer copy. An event already
returned to userland cannot be recalled; no later event routes to that principal.
This is the precise mid-frame boundary.

---

## 4. ISC-3 - Buffers, zero-copy, damage, software composition

### 4.1 Immutable buffer descriptor

G1 supports one deliberately narrow format:

```text
BufferDescV1 {
  width:  u32,
  height: u32,
  stride: u32,     // >= width*4, multiple of 4
  format: u32 = XRGB8888_LE,
  bytes:  u64 = stride*height,
}
```

All multiplication/addition is checked. `width`/`height` must be non-zero and no
larger than the framebuffer. `bytes` is page-rounded and bounded by the current
output size plus one row of alignment, with an absolute 64 MiB G1 ceiling. The
descriptor is stored in the kernel object and cannot be changed through mapped
memory. Alpha formats are reserved for a later slice; the one full-screen G1
surface is opaque, and T0 composites glyph coverage into its own XRGB buffer.

### 4.2 Allocation and mapping lifecycle

```text
client:
  buffer_h = sys_buffer_create(desc)
  client_va = sys_buffer_map(buffer_h, MAP_READ|MAP_WRITE)
  draw into client_va
  IMPORT_BUFFER + grant(buffer_h, USE|MAP_READ|QUERY)

compositor:
  imported_h = recv_grant(max=USE|MAP_READ|QUERY)
  desc = sys_buffer_query(imported_h)
  compositor_va = sys_buffer_map(imported_h, MAP_READ)
```

Both VAs map the same physical pages: zero copies between client and compositor.
The final software composition is necessarily a copy into the active linear
framebuffer; G1 must never label that `ZERO_COPY` presentation.

The shared-Pmap ownership change is explicit:

- `SharedPixelBuffer` owns and eventually frees its frames.
- A Pmap mapping records `BorrowedBuffer`, removes PTEs on unmap/exit, and does
  **not** free borrowed leaf frames in `Pmap::Drop`.
- A mapping pins the Buffer Arc until its PTEs are removed. Buffer destruction
  cannot free frames still mapped or held by an in-flight present syscall.
- The compositor mapping is hardware read-only/NX. Client mapping is RW/NX.
- Mapping/unmapping/revocation performs the existing TLB shootdown discipline.

This is a required kernel slice. Reusing `sys_map_region` frames without changing
the Pmap ownership model would double-free shared frames at process exit.

### 4.3 Ownership and scribble containment

The protocol lease is:

```text
ClientWrite -> PRESENT accepted -> CompositorRead -> terminal PresentFence -> ClientWrite
```

Honest clients use two buffers and never write a buffer in `CompositorRead`.
G1 does not require page-protection flips on every frame; that would add global
mapping walks and TLB shootdowns to the hot path. A malicious or buggy client may
therefore scribble its RW mapping during composition. The result is limited to
tearing or wrong pixels **inside its own clipped surface** because:

- the client cannot alter BufferDesc, stride, byte length, Surface bounds, damage
  normalization, framebuffer address, or compositor pointers;
- the compositor reads only a kernel-created RO mapping, indexes with checked
  descriptor arithmetic, and copies only normalized rectangles;
- the output syscall takes BufferCap plus rects, never a client pointer;
- the client holds no FramebufferOutputCap and cannot name another client's
  BufferCap.

Thus an adversarial scribble can corrupt its own visible content, not compositor
memory, another surface, kernel memory, or governance. A later immutable-snapshot
or write-seal optimization may improve tear resistance, but is not needed for
memory safety and should not burden G1.

### 4.4 Pending state and damage

`ATTACH_BUFFER` and `DAMAGE_BUFFER` modify pending state. `COMMIT` atomically
moves both into an immutable but unscheduled `Commit`; `PRESENT` queues it:

```text
Commit {
  serial,
  BufferId,
  normalized_damage: [Rect; <=32],
  submitted_tick,
}
```

Damage uses buffer coordinates only. The compositor:

1. rejects arithmetic overflow;
2. clips each rectangle to BufferDesc;
3. removes empty rectangles;
4. unions overlapping/touching rectangles;
5. if more than 32 remain, collapses to their checked bounding box;
6. forces full-surface damage on first attach, dimension/format change, or output
   takeover.

An empty-damage commit with unchanged buffer is a real no-op. Its fence returns
`NO_DAMAGE`; no framebuffer-present claim is emitted.

### 4.5 The G1 software composition loop

```text
wait for a queued commit
  -> if another non-executing commit supersedes it, terminate old fence discarded
  -> snapshot userland Surface state; hold no scene lock across a syscall
  -> for each normalized output-clipped rect:
       prepare {src_x, src_y, dst_x, dst_y, w, h}
  -> sys_fb_present_batch(output_cap, buffer_cap, rects, signal_fence, commit_id)
       kernel validates both cap kinds and every bound before taking FB lock
       kernel copies XRGB rows using BufferDesc stride into native framebuffer
       kernel releases FB lock
       kernel stamps completion ticks and completes the fence
  -> publish userland committed state
```

For the single opaque full-screen G1 surface, this is final composition. The API
still names source and destination rects so later retained scene nodes can compose
into a compositor-owned staging BufferCap without changing client verbs.

The current `blit_rgba`/`fill_rect` code is the implementation floor, but the new
output operation must be stride-aware and take the framebuffer lock once per
bounded batch, not once per pixel/glyph. No client-controlled validation occurs
while the lock is held.

---

## 5. ISC-4 - Present pacing and honest liveness

### 5.1 What "vsync" means in G1

It means **not available**.

QEMU and the current bare-metal framebuffer expose no vblank event, page-flip
completion, display clock, or scanout sequence to this kernel. A 100 Hz timer or
the return of a CPU copy cannot prove vertical retrace. G1 therefore advertises:

```text
pacing_mode = SOFTWARE_IMMEDIATE
refresh_ns  = 0
flags       = SOFTWARE_COPY
```

It never sets `VSYNC`, `HW_CLOCK`, `HW_COMPLETION`, or `ZERO_COPY`. This follows
the upstream presentation-time distinction: software scheduling is insufficient
for a vsync flag, and a timer guess is not hardware completion.

### 5.2 Pacing without theater

G1 pacing is bounded backpressure, not a fake refresh clock:

- one commit may execute and one may wait;
- a third submit either supersedes the waiting commit (with an explicit discard
  result) or receives `BUSY`, as selected by the surface's fixed v1 policy;
- a client must own a free buffer, normally learned from a terminal fence, before
  drawing another frame;
- the compositor begins work immediately when idle; it does not wait for an
  invented vblank;
- damage limits bound work per present; scheduler preemption bounds CPU monopoly.

This makes frame production track actual copy capacity on both QEMU and metal.
When a real display driver later supplies vblank/page-flip events, the same
present/fence ABI can populate refresh, sequence, and hardware flags truthfully.

### 5.3 PresentFenceCap result

```text
PresentResultV1 {
  present_id:        u64,
  commit_id:         u64,
  outcome:           COMPOSED_TO_FB | NO_DAMAGE |
                     DISCARDED_SUPERSEDED | SURFACE_GONE |
                     COMPOSITOR_GONE | OUTPUT_ERROR,
  submit_tick:       u64,
  compose_start_tick:u64,
  complete_tick:     u64,
  refresh_ns:        u64 = 0,
  output_sequence:   u64 = 0,
  flags:             u32,
}
```

`sys_fb_present_batch` is the only operation allowed to complete a fence as
`COMPOSED_TO_FB`; it stamps the result after the last framebuffer store and lock
release. The compositor may terminate a not-started fence only with a discard or
fault outcome. If the last completion holder dies, the fence wakes as
`COMPOSITOR_GONE`. `sys_present_fence_wait` returns the payload and consumes the
client's wait handle.

The fence is also the BufferCap release: every terminal outcome guarantees the
compositor no longer reads that commit's buffer. This is stricter and less
ambiguous than a generic frame callback.

### 5.4 Liveness Thread integration

Every visible present segment is backed by the fence timestamps and outcome:

```text
PresentQueued  <- COMMIT accepted at submit_tick
PresentRunning <- output syscall began at compose_start_tick
PresentComplete/Discarded <- terminal fence at complete_tick
```

The G0 Liveness Thread may light `PresentComplete` only from the terminal result.
No animation timer, spinner, or UI acknowledgement may synthesize it. A coarse
clock may produce equal begin/end ticks; that is honest coarse measurement, not
permission to invent duration.

---

## 6. ISC-5 - Input routing is the cap check

### 6.1 Replace the ambient fd-0 path only when G1 takes over

Today HID and serial bytes are polled by `sys_read(0)`. With compositor takeover
active, ordinary `sys_read(0)` must return `EBADF`/`EPERM`; otherwise every client
can still read global input and the new focus caps are decoration.

The exact compositor-off branch keeps today's `sys_read_stdin` and
`try_read_stdin_byte` behavior. The compositor-on branch routes the same decoded
bytes through `sys_input_recv(InputFocusCap, ...)`.

### 6.2 InputFocus kernel object

```text
InputFocus {
  object_id,
  bound_surface_generation,
  bound_principal,       // immutable creator/grant provenance
  active_generation,
  revoked,
}

InputEventV1 {
  kind: TEXT_BYTE | FOCUS_GAINED | FOCUS_LOST,
  value: u32,
  modifiers: u32 = 0,
  ticks: u64,
  focus_generation: u64,
}
```

G1 deliberately preserves today's decoded-byte semantics. Raw keycodes,
key-release events, pointer, and touch can extend the versioned event union later;
they do not justify widening this slice.

### 6.3 One routing/enforcement path

```text
client FOCUS_REQUEST over SurfaceCap
  -> compositor checks visible surface + user-activation policy
  -> compositor calls sys_input_focus_activate(root InputFocusCap)
  -> kernel atomically swaps ACTIVE_INPUT_FOCUS
  -> old receiver observes FOCUS_LOST
  -> new receiver observes FOCUS_GAINED

client sys_input_recv(receive-only InputFocusCap)
  -> kernel checks Handle kind + USE + RECEIVE
  -> kernel checks this exact object == ACTIVE_INPUT_FOCUS
  -> kernel rechecks active generation before each HID/serial poll
  -> one decoded event is returned only to that holder
```

There is no broadcast queue, global client list, or "select all input" right.
The compositor decides **which** focus object should become active; the kernel
enforces that only that object can consume the physical stream. Input security
and input routing are literally the same comparison.

### 6.4 Transfer, revoke, and no snooping

- Focus transfer is atomic with respect to polling: the old receiver rechecks
  generation before touching the device queue; after the swap it cannot consume
  the next byte.
- The client focus leaf has no ACTIVATE, DERIVE, GRANT, or REVOKE right.
- Revoking the root clears the active slot and wakes the old blocked receiver.
- A process without the exact active InputFocusCap gets `EPERM` before any input
  driver poll. Timing of other surfaces' input is not exposed through a global
  queue length.
- CaptureCap is irrelevant to input; it cannot be used as an observation bypass.

An adversarial two-process QEMU scenario must prove: focused client receives a
byte; unfocused client and raw fd 0 cannot; focus transfer sends the next byte
only to the new client; revoke stops delivery without stalling the HID path.

---

## 7. ISC-6 - The seams, honestly

### 7.1 OT-22 multi-principal readiness

G1 ships `PrincipalId::OwnerSession`, but the following are already protocol
invariants:

- one session channel per principal;
- one namespace per session, so SurfaceId/BufferId are never global authority;
- one SurfaceCap channel per surface, with no raw endpoint sharing;
- BufferDesc stores immutable creator principal provenance in the kernel object;
- InputFocus binds an immutable recipient principal and generation;
- all quotas are keyed by session/principal, even though G1 has one entry;
- client-provided PID/principal fields are absent from authority decisions.

When OT-22 opens, exactly these mechanisms are added:

1. **Principal provenance:** `neo-init` supplies a kernel-authenticated principal
   cap or a read-only channel-peer-principal query. The compositor never trusts a
   principal number in a client message.
2. **Wait-many or workers:** today's compositor cannot block on N synchronous
   surface channels with one user thread. Add a bounded wait-many primitive or
   one isolated session worker per principal. This changes event-loop plumbing,
   not protocol verbs or object semantics.
3. **Per-principal quotas/accounting:** maximum surfaces, mapped bytes, queued
   commits, damage rects, and CPU budget; attribution uses immutable cap creator
   provenance, not message claims.
4. **Cross-principal focus policy:** the compositor may select among principals;
   the kernel still activates one InputFocusCap and emits principal-tagged focus
   generations. The input ABI does not change.
5. **Surface export:** a holder requests an attenuated export; the compositor
   creates a fresh endpoint/profile for the target principal. It does not grant
   one channel side to concurrent senders.

OT-22 remains one pillar, kill-switched and consolidated separately. None of
POSIX personality, app distribution, or multi-tenant QoS needs to enter the G1
build dispatch.

### 7.2 G0 to G1

The G0 Workfield becomes G1's one full-screen `WORKFIELD_ROOT` surface.

Keep the G0 renderer's model/layout and replace only its output trait:

```text
G0: WorkfieldRenderer -> RawFramebufferSink
G1: WorkfieldRenderer -> BufferSink(BufferCap RW mapping)
```

The named G0 regions - Intent surface, Folder/Candidate/Action surfaces, Grant
surface, Undo Shadow, Command Graph, and Liveness Thread - remain retained client
scene nodes with stable role IDs inside that root buffer. G3 can promote the
nodes that need independent isolation/input boundaries into separate SurfaceCaps;
G1 does not force per-widget compositor objects.

At compositor takeover, kernel stdout remains serial but stops writing the
framebuffer; the G0 client renders the transcript/Workfield into its BufferCap.
The old framebuffer console is not deleted or rewritten. Releasing takeover
restores it.

### 7.3 T0 text

T0 is an ordinary client renderer:

1. map the client's BufferCap RW;
2. shape/rasterize glyphs in userland;
3. paint glyph coverage and background into the XRGB surface buffer;
4. damage the affected text rows;
5. commit/present through the same SurfaceCap.

The compositor never knows glyphs, fonts, transcripts, or T0. The kernel 8x16
font remains early-boot/fallback only. G2 may change how the client fills the
buffer; the G1 sharing and present protocol stays unchanged.

### 7.4 I-14 fault containment - required enumeration

A compositor fault **can**:

- corrupt or tear the pixels of buffers explicitly granted to it only through
  its read access (it cannot write those mappings in G1);
- write wrong but bounds-clipped pixels to the framebuffer through its output cap;
- miss frames, freeze the last frame, lose focus, or terminate client fences with
  `COMPOSITOR_GONE`;
- consume its scheduled CPU budget until preempted/killed.

A compositor fault **cannot**:

- map or read any client memory outside a granted BufferCap;
- write client BufferCaps, kernel memory, another process's Pmap, or raw physical
  framebuffer memory;
- read physical input without the active InputFocus mechanism, or grant itself a
  client BufferCap/CaptureCap it was never given;
- access NeoFS, network, process control, model/tensor, or governance state without
  separate caps unrelated to the compositor role;
- issue device DMA. G1 buffers are CPU pages; any later GPU buffer remains fenced
  by the existing IOMMU/DMA arena before this protocol accepts it;
- hold, acquire, or wait on `INFLIGHT`, `RECENCY`, `ARENA`, reservation, tensor,
  model, or other AI-governance locks.

Kernel-side containment rules:

- Buffer descriptors and rect batches are fully validated before taking the
  framebuffer lock.
- The FB critical section is a bounded leaf operation and holds no scene, IPC,
  input, scheduler-policy, filesystem, or governance lock.
- PresentFence and InputFocus wait/wake use existing scheduler token patterns;
  teardown wakes waiters rather than killing a parked thread.
- The compositor gets an attenuated output child; `neo-init` retains revoke and
  fallback authority. Process death drops mappings, abandons fences, clears focus,
  and lets the supervisor re-enable the raw console.
- Compositor priority is deadline-aware but budgeted. It is never an unbounded
  real-time task that can starve governance; a missed GUI deadline degrades the
  GUI, not the AI-governance liveness path.

The build's I-14 section must include a grep/lock audit plus a runtime fault
scenario: kill the compositor with a client blocked on present and input, observe
fence/input wake, restore raw console, and prove governance progress continues.

---

## 8. ISC-7 - Kill switch, build decomposition, and forks

### 8.1 Kill-switch equality

Use two layers:

```text
Cargo feature: atrium-compositor-g1   (default off)
Boot/runtime:  atrium_compositor=off  (default until the G1 build is ratified)
```

When either is off:

- `neo-init` does not spawn/take over the compositor;
- `sys_write(1)` calls the existing framebuffer console exactly as today;
- `sys_read(0)` calls the existing serial/HID stdin path exactly as today;
- no Buffer/InputFocus/PresentFence/FramebufferOutput G1 object is minted;
- no Workfield BufferSink or compositor process participates.

Proof shape for the future build:

1. keep `fb/console.rs` and its call from the off branch structurally unchanged;
2. host equality KAT for routing selectors: feature/runtime off chooses the old
   stdout/stdin functions for every state;
3. deterministic QEMU negative scenario with G1 compiled but runtime off: feed
   the same input transcript as the v1 baseline and assert identical serial bytes
   and final framebuffer pixel digest, plus absence of compositor breadcrumbs;
4. no-feature build digest/regression showing no G1 symbols or behavior;
5. separate on-path scenario so the equality test cannot pass because G1 never
   compiled or never reached takeover.

`DESTROY_SURFACE` or a hidden Workfield is not the kill switch. Only the complete
takeover being absent is equality to today's console.

### 8.2 Smallest-first build decomposition

Every kernel-touching slice is default-off and receives the close gate plus the
manual four-shard QEMU gate before merge.

1. **Host protocol/state slice.** Fixed v1 records and hand-laid byte oracle;
   pure surface state machine; damage clip/union/bounding tests including overflow
   and >32-rect collapse; fence outcome truth table. No kernel or framebuffer.
2. **BufferCap + shared-Pmap slice.** `Handle::Buffer`, rights mapping, create/query/
   map, owned-vs-borrowed Pmap leaves, teardown, TLB discipline. Host child-subset
   sweep and QEMU proof that client RW and compositor RO VAs alias the same bytes,
   write is denied from compositor, revoke/exit never UAFs or double-frees.
3. **Output + fence mechanism slice.** `FramebufferOutputCap`, PresentFenceCap,
   clipped stride-aware `sys_fb_present_batch`, kernel-stamped completion, waiter
   abandonment. Negative rect/cap tests and framebuffer-nonblank QEMU proof. Raw
   console remains selected unless takeover is explicitly enabled.
4. **One surface, one client, full repaint.** Userland compositor + SessionCap +
   SurfaceCap channel; create/import/attach/commit/present/destroy; one opaque
   full-screen buffer; present fence releases it. This is the smallest visible
   vertical slice.
5. **Damage + bounded pacing.** Add normalized damage batches, one-executing/
   one-pending queue, superseded/no-damage outcomes. Pixel sentinel test proves
   bytes outside damage remain unchanged and no fake hardware flags are set.
6. **InputFocus slice.** Replace ambient fd 0 only during takeover; focus request,
   activate/transfer/revoke, HID+serial receive through the cap check. Adversarial
   two-process no-snoop QEMU scenario.
7. **G0/T0 migration + takeover.** Workfield renderer gains BufferSink; named G0
   regions remain client nodes; T0 paints the same buffer; atomic output/input
   takeover and supervisor fallback. Kill-switch equality scenario runs here.
8. **Full G1 scenario and I-14 gate.** Boot -> raw console -> compositor ready ->
   one Workfield SurfaceCap -> buffer import -> damaged commit -> measured present
   -> focus input -> revoke -> compositor fault/fallback -> governance progress.
   QEMU 4-shard plus the later OT-41-style bare-metal legibility run.

Slice 4 may full-repaint as an intermediate proof, but **G1 is not complete until
slice 5 damage tracking and honest pacing, slice 6 input caps, and slice 8
containment/equality are green.**

### 8.3 Honest forks and recommendations

| Fork | Recommendation | Reason |
|---|---|---|
| Surface verbs: direct syscalls or IPC | **IPC** | Surface/scene/focus policy belongs in userland. Syscalls are only for kernel resources and bounded device mechanisms. |
| SurfaceCap: new Handle kind or channel role | **Per-surface Channel** | The endpoint is already unforgeable and revocable. A kernel Surface object adds no enforcement unless the kernel mediates policy verbs. |
| PresentFenceCap: one-shot channel or new fence kind | **New one-shot Fence** | Current channels have no general close and a per-frame channel would leak handles/state. A fence can auto-consume, bind completion to framebuffer copy, and wake on compositor death. |
| Input: compositor forwards global fd 0 or kernel InputFocusCap | **Kernel InputFocusCap** | Forwarding leaves an ambient raw-input syscall unless every caller is special-cased. The active-cap comparison makes no-snoop structural. |
| Buffer write sealing during composition | **No hot-path sealing in G1** | RO compositor mapping + checked clipping contains malicious writes to the client's own pixels. Per-frame PTE flips add TLB/locking cost without a memory-safety payoff. |
| Scene graph granularity | **One retained node per cap-surface, not per widget** | G1's Workfield is one surface; its G0 regions remain client nodes. Promote only isolation/input boundaries in G3. |
| First visible repaint | **Full repaint in slice 4, mandatory damage in slice 5** | Separates vertical bring-up from rectangle correctness without redefining final G1. |
| G1 pixel format | **Opaque XRGB8888 only** | Matches the one full-screen MVP, avoids premature alpha/color complexity, and keeps format extension explicit. |
| DecorationCap in v1 | **Reserve role, issue none to clients** | Compositor-owned chrome is settled; building chrome before multiple surfaces is unearned machinery. |
| Pacing | **Immediate measured copy + backpressure** | There is no honest vblank source. Timer-paced fake vsync would damage the liveness contract. |

---

## 9. Acceptance invariants for the build dispatch

The eventual G1 build is not done unless all of these have direct evidence:

1. A client without SurfaceCap cannot issue any surface verb, even with a valid
   SurfaceId copied from another session.
2. The compositor maps a granted BufferCap read-only; its rights and kernel
   `BufferDesc` are a subset/immutable, and client/compositor mappings alias the
   same frames without copy or double-free.
3. Out-of-bounds, overflowed, stale, wrong-format, and too-many-damage inputs are
   rejected or normalized before the framebuffer lock.
4. Pixels outside normalized damage remain byte-identical after a present.
5. A PresentFence never completes `COMPOSED_TO_FB` before the framebuffer batch
   returns; every present reaches exactly one terminal outcome; client reuse is
   safe after every terminal outcome.
6. G1 reports no vsync/hardware/zero-copy flag on the software path.
7. Only the active InputFocusCap holder receives the next HID/serial event; raw
   fd 0 and an unfocused process cannot snoop; transfer and revoke take effect at
   the next event boundary.
8. A client scribbling its buffer can affect only pixels inside its surface and
   cannot corrupt compositor/kernel/governance memory.
9. Killing the compositor wakes blocked fence/input clients, releases mappings,
   clears focus, permits raw-console fallback, and does not stop governance.
10. `atrium_compositor=off` reproduces the v1 raw console serial transcript and
    framebuffer digest bit-for-bit for the equality fixture.

---

## 10. Sources read

### 10.1 Ratified/shared project sources

- `docs/THINK-2026-07-08-gui-vision-SYNTHESIS.md` sections 4-6
- `docs/OPEN-THREADS.md` OT-52 and OT-22
- `docs/THINK-2026-07-10-atrium-g0-buildable-spec.md`
- `docs/THINK-2026-07-10-atrium-design-language.md`
- `docs/THINK-2026-07-10-atrium-text-engine-research.md`
- `docs/ADR-004-ipc-channels.md`
- `docs/ADR-010-capability-system-foundation.md`
- `docs/ADR-061-file-capability-unification-i15.md`
- `docs/complexity-governance.md` I-14/I-15

### 10.2 Live implementation seams

- `kernel/src/sched/process.rs`
- `kernel/src/sched/file_cap_logic.rs`
- `kernel/src/ipc/mod.rs`
- `kernel/src/mm/pmap.rs`
- `kernel/src/arch/x86_64/syscall.rs`
- `kernel/src/arch/x86_64/timer.rs`
- `kernel/src/drivers/fb/mod.rs`
- `kernel/src/drivers/fb/console.rs`
- `kernel/src/drivers/usb/hid_keyboard.rs`
- `kernel/src/drivers/usb/hid_logic.rs`
- `userland/libneo/src/lib.rs`

RAG health was checked first and returned healthy (`ollama=true`, `qdrant=true`,
collection green). RAG was used for discovery; the named implementation files
were then read directly.

### 10.3 Firsthand prior art

The upstream repositories were cloned and the protocol XML read directly, not
through a summary:

- Wayland core `protocol/wayland.xml`, upstream commit
  `c23a8beb1ff41428e841137175c6be738ab627be`:
  [upstream source](https://gitlab.freedesktop.org/wayland/wayland/-/blob/c23a8beb1ff41428e841137175c6be738ab627be/protocol/wayland.xml).
  Relevant primitives: `wl_surface.attach`, buffer-coordinate damage, pending
  state made atomic by commit, frame throttling hints, and `wl_buffer.release`
  only after compositor use ends.
- Wayland protocols `stable/presentation-time/presentation-time.xml`, upstream
  commit `afb614d5fcbd02d261a6ae91920aa91cf3915a8a`:
  [upstream source](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/afb614d5fcbd02d261a6ae91920aa91cf3915a8a/stable/presentation-time/presentation-time.xml).
  Relevant discipline: one-shot presented/discarded feedback; software scheduling
  is not sufficient for the vsync flag; a timer guess is not hardware completion;
  compositing/copying is not zero-copy presentation.
- Wayland's official architecture description:
  [Wayland Architecture](https://wayland.freedesktop.org/docs/book/Architecture.html).
  Relevant primitives: clients own rendering buffers, compositor owns the final
  scene and input hit-testing, and damage bounds recomposition.

We lift those mechanisms because their invariants are useful. We do not import
the Wayland protocol, roles, registry, fd transport, DRM/KMS, GBM/EGL, or Mesa.

---

## 11. Close / RETURN shape

- THINK: `docs/THINK-2026-07-10-g1-compositor-chad.md`
- ISC-1: IPC carrier, concrete envelope/cap-transfer sequence, kernel-vs-userland
  split, full verb table, surface state machine.
- ISC-2: taxonomy mapped; new Buffer/InputFocus/PresentFence kinds; Surface and
  Decoration over Channel; Capture absent; derivation/attenuation/revocation.
- ISC-3: client-allocated shared pages, explicit Pmap ownership, read-only
  compositor mapping, damage normalization, bounded software composition.
- ISC-4: immediate backpressure pacing, no fake vsync, kernel-stamped one-shot
  fence, measured Liveness Thread events.
- ISC-5: focus request -> kernel active-cap check -> one receiver; transfer/revoke
  and no-snoop construction.
- ISC-6: exact OT-22 additions, G0 BufferSink migration, ordinary T0 client path,
  required I-14 can/cannot-touch enumeration.
- ISC-7: two-layer kill switch, eight smallest-first slices with gates, ten
  explicit forks/recommendations.

Docs-only lane: no code and no QEMU. Repository path filters mean no CI run is
expected. Run the local `scripts/worker-close-gate.sh` host set and record the
docs-only skip/note, then diff this RETURN list against the committed artifact
before sending.
