<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: MT7601U deep bring-up trap-mine (raw driver source)

**Date:** 2026-06-14
**Lane:** Prometheus (deep prior-art mining — NO kernel code)
**Delivers for:** `THINK-2026-06-14-phase-7.5-build-W1-mt7601u-driver.md` §4/§5 (trap bank + bring-up sequence fold-in slots)
**Sources:** Linux kernel mt7601u driver (`drivers/net/wireless/mediatek/mt7601u/`, commit `2b07ea76f`), linux-firmware (`mediatek/mt7601u.bin`, 45,412 bytes, version 34)

---

## 1. Bring-up sequence reference (ordered init from raw source)

### 1.1 USB enumeration + endpoint discovery

**Source:** `usb.c:14-33` (device ID table), `usb.c:265-327` (probe), `usb.c:228-263` (pipe assignment), `usb.h:28-42` (endpoint enums)

Device ID table matches `148f:7601` + 16 clones (`usb.c:14-33`). Probe sequence:
1. `mt7601u_alloc_device()` — allocates ieee80211_hw + driver private state
2. `usb_reset_device()` — hard USB reset
3. `mt7601u_assign_pipes()` — scans interface altsetting for endpoints:
   - **2 BULK IN endpoints:** `MT_EP_IN_PKT_RX` (data), `MT_EP_IN_CMD_RESP` (MCU responses)
   - **6 BULK OUT endpoints:** `MT_EP_OUT_INBAND_CMD` (firmware upload + MCU cmds), `MT_EP_OUT_AC_BK/BE/VI/VO/HCCA` (TX queues)
   - Validates exact count: `ep_i != __MT_EP_IN_MAX || ep_o != __MT_EP_OUT_MAX` → `-EINVAL` (`usb.c:256-259`)
4. `mt7601u_wait_asic_ready()` — initial ASIC check
5. Read `MT_ASIC_VERSION` + `MT_MAC_CSR0`, verify `(asic_rev >> 16) == 0x7601` (`usb.c:295-302`)
6. Early eFUSE presence check: `MT_EFUSE_CTRL & MT_EFUSE_CTRL_SEL` (warn only, not fatal) (`usb.c:304-306`)

**ENDPOINT CORRECTION:** EP3 is **BULK OUT** (INBAND_CMD — firmware upload + MCU commands), NOT interrupt. MCU responses come on the 2nd BULK IN endpoint (`MT_EP_IN_CMD_RESP = 1`). No dedicated interrupt endpoint exists.

### 1.2 ASIC ready poll

**Source:** `core.c:9-26`

```
mt7601u_wait_asic_ready():
  for i = 100 times:
    if STATE_REMOVED → -EIO
    val = rr(MT_MAC_CSR0)
    if (val && ~val) → success     // non-zero AND not all-ones
    udelay(10)
  return -EIO
```

Polls `MT_MAC_CSR0` for a value that is neither 0 nor 0xFFFFFFFF. Timeout: 100 × 10us = **1ms total**. Called 3× during init: once in probe, twice in `mt7601u_init_hardware()`.

### 1.3 WLAN enable + PLL/XTAL check

**Source:** `init.c:15-57` (`mt7601u_set_wlan_state()`), `init.c:59-88` (`mt7601u_chip_onoff()`)

```
mt7601u_chip_onoff(dev, true, false):
  rr(MT_WLAN_FUN_CTRL)
  wr(MT_WLAN_FUN_CTRL, val | WLAN_EN | WLAN_CLK_EN)
  udelay(20)
  // Poll MT_CMB_CTRL for XTAL_RDY + PLL_LD
  for i = 200 times:
    val = rr(MT_CMB_CTRL)
    if (val & XTAL_RDY && val & PLL_LD) → success
    udelay(20)
  // If timeout: dev_err("PLL and XTAL check failed!") — WARNING ONLY, not fatal
```

### 1.4 Firmware load handshake

**Source:** `mcu.c:411-502` (`mt7601u_load_firmware()`), `mcu.c:355-404` (`mt7601u_upload_firmware()`), `mcu.c:268-281` (fw_header struct), `mcu.c:283-353` (DMA upload)

