# Bible Amendment Proposal: Article 9 expansion + new Article 28 (Product Accessibility)

**Date:** 2026-05-10
**Author:** PM (drafting)
**Status:** Proposal. NOT ratified. Per Bible Article 27, amendments require project-lead approval; this draft sits on disk awaiting that decision.
**Articles modified:** Article 9 (Drivers and Hardware Abstraction) — expanded. Article 28 (Product Accessibility) — new.
**Bible version bump on ratification:** 1.0.0 → 1.1.0.

---

## Rationale

The current bible commits to "x86_64 (and later aarch64)" architectures (Preamble), names "USB, SATA, NVMe" as priority device classes (Article 9.4), commits to a "POSIX compatibility shim for porting userland tools" (Article 3), and supports filesystem interop via "FAT32 and ext4 read-only" drivers (Article 10.4). These are real commitments that point at running on common hardware.

What the bible does *not* yet articulate:

1. **What hardware classes AltimitNeo targets** (laptop? desktop? server?). Without this, scope decisions about driver coverage have no test.
2. **What input devices are first-class** (Article 9.4 says PS/2 is "Phase 1 boot only" — too narrow for an OS people interact with).
3. **Phase-binding for ARM** (committed-to but not phase-bound).
4. **The end-user accessibility goal itself.** AltimitNeo's reason-for-being articles (1, 2, 3) define what the OS *is*, not who it's for. The product goal of "real people can use this on hardware they already own without an impossible learning curve" needs explicit bible-level commitment so it anchors future scope decisions.

This amendment adds those four pieces. It does NOT add features the project hasn't planned (no full disk encryption, no ZFS-inspired filesystem replacement, no RISC-V — those remain ADR-territory if and when they're considered). It does NOT relax Article 1.4 (no parity-chasing); accessibility is a different goal from feature parity.

---

## Proposed Article 9 expansion

Current Article 9 has four sub-clauses (9.1-9.4). The proposal keeps all four and adds 9.5-9.10.

### Article 9.5 — Architecture coverage (new)

9.5 The supported CPU architectures, with phase-binding, are:

- **x86_64.** Primary target. Required from Phase 0. All current development.
- **aarch64.** Secondary target. Phase 5 or later, after the AI subsystem foundation lands; the foundation work is architecture-portable by design but bringup requires real boot validation on at least one ARM target.
- **RISC-V (riscv64).** Not in current plan. Adding RISC-V requires an ADR with contributor commitment. RISC-V is mentioned here only to make the negative explicit.

Architectures beyond these three are out of scope until proposed via ADR.

### Article 9.6 — Hardware target classes (new)

9.6 AltimitNeo targets the following hardware classes, in priority order:

- **Modern x86_64 laptops with UEFI firmware** (Intel and AMD). Primary target. Driver coverage prioritizes what's needed to boot and run on this class.
- **Modern x86_64 desktops with UEFI firmware** (Intel and AMD). Same priority as laptops; the driver overlap is large.
- **x86_64 servers with UEFI.** Stretch target. Server-specific concerns (BMC, IPMI, advanced storage controllers) are not blockers.
- **aarch64 development boards** (post-Phase-5). Specific boards named via ADR when bringup begins.
- **Embedded / SoC.** Out of scope without ADR.

Legacy BIOS systems are not supported (Article 9.3 already establishes this). 32-bit x86 is not supported.

### Article 9.7 — Input devices (replacing the "PS/2 only as Phase 1 boot requires" framing in 9.4)

9.7 Input device support, in priority order:

- **USB HID (keyboards, mice, touchpads).** Primary input class. Required from Phase 4 (when USB lands). Touchpad gestures and pointing are kernel concerns to the extent of producing input events; gesture interpretation is userspace.
- **PS/2 keyboards and mice.** Graceful-degradation support for older laptops and emulated environments. Required from Phase 1.5 forward (early enough that text-mode userspace is usable). Not a blocker for development, but shipped in real builds.
- **USB gamepads, joysticks, drawing tablets.** Phase 8+ (userland phase). Not a kernel-blocker.
- **Bluetooth input.** Phase 8+ at earliest. Requires Bluetooth stack which isn't in any current phase plan; ADR if pursued.

### Article 9.8 — Display (new)

9.8 Display support:

- **UEFI-supplied framebuffer.** Required from Phase 0. Done.
- **VESA / VBE framebuffer.** Fallback for non-UEFI setups; not currently a target since legacy BIOS isn't supported.
- **GPU 2D/3D acceleration.** Out of scope for the kernel. Accelerator drivers exist for AI/compute (Article 6 + ANSPEC-001); graphics acceleration is a separate driver class deferred until userspace graphics needs it (Phase 8+).
- **AI accelerators.** Their own driver class per Article 6.4 and ANSPEC-001; not graphics.

### Article 9.9 — Storage (clarifying 9.4 with phase-binding)

9.9 Storage device support, with phase-binding:

- **NVMe over PCIe.** Required from Phase 3 (block layer phase).
- **SATA AHCI.** Required from Phase 3.
- **virtio-blk.** Required from Phase 3 for VM testing.
- **USB mass storage.** Phase 4+ (after USB lands). Boot-from-USB support specifically targeted here.
- **eMMC / SD / NVMe-of-M.2.** Phase 5+ when laptop support broadens.
- **IDE.** Not supported. Article 9.4's reference to legacy IDE applies only to Phase-1-boot necessity; production AltimitNeo does not target IDE.

### Article 9.10 — Networking (clarifying)

9.10 Networking device support:

