# RESEARCH — Phase 4 BUILD-1a: Rust virtio-net Ecosystem Prior Art Survey

| | |
|---|---|
| **Date** | 2026-05-28 |
| **From** | Prometheus (ecosystem breadth survey) |
| **To** | PM / Jarvis (BUILD-1a implementation substrate) |
| **Dispatch** | `Plans/DISPATCH-2026-05-28-phase-4-build-1a-rust-virtio-net-prior-art-prometheus.md` |
| **Authority** | Parallel with Cerebro (`RESEARCH-2026-05-28-phase-4-build-1a-smoltcp-and-virtio-net-cerebro.md`) — complementary, not duplicative. Cerebro covers smoltcp surface + virtio-blk audit; this covers what every other Rust OS does for virtio-net. |
| **Type** | RESEARCH-class artifact. Prior art survey for BUILD-1a virtio-net driver design. |
| **Scope** | Asterinas, Theseus OS, Redox OS (`virtio-drivers` crate), Hubris OS, LionsOS (negative result), smoltcp examples, `virtio-drivers` crate architecture. |

---

## 1. Executive Summary

The Rust virtio-net ecosystem converges on a single dependency graph: **`virtio-drivers` crate → smoltcp**. Every Rust OS that implements virtio-net either uses the `virtio-drivers` crate directly (Redox, Toubkal, rCore) or builds an equivalent HAL + queue + transport abstraction from scratch (Asterinas). The variation is in how each OS bridges the driver to its protocol stack and manages DMA buffers.

**Key finding for NeoPlutarch:** We should NOT depend on the `virtio-drivers` crate. Our virtio-blk driver already owns the HAL + Transport + VirtQueue primitives. BUILD-1a's virtio-net driver should extend virtio-blk's existing abstractions (same `Hal`, same `Transport`, same `VirtQueue`) with net-specific device logic — exactly as Asterinas does with its `aster-virtio` crate. This avoids an external dependency, keeps the TCB small, and reuses code we already have.

**Convergence table:**

| Project | virtio-drivers crate? | Own HAL? | Own VirtQueue? | Protocol stack | RX buffer strategy | Notable |
|---|---|---|---|---|---|---|
| **Asterinas** | No | Yes | Yes | smoltcp fork + BigTCP | Pooled DMA, pre-alloc | 100% safe Rust driver, softirq BH |
| **Redox** | Yes (owner) | Yes (trait) | Yes (crate) | smoltcp | Pre-alloc + recycle | Two-tier API (Raw/managed) |
| **Toubkal** | Yes (dep) | Yes | No (crate) | smoltcp 0.13.0 | Pre-alloc + recycle | Global singleton, identity mapping |
| **rCore OS** | Yes (owner) | Yes | Yes (crate) | smoltcp | Pre-alloc + recycle | Crate maintainer, PCI + MMIO |
| **Theseus** | No | N/A | N/A | smoltcp | MPMC queue pool | No virtio; physical NICs only |
| **Hubris** | No | N/A | N/A | smoltcp (UDP-only) | Static pools | No virtio; ARM microcontroller |

---

## 2. Asterinas — Safe Rust Framekernel Approach

**Repo:** `github.com/asterinas/asterinas` (v0.17.2, MPL-2.0)
**Virtio crate:** `kernel/comps/virtio/` (`aster-virtio`)
**Network crate:** `kernel/comps/network/` (`aster-network`)

### 2.1 Architecture

Asterinas uses a **framekernel** model: the OSTD (OS Trusted Software Domain) contains all `unsafe` code (MMIO, DMA, IRQ). Device drivers compile with `#![deny(unsafe_code)]`. The virtio crate is a kernel component — not an external dependency — that sits between OSTD's low-level abstractions and the network stack.

### 2.2 Key types

