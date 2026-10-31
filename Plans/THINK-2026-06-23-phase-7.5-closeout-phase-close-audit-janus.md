<!-- SPDX-License-Identifier: CC-BY-4.0 -->

# THINK: Phase 7.5 closeout phase-close audit - Janus

Date: 2026-06-23
Branch: `codex/phase-7.5-closeout-audit-janus`
Base: `origin/main` at `90774f9c5bba2460880e43ff01863edcec3ddb13`
Scope: research/audit only. No code changes, no QEMU dispatch.

## Executive verdict

Phase 7.5 is not honestly closeable today, but it is closeout-tail work, not an
unknown feature phase. The substrate lanes 0/A/B/V landed with QEMU and/or
OptiPlex evidence; WPA2/WPA3 crypto landed; MT7601U replaced the originally
planned `iwlwifi` target by a recorded Article 24.1 PL decision; MT7601U scan,
cold firmware self-upload, and a targeted WPA2 on-air association are
silicon-proven.

The remaining close blockers are specific:

1. resolve or explicitly contain the ch6+ MT7601U channel-switch wedge;
2. prove GTK/broadcast RX and real DHCP over `wlan0`;
3. close the intermittent SMP AP-bringup hang;
4. finish 7.5-D identity wiring;
5. run one final integrated QEMU + bare-metal close gate;
6. do a final no-feature consolidation/status-edit pass before the phase tag.

The bible, README, and OT-27 mostly tell the same truth: Phase 7.5 is still in
progress. A few stale status fragments should be repaired during closeout, most
notably ADR-057's "Stage-B pending" text after W2 association landed and a
README caveat that still phrases OT-2 as future work.

## Source frame

- Planned order: `Plans/THINK-2026-06-11-phase-7.5-opening-connectivity-lane.md`
  ratifies `0 -> A -> B -> V -> C1 -> C2 -> D`, with Rule-4 consolidation after
  the early feature train.
- Bible phase-close rule: `docs/bible.md:199-205` says a phase is complete only
  when all criteria pass, no anti-criteria trigger, real boot verification has
  happened on QEMU and at least one bare-metal target, and a single squashed
  commit lands on main with the phase tag.
- Current public status: `README.md:18-20` and `docs/OPEN-THREADS.md:55-60`
  both say Phase 7.5 is still in progress and name the open W2/identity tail.
- Scope amendment: `Plans/PL-DECISION-2026-06-13-wifi-target-mt7601u.md` and
  `docs/ADR-017-wifi-support-strategy.md` amend the target from Intel
  `iwlwifi` to MediaTek MT7601U.

## A. Landed-vs-planned gap table

