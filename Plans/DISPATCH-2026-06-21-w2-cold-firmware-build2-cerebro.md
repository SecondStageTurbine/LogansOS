<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: W2 cold-firmware BUILD-2 — un-mask + localize the MCU boot wedge (OT-27a) → Cerebro

**Date:** 2026-06-21
**From:** PM (Claude)
**To:** Cerebro (W2 lane; ran the OT-27a Stage-B on the OptiPlex). New branch off `origin/main` HEAD (`375d20f`/`d1c8bb6` — carries the OT-27a infra + OT-2): `codex/w2-cold-firmware-build2-cerebro`.
**Status:** DISPATCHED. **The cold-upload infra proved itself on silicon; the MCU doesn't boot the cold-uploaded firmware yet, and the all-ones reads are masking *where* it wedges.** Stage-B (cold dongle, firmware from the Limine module, I-14 `fence_refusals=0`) self-bucketed to **McuNoBoot** with a *completed* upload — but `0xffffffff` reads false-pass `dma_fw`'s poll, hiding the actual wedge point. BUILD-2 un-masks the wedge (C1–C3 instrumentation) and localizes its trigger against the raw Linux sequence (C4) so the next cold pass pinpoints the wedge chunk instead of reporting a blind McuNoBoot.

## 0. The decision this build makes
Turn "the cold firmware upload completes but the MCU doesn't boot, and we can't see *where* because reads return `0xffffffff`" into "**the next cold OptiPlex pass names the exact chunk/step that wedges the device**" — by (a) making the poll predicate treat all-ones as *device-unresponsive → fail-fast* (not a false-pass), (b) adding the `DeviceUnresponsive` bucket, (c) per-chunk/per-step breadcrumbs, and (d) a byte-diff of our upload sequence vs the raw Linux `mt7601u` `mcu.c` to find the wedge trigger. C1–C3 are mechanical de-risking; **C4 is the real investigation** (the root-cause fix is iterative — design it diagnostic-on-failure).

## 1. References
```
git fetch origin
git show origin/main:kernel/src/drivers/usb/mt7601u/mod.rs            # load_firmware / the cold-upload sequence + the dma_fw poll that false-passes on 0xffffffff
git show origin/main:kernel/src/drivers/usb/mt7601u/upload_logic.rs   # the host-tested fault classifier (McuNoBoot/endpoint-stall/wrong-variant/bad-blob) — add DeviceUnresponsive here
git show origin/main:Plans/THINK-2026-06-20-w2-mt7601u-discriminator-bank-and-fault-containment.md  # P2 (vendor_reset→COM_REG0), P6 (cold-init/upload wedge) — the bank this extends
# RAW Linux source (the byte oracle for C4 — NOT a summarizing fetch): drivers/net/wireless/mediatek/mt7601u/mcu.c (esp. the firmware-load/DMA path ~lines 446–489 Cerebro flagged) + usb.c — read the literal register-write order, the chunk size, the inband-command/semaphore handshake, the reset sequencing
```
**The trap is in the upload *sequence*, not the blob** (the blob loaded from the Limine module fine, and the upload completed). Mine the exact Linux ordering — chunk size, the `MT_FCE`/`MT_TX_CPU_FROM_FCE` DMA setup, the inband-command semaphore, the reset/COM_REG0 handshake — and byte-diff ours against it.

## 2. What Stage-B established (do not re-litigate)
- The dongle was **genuinely cold** (no Linux warm-load); firmware loaded from `/modules/mt7601u-fw` (not git); the upload **completed** (full byte-progress).
- The classifier bucketed **McuNoBoot** — `COM_REG0` never reached 1 — **but** the post-upload reads are `0xffffffff`, and `dma_fw`'s poll **false-passes** on all-ones, so the *real* wedge point is hidden (we see "no boot," not "wedged at step N").
- **I-14 held: `fence_refusals=0`** — the cold-upload path stays contained. Re-assert this on the next Stage-B; it is not in question.

