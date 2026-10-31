# RESEARCH — Phase 4 BUILD-1a: smoltcp v0.13.0 + virtio-net Pre-Implementation Study

| | |
|---|---|
| **Date** | 2026-05-28 |
| **From** | Cerebro (deep research + light source audit) |
| **To** | PM / Jarvis (BUILD-1a implementation substrate) |
| **Dispatch** | `Plans/DISPATCH-2026-05-28-phase-4-build-1a-pre-implementation-research-cerebro.md` |
| **Authority** | `Plans/PL-RATIFICATION-2026-05-28-phase-4-build-1-pm-decision.md` ratifying `PM-DECISION-2026-05-28-phase-4-build-1-opening.md` §4 Q1 (smoltcp = Hubris-style wrap). |
| **Type** | RESEARCH-class artifact. Recommends per the dispatch's explicit request (Q3 publication discipline, Q4 trait shape). Jarvis writes the code; this is substrate, not a commitment. |
| **Gates** | This return opens the Jarvis BUILD-1a implementation gate (`DISPATCH-...-netdevice-virtio-net-smoltcp-wrap-jarvis.md`). |
| **CRITICAL clause status** | **NOT TRIGGERED.** The Q3 virtio-blk audit found no publication-fence bug. No BLOCKER artifact required. See §5. |

---

## 1. Executive Summary

1. **smoltcp 0.13.1 is the dependency.** Pin `smoltcp = { version = "0.13", default-features = false, features = ["medium-ethernet"] }` for BUILD-1a. The current kernel toolchain (`nightly-2026-04-16`) already satisfies smoltcp's hard gate (edition 2024 / MSRV Rust 1.91) — **no toolchain bump needed.**

2. **BUILD-1a does not need smoltcp's `Interface` or `SocketSet` at all.** A raw Ethernet frame round-trip (ISC-1a-3) requires implementing only smoltcp's `phy::Device` trait against our driver. No IP stack, no sockets, no poll machinery. This keeps the BUILD-1a wrap shim to roughly a Hubris-sized `phy::Device` bridge (~95–144 lines) plus a ~30-line poll/IRQ driver — **comfortably under the ~200-line gate.**

3. **virtio-blk's publication discipline is correct — verified line-by-line.** It implements the full virtio split-ring barrier sequence (Release before the avail-idx publish, SeqCst before the notify doorbell, Acquire on completion), which is *stronger* than the single-`Release` AHCI/NVMe doorbell pattern, not weaker. **No hidden AHCI-class bug. The CRITICAL clause does not fire.** virtio-net must inherit virtio-blk's three-barrier discipline (not a naive single-Release port).

4. **The `NetDevice` trait should be object-safe and net-stack-agnostic, and the smoltcp wrap should *implement* `phy::Device` against it** (the Hubris `Smol` pattern). The publication fence lives inside the `NetDevice` driver methods, never in the wrap. This parallels `BlockDevice` (Article 11.2's "smoltcp-derived *or* original" stays open).

5. **One forward marker worth PM's eye (not a blocker):** smoltcp 0.13.0 *does* perform SLAAC (Hubris's 0.9.1 could not), but it does **not** do RFC 8981 Privacy Extensions, RFC 8106 RDNSS, or (apparently) full DAD. BUILD-1b therefore faces a real choice: lean on smoltcp's EUI-64 SLAAC and layer privacy/RDNSS/DAD on top, or own SLAAC outright and feed addresses via `update_ip_addrs`. This does not affect any ratified BUILD-1 position; it is a BUILD-1b dispatch-time decision (§5 open question OQ-1).

---

## 2. Q1 — smoltcp v0.13.0 surface for in-kernel embedding

**Verdict line:** Depend on **smoltcp 0.13.1** (`version = "0.13"`, `default-features = false`); BUILD-1a needs only the `medium-ethernet` feature and only the `phy::Device` trait. Poll-driven, allocator-free with fixed storage, MSRV satisfied by our toolchain.

### 2.1 Minimal surface for the Ethernet round-trip (BUILD-1a)

The `phy::Device` trait is standalone — it has no dependency on `Interface` or `SocketSet`. Implementing it for our driver, plus a loop/IRQ that calls `receive`/`transmit`, is the *entire* surface ISC-1a-3 needs. The v0.13.0 definitions:

```rust
pub trait Device {
    type RxToken<'a>: RxToken where Self: 'a;
    type TxToken<'a>: TxToken where Self: 'a;
    fn receive(&mut self, timestamp: Instant)
        -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)>;   // (rx, tx) so stack can reply; None if RX empty
    fn transmit(&mut self, timestamp: Instant)
        -> Option<Self::TxToken<'_>>;                        // None if TX ring full
    fn capabilities(&self) -> DeviceCapabilities;
}

pub trait RxToken { fn consume<R, F>(self, f: F) -> R where F: FnOnce(&[u8]) -> R; /* + meta() default */ }
pub trait TxToken { fn consume<R, F>(self, len: usize, f: F) -> R where F: FnOnce(&mut [u8]) -> R; /* + set_meta() default */ }
```

- `RxToken::consume` hands the closure an immutable `&[u8]` of the received frame (read once; token is moved).
- `TxToken::consume` takes `len` up front (reserve exactly `len` from the TX ring), then the closure writes the frame into a `&mut [u8]`; afterwards the driver kicks the TX ring.
- `meta()`/`set_meta()` have default impls — **we do not implement them.**

`DeviceCapabilities` is `#[non_exhaustive]` — build with `DeviceCapabilities::default()` then set fields. Set `caps.medium = Medium::Ethernet` **explicitly** (do not rely on the `Default` variant) and `caps.max_transmission_unit = mtu`. `Medium::{Ethernet, Ip, Ieee802154}` all exist in 0.13.0; we use `Ethernet`. Checksum capabilities are irrelevant at BUILD-1a (no IP) — leave default (software).

### 2.2 Larger surface for BUILD-1b (IPv6) — what smoltcp does vs what we own

**smoltcp 0.13.0 added IPv6 SLAAC** (the headline 0.13.0 feature; gated by cargo feature `proto-ipv6-slaac` *and* runtime `iface::Config.slaac = true`). With it enabled, smoltcp processes Router Advertisements and self-assigns addresses; `Interface::slaac_updated_at()` introspects state.

| Capability | smoltcp 0.13.0 | NeoPlutarch implements |
|---|---|---|
| ICMPv6 echo (+ `auto-icmp-echo-reply`) | ✅ yes | — |
| Neighbor Discovery NS/NA (address resolution) | ✅ yes | — |
| Router Solicitation/Advertisement parsing | ✅ yes (with SLAAC) | — |
| SLAAC autoconfig (EUI-64 / configured IID) | ✅ yes (opt-in) | — (or override; see OQ-1) |
| **RFC 8981 Privacy Extensions (temp addrs)** | ❌ no | ✅ **us** |
| **RFC 8106 RDNSS option lift** | ❌ no | ✅ **us** (per Q2 ratification) |
| **DAD (RFC 4862)** | ⚠️ not confirmed present / likely "optimistic" | ✅ likely **us** — verify against source at BUILD-1b |

Address-config surface for when we configure manually: `Config::new(hw_addr)` then set `.slaac`; `Interface::new(config, &mut device, now)`; `Interface::update_ip_addrs(|addrs| { ... })` (a `heapless::Vec<IpCidr, IFACE_MAX_ADDR_COUNT>`); `wire::{Ipv6Address, Ipv6Cidr, IpCidr}`; `HardwareAddress::Ethernet(EthernetAddress)`.

### 2.3 Poll-loop posture (R1)

smoltcp is **purely poll-driven** — no internal threads, timers, or IRQ hooks. The embedder calls:

```rust
fn poll(&mut self, timestamp: Instant, device: &mut (impl Device + ?Sized), sockets: &mut SocketSet) -> PollResult
// PollResult ∈ { None, SocketStateChanged }
fn poll_at(&mut self, ts, sockets) -> Option<Instant>     // arm a timer to this deadline
fn poll_delay(&mut self, ts, sockets) -> Option<Duration> // …or this relative delay; None = wake only on RX
```