```rust
// Core device struct (device/network/device.rs)
pub struct NetworkDevice {
    config_manager: ConfigManager<VirtioNetConfig>,
    caps: DeviceCapabilities,           // smoltcp capabilities
    mac_addr: EthernetAddr,
    send_queue: VirtQueue,              // queue index 1
    recv_queue: VirtQueue,              // queue index 0
    header: VirtioNetHdr,               // pre-allocated 16-byte virtio-net header
    tx_buffers: Vec<Option<TxBuffer>>,  // indexed by token
    rx_buffers: SlotVec<RxBuffer>,      // indexed by token
    new_rx_buffer: Option<RxBuffer>,    // pre-created replacement RX buffer
    transport: Box<dyn VirtioTransport>,
    poll_stat: PollStatistics,          // batched notification tracking
}
```

Queue constants: `QUEUE_RECV = 0`, `QUEUE_SEND = 1`, `QUEUE_SIZE = 64`.

### 2.3 Transport abstraction

`VirtioTransport` trait cleanly separates device logic from transport details (PCI modern v1.x, PCI legacy, MMIO). The same `NetworkDevice` works over any transport. `ConfigManager<T>` handles modern vs legacy config space access.

### 2.4 Feature negotiation — deliberately conservative

Asterinas only accepts `VIRTIO_NET_F_MAC | VIRTIO_NET_F_STATUS`. It explicitly rejects:
- Checksum offload (`VIRTIO_NET_F_CSUM`, `GUEST_CSUM`)
- TSO/UFO (`GUEST_TSO4/6`, `GUEST_UFO`)
- Multiqueue (`VIRTIO_NET_F_MQ`)
- RSS, USO, RSC

This keeps the driver simple and correct. The `init_caps()` function asserts these features are NOT negotiated at runtime.

### 2.5 DMA buffer management — pooled with pre-allocation

RX and TX buffer pools (`DmaPool<FromDevice>` / `DmaPool<ToDevice>`) are initialized with 32 segments (high watermark 64), each 4096 bytes. All 64 RX queue slots are pre-populated before device activation. A replacement `RxBuffer` is pre-allocated before popping from the used ring — no allocation in the interrupt path.

### 2.6 Interrupt moderation

The send queue starts with interrupts **disabled** (`disable_callback()`). Interrupts are only re-enabled when the send queue is full (`available_desc() == 0`). Classic polling-friendly optimization — the network stack polls for completion, interrupts only fire as a backstop.

### 2.7 Softirq-based bottom half

The driver raises softirqs (`raise_send_softirq()` / `raise_receive_softirq()`) rather than processing packets in the hard IRQ handler. Device references use `Arc<SpinLock<dyn AnyNetworkDevice, BottomHalfDisabled>>` — the `BottomHalfDisabled` tag is a compile-time proof that the lock isn't taken from softirq context.

### 2.8 Malfunctioning device detection

`pop_used_with_min_bytes()` validates tokens and lengths: if the device returns an invalid token (out of range) or invalid length (exceeds original buffer or below `min_bytes`), it logs an error and loops — **never propagating garbage to the caller**. Prefers correctness over liveness.

### 2.9 What Asterinas gets right for NeoPlutarch

- **Own virtio crate over external dependency** — reuses kernel's DMA/IRQ primitives
- **Conservative feature negotiation** — start simple, add offload later
- **Pre-allocated RX buffers** — no allocation in interrupt path
- **Interrupt moderation** — disable send IRQ, poll for completion
- **Softirq bottom half** — defer packet processing from hard IRQ

---

## 3. Redox OS — The `virtio-drivers` Crate

**Repo:** `github.com/redox-os/virtio-drivers` (v0.13.0, 299 stars)
**Role:** De facto Rust virtio driver crate. Used by Redox, rCore OS, Toubkal, and others.

### 3.1 Crate architecture

```
src/
  hal.rs          — Hal trait (DMA alloc/dealloc, phys↔virt, share/unshare)
  queue.rs        — VirtQueue<H, SIZE> (split layout)
  transport/      — Transport trait, MmioTransport, PciTransport
  device/net/     — VirtIONetRaw, VirtIONet, RxBuffer, TxBuffer
  device/blk.rs   — VirtIOBlock
  device/gpu/     — VirtIOGPU
  device/console/ — VirtIOConsole
  device/input.rs — VirtIOInput
  device/rng.rs   — VirtIORng
```

