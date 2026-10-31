<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RUN SHEET: WiFi `f4a0940` bare-metal silicon LINK-UP (JOINT — PL + Jarvis)

**Date:** 2026-07-26
**From:** PM (Claude)
**Candidate:** `codex/mt7601u-post-qselect-terminal-jarvis` @ `f4a0940` — **4-shard gate GREEN, PM-verified** (run `30208021561`, all 4 QEMU shards + host + SPDX + identity, exact SHA). This is the last non-hardware box; the silicon LINK-UP is the WiFi acceptance gate (handover §17).
**Authoritative procedure:** `git show origin/main:docs/runbook-wlan0-dora-optiplex.md` + `…:docs/runbook-baremetal-arthur-optiplex.md`. This sheet is the checklist; the runbooks are authoritative for any detail.
**Roles:** JOINT by construction — **PL** at the box (creds, boot-select, live on-screen read, PuTTY serial); **Jarvis** drives SSH + build + first-pass analysis; **PM** does the byte-level serial read + the merge disposition.

## 0. Readiness — confirmed
- ✅ `f4a0940` 4-shard gate GREEN (PM-verified, exact SHA).
- ✅ Scan-and-join route (`CHANNEL=0`, no baked channel) — the "ship correct" decision.
- ✅ Live-silicon scenario: **`mt7601u_assoc_baremetal`** (cold FW upload → full survey by SSID → WPA2 assoc → EAPOL/GTK → `wlan0` reg → real DHCP DORA).
- ⚠ **You supply:** a real **2.4 GHz WPA2-PSK AP in range** + its **byte-exact SSID + passphrase** (filled locally only — never git/chat/serial).

## 1. Pre-flight — re-inventory the rig (do NOT assume last state; handover §10)
`ssh -i ~/.ssh/id_ed25519 ubuntu@<optiplex-ip>` (if host-key warning: `ssh-keygen -R <optiplex-ip>`). Then confirm:
- `lsblk` — `/dev/nvme0n1` is the internal boot+sink target; `/dev/sda` is the Arthur NeoFS/model volume (**NEVER a dd target**).
- `lsusb | grep 148f:7601` — the MT7601U enumerates clean (high-speed USB). *An empty scan later is NOT "dongle absent" without contrary enum evidence.*
- SSH alive; if not, start `ssh` physically at the box.

## 2. Fill credentials — LOCAL ONLY, never committed (handover §9)
1. Prove clean parity first: candidate branch checked out at `f4a0940`, `git status` clean.
2. Fill **only** SSID + passphrase in `kernel/src/drivers/usb/mt7601u/mod.rs` (lines ~55/57); keep `CHANNEL: u8 = 0`.
3. `git diff --stat` **must name only `mod.rs`**; **no staged credential hunk**.
4. **Never** `git add`/commit/push/`git stash` the filled file.

## 3. Build + verify the WiFi ISO (Jarvis, SSH/build host)
```
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --build-only mt7601u_assoc_baremetal --bare-metal-mode --include-ignored
Get-Content target\neo-test\vmware\mt7601u_assoc_baremetal\manifest.txt   # confirm log_sink_offset=0x10000000, size=16777216, disk 4 role=log-sink/nvme
```
Confirm the firmware module is present (`xorriso … -find /modules/mt7601u-fw`).

## 4. Stage to the OptiPlex + clear the sink (handover §10)
```
# clear the evidence sink BEFORE the boot:
sudo -n dd if=/dev/zero of=/dev/nvme0n1 bs=1M seek=256 count=16 conv=notrunc,fsync ; sync
# copy (scp -O — the live session lacks SFTP):
scp -O target\neo-test\vmware\mt7601u_assoc_baremetal\mt7601u_assoc_baremetal.bare-metal.iso target\neo-test\mt7601u_assoc_baremetal.nvme-log.raw ubuntu@<optiplex-ip>:/tmp/
# read-only identity check, then write ISO + sink to nvme0n1 (confirm nvme0n1 is the target, NOT sda):
sudo -n dd if=/tmp/mt7601u_assoc_baremetal.bare-metal.iso of=/dev/nvme0n1 bs=4M conv=fsync status=progress
sudo -n dd if=/tmp/mt7601u_assoc_baremetal.nvme-log.raw of=/dev/nvme0n1 bs=1M skip=256 seek=256 count=16 conv=notrunc,fsync status=progress
```