Drive `poll` on three triggers: (1) RX interrupt, (2) the timer armed from `poll_at`/`poll_delay`, (3) after writing to a socket. v0.13.0 **exhausts egress within a single `poll`** (it pulls `TxToken`s until the send queue drains or `transmit()` returns `None`) — so `Device::transmit` may be called repeatedly per poll; the TX path must be re-entrant within one poll and defer cleanly when the ring is full.

`smoltcp::time::Instant` is an `i64`-microsecond wrapper. `Instant::now()` is **std-only** — in-kernel we never call it; we supply time on every call via `Instant::from_micros(<monotonic clock since boot>)` (TSC/HPET). smoltcp never reads a clock itself.

This composes cleanly with the existing scheduler: the virtio-blk completion model (`sched::spawn` a kernel thread that loops `poll_all_completions(); yield_now()`, plus an IRQ entry point) is a direct template for an RX-IRQ-driven `iface.poll()` driver. **BUILD-1a does not exercise the poll loop at all** (no `Interface`), so the R1 early-gate is genuinely about the `phy::Device` bridge fit, which is low-risk.

### 2.4 no_std edges

- **Default features pull in `std` + Linux host phy backends (`phy-raw_socket`, `phy-tuntap_interface`).** Mandatory: `default-features = false`.
- **No global allocator required.** There is no `alloc` feature in 0.13.0's manifest; storage is fixed/bounded via `managed` (0.8) + `heapless` (0.9). (Our kernel has an allocator anyway, but the core stack does not demand one.)
- **No `libm`; integer-only core.** The only float path is CUBIC congestion control behind `socket-tcp-cubic` — **do not enable it** (Reno is integer-only and is the default CC).
- **Edition 2024 / MSRV Rust 1.91.** Hard gate. Our `rust-toolchain.toml` pins `nightly-2026-04-16`, which is far newer — **satisfied, no action.** (A crate's edition is per-crate; smoltcp being edition-2024 does not force our crate's edition.)
- **`heapless` 0.9 bump** — if the kernel depends on `heapless` directly elsewhere, align on 0.9 to avoid a duplicate-version conflict. *(Jarvis: check the kernel Cargo tree before adding.)*

### 2.5 Breaking changes vs 0.12.x (embedder-relevant)

(1) edition2024 / MSRV1.91 gate; (2) `consume()` closures now return `R` directly (no more always-`Ok` `Result`); (3) `Interface::poll` returns `PollResult` not `bool`; (4) egress draining on poll (see 2.3); (5) `heapless` 0.9; (6) `Config` gained `slaac: bool` and is `#[non_exhaustive]`. For BUILD-1a only (1) and (2) matter.

### 2.6 Version

Latest published is **0.13.1** (strict bugfix over 0.13.0: fixes a TCP SYN-from-unspecified-addr panic, a SACK seq-overflow panic, and an IPv4 IHL check — two are reachable from adversarial input, which we want patched in a kernel). No 0.14 exists. **Pin `"0.13"`** to float to 0.13.1+. *(Date note: web tooling rendered "2026" release dates; the authoritative 0.13.0 date is 2025-03-20. Immaterial to the version choice.)*

---

## 3. Q2 — Hubris `task/net` wrap pattern

**Verdict line:** Hubris validates "wrap, don't fork" and a ~95–144-line `phy::Device` bridge — but Hubris runs smoltcp **0.9.1** (pre-SLAAC), is UDP-only, and deliberately ignores `poll_at()`. Borrow the bridge shape; do not inherit the UDP-only shortcuts.

### 3.1 Code map (exact `wc -l` on `oxidecomputer/hubris@master`)

| File | Lines | Role |
|---|---:|---|
| `task/net/src/main.rs` | 350 | Entry, HW bringup, **the ~30-line poll loop** (≈ lines 230–290) |
| `task/net/src/server.rs` | 863 | Generic server: idol impl, `poll()` driver, `SocketSet` mgmt, `DeviceExt`, VLAN/trust/watchdog |
| `task/net/src/server_basic.rs` | 144 | **`Smol` `phy::Device` bridge** + `new()` storage wiring (pure bridge ≈ 95 lines) |
| `task/net/src/server_vlan.rs` | 134 | `VLanEthernet` `phy::Device` bridge variant |
| `task/net/src/bsp_support.rs` | 117 | `Bsp` board-hooks trait |
| `task/net/src/mgmt.rs` | 378 | VSC85xx management-PHY logic (not shim) |

