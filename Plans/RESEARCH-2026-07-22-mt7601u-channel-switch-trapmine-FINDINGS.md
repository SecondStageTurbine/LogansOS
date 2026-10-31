<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: MT7601U channel-switch wedge trap-mine — two open questions answered (step 1 of trap→fix→test)

**Date:** 2026-07-22
**Lane:** Prometheus (deep prior-art mining — NO kernel code)
**Delivers for:** `DISPATCH-2026-07-22-mt7601u-channel-switch-trapmine-prometheus.md`
**Sources:** Linux kernel mt7601u driver (`drivers/net/wireless/mediatek/mt7601u/`, mainline v6.8+), our driver `kernel/src/drivers/usb/mt7601u/{mod.rs,phy.rs,mcu_logic.rs,upload_logic.rs,fw_logic.rs}`

---

## 1. Q1 — How does upstream reach the operating channel WITHOUT a wedging mid-stream switch?

### 1.1 Is the wedge the channel-set or the post-channel MCU calibration?

**Answer: The wedge is the `set_channel` + inline `set_bw_filter` (MCU CAL_BW) path, NOT the post-channel deferred calibration.**

Upstream's `mt7601u_phy_set_channel` (`phy.c:442-465`) has two phases:

```c
// phy.c:442 — outer wrapper
int mt7601u_phy_set_channel(struct mt7601u_dev *dev, struct cfg80211_chan_def *chandef) {
    cancel_delayed_work_sync(&dev->cal_work);      // cancel periodic cal
    cancel_delayed_work_sync(&dev->freq_cal.work);  // cancel freq cal

    mutex_lock(&dev->hw_atomic_mutex);
    ret = __mt7601u_phy_set_channel(dev, chandef);  // inline channel switch
    mutex_unlock(&dev->hw_atomic_mutex);
    if (ret)
        return ret;

    if (test_bit(MT7601U_STATE_SCANNING, &dev->state))
        return 0;          // ← SCANNING: DONE HERE. No further MCU cmds.

    ieee80211_queue_delayed_work(dev->hw, &dev->cal_work, MT_CALIBRATE_INTERVAL);
    ieee80211_queue_delayed_work(dev->hw, &dev->freq_cal.work, MT_FREQ_CAL_INIT_DELAY);
    return 0;
}
```

**Critical: During scanning (`MT7601U_STATE_SCANNING` set), upstream returns immediately after `__mt7601u_phy_set_channel`. It does NOT queue the deferred calibration work.** The only MCU commands issued per channel hop are inside `__mt7601u_phy_set_channel`:

```c
// phy.c:342 — inner channel switch (the ONLY path during scan)
static int __mt7601u_phy_set_channel(struct mt7601u_dev *dev, ...) {
    // 1. RF freq plan via MCU RANDOM_WRITE to MT_MCU_MEMMAP_RF (regs 17-20)
    ret = mt7601u_write_reg_pairs(dev, MT_MCU_MEMMAP_RF, channel_freq_plan, 4);

    // 2. TX_ALC_CFG_0 direct register RMW (host-side, no MCU round-trip)
    mt7601u_rmw(dev, MT_TX_ALC_CFG_0, 0x3f3f, chan_pwr & 0x3f);

    // 3. BBP LNA gain via MCU RANDOM_WRITE to MT_MCU_MEMMAP_BBP (regs 62-64)
    ret = mt7601u_write_reg_pairs(dev, MT_MCU_MEMMAP_BBP, bbp_settings, 3);

    // 4. VCO cal — direct RF register writes, NO MCU round-trip
    mt7601u_vco_cal(dev);

    // 5. BBP set_bw — if BW unchanged (scan is always BW20), EARLY RETURNS after one
    //    host-side BBP RMW. No MCU cmds, no MAC stop/restart, no BBP table reload.
    mt7601u_bbp_set_bw(dev, bw);

    // 6. set_bw_filter — MCU CAL_BW calibration (TX then RX), wait_resp=true each
    ret = mt7601u_set_bw_filter(dev, false);   // ← MCU CMD #1 per hop

    // 7. TX_PWR_CFG_0 direct register write (host-side)
    mt7601u_wr(dev, MT_TX_PWR_CFG_0, ...);

    return 0;
}
```

