<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7.5 BUILD-W1 — the MT7601U USB SoftMAC driver (scan) → Jarvis

**Date:** 2026-06-14
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` HEAD: `codex/phase-7.5-build-W1-mt7601u-driver-jarvis`) — **start now, in parallel with CI #171; the bare-metal connectivity lane is yours (7.5-A/B/V).** CI #171 runs unattended on the WSL2 runner; W1's Stage B is the separate OptiPlex; the host-tier ISCs need neither. If #171 returns red, kick it then resume W1.
**Status:** DISPATCHED. **The dongle is in hand and the last gate is cleared.** This promotes the dispatch-ready THINK to a build: bring up the **MT7601U USB SoftMAC driver up to and including scan** on the OptiPlex 7080 — enumerate the `148f:7601` adapter on the 7.5-A/B USB core, load firmware, read eFUSE, init MAC/PHY, and **list real 2.4 GHz BSSIDs on bare metal.**
**This is a blind-hardware + connectivity dispatch** — the I-14 fault-containment analysis (§3), the failure-mode/discriminator section (§4), and the pre-seeded trap bank (§5) are all present per the standing rules.

## 1. Authoritative references (on `main` — read them first; your tree is on a feature branch)
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-14-phase-7.5-build-W1-mt7601u-driver.md            # the governance design + ISCs
git show origin/main:Plans/RESEARCH-2026-06-14-mt7601u-bringup-trap-mine-FINDINGS.md        # §1 bring-up sequence (file:line), §2 the 27-trap bank
git show origin/main:Plans/THINK-2026-06-13-phase-7.5-wifi-arc-mt7601u-supplicant.md         # the W-slice (W1 driver / W2 supplicant)
```
The FINDINGS is the **authoritative bring-up reference** (file:line-cited from the raw `mt7601u` source); this dispatch's ISCs are built on it — do not re-derive the sequence.

## 2. The confirmed adapter — your known-good discriminator reference
The dongle is physically confirmed (`lsusb`/`iw` on a Linux reference machine): **USB ID `148f:7601`, Ralink MT7601U, 802.11n 2.4 GHz-only (single-band).** A **properly-flashed** unit — these are the values W1's bring-up must reproduce on bare metal (turn each into a self-localizing breadcrumb):

