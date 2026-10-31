<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: W2 cold firmware self-upload (OT-27a) + SNonce→CSPRNG (OT-2) → Cerebro

**Date:** 2026-06-21
**From:** PM (Claude)
**To:** Cerebro (W2 lane — landed the BUILD-W2 capstone `4f82155`). New branch off `origin/main` HEAD (`5759dcc` or later): `codex/w2-cold-firmware-snonce-cerebro`.
**Status:** DISPATCHED. **Make WiFi self-sufficient from a cold boot, and make it a real security boundary.** Two bundled W2-lane closes: **(A, OT-27a)** NeoPlutarch uploads the MT7601U firmware *itself* from a cold device — tonight's connect rode Linux-warm-loaded firmware; cold upload still stalls, so "boot NeoPlutarch alone (no Linux) → on WiFi" doesn't work yet. **(B, OT-2)** swap W2's counter SNonce for a CSPRNG draw — until then W2 *associates* but is **not a security boundary**. The recovery prior-art for (A) is already mined (the W2 discriminator THINK P2/P6); 10.1's DRBG + 10.3's kernel fill path make (B) a small swap.

## 0. The decision this build makes
(A) Turn "WiFi works only after Linux pre-loads the firmware" into "**NeoPlutarch cold-boots, uploads the firmware, and associates on its own.**" (B) Turn "W2 associates (counter SNonce, bring-up)" into "**W2's 4-way handshake uses CSPRNG-grade randomness**" — closing OT-2. Both behind the existing `wifi`/`mt7601u` kill-switch (default-off ≡ today).