The MCU commands per channel hop during scan are:
- `RANDOM_WRITE` to RF (freq plan, may be split into multiple cmds if >24 pairs — but 4 pairs fits in one)
- `RANDOM_WRITE` to BBP (LNA gain, 3 pairs, one cmd)
- `CALIBRATION(CAL_BW)` × 2 (TX and RX, each with `wait_resp=true`)

**The wedge happens at step 6 (`set_bw_filter` → `MCU_CAL_BW`).** This is an MCU calibration command that requires a firmware response. The LKML "mt7601u dies during channel switch" bug report specifically identifies the MCU firmware crashing during this calibration round-trip on certain channels. The RF/BBP register writes (steps 1-3) are `RANDOM_WRITE` commands — simpler than `CALIBRATION`, and they don't trigger internal firmware state machines that can deadlock.

**Our driver's `set_channel` (`phy.rs:385-442`) faithfully reproduces this exact sequence**, including the `set_bw_filter` → `mcu_calibrate(MCU_CAL_BW)` calls at lines 423-424. So our driver hits the same wedge because it follows the same upstream path.

### 1.2 Can the cold MCU come up already tuned to the target channel?

**Answer: NO — upstream does NOT set a channel during firmware boot or PHY init.**

Tracing the upstream init ordering (`init.c:317-420`):

```
mt7601u_init_hardware():
  1. chip_onoff(dev, true, false)          // WLAN enable + PLL/XTAL
  2. wait_asic_ready()                     // ASIC ready poll
  3. mcu_init() → load_firmware()          // firmware upload + boot
  4. wait_asic_ready()                     // post-FW ASIC ready
  5. reset_csr_bbp()                       // CSR+BBP reset
  6. init_usb_dma()                        // USB DMA config
  7. mcu_cmd_init() → Q_SELECT(1)          // MCU function select
  8. dma_init()                            // ring buffers, RX page alloc
  9. write_mac_initvals()                  // MAC init tables via MCU RANDOM_WRITE
  10. wait MAC not busy
  11. init_bbp() → BBP initvals            // BBP common + chip tables
  12. init_wcid_mem / key_mem / attr_mem   // WCID/key memory init
  13. eeprom_init()                        // read eFUSE/EEPROM
  14. phy_init()                           // RF tables + init_cal (full calibration)
  15. set_rx_path(0), set_tx_dac(0)        // BBP path/DAC
  16. mac_set_ctrlch(false)                // MAC control channel
  17. bbp_set_ctrlch(false)                // BBP control channel
  18. bbp_set_bw(MT_BW_20)                // bandwidth
```

**No `set_channel` call exists in the init sequence.** The chip comes up with whatever RF state the firmware's ILM/DLM initializes to internally. The first channel program happens at scan time (or when mac80211 calls `conf_chan`), which is a mid-stream switch from whatever power-on-default the firmware set.

The `rf_channel` initvals table (`initvals_phy.h`) written during `phy_init()` sets RF configuration parameters, but these are NOT channel-frequency values — they are general RF setup registers (attenuation, gain tables, etc.). The actual channel frequency is set exclusively through the `freq_plan` in `__mt7601u_phy_set_channel` (RF bank-0 regs 17-20).

**The cold MCU boots on whatever internal default the firmware chooses.** There is no upstream mechanism to pre-tune the channel before the first `set_channel` call. This eliminates Option A as a pure-initvals fix — we'd need to modify firmware behavior, which isn't feasible with a binary blob.

### 1.3 Does upstream set channel via direct RF/BBP register writes vs MCU command?

**Answer: Upstream uses MCU RANDOM_WRITE commands for the RF frequency plan, NOT direct host-side register writes.**

```c
// phy.c:409 — the freq plan goes through MCU RANDOM_WRITE
ret = mt7601u_write_reg_pairs(dev, MT_MCU_MEMMAP_RF, channel_freq_plan, FREQ_PLAN_REGS);
```

`mt7601u_write_reg_pairs` sends `CMD_RANDOM_WRITE` (MCU cmd type 12) with `wait_resp=true` for the last chunk. This is an MCU-mediated write — the firmware processes the command and writes to its internal RF memory map. The host cannot directly access RF registers; all RF/BBP access goes through the MCU command interface or the serialized CSR accessors (`MT_RF_CSR_CFG`, `MT_BBP_CSR_CFG`).

