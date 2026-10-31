<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# BOUNCE: eth0 rung-4 bare-metal — CDC-ECM `connected=false` overrides a live PHY → Chad

**Date:** 2026-08-01
**From:** PM (Claude)
**Run:** JOINT bare-metal, OptiPlex, `codex/eth0-boot-product-flow-chad` @ `c1e4405` (product ISO). **Result: conclusive FAIL, not rung-4 PASS — but a clean, self-diagnosing one with a bug localized to two files.**

## The evidence chain (this is a textbook conclusive run)
**Ground truth — the dongle is genuinely live** (Ubuntu, same box, pre/post):
- `0b95:1790 ASIX AX88179 Gigabit Ethernet`; `enx<mac> UP,LOWER_UP`; **DHCP `<optiplex-ip>/24`**; `ethtool: Link detected: yes`.

**Our product boot got further than ever before, then stopped one step short:**
- `usb: enumerated … vid=0b95 pid=1790` ✅ enumeration
- `cdc-ecm: … native-init … linked=true` ✅ **vendor init reports PHY LINKED**
- `net: interface registered … name=eth0` ✅ interface registration
- `cdc-ecm: link notification connected=false` ← **the class notification disagrees with the PHY**
- `[ kernel ] eth0: no link after bounded settle; boot continues` ← treated as hard LINK_DOWN, **DHCP never attempted**
- **Boot completed normally. No hang, no panic, no regression.** ✅ the non-functional bar held.

## Diagnosis
**Two link signals disagree, and the wrong one wins.** The AX88179 **native/vendor init reports `linked=true`** (authoritative for this chip), but the **CDC-ECM class `NETWORK_CONNECTION` notification reports `connected=false`**, and `boot_eth0`'s explicit-down gate treats that notification as authoritative → it **refuses to even attempt DHCP**. See the link-notification handling in `cdc_ecm.rs` and the explicit-down gate in `boot_eth0_logic.rs`.

**The governing principle for the fix: absence of a positive link notification ≠ link down.** Attempting DHCP and failing is cheap and already bounded (4 attempts, 750k pump budget each). Refusing to attempt is what costs us the lease. A vendor-bound device whose own PHY says "linked" must not be vetoed by a class notification the vendor path may never emit correctly.

## Prior-art trap-mine (required before coding — driver-dispatch standing rule)
Mine the **Linux `ax88179_178a`** driver from **raw upstream source** (git/curl, never WebFetch):
1. **How does upstream determine link for the AX88179?** It is a **vendor** driver, not a pure CDC-ECM class driver — does it use the class `NETWORK_CONNECTION` notification at all, or does it poll the PHY / read vendor status registers (`AX_MEDIUM_STATUS_MODE`, PHY BMSR)? If upstream ignores the class notification for this chip, that is the answer.
2. **Does the AX88179 emit the class notification at all in vendor mode**, and if so, is `connected=false` an *initial* state that only updates after a later interrupt we never wait for?
3. **Is there a required link-setup step** (medium mode / `AX_MEDIUM_STATUS_MODE` write) that must happen before the device reports connected?

## Fix direction (pick per the trap-mine, don't guess)
- **Prefer the vendor PHY status** for vendor-bound devices (AX88179): if native-init reports `linked=true`, proceed to DHCP regardless of the class notification.
- **Or** treat "no positive notification yet" as **unknown**, not down — attempt DHCP anyway; the bounded budget already makes a wrong guess cheap.
- **Or** poll the PHY/medium-status during the settle window instead of waiting on a notification.
- **Keep**: bounded waits, `boot continues` on genuine failure, `no repair/format`, kill-switch, I-14.

## Discriminator bank for the next run (must be conclusive either way)
| Observation | Meaning |
|---|---|
| `link up ip=<a.b.c.d>` + `fence_refusals=0` | **rung 4 PASS** |
| PHY `linked=true`, DHCP attempted, `no lease` | link fixed, DHCP path is next — capture DORA + the `chaddr` non-empty prior-art trap |
| Still `no link` with PHY `linked=true` | the gate still trusts the notification — log **both** signals side by side and which one gated the decision |
| `no device` with the dongle attached | enumeration regression — capture xHCI bind |
| Hang or panic | unacceptable — capture and bounce immediately |
**Instrument first:** log the vendor PHY status and the class notification **together, with which one drove the decision**, so the next run cannot be ambiguous.

## Relay — Chad
- **WORKTREE:** `E:\AI Automation\workers\chad` · **BRANCH:** `codex/eth0-boot-product-flow-chad` (continue)
- **Read:** `git fetch origin && git show origin/main:Plans/BOUNCE-2026-08-01-eth0-link-notification-vs-phy-chad.md`
- **One-line:** **Bare-metal run was conclusive — you got enumeration, vendor init, and `eth0` registration all working, then lost it on a link-signal disagreement.** Ubuntu proves the dongle is live (DHCP `<optiplex-ip>`, `Link detected: yes`); our boot showed `cdc-ecm: native-init … linked=true` **and then** `cdc-ecm: link notification connected=false`, and the explicit-down gate in `boot_eth0_logic.rs` treated the *notification* as authoritative → **DHCP was never attempted**. **Absence of a positive link notification ≠ link down.** Trap-mine the Linux `ax88179_178a` driver from raw source first — it's a *vendor* driver, so check whether upstream uses the class `NETWORK_CONNECTION` notification for this chip at all or polls PHY/medium-status instead, and whether `connected=false` is just an initial state. Then either prefer the vendor PHY status for vendor-bound devices, or treat "no positive notification" as unknown and attempt DHCP anyway (the 4-attempt/750k-pump budget already makes a wrong guess cheap). **Instrument both signals side-by-side with which one gated the decision** so the next boot is unambiguous. Everything else held perfectly: bounded settle, `boot continues`, no hang, no panic, no regression.
- **RAG** health-check first; e.g. `cdc_ecm link notification NETWORK_CONNECTION connected`, `ax88179 native init linked medium status`, `boot_eth0_logic explicit down gate link`.