### 3.2 Hal trait — the kernel integration point

```rust
pub unsafe trait Hal {
    fn dma_alloc(pages: usize, direction: BufferDirection, access_platform: bool)
        -> (PhysAddr, NonNull<u8>);
    unsafe fn dma_dealloc(paddr: PhysAddr, vaddr: NonNull<u8>, pages: usize, access_platform: bool) -> i32;
    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, size: usize) -> NonNull<u8>;
    unsafe fn share(buffer: NonNull<[u8]>, direction: BufferDirection, access_platform: bool) -> PhysAddr;
    unsafe fn unshare(paddr: PhysAddr, buffer: NonNull<[u8]>, direction: BufferDirection, access_platform: bool);
}
```

This is the **only** interface the crate needs from the kernel. Everything else (queue management, feature negotiation, device logic) is self-contained. The `Dma<H>` wrapper auto-frees on `Drop`.

### 3.3 Transport trait — built-in initialization dance

`Transport::begin_init()` is a default method that performs the full VirtIO initialization: ACKNOWLEDGE → DRIVER → feature negotiation → FEATURES_OK → set page size. `finish_init()` handles DRIVER_OK → interrupt enable. This is a significant convenience — the driver doesn't repeat boilerplate.

### 3.4 Two-tier device API

**`VirtIONetRaw<H, T, QUEUE_SIZE>`** — non-blocking, raw slices, no alloc:
```rust
unsafe fn transmit_begin(&mut self, tx_buf: &[u8]) -> Result<u16>;  // returns token
fn poll_transmit(&mut self) -> Option<u16>;                          // check completion
unsafe fn transmit_complete(&mut self, token, tx_buf) -> Result<usize>;

unsafe fn receive_begin(&mut self, rx_buf: &mut [u8]) -> Result<u16>;
fn poll_receive(&self) -> Option<u16>;
unsafe fn receive_complete(&mut self, token, rx_buf) -> Result<(hdr_len, pkt_len)>;
```

**`VirtIONet<H, T, QUEUE_SIZE>`** — alloc-backed, pre-allocated RX buffers:
```rust
fn new(transport, buf_len: usize) -> Result<Self>;  // pre-fills receive queue
fn receive(&mut self) -> Result<RxBuffer>;           // pop received packet
fn recycle_rx_buffer(&mut self, rx_buf: RxBuffer);   // re-enqueue
fn send(&mut self, tx_buf: TxBuffer) -> Result<()>;  // blocking send
```

The two-tier design lets OS developers choose: `Raw` for no-alloc kernels with custom buffer management, managed for convenience with `alloc`.

### 3.5 VirtQueue implementation

Key features:
- **Shadow descriptors** (`desc_shadow: [Descriptor; SIZE]`) — trusted copy, because the device can't be trusted not to tamper with the DMA'd descriptor table
- **Layout support** — Legacy (single contiguous DMA region) vs Modern (separate driver-to-device and device-to-driver DMA regions)
- **Notification suppression** — `should_notify()` / `set_dev_notify()` for event_idx optimization
- **Indirect descriptors** — configurable at queue creation time

### 3.6 Feature negotiation

```rust
const SUPPORTED_FEATURES: Features = Features::MAC
    .union(Features::STATUS)
    .union(Features::RING_EVENT_IDX)
    .union(Features::RING_INDIRECT_DESC)
    .union(Features::VERSION_1)
    .union(Features::ACCESS_PLATFORM);
```

Conservative but includes event_idx and indirect descriptors for performance. Does NOT negotiate packed rings, in-order, or SR-IOV.

### 3.7 What the crate gets right — and why NeoPlutarch should NOT use it

The `virtio-drivers` crate is well-designed and battle-tested. However, NeoPlutarch already has:
- A virtio-blk driver with its own `Hal`-equivalent DMA abstraction
- A `Transport`-equivalent for virtio device access
- A `VirtQueue`-equivalent for split-ring management

