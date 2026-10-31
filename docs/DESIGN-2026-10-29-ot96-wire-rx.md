<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DESIGN: OT-96, wire receive after bring-up (IPv4/UDP/TCP demux, an RX context, DHCP renewal)

**Date:** 2026-10-29 · **Seat:** S4 · **Lane:** OT-96 (PM, 2026-10-29) · **Status:** design, for PM review before any build.

## 1. The gap, as found on #215

On every NIC the product drives, frames are received only inside a bring-up loop, and only ARP and DHCP are routed:

| NIC | Where RX is polled | What the demux routes |
|---|---|---|
| rtl8125 (`drivers/net/rtl8125.rs`) | the DHCP loop and the ARP probe in `rtl8125::init` (`pump_once`) | ARP (`arp::inject`), DHCP (`dhcpv4::demux_eth_rx`) |
| CDC-ECM / `boot_eth0` (`drivers/usb/boot_eth0.rs`) | its bring-up loop | the same |
| mt7601u (`drivers/usb/mt7601u/mod.rs`) | its association loop | DHCP |
| virtio-net | harness scenarios only (`smoltcp_wrap` smoke) | — |

- **Nothing in the stack accepts an inbound IPv4 packet.** `tcp::inject_segment` is reached only from `tcp::pump_loopback_egress`, which runs in syscall context, and from the harness.
- **No UDP socket receives from the wire.** `Socket::enqueue` is reached only by a local `send_to`.
- **`phase4_https_fetch` is loopback:** neo-worker serves TLS on 127.0.0.1.

So "wired networking" on the product is a DHCP lease plus ARP at boot. Once #215's `net-tick` runs, the lease also expires, because the client never renews. That is why #215 keeps `net-tick` out of image A.

**Goal of OT-96:** a frame that arrives after boot reaches the right protocol (ARP, IPv4/UDP to a socket or the DHCP client, IPv4/TCP to `tcp::inject_segment`, ICMP echo), and the DHCP lease renews. Then image A gets `net-tick` and renewal together.

## 2. Slices (dependency order)

| Slice | What | Depends on | Gate |
|---|---|---|---|
| **R1** | `net::rx`: one demux for an Ethernet frame. Ethertype to ARP / IPv4 / IPv6 (NDP and RA only); IPv4 header validation; UDP to the DHCP client, then the bound socket; TCP to `tcp::inject_segment`; ICMP echo request to an echo reply. | — | host KATs with hand-laid byte oracles (format rule); kernel unit tests |
| **R2** | an RX service context: a `net-rx` kthread that services every registered `NetDevice` after bring-up and hands each frame to R1. Polled first (a 1-tick nap). | R1 | QEMU two-NIC hub scenario (§6) |
| **R3** | DHCP renewal: RENEWING (unicast REQUEST at T1), REBINDING (broadcast at T2), NAK back to INIT and a new DISCOVER; plus `net-tick` into image A. | R2, #215 | a scenario with a short lease renewed across its expiry (§6) |
| **R4** | interrupt-driven wake: MSI for rtl8125 (and virtio-net). The ISR acks, masks and wakes the R2 thread; storms fall back to polled. | R2 | QEMU (virtio-net MSI) plus the bare-metal run |
| **R5** | bare-metal acceptance on the MS-7E57 (rtl8125, `dma=arena-only` opt-in): lease renewed past T1, ping from a LAN host, a TCP connect from a LAN host to a listener. | R1–R4 | the acceptance bar, not the merge bar |

R1 and R2 make OT-96's core. R3 is what unblocks #215's image-A change. R4 is a latency and CPU improvement; R2's polling is correct without it.

## 3. R1: the demux

`net::rx::deliver(interface_id, frame: &[u8])`, called with a frame the device no longer owns (§5):

1. **Ethernet.** Need at least 14 bytes. Drop 802.1Q-tagged frames (0x8100) in v1 (trap R4).
   - 0x0806 → `arp::inject`.
   - 0x0800 → IPv4.
   - 0x86DD → the existing NDP and RA inject paths only.
   - Anything else is counted and dropped.