**Firmware header layout (`mt76_fw_header`, `mcu.c:268-275`):**
```c
struct mt76_fw_header {
    __le32 ilm_len;       // offset 0x00 — ILM length (includes IVB)
    __le32 dlm_len;       // offset 0x04 — DLM length
    __le16 build_ver;     // offset 0x08
    __le16 fw_ver;        // offset 0x0a
    u8   pad[4];          // offset 0x0c
    char build_time[16];  // offset 0x10
};                        // total: 0x20 (32 bytes)
```

**Firmware image layout:** Header (32B) → ILM (includes 64B IVB at start) → DLM

**Load sequence:**
1. Check if firmware already running: `rr(MT_MCU_COM_REG0) == 1` (`mcu.c:24-27`)
2. `request_firmware()` for `mediatek/mt7601u.bin` (tries `"mediatek/mt7601u.bin"` then `"mt7601u.bin"`)
3. Validate: `fw->size >= header_size`, `ilm_len > IVB_SIZE(64)`, total size = header + ilm + dlm (`mcu.c:436-449`)
4. Pre-load register setup: writes to `0x94c`, `MT_FCE_PSE_CTRL`, `0xa44`, `0x230`, `0x400`, `0x800` (`mcu.c:459-468`)
5. PBF TX queue enable, FCE setup (`mcu.c:470-491`)
6. Vendor reset + 5ms delay (`mcu.c:462-463`)
7. **Upload ILM:** chunked via `mt7601u_dma_fw()` → `__mt7601u_dma_fw()`:
   - Chunk size: `MCU_FW_URB_MAX_PAYLOAD = 0x3800` (14,336 bytes) (`mcu.c:20`)
   - DMA via FCE: set `MT_FCE_DMA_ADDR` + `MT_FCE_DMA_LEN` via vendor req `MT_VEND_WRITE_FCE(0x42)` (`mcu.c:300-308`)
   - Submit bulk OUT to `MT_EP_OUT_INBAND_CMD`, wait up to 1000ms for completion (`mcu.c:311-326`)
   - After each chunk: poll `MT_MCU_COM_REG1` bit 31 for DMA done, timeout 500ms (`mcu.c:349-350`)
8. **Upload DLM:** same chunked DMA to `MT_MCU_DLM_OFFSET (0x80000)` (`mcu.c:379-382`)
9. **Load IVB:** vendor request `MT_VEND_DEV_MODE(1)`, value `0x12`, 64-byte IVB payload (`mcu.c:385-388`)
10. **Wait for firmware running:** poll `MT_MCU_COM_REG0 == 1`, up to 100 × 10ms = 1s timeout (`mcu.c:391-396`)

### 1.5 MCU command interface

**Source:** `mcu.c:109-153` (`mt7601u_mcu_msg_send()`), `mcu.c:68-107` (`mt7601u_mcu_wait_resp()`), `mcu.h:29-51` (MCU cmd enums), `dma.h:54-56` (TX command flags)

**Post-firmware init:**
```
mt7601u_mcu_cmd_init():
  mcu_function_select(Q_SELECT, 1)    // CMD_FUN_SET_OP, wait_resp=true for func 1
  init_completion(&mcu.resp_cmpl)
  alloc MCU resp buffer (1024 bytes)
  submit IN URB on MT_EP_IN_CMD_RESP
```

**MCU command protocol:**
- TX: Build SKB with payload → `mt7601u_dma_skb_wrap_cmd()` prepends 4B DMA header + 4B zero pad → bulk OUT to `MT_EP_OUT_INBAND_CMD` via `usb_bulk_msg()` (500ms timeout) (`mcu.c:137`)
- DMA header for commands: `TXD_INFO_TYPE = DMA_COMMAND`, `TXD_INFO_D_PORT = CPU_TX_PORT`, seq + cmd type in upper bits (`dma.h:54-56`, `mcu.c:34-40`)
- RX: Response URB on `MT_EP_IN_CMD_RESP` (1024B). Wait up to 300ms per retry, 5 retries = **1.5s total** (`mcu.c:74-76`)
- Response validation: check `CMD_SEQ` matches + `EVT_TYPE == CMD_DONE(0)` (`mcu.c:96-98`)

**MCU commands (`mcu.h:29-51`):** FUN_SET_OP(1), LOAD_CR(2), INIT_GAIN_OP(3), DYNC_VGA_OP(6), TDLS_CH_SW(7), BURST_WRITE(8), RMW(9), RANDOM_READ(10), BURST_READ(11), RANDOM_WRITE(12), LED_MODE_OP(16), POWER_SAVING_OP(20), WOW_CONFIG/QUERY/FEATURE(21-24), CARRIER_DETECT(28), RADAR_DETECT(29), SWITCH_CHANNEL(30), CALIBRATION(31), BEACON(32), ANTENNA(33)