| Planned lane | Planned close meaning | Landed evidence | Still open / gap | Owner / close action |
| --- | --- | --- | --- | --- |
| 7.5-0 fenced DMA arena | Driver DMA must be confined before unsafe external devices become phase-critical. | ADR-045 accepted the staged hybrid and QEMU adversarial gate; close evidence landed in `eef664a`. The arena is consumed by later USB work. | No known phase-close blocker in 0. I-14(c2) was intentionally deferred until VT-d and later closed by V. | None beyond final close report evidence rollup. |
| 7.5-A xHCI + USB core | USB host stack enumerates real devices without taking new network/WiFi scope. | ADR-046 accepted the xHCI/USB core. `12b64de` records Stage-B PASS: first real-device enumeration on OptiPlex silicon. Later W1/W2 MT7601U work also depends on this substrate. | No known A-specific blocker. | None beyond final close report evidence rollup. |
| 7.5-B USB-Ethernet | Existing net stack works through a USB NIC, proving the USB/network bridge before WiFi. | ADR-047 records Stage A QEMU slirp plus Stage B OptiPlex physical AX88179/CDC-ECM evidence: DHCP DORA, DNS, ARP, and real LAN lease `<optiplex-ip>`; Stage-B evidence path is `Plans/evidence/phase75-buildB-optiplex-PASS-stageB.log`. | No known B-specific blocker. This does not substitute for DHCP over `wlan0`; it proves the pre-WiFi network path. | None beyond final close report evidence rollup. |
| 7.5-V VT-d / IOMMU | Close DMA threat split before firmware-bearing WiFi and prove I-14(c2) on real hardware. | ADR-050 records QEMU intel-iommu PASS and Stage-B OptiPlex PASS, with evidence at `Plans/archive/evidence/phase7.5-build-V-vtd-optiplex-7080-20260613-stageB-silicon.log`; `7fe15bb` records Stage-B silicon PASS. I-14 is marked fully proven. | No known V-specific blocker. | None beyond final close report evidence rollup. |
| Rule-4 consolidation | Pause feature work after the early train; delete/reconcile dead scaffolding, docs, invariant ledger. | ADR-048 / BUILD-K performed a no-feature consolidation after 7.5-0/A/B plus branding: per-item dead-code audit, docs/ANSPEC renumbering, 14/14 ledger verification, zero new unsafe/invariants, and net LOC reduction. | The opening plan placed consolidation after 0/A/B/V, but actual BUILD-K happened before V. V, W0/W0b/W1/W2, and the banked MT7601U warning debt were not covered by that BUILD-K digest. Later general consolidations exist, but a final Phase 7.5 close consolidation is still owed for the current tail. | PM/closeout owner: final no-feature cleanup before phase tag; decide whether to pay or explicitly bank the `mt7601u` feature-config clippy warnings and reconcile stale docs. |
| 7.5-C1 originally `iwlwifi`; amended MT7601U bring-up | Firmware-bearing WiFi device comes up on silicon with DMA containment and scan evidence. | PL decision `2026-06-13-wifi-target-mt7601u` amends `iwlwifi` to MT7601U. ADR-054 records Stage-B complete PASS on OptiPlex: MT7601U enumeration, firmware run, eFUSE, MAC/PHY, 10 BSSIDs scanned, `fence_refusals=0`, evidence at `Plans/archive/evidence/phase7.5-build-W1-mt7601u-optiplex-7080-stageB-silicon.log` (kept in the private repo). OT-27a later records cold firmware self-upload proven on silicon in `f378f12`. | Scope fork must be stated plainly at close: C1 is only closeable as MT7601U, not as Intel `iwlwifi`, unless PL reopens the original laptop-coverage goal. Open technical gap: ch6+ channel-switch wedge under sustained load gates full cold end-to-end WiFi close. | PL/PM: restate the MT7601U ruling in the close artifact. W1/W2 owner: fix or formally contain ch6+ wedge. |
| 7.5-C2 WPA2/WPA3 control plane | Scan/auth/assoc/crypto reaches real client-mode connectivity. | ADR-051 records WPA2 core KATs and WPA3 SAE H2E KATs. ADR-057 records the in-kernel MLME/supplicant design and W2 test surface. `4f82155` records WPA2 on-air association on bare metal with targeted channel, failover filter, and M3 MIC PDU-length fix. `d1c8bb6` closes OT-2 by swapping SNonce to the kernel ChaCha20 DRBG. | Still open in OT-27: GTK extraction/broadcast RX plus real DHCP over `wlan0`, intermittent SMP AP-bringup hang, and ch6+ channel-switch wedge. Current on-air evidence is WPA2 targeted association, not a full cold boot through DHCP. WPA3 is host/KAT-proven; if PL requires WPA3 on-air evidence, that must be added explicitly. | W2 closeout owner: prove cold end-to-end WPA2 path through DHCP and stability at `NEO_SMP=4`; PM/PL: decide whether WPA3 host/KAT proof is sufficient for Phase 7.5 or whether to require WPA3 AP evidence. |
| 7.5-D identity wiring | Hotplug lifecycle, interface names/identity, and multi-interface `select_source` are coherent across USB-Ethernet and WiFi. | Partial substrate exists: CDC-ECM implements the existing `NetDevice`; W2 adds `WifiNetDevice` conversion after association. | The D lane itself is not landed. README names `7.5-D identity wiring` as an honest open tail. No closeable phase tag without this or a written deferral that changes the close criteria. | D/identity owner: design and land the minimal interface identity/hotplug/source-selection slice, then include it in final QEMU and bare-metal close evidence. |

## B. Bible close-criteria map

| Bible close criterion | Current Phase 7.5 state | Close answer |
| --- | --- | --- |
| All criteria pass | Partial. 0/A/B/V pass; C1 passes only under the MT7601U amendment and with ch6+ still open; C2 has targeted association but lacks GTK/broadcast RX + DHCP-over-`wlan0`; D is not landed. | Not yet. |
| No anti-criteria triggered | No clear anti-criteria violation found. The biggest apparent fork, `iwlwifi` -> MT7601U, has a recorded Article 24.1 PL decision and ADR-017 amendment. Explicit non-scope items remain power management, AP/mesh/IBSS, WPA Enterprise, driver parity, GUI, and additional chipsets. | Tentatively yes, provided the final close report restates the MT7601U amendment. |
| Real boot verification on QEMU and at least one bare-metal target | Component evidence exists across QEMU and OptiPlex: B, V, W1, OT-27a, and W2 targeted association. The integrated final close run is still missing. | Not yet. |
| Single squashed commit lands on main with phase tag | Not done; current work is still closeout research and follow-up implementation. | Not yet. |

### Minimum honest bare-metal bar

The minimum honest Phase 7.5 bare-metal close bar is:

1. cold boot on the OptiPlex from a state where Linux has not warmed the MT7601U;
2. NeoPlutarch uploads MT7601U firmware itself and scans successfully;
3. channel selection is not dependent on a hidden preselection workaround, or the
   remaining channel limitation is explicitly accepted and scoped by PL;