Dependending on `virtio-drivers` would mean either (a) adapting our existing primitives to the crate's `Hal` trait (adding an indirection layer), or (b) maintaining two sets of DMA/queue abstractions. **Asterinas's approach — own the virtio crate as a kernel component — is cleaner for our architecture.**

---

## 4. Toubkal OS — Minimal OS Reference

**Repo:** `github.com/halimbahae/toubkal_os` (8 stars)
**Depends on:** `virtio-drivers 0.7.0`, `smoltcp 0.13.0`

### 4.1 Architecture

Toubkal is the best-documented hobby OS example of the `virtio-drivers` + smoltcp pattern:

```
smoltcp (0.13.0)  ← network protocol stack
    ↑
virtio-drivers (0.7.0)  ← VirtIO guest driver crate
    ↑
Toubkal's VirtioHal  ← hardware abstraction layer
```

### 4.2 Key patterns

- **Identity mapping HAL:** `VirtioHal` assumes virtual == physical addresses. Simple but limits use to early boot or flat memory environments.
- **Global singleton:** `NETWORK_INTERFACE: Lazy<Mutex<Option<VirtIONet<VirtioHal, MmioTransport, 64>>>>` — not composable for multi-NIC.
- **MMIO probing:** Scans `0x0A00_0000 → 0x0A00_3FFF` in `0x200`-byte slots, magic check `0x7472697iv`, dispatches by device ID.
- **Fixed queue size:** `QUEUE_SIZE = 64` compile-time const generic.

### 4.3 What's useful

Toubkal's MMIO probing and device discovery pattern is directly applicable to NeoPlutarch's virtio device enumeration. The `VirtioHal` implementation shows the minimal DMA operations needed — five methods, all straightforward with our page allocator.

---

## 5. Theseus OS — Physical NIC Architecture (No Virtio)

**Repo:** `github.com/theseus-os/Theseus` (branch `theseus_main`)
**Result:** Zero virtio implementation. Networking built for physical Intel ixgbe and Mellanox CX5 NICs.

### 5.1 Architecture

Theseus implements a **hardware NIC driver architecture** with highly generic abstractions over descriptor rings:

```
Applications → kernel/net (smoltcp) → NetworkDevice trait
    → virtual_nic / physical_nic → nic_queues + nic_buffers
    → intel_ethernet / mlx_ethernet → ixgbe / mlx5 drivers
```

### 5.2 Generic descriptor traits

```rust
pub trait RxDescriptor: FromBytes {
    fn init(&mut self, packet_buffer_address: PhysicalAddress);
    fn descriptor_done(&self) -> bool;
    fn end_of_packet(&self) -> bool;
    fn length(&self) -> u64;
}

pub trait TxDescriptor: FromBytes {
    fn init(&mut self);
    fn send(&mut self, addr: PhysicalAddress, len: u16);
    fn wait_for_packet_tx(&self);
}
```

Concrete implementations for Intel legacy (e1000), Intel advanced (ixgbe/82599). The trait-based approach lets queue/buffer/initialization code be shared across NIC families.

### 5.3 Language-level NIC virtualization

Theseus's signature feature: `VirtualNic<S,T,U,V>` takes queues from a physical NIC and gives them to an application with zero-copy, zero-kernel-mediation access. On `Drop`, queues are returned via the `PhysicalNic` trait. Not virtio, but demonstrates Rust's capability to express hardware queue ownership transfer safely.

### 5.4 What's useful for NeoPlutarch

- **Descriptor trait pattern** — generic over descriptor format, not hardcoded to one NIC family
- **`volatile` crate for MMIO** — `Volatile<T>` / `ReadOnly<T>` fields prevent compiler reordering of MMIO accesses
- **Buffer pool with `mpmc::Queue`** — lock-free MPMC queue for receive buffer recycling
- **`IrqSafeMutex`** — disables interrupts during lock acquisition to prevent deadlock

These patterns are relevant for NeoPlutarch's e1000e driver (also Phase 4 per Bible Article 9.10), but not directly for virtio-net.

---

## 6. Hubris OS — Embedded Microcontroller Networking (No Virtio)