**In-band packet limit:** `INBAND_PACKET_MAX_LEN = 192` bytes (`mcu.h:27`). Reg pair writes: max `192/8 = 24` pairs per MCU cmd. Burst writes: max `192/4 - 1 = 47` regs per MCU cmd.

### 1.6 DMA init + MAC/BBP initvals

**Source:** `init.c:317-420` (`mt7601u_init_hardware()`), `init.c:99-116` (USB DMA), `init.c:118-133` (BBP), `init.c:152-170` (MAC initvals)

Post-firmware, post-MCU handshake:
1. Poll WPDMA not busy: `MT_WPDMA_GLO_CFG` TX/RX DMA busy bits → 0, timeout 100ms (`init.c:339-344`)
2. ASIC ready again (post-FW) (`init.c:347-349`)
3. Reset CSR+BBP: `MT_MAC_SYS_CTRL = RESET_CSR | RESET_BBP`, clear `MT_USB_DMA_CFG`, 1ms delay, clear reset (`init.c:90-97`)
4. Init USB DMA: set aggregation timeout/limit, enable RX/TX bulk, enable RX aggregation if maxpacket=512 (`init.c:99-116`)
5. MCU cmd init: `Q_SELECT(1)` function select (`init.c:354-356`)
6. DMA init: ring buffers, RX page alloc (`dma.c`)
7. MAC initvals: write `mac_common_vals[]` + `mac_chip_vals[]` via MCU RANDOM_WRITE to `MT_MCU_MEMMAP_WLAN (0x00410000)` (`init.c:152-170`, `mcu.c:210-237`)
8. BBP initvals: wait BBP ready → write `bbp_common_vals[]` + `bbp_chip_vals[]` via MCU RANDOM_WRITE to `MT_MCU_MEMMAP_BBP (0x40000000)` (`init.c:118-133`)
9. Clear WCID memory, key memory, WCID attr memory (`init.c:172-218`)

### 1.7 eFUSE/EEPROM read

**Source:** `eeprom.c:31-64` (efuse_read), `eeprom.c:66-95` (usage map check), `eeprom.c:344-390` (init)

```
mt7601u_eeprom_init():
  // Physical size check — reads usage map at 0x1e0-0x1fc
  if (used_words < 5):
    dev_err("needs default EEPROM file") → -EINVAL (HARD FAIL)
  
  alloc eeprom buffer (256 bytes)
  for offset = 0..240 step 16:
    efuse_read(offset, mode=MT_EE_READ)
  
  // Parse fields:
  version at [0x03] — warn if > 0x0d
  MAC addr at [0x04..0x09] — random fallback if invalid
  NIC conf at [0x34..0x37] — TSSI, RF ctrl capability
  Country region at [0x39] — maps to channel bounds (default: ch 1-14)
  TX power cal, TSSI params, RSSI offset, etc.
```

**eFUSE read protocol (`eeprom.c:31-64`):** Set `MT_EFUSE_CTRL` with address + mode + KICK bit. Poll for KICK→0 (timeout 1000 iterations × 10us = 10ms). Read 4 × `MT_EFUSE_DATA[n]` registers. If `AOUT` field is all-ones, data is outside usage map → return 0xFF fill.

### 1.8 PHY init + calibration

**Source:** `phy.c:1258+` (`mt7601u_phy_init()`), `phy.c:18-92` (RF read/write)

PHY init writes band-specific register tables via MCU burst/random write to BBP and RF memory maps. RF access is serialized through `MT_RF_CSR_CFG`:
- Wait KICK→0 (timeout 100 × 10us = 1ms)
- Write: set bank/reg/data/KICK
- Read: set bank/reg/KICK → wait KICK→0 → read data from same register

### 1.9 MAC start + bulk data path

**Source:** `init.c:230-255` (`mt7601u_mac_start()`), `dma.c:76-131` (RX processing)