2. **IPv4.** Drop unless all of these hold:
   - version 4;
   - IHL ≥ 5 and IHL·4 ≤ the packet;
   - total length ≤ the frame and ≥ the IHL;
   - a header checksum that verifies;
   - not a fragment (MF set, or a non-zero offset, in v1);
   - destination is this interface's address, its subnet broadcast, or 255.255.255.255;
   - a source that is not broadcast or multicast.

   Every length the frame claims is bounded by the bytes actually present before any slice is built (I-14(c): device-written data is untrusted).
3. **UDP (17).**
   - Verify the length, and the checksum when non-zero.
   - Destination port 68 → `dhcpv4` (the existing `demux_eth_rx` logic, reached through here).
   - Otherwise deliver to the socket bound to (dst, port), or to the wildcard bind. This uses a crate-visible `socket::deliver_inbound` built on `Socket::enqueue`, which already bounds the queue (`RX_QUEUE_LIMIT`). With no bound socket, drop it; an ICMP port-unreachable reply is deferred.
4. **TCP (6).** Pass the TCP segment to `tcp::inject_segment(src, dst, segment)`. That already verifies the checksum with the pseudo-header (`tcp.rs` `parse_segment`) and has OT-94's half-open and accept bounds (#211). A RST is never answered with a RST (#211).
5. **ICMP (1).**
   - An echo request to us gets an echo reply, rate-limited per second (a flood is bounded, I-14(b)).
   - Everything else is dropped.

**Byte oracle (required by the format rule).** A hand-laid frame per path goes in a host KAT crate (`tools/net-rx-logic-tests`): a DHCPACK in Ethernet/IPv4/UDP, a TCP SYN, an ICMP echo request, and one of each malformed case (bad IHL, total length past the frame, bad checksum, fragment, tagged frame). The parser is pure logic (`net/rx_logic.rs`), kernel-free, so it runs on the host.

## 4. R2: the RX context, and I-14(a)

- **Who runs the demux.** A `net-rx` kernel thread, never an interrupt handler. Each pass, for every registered device:
  1. `service()` (ack status, reap both rings, refill);
  2. drain up to a per-pass frame budget (the existing `PUMP_FRAME_BUDGET`, 24);
  3. hand each frame to R1;
  4. `rx_recycle` it.

  Then it naps. In R2 that is `tick_sleep` for one tick (10 ms); in R4 it parks until the ISR wakes it.
- **Locks (I-14(a)).**
  - The driver lock (`lock_order::NET_DRIVER`, above the governance band) is held only inside `service`, `rx_take` and `rx_recycle`, as today.
  - The demux runs with no driver lock held; `rx_take` hands out the buffer, which leaves the ring.
  - The demux takes only net locks (ARP, the socket table, `CONNECTIONS`, `LISTENERS`, DHCP), and no governance lock. That is greppable and asserted by lockdep in neo-test builds, as in rung 1.
- **IF state (CP-37).** Thread context, IF on, the same as the syscalls and #215's tick thread. No interrupt handler takes a net lock or the driver lock (R4 keeps it that way: its ISR touches only MMIO and a wake token).
- **Liveness (I-14(b)).** The per-pass budget bounds one device's share; the storm detector (rung 1) masks a stuck status bit. A broadcast flood costs at most the budget per pass, and the nap yields the CPU.
- **Bring-up handoff.** The thread starts after every bring-up loop has returned, beside #215's tick thread (`kmain`, after `sched::init_bsp`), so no driver ring is serviced by two contexts at once. A hot-plugged NIC (USB) registers with the thread instead of running its own loop after bring-up.
- **Ring size.** Polled at 100 Hz, 16 RX slots of 2 KiB take at most 1600 frames a second (about 19 Mbit/s of full frames). That is enough for DHCP, DNS and small HTTP; R4 removes the polling bound. Raising `RX_SLOTS` to 64 is cheap in the arena and is part of R2.

