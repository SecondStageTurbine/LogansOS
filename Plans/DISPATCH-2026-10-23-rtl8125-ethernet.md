<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Realtek RTL8125 2.5GbE driver, rung 1: link, DHCP lease, ping (S4)

**Date:** 2026-10-23 · **Author:** PM · **Seat:** S4 (Claude cloud session) · **PL request:** J.c., 2026-10-23
("Start S4 on the Realtek driver").
**Branch:** `claude/rtl8125-ethernet` (cut from `origin/main`). **Driver-coverage tier:** 5
(`Plans/NOTE-2026-10-22-driver-coverage.md`).

## Goal

Wired networking on the PL's main PC (MSI MS-7E57). The inventory has `PCI\VEN_10EC&DEV_8125&SUBSYS_7E571462&REV_0C`,
"Realtek PCIe 2.5GbE Family Controller" (from the PL's hardware inventory, kept privately). The RTL8125 is
the most common 2.5GbE NIC on current desktop boards, and the same r8169-family design covers the RTL8111/8168 on
most older ones.

**Rung 1:**
- bind by PCI ID and identify the MAC version from the chip's XID (`TxConfig`);
- read the MAC address; bring the PHY up and report link speed/duplex;
- TX and RX through descriptor rings;
- plug in as a `net::device::NetDevice` (`kernel/src/net/device.rs`), the same seam `virtio_net` and CDC-ECM use, so
  the existing stack does DHCP and ping unchanged.

Out of scope for rung 1: multi-queue/RSS, checksum and segmentation offload, jumbo frames, EEE tuning, WoL, and power
management beyond D0. Polled mode or a single interrupt vector is fine.

## The DMA containment question (I-14(c)): read this first

- **The main PC is AMD (Ryzen 9 9900X).** Our IOMMU containment, I-14(c2), is **Intel VT-d only**
  (`kernel/src/drivers/iommu/vtd_*`); there is no AMD-Vi driver.
- The fenced DMA arena, I-14(c1) (`kernel/src/drivers/dma_arena.rs`), works on any CPU. It stops *our* driver from
  handing the device a bad address, but not a misbehaving device DMA-ing wherever it likes.
- CLAUDE.md's connectivity rule asks for every DMA buffer to go through `dma_arena::fence(...)` **under the existing
  IOMMU containment**.

So rung 1 is built like this:
1. **Every descriptor ring and packet buffer is allocated in the arena, and every address programmed into the chip is
   a `FencedDmaAddr`.** No exceptions; the type system enforces it.
2. **When VT-d is present and enabled,** the device's DMA is also confined by the IOMMU, as for the other connectivity
   drivers.
3. **When no supported IOMMU is present (the AMD main PC), the driver does NOT enable bus mastering by default.** It
   binds, reads the MAC and link state, prints `rtl8125: dma=blocked (no iommu)`, and stops. Enabling DMA on a host
   without an IOMMU needs a separate, explicit feature, `rtl8125-dma-without-iommu`. It is off in every image and is
   never implied by another feature. Whether that feature is ever used is a **PL decision at acceptance time**, not
   part of this build. S4 just makes both modes exist, makes the fail-closed default provable, and has the console
   line say which mode is live.
4. Note in the PR that an AMD-Vi driver would close this gap properly; that is a separate lane, not this one.

## Scope

1. **PCI bind.**
   - Match `10EC:8125`, plus `10EC:8126` if it shares the register map, which you must verify.
   - Enable memory decoding; leave bus mastering off unless the containment rule above allows it.
   - Use the MMIO BAR (BAR2 on r8169-family parts; verify), put the device in D0, and use MSI if the kernel supports
     it, else INTx through the IOAPIC.
2. **Identify the chip.** Use the XID bits of `TxConfig` (RTL8125A vs 8125B vs later), and refuse unknown
   versions with a console line rather than guessing.
3. **Reset and MAC address.** Do the software reset with a bounded wait, unlock the config registers, and read the
   MAC address (`MAC0`/`MAC4`; if it reads zero, fall back to the documented backup register).
4. **PHY.** Use OCP/`PHYAR` access, then autonegotiate with a bounded wait. Report the link state, speed and duplex.
   - **RTL8125 PHY firmware:** Linux loads `rtl_nic/rtl8125*` firmware patches for some revisions. Find out whether
     **this** revision links without it.
     - If it does, rung 1 ships without firmware.
     - If it doesn't, STOP AND REPORT: the firmware's licence (linux-firmware, redistributable) and how we'd carry it
       is a PL decision.
5. **Rings.** TX and RX descriptor rings in the arena: the ownership bit, end-of-ring bit, buffer address and length,
   with the RTL8125's descriptor format (verify it against the 8168's). Program the ring base registers and the
   RX/TX config, and enable TX/RX. Handle RX overflow and FIFO-overflow status without wedging.
