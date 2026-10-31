<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: the VT-d bring-up window + deny-by-default — a pre-condition for the first untrusted PCIe bus master

**Date:** 2026-06-17
**Author:** Claude (PM), from a PL adversarial-test session
**Status:** FINDING — for ratification as a pre-condition ADR on the accel-driver-class lane
**Invariant:** I-14(c) memory isolation (`docs/complexity-governance.md`); relates ADR-045 (fenced DMA arena), ADR-050 (VT-d/IOMMU)

---

## 1. The attack (PL, adversarial test)

> Once VT-d is active it traps unauthorized DMA. But there is a temporal window during driver bring-up: between the moment hardware is powered on and the moment the IOMMU page tables for that device's domain are constructed and translation is enabled. How does LogansOS stop a malicious/buggy driver from issuing a DMA in that window to overwrite kernel structures **before the fence exists**?

## 2. Verdict: defended for the actual connectivity lane; a real residual for a device class not yet built

### 2A. The posed attack does not reach the Phase-7.5 connectivity drivers — they are not bus masters

The MT7601U WiFi driver and the CDC-ECM USB-Ethernet driver are **USB devices**, not PCIe bus masters. A USB device has no DMA engine of its own; it can only respond to transfers the **xHCI host controller** programs for it (`kernel/src/drivers/usb/mt7601u/mod.rs:44` claims the device *through* `xhci::ClaimedDevice`; every bulk/MCU transfer is gated `dma_arena::fence(...) → FencedDmaAddr` first — lines 364, 384, 435). However malicious the device firmware is during bring-up, it has nothing to fire. The sole master in the lane is xHCI.

### 2B. The software fence has **no** bring-up window — by design, it precedes the hardware fence

The connectivity threat model (I-14(c)) is defended by the **software** `dma_arena` (BUILD-7.5-0, I-14(c1)), which shipped *before* VT-d (BUILD-7.5-V, I-14(c2)) precisely so this gap was never open. The arena does not gate *bus access* (a hardware structure that goes live at some moment); it gates *address-minting*: `dma_arena::fence(phys, len)` (`kernel/src/drivers/dma_arena.rs:142`) is the only producer of a `FencedDmaAddr` (`dma_arena.rs:90`), and `FencedDmaAddr` is the only currency the xHCI programming path accepts (BUILD-7.5-A made every TRB/DCBAA/buffer address a `FencedDmaAddr`, grep-asserted, type-enforced). A connectivity driver **cannot express** an out-of-arena DMA address — `fence()` refuses it and bumps `fence_refusals` — and that is true from the first instruction of boot, with or without VT-d. There is no table to race because the gate is in the code path that hands out addresses. *Hardware fence has a construction moment; software fence does not. The connectivity lane rides the one without a window.*

### 2C. Adversarial verification of §2B (2026-06-17) — the xHCI bootstrap is fenced from the first programmed address

The §2B claim was attacked directly: *is the xHCI controller's own ring/context setup `FencedDmaAddr`-only from the very first programmed address, or is there a bootstrap moment where it writes a raw phys addr before the arena discipline applies?* Verified against `kernel/src/drivers/usb/xhci.rs` — the claim **holds and is type-enforced**, not merely grep-asserted:

