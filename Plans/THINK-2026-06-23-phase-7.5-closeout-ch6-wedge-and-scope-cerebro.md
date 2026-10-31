<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase-7.5 closeout — the ch6+ channel-switch / bulk-pipe wedge (OT-27(d)) + the MT7601U-vs-iwlwifi scope call

**Date:** 2026-06-23
**Author:** Cerebro (owns the W1/W2 MT7601U lane)
**Dispatch:** `Plans/DISPATCH-2026-06-23-phase-7.5-closeout-ch6-wedge-cerebro.md`
**Kind:** Research/design THINK — **no code, no QEMU, no kernel change.** One of four parallel Phase-7.5-closeout research lanes (Cerebro / Prometheus / Oracle / Janus); the PM synthesizes the four into one closeout PLAN.
**Branch:** `codex/phase-7.5-closeout-ch6-wedge-cerebro` off `origin/main` `90774f9`.

---

## Executive summary (for the PM's synthesis)

- **Charter A (the wedge).** The ch6+ wedge is a **halted xHCI bulk endpoint with no host-side reset primitive**. `set_channel` fires a burst of MCU commands per switch; a cumulative 1→14 sweep kills the MT7601U firmware (the LKML "mt7601u dies during channel switch" bug); the resulting STALL/error halts the **in-band bulk-OUT** endpoint (`out_eps[0]`), and `recover_chip`'s forced firmware re-upload runs over that **still-halted** endpoint → `inband bulk-out timeout`. The kernel xHCI driver has **no `Reset-Endpoint` / `Set-TR-Dequeue` / `Clear-Feature(ENDPOINT_HALT)`** primitive (only the diagnostic `dump_halt_state`, xhci.rs:598). **Leading fix: add that xHCI endpoint-reset primitive and call it on the in-band pipe before the recovery re-upload.** Three captures must be added *before* the next bare-metal run for it to be conclusive (completion code, EP-context state, post-recovery MCU probe).
- **The reframe that may delete the gate.** The wedge blocks a **full 14-channel survey**, not **association to a known-channel AP**. The driver already has the Linux-faithful **targeted-connect** path (mod.rs:1681-1707): a station joining a *configured* AP tunes **once** and dwells — and **one switch cannot wedge** (the wedge is cumulative). So **cold end-to-end association to a configured AP does not require the endpoint reset at all.** The endpoint reset is required only to make a full *survey* reach ch7-14. This splits OT-27(d) into a P0 (none — association is already wedge-free) and a P2 (the survey hardening).
- **Charter B (the scope call).** **MT7601U closes Phase 7.5; iwlwifi is out of scope.** This is not a new call — the **bible already amended the target from iwlwifi → MT7601U on 2026-06-13** (docs/bible.md:179, per Article 24.1), listing iwlwifi as an explicit follow-up needing its own ADR. Silicon has since *proven* the research's prediction (enumerate → cold-upload → scan → associate, all on the OptiPlex 7080). iwlwifi is **~80 K LOC (14× MT7601U), PCIe (a whole new bus stack the USB lane lacks), and SoftMAC — so it deletes none of the supplicant work** and would discard proven silicon. **Recommendation: finish MT7601U's open tail (b/c/d + WPA3-on-air + cold-end-to-end); do not switch chips.**

---

# Charter A — the ch6+ channel-switch / bulk-pipe-reset wedge (OT-27(d))

## A.0 The failure, located in the live driver

**Per-switch MCU load.** `phy::set_channel` (phy.rs:385) issues, every channel switch: an RF PLL-plan write (`mcu_write_reg_pairs(RF, 17..20)`, phy.rs:393), a `TX_ALC_CFG_0` rmw (phy.rs:416), a BBP 62/63/64 write (phy.rs:420), `vco_cal`, `bbp_set_bw` (skipped when BW is unchanged — the F1 fix, phy.rs:175-198), `set_bw_filter`, and a per-rate TX-power re-assert. Each `mcu_write_reg_pairs`/`mcu_send` is a **bulk-OUT on the in-band pipe + a `CMD_DONE` wait** (mod.rs:894-915). A rapid 1→14 sweep is therefore *sustained* MCU command traffic — exactly the upstream-documented "mt7601u dies during channel switch" condition.