## 5. The DMA fence (I-14(c))

What exists and stays:
- every ring and buffer is in the fenced arena;
- every address given to the chip is a `FencedDmaAddr`;
- bus mastering is granted only after the rings are programmed, and only under `dma=iommu` or the `rtl8125-dma-without-iommu` opt-in.

What R1 and R2 add:
- **Parse only a buffer the device no longer owns.** `rx_take` returns a buffer already out of the ring (its descriptor is not OWN). With VT-d, the device can no longer write it. With `dma=arena-only` (the AMD main PC) a misbehaving device could still write anywhere in the arena, so the demux **copies the frame into a kernel buffer first** and recycles the arena buffer at once. It never parses DMA memory in place, and never re-reads a field after validating it (no double fetch). Cost: one copy of at most 1514 bytes per frame.
- **Device-written metadata is untrusted.** The descriptor length is already bounded against the armed buffer (`rx_classify_bounded`). The chip's checksum-offload bits are ignored; IPv4, UDP and TCP checksums are verified in software.
- **No new DMA.** R1 to R3 add no device-visible memory. R4 adds an MSI vector, not DMA.

## 6. Discriminators and scenarios

QEMU has no RTL8125 model, so the merge gate uses **virtio-net through the same R1/R2 code**, and rtl8125 is proven at R5 (acceptance).

- **Two virtio-net NICs on one QEMU hub** (the `interface_identity_select` precedent, hermetic: no slirp, no internet). NIC 0 is the stack under test, serviced by the `net-rx` thread. NIC 1 is a harness peer that sends and reads raw frames.
  - **R2 scenario `wire_rx_demux`:**
    - the peer sends a UDP datagram to a socket bound by neo-init;
    - a TCP SYN to a listener (the handshake completes and data arrives);
    - an ICMP echo request (a reply comes back);
    - the malformed set (all dropped, counters say why).

    Falsifiers: the thread not started (nothing arrives); the demux not called (nothing arrives); the IPv4 checksum check removed (the bad-checksum frame is delivered).
  - **R3 scenario `dhcp_renew`:** the peer acts as a DHCP server with a 20 s lease, T1 10 s and T2 17.5 s.
    - The client must renew by unicast at T1 and keep the address past 20 s.
    - A second leg ignores the unicast, so the client must rebind by broadcast after T2.
    - A third leg NAKs, so the client must DISCOVER again.

    Falsifier: no renewal transmit, and the lease is dropped at 20 s.
- **Bare-metal (R5), the blind-hardware rule.** It references the runbook for the target. Expected failures, each with its discriminator:
  - RX stalls after idle (ASPM; trap R7): the RX counter freezes while link is up.
  - RDU drops under load: the descriptor-unavailable status count and the tally counter.
  - Frames arrive tagged: the dropped-0x8100 counter.
  - A checksum mismatch from offload confusion: the per-reason drop counters.

  All of these go on the console line (serial) and the fb progress console. R5's dispatch names the MS-7E57's runbook, which it needs before it is dispatched.

## 7. Trap-mining for rtl8125 RX (beyond rung 1's traps 1–10)

Read for principle: OpenBSD `rge(4)` (`if_rge.c`, `if_rgereg.h`, ISC) and Linux `r8169_main.c` (GPL, nothing copied). **The exact register offsets and bit positions below are to be pulled from the raw sources at build time, never through a summarizing fetch** (the byte rule).

