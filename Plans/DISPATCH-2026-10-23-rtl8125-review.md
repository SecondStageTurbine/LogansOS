<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW: RTL8125 2.5GbE driver, rung 1 (cross-family checklist)

**MODEL:** GPT-6-Luna. **EFFORT:** high. One contained subsystem: a single PCI NIC driver, no SMP core, no new
syscall.

**Branch:** `claude/rtl8125-ethernet` (author: S4, Claude family). **Pin the head SHA** the PM relays with this
file; review that SHA only.

**Read at that SHA:**
- `kernel/src/drivers/net/rtl8125_logic.rs` (pure logic: registers, XID, descriptors, rings, containment);
- `kernel/src/drivers/net/rtl8125.rs` (MMIO, PCI, bring-up, `NetDevice`);
- `tools/rtl8125-logic-tests/src/lib.rs` (host tests and byte oracles);
- `kernel/src/test_harness.rs`: `run_rtl8125_absent`, `run_rtl8125_vtd`, `rtl8125_stand_in`;
- the `rtl8125*` lines in `kernel/Cargo.toml`, `kernel/src/main.rs`, `kernel/src/drivers/pci.rs`,
  `kernel/src/sync/mod.rs`, `tools/xtask/src/main.rs`;
- the dispatch: `Plans/DISPATCH-2026-10-23-rtl8125-ethernet.md`.

**Answer format:** for each item, PASS or FAIL, with the enforcing `file:line`. For a FAIL, give a concrete input
or register sequence that shows it. Push the RETURN as `Plans/RETURN-2026-10-23-rtl8125-review-<seat>.md`.

## A. DMA containment decision table

Check `rtl8125_logic::containment` and `bus_master_allowed` against this table, and check the tests cover every row:

| arena ready | device fenced by VT-d | `rtl8125-dma-without-iommu` | mode | bus master |
|---|---|---|---|---|
| no | any | any | `Blocked(NoArena)` | off |
| yes | yes | any | `Iommu` | on |
| yes | no | yes | `ArenaOnly` | on |
| yes | no | no | `Blocked(NoIommu)` | off |

1. The function returns exactly the table's mode for all 8 input combinations.
2. `bus_master_allowed` is true only for `Iommu` and `ArenaOnly`.
3. `containment_decision_table` in the host crate asserts all 8 rows.
4. In `bind_containment`, `vtd_fenced` is true only when `iommu::enabled()` and `iommu::fence_source_id(sid)`
   both succeed. The source id is `bus << 8 | dev << 3 | fn`. Without the `vtd` feature, `vtd_fenced` is the
   constant `false`.
5. The opt-in is `cfg!(feature = "rtl8125-dma-without-iommu")` and nothing else. No other feature in
   `kernel/Cargo.toml` lists it, so no image enables it.

## B. Bus Master Enable

6. `pci::enable_bus_mastering` is called from exactly one place in the driver (`grant_bus_master`), and that
   call is behind `bus_master_allowed(mode)`.
7. `bind_containment` writes COMMAND with Bus Master Enable **cleared**, before any MMIO access to the chip.
   Check `bind_command` too: it sets memory decode and INTx disable, and clears I/O decode and bus master.
8. In `bring_up`, `grant_bus_master` runs only after both `write_ring_base` calls, and after `refill_rx` has
   armed the RX ring.
9. On the blocked path (`!bus_master_allowed(mode)`), `bring_up` returns before `alloc_rings`. No arena
   buffer is allocated, no ring base is written, and `CMD` is never written with TX/RX enable.
10. The `rtl8125_absent` scenario pre-sets Bus Master Enable on the stand-in. It reads the bit back from
    config space after the bind, and fails if it is set. The control feature makes it FAIL with
    `bus_master_set`.

## C. Every address the chip sees is fenced

11. Every value written to `TX_DESC_LO/HI` or `RX_DESC_LO/HI` comes from a `FencedDmaAddr`
    (`write_ring_base` takes nothing else).
12. Every descriptor buffer address comes from `fence_buffer(...)` (`dma_arena::fence`), in `refill_rx` and
    in `tx_submit`. A fence refusal never reaches `publish`: the buffer goes back to the pool, or the
    submit fails.
13. Every `DmaBuffer` the driver allocates is `alloc_in_arena`. There is no `alloc_zeroed` in the file.

