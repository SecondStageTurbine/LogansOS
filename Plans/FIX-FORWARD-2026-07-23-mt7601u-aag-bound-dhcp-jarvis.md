<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# FIX-FORWARD: MT7601U post-FW total bound + truthful LINK-UP terminal → Jarvis

**Date:** 2026-07-23  
**From:** PM  
**To:** Jarvis  
**Priority:** Release-blocking TEST-3 close; build now, runner token controlled by PM/PL  
**Rejected merge/silicon-close candidate:** `f73ee30073c20b4f4c9e8978ee304985ca044dad`  
**Verified candidate gate:** `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30063259618` — exact SHA, all four shards green  
**Verdict:** **AAG BLOCK — do not merge and do not treat the existing ISO as the final two-boot close artifact**

## Read first

```powershell
git fetch origin
git show origin/main:Plans/FIX-FORWARD-2026-07-23-mt7601u-aag-bound-dhcp-jarvis.md
git show origin/main:Plans/DISPATCH-2026-07-23-mt7601u-warm-postfw-fix-jarvis.md
git show origin/main:Plans/RESEARCH-2026-07-23-mt7601u-warm-postfw-trapmine-FINDINGS.md
git show origin/main:docs/runbook-wlan0-dora-optiplex.md
```

The AAG independently verified the candidate's ancestry/stat, exact-SHA push and four-shard gates, all-ones oracle, ISO hash/manifest, credential hygiene, state replay, ordering, and no-new-DMA/lock/IRQ properties. It is a useful diagnostic step, but two completeness defects remain before the hardware close.

## A. P0/P1 — the post-FW bound is per wait, not per operation

The rejected candidate's stated dead-device bound is incomplete:

- `post_fw_wpdma_poll` permits 10,001 register observations;
- each `rr()` permits ten vendor attempts;
- the reduced `POST_FW_WAIT_BUDGET` applies to one `wait_event`;
- one xHCI `control_in` can call `wait_event` up to eight times while consuming valid but non-status/late transfer events.

Evidence in `f73ee300…`:

- `kernel/src/drivers/usb/mt7601u/mod.rs:109-122` — ten-attempt `vend_retry`
- `kernel/src/drivers/usb/mt7601u/mod.rs:800-821` — one `rr()` composes the retry loop
- `kernel/src/drivers/usb/mt7601u/mod.rs:910-925` — 10,001-observation WPDMA outer loop
- `kernel/src/drivers/usb/xhci.rs:1489-1566` — `control_in` tolerates up to eight event waits

The all-attempts-dead case now terminates after one logical read, which is good. But an intermittent/late-event path can consume multiple full waits, eventually return a responsive busy value, and reset the whole nested composition on the next outer observation. The structural ceiling is up to:

```text
10,001 WPDMA observations
× 10 vendor attempts
× 8 wait_event calls
× 50,000 wait-loop iterations
+ retry gaps and the outer polling gaps
```

That is the same multi-minute class the original dispatch required eliminating. A reduced inner budget is not an operation-wide elapsed bound.

### Required repair

- Give the first post-firmware operation one total budget spanning every xHCI event wait, vendor retry, and WPDMA observation. A partial response, late event, or busy value must not replenish the total.
- Carry the same explicit total-bound discipline across ASIC-ready, CSR/BBP reset, USB-DMA init, and MCU Q-select. Each boundary must finish as success or one terminal discriminator.
- Preserve all-ones precedence and the Linux-shaped retry behavior within the total ceiling.
- RAII-restore the prior xHCI budget on every return.
- Record enough counters/state in the breadcrumb to distinguish:
  - no transfer event;
  - tolerated non-status/late events exhausted;
  - vendor retries exhausted;
  - responsive WPDMA busy until the operation budget expired.
- Replace the findings document's one-read formula with the complete nested bound. State it in iterations/call counts, not human-time estimates.

### Required oracle

Add a scripted host model that drives the real decision/budget core through at least:

1. immediate idle;
2. immediate all-ones;
3. persistent responsive busy;
4. repeated missing events;
5. seven tolerated late/non-status events followed by status;
6. late events followed by attempt failure, later retry success, then busy;
7. alternating failure/busy that would reset the rejected candidate's outer composition.

For every row assert the exact terminal state, exact or bounded wait/retry/observation count, and that no path can exceed the operation-wide ceiling. Include a negative control that demonstrates the rejected nested policy exceeds that ceiling.

