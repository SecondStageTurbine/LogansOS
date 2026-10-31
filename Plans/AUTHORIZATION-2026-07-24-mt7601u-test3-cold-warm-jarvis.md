<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# AUTHORIZATION: MT7601U TEST 3 cold → immediate-warm silicon close → Jarvis

**Date:** 2026-07-24
**From:** PM
**To:** Jarvis, JOINT with PL at the OptiPlex
**Candidate:** `2d43670efd354df4f5d28cc62c43bcb2cb4fc234`
**Parent:** `a09a65bb7cdcac48ddf4ceaaf630d69df9257156`
**Frozen base:** `5c990a9b2e934b79b32cc350c4da4cc4344d5ad3`
**ISO size:** `126507008` bytes
**ISO SHA-256:** `eab064bcf5a372e60990cd30c20715fd788486f4015e17652dd22bc41bd96973`
**Verdict:** **SILICON HOLD LIFTED for this exact two-boot sequence and exact ISO only. This is not merge authorization.**

## Independent verification

- Remote branch tip is the candidate SHA; frozen-base relation is ahead 4, behind 0.
- Push CI `30103981191` is attempt 1, exact-SHA green:
  `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30103981191`.
- Four-shard gate `30104558141` is attempt 1, exact-SHA green: all four QEMU shards plus host, SPDX, and identity:
  `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30104558141`.
- `sys_generate_kv_persist_equivalence` and `arthur_multistep_agentic` both passed their first attempt.
- Source placeholders are `SSID=b""`, `PASSPHRASE=b""`, `CHANNEL=0`; no credential bytes are committed and the coder worktree is clean.
- The manifest attaches only the NVMe log sink on hardware; QEMU filler disks are omitted. `/dev/sda` is not a TEST 3 target.

## Controlling documents

Read and obey:

```powershell
git fetch origin
git show origin/main:Plans/AUTHORIZATION-2026-07-24-mt7601u-test3-cold-warm-jarvis.md
git show origin/main:Plans/DISPATCH-2026-07-23-mt7601u-warm-postfw-fix-jarvis.md
git show origin/main:docs/runbook-wlan0-dora-optiplex.md
git show origin/main:docs/runbook-baremetal-arthur-optiplex.md
```

The runbook and its ordered breadcrumb/discriminator tables remain authoritative. Do not reinterpret this authorization as permission to change code, rebuild the ISO, merge, or weaken a terminal boundary.

## Rig preflight

1. Use only the already-verified ISO above. Check byte size and SHA-256 before copying and again on Ubuntu.
2. Modern `scp` will fail because this live session has no SFTP subsystem. Use `scp -O`.
3. Ubuntu live USB host keys rotate. On every warning:

   ```powershell
   ssh-keygen -R <optiplex-ip>
   ```

   Then reconnect. Trust is the JOINT physical/LAN ownership of this box, not continuity of the generated host key.
4. On Ubuntu, run the read-only identity check first:

   ```bash
   lsblk -d -o NAME,MODEL,SIZE,SERIAL,TRAN
   ```

   Stop unless `/dev/nvme0n1` is the confirmed internal NVMe boot target. The manifest must not stage a disk to `/dev/sda`; do not write `/dev/sda`.
5. Burn the exact ISO to `/dev/nvme0n1`. Before each TEST 3 boot, clear the contaminated evidence region independently:

   ```bash
   sudo -n dd if=/dev/zero of=/dev/nvme0n1 bs=1M seek=256 count=16 conv=notrunc,fsync
   sync
   ```

6. Start a fresh PuTTY capture for each boot. Do not reuse or append to a prior TEST 1/Arthur/TEST 3 capture.
7. Let each boot reach terminal PASS or one bounded terminal discriminator. Do not halt because a stage looks stalled; preserve the first result before any recovery action.

## Boot 1 — genuinely cold dongle

Physically depower the MT7601U first, then reconnect it to the confirmed port before the one-shot ISO boot. Require:

- physical USB census `148f:7601`;
- cold firmware upload of the verified `45412` bytes and firmware boot;
- post-firmware control-plane readiness, EEPROM identity, and calibration;
- `CHANNEL=0` full survey with `cal_bw=false`, `channels_wedged=0`, and target SSID found on its discovered channel;
- auth/association with aligned beacon/tuned/TX channel;
- deferred post-association `CAL_BW` result `OK`;
- WPA four-way/GTK completion;
- DHCP Offer/Request/Ack, assigned nonzero IPv4 address, `LINK UP`, and terminal PASS;
- sink and serial agreement, with `fence_refusals=0`.

Any bounded failure is RED. Preserve fresh serial and the raw 16 MiB sink before continuing.

## Boot 2 — immediate warm reboot

Do not unplug, physically depower, or move the dongle after Boot 1. Return to Ubuntu without removing power from the dongle, archive Boot 1 evidence, clear the 16 MiB sink, start a new PuTTY capture, and one-shot boot the **same ISO**.

Require:

- warm-resident firmware path;
- `WARM-RESYNC` setup `OK`;
- firmware already resident, FCE resync applied, ready poll `OK`, upload skipped;
- the same complete survey → association → deferred CAL → WPA/GTK → DHCP ACK → `LINK UP` → PASS chain as Boot 1;
- sink and serial agreement, with `fence_refusals=0`.

If Boot 2 performs a cold firmware upload, the warm proof is not satisfied: preserve evidence and report that the platform reset/depowered the device across reboot. Do not silently promote it or repeat blindly.

## NOTE-A remains armed

If the target AP is found but auth/association fails before post-association calibration, classify it as NOTE-A and follow the existing **CAL-retry path**. A baked-channel fallback is forbidden. Do not rebuild with a channel, take a runtime targeted-channel shortcut as shipping proof, or relabel association failure as scan failure.

## Evidence return

Before pasting serial to PM, grep both captures for the configured SSID and passphrase. A scan-result SSID line may be retained consciously; the passphrase must not appear.

Return:

- exact ISO size/hash re-verification;
- cold and warm raw PuTTY logs plus SHA-256;
- cold and warm raw 16 MiB sink dumps plus SHA-256 and `strings` extracts;
- a cold/warm ordered-breadcrumb table;
- exactly one terminal category per boot;
- DHCP-assigned IP for each PASS;
- explicit `fence_refusals` value;
- credential-grep disposition;
- confirmation that `/dev/sda` was untouched and no merge occurred.

No identical blind rerun. No fail-fast result counts as success. No merge until PM independently verifies both boots.