**The wedge sequence (file:line).**
1. The MCU stops answering → `mcu_wait_resp` exhausts its `5×` retry and returns **`mcu response timed out`** (mod.rs:956).
2. The next in-band bulk-OUT (`inband_out_sync`, mod.rs:850) never sees a good completion code and returns **`inband bulk-out timeout`** (mod.rs:865) — or `inband bulk-out completion error` (mod.rs:860) if it *does* get a completion, but a bad code (e.g. STALL=4).
3. `recover_chip` (mod.rs:1619) runs `vendor_reset` → force `COM_REG0=0` → `bring_up` — which re-uploads firmware over the **same in-band bulk pipe**. The re-upload fails.

**The pipe distinction the dispatch flags is real and load-bearing.** BUILD-2's `vend_retry` fix (`f378f12`) cured the cold-upload wedge on the **vendor CONTROL pipe** (every `mt7601u_vendor_request` retried 10×/5 ms). The ch6 wedge is on the **in-band BULK-OUT pipe** (`inband: EndpointPipe` = `out_eps[0]`, mod.rs:601-602) — a different endpoint, a different USB mechanism (a halted transfer ring / STALLed endpoint, not a NAKing control setup), needing a different fix. BUILD-2 already classifies it cleanly as `EndpointStall unresponsive=false` (responsive chip, stalled endpoint — *not* the `DeviceUnresponsive` all-ones bucket).

**The smoking gun is already a code comment.** `recover_chip`'s own NOTE (mod.rs:1616-1618): *"the inband xHCI endpoint may remain halted host-side after the wedge (no endpoint-reset primitive yet); if the reload fails here, that is the next capture-before-you-need-it item (an xHCI Reset-Endpoint)."* Confirmed: `grep` across `kernel/src/drivers/usb/xhci.rs` finds **no** `reset_endpoint` / `clear_halt` / `set_tr_dequeue` / `ClearFeature(ENDPOINT_HALT)` — only `dump_halt_state` (xhci.rs:598), which *reads* halt state but does not clear it.

## A.1 Ranked root-cause hypothesis bank (each: discriminator + the capture it needs)

> Ranking is by posterior given the code evidence + the dispatch's stated symptom ("recovery re-upload fails on the **bulk** pipe", i.e. the survey's `recovered_after_wedge == 0`).

### H1 — *(leading)* The in-band bulk-OUT xHCI endpoint is in the **Halted** state and is never reset
The STALL/transfer-error that ended the wedged transfer left the endpoint's xHCI context in **Halted (EP State = 2)**. Submitting the recovery re-upload's TRBs to a halted ring → they never execute → `inband bulk-out timeout`. `vendor_reset` is a *device-register* reset; it does **not** touch the **host-side** xHCI endpoint state, so the halt persists across recovery.
- **Discriminator:** read the in-band EP-context EP-State at three points — at wedge, after `vendor_reset`, after the full recovery. **Halted-then-still-Halted-after-vendor_reset ⇒ H1 confirmed** (the device was reset but the host endpoint is stuck). `dump_halt_state` already computes this; it is simply not wired into the wedge/recovery path.
- **Capture to add before the run:** (1) wire `dump_halt_state(inband)` into `inband_out_sync`'s error/timeout arm + into `recover_chip` (before and after `vendor_reset`); (2) **the completion code** — `inband_out_sync` currently collapses every bad code into the string `"inband bulk-out completion error"` (mod.rs:860), discarding the *which*. Log the raw code: **4 = STALL** (device+host halt — see H2), **6 = Babble**, **others = host-ring error** (see H4).
- **Fix:** add an xHCI **Reset-Endpoint** sequence (Reset-Endpoint TRB → `Set-TR-Dequeue-Pointer` TRB to a fresh ring position → ring doorbell), and for a USB STALL also a **Clear-Feature(ENDPOINT_HALT)** control request to the device endpoint. Call it on the in-band pipe **inside `recover_chip`, before `bring_up`'s re-upload.** This is a **new kernel xHCI capability** (a named build requirement; see A.4) — generic enough to also serve `cdc_ecm` bulk-stall recovery later. *(The exact Linux `mt7601u`/`usb.c` reset ordering + the Reset-Endpoint/Set-TR-Dequeue/Clear-Feature byte sequence is **Prometheus's trap-mine lane** — this THINK names it as the dependency, does not re-mine it.)*