| # | Trap | Guard in this design | Discriminator |
|---|---|---|---|
| R1 | **Descriptor unavailable.** All RX descriptors host-owned: the chip drops frames and sets an RDU status bit. Polled slowly, this is normal under load. | Refill in every `service`; never let the storm detector count RDU as a storm while the ring is progressing. | RDU count vs RX count on the console line |
| R2 | **RX FIFO overflow** status on some revisions. Older r8169 chips needed a reset; check what `rge(4)` and r8169 do for the 8125. | Count it, don't reset (unless the sources say the 8125 wedges), and keep servicing. | FIFO-overflow count; RX keeps advancing |
| R3 | **Missed frames are invisible without the tally counters.** | Read the hardware tally (missed, CRC, runt) into the console line at R5. | tally vs software counts |
| R4 | **VLAN stripping.** If the C+ command register's RX-VLAN bit is set, tags move to opts2 and the frame looks untagged. | Leave RX-VLAN off (verify at bind); drop 0x8100 frames in v1. | dropped-tagged counter |
| R5 | **Checksum-offload bits** in the RX descriptor can't be trusted, and differ per version. | Ignore them; verify in software. | the bad-checksum KAT and scenario frame |
| R6 | **Oversize and jumbo.** | `RX_MAX_SIZE` = the buffer size (rung 1); FS/LS fragments dropped (rung 1). | the fragment and bad-length counters |
| R7 | **ASPM/L1 RX stalls after idle** on some boards (rung 1 trap 7, cleared at bind). | Keep it cleared; the R5 run includes a long idle followed by traffic. | RX counter frozen with link up |
| R8 | **Interrupt registers moved on the 8125** (rung 1 trap 2), are 32-bit and write-1-to-clear. MSI needs INTx disabled in PCI COMMAND. | R4 uses the 8125 offsets from the raw `if_rgereg.h`; it disables INTx when MSI is on. | ISR count vs wakes |
| R9 | **Ack-then-reap ordering.** Reap then ack, and a frame landing in between loses its interrupt. | Ack status first, then reap, then unmask (R4). In polled mode the next pass catches it anyway. | an R4 scenario with a frame sent right after a reap |
| R10 | **Interrupt moderation** register on the 8125 (timer and packet thresholds). | Leave it at the reset value in R4; measure before tuning. | — |
| R11 | **Double fetch** of device-written memory. | Copy first, parse the copy (§5). | by construction |
| R12 | **Broadcast or multicast flood** from the LAN. | The per-pass budget, the nap, and per-protocol drops (ICMP rate limit). | CPU stays available; the frame budget holds |
| R13 | **Accept modes.** RCR accepts own MAC, broadcast and multicast (MAR is all-ones, rung 1), never all-physical. | Verify the RX config at bind; v1 keeps all-multicast for NDP. | a frame for another MAC is not delivered (R5) |

## 8. Pitfall-bank consultation (for R2 and R4)