| Stage | Expected on this adapter (the breadcrumb target) |
|---|---|
| Firmware load | firmware `0.1.00 Build 7640` (2013-02-05); **ASIC rev `0x76010001`**, **MAC rev `0x76010500`** |
| eFUSE/EEPROM read | **EEPROM ver `0x0d`, fae `0x00`**; **MAC `xx:xx:xx:xx:xx:xx`** |
| Scan | real 2.4 GHz BSSIDs (the AP you're near) |

**⚠ eFUSE HARD-FAIL — CLEARED for THIS adapter (the trap-mine Trap 2 reconciliation).** Trap 2: a blank-eFUSE clone (usage map < 5 words) aborts init `-EINVAL`. This unit reports a **programmed MAC (`xx:xx:xx:xx:xx:xx`) + valid EEPROM ver `0x0d`** ⇒ the usage map is populated ⇒ it passes the check. So the **port-the-hard-fail** path (ISC-4) is correct here — no default-cal blob needed; if the read yields MAC `xx:xx:xx:xx:xx:xx`, the eFUSE path is provably correct, and a `-EINVAL`/blank-eFUSE breadcrumb would mean a *different* (unflashed) unit, not our path.

## 3. I-14 governance fault-containment analysis (the standing rule — satisfied by USB, no new guard)
A one-line confirmation, per the arc THINK §3 and the W1 THINK §2 (do not omit it — restate in the RETURN):
- **(a) lock isolation:** the W1 bulk-completion/event path takes **no** AI-governance lock — reuse 7.5-A/B's bounded ISR drain; W1 adds a **device-state leaf lock only** (no new ordered level; the `USB_CORE`=31 lockdep ceiling is unchanged). Grep-assert + the runtime no-AI-lock-in-IRQ assertion.
- **(b) liveness:** RX/event flood capped per service pass (the 7.5-B `EP_QUEUE_CAP` pattern, reused).
- **(c) memory isolation:** **handled by USB itself** — the MT7601U firmware (the adversary) moves data only through transfer descriptors the driver maps; xHCI mediates all DMA and already sits in VT-d's fence domain (7.5-V, silicon-proven). **No new (c) vector, no new guard.**

## 4. Blind-hardware failure-mode + discriminator framework (the standing rule)
W1 is bare-metal blind bring-up — the AX88179 day is the cautionary case. **Every run must be conclusive whether it passes OR fails.** Each stage emits a self-localizing breadcrumb keyed to §2's known-good values:
| Stage | PASS breadcrumb | The distinct FAILURE breadcrumbs |
|---|---|---|
| Enumerate | `148f:7601` claimed, **EP1 OUT TX / EP2 IN RX / EP3 BULK OUT** found | no-claim (class/interface) vs wrong-EP-layout |
| Firmware load | header-OK → 14,336 B chunks via FCE-DMA (vendor req `0x42`) acked → ASIC rev `0x76010001` | `-ENOENT` (fw missing) vs upload-DMA `-ETIMEDOUT` vs FW-ready-poll `-ETIMEDOUT` (`MT_MAC_CSR0`, 1 ms) — three distinct paths (FINDINGS §1.4) |
| eFUSE read | EEPROM ver `0x0d` + MAC `xx:xx:xx:xx:xx:xx` | `MT_EFUSE_CTRL` no-valid-data / usage-map<5 ⇒ `-EINVAL` (a *different*, unflashed unit — not expected here) |
| MAC/PHY init | RF (`MT_RF_CSR_CFG`, 1 ms) + BBP soak-loops converge | which soak-loop timed out (log the register address) |
| Scan | ≥1 real 2.4 GHz BSSID | reg-lockout (too few channels) vs no-AP-in-range vs RX-path-dead — distinct |

**Capture-before-you-need-it:** the MCU/firmware fault codes to log from boot are enumerated in FINDINGS §1.4/§1.5; capture the ASIC/MAC rev + EEPROM ver + the read-back MAC so a wrong value is self-diagnosing against §2. Do not discover a missing capture on the failing run.

## 5. Trap bank — VERIFIED (FINDINGS §2; carry it, don't re-derive)
The 7 starter traps verified/corrected + 20 raw-source traps (T-001…T-020) are in FINDINGS §2, each with a discriminator. The load-bearing corrections (so they're not lost): **firmware chunks = 14,336 B (`MCU_FW_URB_MAX_PAYLOAD`), FCE-DMA via vendor req `0x42`, no header magic; EP3 is BULK OUT (not interrupt); MCU in-band ≤192 B; eFUSE hard-fail (cleared §2); 2.4 GHz-only.** Pre-seed the failure bank from these on day zero.

## 6. ISCs (from the W1 THINK §6; bodies finalized against FINDINGS §1)
- **ISC-1 — Enumerate + claim** the vendor-class interface on the 7.5-A/B core; identify the 3 bulk EPs (EP1 OUT TX / EP2 IN RX / **EP3 BULK OUT**); register the MT7601U class driver (the 7.5-B seam).
- **ISC-2 — Firmware load:** header parse + chunked FCE-DMA upload (14,336 B, vendor req `0x42`) + ready-poll; assert ASIC rev `0x76010001`.
- **ISC-3 — MCU command interface:** `MT_H2M`/`MT_M2H` over control-OUT + the response path (≤192 B in-band).
- **ISC-4 — eFUSE/EEPROM read + MAC/PHY init + calibration:** **port the hard-fail** (this adapter passes it; §2); assert EEPROM ver `0x0d` + MAC `xx:xx:xx:xx:xx:xx`; RF/BBP init (FINDINGS §1.8).
- **ISC-5 — Bulk data path + scan:** RX (TXWI/RXWI handling, FINDINGS §1.9), receive beacons/probe-responses, **enumerate real BSSIDs** — the gate output.
- **ISC-6 — Standards:** the `mt7601u`/`wifi` feature (default-off ⇒ bit-for-bit today — the kill-switch); `unsafe` is the USB-transfer/MMIO idiom (justified, like 7.5-A/B); open **ANSPEC §10.10** (the MT7601U driver — confirm next free §10.x) + **ADR-054**; restate the §3 I-14 confirmation.

## 7. Gate
- **Stage A (host — no QEMU model for the MT7601U):** host-testable logic as byte-oracles (I-12) — the `mt7601u_fw_header`/firmware-chunk math, the MCU framing, the eFUSE-field decode (assert it yields ver `0x0d` + the expected MAC from a captured EEPROM image if available), the TXWI/RXWI descriptor layout; the existing family green with the feature off. State honestly that QEMU can't exercise the radio.
- **Stage B (OptiPlex 7080 blind — the conclusive gate):** dongle in the OptiPlex; the driver enumerates → loads firmware (ASIC `0x76010001`) → reads eFUSE (MAC `xx:xx:xx:xx:xx:xx`) → inits MAC/PHY → **scans and lists real 2.4 GHz BSSIDs**, each stage emitting its §4 breadcrumb so any failure self-localizes in one run.

## 8. Scope guard
W1 = the driver up to **scan** only. **Do NOT** build the MLME auth/assoc/4-way state machine, the `NetDevice` glue, or CCMP on-air (that's W2, driving W0/W0b's proven crypto); **do NOT** add AP/mesh/P2P/roaming. **Do NOT** grant the driver unfenced DMA (every buffer is a `FencedDmaAddr`, per 7.5-A/B). Keep the surface off the AI hot path. If the dongle isn't in the OptiPlex (Stage B can't run), the host-tier ISCs (1–4 logic, byte-oracles) still land; BLOCKER-to-PM on the hardware gate rather than faking it.

## 9. Merge (PM)
Close-gate: rebase onto current `main`, re-run the host family + the Stage-B scan evidence, push. PM `pm-merge-check` → `--ff-only` → `main`; ledger Article 13 + ANSPEC §10.10 + ADR-054. With W1, **we can see the air on real silicon** — and only **W2** (the MLME + on-air supplicant wiring W0/W0b's proven crypto to W1's frames) remains to *join* it.

---

## Relay — what to tell Jarvis
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-14-phase-7.5-build-W1-mt7601u-driver-jarvis.md` (+ the THINK + FINDINGS, §1 read-commands above).
- **Branch:** `codex/phase-7.5-build-W1-mt7601u-driver-jarvis` off `origin/main` HEAD. **Start now — in parallel with CI #171** (it runs unattended on the WSL2 runner; W1's bare-metal Stage B is the separate OptiPlex). Work W1 on its own branch (worktree-isolated from any CI branch).
- **One-line:** Bring up the **MT7601U USB SoftMAC driver to scan** on the OptiPlex. Dongle in hand (`148f:7601`, properly-flashed: **ASIC `0x76010001`, MAC rev `0x76010500`, fw Build 7640, EEPROM ver `0x0d`, MAC `xx:xx:xx:xx:xx:xx`** — these are your known-good discriminator targets; the eFUSE hard-fail is **CLEARED** for this unit, so port-the-hard-fail). Ride the 7.5-A/B USB core: enumerate (EP1 TX / EP2 RX / **EP3 BULK OUT**) → firmware load (14,336 B FCE-DMA chunks, vendor req `0x42`) → eFUSE read → MAC/PHY init → **scan real 2.4 GHz BSSIDs** (the gate). Bring-up sequence + the verified 27-trap bank are in FINDINGS §1/§2 — carry them, don't re-derive. I-14 USB-contained (no new guard); every buffer a `FencedDmaAddr`; feature default-off kill-switch. Scan-only (W2 is the on-air supplicant). Open ANSPEC §10.10 + ADR-054. **This is the WiFi arc meeting silicon.**