## 1. References
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-20-w2-mt7601u-discriminator-bank-and-fault-containment.md   # P2 (mt7601u_vendor_reset → reload → poll MT_MCU_COM_REG0==1), P6 (cold-init/upload wedge), the §4 I-14 paragraph (lift verbatim)
git show origin/main:kernel/src/drivers/usb/mt7601u/mod.rs                                            # the driver: configured_target, the supplicant, where firmware load is assumed-warm today
git show origin/main:kernel/src/rng/                                                                  # 10.1 ChaCha20 DRBG + 10.3's kernel fill_random (the SNonce source for B)
git show origin/main:kernel/src/net/wpa/                                                              # gen_snonce (the counter to replace) + the 4-way handshake KATs the new SNonce must still satisfy
# Linux mt7601u firmware-load path (raw source, NOT a summarizing fetch): drivers/net/wireless/mediatek/mt7601u/{mcu.c,usb.c} — the upload sequence + mt7601u_vendor_reset + firmware_running
```
**The trap-mining pass is already done** — the W2 discriminator THINK pre-seeded P2 (the vendor-reset recovery) + P6 (the cold-upload wedge). Build from that bank; pull the exact upload sequence from the raw Linux mt7601u source.

## 2. ISCs — Part A: cold firmware self-upload (OT-27a)
- **ISC-1 — The cold upload sequence + MCU-running confirmation.** From a cold device: upload the MT7601U firmware blob over USB (the control/bulk-transfer sequence per the raw Linux `mcu.c`/`usb.c`), then **`mt7601u_vendor_reset` → `msleep(5)` → poll `firmware_running` (`MT_MCU_COM_REG0 == 1`), ~100×10ms** (the P2 recovery). On stall, retry the vendor-reset (bounded). The MCU-running poll is the success oracle.
- **ISC-2 — Firmware-blob provisioning (NOT in git).** The MT7601U firmware is a vendor blob — **stage it onto the boot image** (NeoFS file or a Limine module) and reference it from the driver; **never commit the blob to git** (binary + licensing — the same discipline as the model). State where it lives + how the driver finds it; fail-closed (missing/corrupt blob → clear error, no crash).
- **ISC-3 — Blind-hardware discriminators (capture-before-you-need-it, from P2/P6).** Log, every cold-boot attempt: the **upload byte-progress** + the **`COM_REG0` poll progression** (each read) + the **chip-id/efuse** (variant check). The run must self-localize: **USB-endpoint-stall** (upload stops mid-transfer) vs **MCU-no-boot** (`COM_REG0` never reaches 1) vs **wrong USB-variant** (chip-id mismatch) vs **blob-missing/corrupt**. A blind failure you can't bucket from these breadcrumbs is a dispatch defect.
- **ISC-4 — Host-testable where possible.** The upload **state machine** (sequence ordering, the `COM_REG0` poll/retry/timeout logic, the control-transfer construction) splits into pure host-tested logic (the `*_logic` discipline); the **actual cold upload is the bare-metal Stage-B** (QEMU has no MT7601U model — the W2 precedent).

## 2b. ISCs — Part B: SNonce → CSPRNG (OT-2)
- **ISC-5 — Swap `gen_snonce`'s counter for the DRBG.** Replace the counter SNonce with a draw from **10.1's ChaCha20 DRBG via 10.3's kernel `fill_random`** (kernel-internal). The `wifi` feature now depends on `kernel-rng` (the DRBG) for the secure path — wire the feature dep; the old counter stays only as an **explicitly-marked insecure bring-up fallback** if `kernel-rng` is off (and W2 is then *documented* as not-a-security-boundary in that config). **KAT (I-12):** the SNonce is now CSPRNG-derived (distinct successive draws, non-counter) **and the 4-way handshake still derives the correct PTK** (the SNonce feeds PTK — re-run the published handshake KATs to confirm a random SNonce doesn't break key derivation; the handshake is SNonce-value-agnostic by construction, so the existing PTK/MIC KATs must still pass). **This closes OT-2.**

## 3. Standards + I-14 + kill-switch (ISC-6)
- **Kill-switch:** both behind the existing `wifi`/`mt7601u` feature (default-off ⇒ bit-for-bit today). No new top-level feature.
- **I-14 (lift from the W2 THINK §4, verbatim):** USB ⇒ no device-initiated DMA, no hardware IRQ (xHCI-poll-driven) ⇒ vectors (a)/(b) collapse; all transfers fenced behind VT-d (c). The cold-upload path adds **no** new containment vector — re-assert `fence_refusals=0` on the Stage-B.
- No new invariant. New `unsafe` only if the upload path needs it (USB transfer buffers — fenced, SAFETY-doc'd). SPDX. `fmt`/`clippy --workspace --all-targets -D warnings` via Docker (kernel clippy blocking). ADR-057 (W2 supplicant) amendment + the OT-27a/OT-2 closure notes.

## 4. Gate
- **Host KATs:** the upload state machine + `COM_REG0` poll/retry logic; the SNonce-is-CSPRNG + the handshake-PTK-still-correct KATs; builds on+off the feature.
- **4-shard QEMU `workflow_dispatch` gate** (for the kernel-internal pieces — the SNonce swap + any non-WiFi regression; the WiFi driver isn't QEMU-modeled). Green, serialized behind the in-flight kernel gates (bundled-model, M2) — **rebase LAST** onto the then-current main.
- **Bare-metal Stage-B (the conclusive run, OT-27a):** on the OptiPlex, **cold boot NeoPlutarch alone — no Linux warm-load** → firmware uploads → `COM_REG0==1` → scan/auth/assoc → `wlan0`. The discriminators (ISC-3) make a failure self-diagnosing. *(This is the PL's hardware run; expect a blind-iteration loop — the bank is what makes each conclusive.)*

## 5. Scope guard
Cold firmware self-upload + the SNonce→CSPRNG swap **only**. **Do NOT** commit a firmware blob (or model) to git, rework the MLME/supplicant state machine (W2, frozen — this adds the cold-upload *prelude* + swaps the SNonce *source*), build GTK/broadcast-RX or DHCP-over-`wlan0` (OT-27b, separate), or touch other drivers. No new invariant; new `unsafe` limited to fenced USB transfer buffers.

## 6. Merge (PM)
**Close-gate: run `scripts/worker-close-gate.sh`** (rebase-freshness — **rebase LAST**, the funnel's hot; the exact CI host-checks; SPDX; self-stat). Two hard rules: green-CI-URL (push-CI + the 4-shard QEMU URL for the kernel pieces — and the **Stage-B sink evidence** for the cold upload, like the W2 capstone's `event=PASS`), and the PM re-inspects your `git show --stat` (**credential/blob discipline: confirm no firmware blob or WiFi secret in the commit** — same scan as W2). PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (**WiFi self-sufficient from cold boot + OT-2 closed: W2 is now a security boundary**). 

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main` and confirmed. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-21-w2-cold-firmware-snonce-cerebro.md` (+ the W2 discriminator THINK for P2/P6 + the §4 I-14 paragraph; the raw Linux `mt7601u/{mcu,usb}.c` for the upload sequence). **Branch:** `codex/w2-cold-firmware-snonce-cerebro` off `origin/main`.
- **One-line:** Make WiFi self-sufficient + a security boundary. **(A, OT-27a) cold firmware self-upload:** from a cold device, upload the MT7601U firmware over USB (raw-Linux `mcu.c`/`usb.c` sequence) → `mt7601u_vendor_reset` → poll `MT_MCU_COM_REG0==1` (P2 recovery); **stage the firmware blob on the boot image, NEVER in git** (model-blob discipline); discriminators (P2/P6): log upload-progress + COM_REG0-poll + chip-id so a cold-boot failure buckets into endpoint-stall / MCU-no-boot / wrong-variant / bad-blob. Upload state-machine = host-tested logic; the cold upload is the **bare-metal Stage-B** (no QEMU MT7601U). **(B, OT-2) SNonce→CSPRNG:** swap `gen_snonce`'s counter for `fill_random` (10.1 DRBG via 10.3's kernel fill); `wifi` pulls `kernel-rng`; KAT the SNonce-is-CSPRNG + that the 4-way handshake still derives the right PTK — **closes OT-2 (W2 becomes a security boundary)**. **Kill-switch: behind `wifi`/`mt7601u` default-off ≡ today; I-14 lifts from the W2 THINK §4 (no new vector, re-assert fence_refusals=0).** **Scope: cold-upload + SNonce-swap ONLY — no blob in git, no MLME rework, no GTK/DHCP (OT-27b separate).** Gate: host KATs + the 4-shard QEMU gate (kernel pieces, rebase LAST behind bundled-model/M2) + the **bare-metal Stage-B cold boot** (NeoPlutarch alone → firmware uploads → associates) with sink evidence. **Close-gate: `scripts/worker-close-gate.sh`; RETURN pastes the green push-CI + QEMU URLs + the Stage-B sink log; PM re-scans for any firmware blob / WiFi secret.**
