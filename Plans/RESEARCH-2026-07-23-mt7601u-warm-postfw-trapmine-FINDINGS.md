<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# FINDINGS — MT7601U warm post-firmware control-plane trap mine

**Date:** 2026-07-24
**Owner:** Jarvis
**Scope:** Independent derivation for the TEST-3 stall after `firmware running`
and the AAG operation-bound/DHCP fix-forward.

The PM diagnostic draft was treated only as untrusted evidence and was neither
merged nor cherry-picked. The first Jarvis candidate was useful diagnostic
work, but its per-wait ceiling and association-only terminal predicate were not
complete acceptance bounds.

## Observed localization

TEST 3 completed enumeration, resident-firmware FCE/vendor resync, and the
`COM_REG0 == 1` ready poll. It emitted `firmware running` and no later bytes.
In `bring_up()`, the next operation is the post-firmware `WPDMA_GLO_CFG` idle
poll.

That identifies the first unobserved boundary; it does not prove whether the
first vendor read returned all ones, returned a persistent busy value, consumed
late/non-status events, or failed later. The repair retains distinct counters
and terminal states rather than assuming one.

## Upstream sequence and reset semantics

Linux v6.6 performs these relevant steps:

1. `mt7601u_probe()` calls `usb_reset_device()` before assigning pipes or
   performing the first ASIC/vendor reads.
2. `mt7601u_init_hardware()` enables WLAN, checks ASIC readiness, calls
   `mt7601u_mcu_init()`, polls `WPDMA_GLO_CFG` idle, checks ASIC readiness
   again, resets CSR/BBP, initializes USB DMA, then performs MCU `Q_SELECT`.
3. `mt7601u_load_firmware()` returns early when `COM_REG0 == 1`; it does not
   upload over a running MCU. The FCE/vendor-reset sequence is the cold-upload
   preparation path.
4. `mt7601u_vendor_request()` performs at most ten control transfers, with a
   retry gap after each failed non-terminal attempt.

Primary references:

- Linux v6.6 `main.c` (`mt7601u_probe`):
  <https://github.com/torvalds/linux/blob/v6.6/drivers/net/wireless/mediatek/mt7601u/main.c>
- Linux v6.6 `init.c` (`mt7601u_init_hardware`):
  <https://github.com/torvalds/linux/blob/v6.6/drivers/net/wireless/mediatek/mt7601u/init.c>
- Linux v6.6 `mcu.c` (`mt7601u_load_firmware`, `mt7601u_mcu_cmd_init`):
  <https://github.com/torvalds/linux/blob/v6.6/drivers/net/wireless/mediatek/mt7601u/mcu.c>
- Linux v6.6 `usb.c` (`mt7601u_vendor_request`):
  <https://github.com/torvalds/linux/blob/v6.6/drivers/net/wireless/mediatek/mt7601u/usb.c>

Our xHCI enumerator already applies `PORTSC.PR` to this USB2 root port before
addressing the device on every boot. Linux adds a second device reset at probe.
Our claimed-device API has no atomic reset + re-address +
restore-configuration transaction. A bare port reset after claim would
invalidate the address and endpoints, so the driver must not approximate it
with endpoint recovery. If silicon proves a second reset necessary, the next
unit is safe xHCI re-enumeration or an explicit physical-power policy.

## Nested-wait trap and corrected operation-wide bound

`rr()` composes the WPDMA observation loop, up to ten vendor attempts per
observation, and xHCI transfer-event waits. One `control_in` can call
`wait_event` up to eight times while tolerating valid late/non-status events.

The rejected candidate lowered one `wait_event` ceiling but replenished that
ceiling at every layer. Its structural ceiling remained:

```text
10,001 WPDMA observations
× 10 vendor attempts
× 8 wait_event calls
× 50,000 wait-loop iterations
+ retry gaps and outer poll gaps
```

The fix-forward uses one 50,000-unit ledger per post-firmware boundary. A unit
is charged for every event-wait call, event-wait iteration, vendor attempt,
vendor retry gap, register observation, register poll gap, bulk response
attempt, or bulk poll. No nested layer can replenish it:

```text
wait_calls + wait_iterations
+ vendor_attempts + vendor_retry_gaps
+ observations + poll_gaps
+ bulk_attempts + bulk_polls
<= 50,000 charged units per boundary
```

The Linux-shaped inner limits remain: at most ten vendor attempts per request,
at most eight tolerated transfer events per control request, and at most
10,001 WPDMA observations. They terminate earlier when their combined charges
reach the shared ceiling.

The RAII ledger is applied separately to WPDMA, ASIC-ready, CSR/BBP reset,
USB-DMA init, and MCU Q-select. Retained counters distinguish:

- no transfer event (`no_event_exhaustions`);
- tolerated late events and eight-event desynchronization;
- ten-attempt vendor exhaustion;
- responsive WPDMA busy until the operation budget is exhausted.

`0xffff_ffff` still precedes mask evaluation. It is the failed-control-read
sentinel, never a legitimate WPDMA value. A completed ten-attempt read that
returns it terminates device-unresponsive; a ledger exhausted before that
composition finishes terminates operation-budget.

## Recovery decision

- EP0/control read succeeds: classify WPDMA idle versus busy.
- Vendor retries complete with all-ones: WPDMA device-unresponsive.
- The shared ledger expires first: fail the current named boundary as
  operation-budget.
- Responsive WPDMA remains busy: fail WPDMA busy/operation-budget distinctly.
- A later boundary fails: retain ASIC, CSR/BBP, USB-DMA, or Q-select as the
  exact terminal category with its counter bank.
- Port reset/power-cycle is not silently performed. A required second device
  reset remains a separately dispatched hardware/API unit.

## DHCP/LINK-UP terminal truth

The rejected candidate returned normally after bounded DHCP timeout, while the
bare-metal harness gated PASS only on bind plus association. That admitted a
false PASS with no lease.

The fix-forward retains DHCP terminal state, XID, assigned IP, GTK state, and a
derived LINK-UP bit for sink replay. PASS eligibility is the conjunction of:

```text
bound
&& associated
&& GTK installed
&& DHCP state == Bound
&& assigned IPv4 != 0.0.0.0
&& LINK-UP
```

Selecting timeout (`no Offer`) and Requesting timeout (`Offer, no ACK`) are
distinct terminal FAIL discriminators. Association without a lease is a
host-oracle negative control and cannot pass. The DORA loop and existing
device-free DORA scenario remain bounded and unchanged.

## Cold-path and containment claims

- No new lock, IRQ work, DMA buffer/address, or device index is introduced.
  The ledger is lock-free atomics charged only from existing process paths.
- The operation guard RAII-restores the prior per-wait budget and prior ledger
  on every return.
- Cold firmware upload and its upload budget are unchanged. The shared
  post-firmware control path runs after both cold upload and resident-firmware
  detection, so the new ledger intentionally affects both at that later
  boundary. Genuinely-cold silicon remains a load-bearing regression gate.
- Survey `CHANNEL=0`, association frames, and sink binary format are unchanged;
  diagnostic text and durable DHCP/budget state are added.
- Linux GPL-2.0 source was read for sequence and hardware facts only. No GPL
  implementation text was copied. New Rust logic is independently written
  under MPL-2.0 using existing NeoPlutarch abstractions.