| Pitfall | R2/R4 | Guard |
|---|---|---|
| **CP-1** lock order | the driver lock, then (released) the net locks | the demux runs with no driver lock held |
| **CP-37** mixed IF | thread context only; the R4 ISR takes no lock but a wake token | as in §4 |
| **CP-56** hidden drop under a lock | the demux drops copies and closes connections only outside the driver lock; TCP's own closes are #202 and #211's. **One driver drop is under its lock** (Chad #173 item 2): virtio's `service_tx_completions` drops a completed TX header, a private `Arc<DmaBuffer>` from `alloc_zeroed` and normally its last reference, with the device `inner` held. That buffer is buddy-backed, so its `Drop` frees frames through `with_frame_alloc`: the order is device `inner` → frame allocator. The allocator is a leaf (it masks IF itself and calls nothing that takes a driver or net lock), so no inverse order exists. An arena-backed buffer would take `dma_arena::SPANS` instead, also a leaf; the virtio header is never arena-backed | the order is by construction; in debug builds the allocator's per-CPU re-entry guard (`frame_reentry_guard`) panics on a re-entry |
| **CP-54** retained state and queued work | what a LAN sender can grow: the ARP, NDP, route, prefix and address tables (capped, §11), the TCP egress queue (with `net-tick`, an admission threshold of 512: past it a new segment is dropped and counted; segments a pass took out and puts back, and those the syscalls' pump retains, return whatever the length, so the queue can pass 512 by what is out at that moment, at most what was admitted before; each flush and pump takes at most its budget off the front and touches nothing else, and the wire send's interface snapshot makes at most 4 passes, `identity_logic::SNAPSHOT_PASSES`). Not bounded: the syscalls' own loopback pump, which drains until no delivery while producers can refill the queue (Chad #241 1b, 1c) and the listeners' queues (`backlog`) | `wire_rx_demux`'s flood leg; the host `egress_pass` KATs (same allocation at a 1,000- and a 100,000-segment backlog) |
| **CP-8 / CP-14** lost wake | R4: ISR wake vs the thread checking for work | the thread re-checks the ring after arming its wake token, as `tick_sleep` does |

## 9. Kill-switch

- **R1 and R2: feature `wire-rx`.** Off means not compiled, and every driver's bring-up loop is unchanged.
- **R3: part of `net-tick`'s image-A step.** DHCP renewal is cfg'd on `wire-rx`, since there's no point renewing without receiving.
- **R4: `rtl8125-msi`** (and `virtio-net-msi`).

Each step is byte-identical to `main` with its feature off, for default, desktop, usb-host and image A (the #215 method), **except what this PR changes unconditionally or under another feature** (corrected after the PM's AAG of `b7e3d53e`):

| Change | Built in | Effect with `wire-rx` off |
|---|---|---|
| mt7601u's late-bind DORA loop runs each pass IF-off, at most 24 frames a pass (Chad #173 F1) | every kernel with `mt7601u` | changed. None of the four measured kernels builds `mt7601u` |
| The per-tick bounds: TCP table rounds, the bounded wire flush and loopback pump, the ARP and NDP caps (#175 item 4) | every kernel with `net-tick` | changed wherever `net-tick` is on (CI's product-real-lm arm). The four measured kernels don't build `net-tick` |
| `compile_error!` for `net-tick` with IF-on harness scenarios (#175 item 1) | `net-tick` test kernels | refuses those combinations; nothing else |
| The identity registry's `refresh_link_states`, `name` and `snapshot_links` allocate nothing under its lock (#175 item 2) | every kernel (compiled always; active only with `interface-identity`, which `boot-eth0` and so the product turn on) | changed code in every kernel; the same results, built outside the lock |

The off baseline for the measured kernels is therefore `main` (which carries #215's unconditional registry change and #230's listener flag) plus the identity row, whose code is in every kernel. So `wire-rx` off is **not** byte-identical to `main`; the identity row is the difference in the four measured kernels.

## 10. Decisions (PM, 2026-10-29)

1. **R1 and R2 ship as one PR**, then R3 (renewal, plus `net-tick` into image A).
2. **The MVP is IPv4-only for data.** NDP and RA keep their existing inject paths, so nothing on the wire is answered wrongly. IPv6 data becomes its own OT row after the MVP.
3. **Closed ports:** TCP to a closed port gets TCP's RST; UDP to a closed port is dropped silently; no ICMP unreachable is sent.
4. **Review:** Muse reviews R1's parser after the merge (hostile input). R2 gets a pre-merge AAG with a cross-family lens.

## 11. Build notes (R1+R2)

- **ARP replies.** The design missed that `arp::inject` only learns from replies: a request for our address was never answered, so a LAN peer could not send to us unicast. R1 adds `rx_logic::arp_request_for` and `arp_reply`, with KATs. A request for our address is answered, and the requester is learned (RFC 826). Requests with a broadcast, multicast or zero sender MAC, or a broadcast sender IP, are neither answered nor learned.
- **Echo replies** go straight back to the requester's MAC (no ARP), at most 16 a second. They are never sent for a ping to a broadcast address (a smurf amplifier).
- **TCP replies to a wire peer** (a SYN-ACK, an ACK) are queued by the TCP layer. The receive pass ends with `tcp::flush_wire_egress`, so they go out within the pass, not only at the next `tcp::tick`.
- **Harness scenarios that drive TCP or a NIC themselves, IF-on** (`phase4_tcp_connect_stream_close`, `phase4_dhcpv4_ipv4_udp`, the `smoltcp_wrap` smoke) hang if built with `wire-rx`. The harness is a kernel thread taking the net spin locks IF-on; the `net-rx` thread takes them IF-off on the same CPU (CP-37). `phase4_tcp_connect_stream_close` hit its 180 s budget with `NEO_TEST_EXTRA_FEATURES=wire-rx`. No CI kernel combines them, and on `main` they have no IF-off contender. `wire_rx_demux` and `tcp_listen_bounds` take the locks IF-off and pass with the thread running. The rule holds by construction once the net locks mask interrupts (OT-97).
- **A pre-existing ordering nit, left as it is.** `tcp::flush_wire_egress` (on `main`, called by `tcp::tick` and now by the receive pass) pops `OUTBOUND` one segment at a time, then appends the loopback segments back at the end. A segment another CPU queues meanwhile lands ahead of them, and a drain in between sees them gone. Loopback TCP recovers by retransmit. Changing it would change kernels without `wire-rx`, so it stays for its own change (with OT-97).

- **Chad #173, the next round (seat C).**
  - **F1, IF-on takers (CP-37).**
    - The kernel's post-reap `virtio_net::service_all` was already safe: it takes each device's lock under the device table's `IrqSafeMutex` guard (an `IrqSafeMutex` at the reviewed head too), so IF is off there whatever the caller's state. `wire_rx_demux` now calls it from its IF-on harness while `net-rx` runs, as a regression check.
    - mt7601u's late-bind DORA loop runs after wlan0 is registered, with IF on. Each pass's service, frame drain, `net::tick` and DHCP snapshot (and the discover) now run IF-off, at most 24 frames a pass; the 200 µs delay stays IF on.
    - **The discriminator:** in neo-test builds, once `net-rx` starts, the virtio and wlan0 device methods panic if their lock is taken with IF on (`rx::assert_if_off`). A direct IF-on `service()` in `wire_rx_demux` trips it. The harness's own NIC 1 calls were IF-on, and are now IF-off: the virtio interrupt takes that lock too.
  - **F2, bounded work.** A frame count didn't bound the pass: `flush_wire_egress` drained until empty, so a producer on another CPU could keep it running. With `wire-rx` the pass flushes through `tcp::flush_wire_egress_some`: at most 16 segments per IF-off chunk and 4 chunks a pass, the rest kept in order for the next pass. *(Superseded by the Chad #189 note below: that version bounded the sends but still rebuilt the rest of the queue under the lock.)* `tcp::tick` uses it too. Virtio's TX completion loop takes at most one queue's worth a call.
  - **F3, listener retirement** is #230's `TcpListener::retired`, merged in here.
  - **`wire-rx` requires `net-tick`**, now on `main` (#215).
- **Chad #175 (#215's review), folded in here (seat C).**
  - **Item 1, IF-on harness takers:** the `compile_error!` moved from `net::rx` to `net::periodic`, so it refuses `net-tick` (and `wire-rx`, which requires it) with every harness scenario that takes the net locks IF-on: the six above, plus `dhcp_rx_real_lease`.
  - **Item 2, allocation under the registries:** the identity registry's hot paths allocate nothing under it now. `refresh_link_states` (from the tick) reads one id at a time; `name` copies the bytes out; `snapshot_links` sizes its `Vec` outside the lock. Registration still allocates under both registries (boot and hotplug only), and that is safe IF-off: the kernel heap is a fixed range behind its own IF-off spin lock that takes no other lock and never parks or grows, so the order is registry → heap, a leaf.
  - **Item 4, bounded work per tick** (with `net-tick`; the egress part corrected by the Chad #189 note below): TIME_WAITS, the listeners and the connections are each visited a round of at most 64 entries per tick, a rotating cursor resuming where the last stopped. The wire flush looks at no more than 16 segments, and the tick thread's loopback pump no more than 128 (`pump_loopback_egress_some`; the syscalls' pump is unchanged). The ARP and NDP caps (128) apply with `net-tick`, not only `wire-rx`. The price: with N connections, a retransmit can wait up to N/64 ticks for its turn.
  - **Items 5 and 7, the wording:** the registry change is unconditional, so a kernel without `net-tick` is not unchanged from before #215 (desktop and image A `.text` +384 B, usb-host +64; default unchanged). And CP-37 holds for the product's takers and every harness scenario that can be built with the feature, not for every taker in every build. `net::periodic`'s module doc now says both.

- **Chad #189 (re-check of #218): the whole egress call is bounded, not only its sends.** The first bounded flush and pump took a snapshot of the queue, sent at most their budget, then put the untouched rest back and appended what other CPUs had queued meanwhile, all under the lock and IF-off: work proportional to the backlog (Chad measured 63,040 bytes allocated at 1,000 queued segments and 6,399,040 at 100,000). Now each is one `egress_logic::egress_pass`: under the lock it pops at most the budget off the front; outside it, it sends; under the lock again it pushes back only what it kept. Nothing else in the queue is read or moved. With `net-tick` the queue admits a new segment only below `tcp::OUTBOUND_ADMIT` (512): past it the segment is dropped and counted (`tcp::outbound_drops`), as a full NIC queue drops a frame; TCP recovers by retransmission, since data and FIN stay in `unacked`, and a lost RST or ACK is drawn again by the peer's own retransmission. **It is an admission threshold, not a hard cap** (Chad #241 1c): a pass's put-back and the syscalls' pump's retained segments go back whatever the length, so producers on other CPUs can push the queue past 512 by what is out (512 → 528 with one 16-segment flush out; the KAT `the_admission_threshold_is_not_a_hard_cap` shows it). Only segments admitted earlier come back, so the overshoot is what passes have out. The queue is made with room for the threshold plus a pump's budget, so a put-back rarely reallocates it. The price of not walking the queue: a wire segment at the front holds back loopback ones behind it until a flush sends it, and the reverse. The syscalls' own loopback pump is unchanged; it drains until no delivery, and since producers can refill the queue while it drains, not even a round is bounded. **The wire send's interface snapshot** (`identity::snapshot_links`, from `transmit_wire_segment`) retried for as long as interfaces kept registering between its sizing and its fill (Chad #241 1b); it now makes at most `identity_logic::SNAPSHOT_PASSES` (4) passes, the last taking the oldest entries that fit, so an interface registered during the call can be missed by that one send. KAT `a_snapshot_of_a_table_that_keeps_growing_is_bounded` (with the bound removed it runs into the probe's 1,000-pass cutoff). Host KATs: `an_egress_pass_does_the_same_work_whatever_the_backlog` (the same bytes allocated at 1,000 and 100,000; with the old snapshot-and-rebuild put back, 23,872 against 2,399,872, and the KAT fails), the order and queue-end KAT, and the cap.

- **Hardening from the pre-merge AAG (2026-10-29).** `wire-rx` makes four older consumers reachable from the LAN for the whole uptime: DHCP, the RA handler, ARP and NDP. Each is now held to the rule the wire needs, behind `wire-rx`:
  - **DHCP:** a reply counts only for this client's own exchange (RFC 2131 §4.1, §4.3). It must be a BOOTREPLY with our xid and our `chaddr`; an OFFER only while SELECTING; an ACK or NAK only while REQUESTING, and from the server we requested (`rx_logic::dhcp_reply_ok`). Before this, any host's broadcast ACK installed its router, mask and DNS, and a forged OFFER drew a REQUEST even from BOUND.
  - **RA and NDP (RFC 4861 §6.1, §7.1):** ICMPv6 with no extension headers, hop limit 255, a verified checksum, code 0, a non-multicast source, an RA only from a link-local source, and a destination filter: ours, all-nodes, or our solicited-node groups (`rx_logic::classify_ipv6`). `ndp::parse` no longer parses bytes that carried no IPv6 header.
  - **Bounds on state a sender can grow:** 16 IPv6 addresses (one temporary per prefix, refreshed rather than added), 16 routes (deduplicated), 4 prefixes per RA, 128 ARP and 128 NDP entries, 8 IPv4 addresses. No print per RA; the receive thread no longer copies the IPv4 table for every frame.
  - **Source MACs:** a group or all-zero source MAC is dropped, so no echo reply goes to the whole link. ARP senders that are ours, unspecified, loopback, multicast or reserved, or a broadcast, are neither learned nor answered.
  - **Still open:** any LAN host can still send a valid RA from a link-local source and set our prefix and DNS. That is SLAAC's trust model; RA Guard is a switch feature. Aging needs `net-tick`, which `wire-rx` now requires (R3 puts both in image A).

## 12. IPv6 data: scope (for J.c., 2026-10-29)

J.c.: "if it's not much more work, do it; if it's a lot, it can wait." **Recommendation: after the MVP**, as R6 and R7 below, behind its own feature. IPv4 reaches every network the MVP targets: home routers are dual-stack, and DHCPv4 and the v4 paths are what rtl8125 has proven on silicon. IPv6 data has one gap no QEMU gate can close (off-link routing, below) and one only a real network shows (MLD snooping).

**What exists** (send side and configuration):
- SLAAC from Router Advertisements, with prefixes, routes and RDNSS (`net/ipv6.rs`, `net/icmpv6.rs`). DAD is a one-tick timer; no DAD probe is sent.
- The neighbour cache, and building and parsing NS and NA (`net/ndp.rs`). An NA is learned.
- UDP and TCP send over IPv6 (`socket.rs`; `tcp::transmit_ipv6_segment`), proven on loopback (`phase4_slaac_udp_ipv6_loopback`).
- rtl8125 accepts all multicast (`rtl8125.rs:1188`), so solicited-node traffic arrives.

**What's missing:**

| Rung | Work | Kind |
|---|---|---|
| a | `rx_logic` v6: the fixed 40-byte header; payload length within the frame; next header. Extension headers are dropped (MVP), as are fragments. Destination filter: our addresses, the all-nodes group, our solicited-node groups. Source filter: no multicast; unspecified only for DAD. | Pure logic, host KATs plus a byte oracle; the R1 shape |
| b | Answer an NS for our addresses (`ndp::inject` returns `Unsupported` for one today), so a peer can reach us: the ARP-reply gap again. Send a real DAD probe. | Kernel, thread context |
| c | ICMPv6 echo reply (the checksum is mandatory, over the pseudo-header). UDP over v6 (a zero checksum is invalid in v6). TCP over v6 into `inject_segment`. | Reuses R2's delivery |
| d | **Off-link egress.** `ndp::submit_or_queue_ipv6` resolves the destination itself, and nothing keeps a default router. So a packet to a global address off the LAN solicits the remote host, gets no answer and is dropped. The fix: keep the RA's source as the default router and resolve the next hop. | Kernel; send path |
| e | MLD reports for our solicited-node groups. A switch with MLD snooping may otherwise drop the multicast NS meant for us. | Only a real network shows it |

DHCPv6 isn't needed: SLAAC and RDNSS cover the MVP's configuration.

**Cycles.** R6 is a–c plus v6 legs in `wire_rx_demux` (NS answered, echo, UDP, TCP over link-local and global addresses): one host cycle and one or two QEMU gate cycles, the R1+R2 shape again. R7 is d, plus a QEMU leg with a router peer on the hub: one or two gate cycles. e is uncertain on bare metal: unknown depth, and it depends on the network in front of the MS-7E57 (R5's target).

**Genuinely uncertain:**
- whether the network in front of the MS-7E57 snoops MLD;
- whether DAD collisions need handling beyond a log line;
- which source address that network's router expects for the global prefix (address selection, RFC 6724, exists only as `preferred_global_address`).

**Kill-switch.** A `wire-rx-v6` feature. Off means v6 frames go only to NDP and the RA handler, as in R2.