## B. P0 — terminal PASS is possible without DHCP LINK-UP

`run_configured_association()` logs a bounded DHCP timeout but returns normally. The bare-metal harness then gates terminal PASS only on `is_bound()` plus `is_connected()`. It can therefore emit `NEO_TEST PASS` after association even when DORA failed and no IP/LINK-UP exists.

Evidence:

- `kernel/src/drivers/usb/mt7601u/mod.rs:3717-3735` — DHCP ACK versus timeout is logged but not returned as success/failure
- `kernel/src/test_harness.rs:11864-11874` — PASS checks bind + association only

The dispatch and runbook require DHCP ACK, assigned IP, and `LINK UP`. Human log review must not be the only thing preventing a false terminal PASS.

### Required repair

- Make the association result expose a durable DHCP terminal state and assigned IP.
- `mt7601u_assoc_baremetal` may PASS only when bind, association, WPA/GTK, DHCP ACK, assigned nonzero IP, and `LINK UP` are all proven.
- DHCP timeout must emit terminal `NEO_TEST FAIL` with the DORA state/xid discriminator after flushing it to the sink.
- Replay DHCP state/IP in the final sink-backed summary.
- Keep the DORA loop bounded and preserve the device-free DORA KAT.

### Required oracle

- ACK fixture → assigned expected IP → LINK-UP → PASS eligible.
- No-offer and offer-without-ACK fixtures → exact timeout discriminator → PASS ineligible.
- Negative control: association/`is_connected=true` with no DHCP lease must red.

## Cold-path claim — correct the wording

Cold firmware upload and its upload budget were unchanged in `f73ee300…`; the shared post-firmware control path was not. The reduced wait guard and WPDMA classifier apply after both cold upload and resident-firmware detection. State that distinction explicitly. The genuinely-cold silicon boot remains a load-bearing regression gate.

## Required close gate and RETURN

1. Rebase onto current `origin/main`; keep both credential placeholders `b""` in every commit and remote branch state.
2. Limit the code change to the operation-wide bound/accounting, truthful DHCP/LINK-UP result, their host oracles, sink terminal data, and associated findings/runbook text.
3. Run host tests, format, workspace clippy, blocking kernel clippy, SPDX, and `scripts/worker-close-gate.sh`.
4. Push the exact repair SHA and return the push CI URL. Push CI is not the QEMU gate.
5. Do not fire the four-shard `workflow_dispatch` until the PM/PL issues a new exact-SHA runner token.
6. Build a new `mt7601u_assoc_baremetal` ISO from the final repair SHA with the verified 45,412-byte firmware and local credentials; return size, SHA-256, manifest, sink offset/size, and placeholder-restoration proof.
7. Return a blocker→repair table, corrected nested bound, scripted-oracle results, own-stat, close-gate output, exact-SHA CI URLs, and deletable worktree.

No merge. PM repeats own-stat, exact-SHA CI verification, and the AAG before authorizing the two physical boots.

## Silicon close after the repaired candidate passes AAG

Both boots use the same exact repaired ISO, fresh serial, and a freshly cleared 16 MiB sink:

1. **Genuinely cold dongle:** physically depower first. Require cold 45,412-byte firmware upload, `post_fw=control_plane_ready`, EEPROM/calibration, full `CHANNEL=0` survey with `cal_bw=false`, configured SSID found on its discovered channel, auth/association, post-association CAL OK, WPA/GTK, DHCP ACK with assigned IP, `LINK UP`, and terminal PASS. Serial and sink must agree; `fence_refusals=0`.
2. **Immediate warm reboot:** leave the dongle connected and powered; no unplug or power-cycle. Require resident-firmware/upload-skipped and warm-resync evidence, then the same complete LINK-UP chain and terminal PASS.

Any bounded post-FW fail-fast result is conclusive RED, not success. Return exactly one terminal category: WPDMA all-ones, WPDMA busy/operation-budget, ASIC, CSR/BBP, USB-DMA, Q-select, scan, association/NOTE-A, WPA, or DHCP. Do not rerun an identical warm state blindly. If a second device reset is required, dispatch the safe xHCI reset→re-address→restore-configuration/re-enumeration unit or adopt an explicit physical-power policy; do not approximate it with endpoint recovery.
