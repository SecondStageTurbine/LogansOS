<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Runbook — wlan0 cold-association and DORA on the OptiPlex 7080

**Status:** JOINT-run procedure. **Prepared:** 2026-07-21. **Do not boot bare metal until the PL signals.**

This is the WiFi sibling to
[runbook-sliceB-arthur-showcase-optiplex.md](runbook-sliceB-arthur-showcase-optiplex.md).
Use [runbook-baremetal-arthur-optiplex.md](runbook-baremetal-arthur-optiplex.md) for the fixed SSH
target, PuTTY capture, device-identity checks, one-shot UEFI boot selection, return to Ubuntu, and
recovery. Serial plus the independent NVMe evidence region is the proof; a photo is not.

The everything package uses the dispatch-approved **two-ISO fallback**. The same JOINT OptiPlex
session first stages and boots the Arthur ISO, returns to Ubuntu, then stages and boots the WiFi ISO.
The 768 MiB SATA model volume remains in place between boots. This keeps the two scenario kernels
independent and avoids changing the established one-kernel Limine builder.

## Honest scenario identity and success boundary

The live-silicon entry is **`mt7601u_assoc_baremetal`**. Despite its older name, its production path
does all of the required work: cold firmware upload, full survey by SSID, WPA2 association, EAPOL/GTK,
`wlan0` registration, and real wire-RX DHCP Discover → Offer → Request → Ack.

Do **not** select or cite `mt7601u_wlan0_dora` as hardware evidence. That scenario is deliberately
hermetic: it has no dongle or live network and injects Offer/Ack packets to prove logic in QEMU.

The silicon success boundary requires all three of these tokens from the same boot, in order:

```text
mt7601u: 4-way handshake complete — CCMP keys installed
mt7601u: wlan0 dhcp ack ip=<a>.<b>.<c>.<d> — LINK UP
NEO_TEST/1 event=PASS name=mt7601u_assoc_baremetal
```

The harness now makes this boundary structural: terminal PASS requires
`connected=true`, GTK installed, DHCP `Bound`, a non-zero assigned `ip=`,
`link_up=true`, and `pass_eligible=true` in the sink-backed summary. A bounded
DORA timeout terminates FAIL with its state and XID. Retain the full serial/sink
evidence rather than relying on the terminal line alone.

## Prerequisites and local-only AP configuration

- OptiPlex 7080 with its confirmed internal NVMe boot target and Samsung 860 EVO SATA model target.
- The USB keyboard for the Arthur boot.
- A real MediaTek MT7601U dongle whose USB census is `148f:7601`, plugged in before the WiFi cold
  boot. A warm/emulated device is not a substitute for the firmware self-upload proof.
- A real 2.4 GHz WPA2-PSK AP in range. The PL supplies only its byte-exact SSID and passphrase; the
  driver discovers the operating channel. Do not put a production or personal AP credential into git,
  a runbook, chat, or serial.
- The 485,489,152-byte Arthur model and its adjacent vocabulary, plus the 45,412-byte MT7601U
  firmware blob, staged outside git as described below.

The authoritative AP config-swap is `configured_target()` in
`kernel/src/drivers/usb/mt7601u/mod.rs`. Immediately before the final WiFi build, edit only these
local constants:

```rust
const SSID: &[u8] = b"<PL-supplied byte-exact SSID>";
const OPEN: bool = false;
const PASSPHRASE: &[u8] = b"<PL-supplied WPA2 passphrase>";
const CHANNEL: u8 = 0; // shipping scan-and-join route; do not bake a channel
```

`origin/main` intentionally has `SSID=b""`, `PASSPHRASE=b""`, and `CHANNEL=0`; that configuration
fails closed to scan-only. A WiFi image built from those placeholders is useful for QEMU/dongle
diagnostics but is **not** ready to associate. Review the local diff for
`kernel/src/drivers/usb/mt7601u/mod.rs` before building, never commit the credential hunk, and restore
the placeholders locally after preserving the run evidence.

## Stage both out-of-band blobs and assemble both ISOs

From a freshly fast-forwarded repository on the Windows build box, stage and verify the firmware:

```powershell
New-Item -ItemType Directory -Force kernel\assets\firmware\mediatek | Out-Null
Copy-Item "E:\AI Automation\mt7601u.bin" kernel\assets\firmware\mediatek\mt7601u.bin
Get-Item kernel\assets\firmware\mediatek\mt7601u.bin | Select-Object Name,Length
Get-FileHash -Algorithm SHA256 kernel\assets\firmware\mediatek\mt7601u.bin
```

The only accepted firmware artifact is:

```text
mt7601u.bin
  bytes  45412
  sha256 4511b1d840e22aea2bf5fdca419c91c0d94cbfb291b9ac4f8be6d9100d1a7046
```