**Repo:** `github.com/oxidecomputer/hubris` (Oxide Computer Company, 3,526 stars, MPL-2.0)
**Result:** Zero virtio. ARM Cortex-M microcontroller OS — no PCI, no virtualization.

### 6.1 Architecture

Hubris has a **production-grade networking stack** for Oxide's rack-scale hardware:

| Component | Details |
|---|---|
| Netstack | smoltcp (UDP only, no TCP) |
| Ethernet driver | `drv/stm32h7-eth` — STM32H7 built-in Ethernet MAC with DMA ring buffers |
| PHY drivers | `drv/vsc85xx`, `drv/ksz8463` |
| Switch driver | `drv/vsc7448` — Microchip 10GbE switch |
| Protocol | IPv6 primary, UDP only, ICMP echo, NDP |
| VLAN | Full 802.1Q via DMA context descriptors |

### 6.2 Notification-driven socket model

The `net` task is a centralized network server:
- All sockets statically configured at compile time in `app.toml`
- Each socket has an owner task, TX/RX buffer pools, notification channel
- Packet flow: PHY → MAC DMA → net task (smoltcp parse) → socket buffer → owner task (3 copies per direction)
- No IPC during normal packet processing — only interrupt-driven notifications

### 6.3 What's useful for NeoPlutarch

- **Notification-driven async pattern** — architecture-independent, worth studying for our sockets-equivalent API
- **VLAN handling** — `VLanEthernet` facade pattern is clean
- **DMA ring buffer management** — `drv/stm32h7-eth/ring.rs` descriptor ring code is a good reference for virtio ring design

---

## 7. LionsOS — Negative Result

**Expected repo:** `github.com/LionsOS/Lions`
**Result:** Repository does not exist (404). No Rust OS named "LionsOS" with virtio-net was found on GitHub. The closest match is Toubkal OS (§4 above), which is already covered.

---

## 8. Cross-Cutting Patterns and Anti-Patterns

### 8.1 Universal patterns (every project does these)

1. **Pre-allocated RX buffer pool.** Every project pre-allocates receive buffers at init time. The virtio-net device consumes buffers faster than the kernel can allocate them during packet storms. Pre-allocation is mandatory for correctness.

2. **Token-based buffer lifecycle.** Every enqueue returns a token; every dequeue validates the token. This prevents use-after-free when the device and kernel race on descriptor chains.

3. **Conservative feature negotiation.** No project negotiates checksum offload, TSO, or multiqueue in their initial implementation. All start with MAC + STATUS only, adding features later.

4. **smoltcp as protocol stack.** Every Rust OS uses smoltcp. No one rolls their own TCP/IP for virtio-net.

5. **Queue 0 = RX, Queue 1 = TX.** Universal convention. Multiqueue projects add control queue at index 2.

### 8.2 Divergent patterns (where projects disagree)

| Decision | Asterinas | Redox/virtio-drivers | Toubkal |
|---|---|---|---|
| External crate? | No — own crate | Yes — `virtio-drivers` | Yes — `virtio-drivers` |
| Buffer management | Pooled DMA segments | Pre-alloc + recycle | Pre-alloc + recycle |
| IRQ handling | Softirq bottom half | Not specified | Not specified |
| Thread safety | `BottomHalfDisabled` tag | `!Send + !Sync` | `Mutex` guard |
| Feature set | MAC + STATUS only | MAC + STATUS + event_idx + indirect | Same as crate |

### 8.3 Anti-patterns to avoid

1. **Global singleton NIC.** Toubkal's `Lazy<Mutex<Option<VirtIONet>>>` doesn't scale to multi-NIC. NeoPlutarch needs a device table from day one (Bible Article 9.10 implies multiple NIC classes).

2. **Identity mapping assumption.** Toubkal's HAL assumes virtual == physical. NeoPlutarch has a page table with user/kernel separation — the HAL must do proper phys↔virt translation.

3. **Allocation in interrupt path.** Any design that allocates RX buffers during packet receive will fail under load. Pre-allocation or pooled allocation is mandatory.

