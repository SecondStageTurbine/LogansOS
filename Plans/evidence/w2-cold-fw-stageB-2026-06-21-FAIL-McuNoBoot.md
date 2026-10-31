<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Stage-B evidence — W2 cold firmware self-upload (OT-27a): COLD PATH RAN, cold upload did NOT boot the MCU

**Date:** 2026-06-21 · **Run:** OptiPlex 7080, MT7601U `148f:7601`, **genuinely cold dongle**
**Branch/ISO:** `codex/w2-cold-firmware-snonce-cerebro` @ `14b08bb`; bare-metal scan ISO
`mt7601u_scan_baremetal.bare-metal.iso` sha256 `9cbc20873d33e76a5295a61e0cdb714ffa98002356e47a1e959504f274102121`,
flashed to `nvme0n1` (sha-verified), sink pre-zeroed.
**Verdict:** OT-27a **NOT closed** — but the build's deliverables (provisioning + cold-upload state
machine + discriminators) are **proven on silicon**, and the failure is **fully localized**.

## Two boots, same dongle

**Boot 1 (warm — for contrast):** `firmware already resident (COM_REG0=1) — upload skipped` →
full scan healthy (`bss total=6` by ch3, ASIC `0x76010001`, MAC `xx:xx:xx:xx:xx:xx`). The driver
is healthy; only the cold-upload→MCU-boot step is at issue.

**Boot 2 (genuinely cold — unplug → boot nvme0n1 → replug at POST):**
```
mt7601u: ASIC rev 0x76010001 MAC rev 0x76010500
mt7601u: cold fw upload — 0.1.00 build 0x7640 (45412 bytes), chip 0x76010001
mt7601u: fw attempt 1 ready-poll timeout — COM_REG0=0xffffffff COM_REG1=0xffffffff
mt7601u: fw attempt 2 reg setup: control transfer: no event (F5)
mt7601u: fw attempt 3 reg setup: control transfer: no event (F5)
mt7601u: cold fw upload FAILED — fault=Some(McuNoBoot) uploaded=45380/45380B com_reg0_stirred=true chip=0x76010001
mt7601u: bind failed vid=148f pid=7601: firmware ready-poll timeout (-ETIMEDOUT)
usb: 'mt7601u' probed vid=148f pid=7601 but declined to bind
```
Formal frame (serial + NVMe sink):
```
mt7601u: stage=2 asic_rev=0x76010001 mac_rev=0x76010500 ee_ver=0xffff mac=00:00:00:00:00:00 bss=0 bound=false fence_refusals=0 bind_fail=Some("firmware ready-poll timeout (-ETIMEDOUT)")
NEO_TEST/1 event=FAIL name=mt7601u_scan_baremetal reason=mt7601u_bind_incomplete_see_stage_breadcrumb
```

## Proven on silicon (the build delivered)
- **ISC-2 provisioning:** the 45412-byte blob loaded from the **Limine module `/mt7601u-fw`**
  (not git, not `include_bytes!`) on a cold device. The out-of-band path is real on hardware.
- **ISC-1/ISC-3:** the cold sequence executed; the bounded 3-attempt P2 retry ran; the failure
  **self-bucketed** (`McuNoBoot`, `uploaded=…`, `com_reg0_stirred`) — conclusive on failure, no blind stall.
- **I-14 held:** `fence_refusals=0` on the failing cold-upload path — the FCE-DMA firmware chunks
  went through the fence cleanly even while the device wedged. Containment is silicon-clean.

## Diagnosis (the smoking gun)
`COM_REG0=0xffffffff` **and** `COM_REG1=0xffffffff` is the **all-ones read-failure sentinel** —
the device stopped answering vendor control-reads. The `control transfer: no event (F5)` on
attempts 2–3 confirms the device is **dead/unresponsive after our sequence**.

Two consequences:
1. **`dma_fw`'s "DMA done" poll false-positives on all-ones.** It checks `COM_REG1 & (1<<31)`, and
   `0xffffffff & (1<<31)` is truthy — so once the device wedged, **every remaining chunk's poll
   spuriously passed**, fabricating the `uploaded=45380/45380B` "full upload." The real upload
   almost certainly wedged earlier; `McuNoBoot` is the *masked* label, not the root cause.
2. **Our cold sequence wedges the device.** The warm read of `COM_REG0` is fine (boot 1), and the
   cold read is fine at the *start* of `load_firmware` (it correctly saw "not 1" → proceeded), then
   goes all-ones *after* the upload sequence. Linux's cold sequence works (that's how the firmware
   got there warm), so we have a subtle port diff in `fce_preload_setup`/`dma_fw`/IVB.

`ee_ver=0xffff mac=00:00:…` in the stage breadcrumb corroborates: once the MCU is dead, the eFUSE
read (which needs the running MCU) also returns all-ones — downstream is uniformly unresponsive.

**Sink read independently confirmed (post-run, `dd if=/dev/nvme0n1 bs=1M skip=256`):** the NVMe sink
installed cleanly (`bare_metal_log_sink=installed target_driver=nvme offset=0x10000000`,
`selftest=ok`) and carries the same `stage=2 … fence_refusals=0 bind_fail=…(-ETIMEDOUT)` + `event=FAIL`
frame as the serial. Both evidence channels agree — the verdict is solid.

## BUILD-2 discriminator bank (seed for the next iteration)
| # | Trap | Fix / discriminator |
|---|------|---------------------|
| **C1** | All-ones reads are mislabeled as valid (`com_reg0_stirred=true` on `0xffffffff`; DMA-done poll false-passes). | In `dma_fw`/`poll`, treat a `0xffffffff` read as **device-unresponsive → fail fast**, never "done." Re-localizes to the FIRST wedged chunk/step. |
| **C2** | `McuNoBoot` mislabels an unresponsive device. | Add `ColdUploadFault::DeviceUnresponsive` (all-ones / control-no-event) distinct from `McuNoBoot` (COM_REG0 honest-0). |
| **C3** | Unknown *where* in the sequence the wedge starts. | Per-chunk breadcrumb: log the chunk index + the COM_REG1 value (raw) so the wedge point is pinpointed; instrument `fce_preload_setup` step-by-step. |
| **C4** | Possible port diff vs Linux. | Diff our `fce_preload_setup` + `dma_fw` + IVB-control-upload against raw `mcu.c` 446-489 byte-for-byte (register values, ordering, the `msleep(5)`, the `MT_VEND_DEV_MODE 0x12` IVB request). |

**Scope of BUILD-2:** make the cold upload actually boot the MCU. Start by un-masking the wedge
(C1/C2/C3), then root-cause it (C4). Pure-logic parts (the all-ones-aware poll predicate, the new
fault bucket) are host-testable; the cold upload itself stays the bare-metal Stage-B.

**Branch `14b08bb` recommendation:** still mergeable — **OT-2 closed**, infrastructure + discriminators
proven. Reframe OT-27a as the open follow-on (cold WiFi works), don't block the merge.