The MAC driver itself is a separate crate (`drv-stm32h7-eth`). The bridge "was once in that crate but became a little gross," so Hubris newtypes it in `task/net`.

### 3.2 Shim shape and the ~200-line gate

The directly-analogous "wrap shim" is the `phy::Device` bridge: **~95–144 lines in one file.** The poll-loop driver adds ~30 lines. Counting smoltcp lifecycle management (`poll()`, `SocketSet` construction, recv/send copy paths) but *excluding* Hubris-specific concerns (VLAN trust, KSZ8463, the stuck-queue watchdog, idol boilerplate), the essential smoltcp-touching code is **~250–350 lines**.

**Implication for the gate:** the PM-DECISION §6 ~200-line gate is realistic *for BUILD-1a*, because BUILD-1a needs only the bridge + a poll/IRQ driver (no `Interface`/`SocketSet`/socket plumbing per §2.1). The full IPv6/socket shim (BUILD-1b/1c) will approach Hubris's ~250–350 once the `Interface` + `SocketSet` driver lands — flag at BUILD-1b that the gate is a BUILD-1a-scoped signal, not a whole-Phase-4 budget.

### 3.3 The bridge is a named newtype (validates the Q4 architecture)

`struct Smol<'d> { eth: &'d eth::Ethernet }` implements `phy::Device`; `OurRxToken`/`OurTxToken` implement the token traits, delegating `consume` to `eth.recv(f)` / `eth.try_send(len, f)`. `transmit()` returns `Some` only when the driver `can_send()`. This is exactly the Q4 recommendation: **the wrap implements `phy::Device` against the lower driver abstraction.**

### 3.4 Fully wrapped — clients never see smoltcp

Hubris's client API is the `Net` idol interface (`send_packet`/`recv_packet` with a `SocketName` enum + leased bytes + `UdpMetadata`). No `Interface`/`Device`/`SocketSet`/`SocketHandle` crosses the boundary. Access is a compile-time owner check (`SOCKET_OWNERS[i] == sender`).

### 3.5 Deltas Hubris does NOT handle that NeoPlutarch must

