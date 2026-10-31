<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: AAG delta re-check — Slice-4 M710q Gen9 EU (`72570f72`) — Jarvis

**Date:** 2026-08-01
**Dispatch:** `Plans/DISPATCH-2026-08-01-aag-slice4-delta-jarvis.md`
**Candidate:** `origin/codex/slice4-m710q-hd530-gen9-allowlist-cerebro` @ `72570f72`
**Reviewer:** Jarvis (Codex-family), cross-family on Cerebro's work and author of the original I-14(c) bounce
**Mode:** doc-only delta re-check. No build or gate was run.
**RAG:** healthy (`ollama=true`, `qdrant=true`).

## Verdict

**AAG CLEAR — I-14(c) remains closed; no rebound finding. Merge-ready.**

The repair removes the previously reachable shape: a real iGPU receiving bus-master
rights before DMA containment exists, then receiving unrestricted buddy-backed
GPU buffers. The refusal path is fail-closed, its hardware-state assertion has real
teeth, and the BAR check is deliberately limited to the accel path that needed it.

The two proof boundaries below are recorded, not blockers. The first is a required
precondition for the planned M710q bare-metal refusal run; the second separates a
structural degradation proof from operational proof.

## Delta checks

| Check | Result | Evidence |
|---|---|---|
| 1. Bus-master ordering | **Clean** | `accel_intel::probe_pci` first permits memory decode, maps BAR0, and validates GT offsets. Every failure before the gate returns without calling `enable_bus_mastering`; the only call is after `graphics_arena::containment_armed()` succeeds. |
| 2. No unrestricted hardware fallback | **Clean** | `DmaBuffer::alloc_gpu` uses the graphics arena when narrowing is on. If a real accelerator is bound while it is off, it returns `DmaError::ContainmentRequired`; only simulated/no-device execution retains the pre-Slice-5 `alloc_zeroed` buddy path. |
| 3. Refusal assertion has hardware teeth | **Clean** | `debug_command_register()` loads the matched probe BDF from `LAST_BDF`, calls `pci::command_register_at`, then `config_read_u16`/`config_read_u32`. The latter writes the BDF+offset address to **CF8** and reads **CFC**. The scenario therefore reads the device's live COMMAND register, not a cached value or a printed string. |
| 4. VT-d-off degradation and discriminator | **Structurally clean; unexercised** | `arm_boot_containment()` runs before `ai::init()` reaches the accel probe. If VT-d is unavailable or disabled it leaves containment false; the probe refuses BME, leaves the device decode-only/unregistered, and the CPU accelerator remains available. The refusal breadcrumb reports `vtd_available`, `vtd_enabled`, narrowing, arena, and `domain_armed`, separating firmware-off (`false`/`false`) from an arena/domain-arming failure. |
| 5. BAR-size validation scope | **Clean** | `map_bar_region_validated` checks `offset + length` against the advertised BAR size and has exactly one production caller: `accel_intel::probe_pci` for GTTMMADR. The six existing generic BAR mappings remain on `map_bar_region`; widening this is correctly left to OT-72 after the reverted `1255b665` regression. |

### 1. Fail-closed ordering

`probe_pci` stores the matched BDF, rejects unsupported/already-attached devices,
enables decode only, then maps and validates the GTT MMIO window. Only after
`containment_armed()` confirms all three facts — narrowing policy enabled, graphics
arena reserved, and iGPU requester moved into the graphics VT-d domain — does it
call `pci::enable_bus_mastering` and mark hardware acceleration bound. No error or
early-return path grants BME.

This closes the original finding. Host-initiated MMIO setup is permitted ahead of
containment; device-initiated DMA is not.

### 2. Allocation chokepoint

The fallback rule is now keyed to whether a *real* DMA master is bound. A hardware
accelerator cannot receive buddy-backed GPU memory when graphics narrowing is off:
`alloc_gpu` returns `ContainmentRequired`. The simulation/no-device case has no
hardware DMA master and remains byte-for-byte on the prior buddy allocation path,
which preserves the Rule-1 kill-switch equality without using it to excuse a real
uncontained device.

### 3. Repurposed refusal scenario

The hardware assertion is not self-referential. The scenario requires a probed,
matched, unsuccessful accel probe; no bound hardware accelerator; no registered
Intel device; and COMMAND.BUS_MASTER clear. Its COMMAND value traverses:

```text
debug_command_register()
  -> LAST_BDF (matched probe)
  -> pci::command_register_at(bus, device, function)
  -> config_read_u16(..., 0x04) -> config_read_u32(...)
  -> outl(CF8, encoded BDF/COMMAND) + inl(CFC)
```

It thus proves the safety fact the old identity assertion could not: a refused
device has not been given live hardware DMA authority.

### 4. VT-d degrade

The boot path calls `iommu::init`, then `graphics_arena::arm_boot_containment`,
before `ai::init` can initialize `accel_intel`. A missing/disabled VT-d unit, an
arena-reservation failure, a narrowing-policy failure, or no successfully narrowed
graphics requester all leave containment disarmed. The subsequent Intel probe
therefore refuses bus mastering; it does not abort boot. The CPU floor is separately
registered and is asserted by the refusal scenario.

This is the right fail-closed structure. It is not yet an operational demonstration
that a firmware-off M710q boots, composites, and lets Arthur answer on the CPU
floor; see proof boundary 2.

### 5. BAR validation is intentionally local

The candidate's checked mapper refuses unknown/too-small memory BARs before mapping
the requested region. Its sole consumer is the Intel GT probe, so the fix validates
the GTTMMADR claim without changing xHCI, AHCI, NVMe, virtio, or other established
BAR behavior. The measured xHCI/AHCI over-mapping question is OT-72, explicitly out
of this delta.

## Proof boundaries

### 1. The refusal scenario does not latch the error kind

The scenario asserts a matched-but-unsuccessful probe, no registration, and BME
clear. An earlier `MissingGttMmadr` or `MmioOffsetOutOfRange` result after matching
the device would satisfy those same assertions. Therefore the CF8/CFC read proves
the **safety property** (no accidental BME), but not that the specific
`ContainmentNotArmed` branch was reached.

This is not a merge blocker: this hardware-only scenario cannot run in CI, and the
guard itself is structurally correct. It **is** a precondition of the M710q
bare-metal refusal run. Add and assert a latched probe-error kind before that boot,
so either a pass or a failure is conclusive rather than consuming a blind-hardware
opportunity ambiguously.

### 2. VT-d-off degradation is structurally established, not exercised

The 11/11 candidate gate cited by the dispatch did not boot firmware-off M710q
hardware or exercise the no-GPU/CPU-floor/Arthur operational path. The ordering,
state gate, refusal behavior, and breadcrumbs establish that degradation by code
structure; they do not replace that bare-metal observation.

The firmware-off M710q refusal run is the acceptance milestone: it must show the
firmware-off discriminator, refused iGPU BME, continuing boot/composition, and an
answer from the CPU floor. This is an acceptance-bar proof, not a condition for this
gate-and-AAG merge decision.

## Scope and recommendation

The original GT/EU coupling and allowlist/KAT review remains unchanged by this
delta. The two documented boundaries do not reopen I-14(c), and neither authorizes
scope expansion into OT-72. Merge the candidate; add the error-kind latch before
the M710q hardware run, then treat that run as the operational proof of the VT-d-off
degrade path.