## 3. ISCs
- **C1 — Un-mask the wedge: `0xffffffff` ⇒ DeviceUnresponsive → fail-fast (the poll predicate).** Every MMIO/register poll in the cold-upload path (`dma_fw`'s `COM_REG0` poll especially) must treat an **all-ones read as "device fell off / wedged," not as a valid value**. Today an all-ones read either false-passes or is counted as a normal not-ready retry until timeout (reported as McuNoBoot). After C1: an all-ones read short-circuits to a `DeviceUnresponsive` failure at the *point of the read*, so the breadcrumb captures *where* it went unresponsive. **Pure-logic predicate, host-tested** (an all-ones-aware poll predicate: `0xffffffff` ⇒ unresponsive; the real target value ⇒ pass; anything else ⇒ retry-until-timeout).
- **C2 — The `DeviceUnresponsive` bucket (classifier).** Add a fifth bucket to `upload_logic::classify_cold_upload_failure`: **DeviceUnresponsive** (all-ones / bus-drop), distinct from **McuNoBoot** (responsive but `COM_REG0` never 1). Host-test the 5-way classification truth table. This is the discriminator that separates "the chip wedged off the bus" from "the chip is alive but the firmware didn't start."
- **C3 — Per-chunk / per-step breadcrumbs (capture-before-you-need-it).** Log, on every cold-upload attempt: **each DMA chunk index + offset + the post-write status read**, and **each step of the reset/COM_REG0 handshake** with its read value. The next cold pass must show the **last good chunk/step before the all-ones onset** — i.e., pinpoint the wedge chunk. (This is the blind-hardware "capture the data the discriminator needs *before* the run that needs it" rule.)
- **C4 — Localize the wedge trigger: byte-diff vs raw Linux `mcu.c` (the real investigation).** With C1–C3 making the next pass name the wedge chunk, **diff our upload sequence against the raw Linux `mt7601u` `mcu.c`/`usb.c`** at and around that point — chunk size, register-write order, the FCE/DMA-from-CPU setup, the inband-command semaphore wait, the reset sequencing. The **fix** is whatever ordering/handshake we diverge on that wedges the MCU. **Design this diagnostic-on-failure** (the next pass after the fix may reveal the *next* layer — the blind-bring-up base rate); state, per the failure-mode section below, what each next-pass outcome would mean.

## 4. Expected-failure-mode analysis + discriminators (blind-hardware standing rule)
The next cold OptiPlex pass after C1–C3 (and a C4 hypothesis fix) is still blind — design it conclusive whether it passes or fails:
- **Mode A — wedges mid-upload at a specific chunk.** Discriminator: C3's per-chunk breadcrumb shows the last good chunk before all-ones → C1 fires `DeviceUnresponsive` at chunk N. Maps to: a chunk-size / DMA-pacing divergence from Linux (C4 diff at that chunk).
- **Mode B — upload completes, then wedges at the reset/COM_REG0 handshake.** Discriminator: C3's per-step handshake log + `DeviceUnresponsive` at the reset step. Maps to: a reset-sequence/semaphore divergence (C4 diff in the post-upload handshake).
- **Mode C — responsive but `COM_REG0` never reaches 1 (true McuNoBoot, no all-ones).** Discriminator: reads are sane (not all-ones) but never 1 → the firmware started loading but didn't run. Maps to: a firmware-entry / FCE-DMA-setup divergence.
- **Mode D — passes** (COM_REG0==1 → scan/auth/assoc → `wlan0`). The OT-27a close.
A next-pass result that doesn't fall cleanly into A/B/C/D is a dispatch defect — C2+C3 are sized so it does.

## 5. I-14 fault-containment (connectivity/driver standing rule)
USB ⇒ **no device-initiated DMA to governance, no hardware IRQ** (xHCI-poll-driven) ⇒ vectors (a)/(b) collapse; all cold-upload transfers stay in the **fenced arena behind VT-d** (c). BUILD-2 adds **no new containment vector** — it's instrumentation + a sequence fix on the existing fenced path. **Re-assert `fence_refusals=0` on the Stage-B** (it held in BUILD-1; it must keep holding). Lift the I-14 paragraph from the W2 discriminator THINK §4.

## 6. Standards + kill-switch
Behind the existing `wifi`/`mt7601u` feature, **default-off ⇒ bit-for-bit today** (no new top-level feature). C1's all-ones-aware predicate + C2's bucket are **pure-logic, host-tested**; C3 is instrumentation; C4 is the sequence fix. SPDX on any new file. `fmt`/`clippy --workspace --all-targets -D warnings` via Docker (kernel clippy blocking). No new invariant. New `unsafe` only if a transfer-buffer change needs it (fenced, SAFETY-doc'd). ADR-054 amendment (the cold-upload BUILD-2 findings + the DeviceUnresponsive bucket).

## 7. Gate
- **Host KATs:** the all-ones-aware poll predicate (`0xffffffff`⇒unresponsive, target⇒pass, else⇒retry); the 5-way classifier truth table (incl. DeviceUnresponsive); the per-chunk/per-step breadcrumb formatting; builds on+off `wifi`.
- **4-shard QEMU `workflow_dispatch`** (kernel-touching — the driver + predicate): green, no non-WiFi regression (the WiFi driver isn't QEMU-modeled; this proves the kernel pieces don't regress). **Serialize behind any in-flight kernel gate; rebase LAST.**
- **Bare-metal Stage-B (the conclusive run, the PL+Cerebro hardware loop):** cold boot → upload → the breadcrumbs **name the wedge chunk/step** (C1–C3) → with the C4 fix, drive toward `COM_REG0==1` → scan/auth/assoc → `wlan0`, NVMe sink evidence, `fence_refusals=0`. Expect a blind-iteration loop; the discriminators make each pass conclusive.

## 8. Scope guard
The cold-upload **un-masking + localization + the wedge-trigger fix** only. **Do NOT** rework the MLME/supplicant (frozen — this is the cold-upload *prelude*), build GTK/broadcast-RX or DHCP-over-`wlan0` (OT-27b, separate), touch the SNonce (OT-2 closed), or commit a firmware blob to git. New `unsafe` limited to fenced transfer buffers. No new invariant.

## 9. Merge (PM)
**Close-gate: `scripts/worker-close-gate.sh`** (rebase-freshness — rebase LAST; the exact CI host-checks; SPDX; self-stat). Two hard rules: green-CI-URL (push-CI + the 4-shard QEMU `workflow_dispatch` URL for the kernel pieces — and the **Stage-B sink evidence** for the cold pass: the wedge-chunk breadcrumb on failure, or `event=PASS` on success), and the PM re-inspects your `git show --stat` (**no firmware blob / WiFi secret** — the W2 scan). PM independently re-verifies `head_sha` on every cited CI run. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger. **Note:** C1–C3 (instrumentation) are mergeable as soon as they gate green even if C4's root-cause fix is still iterating on hardware — landing the un-masking/breadcrumbs first makes every subsequent cold pass conclusive.

**Disk reclaim (after PM confirms merge):** keep the `-w2` tree (Cerebro's reusable W2/Stage-B build tree); `cargo clean` if reclaiming space without losing it.

---
## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-21-w2-cold-firmware-build2-cerebro.md` (+ `mt7601u/mod.rs` + `upload_logic.rs` + the W2 discriminator THINK §4; the raw Linux `mt7601u/mcu.c` ~446–489 for C4). **Branch:** `codex/w2-cold-firmware-build2-cerebro` off `origin/main` (`14b08bb`'s code is already on main as `d1c8bb6` — do NOT re-merge it).
- **One-line:** Un-mask + localize the cold-firmware MCU boot wedge. **C1:** make every cold-upload poll treat `0xffffffff` as **DeviceUnresponsive → fail-fast** (today `dma_fw`'s poll false-passes on all-ones, hiding the wedge) — pure-logic, host-tested predicate. **C2:** add the **DeviceUnresponsive** 5th bucket to `upload_logic` (all-ones/bus-drop, distinct from McuNoBoot) — host-test the truth table. **C3:** **per-chunk + per-handshake-step breadcrumbs** so the next cold pass names the last-good chunk before the all-ones onset (capture-before-you-need-it). **C4 (the real investigation):** byte-diff our upload sequence vs **raw Linux `mcu.c`** (chunk size / FCE-DMA-from-CPU setup / inband-command semaphore / reset+COM_REG0 ordering) at the wedge point → fix the divergence; design it **diagnostic-on-failure** (modes A=mid-upload-chunk-wedge / B=post-upload-handshake-wedge / C=true-McuNoBoot-no-all-ones / D=pass). **I-14:** no new vector, re-assert `fence_refusals=0` on Stage-B. **Kill-switch:** `wifi`/`mt7601u` default-off ≡ today. **Scope: cold-upload un-mask + localize + sequence fix ONLY — no MLME rework, no GTK/DHCP, no SNonce (OT-2 closed), no blob in git.** **C1–C3 are GREEN-LIT to start now (mechanical, de-risk the next cold pass) + start the C4 Linux diff; they're mergeable ahead of C4's hardware iteration.** Gate: host KATs + 4-shard QEMU (rebase LAST) + the bare-metal Stage-B (the breadcrumbs name the wedge chunk). **Close-gate: `scripts/worker-close-gate.sh`; RETURN pastes the green push-CI + QEMU URLs + the Stage-B breadcrumb/sink log.**