**However**, VCO cal (`mt7601u_vco_cal`, `phy.c:263-269`) uses direct RF CSR writes:
```c
static void mt7601u_vco_cal(struct mt7601u_dev *dev) {
    mt7601u_rf_wr(dev, 0, 4, 0x0a);      // direct RF CSR write
    mt7601u_rf_wr(dev, 0, 5, 0x20);      // direct RF CSR write
    mt7601u_rf_set(dev, 0, 4, BIT(7));   // direct RF CSR RMW
    msleep(2);
}
```

These go through `MT_RF_CSR_CFG` — a host-accessible register that the hardware uses to serialize RF chip access. This bypasses the MCU firmware entirely. **But the frequency plan (regs 17-20) MUST go through MCU RANDOM_WRITE** because those are in the MCU's internal RF memory map, not directly accessible through `MT_RF_CSR_CFG`.

### 1.4 The persistent-MCU-response-buffer pattern

**Answer: This is upstream's `mt7601u_mcu_wait_resp` (`mcu.c:68-107`) — a reusable response URB that stays armed between MCU commands.**

```c
// mcu.c:68 — the persistent response buffer
static int mt7601u_mcu_wait_resp(struct mt7601u_dev *dev, u8 seq) {
    struct urb *urb = dev->mcu.resp.urb;
    // ...
    while (i--) {
        if (!wait_for_completion_timeout(&dev->mcu.resp_cmpl, msecs_to_jiffies(300))) {
            dev_warn(dev->dev, "Warning: %s retrying\n", __func__);
            continue;  // up to 5 retries = 1.5s total
        }

        // Re-submit the response URB IMMEDIATELY after reading the completion
        ret = mt7601u_usb_submit_buf(dev, USB_DIR_IN, MT_EP_IN_CMD_RESP,
                                     &dev->mcu.resp, GFP_KERNEL, ...);

        if (FIELD_GET(MT_RXD_CMD_INFO_CMD_SEQ, rxfce) == seq &&
            FIELD_GET(MT_RXD_CMD_INFO_EVT_TYPE, rxfce) == CMD_DONE)
            return 0;
    }
    return -ETIMEDOUT;
}
```

The pattern: one response URB is submitted in `mcu_cmd_init` (`mcu.c:519-542`). After each MCU command, `mcu_wait_resp` waits for the completion, reads the response, and **immediately re-submits the URB** for the next command. This avoids URB allocation overhead and keeps the IN endpoint primed.

**Does adopting it change wedge behavior?** No — this is an optimization for response handling efficiency, not a fix for MCU firmware crashes. The wedge occurs because the MCU firmware dies during `CAL_BW` calibration and never sends a response. A persistent buffer vs per-command allocation doesn't change whether the firmware produces a response. It only changes timeout accounting (upstream retries 5× at 300ms = 1.5s; our driver's budget may differ).

**Our driver already uses this pattern.** Our `mcu_calibrate` in `mod.rs` submits a cmd, then polls for the MCU response on the pre-armed IN endpoint. The difference is in timeout budgets and retry semantics.

---

## 2. Q2 — The warm-firmware-resident resync path

### 2.1 What does upstream do when firmware is already resident?

**Answer: Upstream has a fast-path that ONLY caches the firmware blob and skips everything else.**

```c
// mcu.c:411-424 — load_firmware
static int mt7601u_load_firmware(struct mt7601u_dev *dev) {
    // Enable USB DMA
    mt7601u_wr(dev, MT_USB_DMA_CFG, RX_BULK_EN | TX_BULK_EN);

    if (firmware_running(dev))          // COM_REG0 == 1
        return firmware_request_cache(...);  // Just cache the blob for info, RETURN

    // ... full upload path only reached if firmware NOT running
}
```

`firmware_running()` (`mcu.c:24-27`) is simply `mt7601u_rr(dev, MT_MCU_COM_REG0) == 1`. If true, upstream calls `firmware_request_cache()` (which just ensures the firmware blob is in memory for logging/debug purposes) and **returns immediately**. No reset, no re-init, no DMA init, no MCU FCE re-init.