1. **No dynamic socket allocation** — the whole socket set is a const table generated from `app.toml`; NeoPlutarch needs a runtime `socket/bind/close` lifecycle and handle recycling (BUILD-1c).
2. **No capability-scoped access** — only a compile-time owner allow-list; NeoPlutarch needs per-socket capability objects with passing/derivation/revocation (BUILD-1c).
3. **No fairness across many clients** — fixed ~6–8 sockets; NeoPlutarch needs backpressure/fairness for an open client population.
4. **Addresses pinned statically** (EUI-64 link-local, one `update_ip_addrs` at construction) — NeoPlutarch needs SLAAC + runtime address mutation (BUILD-1b).
5. **UDP-only → `poll_at()` sidestepped** — Hubris eager-polls + RX-IRQ + coarse wake timer and never calls `poll_at()` because there are no TCP timers. **NeoPlutarch's shim MUST honor `poll_at()` and arm a real timer before TCP (BUILD-3).** Flag this now so the BUILD-1a poll driver is structured to grow into it.
6. **No multi-interface / routing**, **fixed per-socket buffer sizing**, and a **stuck-TX-queue watchdog** (smoltcp#594 workaround) — all deployment-specific to Oxide hardware.

### 3.6 Wrap-not-fork confirmed

Workspace `Cargo.toml` line 129: `smoltcp = { version = "0.9", default-features = false, features = ["proto-ipv6","medium-ethernet","socket-udp","async"] }`; `Cargo.lock` resolves **0.9.1** from crates.io (checksum present, not a git/path fork). **Note the era gap:** Hubris's 0.9.1 predates SLAAC — which is *why* it pins addresses statically. NeoPlutarch on 0.13.1 has options Hubris never had.

---

## 4. Q3 — virtio-net publication-fence verification + virtio-blk audit

**Verdict line:** virtio-blk's publication discipline is **correct and spec-complete** (verified line-by-line). The AHCI-class missing-fence bug is **NOT present**. The CRITICAL clause does **NOT** trigger. virtio-net must reuse virtio-blk's exact three-barrier discipline; a naive single-`Release` port would reintroduce the BUILD-5b hazard.

### 4.1 The hazard, restated

On real hardware, virtio split-ring descriptors + the available ring live in main memory read by the device via DMA; the notify is an MMIO store. Two orderings must hold or the device can act on stale/partial state (the exact AHCI BUILD-5b failure mode — wrong-LBA / `.rodata` over-DMA, invisible under QEMU's TSO timing):
- **(P1)** descriptor + avail-ring stores must be globally visible *before* the `avail->idx` store. → **Release fence.**
- **(P2)** the `avail->idx` store must be visible *before* the notify MMIO write (and the notify must not pass it). → **store-load barrier = SeqCst / `mfence`**, because a `Release` fence does not prevent the idx store from being reordered after a subsequent load of suppression state, and on x86 store→load is the one reordering hardware permits.
- **(C1)** on completion, the `used->idx` load must happen-before reads of the device-written used element / DMA'd payload. → **Acquire fence.**

### 4.2 virtio-blk audit (`kernel/src/drivers/block/virtio_blk.rs`) — CLEAN

The submission path `VirtioBlkInner::submit_pending` (lines 559–645):

```
write descriptor table entries (575–615)  +  write avail-ring entry (618)
  → descriptor_publish_barrier()           // line 619  → virtio_barrier::publish_descriptors() = fence(Release)   [P1 ✔]
write avail->idx (620–621)
  → notification_suppression_barrier()      // line 622  → virtio_barrier::before_notification_check() = fence(SeqCst) [P2 ✔]
notify doorbell MMIO write (644)
```

The completion path `poll_used` (647–697):

```
read used->idx (653); if advanced →
  → used_ring_acquire_barrier()             // line 657  → fence(Acquire)                                          [C1 ✔]
read used element id/len (660–661); read device-written status/data
```

The shared barrier helpers (`kernel/src/drivers/block/virtio_barrier.rs`):

```rust
pub fn publish_descriptors()       { fence(Ordering::Release); }   // step-3 / P1
pub fn before_notification_check() { fence(Ordering::SeqCst);  }   // step-5 / P2
```

This maps **exactly** onto the virtio 1.x spec §2.7.13 prescribed barrier placement (step-4 barrier before the idx publish; step-6 barrier before the notify). All descriptor/ring/MMIO accesses use `write_volatile`/`read_volatile` (compiler-reordering/elision protection per access); the explicit fences provide the inter-observer ordering. **This is stronger than the single-`Release`-before-doorbell discipline of NVMe/AHCI**, because virtio's two-stage publish (descriptors→idx, idx→notify) requires the second (SeqCst) barrier that a single-doorbell device does not.

### 4.3 Reference patterns confirmed (the discipline virtio-blk matches)

- **NVMe** (`nvme.rs:485–496`): copy SQ entry → `fence(Release)` (487) → SQ-tail doorbell. Completion: phase-bit match → `fence(Acquire)` (503) → read completion. *The canonical model.*
- **AHCI** (`ahci.rs:417–447`, commit `2fdb7cb`): `write_prdt` → `fence(Release)` (423, with the explicit "match the NVMe doorbell path's release publication discipline so real hardware cannot observe stale LBA/PRDT/source fields" comment) → PX_CI doorbell. Completion → `fence(Acquire)` (447).

### 4.4 Recommended virtio-net publication discipline

virtio-net is a virtio split-ring device — it inherits virtio-blk's discipline verbatim, on **both** queues:

- **TX submit** (and RX-buffer posting, which is structurally identical — the driver hands the device empty *writable* descriptors): write descriptors + avail-ring entry → `fence(Release)` → write `avail->idx` → `fence(SeqCst)` → notify doorbell.
- **RX consume / TX completion**: observe `used->idx` advance → `fence(Acquire)` → read used element + the DMA'd packet payload. The Acquire is load-bearing on RX: it guarantees the device's DMA write of the frame body is visible before the driver reads it.

**Reuse, do not reinvent:** the two `virtio_barrier.rs` one-liners + a local `fence(Acquire)` are the complete primitive set — no new barrier code. The module currently lives under `drivers/block/`; either promote it to a shared location (e.g. `kernel/src/drivers/virtio_barrier.rs`) for `drivers/net/` to import, or duplicate the two trivial functions in a net-local barrier module. The dispatch's "copy the discipline, not the file" guidance makes either acceptable; promotion is the cleaner long-term shape. **Do not modify virtio-blk's use of it** (Article-15.7 / lean-patch; out of BUILD-1a scope).

**Anti-pattern to avoid:** a naive port that uses a single `fence(Release)` before the notify (mirroring NVMe/AHCI's single doorbell) would skip the (P2) SeqCst barrier and reintroduce the BUILD-5b hazard — invisible on QEMU, live on real Intel hardware. virtio-net is two-stage; it needs both barriers.

---

## 5. Q4 — Recommended `NetDevice` trait shape

**Verdict line:** An **object-safe, net-stack-agnostic `NetDevice`** trait paralleling `BlockDevice`, with the publication fence inside its method bodies; the smoltcp wrap is a separate bridge type that **implements `phy::Device` against `NetDevice`**. Buffer-handle (not closure) methods keep `Arc<dyn NetDevice>` object-safe while still mapping cleanly onto smoltcp's closure-based tokens.

### 5.1 Why this shape (three decisions)

1. **`NetDevice` is net-stack-agnostic** (no smoltcp types in its signatures). Article 11.2 keeps "smoltcp-derived *or* original" open; a clean trait preserves that. This is the BlockDevice parallel the dispatch requires — `BlockDevice` is storage-stack-agnostic.
2. **The wrap implements `phy::Device` against `NetDevice`** (the Hubris `Smol` pattern, §3.3) — *not* `phy::Device` directly on the driver (Hubris found that "gross"), and *not* `NetDevice` on top of `phy::Device`. The wrap is the thin adapter; the driver is the substrate.
3. **The publication fence (Q3) lives inside `NetDevice`'s `tx_submit`/`rx_take` bodies, never in the wrap.** The wrap stays trivial and stack-swappable; memory ordering stays in one auditable place (the driver), exactly as virtio-blk keeps it in `submit_pending`/`poll_used`.

### 5.2 Object-safety wrinkle (the load-bearing design note)

smoltcp's tokens use **generic** `consume<F>` methods. A `dyn NetDevice` trait object **cannot have generic methods**. Resolve this by giving `NetDevice` **buffer-handle** methods (non-generic, object-safe) and letting the *wrap's* token impls own the generic closure:

```rust
// kernel/src/net/device.rs  — SKETCH, not a Rust file (anti-scope §4)
pub trait NetDevice: Send + Sync {
    fn identity(&self) -> &NetDeviceIdentity;     // driver/name/pci_bdf — parallels BlockDeviceIdentity
    fn mac_address(&self) -> [u8; 6];             // smoltcp HardwareAddress + future EUI-64/SLAAC
    fn link_state(&self) -> LinkState;            // ISC-1a-1: link observed in serial log
    fn mtu(&self) -> usize;                        // → DeviceCapabilities.max_transmission_unit

    /// Reserve a TX buffer (size ≥ MTU). None if the TX ring has no free slot
    /// (smoltcp's transmit() returns None → retried next poll). Object-safe handle.
    fn tx_reserve(&self) -> Option<TxFrame<'_>>;   // TxFrame derefs to &mut [u8]
    /// Publish a filled TX frame of `len` bytes. INTERNALLY performs the virtio
    /// discipline: write desc + avail entry → fence(Release) → write avail->idx
    /// → fence(SeqCst) → notify doorbell.  (Q3)
    fn tx_submit(&self, frame: TxFrame<'_>, len: usize) -> NetResult<()>;

    /// Take the next received frame. INTERNALLY: read used->idx; if advanced →
    /// fence(Acquire) → read used elem. RxFrame derefs to &[u8]; Drop recycles
    /// the buffer back onto the RX ring (re-posts to the device).  (Q3)
    fn rx_take(&self) -> Option<RxFrame<'_>>;

    /// Drain completed TX descriptors / re-arm RX. Called from the NIC IRQ or
    /// the poll driver. Mirrors virtio-blk poll_used().
    fn service(&self);

    /// Register a wake token fired by the RX-available / TX-complete IRQ, so the
    /// poll loop is driven by device activity instead of busy-polling.
    fn set_activity_token(&self, token: ThreadWakeToken);
}
```

```rust
// kernel/src/net/smoltcp_wrap.rs  — SKETCH
pub struct SmolDevice<'d> { dev: &'d dyn NetDevice }

impl<'d> smoltcp::phy::Device for SmolDevice<'d> {
    type RxToken<'a> = NetRxToken<'a> where Self: 'a;
    type TxToken<'a> = NetTxToken<'a> where Self: 'a;
    fn receive(&mut self, _t: Instant) -> Option<(NetRxToken<'_>, NetTxToken<'_>)> {
        let rx = self.dev.rx_take()?;                 // fence(Acquire) inside
        Some((NetRxToken(rx), NetTxToken { dev: self.dev, frame: self.dev.tx_reserve() }))
    }
    fn transmit(&mut self, _t: Instant) -> Option<NetTxToken<'_>> {
        let frame = self.dev.tx_reserve()?;           // None → ring full → smoltcp retries
        Some(NetTxToken { dev: self.dev, frame: Some(frame) })
    }
    fn capabilities(&self) -> DeviceCapabilities {
        let mut c = DeviceCapabilities::default();
        c.medium = Medium::Ethernet;                  // explicit (Q1 §2.1)
        c.max_transmission_unit = self.dev.mtu();
        c
    }
}
impl RxToken for NetRxToken<'_> {
    fn consume<R, F: FnOnce(&[u8]) -> R>(self, f: F) -> R { f(&self.0) } // RxFrame Drop recycles
}
impl TxToken for NetTxToken<'_> {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(mut self, len: usize, f: F) -> R {
        let mut frame = self.frame.take().expect("reserved in transmit()");
        let r = f(&mut frame[..len]);
        let _ = self.dev.tx_submit(frame, len);       // fence(Release)/SeqCst + doorbell inside
        r
    }
}
```

Reserving the TX buffer in `transmit()` (not in `consume`) makes the ring-full case return `None` correctly and guarantees `consume` cannot fail — `consume` must return `R`, so it must not be able to fail to get a buffer.

### 5.3 BUILD-1a usage (no `Interface`)

The ISC-1a-3 smoke test goes *through the wrap* but not through an `Interface`: construct `SmolDevice`, call `transmit().consume(len, |buf| write_test_frame(buf))` (frame reaches the TX ring + emerges on QEMU's backend), then on RX call `receive()` → `rx.consume(|frame| assert/echo)`. This exercises the bridge and the fence discipline without any IP/socket code — the minimal R1 early-gate.

### 5.4 Forward markers

- **FM-A (BUILD-1b, SLAAC):** `NetDevice` stays L2 (frames only); addresses live in the stack layer. `mac_address()` is already on the trait for HardwareAddress/EUI-64. The smoltcp-SLAAC-vs-own-SLAAC choice is OQ-1 below.
- **FM-B (BUILD-3, TCP):** the poll driver must gain `poll_at()`-driven timer arming (Hubris delta §3.5 #5). Structure the BUILD-1a poll/IRQ driver so a timer source can be added without reshaping it.
- **FM-C (multi-queue):** single TX + single RX queue at BUILD-1a (per Jarvis dispatch §4). If multi-queue ever lands, `NetDevice` grows a queue index / per-queue handle.
- **FM-D (offload):** `capabilities()` reports software checksums at BUILD-1a; when offload features are negotiated later, `NetDevice` grows offload accessors and `ChecksumCapabilities` reflects them. Do not negotiate `VIRTIO_NET_F_*` offload/MRG_RXBUF features at BUILD-1a (Jarvis dispatch ISC-1a-1).
- **FM-E (`virtio_barrier` location):** promote `drivers/block/virtio_barrier.rs` to a shared `drivers/` module, or duplicate the two one-liners net-side (§4.4).

---

## 6. References

**smoltcp (Q1):**
- `phy::Device` / `RxToken` / `TxToken` / `Medium` / `DeviceCapabilities` — https://docs.rs/smoltcp/0.13.0/smoltcp/phy/index.html
- `iface::Interface` / `Config` / `PollResult` — https://docs.rs/smoltcp/0.13.0/smoltcp/iface/index.html
- `time::Instant` — https://docs.rs/smoltcp/0.13.0/smoltcp/time/struct.Instant.html
- CHANGELOG — https://github.com/smoltcp-rs/smoltcp/blob/main/CHANGELOG.md
- v0.13.0 Cargo.toml (features, edition, MSRV) — https://raw.githubusercontent.com/smoltcp-rs/smoltcp/v0.13.0/Cargo.toml
- crates.io versions (0.13.1 latest) — https://crates.io/crates/smoltcp

**Hubris (Q2):**
- `task/net/src/{main,server,server_basic,server_vlan}.rs` — https://github.com/oxidecomputer/hubris/tree/master/task/net/src
- workspace `Cargo.toml` line 129 (smoltcp 0.9) + `Cargo.lock` (0.9.1) — https://github.com/oxidecomputer/hubris
- `idl/net.idol` (client API) — https://raw.githubusercontent.com/oxidecomputer/hubris/master/idl/net.idol

**Local source (Q3/Q4):**
- `kernel/src/drivers/block/virtio_blk.rs` (submit_pending 559–645; poll_used 647–697; barriers 619/622/657/994–1006)
- `kernel/src/drivers/block/virtio_barrier.rs` (publish_descriptors=Release; before_notification_check=SeqCst)
- `kernel/src/drivers/block/ahci.rs` (fence(Release) 423 / fence(Acquire) 447; commit `2fdb7cb`)
- `kernel/src/drivers/block/nvme.rs` (fence(Release) 487 / fence(Acquire) 503)
- `kernel/src/drivers/block/mod.rs` (`BlockDevice` trait 63–94 — the parallel for `NetDevice`)
- `rust-toolchain.toml` (`nightly-2026-04-16`)
- `docs/bible.md` Article 11.2 (networking), Article 9.10 (virtio-net required Phase 4), Article 17 (`kernel/src/net/` subsystem layout)

**Spec / RFC:**
- virtio 1.x §2.7.13 "Supplying Buffers to The Device" (split-ring publish barrier placement).
- Prior substrate: `Plans/OBSERVE-2026-05-20-phase-4-network-stack-survey-cerebro.md`.

---

## 7. Open Questions for PM

- **OQ-1 (BUILD-1b, forward — not a blocker, not CRITICAL):** smoltcp 0.13.0 can do SLAAC itself (EUI-64), but lacks RFC 8981 Privacy Extensions, RFC 8106 RDNSS, and (apparently) full DAD — all of which BUILD-1b's ISCs require us to own. Does BUILD-1b lean on smoltcp's SLAAC and layer privacy/RDNSS/DAD over it, or own SLAAC outright and feed addresses via `update_ip_addrs`? This affects the BUILD-1b dispatch shape; it does **not** affect any ratified BUILD-1 position (the wrap stands; RDNSS-lift-by-us stands; the TCP sketch stands). Recommend resolving at the BUILD-1b dispatch, not now.
- **OQ-2 (housekeeping):** `virtio_barrier.rs` lives under `drivers/block/`. Promote to a shared `drivers/` module for net reuse (clean), or duplicate the two one-liners net-side (matches "copy the discipline, not the file")? Jarvis's call unless PM has a preference. (§4.4 / FM-E.)
- **OQ-3 (BUILD-3, forward):** Hubris never honors `poll_at()` because it is UDP-only; our TCP work will require it. Confirm the BUILD-1a poll/IRQ driver is reviewed for "can grow a timer source" at BUILD-1a close, so BUILD-3 is not a reshape. (FM-B.)

**No CRITICAL open questions. The Q3 virtio-blk audit is clean; no BLOCKER artifact is filed.**

---

**End of research return.**

*Cerebro: smoltcp 0.13.1 (`medium-ethernet`, `default-features=false`); BUILD-1a needs only `phy::Device`; toolchain already satisfies MSRV. Hubris bridge ~95–144 lines validates the ~200-line gate. virtio-blk publication discipline verified CLEAN — CRITICAL clause not triggered. `NetDevice` = object-safe, fence-internal, stack-agnostic; smoltcp wrap implements `phy::Device` against it. Three forward open questions, none blocking. Jarvis is cleared to start.*