4. **Blocking in `receive()`.** smoltcp's `phy::Device::receive()` must return `None` if no packet is available — it must not block. Blocking belongs in the poll loop, not the trait implementation.

---

## 9. Recommendations for NeoPlutarch BUILD-1a

### 9.1 Architecture decision: Own the virtio-net driver

**Do NOT depend on `virtio-drivers`.** Extend our existing virtio-blk abstractions:
- Reuse virtio-blk's DMA abstraction for net buffer pools
- Reuse virtio-blk's Transport trait for device access
- Reuse virtio-blk's VirtQueue for split-ring management
- Add net-specific device logic (header handling, feature negotiation, RX/TX queue pair)

This mirrors Asterinas's approach and keeps the TCB small.

### 9.2 Driver structure

```
kernel/src/drivers/virtio/
  mod.rs          — virtio module root
  hal.rs          — Hal trait (extend from virtio-blk)
  transport.rs    — Transport trait (reuse from virtio-blk)
  queue.rs        — VirtQueue (reuse from virtio-blk)
  net/
    mod.rs        — virtio-net module
    device.rs     — VirtioNetDevice struct
    config.rs     — VirtioNetConfig, feature bitflags
    header.rs     — VirtioNetHdr
    buffers.rs    — RxBuffer, TxBuffer, buffer pools
  blk/            — existing virtio-blk driver
```

### 9.3 Feature negotiation: MAC + STATUS only

Follow Asterinas's conservative approach. No checksum offload, no TSO, no multiqueue for BUILD-1a. Add features later as needed.

### 9.4 Buffer management: Pooled DMA with pre-allocation

Pre-allocate all RX buffers at init time (QUEUE_SIZE = 64). Pre-create replacement buffer before popping from used ring. TX buffers allocated on-demand from pool.

### 9.5 IRQ handling: Softirq bottom half

Follow Asterinas's softirq pattern. Hard IRQ raises softirq; softirq handler processes packets. This keeps hard IRQ latency low and avoids reentrancy issues.

### 9.6 smoltcp integration: Hubris-style wrap

Implement `phy::Device` against our `NetDevice` trait (per Cerebro research §4). The wrap shim is ~95–144 lines. Publication fence lives in the driver, not the wrap.

---

## 10. Open Questions for PM

1. **OQ-1: PCI vs MMIO transport priority.** virtio-net in QEMU defaults to PCI (`-netdev user,id=n0 -device virtio-net-pci,netdev=n0`). Real cloud hypervisors (AWS Nitro, Firecracker) use MMIO via vhost-user. Which transport does BUILD-1a target first?

2. **OQ-2: Queue size.** 64 is universal across all surveyed projects. Is this sufficient for BUILD-1a's throughput targets, or should we start larger (256)?

3. **OQ-3: Control queue.** virtio-net spec defines a control queue (index 2) for VLAN configuration and offload negotiation. Do we need this for BUILD-1a, or is it BUILD-1b+?

4. **OQ-4: Indirect descriptors.** The `virtio-drivers` crate negotiates `RING_INDIRECT_DESC`. This reduces interrupt frequency for multi-buffer chains but adds an extra DMA allocation per chain. Worth it for BUILD-1a?

---

## 11. References

| Project | URL | License |
|---|---|---|
| Asterinas | `github.com/asterinas/asterinas` | MPL-2.0 |
| Redox virtio-drivers | `github.com/redox-os/virtio-drivers` | MIT/Apache-2.0 |
| rCore virtio-drivers | `github.com/rcore-os/virtio-drivers` | MIT/Apache-2.0 |
| Toubkal OS | `github.com/halimbahae/toubkal_os` | MIT |
| Theseus OS | `github.com/theseus-os/Theseus` | MIT |
| Hubris OS | `github.com/oxidecomputer/hubris` | MPL-2.0 |
| smoltcp | `github.com/smoltcp-rs/smoltcp` | BSD-3-Clause |
| virtio spec v1.4 | `docs.oasis-open.org/virtio/virtio/v1.4` | OASIS |