- **virtio-net.** Required from Phase 4.
- **e1000e.** Required from Phase 4 (real hardware coverage).
- **Realtek RTL8169-class** and **Intel ixgbe-class** (10G server NICs). Phase 5+ as available.
- **WiFi.** Out of scope until Phase 8+ at earliest. Requires substantial driver work (firmware blob handling, 802.11 stack); ADR if pursued.
- **Cellular modem.** Not in scope.

---

## Proposed new Article 28 — Product Accessibility

(Inserted after Article 27 "Amendments." This is a Part-V-equivalent / governance-adjacent article since it constrains all future technical articles' scope decisions.)

### Article 28 — Product Accessibility

28.1 AltimitNeo is built to be usable by real people on hardware they already own. This is a product principle, not a marketing slogan; it constrains scope decisions throughout the project.

28.2 Concretely, "usable by real people" means:

- **Installable on a common modern laptop or desktop** (per Article 9.6) without requiring exotic hardware, custom firmware, or developer-grade test rigs.
- **Boot, log in, type, point.** A user with a fresh AltimitNeo install on supported hardware can power on, see a working display, use their keyboard and mouse/touchpad, and reach a working userspace shell or graphical environment (depending on phase). This is the floor.
- **Read their existing files.** AltimitNeo can read the FAT32 and ext4 partitions a user already has (Article 10.4), so transitioning is not "wipe everything and start over."
- **Run useful software.** The POSIX compatibility shim (Article 3) means common command-line software with sane build systems can be ported with reasonable engineer effort. AltimitNeo is not Linux, but it is not so alien that the existing software ecosystem is unreachable.

28.3 Accessibility is bounded, not infinite. AltimitNeo does not aim to be Linux. It does not aim for hardware coverage parity with any existing OS (Article 1.4 still applies). The accessibility goal is satisfied when a competent user with supported hardware can install AltimitNeo and accomplish meaningful work without expert-level system administration knowledge.

28.4 Decisions about scope expansion (new device class, new architecture, new userspace feature) are evaluated against accessibility: does this expansion materially help a real user reach the bar in 28.2? An expansion that helps is candidate scope; one that doesn't is deferred or rejected. This test is applied alongside Articles 1.2 (correctness > coherence > performance) and 1.4 (no parity-chasing); it does not override them.

28.5 Accessibility is a goal, not a delivery date. Phases 0–4 build the substrate that makes accessibility possible; phases 5–7 deliver the AI-native value proposition; phase 8+ delivers the userland that brings the accessibility goal within reach of non-developers. AltimitNeo at Phase 1.4 is not yet accessible to a non-developer; the bible commits to making it so over the project's lifetime.

---

## What this amendment does NOT do

For clarity, the amendment does not:

- **Add full disk encryption** to the bible. FDE is a real concern but requires its own design work; ADR territory.
- **Change NeoFS** (Article 10.2). NeoFS remains content-addressed, not ZFS-inspired. AltimitNeo's storage design choice stands.
- **Add RISC-V to phase plans.** RISC-V is mentioned in 9.5 only to be explicit that it's NOT in plan; adding it requires an ADR.
- **Relax Article 1.4** (no parity-chasing). Accessibility is a different goal. The text in 28.3 makes this explicit.
- **Add WiFi, Bluetooth, GPU-graphics, or cellular** to current phase plans. Each is mentioned with deferred phase-binding so future ADRs have a starting point, but none are committed to.
- **Promise hardware support beyond what driver writers can actually deliver.** Phase-binding throughout 9.5–9.10 is honest about delivery timing.

---

## Compatibility with existing bible articles

- **Article 1.4 (no parity-chasing)** — preserved. Accessibility is not parity. Article 28.3 makes this explicit.
- **Article 3 (what we are not)** — preserved. POSIX compatibility shim was already in scope; Article 28 references it without expanding.
- **Article 9.1–9.4** — preserved. The new sub-clauses extend rather than replace the existing ones. 9.4's PS/2-only-for-Phase-1-boot framing is superseded by 9.7's explicit input-device commitment.
- **Article 10.2 (NeoFS)** — preserved unchanged. NeoFS remains content-addressed.
- **Article 10.4 (FAT32 + ext4 interop)** — preserved unchanged. Article 28.2 references it.
- **Article 13 (phase ledger)** — no change to phase definitions. Phase-binding in 9.5–9.10 is consistent with the existing phase plan; no phases get new criteria from this amendment.
- **Article 14 (algorithm)** — preserved. This amendment goes through the standard amendment process per Article 27.
- **Article 24.1 (project lead decides)** — preserved. This proposal awaits project-lead ratification.

---

## Version bump

On ratification, bible footer changes from `Version: 1.0.0` to `Version: 1.1.0`. Last-amended date updates to ratification date.

---

## Cross-references

- `docs/bible.md` v1.0.0 — the document this amendment proposes to modify.
- `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md` — recently filed; mentions Redox's `switch_finish_hook` mechanism which is candidate prior art for unrelated SwitchMutex structural ADR. This amendment intentionally does not borrow Redox's feature list; the Redox lineage stays in research artifacts.
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` §2.1 — Redox's prior-art relevance is mechanism-level, not feature-list-level.
- ANSPEC-001 — AI subsystem spec, referenced by Article 9.8's note that AI accelerators are their own driver class.

---

## Project-lead decision required

Three options:

- **Ratify as drafted.** Update Bible to v1.1.0 with these sections.
- **Ratify with edits.** Project lead names changes; PM redrafts.
- **Reject.** Amendment doesn't land; carry-forward note records the decision.

Decision goes in the bible amendment's commit message + a short ADR if substantive design discussion accompanies it.

---

**End of proposal.**
