<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: MT7601U channel-switch wedge fix (OT-39) — Jarvis

**Date:** 2026-07-22
**Branch:** `codex/mt7601u-channel-switch-fix-jarvis`
**Dispatch:** `Plans/DISPATCH-2026-07-22-mt7601u-channel-switch-fix-jarvis.md`
**Status:** BUILD COMPLETE; host/feature-on gates green; silicon confirmation is
the separately scheduled JOINT step 3.

## PM headline — the home-session split

- **Arthur: PASS.** On the OptiPlex, the Slice-B physical keyboard path reached
  Arthur, ran the real forward pass, composited the answer, and showed PASS. This
  is not implicated in the WiFi failure.
- **WiFi: RED, with two localized faults rather than an unexplained regression.**
  The warm boot trusted resident firmware with a stale MCU/FCE queue and timed out
  in calibration. The cold boot uploaded firmware and passed DPD, then wedged on
  the first configured-channel tune's inline CAL_BW; the bounded recovery found
  the MCU could no longer accept a reload. Prometheus's source-cited trap-mine
  maps these to Options D(b) and B respectively.

## What changed

1. **Fix B — first-targeted-tune CAL_BW deferral.** The ordinary 1→14 survey
   continues to issue CAL_BW on every `set_channel`. Only the first successful
   configured operating-channel tune skips it; RF frequency plan, EEPROM power,
   BBP LNA, VCO, BW20 and per-rate TX power still run. A failed tune does not
   consume the one-shot. After the AP accepts association, TX+RX CAL_BW runs once
   under the fast-fail budget before the 4-way/data path.
2. **Fix D(b) — resident-firmware resync.** `COM_REG0==1` now selects FCE resync,
   not a zero-work return. `fce_preload_setup` performs the vendor reset and
   reprograms FCE/PBF/USB-DMA; a bounded COM_REG0 ready-poll must then pass. The
   45,412-byte firmware image is not re-uploaded on a healthy warm path.
3. **Blind-run breadcrumbs.** Targeted RF_FREQPLAN, BBP_LNA, CAL_BW skip and
   deferred CAL_BW each have before/after lines. Warm resync has before, setup
   result and ready result. Pre-sink state is mirrored later as `WEDGE-STATE` for
   the NVMe evidence region.
4. **Host-testable policy.** `wedge_logic.rs` is consumed by the kernel and mounted
   into `ai-ops-tests`; it proves survey preservation, first-target deferral,
   failure-not-consuming-the-one-shot, saturation, and resident-vs-cold selection.

Options A (boot on target channel) and C (host-CSR frequency programming) were not
attempted; the firmware owns RF regs 17–20.

## I-14 AAG

- **(a) Lock isolation:** no `INFLIGHT`, `RECENCY`, `ARENA`, reservation, `TABLE`,
  or other AI-governance lock is touched. The existing `USB_DEVICE` leaf lock is
  the only driver-state lock. No new lock or lock level.
- **(b) Liveness / degrade-on-wedge:** targeted tune and deferred calibration use
  `SCAN_BULK_BUDGET`; resident readiness uses a bounded 100 ms poll; existing MCU
  response loops are finite. Any timeout returns bind/association failure and
  releases the leaf lock. A dead dongle cannot stall governance or the residency
  daemon.
- **(c) Memory isolation:** no DMA seam changed. All device buffers retain the
  existing `dma_arena::fence` path and VT-d containment. Step 3 must again report
  `fence_refusals=0`.

## Concurrency-pitfall consult

| Class | Result | Guard / reason |
|---|---|---|
| CP-1 lock order | Guard | Existing single `USB_DEVICE` leaf; no nesting change. |
| CP-2 IRQ-off lock / CP-3 waits | Guard | No IRQ disabling or cross-CPU wait; device polls are bounded. |
| CP-11 ordering / CP-17 publication | N/A | New Relaxed atomics are diagnostic breadcrumbs only, not control/data publication. Existing `CONNECTED` Release/Acquire is unchanged. |
| CP-18 single-in-flight device | Guard | One device lock + synchronous command/response; resident FCE/MCU queue reset precedes the first command. |
| Other CP classes | N/A | No scheduler, IPI, wait queue, teardown, allocator, or lock-free reclamation seam changed. |