### H2 — Device-side endpoint STALL not cleared (host reset alone is insufficient)
If the wedged completion code is **4 (STALL)**, the *device* endpoint is halted, not just the host ring. A host-only Reset-Endpoint clears the host context but the device will STALL/NAK the very next transfer until a **Clear-Feature(ENDPOINT_HALT)** control request resets the device's data toggle + halt.
- **Discriminator:** completion code **4** on the first wedged transfer; **and** after a host-only Reset-Endpoint (no Clear-Feature) the re-upload **still** stalls (code 4 again). ⇒ Clear-Feature is mandatory, not optional.
- **Capture:** the completion code (same add as H1). *(This is why H1's fix lists Clear-Feature for the STALL case — H2 is the sub-case that makes it non-optional.)*

### H3 — `vendor_reset` is too shallow; the chip needs a USB port reset / re-enumeration
The MCU may be dead in a way that a vendor-register reset cannot revive — needing an xHCI **port reset** (and possibly re-address/re-enumerate).
- **Discriminator:** `COM_REG0` after the *full* recovery (the existing `load_firmware` poll, referenced at mod.rs:1615). With the endpoint reset in place (H1/H2), **`COM_REG0` boots to 1 ⇒ H3 is false** (the endpoint was the only blocker). **Stuck at 0 after a clean endpoint + re-upload ⇒ H3** — escalate to a port reset.
- **Capture:** already present (`COM_REG0` poll in `load_firmware`); just ensure it is reported in the recovery breadcrumb.

### H4 — Transfer-ring dequeue desync (Reset-Endpoint without Set-TR-Dequeue)
A Reset-Endpoint **alone** leaves the ring's dequeue pointer parked on the failed TRB; the controller resumes on a stale/garbage TRB.
- **Discriminator:** after Reset-Endpoint *without* `Set-TR-Dequeue`, the first re-upload TRB still never completes; *with* `Set-TR-Dequeue` to a fresh enqueue position it does. (This is a *fix-correctness* discriminator, exercised when implementing H1.)
- **Capture:** the EP-context TR-Dequeue-Pointer + the in-band ring enqueue/dequeue indices in the recovery breadcrumb.

### H5 — Cumulative-load re-death (the reset only buys one channel)
Even a perfect endpoint reset + re-upload may die **again** on the *next* `set_channel`, because the trigger is the per-switch MCU load itself, not a one-off corruption.
- **Discriminator:** after recovery, issue **one benign MCU command** (e.g. a register read) and confirm `CMD_DONE` **before** the next `set_channel`. Responds-then-dies-only-on-the-next-switch ⇒ the per-switch load is the trigger (mitigation = the targeted single-switch path, A.2; or rate-limit / settle-delay between switches). The existing `recovered_after_wedge` counter (mod.rs:1772) already measures "tuned despite an earlier wedge" — H5 is "recovers but re-wedges immediately."
- **Capture:** a post-recovery benign-probe breadcrumb (new); `recovered_after_wedge` (exists).

### A.1.x — The primary discriminator already exists in the code
The survey loop's summary line (mod.rs:1771-1775) is purpose-built for exactly this question:
> `recovered_after_wedge > 0 ⇒ fast-skip suffices` (the vendor_reset+reload recovery works — **no endpoint reset needed**); `== 0 with wedges ⇒ needs endpoint/chip reset`.

The dispatch's framing ("recovery re-upload fails on the bulk pipe") is the **`== 0`** branch ⇒ points straight at **H1/H2**. **Caveat to verify before building:** confirm this value is from an *actual silicon run that exercised the W2 `recover_chip` code* (added in W2, THINK §5 P2), not from analysis/the pre-recovery W1 run. If `recover_chip` has not yet run on silicon, the next bare-metal run must capture `recovered_after_wedge` **plus** the H1 EP-state + completion-code captures to lock H1 over H3/H5 in a single pass.

## A.2 The reframe: association does not need the endpoint reset — only the survey does

The driver already carries the Linux-faithful **targeted-connect** path (mod.rs:1673-1707). Its own comment states the principle exactly:

> *"A station joining a KNOWN AP parks on that AP's channel; it does NOT rapid-hop all 14. The 1→14 sweep is precisely what cumulatively wedges this chip's MCU around ch6-7 … upstream NEVER reloads firmware to recover — it stays on the AP's channel. So when a channel is configured, tune ONCE and dwell: a single channel switch, no wedge, no firmware reload, no endpoint reset."* (mod.rs:1676-1680)

Consequence for the closeout gate:
- **Cold end-to-end association to a *configured-channel* AP needs:** (a)✓ cold firmware self-upload (DONE, `f378f12`) → targeted single-tune to the AP's channel (**no wedge** — one switch is not cumulative) → the in-kernel supplicant auth→assoc→4-way→CCMP (**proven on silicon**, `4f82155`) → (b) GTK + DHCP-over-`wlan0`. **The endpoint reset is not on this path.**
- **The endpoint reset (H1) is required only to make a full *14-channel survey* succeed cold** — i.e. to *discover* an AP whose channel is not pre-configured.

**Recommended split of OT-27(d):**
- **(d-assoc) — P0, already wedge-free:** prove cold-upload → targeted-tune → associate → DHCP in one cold boot, sink-captured. Uses the configured-target path; **no new kernel primitive**. This is the headline "WiFi connects on bare metal" deliverable and it does **not** block on the endpoint reset.
- **(d-survey) — P2, the hardening:** the xHCI Reset-Endpoint primitive (H1/H2) so a full survey reaches ch7-14. Needed for "join an AP whose channel I don't know in advance"; valuable but not on the association critical path. If H1 proves harder than budgeted, **the targeted path is the standing fallback** — the phase can close on (d-assoc) with (d-survey) tracked as a follow-on.

## A.3 The next bare-metal run — conclusive on PASS *and* FAIL (blind-hardware standing rule)

**Captures to add BEFORE the run (capture-before-you-need-it):**
1. Raw **completion code** in `inband_out_sync`'s error/timeout arms (mod.rs:857-865) — STALL(4) vs Babble(6) vs ring-error vs no-completion.
2. `dump_halt_state(inband)` (xhci.rs:598) wired into: the wedge point, post-`vendor_reset`, post-recovery — emitting EP-State + TR-Dequeue-Pointer.
3. A **post-recovery benign-MCU-probe** breadcrumb (H5).
4. Surface `recovered_after_wedge` + `COM_REG0`-after-recovery + a new `ep_state_after_reset` field in the scan summary.

**The discriminator truth table (what each outcome proves):**

| Observation | Verdict |
|---|---|
| EP-State Halted at wedge, **still Halted after `vendor_reset`** | **H1** — endpoint reset is the fix (host-side halt survives the device reset) |
| Wedged completion code = **4 (STALL)**, and re-upload re-stalls after a host-only reset | **H2** — Clear-Feature(ENDPOINT_HALT) is mandatory, not optional |
| After endpoint reset + re-upload, `COM_REG0` **boots to 1** | H1/H2 confirmed sufficient; **H3 false** — recovery now works |
| After a clean endpoint + re-upload, `COM_REG0` **stuck at 0** | **H3** — escalate to xHCI port reset / re-enumerate |
| Re-upload TRB never completes with Reset-Endpoint but **no** Set-TR-Dequeue; completes **with** it | **H4** — Set-TR-Dequeue is required (fix-correctness) |
| Recovery succeeds, benign probe answers, **then dies on the next `set_channel`** | **H5** — per-switch load is the trigger; use the targeted path / inter-switch settle |
| `recovered_after_wedge > 0` on this run | the existing vendor_reset+reload recovery already suffices — **endpoint reset deprioritized** |

## A.4 Named build requirements (for the closeout PLAN — not implemented here)

1. **Kernel xHCI endpoint-reset primitive** (`kernel/src/drivers/usb/xhci.rs`): Reset-Endpoint TRB + Set-TR-Dequeue-Pointer + a device Clear-Feature(ENDPOINT_HALT) helper. **Cross-references Prometheus's trap-mine** for the exact Linux `mt7601u`/`usb.c` reset ordering + byte sequences (do not re-derive). I-14 note: the re-upload DMA still routes through `dma_arena::fence` (the recovery re-runs `load_firmware`, already fenced) — the new primitive is control/command-ring only, no new device-initiated DMA path, so the fault-containment posture is unchanged.
2. **Completion-code + EP-state capture** in `inband_out_sync` / `recover_chip` (A.3 items 1-2) — small, do it in the same build as #1 so the bring-up run is self-diagnosing.
3. **(d-assoc) cold-end-to-end scenario** — cold-upload → targeted-tune → associate → DHCP-over-`wlan0`, sink-captured PASS. Depends on (b) GTK/DHCP (Prometheus's lane), **not** on #1.

---

# Charter B — the scope call: does MT7601U close the WiFi phase, or is iwlwifi in scope?

## B.0 The fork is already resolved in the bible — this re-validates it against silicon

`docs/bible.md:179` (current `main`):
> *"WiFi — Phase 7.5. Initial chipset target is **MediaTek MT7601U** (USB SoftMAC, ~5.5K-LOC driver, BSD-3-Clause redistributable firmware), **amended 2026-06-13 from Intel iwlwifi per Article 24.1** … research FINDINGS showed FullMAC saves no supplicant, so the smallest USB-attached BSD-licensed SoftMAC chip is strictly superior … WPA2/WPA3 client mode; hardware-default regulatory; bundled redistributable firmware … Power management, AP mode, mesh, WPA Enterprise, and **additional chipset families (Intel iwlwifi, Realtek, Qualcomm) are explicit follow-ups requiring their own ADRs.**"*

So the dispatch's "the bible Art. 13 opens 7.5 as 'iwlwifi family'" is the **superseded original wording**; the live bible already names MT7601U and demotes iwlwifi to a follow-up. The PL's deferral of "the scope fork to this analysis" is a request to **re-confirm that 2026-06-13 amendment now that silicon has spoken** — and silicon strengthens it.

## B.1 What MT7601U already satisfies vs the bible's WiFi acceptance intent

**Acceptance intent (bible:179):** WPA2/WPA3 client mode · hardware-default regulatory · bundled redistributable firmware.

**Proven on silicon (OptiPlex 7080):**
- **Enumerate** `148f:7601`, read ASIC/MAC/EEPROM IDs matching refs (ADR-054).
- **Cold firmware self-upload + boot** — OT-27a BUILD-2 `f378f12` (no Linux warm-load; `com_reg1` real DMA-done; 12 BSSIDs scanned).
- **Scan** 10-12 real 2.4 GHz BSSIDs (W1, ADR-054).
- **Associate** — auth → assoc → 4-way → **pairwise CCMP** on silicon (`4f82155`); SNonce is now a real CSPRNG draw (OT-2 closed `d1c8bb6`) so it is a genuine security boundary.
- **Hardware-default regulatory** — the driver honors the chip's regulatory domain (`ee.reg_start`/`ee.reg_num`, mod.rs:1709) + real per-channel eFUSE power (phy.rs:409). Satisfies "hardware-default regulatory."
- **Bundled redistributable firmware** — OT-27a Limine-module blob.

**Owed (the open tail — all MT7601U *hardening*, none iwlwifi-related):**
- **(b) GTK / broadcast-RX + DHCP-over-`wlan0`** — the *usable IP link* (group key + lease). Pairwise CCMP works; group key + a DHCP lease over the WiFi `NetDevice` are next. **[Prometheus's lane.]**
- **(d-survey) the 14-channel survey wedge** — see Charter A; **not** required for known-channel association.
- **(c) the intermittent SMP AP-bringup hang.** **[Oracle's lane.]**
- **WPA3/SAE on-air** — W0 shipped SAE crypto KAT'd against IWD vectors, but **on-air SAE association is code, not yet silicon-proven.** Flag as a named closeout deliverable (the bible promises "WPA2/**WPA3** client mode").
- **Cold end-to-end in one boot** — cold-upload → targeted-associate → DHCP, single cold boot, sink-captured PASS (the (d-assoc) deliverable, A.2).

**Read:** the bible's acceptance *intent* is largely **met or in-flight on MT7601U**; the residual is hardening + a WPA3-on-air confirmation, not a capability the chip lacks.

## B.2 The honest cost of an iwlwifi driver (if it were in scope)

From `RESEARCH-2026-06-13-wifi-target-fullmac-vs-softmac-FINDINGS.md` (cited, adversarially verified) — the load-bearing facts:

- **Size:** iwlwifi is **~80,000+ LOC** across generations (3168/AX200/AX210/BE200, each a separate firmware + protocol) vs MT7601U's **~5,500 LOC** — **~14×**. Difficulty rated **10/10** (the max in the field); MT7601U **4/10** (the min among viable candidates).
- **Bus:** iwlwifi is **PCIe**. The Phase-7.5 connectivity lane is **USB** (xHCI + USB core). iwlwifi rides **none** of it — it needs a **from-scratch PCIe enumeration + MSI + BAR-mapping bus stack** that does not exist today. MT7601U's bus bring-up was **zero** (it rode 7.5-A/B).
- **The supplicant is NOT deleted.** iwlwifi is **SoftMAC** — the host (mac80211 in Linux; *our in-kernel supplicant* here) builds **every** mgmt frame and runs the **entire** MLME + 4-way. iwlwifi offloads only data-plane crypto. So the from-scratch supplicant W2 already built (scan/auth/assoc/4-way/CCMP) would have to be **re-targeted to iwlwifi's host-command interface** — *plus* the 80 K-LOC driver *plus* the PCIe stack. **iwlwifi is strictly more work on every axis, with zero supplicant savings.**
- **The easier-path question (standing rule):** is there a chip class that runs MLME+4-way in firmware so we delete the supplicant? Per the FINDINGS, the **only** mainline driver that offloads the 4-way is **Broadcom `brcmfmac` with `BRCMF_FEAT_FWSUP`** — and that is narrow (embedded **SDIO** BCM in phones/tablets), **not** available in USB-dongle firmware. **No USB FullMAC-with-FWSUP option exists.** The supplicant burden is **irreducible for any USB chip** — and **W2 already paid it and proved it on silicon.** Switching chips re-pays it for nothing.

## B.3 Recommendation + residual risk (you recommend; the PL decides)

**RECOMMENDATION: MT7601U closes Phase 7.5. iwlwifi is out of scope.**
- It is already the bible's chosen target (amended 2026-06-13); the live silicon proof (enumerate → cold-upload → scan → associate) is exactly the path the 2026-06-13 research predicted.
- The closeout work is **finishing MT7601U's open tail** — (b) GTK+DHCP-over-`wlan0`, (d-assoc) cold-end-to-end, (d-survey) the endpoint reset, (c) the SMP hang, and a WPA3-on-air confirmation — **not switching chips.**

**Residual risk — MT7601U-closes-it (LOW):**
- The open tail (b/c/d) is *hardening of a proven driver*, each item silicon-boundable one at a time.
- The survey wedge (d-survey) — if the endpoint reset (A.2/H1) proves stubborn, the **targeted-connect path is the standing fallback** that still delivers cold association (d-assoc) without it. The phase can close on association with the survey tracked as a follow-on.
- **WPA3/SAE on-air is unproven on silicon** — call it out as an explicit named closeout deliverable so "WPA3 client mode" is not assumed-done from crypto KATs alone.

**Residual risk — iwlwifi-required (HIGH, and unnecessary):**
- A multi-build, from-scratch **PCIe stack + ~80 K-LOC driver** effort that restarts connectivity from zero on the hardest chip in the field, **discards** proven MT7601U silicon, and **still** requires the same in-kernel supplicant. Justified **only** if a hard requirement emerges that MT7601U physically cannot meet — and none is identified: WPA2/WPA3 client mode + hardware-default regulatory are all within MT7601U's envelope (WPA2 proven; WPA3 crypto in hand, on-air pending).

**Net:** rule **"MT7601U closes Phase 7.5"**; keep iwlwifi exactly where the bible already put it — a future chipset-family follow-up behind its own ADR, pulled only on a concrete future need (a device the user actually has that MT7601U can't serve), never as a Phase-7.5 gate.

---

## Cross-lane handoffs (for the PM's synthesis)
- **→ Prometheus (trap-mine):** the exact Linux `mt7601u`/`usb.c` **Reset-Endpoint / Set-TR-Dequeue / Clear-Feature(ENDPOINT_HALT)** byte sequence + ordering (A.1/H1, A.4#1); and (b) GTK/group-key + DHCP-over-`wlan0` (Charter B owed tail).
- **→ Oracle:** (c) the SMP AP-bringup hang is orthogonal to the wedge but on the same cold-boot path; the (d-assoc) end-to-end scenario will exercise it.
- **→ Janus (gap table / close-criteria):** map the bible:179 acceptance intent (WPA2/WPA3 client mode · hardware-default regulatory · bundled firmware) to: WPA2-assoc ✓silicon, regulatory ✓, firmware ✓; **owed** = (b) GTK+DHCP, (d-assoc) cold-end-to-end, **WPA3-on-air**, with (d-survey)+(c) as hardening follow-ons.
- **Suggested closeout ordering (my lane's view):** (d-assoc cold-end-to-end, wedge-free via targeted path) + (b GTK/DHCP) are the P0 "WiFi actually connects + gets an IP cold on bare metal" gate; the xHCI endpoint reset (d-survey), (c) SMP hang, and WPA3-on-air are the P1 hardening that makes the link self-sufficient and complete.
