<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# FIX-FORWARD: MT7601U Linux parity after Q_SELECT -> Jarvis

**Date:** 2026-07-24
**From:** PM
**To:** Jarvis
**Parallel source-parity reviewer:** Oracle
**Contract/sink reviewer:** Cerebro
**Rejected silicon candidate:** `625e531f353b417498a968dfa0b74f1d45c0e592`
**Rejected ISO:** `53b625dcc2b03bf53a994d8dd6cb72fc42302a3b1a70309fcc57b59d77c067d2`
**Verdict:** **TEST 3A RED. Port the known-good Linux initialization semantics. No identical boot, reset integration, workflow dispatch, merge, or PR.**

## What the boot proved

The exact candidate enumerated the MT7601U at high speed as `148f:7601`.
WPDMA idle, ASIC readiness, CSR/BBP reset, USB DMA setup, and `Q_SELECT`
completed.

Although the procedure requested a cold dongle, `COM_REG0=1`; the driver took
its resident-firmware path and skipped the 45,412-byte upload. The cold-upload
row is therefore untested, not failed.

After:

```text
mt7601u: POST-FW after op=MCU_Q_SELECT result=OK
```

serial produced nothing for nearly seven minutes. No EEPROM, calibration,
survey, association, DHCP, link, PASS, FAIL, or `fence_refusals` value
followed. The next production call is `write_mac_initvals()`, whose first work
is an MCU `RANDOM_WRITE`.

This is evidence-neutral: the kernel hung at or before the first MAC-init
command. It does not establish MCU death.

The sink installed and self-tested but stopped after xHCI endpoint
configuration at 3,927 extracted bytes, while serial reached 9,134 bytes and
`Q_SELECT`. A zero terminal-category count and serial/sink divergence are both
contract failures.

Evidence:

```text
Workers/jarvis/target/neo-test/baremetal-evidence/mt7601u-test3a-625e531f-20260725

serial:
  sha256 39ec0c2002b85e27e41963ad06178f799b341d045d4010233f3a191220f1e7bc
sink raw:
  sha256 e5d18c0b5c79eaf5f7e116ffc86d75dadb5bf1ab67529ea2be20497de213cebf
sink extracted:
  sha256 0be0c1f1c349228e25aa1416b0e0458f451830077a77ee9be74e6c4cf02fa7e2
```

Credentials were absent from both evidence paths. `/dev/sda` was untouched.
Exactly one NeoPlutarch boot occurred.

## The upstream answer

Use current mainline Linux as the primary reference:

- `drivers/net/wireless/mediatek/mt7601u/mcu.c`
  - `mt7601u_load_firmware`
  - `mt7601u_mcu_cmd_init`
  - `mt7601u_write_reg_pairs`
- `drivers/net/wireless/mediatek/mt7601u/dma.c`
  - `mt7601u_dma_init`
  - `mt7601u_submit_rx`
- `drivers/net/wireless/mediatek/mt7601u/init.c`
  - `mt7601u_init_hardware`

Primary-source URLs:

```text
https://raw.githubusercontent.com/torvalds/linux/master/drivers/net/wireless/mediatek/mt7601u/mcu.c
https://raw.githubusercontent.com/torvalds/linux/master/drivers/net/wireless/mediatek/mt7601u/dma.c
https://raw.githubusercontent.com/torvalds/linux/master/drivers/net/wireless/mediatek/mt7601u/init.c
```

The load-bearing sequence is explicit:

1. Linux enables USB DMA.
2. If firmware is already running, Linux returns from firmware loading. It
   does **not** execute the cold-upload FCE/PBF setup and does **not** issue a
   vendor reset.
3. Linux executes `Q_SELECT`.
4. Linux allocates and submits the command-response URB immediately.
5. Linux initializes DMA and posts packet-RX URBs.
6. Only then does Linux issue the MAC-init `RANDOM_WRITE` table.

The rejected NeoPlutarch candidate diverges at all three relevant seams:

1. its resident path calls `fce_preload_setup()`, including `vendor_reset()`
   and the cold FCE/PBF programming, before claiming the resident firmware is
   resynchronized;
2. its command-response TD is not armed at the `Q_SELECT` boundary; it is
   deferred until a later response-bearing command;
