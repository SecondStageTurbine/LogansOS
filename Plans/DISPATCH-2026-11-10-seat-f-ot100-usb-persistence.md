<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Seat F, OT-100 — Tails-style persistence on the USB stick (2026-11-10)

**PL decision (J.c., 2026-11-10 ~00:40 UTC):** "Yes tails style." LogansOS saves to a persistence area **on the
boot stick itself**, and **never writes the laptop's internal drive**. This closes 0.1 rung 5 ("work persists") on
the Zenbook, so the one-boot 0.1 showcase can run there. It also gives the setup menu a real "Try" mode, and bare-metal
runs a durable log channel instead of screen photos.

**Seat:** Seat F (cloud), after #90 lands. **OT:** OT-100 (add it to `docs/OPEN-THREADS.md`).
**Step 1 branch:** `claude/ot100-usb-persistence-think`. A **THINK** first, then a cross-family read, then the build
(the pattern every new OT follows).

## Step 1: the THINK (`docs/THINK-2026-11-10-ot100-usb-persistence.md`)

It must settle:
1. **The stick layout.** Where the persistence area lives (a partition, or an extent outside the partition table)
   and how it's created: at image build time, or on first boot with consent, the way Tails asks. The existing write
   guard (`kernel/src/drivers/block/mod.rs`, `GuardedDevice`, `write_guard_logic::judge`) admits a write only inside
   an extent a **sealed marker claims and no partition table allocates**. Design the area to fit that rule rather
   than loosening the guard. If the rule must change, say exactly how and why, and treat it as relax-and-re-prove.
2. **The USB mass-storage write path:** `MscDisk::write_block`, today `Unsupported` at
   `kernel/src/drivers/usb/msc.rs:360`. WRITE(10) or WRITE(16), SYNCHRONIZE CACHE, write-protect detection (MODE
   SENSE), CSW/sense error handling, the stick being pulled mid-write, and the max transfer size.
3. **NeoFS on the area.** Mounted read-write as the product volume when the boot device is the stick. What lives
   there (Arthur's memory, settings, user files), and what happens when the area is missing: the boot carries on
   read-only and nothing hangs.
4. **Safety:** the internal NVMe is never written (the Zenbook standing rule). The guard is the enforcement, and a
   scenario proves an attempted write to an unclaimed disk is refused. Guard-control features stay unbuildable with
   bare-metal, product and fb-progress-console.
5. **Encryption:** out of scope for v1, as a named later step (Tails encrypts its persistent storage). State what
   v1 stores in the clear.
6. **The rungs** (R1 design, then the write path, then the area and its mount, then the showcase). Give each a gate,
   a kill-switch (off means today's read-only behaviour, bit for bit) and an oracle.

**Required sections** (CLAUDE.md driver rules):
- **Prior-art survey (unfamiliar feature):** how Tails does Persistent Storage (partition layout, creation flow,
  unlock), and any other live-USB persistence schemes. Licence and last-commit date for anything we'd borrow, and
  read GPL code for the principle only. Linux `usb-storage` and `sd` for the write path, also read for the principle
  only.
- **Trap-mining** for USB mass-storage writes: the Linux `unusual_devs.h` quirks, sticks that lie about write cache
  or write protect, `SYNCHRONIZE CACHE` unsupported, 0-length or short CSWs, and reset recovery. Seed the
  failure-mode bank from these traps.
- **I-14 fault containment:** the DMA path goes through the fenced arena and the existing IOMMU containment, and no
  AI-governance lock is taken on the USB completion path.
- **Blind-hardware failure modes and discriminators** for the Zenbook run (`docs/runbook-baremetal-zenbook-s14.md`),
  each with what's printed on screen or written to the new stick log to tell it apart.
- **Concurrency pitfall-bank** consultation for the write path and writeback.

Close step 1: open a PR with the THINK and post PM:. The PM queues the cross-family read (Dexter) before any build is
dispatched.

## Then

The PM dispatches the rungs from the approved THINK. The 0.1 showcase (one ISO, one Zenbook boot, all five rungs)
follows the last rung.
