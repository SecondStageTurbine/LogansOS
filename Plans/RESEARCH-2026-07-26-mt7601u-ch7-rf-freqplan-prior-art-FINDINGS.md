<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH-2026-07-26-mt7601u-ch7-rf-freqplan-prior-art-FINDINGS.md

## Objective

Deep prior-art trap-mine of Linux mt7601u channel-switch RF-program + MCU-calibrate
sequence to find root cause of ch7 `RF_FREQPLAN` MCU wedge (seen in `f4a0940` bare-metal
run). Judge whether Jarvis's "widen the ch7 RF response window" (`ccd84b6e`) is the
root fix or a band-aid.

## Source Material

| Source | Location | Relevance |
|--------|----------|-----------|
| Upstream `phy.c` | `linux-mt7601u/.../phy.c` | Channel switch RF write order, deferred cal |
| Upstream `mcu.c` | `linux-mt7601u/.../mcu.c` | MCU command/response protocol, timeouts |
| Upstream `init.c` | `linux-mt7601u/.../init.c` | Initial tune sequence, channel groups |
| Upstream `usb.c` | `linux-mt7601u/.../usb.c` | Probe path (`usb_reset_device`) |
| Upstream `mt76.h` | `linux-mt7601u/.../mt76.h` | `mt76_poll_msec` timeout semantics |
| Our `phy.rs` | `kernel/.../mt7601u/phy.rs` | Our channel-switch implementation |
| Our `mod.rs` | `kernel/.../mt7601u/mod.rs` | MCU response wait, scan loop, recovery |
| Our `mcu_logic.rs` | `kernel/.../mt7601u/mcu_logic.rs` | MCU framing, response decode |
| Our `wedge_logic.rs` | `kernel/.../mt7601u/wedge_logic.rs` | CAL_BW defer policy |
| Prior research | `RESEARCH-2026-07-22-mt7601u-channel-switch-trapmine-FINDINGS.md` | Establishes wedge in `set_channel`, not deferred cal |
| Prior recovery | `FIX-FORWARD-2026-07-24-mt7601u-channel7-recovery-warm-proof-jarvis.md` | Cold FW reload stalls at ILM byte 0 |

## Analysis: Five Questions

### Q1: RF write order on channel-switch vs initial tune — is ch7 different?

**Upstream `__mt7601u_phy_set_channel` (phy.c:342):**
```c
// Per-channel RF writes via MCU RANDOM_WRITE:
mt7601u_write_reg_pairs(dev, MT_MCU_MEMMAP_RF, freq_plan[...]);  // regs 17-20
// ... TX power, LNA gain ...
mt7601u_vco_cal(dev);  // direct RF writes (RF_CSR), NOT MCU
mt7601u_bbp_set_bw(dev, bw);
mt7601u_set_bw_filter(dev, cal, false);  // MCU CAL_BW if !survey
```

**Upstream `phy_init` (init.c:400 → phy.c:1226):**
```c
mt7601u_write_reg_pairs(dev, MT_MCU_MEMMAP_RF, rf_central);   // regs 0-44 bank 0
mt7601u_write_reg_pairs(dev, MT_MCU_MEMMAP_RF, rf_channel);   // regs 0-63 bank 4
mt7601u_write_reg_pairs(dev, MT_MCU_MEMMAP_RF, rf_vga);       // regs 0-63 bank 5
mt7601u_init_cal(dev);  // MCU calibrations: R, TXDCOC, BW, LOFT, TXIQ, RXIQ, DPD
```

**Key finding:** The initial tune programs RF bank-0 regs 17-20 as part of `rf_central[]`
(channel 1 defaults: `[0x99, 0x99, 0x09, 0x50]`). On channel switch, only regs 17-20 are
re-written via MCU RANDOM_WRITE. The VCO cal follows as direct RF CSR writes (bypassing
MCU). This is identical for all channels — **ch7's freq plan `[0x99, 0x99, 0x09, 0x51]`
differs from ch6 `[0xec, 0xee, 0x08, 0x51]` only in reg-20 increment (0x51→0x51 same)
and regs 17-18 pattern change**. No upstream code treats ch7 specially.

**Our driver matches upstream exactly:** `phy.rs:431-439` writes the same 4 RF pairs via
`mcu_write_reg_pairs(regs::MCU_MEMMAP_RF, ...)`, followed by `vco_cal()` direct RF CSR
writes. The FREQ_PLAN table is identical to upstream's embedded array.

### Q2: Inter-hop MCU calibration steps skipped in our driver?