**Then `mt7601u_init_hardware` continues with:**
```c
// init.c:332-356 — post-FW steps (run regardless of warm/cold)
ret = mt7601u_wait_asic_ready(dev);    // ASIC ready post-FW
mt7601u_reset_csr_bbp(dev);            // CSR+BBP reset
mt7601u_init_usb_dma(dev);             // USB DMA config
ret = mt7601u_mcu_cmd_init(dev);       // Q_SELECT(1) + response URB
ret = mt7601u_dma_init(dev);           // ring buffers, RX pages
ret = mt7601u_write_mac_initvals(dev); // MAC tables via MCU RANDOM_WRITE
ret = mt7601u_init_bbp(dev);           // BBP tables via MCU RANDOM_WRITE
// ... eeprom, phy_init (includes init_cal), mac start
```

**Critical: These post-FW steps run for BOTH warm and cold paths.** The `reset_csr_bbp`, `init_usb_dma`, `mcu_cmd_init`, `dma_init`, MAC/BBP initvals all execute regardless of whether firmware was re-uploaded. This is correct for Linux because the device state is preserved across a driver reload (USB interface unbind/rebind doesn't power-cycle the device).

### 2.2 What did our resident-fw branch omit that caused the warm calibration timeout?

**Answer: Our `load_firmware` correctly skips the upload when `COM_REG0==1`, and `bring_up()` continues with the full post-FW init sequence.** The warm path runs:
1. `reset_csr_bbp()` — CSR+BBP reset
2. `init_usb_dma()` — USB DMA config
3. `mcu_cmd_init()` → `Q_SELECT(1)` MCU function select + response buffer setup
4. `dma_init()` — ring buffers, RX page alloc
5. MAC initvals via MCU RANDOM_WRITE
6. BBP initvals via MCU RANDOM_WRITE
7. eeprom_init()
8. `phy_init()` → `init_cal()` — full calibration sequence

**The warm calibration timeout happened at step 8 (`init_cal`).** The silicon log showed: "MCU calibration timed out" during the warm-resident path. This means one of the MCU commands in `init_cal` (R, TXDCOC, BW filter, LOFT, TXIQ, RXIQ, DPD) timed out.

**Root cause hypothesis: After a host reboot, the firmware is resident but its internal DMA/command processing state may be stale.** The USB endpoints were reset by the xHCI controller during host boot, but the MCU firmware's internal command queue and response buffer may not have been flushed. When our driver re-submits the response URB in `mcu_cmd_init`, the firmware might still be holding a stale sequence number or have an orphaned pending command from before the reboot.

**Upstream avoids this because Linux doesn't reboot between probe calls.** In Linux, `mt7601u` is loaded as a module and probed on USB bind. A warm path only happens if the module is unloaded/reloaded while the device stays powered — and even then, the USB core does a full `usb_reset_device()` before re-probing. Our driver runs in a bare-metal environment where the host reboot resets xHCI but NOT the USB device's internal state.

**The missing step: `usb_reset_device()` equivalent.** Linux's USB core calls `usb_reset_device()` in `usb.c:265` during probe, which sends a USB SET_CONFIGURATION + resets all endpoints. This flushes the device's USB state machine and any pending transfers. Our driver needs an equivalent endpoint reset before trusting resident firmware after a host reboot.

---

## 3. Design options graded

### Option A: Bring firmware up already on the target channel
**Verdict: DEAD — firmware is a binary blob with no configurable boot channel.**

Upstream's init sequence (`init.c:317-420`) does not call `set_channel` during bring-up. The `rf_channel` initvals table sets general RF parameters, not the channel frequency. The only way to set the operating channel is through `__mt7601u_phy_set_channel`, which writes RF bank-0 regs 17-20 via MCU RANDOM_WRITE. These registers are NOT part of any initvals table loaded during firmware boot. The firmware blob's ILM/DLM may initialize these registers to some default, but there's no mechanism to control what that default is without modifying the firmware (which we can't — it's a closed binary).

### Option B: Skip/defer post-channel MCU calibration on the initial targeted tune
**Verdict: VIABLE-NOW — strongest candidate for the fix.**

During scan, upstream already skips deferred calibration (`phy.c:456-457`). The wedge occurs at `set_bw_filter` → `MCU_CAL_BW` inside `__mt7601u_phy_set_channel`. For a targeted single-channel association (not a scan), we can:

1. **First channel set:** Skip `set_bw_filter` (the MCU CAL_BW commands). The RF frequency plan, LNA gain, VCO cal, and TX power will be set — enough for RX scan + TX management frames. Calibration is a refinement, not a requirement for basic operation.
2. **Post-association recalibration:** Run the deferred calibration AFTER association succeeds (upstream does this via `cal_work` at `MT_CALIBRATE_INTERVAL`).

**Why this works:** The LKML bug report identifies the MCU firmware crashing during CAL_BW specifically. Skipping it on the first channel set avoids the wedge trigger entirely. Subsequent calibrations (post-association, when signal is stable) are less likely to crash because the RF state is settled.

**Smallest de-risking step:** Modify `set_channel` to accept a `skip_cal: bool` parameter. Pass `true` for the first targeted tune, `false` for post-association recalibration. If association fails without calibration, retry with calibration enabled as a fallback.

### Option C: Channel-set via direct RF/BBP register writes instead of MCU command
**Verdict: DEAD — RF frequency plan registers are NOT directly host-accessible.**

The channel frequency (RF bank-0 regs 17-20) is in the MCU's internal RF memory map (`MT_MCU_MEMMAP_RF`). The only way to write these registers is through `mt7601u_write_reg_pairs` → MCU RANDOM_WRITE command. The host-side `MT_RF_CSR_CFG` accessor can only reach a small subset of RF registers (the ones the hardware exposes through that CSR — typically bank 0, regs 0-12 for VCO/cal). Regs 17-20 are not accessible this way.

### Option D: Bound channel-switch wait + real resident-fw reset/resync
**Verdict: VIABLE-AFTER-X — requires two fixes to be sufficient.**

This is Jarvis's (a)+(b) approach. Two sub-fixes needed:

**(a) Bounded wait for set_channel:** Already partially implemented via `SCAN_BULK_BUDGET`. Extend to the targeted path so a wedged channel switch fast-fails instead of burning the full bulk-poll timeout. **This is already in place** for scan (`mod.rs:1700-1708`), needs extension to targeted mode.

**(b) Resident-fw reset/resync:** Before trusting resident firmware, run an endpoint-level reset equivalent to Linux's `usb_reset_device()`. Specifically:
- Clear host-side xHCI endpoint state (already done in `recover_chip`)
- Clear device-side endpoint halt via `ClearFeature(ENDPOINT_HALT)` (already done in `recover_chip`)
- **NEW:** Re-run `fce_preload_setup` + `vendor_reset` + `msleep(5)` before `mcu_cmd_init`, even when firmware is resident. This flushes the MCU's internal command queue and resets FCE DMA state, bringing it back to a known-good post-boot state without re-uploading firmware.

**Smallest de-risking step:** In `load_firmware`, when `firmware_running()` is true, still run `fce_preload_setup` (vendor reset + register setup) before returning. This is the pre-FW-register-setup from `mcu.c:459-491` that prepares FCE DMA state. Our driver may skip this on warm path because it assumes the firmware's DMA state is intact — but after a host reboot, the USB transfer state machine on the device side may be stale.

### Option E: Operational sidestep — put AP on chip's power-on-default channel
**Verdict: VIABLE-NOW as test-unblocker only.**

If we can determine what channel the cold MCU boots on (by reading RF bank-0 regs 17-20 immediately after firmware boot, before any `set_channel`), we could set the AP to that channel and avoid the switch entirely. This is a **test-unblocker**, not a durable fix — it proves association works on silicon while the real fix lands.

**Which channel?** The `rf_channel` initvals written during `phy_init()` don't set the frequency plan. The firmware's ILM may initialize RF regs 17-20 to some default. Our `log_channel_triple` function (`phy.rs:130-161`) already reads these regs and reverse-matches them against `FREQ_PLAN`. **Actionable:** On the next silicon run, log `tx` channel immediately after `bring_up()` but before any `set_channel`. That tells us the default channel.

---

## 4. Recommended fix shape for Jarvis (step 2)

**Primary: Option B (skip CAL_BW on first targeted tune) + Option D(b) (warm-fw resync).**

### Fix 1: Skip `set_bw_filter` on first targeted channel set
In `phy.rs`, modify `set_channel` to accept `calibrate: bool`:
```rust
pub fn set_channel(dev: &mut Mt7601uDev, ch: u8, calibrate: bool) -> Result<(), &'static str> {
    // ... freq plan, ALC, BBP LNA, VCO cal, bbp_set_bw (unchanged) ...
    if calibrate {
        set_bw_filter(dev, false)?;  // MCU CAL_BW — skip on first tune
    }
    // ... TX power (unchanged) ...
}
```
Call with `calibrate=false` for the initial targeted tune. After association succeeds, run a recalibration pass with `calibrate=true`.

### Fix 2: Warm-firmware resync in `load_firmware`
When `firmware_running()` is true, still run the FCE preload setup + vendor reset before returning:
```rust
fn load_firmware(&mut self) -> Result<(), &'static str> {
    // ... USB DMA enable ...
    if self.firmware_running() {
        // WARM-RESYNC: flush stale MCU command state after host reboot.
        // The firmware is alive but its internal DMA/command queue may hold
        // orphaned state from before the xHCI reset. Re-run the pre-upload
        // register setup (Linux mcu.c:459-491) to bring FCE/DMA back to a
        // known-good state without re-uploading the firmware image.
        self.fce_preload_setup()?;
        return Ok(());
    }
    // ... cold path unchanged ...
}
```

### Fix 3: Post-association recalibration
After association succeeds, run `init_cal` (or a subset covering CAL_BW) to restore full calibration. This is upstream's `cal_work` pattern — deferred until the RF state is stable on the operating channel.

---

## 5. Extended trap bank (T-021 through T-027)

| # | Trap | Status | Raw-source citation |
|---|------|--------|---------------------|
| T-021 | **Channel-switch MCU wedge (CAL_BW specific)** — `set_bw_filter` → `MCU_CAL_BW` is the exact MCU command that crashes the firmware during channel hops. The wedge is NOT the RF frequency plan write, NOT the VCO cal, and NOT the deferred calibration work. It's the inline CAL_BW inside `__mt7601u_phy_set_channel`. | **CONFIRMED** | `phy.c:424` (`mt7601u_set_bw_filter(dev, false)`), `phy.c:271-287` (CAL_BW TX then RX, each `wait_resp=true`). LKML "mt7601u dies during channel switch." CB-2 silicon: ch6 set_channel irrecoverably wedges. |
| T-022 | **Scan-mode calibration skip** — upstream avoids the deferred calibration work during scan (`MT7601U_STATE_SCANNING` → early return, `phy.c:456-457`). The inline CAL_BW still runs per channel hop during scan. This means the wedge can hit during scan too (it does — W1's ch7+ wedge). Upstream survives by continuing the scan on remaining channels; it doesn't recover the MCU. | **CONFIRMED** | `phy.c:456-457` (`if test_bit(SCANNING) return 0`). The inline CAL_BW at `phy.c:424` runs regardless of scan/non-scan mode. |
| T-023 | **No boot-time channel programming** — upstream's `init_hardware` never calls `set_channel`. The chip boots on whatever RF state the firmware's ILM initializes. First channel program is always a mid-stream switch. No initvals table sets RF freq-plan regs 17-20. | **CONFIRMED** | `init.c:317-420` (no set_channel call). `initvals_phy.h` `rf_channel[]` is general RF setup, not frequency plan. |
| T-024 | **Warm-fw fast-path skips everything except blob cache** — upstream's `load_firmware` returns immediately when `COM_REG0==1`, running zero reset/re-init/DMA-sync. Post-FW init (CSR reset, DMA init, MCU cmd init, MAC/BBP tables) runs regardless of warm/cold. | **CONFIRMED** | `mcu.c:422-424` (`if firmware_running(dev) return firmware_request_cache(...)`). Linux USB core's `usb_reset_device()` in `usb.c:265` handles device-side state flush during probe — our bare-metal driver has no equivalent. |
| T-025 | **Warm-fw stale command queue after host reboot** — after host reboot, xHCI resets but the MT7601U's internal USB endpoint state and MCU command queue persist. The firmware may hold orphaned sequence numbers or pending DMA descriptors. Our driver re-submits the response URB without flushing this state, causing calibration timeouts. | **LOCALIZED (silicon)** | OT-39 warm path: "MCU calibration timed out" → `mt7601u_bind_incomplete`. Upstream avoids because Linux doesn't reboot between probe calls; `usb_reset_device()` in `usb.c:265` flushes device USB state on module reload. |
| T-026 | **FCE preload setup skipped on warm path** — the pre-upload register setup (`mcu.c:459-491`: writes to 0x94c, FCE_PSE_CTRL, 0xa44, 0x230, 0x400, 0x800, PBF_CFG, FCE PDMA/SKIP_FS) only runs during cold firmware upload. On warm path, these registers retain whatever state they had before the host reboot. If any are stale (e.g., FCE DMA pointers), MCU commands may fail silently. | **HYPOTHESIZED** | `mcu.c:459-491` (only reached when `!firmware_running`). Our `fce_preload_setup` in `mod.rs` similarly only runs during cold upload. |
| T-027 | **RF freq-plan regs inaccessible via host CSR** — RF bank-0 regs 17-20 (channel frequency) are in the MCU's internal RF memory map, NOT accessible through `MT_RF_CSR_CFG`. Channel switch MUST go through MCU RANDOM_WRITE + CAL_BW. Direct-register bypass (Option C) is impossible for the frequency plan. | **CONFIRMED** | `phy.c:409` (`mt7601u_write_reg_pairs(dev, MT_MCU_MEMMAP_RF, ...)`). `MT_RF_CSR_CFG` only exposes a small subset of RF registers through hardware serialization; regs 17-20 are not among them. |

---

## 6. Pre-seeded failure-mode/discriminator bank (for bare-metal retry)

| Expected outcome | Serial signature | Capture channel | Verdict |
|-----------------|------------------|-----------------|---------|
| **Fix 1 success** (CAL_BW skipped, first tune OK) | `set_channel(ch=N, cal=false) OK` → scan/assoc proceeds | Serial + NVMe sink | Fix 1 works; proceed to post-assoc cal |
| **Fix 1 partial** (CAL_BW skip helps but assoc fails — TX power wrong without cal) | `set_channel OK` → `auth_tx OK` → `directed_to_us=0` or `no_assoc_resp` | Serial + TX_STAT_FIFO dump | Need post-assoc cal before TX, or run cal on a safe channel first |
| **Fix 1 insufficient** (wedge at RANDOM_WRITE, not CAL_BW) | `set_channel(ch=N, cal=false)` → `mcu response timed out` at RF freq-plan write | Serial breadcrumb at each MCU cmd | Wedge is broader than CAL_BW; need Option D(b) resync |
| **Fix 2 success** (warm resync works) | `firmware already resident — FCE resync applied` → `init_cal OK` → bring_up completes | Serial + NVMe sink | Warm path fixed |
| **Fix 2 partial** (FCE resync helps but init_cal still times out) | `FCE resync applied` → `mcu_calibrate(CAL_R) timed out` | Serial at each cal step | MCU firmware truly stale; need full re-upload on warm path |
| **Option E discovery** (default channel known) | `CHANNEL-TRIPLE beacon_ds=0 tuned=0 tx=M` immediately post-boot | `phy.rs:130-161` `log_channel_triple` | AP can be set to ch=M for test-unblock |

---

## 7. Honest negatives

1. **Default channel unknown:** We don't know what channel the cold MCU boots on without a silicon read of RF regs 17-20 post-boot. Option E requires one more silicon run to discover this.
2. **CAL_BW specificity unproven on our hardware:** The LKML report identifies CAL_BW as the wedge trigger, but we haven't isolated which exact MCU command in `set_channel` fails on our dongle. The discriminator bank (§6) will localize this on the next silicon run.
3. **Firmware blob is closed binary:** Option A (boot on target channel) would require firmware modification. Not feasible with `mt7601u.bin` v34.

---

## 8. Source provenance

| Source | Obtained | Commit/Version |
|--------|----------|----------------|
| Linux kernel mt7601u driver | `git clone --depth 1` (sparse) | mainline v6.8+ (`phy.c`, `mcu.c`, `init.c`) |
| Our driver | `git show origin/main:` | Current `main` branch |

All register values, struct layouts, and sequences above are from direct Read of the cloned source files. No WebFetch was used for any byte/register data.
