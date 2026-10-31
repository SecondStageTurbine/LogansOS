<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: W2 mt7601u — prior-art trap-mining + discriminator bank + I-14 fault-containment

**Date:** 2026-06-20
**Author:** Claude (PM)
**For:** Cerebro (W2 / WiFi-dongle lane). A governance artifact, not a build — it discharges the two standing-rule obligations the live W2 work did not yet fully cover: the **driver/blind-hardware prior-art trap-mining pass** and the **failure-mode + discriminator** section, plus the formal **I-14 fault-containment** write-up.

## 1. Why this exists (the load-bearing gap)
W2 bring-up cost **~8 blind iterations** on a marginal link, and the actual root cause — **wrong-channel TX from DS-parameter mis-tagging** — was **not localizable from the captured evidence**. The in-driver discriminators (`tx_stat`, `directed_to_us`, the `STATE` dump, `TXDUMP`) established "the AP never hears us" but **not why**; the cause was only found by an *external control experiment* (booting Linux and diffing its live state). That is exactly the defect the standing rule names: *"a failure you can't localize from the captured evidence is a defect in the dispatch."* This artifact pre-seeds the bank so the **next** run is conclusive on failure, not just on success — and mines the mt7601u prior-art that would have paid for those iterations up front.

**Acquisition note (per the byte-oracle corollary):** the signatures/constants below were pulled from the **raw upstream driver source** (`torvalds/linux drivers/net/wireless/mediatek/mt7601u/{mcu.c,phy.c}`) and the kernel bug/patch record, not a summarizing fetch — verbatim symbol names and values so the discriminators match real chip behavior. Sources in §6.

## 2. Prior-art trap-mining (mt7601u-specific, raw-sourced)
The documented field gotchas for **this exact chip**, each paired with the discriminator that makes it self-diagnosing:

| # | Prior-art gotcha (source) | Signature on our side | Discriminator to pre-seed |
|---|---------------------------|----------------------|---------------------------|
| **P1** | **MCU command wedge.** Upstream `mt7601u_mcu_wait_resp()` waits **300 ms × 5 retries** then `"Error: %s timed out"` / `-ETIMEDOUT`; "mt7601u dies during channel switch" is a known LKML report. | Our 3rd+ MCU channel-switch (ch7+) times out — the **post-ch6 wedge** (W1 F1-part-2). | Log every MCU cmd's **opcode + seq + elapsed-ms + COM_REG0 before/after**; on timeout dump `firmware_running` (`MT_MCU_COM_REG0 == 1`?) to split **MCU-dead vs MCU-slow vs seq-mismatch**. |
| **P2** | **MCU recovery = vendor reset.** Upstream recovers a wedged MCU via `mt7601u_vendor_reset(dev)` → `msleep(5)` → firmware reload → poll `firmware_running` 100×10 ms. A community `vnd_reset` module-param exists for exactly the variants that wedge. | No recovery path today ⇒ a wedge is terminal (re-plug). | Implement + log a **vendor-reset → COM_REG0-poll recovery** on MCU timeout; the poll progression is itself the discriminator (boots vs never-reaches-1). |
| **P3** | **Channel-banded TSSI/power.** `mt7601u_tssi_cal()` selects `tssi_data.offset[0]` for ch<5, `[1]` for ch<9, **`[2]` for ch≥9** — high channels use a *different* EEPROM calibration bucket. | ch9–14 power/link quality differs from ch1–6 even if the MCU switches. | Log the **selected tssi offset bucket + the EEPROM `tssi_data` for the tuned band**; a zero/garbage `offset[2]` explains high-channel weakness independent of the MCU wedge. |
| **P4** | **Per-frame RSSI exists upstream.** `mt7601u_phy_get_rssi()` = LNA lookup − `lna_gain` − `rssi_offset[0]`; Linux selects a BSS by signal. | Our `Bss` has **no signal field** ⇒ we pick the first SSID match, not the strongest (we'd target a weak node if it scanned first). | Add a `signal` field to `Bss` from the RXWI LNA math; **log RSSI for every BSS at scan**; select strongest. A "weak-node targeted" failure becomes visible in the scan dump. |
| **P5** | **Temp/DPD decalibration.** `mt7601u_temp_comp()` re-runs DPD when temp drift > **±450 units**; PLL-lock guard <−50/>+50; `__mt7601u_phy_freq_cal()` corrects RF-reg-12 freq offset (CCK 19/5, OFDM 102/32, HT 82/20). | Marginal/slow link even when "up"; drifts over time. | Periodic breadcrumb: **EWMA RSSI + `agc_tune` step (−0x20<−70dBm, −0x10<−60dBm) + temp reading + DPD-recal-triggered + freq-offset correction**. Splits **rate-control-stuck vs temp-drift vs freq-drift**. |
| **P6** | **Cold-init / firmware-upload wedge.** "firmware upload timed out", `probe failed -110` (`-ETIMEDOUT`) on cold plug; vendor-variant USB-ID collisions (same ID, different silicon) — the "vendor could not connect" class. | Cold-replug upload wedge (open). | Log the **`vendor_reset` → `COM_REG0` poll** progression on init; distinguish **USB-endpoint-stall vs MCU-no-boot vs wrong-variant** (dump efuse/chip-id). |
| **P7** | **Boot calibration sequence.** `mt7601u_init_cal()` runs `MCU_CAL_R, TXDCOC, LOFT, TXIQ, RXIQ, DPD` in order; a hang in any one bricks bring-up. | A cal step hanging looks like a generic init hang. | Log **each cal step's start/done**; a hang is localized to the exact calibration, not "init failed." |

## 3. The discriminator bank (capture-before-you-need-it)
The **channel triple** is the headline — the discriminator that would have caught the just-fixed bug on iteration 1, now mandatory on every W2 bring-up run:

> **Log, per association attempt: `beacon_ds_channel` (the DS Parameter Set IE in the AP's beacon) · `tuned_channel` (what we programmed via the `freq_plan[14]` table) · `tx_channel` (what the frame actually egressed on).** Any divergence among the three = wrong-channel TX. The bug was `beacon_ds_channel ≠ tuned_channel` (DS-param mis-tag); a future class is `tuned_channel ≠ tx_channel` (stale RF regs after a failed MCU switch — ties to P1).

Bank rule (the standing-rule contract): **every blind W2 run must be conclusive on failure.** Each row in §2 maps an *observation* to a *cause*. If a future failure does not fall into one of these buckets from the captured breadcrumbs, that is a new defect to add to the bank — not a re-debug from scratch. The instrumentation must be present **before** the run that needs it (e.g. P3's tssi-bucket log and P4's per-BSS RSSI must land before the next channel/roam test, not after it fails).

## 4. I-14 fault-containment (formal, auditable) — SATISFIED for this lane
Correcting the PM's earlier stale note ("DMA open, no IOMMU yet"): for the **USB** mt7601u, all three I-14 vectors are closed; this is the auditable record.

- **(a) Lock isolation — N/A by construction.** The mt7601u is **USB**: no hardware IRQ line, no device ISR. It is **xHCI-event-poll-driven**, so there is *no interrupt handler that could acquire an AI-governance lock* (`INFLIGHT`/`RECENCY`/`ARENA`/reservation/`TABLE`). The vector collapses — there is no ISR. (Grep-assert: no W2 path takes a governance lock from an xHCI completion.)
- **(b) Liveness degradation — bounded.** No hardware interrupt ⇒ no interrupt storm. If the poll loop monopolizes a CPU, the residency daemon degrades to **P4b's synchronous direct-reclaim backstop (the I-2 kill-switch)** — governance slows, never fails. Stated, not new.
- **(c) Memory isolation — CLOSED.** USB has **no device-initiated DMA**: transfers are the xHCI controller's, host-managed. Beyond that, every W2 buffer is **already fenced**: TX staging and the RX pool route through `dma_arena::fence(...)` (the **I-14 c1** guards `"tx staging outside the fence (I-14c1)"` / `"rx buffer outside the fence"` in `tx_raw`/`post_rx`), and **VT-d/IOMMU (c2) is proven** per the ledger (BUILD-V). A driver fault cannot reach governance or tensor memory.

**Conclusion:** the mt7601u W2 lane is I-14-containable on all three vectors **today**; no DMA-isolation prerequisite blocks it. This section makes that explicit so a future audit reads it rather than re-deriving it.

## 5. The forward fork (what to do with this)
The channel-fix ISO is flashed + verified, awaiting the boot:
- **If it connects** ⇒ the lane shifts to **robustness/consolidation**, and §2 becomes the build backlog, priority-ordered: **P2 (MCU vendor-reset recovery — unblocks ch7+ roam)** → **P4 (per-BSS RSSI selection — stop targeting weak nodes)** → **P3/P5 (high-channel TSSI + sustained-link temp/AGC cal)** → **P6 (cold-replug recovery)**. Each ships with its §3 breadcrumb wired in.
- **If it doesn't** ⇒ the §3 channel-triple + the §2 discriminators are immediately **what makes the next run conclusive** — no more external-Linux-diff archaeology.

Either way this is the right next governance move for the lane. A W2 *dispatch* (when the next build is scoped) carries this THINK's bank as its required failure-mode/discriminator section + the §4 I-14 paragraph verbatim.

## 6. Sources (raw-sourced prior-art)
- Linux mt7601u driver — `drivers/net/wireless/mediatek/mt7601u/mcu.c` (MCU wait/timeout, `mt7601u_vendor_reset`, `firmware_running`/`MT_MCU_COM_REG0`) and `phy.c` (`__mt7601u_phy_set_channel`/`freq_plan[14]`, `mt7601u_tssi_cal` channel-banded offsets, `mt7601u_temp_comp`, `mt7601u_phy_get_rssi`, `mt7601u_agc_tune`, `mt7601u_init_cal`, `__mt7601u_phy_freq_cal`) — `github.com/torvalds/linux`.
- `kuba-moo/mt7601u` issue #64 (init failure); LKML "mt7601u dies during channel switch"; linux-wireless "[RFC] make the driver work again" + the "vendor could not connect" / `vnd_reset` patch; Ubuntu bug #1716301; Linux Mint forum (newer-kernel breakage); "probe failed -110".