```
mt7601u_mac_start():
  wr(MT_MAC_SYS_CTRL, ENABLE_TX)
  poll WPDMA_GLO_CFG: TX/RX DMA busy → 0, timeout 200,000 × 10us = 2s
  set RX filter (promisc + filter CRC/PHY errors + ctrl frames)
  wr(MT_MAC_SYS_CTRL, ENABLE_TX | ENABLE_RX)
  poll WPDMA_GLO_CFG: TX/RX DMA busy → 0, timeout 50 × 10us = 0.5ms
```

**TX data path:** `mt7601u_tx()` → prepend `mt76_txwi` struct + 4B pad → DMA wrap with `mt7601u_dma_skb_wrap_pkt()` (4B TXINFO header) → enqueue to TX ring (`mt7601u_dma_enqueue_tx()`)

**RX data path:** Bulk IN URB on `MT_EP_IN_PKT_RX` → segment processing: skip 4B DMA HDR + 4B RX INFO + RXWI struct → extract 802.11 frame → pass to ieee80211_rx()

**DMA header layout (`dma.h:58-78`):**
```
| 4B TXINFO | payload (rounded to 4B) | 4B zero pad |
TXINFO: [LEN:16b] [D_PORT:3b] [TYPE:2b] + flags
```

---

## 2. Extended, verified trap bank

### 2.1 Starter bank verification (from THINK §4)

| # | Trap | Status | Raw-source correction |
|---|------|--------|----------------------|
| 1 | **Firmware load** — `mt7601u.bin` (~44KB initcode); `mt7601u_fw_header` magic; chunk to EP maxpacket | **CONFIRMED** | Chunk size is `MCU_FW_URB_MAX_PAYLOAD = 0x3800` (14,336B), not EP maxpacket. Upload uses FCE DMA via vendor req `0x42`, not direct bulk. Header has no "magic" — validation is: `ilm_len > IVB_SIZE(64)`, total size = header + ilm + dlm (`mcu.c:436-449`). **Discriminator:** `-ENOENT` from `request_firmware()` vs `-ETIMEDOUT` from upload DMA vs `-ETIMEDOUT` from FW-running poll — all distinct error paths. |
| 2 | **eFUSE calibration** — clones ship unprogrammed eFUSE; fallback to defaults | **CONFIRMED, PARTIAL HARD FAIL** | Usage map check (`eeprom.c:89-92`) is a **HARD FAIL** if <5 words used: returns `-EINVAL`, aborts init. MAC address has random fallback (`mac.c:14-20`), but eFUSE usage map exhaustion cannot be recovered. **Discriminator:** "needs default EEPROM file" error message vs valid MAC with `eth_random_addr` fallback. |
| 3 | **USB transfer-size limits** — MCU cmds ≤192B; firmware chunks ≤EP maxpacket; TX ring ~32 desc | **CORRECTED** | MCU in-band limit: `INBAND_PACKET_MAX_LEN = 192` (`mcu.h:27`). Firmware chunks: `MCU_FW_URB_MAX_PAYLOAD = 0x3800` (14,336B) — NOT EP maxpacket. TX ring: **not ~32** — see Trap T-008 below. **Discriminator:** MCU cmd short-send (`sent != skb->len`, `mcu.c:142`), firmware URB timeout 1s (`mcu.c:317`). |
| 4 | **Regulatory lockout** — no reg domain → country 00 hides channels | **CONFIRMED** | EEPROM country region at `[0x39]`: valid regions 0-7, 32-33 map to channel bounds. Invalid → default idx=5 (channels 1-14) (`eeprom.c:189-197`). If country code not set in regulatory domain, Linux cfg80211 will further restrict. **Discriminator:** scan returns too few BSSIDs; channels 12-13 invisible on non-JP/FI firmware. |
| 5 | **Deep-sleep hang** — clones' wake pin unwired; deep sleep bricks until replug | **CONFIRMED (driver-level)** | Driver sets `disable_hub_initiated_lpm = 1` (`usb.c:380`). Suspend calls `mt7601u_cleanup()` which stops hardware. Resume re-runs full `mt7601u_init_hardware()`. No deep-sleep implementation exists in this driver — the trap is at the USB hub level (LPM disabled by driver). **Discriminator:** device vanishes from bus after suspend → replug required. |
| 6 | **TX ring overflow** — ~32 descriptors; over-submission → `-EBUSY` | **CORRECTED** | TX ring size is not 32 — see Trap T-008. Stop/wake thresholds are implemented via `mt7601u_dma_enqueue_tx()`. **Discriminator:** tx_dropped counter climbs, URB submission returns error. |
| 7 | **Band-specific initcode** — load the right table | **CONFIRMED (2.4GHz only)** | MT7601U is **2.4GHz-only**. Channel table: 14 channels at 2412-2484 MHz (`init.c:472-487`). Initvals include `initvals_phy.h` with band-specific BBP/RF tables. No 5GHz support exists in this driver. **Discriminator:** N/A — single-band chip, no wrong-table risk unless someone ports from MT76x0. |