4. WPA2-PSK association reaches connected state with PTK and GTK installed;
5. broadcast RX works well enough for DHCP DORA over `wlan0`;
6. the run is stable at `NEO_SMP=4` and does not hit the AP-bringup hang;
7. evidence is captured in serial/NVMe logs and linked from the close artifact.

WPA3 SAE is currently a host/KAT-proven control-plane feature. That can be an
honest Phase 7.5 close if PL defines the hardware bar as WPA2 on-air plus WPA3
byte/algebraic proof. If "WPA2/WPA3 client mode" is interpreted as requiring
both protocols over a real AP, then add a WPA3-SAE on-air bare-metal run before
close.

### Ordered close checklist

1. PL/PM ruling: restate that the 2026-06-13 MT7601U amendment satisfies C1 for
   Phase 7.5, and record Intel `iwlwifi` as follow-up only. If not, C1 remains
   open by definition.
2. Resolve or explicitly contain the MT7601U ch6+ channel-switch wedge. The
   close artifact must say whether all target channels are supported or which
   limitation is intentionally carried.
3. Close the intermittent SMP AP-bringup hang, or produce a narrow diagnosis
   that proves it is outside the Phase 7.5 close path.
4. Land and prove GTK extraction/broadcast RX plus real DHCP DORA over
   `wlan0`.
5. Land 7.5-D identity wiring: hotplug lifecycle, stable interface identity,
   and multi-interface source selection across USB-Ethernet and WiFi.
6. Run the integrated QEMU close gate at `NEO_SMP=4` for the final close tree.
7. Run the minimum bare-metal Stage-B close bar on OptiPlex and archive serial /
   NVMe evidence.
8. Run a final no-feature Rule-4 consolidation for the actual close tree:
   reconcile module docs/ADR pointers, remove or justify dead scaffolding,
   decide the banked `mt7601u` clippy warnings, and re-check the invariant
   ledger/page-fit warnings.
9. Repair close-status drift in docs before tagging: ADR-057 status, README
   caveat wording, OT-27 if the open tail changes, and any bible firmware
   wording affected by the out-of-git blob staging rule.
10. Publish the close report with commit hashes, evidence paths, residual
    follow-ups, and then land the single squashed phase-tagged commit on main.

## C. Honesty cross-check: bible / README / OT-27

### Aligned and honest

- Bible Article 13 remains the right close rule: Phase 7.5 cannot be called
  complete until criteria, anti-criteria, QEMU, bare-metal, and tagged landing
  are all satisfied.
- README top status is honest: Phase 7.5 is "in progress"; it names USB host,
  USB-Ethernet, VT-d/IOMMU, WPA crypto, MT7601U scan, and W2 association as
  proven, and it also names ch6+ wedge, identity wiring, and warning/forward
  assert debt as open.
- OT-27 is the most precise current ledger for W2 tail status: cold firmware
  self-upload is closed; ch6+ channel-switch wedge, GTK/broadcast RX + DHCP over
  `wlan0`, and intermittent SMP AP-bringup hang remain open.
- OT-2 is closed: SNonce now uses the kernel ChaCha20 DRBG, with KAT coverage.

### Status drift to fix during closeout

- `docs/ADR-057-phase-7.5-w2-mlme-supplicant.md` still says W2 is
  CODE-COMPLETE / Stage-B pending even though `4f82155` proved targeted WPA2
  on-air association on bare metal. The ADR should be amended to distinguish
  "association proven" from the still-open DHCP/GTK/ch6/SMP tail.
- `README.md` has a caveat that still phrases "W2's SNonce swap" as future
  OT-2 closure. OT-2 is already closed by `d1c8bb6`; the caveat should now refer
  only to the actual open W2 tail.
- `docs/bible.md` Article 9.10 says MT7601U firmware is bundled under
  `boot/firmware/mediatek/mt7601u`, while ADR-054's OT-27a amendment says the
  vendor blob is no longer committed and is staged as a Limine boot module only
  when present. This may be wording drift rather than a behavioral mismatch, but
  closeout should make the firmware policy unambiguous.
- ADR-017 still contains historical `iwlwifi` decision text below the 2026-06-13
  amendment. That is acceptable as history, but final close docs should point
  readers to the MT7601U amendment first so the scope fork is not rediscovered
  as a contradiction.
- README says the lane is parked behind the Phase-8/Arthur push. As of this
  closeout fan-out, active Phase 7.5 close research is in flight; update the
  status line when PM lands closeout docs.

## Final call

Do not close Phase 7.5 yet. The honest close is small enough to order, but it is
not a paperwork-only phase tag: D identity wiring plus one real cold
WiFi-through-DHCP bare-metal proof still need to exist. Once those are in hand,
Phase 7.5 can close with MT7601U as the accepted WiFi target and Intel `iwlwifi`
as an explicit future chipset, not as an unfulfilled hidden requirement.
