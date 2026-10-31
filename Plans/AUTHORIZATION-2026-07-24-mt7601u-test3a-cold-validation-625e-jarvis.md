<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# AUTHORIZATION: MT7601U TEST 3A cold WiFi validation -> Jarvis

**Date:** 2026-07-24
**From:** PM
**To:** Jarvis, JOINT with PL at the OptiPlex
**Candidate:** `625e531f353b417498a968dfa0b74f1d45c0e592`
**Branch:** `codex/mt7601u-aag-oracle-binding-jarvis`
**Push CI:** `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30130344424`
**Four-shard gate:** `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30135801447`
**Verdict:** **COLD-ONLY SILICON HOLD LIFTED for one exact-SHA scan/join/DHCP boot. No warm boot, reset qualification, merge, or acceptance close.**

## Purpose

Separate the core WiFi question from the platform warm-reset question.

The candidate has passed the driver AAG, production-bound hostile oracle,
normalized-OFF proof, exact-SHA push CI, and all four QEMU shards. A single
genuinely cold boot now answers whether its response ownership, bounded
probe/retry, terminal evidence, full survey, association, WPA, DHCP, and link
path work on the MT7601U silicon.

This boot does not test or claim firmware residency across reset. Chad's
FADT-reset qualification remains independent and may continue in parallel.

## Read first

```powershell
git fetch origin
git show origin/main:Plans/AUTHORIZATION-2026-07-24-mt7601u-test3a-cold-validation-625e-jarvis.md
git show origin/main:Plans/FIX-FORWARD-2026-07-24-mt7601u-aag-oracle-binding-jarvis.md
git show origin/main:docs/runbook-wlan0-dora-optiplex.md
git show origin/main:docs/runbook-baremetal-arthur-optiplex.md
```

Do not rebase, amend, or change the candidate. The newer `origin/main`
documents do not alter the frozen candidate.

## Local-only credential build

Build from exact candidate SHA only. Before filling credentials:

1. verify local and remote branch tips are the candidate;
2. verify the worktree is clean;
3. verify committed `SSID=b""`, `PASSPHRASE=b""`, and `CHANNEL=0`;
4. verify the 45,412-byte firmware SHA-256 is
   `4511b1d840e22aea2bf5fdca419c91c0d94cbfb291b9ac4f8be6d9100d1a7046`.

Fill only the local `SSID` and `PASSPHRASE` constants in
`kernel/src/drivers/usb/mt7601u/mod.rs`. Keep `CHANNEL=0`.

Before building, require:

- `git diff --stat` names only that file;
- `git diff --cached` is empty;
- the credential hunk is not staged.

Never `git add`, commit, push, or stash the filled file. Build:

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --build-only mt7601u_assoc_baremetal --bare-metal-mode --include-ignored
```

Record the resulting ISO byte size and SHA-256, complete manifest, and
`xorriso` proof that `/modules/mt7601u-fw` is present.

Immediately restore the source:

```powershell
git checkout -- kernel/src/drivers/usb/mt7601u/mod.rs
```

Then prove `SSID=b""`, `PASSPHRASE=b""`, `CHANNEL=0`, an empty staged diff,
a clean worktree, and unchanged local/remote tips. Preserve only the built ISO
and ignored evidence artifacts.

The hardware manifest may attach only the NVMe log sink. QEMU filler disks
must be omitted. `/dev/sda` is not a target.

## Rig staging

Use `scp -O`; the live Ubuntu session has no SFTP subsystem. If the live USB
host-key warning recurs:

```powershell
ssh-keygen -R <optiplex-ip>
```

On Ubuntu, first run:

```bash
lsblk -d -o NAME,MODEL,SIZE,SERIAL,TRAN
```

Stop unless `/dev/nvme0n1` is the confirmed internal NVMe boot target. Do not
write `/dev/sda`.

Before booting, clear the contaminated evidence region:

```bash
sudo -n dd if=/dev/zero of=/dev/nvme0n1 bs=1M seek=256 count=16 conv=notrunc,fsync
sync
```

Burn only the returned exact ISO to `/dev/nvme0n1`, verify its size/hash on
Ubuntu, and start a fresh non-append PuTTY capture. Preserve the prior capture
under a different name.

## One cold boot

1. Fully unplug/depower the MT7601U dongle.
2. Reconnect it to the same confirmed USB port.
3. One-shot boot the exact ISO.
4. Let it reach terminal PASS or one bounded terminal discriminator. Do not
   halt early because a stage appears stalled.

Green requires:

- USB census `148f:7601`;
- cold upload of exactly 45,412 firmware bytes and firmware start;
- EEPROM/calibration readiness;
- genuine `CHANNEL=0` survey through all required channels, with no baked or
  runtime target-channel shortcut;
- target SSID found on its discovered channel;
- auth/association with aligned beacon/tuned/TX channel;
- deferred post-association `CAL_BW` result `OK`;
- WPA four-way and GTK completion;
- DHCP Offer/Request/Ack with a nonzero assigned IPv4 address;
- `LINK UP`;
- terminal PASS;
- serial and NVMe sink agreement;
- explicit `fence_refusals=0`.

NOTE-A remains armed only when the scan finds the target AP but
auth/association fails before deferred post-association calibration. Use the
existing CAL-retry discriminator; never introduce a baked-channel fallback.

Any other bounded terminal is RED. Do not perform an identical rerun, second
NeoPlutarch boot, FADT reset, or warm-residency experiment under this
authorization.

## Evidence return

After the boot, return to Ubuntu only to preserve evidence. Read the raw
16 MiB sink before any other NeoPlutarch boot.

Before pasting serial, grep it and the sink strings for both configured
credential values. A scan-result SSID may be retained consciously and called
out; the passphrase must not appear.

Return:

- candidate SHA and unchanged branch tips;
- ISO size/hash, manifest, and firmware-module proof;
- credential-fill preflight and placeholder-restoration proof;
- raw PuTTY log plus SHA-256;
- raw 16 MiB sink plus SHA-256 and `strings` extract;
- serial/sink ordered-breadcrumb agreement;
- exactly one terminal category;
- survey/channel summary and target-discovery evidence;
- association, CAL, WPA/GTK, DHCP, assigned IP, and `LINK UP` evidence;
- explicit `fence_refusals` value;
- credential-grep disposition;
- confirmation that `/dev/sda` was untouched;
- confirmation that no second boot, reset, merge, or push occurred.

This cold result can validate the WiFi repair. It cannot satisfy the separate
immediate-warm acceptance row and is not merge authorization.