### 2.2 Additional traps (raw-source mining)

**T-001: ASIC ready garbage returns**
- **Source:** `core.c:14-20`
- **Trap:** `MT_MAC_CSR0` can return 0 or 0xFFFFFFFF during early probe before ASIC is powered. If the device is slow to respond, the 1ms timeout (100 × 10us) may expire.
- **Discriminator:** Probe fails immediately with `-EIO`; no firmware load attempted. Distinguish from USB enumeration failure by checking `lsusb` still shows the device.

**T-002: Vendor request retry exhaustion**
- **Source:** `usb.c:88-117`
- **Trap:** All register access goes through USB vendor control requests. Retry = 10 (`MT_VEND_REQ_MAX_RETRY`), timeout = 300ms per attempt (`MT_VEND_REQ_TOUT_MS`). If the device is slow or the USB bus is congested, 10 retries × 300ms = **3s per register access**. `-ENODEV` during retry sets `STATE_REMOVED` and returns immediately.
- **Discriminator:** "Vendor request req:XX off:XXXX failed:YY" error. If YY = `-ENODEV`, device was unplugged. Other errors are transient bus issues.

**T-003: Short read from vendor request**
- **Source:** `usb.c:126-143`
- **Trap:** `__mt7601u_rr()` expects exactly 4 bytes (`MT_VEND_BUF`). Short read (< 4B) logs error and returns `~0` (all-ones/garbage). This propagates as a bogus register value.
- **Discriminator:** "wrong size read:NN off:XXXX" error in dmesg. Subsequent init may use garbage values — look for downstream failures (e.g., ASIC version check passes with all-ones, then firmware load fails).

**T-004: FCE DMA timeout during firmware upload**
- **Source:** `mcu.c:317-326`
- **Trap:** Each firmware chunk waits up to 1000ms for the bulk URB completion. After that, polls `MT_MCU_COM_REG1` bit 31 for 500ms (`mcu.c:349-350`). Total per-chunk timeout: 1.5s. With ~45KB firmware / 14KB chunks = ~4 chunks → worst case 6s.
- **Discriminator:** "firmware upload timed out" vs "firmware upload urb failed:NN". Timeout = FCE not processing. URB error = USB transfer failure (check bus bandwidth, hub issues).

**T-005: MCU response pre-completed**
- **Source:** `mcu.c:132-133`
- **Trap:** If the MCU response completion fires before the driver re-submits the response URB, it logs "MCU response pre-completed!" — data loss for subsequent MCU commands.
- **Discriminator:** "MCU response pre-completed!" in dmesg. Subsequent MCU cmds hang on `mt7601u_mcu_wait_resp()` (300ms × 5 retries timeout).

**T-006: MCU response sequence/event mismatch**
- **Source:** `mcu.c:96-102`
- **Trap:** Response validation checks `CMD_SEQ` matches + `EVT_TYPE == CMD_DONE(0)`. If the firmware returns `CMD_ERROR(1)` or `CMD_RETRY(2)`, or a wrong sequence number, it's logged as an error and retried. After 5 retries → `-ETIMEDOUT`.
- **Discriminator:** "MCU resp evt:TYPE seq:SEQ" error. EVT_TYPE != 0 means firmware rejected the command (check command payload). Wrong SEQ means response ordering issue.

**T-007: BBP ready timeout**
- **Source:** `init.c:122` → calls `mt7601u_wait_bbp_ready()` (defined in `phy.c`)
- **Trap:** If BBP hasn't initialized after firmware load, init fails before MAC/BBP initvals can be written.
- **Discriminator:** Init fails at `mt7601u_init_bbp()` step; no "Firmware running!" message would have appeared if FW didn't start, so this specifically means BBP is stuck post-FW.