## Blind-hardware discriminator bank

Both procedures are JOINT and use the authoritative mechanics in
`docs/runbook-baremetal-arthur-optiplex.md` and
`docs/runbook-wlan0-dora-optiplex.md`: PuTTY serial capture at
`C:\Users\<user>\neoplutarch-serial.log`, one-shot `efibootmgr`, and the NVMe sink
at the documented 256 MiB evidence offset. Serial owns pre-sink detail;
`WEDGE-STATE` and all later association/calibration lines are copied into the sink.

| Mode | Ordered signature | Serial + NVMe verdict |
|---|---|---|
| 1. Fix B succeeds | `set_channel(... cal_bw=false)` → `RF_FREQPLAN ... OK` → `BBP_LNA ... OK` → `CAL_BW ... SKIP` → `set_channel ... OK` | First-tune wedge removed; proceed through auth/assoc. `target_tune=2(tuned_ok)` mirrors to sink. |
| 2. Skip works, join still fails | Mode 1 signature, then TX/auth discriminator (`TX_STAT`, `directed_to_us`, auth status, assoc status, 4-way stage) | Separate RF-power/RX/MLME fault; do not mislabel as the channel wedge. |
| 3. Wedge moves to another MCU command | A `before op=RF_FREQPLAN` or `before op=BBP_LNA` with no matching `after`, then bounded failure | CAL_BW specificity insufficient; D(b) or a wider MCU problem remains. `target_tune=3(failed)`. |
| 4. Warm resync succeeds | `WARM-RESYNC before` → `after setup=OK` → `FCE resync applied ready-poll=OK` → later init calibration succeeds | Stale queue flushed without blob upload. `warm_resync=3(ready)` mirrors to sink. |
| 5. Warm resync is partial | Setup/ready says OK, but a later named init-cal MCU command times out; or `setup=FAIL` / `ready-poll=FAIL` | Distinguishes reset/setup failure, firmware-not-ready, and firmware-still-stale. States 4/5 name the early branches. |
| 6. Deferred calibration re-wedges | Association succeeds → `POST-ASSOC-CAL before` with no OK, or explicit `result=FAIL` | Different fault from first tune; consider eliminating CAL_BW rather than deferring. `post_assoc_cal=3(failed)`. |

Auxiliary Option-E discovery remains available: a pre-tune `CHANNEL-TRIPLE ... tx=M`
would reveal the firmware's power-on channel, but it is a test unblocker, not this
fix. The success boundary is still the same-boot ordered WPA2 handshake, DHCP ACK
with assigned `ip=`, `LINK UP`, terminal PASS, and `fence_refusals=0`; terminal
PASS without `LINK UP` is insufficient.

## Gate honesty and evidence

- **Host KAT:** `cargo test -p ai-ops-tests mt7601u_ot39` — 2/2 PASS.
- **Feature-on compile/ISO:**
  `cargo neo-test --build-only mt7601u_assoc_baremetal --bare-metal-mode` — PASS;
  firmware staged at `/modules/mt7601u-fw` and bare-metal ISO assembled.
- **Close gate:** pending final evidence fill after commit/rebase.
- **4-shard workflow_dispatch:** pending final evidence fill after push.
- **Silicon:** intentionally not run in this build. QEMU has no vendor MCU and
  cannot prove the physical wedge is gone. The real proof is step 3: the JOINT
  OptiPlex `mt7601u_assoc_baremetal` run reaching the ordered `LINK UP` boundary.

## Kill-switch / credential audit

`mt7601u` remains default-off, so the driver is absent when disabled. The
non-targeted survey selects its old inline-CAL behavior. `configured_target()`
still contains empty SSID/passphrase/channel placeholders; no credential hunk is
part of this branch.