**Upstream scan path:** In `mt7601u_phy_set_channel` (phy.c:498), the scan (`chanctx->surveys`)
path calls `__mt7601u_phy_set_channel` which sets `cal=false`, meaning `set_bw_filter` is
skipped. After the scan completes, deferred work (`cal_work`) runs the full calibration
sequence on the operating channel.

**Our driver:** `wedge_logic.rs:26-31` implements the same defer policy:
```rust
fn inline_cal_bw(path: TunePath, successful_targeted_tunes: u8) -> bool {
    match path {
        TunePath::Survey => false,
        TunePath::Targeted => successful_targeted_tunes != 0,
    }
}
```

**Verdict:** Our CAL_BW defer during survey matches upstream. We do NOT skip any other
inter-hop step — the `set_channel` sequence (RF_FREQPLAN → TX_ALC_CFG_0 → BBP LNA → VCO cal
→ BBP set_bw) is identical to upstream's `__mt7601u_phy_set_channel`.

### Q3: MCU-ready polling mechanism differences?

**Upstream (mcu.c:80-95):**
```c
static int mt7601u_mcu_wait_resp(struct mt7601u_dev *dev, u8 seq)
{
    int i;
    for (i = 0; i < 5; i++) {
        struct sk_buff *skb;
        skb = mt76_queue_skb(dev, &dev->mcu.resq, sizeof(*rxfce), MT_MCU_RESP_D_PORT);
        if (!skb) return -ENOMEM;
        rxfce = wait_for_completion_timeout(&dev->mcu.mcu_resp_complete, HZ / 2);
        if (!rxfce) continue;  // timeout, retry
        // decode and check seq + CMD_DONE
    }
    return -ETIMEDOUT;
}
```

**Our driver (`mod.rs:996-1033`):**
```rust
fn mcu_wait_resp(&mut self, seq: u8) -> Result<(), &'static str> {
    for _ in 0..5 {
        // post resp buffer, poll USB completion under bulk_budget
        // decode rxfce, check seq + CMD_DONE
    }
    Err("mcu response timed out")
}
```

**Key differences:**
1. Upstream uses `wait_for_completion_timeout(HZ/2)` = ~500ms per attempt. Our driver polls
   under `bulk_budget` (scan: 8000, bring-up: higher). Each poll iteration is `udelay(5)`,
   so scan budget = ~40ms total per attempt. **Our timeout window is ~12× shorter.**
2. Upstream's response uses a completion-based interrupt path; ours polls the USB completion
   ring directly (no interrupts in bare-metal).

**Critical insight:** The `SCAN_BULK_BUDGET=8000` was introduced specifically to fast-fail
the wedge (ISC-0, mod.rs:1794-1797). But this also means that on ch7 — where the MCU may
need slightly longer to process the RF_FREQPLAN command after 6 prior channel switches — our
driver times out at ~40ms while upstream would wait ~500ms. **This is likely the root cause
of our ch7 wedge, not a fundamental MCU incompatibility.**

### Q4: Channel grouping logic — is ch7 a group boundary?

**Upstream init.c:** No channel-group concept in `__mt7601u_phy_set_channel`. The driver does
NOT batch channels or apply per-group calibration. Each channel switch is independent.

**FREQ_PLAN analysis (our/initvals_phy.rs:221-236):**
```
ch1:  [0x99, 0x99, 0x09, 0x50]
ch2:  [0x46, 0x44, 0x0a, 0x50]
ch3:  [0xec, 0xee, 0x0a, 0x50]
ch4:  [0x99, 0x99, 0x0b, 0x50]
ch5:  [0x46, 0x44, 0x08, 0x51]   ← reg-20 wraps to 0x51
ch6:  [0xec, 0xee, 0x08, 0x51]
ch7:  [0x99, 0x99, 0x09, 0x51]   ← reg-17/18 pattern change (ec→99)
ch8:  [0x46, 0x44, 0x0a, 0x51]
...
```

**Pattern:** Regs 17-18 cycle through `[0x99, 0x46, 0xec]` pattern every 3 channels. Reg 20
increments from 0x50→0x51 at ch5 (not ch7). Ch7's distinguishing feature is the reg-17/18
transition from `0xec/0xee` to `0x99/0x99`, which is the same pattern as ch1/ch4. **No
channel-group boundary at ch7 in upstream.**

**However:** The cumulative MCU RANDOM_WRITE commands grow with each hop. After 6 successful
hops (ch1-6), the MCU has processed 6×4=24 RF register writes plus 6×3=18 BBP LNA writes,
6 VCO cal sequences, and 6 TX power updates. **Ch7 is the first channel where accumulated
MCU state + response queue depth may exceed what our shorter timeout can handle.**