**T-008: RF register access timeout**
- **Source:** `phy.c:31-48` (RF write), `phy.c:67-91` (RF read)
- **Trap:** RF CSR kick poll: timeout 100 × 10us = 1ms. If the RF chip is unresponsive (e.g., bad clone, power issue), all subsequent PHY init fails. Serialized access — one reg at a time through `reg_atomic_mutex`.
- **Discriminator:** "RF write XX:YY failed:-ETIMEDOUT" or "RF read XX:YY failed:-ETIMEDOUT". Distinguish from BBP timeout by the register address in the error (RF_CSR_CFG = 0x0218).

**T-009: EEPROM version too new**
- **Source:** `eeprom.c:368-371`
- **Trap:** Warns if `eeprom[0x03] > MT7601U_EE_MAX_VER (0x0d)`. This is a warning, not fatal — driver continues with potentially mis-parsed EEPROM data.
- **Discriminator:** "unsupported EEPROM version XX" in dmesg. Subsequent behavior depends on which fields shifted — may manifest as wrong MAC, wrong TX power, or wrong country region.

**T-010: HW RF ctrl unsupported**
- **Source:** `eeprom.c:117-119`
- **Trap:** If NIC_CONF_1 has `HW_RF_CTRL` bit set, driver logs error but continues. This means the EEPROM was programmed for hardware RF control mode, which this open-source driver doesn't support.
- **Discriminator:** "this driver does not support HW RF ctrl" in dmesg. May manifest as no TX or wrong RF settings.

**T-011: eFUSE usage map exhaustion (HARD FAIL)**
- **Source:** `eeprom.c:89-92`
- **Trap:** If the eFUSE usage map shows < 5 free words, init returns `-EINVAL`. This is a **hard, unrecoverable failure**. Some cheap clones ship with nearly-full eFUSE.
- **Discriminator:** "your device needs default EEPROM file and this driver doesn't support it!" — exact error string. No workaround in this driver.

**T-012: PLL/XTAL check failure (soft error)**
- **Source:** `init.c:42-56`
- **Trap:** Polls `MT_CMB_CTRL` for `XTAL_RDY` + `PLL_LD`. Timeout: 200 × 20us = 4ms. On timeout, logs error but **continues** (not fatal). If PLL never locks, subsequent register accesses will fail with garbage.
- **Discriminator:** "PLL and XTAL check failed!" in dmesg followed by downstream init failures (ASIC ready timeouts, vendor request errors). Vendor driver's retry code is described as "so buggy it must have never triggered" (`init.c:51-53`).

**T-013: MCU calibration failures**
- **Source:** `mcu.c:192-208`
- **Trap:** Calibration commands (R, DCOC, LC, LOFT, TXIQ, BW, DPD, RXIQ, TXDCOC) are sent via `CMD_CALIBRATION_OP(31)` with wait_resp=true. Failure returns error from MCU cmd path. If firmware rejects calibration, PHY performance degrades.
- **Discriminator:** MCU cmd timeout at calibration step. Distinguish from general MCU failure by the init stage — this happens after eFUSE read + PHY init start.

**T-014: Channel 14 power bugs**
- **Source:** `eeprom.c:270-273`
- **Trap:** CCK BW20 power for channel 14 is saved separately (`real_cck_bw20`). The TODO at `eeprom.c:199-201` notes that country region 33 requires B-mode before channel 14 — this is not implemented.
- **Discriminator:** Non-compliant TX on channel 14 if driver attempts HT mode. In practice, channel 14 is rarely used; trap is low-severity unless operating in Japan.

**T-015: TSSI saturation / invalid target power**
- **Source:** `eeprom.c:130-142`
- **Trap:** If EEPROM target power (`[0xd0]`) is > max_pwr from hardware or is zero, driver falls back to `0x20`. Invalid TSSI slope/offset values from EEPROM will cause TX power to be wrong.
- **Discriminator:** "EEPROM trgt power invalid XX" in dmesg. Manifests as weak signal or regulatory non-compliance.

**T-016: RX URB length mismatch**
- **Source:** `dma.c:117-131`
- **Trap:** RX segment validation checks multiple conditions on `dma_len`. If firmware sends malformed RX descriptors, `WARN_ON_ONCE` fires and segment is skipped. Persistent mismatches indicate firmware/driver version incompatibility.
- **Discriminator:** "RXWI zero fields are set" or "RX path seen a non-pkt urb" (`dma.c:97-100`). Frames dropped silently after the warning.