6. **Interrupts.** The RTL8125 moved the interrupt mask/status registers to new offsets compared with the 8168, so use
   the 8125 ones. Keep the ISR short (ack + signal); the service thread reaps rings. A polled fallback is fine for
   rung 1 if the console line says which mode is live.
7. **`NetDevice` integration.** Register it as the boot interface the same way `boot-eth0` brings up the first NIC,
   without changing behaviour for virtio-net or CDC-ECM.
8. **Console line** (fb progress console and serial):
   `rtl8125: xid=… mac=… link=up 2500/full dma=iommu|arena-only|blocked irq=msi|intx|polled tx=N rx=N err=…`.

## Kill-switch (required)

Feature `rtl8125`. Off ⇒ the kernel `.text` is **bit-identical** to `main` for `default`, `desktop`, `product`,
`boot-eth0`, and image A's feature set (the #95 identity method). `rtl8125-dma-without-iommu` exists only as a
separate opt-in. Show that `rtl8125` alone, on a host without VT-d, never sets PCI Bus Master Enable.

## I-14 governance fault-containment analysis (required)

- **(a) Lock isolation.** The ISR takes no AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`/reservation/`TABLE`)
  and no lock held across a ring reap. Show it by grep, plus a runtime assertion in `neo-test` builds.
- **(b) Liveness.** An interrupt storm (a stuck status bit, RX overflow) must not monopolise a CPU. Ack-and-mask in the
  ISR, then fall back to polled mode after K storms, and report it. A wedged chip times out and is reported, never
  spins.
- **(c) Memory isolation.** As above: the arena always (c1), VT-d when present (c2), and bus mastering blocked by
  default when neither IOMMU form exists. State all three in the PR.

## Prior-art trap-mining (required; seed the failure bank, then add what you find)

Read for principle only: Linux `drivers/net/ethernet/realtek/r8169_*`, FreeBSD `re(4)` and the newer `rge(4)`,
OpenBSD `rge(4)` (ISC-licensed, the cleanest RTL8125 reference), and Realtek's public datasheets where they exist.
**Never copy GPL code;** ISC/BSD code may be read freely, but we still write our own. Known traps to check:

1. **MAC version zoo.** Behaviour differs per XID. Decide and test per version; never "close enough".
2. **RTL8125-specific register moves:** the interrupt mask/status offsets, and the `MISC`/`RCR` differences vs the 8168.
3. **PHY firmware** for some 8125 revisions (above).
4. **Bus mastering before the rings are programmed:** the chip may DMA a garbage ring base. Program the rings first,
   then enable bus mastering.
5. **RX descriptor ownership race:** read the OWN bit, *then* the length, with an acquire barrier, and give the
   descriptor back only after copying out.
6. **TX doorbell ordering:** descriptor writes must be visible before the doorbell (`TPPoll`). Use a write barrier.
7. **ASPM / L1 link-state problems** on some boards: link flaps, or TX hangs. Linux disables ASPM for many versions.
8. **MAC address reads as zero or all-ones** if the EEPROM or autoload isn't done. Use a bounded wait and a backup
   register.
9. **Autonegotiation takes seconds at 2.5G.** Use a generous bounded wait, and DHCP must not give up before link-up.
10. **DHCP needs a non-empty `chaddr`:** the CDC-ECM precedent; already handled by the stack. Confirm the MAC flows
    through.

## Concurrency (required): pitfall-bank consultation

Consult `docs/concurrency-pitfalls.md` (CP-1…CP-73 on `main`). For each class the ISR → service thread → ring path
touches, state whether you introduce, guard, or leave it N/A, and name the discriminator. The ring-ownership and
doorbell-ordering traps above are memory-ordering pitfalls; add any new one back to the bank.

## Gates

**Merge bar:**
- rebase-fresh;
- `scripts/worker-close-gate.sh`;
- the 4-shard QEMU gate, green on the exact SHA (the PM dispatches it);
- the cross-family review;
- the kill-switch identity.

**Acceptance bar** (a milestone, not a merge gate): a DHCP lease and ping on real RTL8125 silicon.

QEMU has no RTL8125 model, so:
- **Host logic-test crate** (`tools/rtl8125-logic-tests`):
  - **byte oracles** for the descriptor format: hand-laid TX and RX descriptors (OWN/EOR/FS/LS bits, length,
    address), taken from raw sources, never a summarising fetch;
  - XID → version decode;
  - the ring reap state machine against a register model: wraparound, an overflowed RX, a TX completion out of order;
  - the containment decision table: VT-d on / off × feature on / off → bus master on / off.
- **QEMU scenarios:**
  - `rtl8125_absent`: the feature is on, no device, and it reports "not present". Its control proves the DMA-blocked
    path: a synthetic bind on a host with no IOMMU never sets Bus Master Enable;
  - the existing `boot_eth0` / virtio-net / `desktop_*` scenarios still pass.

## Bare-metal acceptance run (required failure-mode + discriminator section)

**The target is a PL decision at acceptance time, not now.** The only RTL8125 we know of is in the PL's daily main
PC. If he chooses to boot it:
- it's from a USB stick, with the write guard on (`GuardedDevice`, PR #85/#88). The main PC's NVMe and SATA disks are
  **visible** to us (no VMD), so the image must carry no disk-sink bundle;
- the run also needs his decision on `rtl8125-dma-without-iommu`, since the box is AMD.
- **The main PC has a real COM1** (`PNP0501`), so serial capture is possible if he has a cable; otherwise screen
  photos.

Adapt the runbook methodology (`docs/runbook-baremetal-zenbook-s14.md`) to that machine in a short runbook section in
the PR; don't write a new runbook yet.

Pre-seeded failure bank (the console line must tell each mode apart):

| Mode | What the console line shows |
|---|---|
| not bound | `pci 10ec:8125 absent` |
| unknown XID | `xid=0x… unsupported` |
| DMA blocked | `dma=blocked (no iommu)` and no TX/RX attempted |
| MAC zero | `mac=00:00:…` + `autoload=timeout` |
| PHY firmware needed | `link=down an=timeout` on a known-good cable |
| link but no RX | `link=up rx=0 tx=N` |
| RX overflow storm | `irq storm→polled rxovf=N` |
| DHCP fails | `link=up rx=N dhcp=timeout` |

## Cross-family review

**MODEL:** GPT-6-Luna. **EFFORT:** high. One contained subsystem: a single driver, no SMP core, no new syscall.
- Make it a plain pass/fail checklist, with no refute/attack/exploit wording.
- Include the DMA-containment decision table and the ring-ordering barriers as explicit items.
- The file is `Plans/DISPATCH-<date>-rtl8125-review.md`, and the PM relays it to a Codex seat.

## Close

- Commit and push at every checkpoint.
- Open a PR against `main`, and post "PM: ready to gate <SHA>" with:
  - the close-gate output;
  - the identity table;
  - the host-test and scenario results;
  - the containment decision table;
  - the pitfall consult;
  - the trap-mining notes.
- **Do not dispatch the 4-shard gate yourself.**
- End GitHub comments with the Claude Code attribution footer.
- **Disk reclaim:** after merge, `cargo clean` if you keep the tree.