- **Arena is a hard precondition.** `xhci::init` calls `dma_arena::init(ARENA_PAGES)` (`xhci.rs:370`) and then `if dma_arena::bounds().is_none() { … "refusing unfenced bring-up"; return }` — bring-up aborts if the arena does not exist. The fence precedes the feature at the host-controller level, not just the device level.
- **Every controller DMA-pointer register is programmed with a fenced address:** DCBAA (`OP_DCBAAP` ← `dcbaa_addr.phys()`), scratchpad array/pages, command ring (`OP_CRCR` ← `cmd_addr.phys()`), ERST base (`RT_ERSTBA0` ← `erst_addr.phys()`), event-ring dequeue (`RT_ERDP0` ← `ev_addr.phys()`), and the Link TRB — each address comes from `fence_buffer()` (`xhci.rs` ~448–492).
- **The single-currency discipline is enforced by the type system, not convention.** `fence_buffer` → `dma_arena::fence` → `dma_arena_logic::validate(phys,len,base,arena_len)`, which bounds-checks against the arena and *refuses* anything outside (→ "buffer outside the fence" → bring-up fails). The pointer-writing helpers `write_dma_u64` (`xhci.rs:537`) and `arm_link_trb` (`xhci.rs:549`) **take `FencedDmaAddr` by type** — a raw `PhysAddr` cannot be programmed. `DmaBuffer::alloc_in_arena` (`block/dma.rs:92`) allocates structurally within the arena (`alloc_span`: `base + offset`).
- **Run/Stop is gated last.** Halt + `HCRST` reset (clears firmware-era controller state, xHCI §4.2) happens before any ring is programmed, and `USBCMD_RS` (the bit that lets the controller initiate ring-DMA) is set **after** every pointer is fenced (`xhci.rs` ~508). The controller cannot self-initiate DMA against an unfenced address at any point in the OS-controlled path.