**T-017: TX retry reporting broken for MCS0**
- **Source:** `tx.c:39-54`
- **Trap:** Documented in source: "TX retry reporting is a bit broken." Retries reported once per AMPDU, often a frame early. For MCS0, retries are **never reportable** — must rely on transmission failures alone. PKT_ID 0 disables status reporting; MCS0 and MCS7 with is_probe share PKT_ID 9.
- **Discriminator:** Rate control appears broken for lowest MCS rates. Not a bring-up blocker but affects data-plane tuning.

**T-018: MAC start DMA busy timeout**
- **Source:** `init.c:234-236`, `init.c:250-252`
- **Trap:** After enabling TX, polls WPDMA for not busy (2s timeout). After enabling RX, same poll (0.5ms timeout). If DMA is stuck (firmware bug, descriptor corruption), MAC never starts.
- **Discriminator:** "Time out with reg XXXX" from `mt76_poll()`. Check which register — `MT_WPDMA_GLO_CFG` = DMA busy timeout.

**T-019: MAC stop hangs on page count drain**
- **Source:** `init.c:272-296`
- **Trap:** Stop sequence polls TX/RX page counts with 200 × 10ms = 2s timeout for TX, 200 × 1ms = 200ms for RX. If firmware is still processing descriptors, cleanup stalls. Warnings for TX DMA, MAC TX, MAC RX, RX DMA not stopping.
- **Discriminator:** "TX DMA did not stop!", "MAC TX did not stop!", etc. during cleanup. Indicates firmware is holding descriptors — may leak memory on repeated init/cleanup cycles.

**T-020: Vendor request size constraints for IVB upload**
- **Source:** `mcu.c:385-388`
- **Trap:** IVB (64 bytes) uploaded via vendor request `MT_VEND_DEV_MODE(1)`, value `0x12`. USB control transfer limit is typically 256B for bMaxPacketSize0, but some hubs/controllers have smaller limits. If the IVB upload fails, firmware won't start.
- **Discriminator:** Firmware running poll times out after IVB upload step (no "Firmware running!" message). Check if vendor request returned short count or error.

---

## 3. W2 minimal-STA MLME crib (from Linux mac80211 + driver source)

**Note:** The FreeBSD net80211 source was not available in the cloned tree (partial clone missing `sys/`). The following is derived from the Linux mt7601u driver's mac80211 interface, which shows what the host MLME must provide.

### 3.1 Interface mode constraint
**Source:** `init.c:612` — `wiphy->interface_modes = BIT(NL80211_IFTYPE_STATION)`

The driver **only supports STA mode**. No AP, mesh, P2P, or monitor (though active monitor is supported as a feature flag at `init.c:610`). This constrains the MLME to the STA state machine only.

### 3.2 TX queues + AC mapping
**Source:** `tx.c:10-18`, `tx.c:21-37`, `usb.h:34-42`

4 hardware TX queues, mapped from IEEE80211 AC:
- VO → HW Q3 (lowest priority), VI → Q2, BE → Q1, BK → Q0 (highest)
- Hardware uses mirrored order (`q ^ 0x3`)
- EDCA params configured via `mt7601u_conf_tx()` → writes `MT_EDCA_CFG_AC(n)` + WMM registers

### 3.3 WCID (WLAN Client ID) model
**Source:** `init.c:172-218`, `init.c:580-582`

- N_WCIDS stations, WCID 0 reserved for multicast
- APs get WCID starting at 1
- Each WCID has: index, TX rate, key index, rate set flag
- WCID attr mem initialized to all 1s; WCID addr mem to `0xffffffff`/`0x00ffffff` (invalid)
- Monitor mode uses fake WCID 0xFF

### 3.4 Key installation
**Source:** `init.c:193-199`, `mac.h` (key structs)

Keys installed in hardware key memory via burst write to `MT_SKEY_MODE_BASE_0`. 4 key slots per WCID. The driver's mac80211 `set_key` op programs the firmware's key table. For W2, keys derived from W0 crypto (PTK/GTK) must be pushed via this path.

### 3.5 Required MLME frames for minimal STA
Based on the driver's TX/RX path and mac80211 ops:

| Frame | Direction | Purpose |
|-------|-----------|---------|
| Beacon (passive scan) | RX only | Discover APs, parse SSID/capabilities |
| Probe Request | TX | Active scan of known SSID |
| Probe Response | RX | Reply to probe |
| Authentication (open/SAE) | TX/RX | Auth exchange |
| Association Request | TX | Join BSS |
| Association Response | RX | Get AID, capabilities |
| EAPOL-Key (4-way/SAE) | TX/RX | Key derivation (via W0 crypto) |
| Deauthentication | RX/TX | Disconnect |
| Data (CCMP-encrypted) | TX/RX | Encrypted data frames |

### 3.6 Firmware offload
**Source:** `mcu.h:29-51` — MCU commands include BEACON_OP(32), SWITCH_CHANNEL(30), ANTENNA(33), POWER_SAVING(20)

The firmware handles:
- Data-plane CCMP encryption/decryption (keys pushed via key mem)
- Beacon processing
- Channel switching
- TX rate control (firmware-driven, driver provides rate hints via TXWI)

---

## 4. Firmware provenance

**Source:** `linux-firmware` tree, `mediatek/mt7601u.bin` (45,412 bytes)
**WHENCE entry (`WHENCE:1629-1638`):**
- Driver: mt7601U — MediaTek MT7601U Wireless MACs
- File: mediatek/mt7601u.bin
- Version: 34
- License: "Redistributable. See LICENCE.ralink_a_mediatek_company_firmware"
- Source: http://www.mediatek.com/en/downloads/

**License note:** The LICENCE.ralink_a_mediatek_company_firmware file in the cloned tree is a cgit 404 (incomplete clone). The WHENCE entry states "Redistributable." The linux-firmware project includes this blob, which implies it has been reviewed for redistribution. For LogansOS, vendor under `boot/firmware/mediatek/mt7601u.bin` with the WHENCE attribution.

**Firmware header (from `mcu.c:268-281`):**
```c
struct mt76_fw_header {
    __le32 ilm_len;       // ILM length (includes 64B IVB)
    __le32 dlm_len;       // DLM length
    __le16 build_ver;     // Build version
    __le16 fw_ver;        // Firmware version
    u8   pad[4];
    char build_time[16];
};                        // 32 bytes total

struct mt76_fw {
    struct mt76_fw_header hdr;
    u8 ivb[MT_MCU_IVB_SIZE];  // 64 bytes (MT_MCU_IVB_SIZE = 0x40)
    u8 ilm[];                  // ILM image (ilm_len - IVB_SIZE)
                               // followed by DLM (dlm_len bytes)
};
```

---

## 5. Honest negatives

1. **BSD net80211 crib incomplete:** The FreeBSD source tree clone was partial (missing `sys/` directory). The MLME crib in §3 is derived from the Linux driver's mac80211 interface rather than BSD net80211 directly. For the actual W2 implementation, a full FreeBSD clone or direct reference to `sys/net80211/ieee80211_proto.h` and `ieee80211_sta.c` is recommended.

2. **License file unavailable:** The LICENCE.ralink_a_mediatek_company_firmware file resolved to a cgit 404 in the cloned tree. The WHENCE entry states "Redistributable" but the actual license text wasn't retrievable from this clone. Verify against the canonical linux-firmware repository before production use.

3. **eFUSE hard fail unresolved:** Trap T-011 (eFUSE usage map < 5 words) returns `-EINVAL` with no fallback path. The source says "this driver doesn't support it" — meaning a default EEPROM file would be needed, which isn't available in the open-source tree. This is a **known blocker for some clone dongles** and should be flagged as a hardware procurement risk.

4. **PLL/XTAL retry abandoned:** The vendor driver's PLL retry code was described by the upstream author as "so buggy it must have never triggered" (`init.c:51-53`). No alternative retry strategy exists. If PLL fails on first try, the device is unusable without replug.

---

## 6. Source provenance

| Source | Obtained | Commit/Version |
|--------|----------|----------------|
| Linux kernel mt7601u driver | `git clone` → temp dir | `2b07ea76f` (drivers/net/wireless/mediatek/mt7601u/, 24 files) |
| linux-firmware | Direct copy | `mediatek/mt7601u.bin` (45,412 bytes), WHENCE v34 entry |
| FreeBSD source | `git clone` → temp dir | Partial — missing sys/ (net80211 unavailable) |

All register values, struct layouts, and sequences above are from direct Read of the cloned source files. No WebFetch was used for any byte/register data.