3. packet-RX buffers are posted only in `scan()`, after the entire MAC/BBP/
   EEPROM/PHY bring-up.

The first divergence has no mainline sanction and can invalidate a running
MCU/FCE state. The second and third change the known-good ordering immediately
before the observed hang. This dispatch is to restore Linux parity, not invent
another recovery scheme.

## Jarvis implementation lane

Create a new branch from current `origin/main`. Replay the eight frozen WiFi
candidate commits without semantic changes, preserving their stable patch
IDs, then append the narrow repair. Preserve the rejected branch and SHA.

Independently inspect the primary source and own the patch. Required outcomes:

1. **Resident firmware:** after enabling USB DMA, `COM_REG0=1` must take the
   Linux early-return behavior. Do not run cold-upload preload, vendor reset,
   FCE/PBF reprogramming, firmware reload, or forced `COM_REG0` mutation.
2. **Command response:** after successful `Q_SELECT`, arm exactly one owned
   command-response TD before any MAC-init traffic. Preserve the existing
   copy-before-rearm and sequence/CMD_DONE validation.
3. **Packet RX:** establish the Linux-equivalent posted packet-RX readiness
   before MAC init. Refactor ownership so `scan()` reuses the posted pool
   rather than double-posting it.
4. **MAC-init bound:** put the complete MAC-init table operation under one
   finite, non-replenishing operation budget. Emit before/after and per-chunk
   command/sequence/length/wait-response breadcrumbs sufficient to identify
   the exact first non-progressing transfer.
5. **Truthful terminal:** any bulk-OUT or response timeout must produce one
   terminal category and stop. No forced reload, continued traffic, or MCU-
   death claim.
6. **Sink durability:** flush the sink at each post-firmware boundary and at
   the MAC-init terminal. Serial and sink must reach the same final
   discriminator.
7. Preserve the cold firmware-upload path, full `CHANNEL=0` survey, NOTE-A
   CAL retry, DHCP/`LINK UP` PASS contract, operation ledger, DMA fence, and
   structural kill switches.

Do not integrate Chad's reset actuator. It is banked and unrelated to this
repair.

## Required hostile oracles

Add device-free coverage for:

- resident firmware takes the Linux early return and performs zero preload,
  vendor-reset, or FCE/PBF writes;
- cold firmware still takes the existing verified upload path;
- successful `Q_SELECT` is followed by response-TD ownership and packet-RX
  readiness before MAC init;
- multi-chunk MAC initialization requests a response only on the final chunk,
  matching Linux;
- the first MAC `RANDOM_WRITE` bulk or response timeout terminates within the
  operation-wide budget;
- terminal traffic count after failure is zero;
- terminal serial/sink flush is requested;
- a negative control reproduces the rejected resident-reset/order behavior.

## Parallel review lanes

### Oracle — primary-source parity

Analysis only. Independently compare the exact Linux functions above with the
rejected candidate and Jarvis's returned repair. Return:

- an ordered Linux/Neo operation table;
- byte/frame equivalence or divergence for the first MAC command;
- whether command-response and packet-RX readiness match mainline;
- any remaining invented reset/recovery behavior;
- `TOKEN-OK` or `BOUNCE`.

Do not write production code or touch hardware.

### Cerebro — contract and sink truth

Review the returned exact SHA. Require a production-bound hostile oracle,
finite whole-MAC-init budget, exactly one terminal category, zero post-terminal
traffic, and serial/sink convergence. Return `TOKEN-OK` or `BOUNCE`.

Do not write production code or touch hardware.

## Gates and RETURN

Jarvis returns:

- new branch, exact tip/parent/base, old-to-new commit map, stable patch IDs,
  own-stat, touched paths, clean worktree, and remote parity;
- primary-source parity table;
- focused oracle commands/counts;
- all MT7601U host tests;
- blank-credential `mt7601u_assoc_baremetal` build with `CHANNEL=0`;
- normalized OFF equality against the accepted baseline;
- one successful `scripts/worker-close-gate.sh`;
- exact-SHA automatic push-CI URL.

No four-shard token is granted here. After Oracle and Cerebro review the exact
return, the PM will decide the runner token and the next single WiFi boot.

No credential-filled build, hardware boot, reset integration, merge, or PR is
authorized.