The path is ignored by git. The builder must copy it into each ISO as the Limine module
`/modules/mt7601u-fw`; a missing blob fails closed. Assemble Arthur first:

```powershell
docker compose run --rm -v "E:\AI Automation\Models:/models:ro" -e NEO_STAGE_B_NEOM=/models/qwen2.5-0.5b-instruct-q4_k_m.neom -e NEO_SMP=4 neo cargo neo-test --build-only arthur_compositor_baremetal --bare-metal-mode --include-ignored
```

Then, after the local AP constants above are set, assemble WiFi:

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --build-only mt7601u_assoc_baremetal --bare-metal-mode --include-ignored
```

Require the model artifacts used by the Arthur build to match:

```text
qwen2.5-0.5b-instruct-q4_k_m.neom
  bytes  485489152
  sha256 b934a7149ede095d9b11e58e5aae9411a233d578e6d772f1eed205a54122ffc9
qwen2.5-0.5b-instruct-q4_k_m.vocab
  bytes  7990862
  sha256 24c573c024cb0552ddd503f5945e1b5b77351de94c1e280963b4427a9344e15a
```

The model and vocabulary live on Arthur's SATA NeoFS image, not in Limine, so the 485 MB file is not
duplicated in RAM during boot. Confirm both manifests and both blob paths before copying anything:

```powershell
Get-Content target\neo-test\vmware\arthur_compositor_baremetal\manifest.txt
Get-Content target\neo-test\vmware\mt7601u_assoc_baremetal\manifest.txt
docker compose run --rm neo cargo run -p neofs-mkfs -- verify target/neo-test/arthur_compositor_baremetal.ahci.raw
docker compose run --rm neo xorriso -indev /workspace/target/neo-test/vmware/arthur_compositor_baremetal/arthur_compositor_baremetal.bare-metal.iso -find /modules/mt7601u-fw -exec lsdl
docker compose run --rm neo xorriso -indev /workspace/target/neo-test/vmware/mt7601u_assoc_baremetal/mt7601u_assoc_baremetal.bare-metal.iso -find /modules/mt7601u-fw -exec lsdl
```

Each manifest must report `log_sink_offset=0x10000000`, `log_sink_size=16777216`, and disk 4 as
`role=log-sink`, `qemu_backend=nvme`, `vmware_controller=nvme`. Arthur additionally has disk 0 as
the SATA/AHCI scenario model. WiFi's disks 0–3 are QEMU fillers and must be omitted on hardware.

## One JOINT session, two boots

First perform the Arthur stage and run exactly as documented in the Slice-B sibling. Its deployable
files are the Arthur ISO, `.ahci.raw` model volume, and `.nvme-log.raw` evidence image. After the PL
captures the composited real answer, return to Ubuntu and archive the Arthur PuTTY log and NVMe sink
before overwriting the boot ISO or evidence region.

For the second boot, copy only the WiFi ISO and sink image:

```powershell
scp target\neo-test\vmware\mt7601u_assoc_baremetal\mt7601u_assoc_baremetal.bare-metal.iso target\neo-test\mt7601u_assoc_baremetal.nvme-log.raw ubuntu@<optiplex-ip>:/tmp/
```

On Ubuntu, repeat the generic runbook's read-only identity check and stop unless `nvme0n1` is the
confirmed internal NVMe boot target. Do not rewrite `sda`; it retains the Arthur model volume.

```bash
lsblk -d -o NAME,MODEL,SIZE,SERIAL,TRAN
sudo -n dd if=/tmp/mt7601u_assoc_baremetal.bare-metal.iso of=/dev/nvme0n1 bs=4M conv=fsync status=progress
sudo -n dd if=/tmp/mt7601u_assoc_baremetal.nvme-log.raw of=/dev/nvme0n1 bs=1M skip=256 seek=256 count=16 conv=notrunc,fsync status=progress
sync
```

The second command resets only the independent 16 MiB region. It must keep both `skip=256` and
`seek=256`; never write the whole 288 MiB sink image over the boot disk.

Then:

1. Plug in the MT7601U dongle before power/boot and verify the configured 2.4 GHz AP is powered,
   broadcasting the byte-exact SSID, and in range. The driver discovers its channel.
2. PL archives/clears the prior PuTTY capture and starts a fresh capture at
   `C:\Users\<user>\neoplutarch-serial.log`.
3. Coder triggers the one-shot boot with `sudo efibootmgr -n 0001 && sudo reboot`.
4. Wait for `LINK UP` or a terminal failure/timeout. Do not retry blindly; preserve the first run's
   serial and sink first.
5. Return to Ubuntu using the generic runbook and read the backup sink:
   `sudo -n dd if=/dev/nvme0n1 bs=1M skip=256 count=16 2>/dev/null | strings`.

## Ordered serial breadcrumb map

Concurrent writers can share a serial line. Match token substrings in semantic order rather than
requiring every token at column zero.

| Order | Required breadcrumb | What it proves |
|---:|---|---|
| 1 | Limine `Loading module` for `mt7601u-fw`; xHCI `running` | The firmware module path resolves and the physical USB controller initialized. |
| 2 | NVMe attached and `bare_metal_log_sink_selftest=ok` | The independent evidence region is writable before scenario dispatch. |
| 3 | USB census/bind includes VID:PID `148f:7601` | The intended physical dongle enumerated; QEMU without it must stop with `mt7601u_dongle_not_enumerated`. |
| 4 | `mt7601u: cold fw upload` with `(45412 bytes)` then `mt7601u: firmware booted` | The cold device accepted ILM/DLM/IVB transfer and its MCU booted the staged firmware. |
| 5 | `mt7601u: EEPROM ver=0x0d`, MAC/PHY/calibration lines, and `stage=` progress | EFUSE identity, calibration, and MAC/PHY bring-up completed. |
| 6 | `survey hop ch=1..14 cal_bw=false`, each hop's ordered RF_FREQPLAN/LNA/VCO/BW20 breadcrumbs, then `scan summary — survey_cal_bw=false channels_wedged=0` | The full sweep skipped only the wedging CAL_BW round-trip while retaining the per-hop RF program. Any missing `after op=` line localizes a different scan-path wedge before association. |
| 7 | `scan complete — <n> BSSID(s)` and `STATE bss ch=... bssid=... ssid=...`, then a target-SSID match/failover line | Real RX found the configured SSID and selected the discovered BSSID/channel. This is the boundary between scan success and association work. |
| 8 | `assoc attempt`, the selected BSSID, and `CHANNEL-TRIPLE beacon_ds=<n> tuned=<n> tx=<n> ... OK(aligned)` | Join target selection and RF TX channel agree. Any divergence is wrong-channel TX. |
| 9 | `associating to`, `authenticated to`, then `assoc result state=Handshaking aid=... last_status=0x0000` | Open-system authentication and association responses arrived from the AP. |
| 10 | `POST-ASSOC-CAL before cmd=CAL_BW` then `result=OK` | The deferred survey calibration completed on the accepted operating channel before protected data. |
| 11 | `4-way — awaiting M1`, `M1 received`, `M2 sent`, `M3 received (MIC ok)` | Real EAPOL M1→M3 crossed the RX/TX path and the PTK/MIC check succeeded. |
| 12 | `gtk_install wcid=0 cipher=ccmp`, `gtk_wcid0_attr readback=...`, then `4-way handshake complete — CCMP keys installed` | Hardware GTK install/decrypt is armed and MLME reached connected. |
| 13 | `wlan0 registered ... index=<n>` then `wlan0 dhcp discover ... iface=<same n>` | Dynamic interface identity routes DHCP through the actual WiFi net device, including a non-zero index. |
| 14 | `dhcp: offer_rx ... (request_tx follows)` then `dhcp: request_tx iface=<n>` | Broadcast Offer traversed real RX/demux and Request transmitted on wlan0. |
| 15 | `dhcp: ack_rx ip=...` then `mt7601u: wlan0 dhcp ack ip=... — LINK UP` | The real AP/DHCP server completed DORA and assigned the recorded IPv4 address. |
| 16 | final association summary has `connected=true gtk=true dhcp=bound_link_up ip=<nonzero> link_up=true pass_eligible=true bound=true`, then terminal scenario PASS | Association, key installation, real DORA, assigned address, USB DMA containment, and terminal framing all passed. |

This live sequence covers the silicon-only broadcast RX filter (T-046/T-047), hardware GTK decrypt,
the real RX ring/demux, and interface-correct TX that the hermetic DORA test cannot cover.

## Blind-hardware failure and discriminator bank

| Symptom / last good breadcrumb | Boundary and discriminator |
|---|---|
| No `Loading module` or `firmware_blob()` failure | Firmware was absent, corrupt, or not wired in Limine. Recheck the 45,412-byte file, hash, ISO listing, and module path. |
| xHCI runs but no `148f:7601`; terminal `mt7601u_dongle_not_enumerated` | Dongle/cable/port enumeration. Cold-replug into the confirmed port only after preserving evidence; this is not an AP problem. |
| `cold fw upload FAILED` | Use its `fault`, uploaded byte count, unresponsive flag, COM_REG0/retry, and chip revision. Zero/partial upload localizes endpoint stall; full transfer with no boot localizes MCU boot/variant. Do not call it a scan failure. |
| Firmware boots but EEPROM/MAC/PHY stops | Record EEPROM version, exact calibration stage, raw/reference temperature, DPD/TSSI and TX-power readbacks. High-channel-only failure points to the TSSI bucket; sustained drift points to temp/DPD/frequency calibration. |
| A survey hop lacks a matching `after op=` line, reports `result=WEDGED`, or the summary has `channels_wedged>0` | **Scan-path failure, before association.** The hop's `cal_bw=false` proves CAL_BW was deferred; the last RF_FREQPLAN/LNA/VCO/BW20 breadcrumb names the different operation that failed. Preserve channel, opcode/sequence/elapsed, COM_REG0, and bounded-recovery result. |
| `channels_wedged=0`, but `scan complete — 0 BSSID(s)` or the expected SSID is absent | **Scan completed; association was never attempted.** Confirm the AP is 2.4 GHz, in range, and broadcasting the byte-exact SSID. Other BSSIDs present but target absent points to AP/range; no BSSIDs points to insufficient uncalibrated RX sensitivity or RX-ring failure. |
| Target SSID/BSSID is selected, then `auth incomplete` or association returns a non-zero status | **Association-path failure, not a scan wedge.** Preserve `directed_to_us`, first directed FC, TX power, AID and status. **NOTE-A:** if scan succeeded but auth/assoc fails before post-association calibration, add the pre-registered CAL-retry fallback rather than restoring a baked channel. |
| `POST-ASSOC-CAL before` has no `result=OK`, or reports `result=FAIL` | The uncalibrated scan and association succeeded; the deferred operating-channel CAL_BW itself re-wedged. This is distinct from both scan and auth failure. |
| Optional runtime-targeted `set_channel`/MCU timeout or `CHANNEL-TRIPLE ... DIVERGENCE` | The non-shipping targeted fast path wedged or RF state is stale. Preserve opcode/sequence/elapsed/COM_REG0 and all three channel values. `beacon_ds != tuned` is DS tagging; `tuned != tx` is stale RF after switching. |
| `auth incomplete` | `directed_to_us=0` means the AP did not hear TX or stayed silent; a directed auth/deauth FC means RX arrived and policy/parsing rejected it. Preserve `rx_frames`, `first_directed_fc`, TX power, and `last_status`. |
| Association response has non-zero `last_status`, or no AID | Explicit AP rejection versus silence. Recheck byte-exact SSID/security and the AP's client policy; do not rotate firmware blindly. |
| Stops at M1/M2/M3, MIC, unwrap, or GTK readback | EAPOL/crypto/hardware-key boundary. The last `handshake=` stage distinguishes missing M1, M2 rejection, M3 extraction/MIC, GTK unwrap, or WCID install. No GTK means DHCP must remain fail closed. |
| Discover logged but no `offer_rx`; timeout says `Selecting` | AP/DHCP silence or broadcast-RX/filter/GTK-decrypt path (T-047). This is the main real-RX discriminator; preserve GTK/WCID and RX-ring breadcrumbs. |
| `offer_rx` and Request exist but no `ack_rx`; timeout says `Requesting` | Request TX/interface selection or AP/DHCP rejection. Compare registration index, Discover/Request `iface=`, TX completion, and AP logs if available. |
| `dhcp=timeout_selecting` / `mt7601u_dhcp_timeout_selecting_no_offer`, or `dhcp=timeout_requesting` / `mt7601u_dhcp_timeout_requesting_no_ack` | The harness terminates FAIL. Preserve the XID and classify no Offer versus no ACK; association alone is never success. |
| `no_log_sink_device_available`, sink self-test missing, or PuTTY truncates | Verify manifest disk 4 is the NVMe sink. After returning to Ubuntu, read the 16 MiB region before another boot; serial and sink stopping at the same token localizes the runtime boundary. |
| Panic, DMA fence refusal, or terminal FAIL | Preserve the first fault and both evidence channels. The failure is red; do not infer success from an AP lease or screen state. |

## Invariants that remain armed

- **Kill switch:** only the feature-gated WiFi silicon scenario loads/uses the external firmware and
  local AP target. No firmware or credential enters git, and default scenarios remain unchanged.
- **I-14(a):** xHCI completion, RX-ring service, DHCP demux, and logging do not acquire an
  AI-governance lock. The ordered live RX/TX breadcrumbs demonstrate forward progress.
- **I-14(b):** a stalled association/DORA loop is bounded and returns a localized timeout; it does
  not stall unrelated kernel services. Preserve the last state rather than extending the budget.
- **I-14(c):** the MT7601U stays inside the existing USB DMA arena/fence. Any nonzero
  `fence_refusals` is a hard failure; this package creates no broader DMA aperture.
- **Concurrency pitfalls:** preserve opcode/sequence/elapsed data for MCU single-in-flight wedges,
  IRQ/IF-off waits, lock/wake stalls, and interleaved serial lines. Never infer a missing stage solely
  from line boundaries.
- The paid 33-warning bank remains closed. This run classifies live behavior using the existing
  breadcrumb and discriminator bank; it does not reopen warning cleanup.
