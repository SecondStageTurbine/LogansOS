<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-054: BUILD-W1 — the MT7601U USB SoftMAC driver (scan)

**Status:** **Stage-B GATE — COMPLETE sink-captured PASS on silicon (OptiPlex 7080, 2026-06-16).** The driver enumerated the `148f:7601` dongle, loaded/ran firmware, read eFUSE, brought up MAC/PHY, and **scanned 10 real 2.4 GHz BSSIDs** — with every DISPATCH §2 reference read matching exactly (ASIC `0x76010001`, MAC rev `0x76010500`, fw Build `0x7640`, EEPROM ver `0x0d`/fae `0x00`, MAC `xx:xx:xx:xx:xx:xx`; eFUSE hard-fail cleared). The formal gate frame was emitted to BOTH serial and the NVMe log-sink: `NEO_TEST/1 event=PASS name=mt7601u_scan_baremetal … scan=bssids_listed i14=usb_contained opens=adr-054`, with `stage=9 … bss=10 bound=true fence_refusals=0 bind_fail=None`. I-14 clean: **zero unfenced DMA** (`fence_refusals=0`). Evidence: `Plans/archive/evidence/phase7.5-build-W1-mt7601u-optiplex-7080-stageB-silicon.log` (kept in the private repo). Host-tier byte-oracles (firmware/MCU/eFUSE/RX layouts) green (I-12). **F1 (ch6 bandwidth-churn — the `bbp_set_bw` bandwidth-tracking early-return fix) and F2 (cold-upload reliability — the reset-and-retry + bounded fast-fail upload commits) are CONFIRMED on silicon this run** — ch6 tuned and scanned, firmware loaded once and stayed resident across warm reboots. F3 (sink-capture) is also closed: the gate frame landed in the NVMe sink. **One characterized reliability follow-on remains (F1-part-2; NOT a gate failure):** after ch6 the chip wedges under sustained channel-switch command load (`mcu response timed out` / `inband bulk-out timeout`), so ch7-14 are skipped — 10 BSSIDs (ch1-6) is well past the gate, but a full 14-channel scan needs the upstream persistent-MCU-response-buffer pattern + likely a per-channel/endpoint recovery; sub-item, a scoped bulk-poll budget over the scan loop so wedged channels skip in seconds rather than burning the full bulk-poll timeout (the cause of this run's ~25-min wall time).
**Date:** 2026-06-14 (Stage-B silicon COMPLETE PASS 2026-06-16)
**Dispatch:** `Plans/DISPATCH-2026-06-14-phase-7.5-build-W1-mt7601u-driver-jarvis.md`
**Bring-up reference:** `Plans/RESEARCH-2026-06-14-mt7601u-bringup-trap-mine-FINDINGS.md` (§1 file:line-cited sequence, §2 the 27-trap bank) + `THINK-2026-06-14-phase-7.5-build-W1-mt7601u-driver.md`.
**Builds on:** ADR-046 (xHCI/USB core, the class-driver seam), ADR-047 (CDC-ECM class-driver precedent), ADR-045 (the fenced DMA arena, I-14(c1)), ADR-050 (VT-d fence domain, I-14(c2)), ADR-051 (W0 supplicant crypto — W2 will join it to W1's frames).
**Extended by:** ADR-057 (W2 — the in-kernel supplicant that drives this scan driver to association: MLME + 4-way + CCMP + NetDevice, joining W0's crypto to these frames; it wires the data-TX endpoint + WCID key table this build left unused).
**Ledger:** **no new invariant, no new ordered lock, no new `unsafe` idiom.** The driver state is one `SwitchMutex` at the existing `lock_order::USB_DEVICE` (29); the `unsafe` is the established USB-transfer/MMIO idiom (ADR-046/047). Default-off ⇒ bit-for-bit today — the Rule-1 kill-switch is the `mt7601u` feature gate (verified: the default-feature kernel builds unchanged, and the driver + PHY tables + the 45,412-byte firmware blob are not compiled).

## The decision

W1 brings the **MT7601U USB SoftMAC driver up to and including scan**: enumerate the
`148f:7601` dongle on the 7.5-A/B USB core, load its firmware, read eFUSE, bring up
MAC/PHY, and **list real 2.4 GHz BSSIDs on bare metal**. It is the WiFi arc meeting silicon.
W1 proves *we can see the air*; **W2** (the MLME auth/assoc/4-way state machine wiring
W0/W0b's proven crypto to W1's frames) proves *we can join it* and is explicitly out of
scope here, as are the `NetDevice` glue, CCMP on-air, and AP/mesh/P2P/roaming (DISPATCH §8).

### The byte-oracle spine (I-12) — what is *proven*, host-side, now

The driver is split so the byte-layout-defining logic is a set of **pure, kernel-free cores**
(`fw_logic`, `mcu_logic`, `eeprom_logic`, `rx_logic`) path-`mod`'d into `tools/ai-ops-tests`
and asserted against **hand-laid byte fixtures** computed independently of the parser — the
CLAUDE.md byte-oracle rule (a writer+reader pair from one spec agree even when both are
wrong, so the oracle's bytes are hand-derived from the raw `struct` definitions). Four
oracles are green:

- **ISC-2 (firmware):** `mt76_fw_header` parse, the three distinct `-ENOENT` validation
  causes, the ILM/DLM/IVB partition, the FCE TXINFO word (`DMA_PACKET | CPU_TX_PORT | LEN`),
  the per-chunk DMA frame, the `MCU_FW_URB_MAX_PAYLOAD` chunk count.
- **ISC-3 (MCU):** the DMA command-wrap header (`SEQ | cmd | LEN | CPU_TX_PORT | DMA_COMMAND`),
  the RANDOM_WRITE reg-pair and BURST_WRITE payload builders, the response seq/`CMD_DONE`
  decode.
- **ISC-4 (eFUSE):** the `MT_EFUSE_CTRL` kick-word, the usage-map physical-size **HARD-FAIL**
  (the blank-clone abort, Trap 2 / T-011, ported faithfully — the confirmed adapter's
  programmed map passes), and the EEPROM field decode yielding ver `0x0d` + MAC
  `xx:xx:xx:xx:xx:xx` (the DISPATCH §2 discriminators).
- **ISC-5 (RX):** the DMA segment walk, the 28-byte RXWI strip, and the 802.11
  beacon/probe-response → BSSID (addr3) / SSID extraction.

The device layer (`mod.rs`) only adds the USB transfers around these cores; it never
re-derives the bytes. This is why the *acquisition* of the bytes matters: every register
constant and struct layout was pulled from the **raw** upstream `mt7601u` source
(torvalds/linux v6.6, GPL-2.0), never a summarizing fetch.

### I-14 fault containment (DISPATCH §3 — restated, no new guard)

The MT7601U is USB, so containment is structural and inherited:
- **(a) lock isolation:** the bulk-completion/event path takes **no** AI-governance lock —
  it reuses the xHCI bounded ISR drain; the driver adds a device-state **leaf** lock at
  `USB_DEVICE` (29, above every AI level), not a new ordered level. The `USB_CORE`=31
  lockdep ceiling is unchanged.
- **(b) liveness:** RX is drained in bounded passes (the 7.5-B `EP_QUEUE_CAP` pattern); a
  firmware/RX event storm degrades USB, never governance.
- **(c) memory isolation:** every DMA address the controller sees is a `FencedDmaAddr` from
  the 7.5-0 arena (no `PhysAddr` in the programming path); xHCI mediates all device DMA and
  already sits in VT-d's fence domain (7.5-V, silicon-proven). The MT7601U firmware (the
  adversary) can only move data through descriptors the driver maps — **no new (c) vector,
  no new guard.** This is precisely why the USB MT7601U target was strictly superior to a
  PCIe `iwlwifi` bus master.

### Blind-hardware discriminators (DISPATCH §4)

Every bring-up stage emits a self-localizing breadcrumb keyed to the DISPATCH §2 known-good
values (ASIC rev `0x76010001`, MAC rev `0x76010500`, EEPROM ver `0x0d`, MAC
`xx:xx:xx:xx:xx:xx`), captured into statics the Stage-B scenario prints, so a failure
self-diagnoses in one run: `stage=` (enumerated → asic-ready → firmware → mcu → efuse →
mac/bbp → phy → mac-start → scanned), the read-back ASIC/MAC revs, the EEPROM ver, the
read-back MAC, and the BSSID count. A `-EINVAL` blank-eFUSE breadcrumb would mean a
*different* (unflashed) unit, not our path — the eFUSE hard-fail is **CLEARED** for the
confirmed adapter.

## Firmware provenance + licensing

`kernel/assets/firmware/mediatek/mt7601u.bin` is the linux-firmware blob (v34, 45,412 bytes,
sha256 `4511b1d840e22aea2bf5fdca419c91c0d94cbfb291b9ac4f8be6d9100d1a7046`). It is third-party,
"Redistributable" (WHENCE), **not** MPL — recorded in `NOTICE`, excluded from the SPDX
first-party header gate.

### Amendment (OT-27a, 2026-06-21) — the blob is no longer committed to git

W1 committed the blob via a `.gitignore` exception and embedded it with `include_bytes!`.
OT-27a reverses both, to the same discipline as the AI model blob (vendor binary + licensing):

- The blob is **untracked** — the `.gitignore` `!kernel/assets/firmware/**/*.bin` exception is
  removed, so the `*.bin` rule keeps it out of git; it stays on disk locally, provided out-of-band.
- The driver no longer `include_bytes!`'s it. `xtask` stages it as a **Limine boot module**
  (`/modules/mt7601u-fw`) **only when present on disk** — CI-safe, since a blob-less checkout
  simply omits the module *and* its injected `limine.conf` line, leaving non-WiFi ISOs untouched.
- The driver looks it up at cold bring-up via `firmware_blob()` (`find_module_by_path_suffix`),
  **fail-closed** with a clear error if the module is absent (a WiFi ISO built without the blob) —
  never a crash. A warm-resident MCU (`COM_REG0==1`) skips the upload and needs no module.
- Consequence: a fresh (blob-less) checkout can now *compile* the WiFi kernel — the blob is needed
  only at ISO-assembly and run time. The sha256 above remains the integrity reference.

## Amendment (OT-39, 2026-07-22) — targeted CAL_BW deferral + warm MCU resync

The 2026-07-21 JOINT home session split the capstone cleanly: Arthur's physical
keyboard → real inference → composited-answer path reached PASS, while WiFi failed
before association. The WiFi captures localized two independent vendor-MCU faults:
the cold first configured-channel tune wedged at inline `MCU_CAL_BW`, and a warm
boot trusted resident firmware whose FCE/MCU queue survived the host reboot.

Prometheus's source-cited trap-mine selected Options B + D(b); Options A/C remain
rejected. At this amendment's merge, `wedge_logic::inline_cal_bw` kept every ordinary
survey hop calibrated, while the first successful targeted operating-channel tune programs RF frequency
plan, EEPROM power, BBP LNA, VCO, BW20 and per-rate TX power without CAL_BW. A
failed tune does not consume the one-shot. Once association is accepted, a bounded
`phy::calibrate_bw` restores TX+RX CAL_BW before the 4-way/data path. Targeted
RF_FREQPLAN, BBP_LNA, skipped CAL_BW and deferred CAL_BW have command breadcrumbs.

A resident-firmware observation now selects `ResyncResident`, never the old direct
return. `fce_preload_setup` (which contains one `vendor_reset` + 5 ms settle)
resets FCE/PBF/USB-DMA state; a bounded 100 ms COM_REG0 ready-poll must then pass
before any MCU command is trusted. Cold firmware keeps the existing out-of-band
upload path. Pre-sink outcomes are retained and re-emitted as `WEDGE-STATE` for the
NVMe evidence region.

Containment is unchanged: **I-14(a)** adds no governance or ordered lock and stays
under the existing `USB_DEVICE` leaf; **I-14(b)** targeted CAL/MCU waits use
`SCAN_BULK_BUDGET` and warm readiness is bounded, so a dead MCU returns an error
instead of stalling governance; **I-14(c)** changes no DMA seam and every buffer
still crosses `dma_arena::fence` (`fence_refusals=0` remains mandatory). The
concurrency-pitfall consult maps CP-1 to the unchanged single leaf, CP-2/3 to no
IRQ-off/cross-CPU wait plus bounded polling, CP-11/17 to N/A (new Relaxed atomics
are diagnostic only), and CP-18 to the single-outstanding synchronous command
path plus resident queue reset. Other CP classes are N/A.

The `mt7601u` kill-switch remains default-off. At this amendment's merge, the
non-targeted survey chose its old inline-CAL behavior. Host KATs prove the policy, while QEMU can prove only
compile/control flow, hermetic DORA logic and the kill-switch—it has no MT7601U
MCU. Physical wedge removal and `LINK UP` remain the separate JOINT OptiPlex run
using `docs/runbook-baremetal-arthur-optiplex.md` and
`docs/runbook-wlan0-dora-optiplex.md`.

## Amendment (scan-path CAL_BW defer, 2026-07-22) — scan and join without a channel

The targeted fix above proved the mechanism but left a shipping crutch: a full
survey still issued CAL_BW on every hop and could irrecoverably wedge around
channels 6-7. Survey `inline_cal_bw` is now always false. Each hop still programs
RF frequency-plan registers 17-20, EEPROM/ALC TX power, BBP LNA, VCO, BW20, and
per-rate TX power; only the wedging TX+RX CAL_BW MCU round-trip is deferred.

Every successful survey hop arms the existing `deferred_cal_pending` state. This
matters when the selected AP is on the final scanned channel: `associate()` is
already tuned and performs no targeted re-tune, but CAL_BW must still run after
the association response and before the 4-way/data path. `CHANNEL=0` is therefore
the configured scenario's shipping contract: scan all channels, match the SSID,
select the observed BSSID/channel, associate, and calibrate on that accepted
operating channel. A non-zero runtime target retains the unchanged targeted fast
path, but is not required for correctness.

The blind-hardware boundary is split explicitly. Every survey hop reports
`path=Survey cal_bw=false` and before/after RF_FREQPLAN, BBP_LNA, VCO, and BW20
breadcrumbs. `channels_wedged>0` is a scan failure in a non-CAL_BW operation;
`channels_wedged=0` plus a missing SSID is scan-RX/AP visibility; an SSID/BSSID
selection followed by auth/assoc failure is an association failure; and a
`POST-ASSOC-CAL` failure is the deferred-calibration boundary. NOTE-A remains
pre-registered: if scan succeeds but auth/assoc fails while uncalibrated, add the
CAL-retry fallback rather than restoring a baked channel.

Containment is unchanged. The scan and post-association waits remain bounded by
`SCAN_BULK_BUDGET` (I-14(b)); no AI-governance lock, new IRQ/atomic/cross-CPU
protocol, or DMA path is added (I-14(a/c)); and `mt7601u` remains default-off.
Host KATs prove survey defer, CHANNEL=0 routing, pending-cal policy, and the
unchanged targeted one-shot. QEMU proves only control flow, compilation, the
hermetic DORA path, and kill-switch; full-sweep RX sensitivity and LINK UP remain
silicon-only under the two OptiPlex runbooks.

## Consequences

- The host-tier slice (ISC 1–4 logic + byte-oracles + the kill-switch) is mergeable now and
  is the durable, verifiable core.
- Stage B (the conclusive scan on the OptiPlex 7080 with the dongle) is the remaining gate;
  the PHY init depth is the most likely Stage-B iteration knob (the RF/BBP/cal port is
  faithful but only silicon proves RX sensitivity). The discriminators make that run
  conclusive whether it passes or fails.
- A consolidation (Rule 4) is due around W1's close (the 4th–5th feature build since BUILD-K).

## Amendment (OT-27a BUILD-2, 2026-06-21) — un-mask + localize the cold-upload MCU-boot wedge

OT-27a BUILD-1's Stage-B (OptiPlex, genuinely cold dongle) proved the cold path runs end-to-end:
firmware loaded from the Limine module, the upload ran, `fence_refusals=0`. But the MCU never
booted — the classifier reported **`McuNoBoot` with a *completed* upload**, while the post-upload
reads were `0xffffffff`. BUILD-2 un-masks and (hypothesis-)fixes it:

- **C1 — all-ones-aware poll (`upload_logic::poll_step`).** `0xffffffff` is the bus-drop sentinel,
  not a value. The BUILD-1 `dma_fw` DMA-done poll (`COM_REG1 & BIT(31)`) **false-passed** on
  all-ones (`0xffffffff & BIT(31) == BIT(31)`), fabricating a "completed" upload over a dead
  device. `poll_step` now returns `Unresponsive` for all-ones (fail-fast at the read), host-tested.
- **C2 — the `DeviceUnresponsive` 5th classifier bucket.** Distinct from `McuNoBoot` (responsive,
  `COM_REG0` honest-0): the device fell off the bus (all-ones read / control "no event"). It
  out-ranks the `EndpointStall`/`McuNoBoot` symptoms it also produces. 5-way truth table host-tested.
- **C3 — per-chunk / per-step breadcrumbs.** `dma_fw` logs each chunk (index/offset/dst/`COM_REG1`),
  the IVB step, and the ready-poll, so a Stage-B failure shows the *last-good chunk before the
  all-ones onset* — the wedge is named, not blind.
- **C4 — the wedge trigger (the real fix).** A byte-diff of our cold-upload sequence against raw
  Linux `mt7601u` `mcu.c`/`regs.h`/`mcu.h`/`dma.h`/`usb.c` found **every register address, value,
  enum (`CPU_TX_PORT=2`, `DMA_PACKET=0`, vendor reqs), chunk size, and the `fce_preload_setup`
  ordering matches Linux exactly** — the wedge is *not* a wrong constant. The one divergence:
  Linux's `mt7601u_vendor_request` **retries every vendor control transfer up to 10× with a 5 ms
  gap** (`usb.c`), and the W1 driver issued each **once**. Fine on the warm path (the device
  answers immediately), but the cold firmware-DMA phase transiently NAKs control transfers, and a
  single un-retried failure returns the all-ones sentinel → the BUILD-1 wedge. Fix: `vend_retry`
  ports Linux's 10×/5 ms retry onto `rr`/`vendor_single_wr`/`vendor_reset`/the IVB upload. Healthy
  (warm) transfers succeed first-try ⇒ no behavior change there.
- **DeviceUnresponsive / I-14 / kill-switch.** No new containment vector — instrumentation + a
  retry on the existing fenced control path (control transfers carry no device-initiated DMA);
  re-assert `fence_refusals=0` on the next Stage-B. Behind `wifi`/`mt7601u`, default-off ⇒
  bit-for-bit today. No new invariant.

## Amendment (Linux parity after Q_SELECT, 2026-07-24)

TEST 3A rejected the OT-39 resident-reset hypothesis. Mainline
`mt7601u_load_firmware` enables USB DMA and returns immediately when `COM_REG0`
reports resident firmware; cold preload, vendor reset, FCE/PBF programming, and
blob upload are exclusive to the cold branch. Neo now follows that behavior.

The post-Q_SELECT order also follows mainline: Q_SELECT completes, exactly one
owned command-response TD is posted, and the fixed packet-RX pool is posted
before the first MAC-init RANDOM_WRITE. `scan()` reuses that pool and cannot
double-post it. Response bytes remain copy-before-rearm and are accepted only
for the owned TD with matching sequence and `CMD_DONE`.

Both MAC tables and the final AUX clock write share one 50,000-unit,
non-replenishing operation budget. Each RANDOM_WRITE chunk reports table,
index/count, command, sequence, payload/frame lengths, and response policy.
Only the final chunk of each table requests a response, matching Linux. Any
bulk, response, budget, or other MAC-init failure emits exactly one
`MAC_INIT_OPERATION_FAILED` terminal with the live budget, response ownership,
packet-RX readiness, and DMA-fence evidence, flushes serial plus the durable
sink, blocks later MCU traffic, and unwinds bind without reset or reload.

The production-consumed `linux_parity_logic` module is also the hostile-oracle
seam. Its negative control reconstructs the rejected resident reset plus late
response/RX ordering. Hardware acceptance remains separate.

**Diagnostic-on-failure (the blind-bring-up base rate):** if the retry is the full fix, the next
cold pass reaches `COM_REG0==1` → scan. If not, C1–C3 make it name the residual wedge (mode A
mid-upload chunk / B post-upload handshake / C honest-`McuNoBoot`-no-all-ones), so the iteration
stays conclusive. C1–C3 are mergeable ahead of C4's hardware confirmation.