## D. Ring ordering and barriers

14. `RingMem::publish` stores the address and opts2, then `fence(Release)`, then opts1. opts1 is the store
    that carries OWN.
15. `RxRing::reap` reads opts1 and returns early while OWN is set. It calls `acquire()` before any event
    that hands out a buffer. The kernel's `acquire` is `fence(Acquire)`.
16. `tx_submit` issues `fence(SeqCst)` after `TxRing::post` and before the `TX_POLL` doorbell write.
17. `TxRing::reap` reclaims strictly in order and stops at the first slot with OWN set. Out-of-order
    completion is held, never reclaimed early (`tx_completion_out_of_order_is_held`).
18. RX ring invariant: the chip-owned slots are `[next, fill)` and the empty ones `[fill, next)`.
    Check that every path keeps `next == fill + empty (mod len)`: refill, frame, and drop. A dropped
    (error/fragment) slot is emptied, never re-armed in place.
19. No slot is armed twice. `refill_rx` only arms the slot `next_fill_slot()` returns, and that slot's
    `rx_slot` entry is `None`.
20. The end-of-ring bit is set exactly on slot `len - 1` in both rings.

## E. Descriptor format and register map

21. The byte oracles in the host crate match the classic 16-byte r8169 layout: opts1, opts2, then the 64-bit
    address, little-endian. Bits: OWN 31, EOR 30, FS 29, LS 28, RX error summary 21, RX length 13..0
    including FCS.
22. `rx_config` never sets bit 24 (the 32-byte RX v3 format). `MAC_INIT_COMMON` clears MAC-OCP `0xeb58` bit 0
    (the new TX format). The two choices agree with each other.
23. The 8125 interrupt registers are used: IMR `0x38` and ISR `0x3c`, both 32-bit. The TX doorbell is `0x90`
    (16-bit). Nothing uses the 8168's 16-bit IMR/ISR.
24. `decode_xid` accepts only 0x609, 0x641, 0x688 and 0x689, and refuses 0x649/0x64a (RTL8126), 0x68a, 0x681
    and 0x708. Device id `10ec:8126` is refused by name in `init`.
25. The RX DV gate (`MISC` bit 19) is set during reset and cleared only after RX config, TX config and the
    multicast filter are programmed.

## F. I-14(a)/(b): locks and liveness

26. The driver's only lock is `Rtl8125::inner`, `SwitchMutex::new_ordered(.., NET_DRIVER = 29)`, above every
    AI-governance level (≤ 28). Code under it takes no governance lock (`INFLIGHT`, `RECENCY`, `ARENA`,
    reservation, `TABLE`).
27. No interrupt handler exists. IMR stays 0 and PCI COMMAND has INTx disabled. `irq=polled` is printed.
28. Every wait is bounded: `Regs::wait` with a µs limit, the OCP polls, reset, FIFO drain, PHY ready,
    autonegotiation, and the boot DHCP/ARP pump loops.
29. A service pass is bounded (`REAP_BUDGET` per ring). `StormDetector` declares a storm after
    `STORM_PASSES` fruitless passes and masks the bits after `STORM_LIMIT`. A TX ring with no completions
    for `TX_WEDGE_PASSES` sets `tx_wedged`, after which `tx_reserve` returns `None`.

## G. Kill-switch

30. With `rtl8125` off, nothing new is compiled. Every new item in `pci.rs`, `sync/mod.rs`, `main.rs` and
    `drivers/net/mod.rs` is `#[cfg(feature = "rtl8125")]`.
31. The `.text` identity table in the PR's ready comment covers `default`, `desktop`, `product`, `boot-eth0`
    and image A's set. Each row matches `origin/main` at the cited base.

## H. NetDevice behaviour

32. `tx_submit` rejects `len == 0` and `len > 1514`, and returns the frame's buffer to the free list on every
    error path. Frames shorter than 60 bytes are zero-padded to 60.
33. `rx_take` hands out the frame without the FCS. `rx_recycle` returns the buffer to the pool and refills.
34. Registration, the boot DHCP and the gateway ARP do not change behaviour for virtio-net or CDC-ECM. DHCP is
    skipped if another interface is already `Bound`, and it does not run in `neo-test` builds.