**Conclusion:** §2B survives. There is no OS-controlled bring-up window for the software fence. The single genuine residual is the **firmware-era window** (between BME at `xhci.rs:391` and the `HCRST` reset, *if* platform firmware left the controller running with stale ring pointers) — but a firmware-left-running controller already had BME (the OS's BME write opens no new window), the OS halts+resets early as the correct mitigation, and this is the pre-boot DMA concern of §6.4, not an arena-discipline hole.

## 3. Where the attack **does** land — the pre-VT-d bus-master window (a real finding)

Boot order in `kernel/src/main.rs` (the `vtd` build):

```
mm::init()                                  # frame allocator, HHDM
acpi::init(rsdp)            # main.rs:431    # DMAR parsed — READ-ONLY; VT-d not enabled
arch::x86_64::smp::init_bsp_apic()
drivers::block::init()     # main.rs:449    # nvme/ahci/virtio_blk → enable_memory_and_bus_mastering()  ← BME set
drivers::net::init()       # main.rs:450    # virtio_net → BME set
drivers::usb::init()       # main.rs:452    # xhci → BME set
drivers::iommu::init()     # main.rs:460    # ← VT-d translation enabled HERE, identity domains, LAST
```

`pci::enable_memory_and_bus_mastering` (`kernel/src/drivers/pci.rs:529`) sets the bus-master bit **unconditionally** during each device's probe, and every probe runs **before** `iommu::init()`. Callers that set BME in the window: `nvme.rs:143`, `ahci.rs:141`, `virtio_blk.rs:209`, `virtio_net.rs:177`, `xhci.rs:391`, and — relevant to §5 — `accel_intel.rs:264`, `virtio_accel.rs:302`.

Two facts from the VT-d driver sharpen this:
- **No deny-by-default.** `iommu::init` builds the identity domain, places **every enumerated device into identity** (GPA==HPA), *then* enables translation (`drivers/iommu/mod.rs` init: identity loop ~lines 71–92, `enable_unit` ~line 116). At the instant translation flips on, devices have full physical reach; before it, they had physical reach anyway (TES=0). There is no interval in which the *default* device state is "blocked."
- **The fence-domain narrowing is not in the production probe path.** `fence_connectivity()` / `fence_source_id()` (`mod.rs:506,538`) are invoked only from `test_harness.rs:7876` (the (c2) proof scenario). In a production boot, connectivity devices remain in the identity domain; their containment is the software arena (§2B), not the VT-d fence domain. (This is acceptable *today* — the arena is the real connectivity guard — but it means VT-d currently provides the substrate identity-mapping + the (c2) *proof*, not a production per-device fence.)

## 4. Why this is benign **today**

The only BME-enabled devices in the window are NVMe / AHCI / virtio / xHCI — **first-party, in-tree controllers, part of the TCB.** They are identity-domained by design; VT-d grants a trusted controller no additional containment (GPA==HPA either way). I-14's threat model is explicitly *connectivity/accel device faults* (untrusted firmware/DMA), **not** the substrate controllers. So nothing untrusted is live during the window. The window is real but, under the current device set, contains only trusted code.

## 5. The residual gap (sharp): the first untrusted PCIe bus master

The instant we build an **untrusted PCIe bus-master accelerator** (the roadmap's accel-driver class — a real 3rd-party GPU/accel card with vendor firmware), its probe will call the same `enable_memory_and_bus_mastering` (cf. `accel_intel.rs:264`, `virtio_accel.rs:302`) **before** it is fenced, and the window becomes a live attack surface: BME set + TES off + identity-by-default = the device can DMA over kernel structures before any fence exists. This is the genuine version of the PL's attack, and it lands on a device class we have not yet built — which is the right time to fix it.

## 6. Recommended fix — invert to deny-by-default; make BME consume proof-of-fencing

Mirror the arena's single-currency trick (an address you cannot express unless fenced) at the **bus-master** layer (a master right you cannot enable unless domained):

1. **Enable VT-d early, deny-by-default.** Construct the root table with a **blocking** (or fence) context entry for un-classified source-ids and enable translation **right after `acpi::init()`, before any device probe.** The default state for anything that powers on becomes "DMA blocked," closing the window structurally rather than by probe-ordering luck.
2. **Substrate opts into identity as a probe pre-condition.** Trusted controllers request their identity context entry, then set BME — never the reverse. Re-order so `enable_memory_and_bus_mastering` is reached only *after* the device's domain is installed.
3. **Make BME take proof-of-fencing.** Give `enable_memory_and_bus_mastering` a parameter that is a token proving the device has a domain (identity for substrate, fence for untrusted) — the type-system analogue of `FencedDmaAddr`. A driver cannot enable bus mastering for an undomained device because it cannot produce the token. Wire `fence_connectivity()`/`fence_source_id()` into the production probe path for untrusted classes (not only the test scenario).
4. **Pre-boot DMA window (defense in depth).** The interval *before the OS runs* (UEFI may leave devices mastering) is closed by Intel pre-boot DMA protection — the DMAR `PLATFORM_OPT_IN_FLAG` — plus enabling VT-d with a blocking root context as the first DMA-relevant OS action. Note as a platform requirement when we target hardware where an untrusted master is present at boot.

## 7. Open decision for PM/PL ratification

- **OD-1:** Adopt deny-by-default VT-d ordering (enable early with a blocking default; opt-in to identity/fence per device) as a **mandatory pre-condition of the first untrusted PCIe-bus-master dispatch** — i.e., that build ships the §6 inversion before granting the device BME, the same way BUILD-0 shipped the arena before the first connectivity DMA. Record as an ADR amending ADR-050.
  - *Recommendation:* **Yes.** It is the I-14 "integrity-before-feature" pattern applied one layer out (bus-master rights, not just DMA addresses), and it costs far less to bake in before the accel-class lane than to retrofit after a device is already setting BME in its probe.
- **OD-2:** Until then, keep the current posture documented as *intentionally* relying on (a) the always-on software arena for connectivity and (b) the all-trusted bus-master set for the substrate — and add a **grep/CI guard** that fails if any *new* `enable_memory_and_bus_mastering` caller appears outside the known-trusted substrate list without an accompanying fence, so an untrusted master cannot be added without tripping this finding.
  - *Recommendation:* **Yes** — cheap, and it converts this THINK into a falsifiable guard rather than a note that rots.

## 8. Summary

The temporal-window attack is **defended for the actual connectivity lane**: those drivers are USB non-masters, and the software DMA arena — which gates address-minting, not bus access — has no bring-up window and predates VT-d on purpose. The attack's sharpened form reveals a **real deny-by-default gap** in the pre-VT-d bus-master window that is harmless under today's all-trusted master set but becomes live the moment an untrusted PCIe bus master is built. The fix is the I-14 integrity-before-feature pattern applied to bus-master rights, banked here as a pre-condition for the accel-driver-class lane.
