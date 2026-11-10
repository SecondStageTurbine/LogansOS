<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 385: Dexter, cross-family read of the OT-100 THINK (Tails-style USB persistence) (2026-11-10)

**MODEL** DeepSeek, high. Why: a Claude-authored design for a new storage write path on the PL's work laptop. It
needs a different-family read before any build.
**Read:** PR #103 at `a94a5e60`, `docs/THINK-2026-11-10-ot100-usb-persistence.md`, from the repo. Also read the code
it builds on:
- `kernel/src/drivers/usb/msc.rs` (`write_block` is `Unsupported` today);
- `kernel/src/drivers/block/mod.rs` (`GuardedDevice`) and `write_guard_logic`;
- the NeoFS mount path;
- `docs/runbook-baremetal-zenbook-s14.md`.

**RETURN:** `Plans/RETURN-2026-11-10-pr103-ot100-think-read-dexter.md` on `dexter/pr103-ot100-think-read`, pushed.

## Checklist (pass or fail, with `file:line` or THINK section, and a concrete input for any fail)

- [ ] **Never the internal drive:** the design keeps every write out of the Zenbook's internal NVMe by the existing
  guard's rule (a sealed-marker extent that no partition table allocates). Any guard change is relax-and-re-prove, with
  a scenario that a write to an unclaimed disk is refused.
- [ ] **The layout is sound:** the persistence area's placement and creation can't collide with the boot partitions,
  survive Rufus DD imaging (the stick is rewritten whole each time), and are recognised on boot without guessing.
- [ ] **The MSC write path:** WRITE(10)/(16), SYNCHRONIZE CACHE, write-protect via MODE SENSE, CSW/sense handling,
  removal mid-write and max transfer are each designed, and the trap-mining bank covers the known
  `unusual_devs` / write-cache-lies class with a discriminator per trap.
- [ ] **Crash consistency:** with NeoFS on the area, a pulled stick or a power cut mid-write leaves a mountable volume.
  Say where the design relies on NeoFS's CoW and where on the device honouring a flush.
- [ ] **I-14:** the DMA path is fenced and no AI-governance lock is on the USB completion path.
- [ ] **Missing area:** with no persistence area, the boot carries on read-only and nothing hangs.
- [ ] **Rungs:** each has a gate, a kill-switch (off means today's read-only behaviour, bit for bit) and an oracle,
  and the blind-hardware failure-mode table has a discriminator visible on screen or in the stick log.
- [ ] **Prior art:** the Tails survey and the Linux references are read for principle only (GPL), with licences and
  dates given.

**Close condition (diagnosis-style read):** for each risk you rate high, give a hypothesis of how it fails and a named
test that would catch it.
