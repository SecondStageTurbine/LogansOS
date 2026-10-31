<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Everything-ISO Arthur + wlan0 DORA JOINT-run prep

**Date:** 2026-07-21
**From:** Jarvis
**To:** PM / PL
**Status:** **RUN-READY — BOTH SCENARIOS.** Both packages assembled and reached their honest QEMU
fail-closed boundaries. No bare-metal boot was performed.

## Result

The everything package uses the dispatch-approved **two-ISO fallback**:

1. `arthur_compositor_baremetal` — Arthur as a G1 client, real 485 MB model on SATA NeoFS, physical
   keyboard → inference → composited-answer capstone.
2. `mt7601u_assoc_baremetal` — physical MT7601U cold firmware upload, scan, WPA2/EAPOL/GTK, and
   wire-driven DHCP DORA → `wlan0` IP.

They run as two boots in the same JOINT OptiPlex session. After the Arthur proof, the operator returns
to Ubuntu, preserves its evidence, stages the WiFi ISO plus a fresh 16 MiB sink overlay, and boots the
second scenario. The SATA model disk remains in place and is not rewritten for WiFi.

The turnkey WiFi procedure is in `docs/runbook-wlan0-dora-optiplex.md`; the Arthur procedure remains
in `docs/runbook-sliceB-arthur-showcase-optiplex.md`. Both defer generic machine access, device
identity, PuTTY, UEFI selection, and recovery to `docs/runbook-baremetal-arthur-optiplex.md`.

## Scenario-name correction required for an honest proof

Audit found that the dispatch-named `mt7601u_wlan0_dora` scenario is deliberately **hermetic**. It has
no device/live network and injects Offer/Ack packets; its terminal frame is `i14=logic_hermetic`.

The already-shipped real-silicon path is `mt7601u_assoc_baremetal`. Its
`run_configured_association()` performs the full cold device path and then pumps real wlan0 RX through
the production DHCP demux. This is therefore the WiFi ISO entry used here. The runbook requires:

```text
4-way handshake complete — CCMP keys installed
wlan0 dhcp ack ip=... — LINK UP
NEO_TEST/1 event=PASS name=mt7601u_assoc_baremetal
```

The terminal PASS alone proves association, not a lease: the current harness can PASS after a DORA
timeout. `LINK UP` is the mandatory live-DORA discriminator.

## Assembly defect found and fixed

The WiFi silicon scenarios accepted `neo-test-log-sink-target-nvme`, but xtask never prepared or
exported that sink because they otherwise use no storage disk. Their generated manifest reported
`disk_count=0`, so there was no durable blind-hardware evidence path.

Xtask now classifies `mt7601u_scan_baremetal` and `mt7601u_assoc_baremetal` with disks 0–3 as omitted
QEMU fillers and disk 4 as the NVMe log sink. It prepares the raw sink, attaches it to QEMU, exports it
in the bare-metal manifest, and gives it the scenario-specific `.nvme-log.raw` path. A regression test
freezes both WiFi mappings. No kernel or userland source changed, so the kernel close gate and
four-shard gate are N/A per dispatch.

## Both out-of-band blobs are staged

Base used for this rehearsal: `00d8f81db37b37378c0d7b4a873f49dd8f8c60d2` (`origin/main`).

Firmware, staged at the ignored path `kernel/assets/firmware/mediatek/mt7601u.bin`:

```text
bytes  45412
sha256 4511b1d840e22aea2bf5fdca419c91c0d94cbfb291b9ac4f8be6d9100d1a7046
Limine path /modules/mt7601u-fw
```

ISO inspection found that exact 45,412-byte module in **both** scenario ISOs and their generated
`limine.conf` files name `module_path: boot():/modules/mt7601u-fw`.

Arthur's model remains outside git and outside Limine:

```text
qwen2.5-0.5b-instruct-q4_k_m.neom
  bytes  485489152
  sha256 b934a7149ede095d9b11e58e5aae9411a233d578e6d772f1eed205a54122ffc9
qwen2.5-0.5b-instruct-q4_k_m.vocab
  bytes  7990862
  sha256 24c573c024cb0552ddd503f5945e1b5b77351de94c1e280963b4427a9344e15a
```

`neofs-mkfs verify` passed on the 805,306,368-byte SATA image with content hash
`3a2c4f70978c5c3db6cf8dfb5e2500d8fa76c0308400ff85fbff23f7e2f847aa` and both model/vocabulary
entries.

## Exact final artifacts and QEMU dress rehearsals

### Arthur

Build command:

```powershell
docker compose run --rm -v "E:\AI Automation\Models:/models:ro" -e NEO_STAGE_B_NEOM=/models/qwen2.5-0.5b-instruct-q4_k_m.neom -e NEO_SMP=4 neo cargo neo-test --build-only arthur_compositor_baremetal --bare-metal-mode --include-ignored
```