## 5. Boot + capture (JOINT)
1. **PL:** PuTTY serial on the OptiPlex COM port → log to **`C:\Users\<user>\neoplutarch-serial.log`** — **start fresh, not append** (preserve any prior capture first).
2. **Boot NeoPlutarch:** `sudo efibootmgr -n 0001 && sudo reboot` (one-shot; falls back to Windows after → boot-select to return to Ubuntu).
3. **Let it run to a bounded terminal — do NOT halt because a stage looks stalled.** f4a0940 bounds the post-Q_SELECT MAC-init (the 250k-op ledger + staged terminal), so a stall self-terminates with diagnostics rather than hanging forever.

## 6. What a PASS looks like (acceptance — handover §17.5)
The serial must show the full chain, not just the terminal:
`scan/survey finds the AP → deferred CAL_BW → WPA2 assoc (mlme=connected) → EAPOL/GTK → wlan0 registration → DHCP DORA (Discover→Offer→Request→Ack) → nonzero IPv4 → **LINK UP** → exactly ONE terminal → fence_refusals=0`, ending:
```
mt7601u: wlan0 dhcp ack ip=<a.b.c.d> — LINK UP
NEO_TEST/1 event=PASS name=mt7601u_assoc_baremetal
```
The terminal PASS **alone is insufficient** — verify the intermediate breadcrumbs + sink agreement. **No baked-channel fallback** may appear.

## 7. Failure-mode / discriminator bank (the run must be conclusive pass OR fail)
| Observation | Cause | Discriminator |
|---|---|---|
| **Scan finds no AP** | RF/antenna/firmware, or AP out of range/band | the survey result list (which SSIDs/channels seen); re-confirm `lsusb 148f:7601` before blaming the dongle |
| **NOTE-A (armed): scan finds AP but assoc fails uncalibrated** | deferred CAL_BW recal didn't complete post-channel-select | `SCAN` shows the AP + `ASSOC` fails → the **CAL-retry** path (NEVER a baked channel) |
| **7-min pre-sink silence before MAC-init** | the post-Q_SELECT init gap `f4a0940` fixes | the 250k-op ledger + staged terminal should now bound/diagnose it (vs the prior blind hang) |
| **MCU wedge on channel-switch** | the `MCU_CAL_BW`-in-`set_bw_filter` wedge | firmware heartbeat / MCU state in serial |
| **Assoc ok, no IP** | DHCP `chaddr` empty (prior-art trap) | LINK path reached but no DORA → check `chaddr` non-empty |
| **FW reload stall at ILM byte 0** (seen once) | firmware upload path | the ILM byte counter in serial |

## 8. Post-run
1. **Save** the serial log (the durable proof). **Read the NVMe sink** (backup): `sudo -n dd if=/dev/nvme0n1 bs=1M skip=256 count=16 2>/dev/null | strings`.
2. Boot-select back to Ubuntu.
3. **Credential revert (mandatory, handover §9):** `git checkout -- kernel/src/drivers/usb/mt7601u/mod.rs`; prove SSID/passphrase back to `b""`, `CHANNEL=0`, staged diff empty, worktree clean.
4. **Security grep BEFORE sharing serial/sink:** grep both for the passphrase — **it must NOT appear**. A scan-result SSID may appear — disclose it consciously, not accidentally.
5. **Paste the serial log to the PM** — I do the byte-level read (PASS/FAIL + which discriminator fired) and, on a real LINK-UP, the WiFi merge disposition.

## Also stageable this session (secondary — say the word and I prep)
- **Arthur perf** (real tok/s): the arthur-perf coverage merged (`32c8ca7`) makes the inventory `sys_generate` path timed + non-vacuous — a **fresh** Arthur ISO would now report the real inventory-path number (`real_silicon`). Needs a rebuild (not the old staged ISO).
- **Slice-4 EU byte-equality** (glass GPU silicon proof) — if you want the glass-on-silicon read too. Separate ISO.
WiFi is the one that's **gate-green and ready now**; the others need a rebuild first.