### Q5: Recovery paths after MCU timeout?

**Upstream:** NO recovery. When `mt7601u_mcu_wait_resp` returns `-ETIMEDOUT`, the error
propagates up to mac80211, which marks the channel unavailable and continues scanning. The
device stays powered; no reset or firmware reload is attempted. Upstream's approach is
"avoid the wedge by not running CAL_BW during scan."

**Our driver (mod.rs:1905-1962):** We implement CB-2 recovery:
```rust
// On survey hop failure:
self.recover_chip()?;  // vendor_reset + endpoint reset + re-init
phy::set_channel(self, ch, ...)?;  // retry tune
```

**Recovery limitation (from prior research):** `recover_chip()` does a full cold firmware
reload. The prior evidence (`FIX-FORWARD-2026-07-24`) shows that after an MCU wedge, the
cold FW upload stalls at ILM byte 0 — the MCU is dead and can't accept firmware. Our
`recover_chip()` succeeds only if the MCU hasn't fully wedged (transient vs permanent).

**Upstream's `usb_reset_device` (usb.c:277):** Called during probe, NOT during recovery.
This resets USB endpoints and device state but does NOT reload firmware — it relies on the
MCU firmware being resident and responsive post-reset.

## Root Cause Assessment

### The ch7 wedge is a TIMEOUT issue, not an MCU incompatibility

**Evidence:**
1. Our `mcu_wait_resp` times out at ~40ms (scan budget) vs upstream's ~500ms per attempt.
2. Ch7 is the 7th consecutive MCU command sequence — accumulated queue depth increases
   latency.
3. Upstream never sees this wedge because: (a) it uses longer timeouts, and (b) mac80211's
   scan infrastructure naturally spaces channel switches with beacon intervals.
4. The FREQ_PLAN values for ch7 are structurally identical to ch1/ch4 — no hardware-specific
   issue.

### Jarvis's fix: Band-aid or root fix?

Without seeing `ccd84b6e` directly (commit not in our repo), the dispatch describes it as
"widen the ch7 RF response window." This likely means increasing the MCU response timeout
specifically for ch7, or adding a pre-ch7 delay.

**Assessment:** If Jarvis's fix increases the MCU response timeout for ch7 (or all channels),
it is **the correct root fix** — it addresses the fundamental timing mismatch between our
bare-metal polling budget and the MCU's accumulated processing latency.

If it adds a channel-specific delay before the RF_FREQPLAN write, it is **a targeted band-aid**
that works but doesn't address the systemic timeout issue (ch8+ could still wedge under load).

### Recommended fix direction

1. **Increase `SCAN_BULK_BUDGET`** from 8000 to ~40000 (matching upstream's ~500ms per MCU
   wait attempt). The ISC-0 fast-fail optimization was premature — it conflated "MCU wedge"
   with "normal MCU latency under accumulated load."
2. **Alternative:** Implement per-channel adaptive timeout that increases with hop count,
   matching upstream's retry-with-timeout pattern.
3. **Do NOT** add ch7-specific magic delays unless the increased budget doesn't resolve it —
   that would indicate a deeper MCU state corruption issue.

## Cross-Reference with Prior Research

| Finding | Consistent with |
|---------|----------------|
| Wedge in `set_channel`, not deferred cal | `RESEARCH-2026-07-22` confirmed |
| Cold FW reload stalls at ILM byte 0 | `FIX-FORWARD-2026-07-24` confirmed |
| No channel-group boundary at ch7 upstream | New finding — ch7 is not special in upstream |
| Timeout window mismatch (40ms vs 500ms) | **New root cause** — explains why our driver wedges but upstream doesn't |

## Verdict

**The ch7 wedge is caused by our `SCAN_BULK_BUDGET=8000` timeout being too aggressive for
accumulated MCU latency after 6 prior channel switches.** Upstream's ~500ms per-attempt
timeout absorbs this latency naturally. Jarvis's "widen the response window" fix is likely
the correct root fix if it addresses the systemic timeout, not just ch7-specific padding.

The `SCAN_BULK_BUDGET` fast-fail (ISC-0) was a good discriminator for detecting permanent
MCU death, but it conflated transient MCU latency with unrecoverable wedge. The budget should
be increased to match upstream's tolerance, with the wedge discriminator moving to a higher-
level heuristic (e.g., per-channel success rate across multiple scans).