Final QEMU-tested ISO:

```text
target/neo-test/vmware/arthur_compositor_baremetal/arthur_compositor_baremetal.bare-metal.iso
bytes  124305408
sha256 f8e3eee933cf263f9eb85b673c66450ead23281fbb4c3d9cc7b9f5bc9323a3e5
```

QEMU used the production AHCI model + NVMe evidence-controller split. The one-vCPU smoke is limited
to the known-safe non-silicon prefix; the kernel artifact was built with `NEO_SMP=4`. It reached:

```text
SHOWCASE greeting composited surface=arthur
vocab_load tokens=151936 bytes=7990862
model_open ... size=485489152
model_load_resident bytes=485489152
SHOWCASE memory stage=model_resident used_bytes=1651732480 frames_used=403255 cores=1
SHOWCASE footprint discriminator=model-485MB-plus-compositor
SHOWCASE model resident with_compositor=true source=neofs
SHOWCASE focus activated surface=arthur no_snoop=true
SHOWCASE keyboard loop=armed source=physical-usb-hid qemu=unsupported fail_closed=wait-for-hid
```

There was no panic or terminal FAIL. QEMU was intentionally stopped at the physical-HID boundary.

```text
serial target/neo-test/arthur_compositor_baremetal.everything-qemu-smoke-20260721.serial.log
bytes  7418
sha256 9fe9a53d90d6fb9974565b2982134f6cd9a7c4190f10ccf66f849a7e5eb7fb35
```

### WiFi

Build command:

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --build-only mt7601u_assoc_baremetal --bare-metal-mode --include-ignored
```

Final QEMU-tested ISO:

```text
target/neo-test/vmware/mt7601u_assoc_baremetal/mt7601u_assoc_baremetal.bare-metal.iso
bytes  126377984
sha256 3087a6f19c401a0ed539ba2b613295f2d481582c5ac290f4d23c7ede1665c6d1
```

QEMU used q35, four vCPUs, xHCI, and the corrected NVMe evidence sink, with no USB dongle
passthrough. It resolved the firmware module, initialized xHCI and the sink, dispatched the exact
scenario, and failed closed at the honest physical-device boundary:

```text
limine: Loading module `boot():/modules/mt7601u-fw`...
[ kernel ] nvme: attached ...
bare_metal_log_sink_selftest=ok
NEO_TEST/1 event=BEGIN name=mt7601u_assoc_baremetal phase=dispatch
[ kernel ] mt7601u-assoc: driving configured association…
mt7601u: assoc — no bound device
[ kernel ] mt7601u-assoc: ... connected=false bound=false ...
NEO_TEST/1 event=FAIL name=mt7601u_assoc_baremetal reason=mt7601u_dongle_not_enumerated
```

The terminal FAIL is the expected QEMU outcome and positively discriminates the silicon-only seam.

```text
serial target/neo-test/mt7601u_assoc_baremetal.qemu-smoke-20260721.serial.log
bytes  9694
sha256 bf5358c9b9c371c2fc24e45e9842c747e7928ba5a785190f5aa433a162dcda61
```

## Final-run prerequisite that stays local

The AP SSID, WPA2 passphrase, and channel are intentionally absent from git. Before the PL-authorized
final WiFi assembly, insert the PL-supplied byte-exact values only into `configured_target()`'s local
constants in `kernel/src/drivers/usb/mt7601u/mod.rs`, leave `OPEN=false`, build the ISO, and do not
commit the credential hunk. Empty upstream placeholders fail closed to scan-only. The runbook makes
this precondition and its exact source location explicit.

## Validation

- RAG health and all four dispatched semantic discovery queries — PASS.
- Full `cargo test -p xtask` suite — 22 passed, 0 failed, including the new WiFi sink regression;
  filtered Arthur checks separately passed 2/2.
- Both bare-metal `--build-only` commands — PASS.
- Both exact final ISOs boot-smoked to their expected non-silicon boundaries — PASS.
- Both ISO trees contain `/modules/mt7601u-fw` at 45,412 bytes — PASS.
- Arthur NeoFS verification — PASS.
- `cargo fmt --all -- --check` — PASS.
- Relevant xtask test filters — PASS.
- `git diff --check` — PASS.

## JOINT-run handoff

Follow the two scenario runbooks exactly, preserve serial plus the NVMe sink between boots, and do
not boot bare metal until the PL signals. The WiFi runbook carries the complete scan → association →
EAPOL/GTK → real DORA breadcrumb map, channel-triple and cold-firmware discriminators, kill switch,
I-14(a/b/c), concurrency pitfalls, and the mandatory `LINK UP` boundary. The paid 33-warning bank was
not reopened.
