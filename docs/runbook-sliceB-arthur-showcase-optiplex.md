<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Runbook — Slice-B Arthur compositor showcase on the OptiPlex 7080

**Status:** JOINT-run procedure. **Prepared:** 2026-07-21. **Do not boot bare metal until the PL signals.**

This is the Slice-B companion to
[runbook-baremetal-arthur-optiplex.md](runbook-baremetal-arthur-optiplex.md). That runbook remains
authoritative for SSH access, PuTTY capture, boot selection, recovery to Ubuntu, and the rule that the
durable proof is serial plus the NVMe sink—not a photo. This document replaces only its old
auto-forward interaction: Slice B waits for the PL to type a nonempty question and press Enter.

## Frozen topology and success boundary

- `nvme0n1`: boot ISO and the independent `NEO_LOG_v1` evidence region at offset `0x10000000`
  (256 MiB), size 16 MiB.
- `sda`: the 768 MiB NeoFS image containing `/model-qwen2.5-0.5b.neom` and
  `/model-qwen2.5-0.5b.vocab`.
- Limine loads `/modules/neo-init`, `/modules/neo-worker`, `/modules/neo-shell`, and
  `/modules/neo-arthur` by path suffix. The 485 MB model is **not** a Limine module: it stays on SATA
  NeoFS so boot does not create another full-RAM copy.
- Success is one session: G1 owns scanout; Arthur is a confined TEXT client surface; USB HID routes
  through `InputFocus`; a real forward produces a reply; G1 composites the answer; the terminal frame
  passes. Direct Arthur framebuffer output is never acceptable.

The selected `.neom` must be exactly **485,489,152 bytes**. The rehearsal artifact was:

```text
qwen2.5-0.5b-instruct-q4_k_m.neom
  sha256 b934a7149ede095d9b11e58e5aae9411a233d578e6d772f1eed205a54122ffc9
qwen2.5-0.5b-instruct-q4_k_m.vocab
  bytes  7990862
  sha256 24c573c024cb0552ddd503f5945e1b5b77351de94c1e280963b4427a9344e15a
```

The builder rejects a missing/empty adjacent vocabulary sidecar and a model of any other byte shape.

## Final re-assembly when the PL signals

Start from freshly fast-forwarded `main`; do not reuse an older ISO. From the repository root on the
Windows build box, the final assembly is this one command:

```powershell
docker compose run --rm -v "E:\AI Automation\Models:/models:ro" -e NEO_STAGE_B_NEOM=/models/qwen2.5-0.5b-instruct-q4_k_m.neom -e NEO_SMP=4 neo cargo neo-test --build-only arthur_compositor_baremetal --bare-metal-mode --include-ignored
```

Before copying anything to the OptiPlex, require all of the following:

```powershell
Get-Item "E:\AI Automation\Models\qwen2.5-0.5b-instruct-q4_k_m.neom", "E:\AI Automation\Models\qwen2.5-0.5b-instruct-q4_k_m.vocab" | Select-Object Name,Length
Get-FileHash -Algorithm SHA256 "E:\AI Automation\Models\qwen2.5-0.5b-instruct-q4_k_m.neom", "E:\AI Automation\Models\qwen2.5-0.5b-instruct-q4_k_m.vocab"
Get-Content target\neo-test\vmware\arthur_compositor_baremetal\manifest.txt
```

The manifest must name:

- `arthur_compositor_baremetal.bare-metal.iso`;
- disk 0 as `role=scenario`, `qemu_backend=ich9-ahci`, `vmware_controller=sata-ahci`;
- disk 4 as `role=log-sink`, `qemu_backend=nvme`, `vmware_controller=nvme`;
- `log_sink_offset=0x10000000` and `log_sink_size=16777216`.

The three deployable files are:

```text
target/neo-test/vmware/arthur_compositor_baremetal/arthur_compositor_baremetal.bare-metal.iso
target/neo-test/arthur_compositor_baremetal.ahci.raw
target/neo-test/arthur_compositor_baremetal.nvme-log.raw
```

Optional but conclusive NeoFS check:

```powershell
docker compose run --rm neo cargo run -p neofs-mkfs -- verify target/neo-test/arthur_compositor_baremetal.ahci.raw
```

Its entry list must contain `model-qwen2.5-0.5b.neom:Model` and
`model-qwen2.5-0.5b.vocab:Regular`.

## Stage from Ubuntu, then hand control to the PL

Use the SSH identity and fixed target from the generic runbook. Copy the three files to `/tmp`, then
on Ubuntu perform the read-only identity check **before** either `dd`:

```powershell
scp target\neo-test\vmware\arthur_compositor_baremetal\arthur_compositor_baremetal.bare-metal.iso target\neo-test\arthur_compositor_baremetal.ahci.raw target\neo-test\arthur_compositor_baremetal.nvme-log.raw ubuntu@<optiplex-ip>:/tmp/
```

```bash
ssh -i ~/.ssh/id_ed25519 ubuntu@<optiplex-ip>
```

```bash
lsblk -d -o NAME,MODEL,SIZE,SERIAL,TRAN
```

Stop unless `nvme0n1` is the internal NVMe boot target and `sda` is the Samsung 860 EVO SATA model
target described by the generic runbook. Once both identities are confirmed, stage in this order:

```bash
sudo -n dd if=/tmp/arthur_compositor_baremetal.bare-metal.iso of=/dev/nvme0n1 bs=4M conv=fsync status=progress
sudo -n dd if=/tmp/arthur_compositor_baremetal.ahci.raw of=/dev/sda bs=4M conv=fsync status=progress
sudo -n dd if=/tmp/arthur_compositor_baremetal.nvme-log.raw of=/dev/nvme0n1 bs=1M skip=256 seek=256 count=16 conv=notrunc,fsync status=progress
sync
```

The third command resets only the 16 MiB evidence region; it must retain both `skip=256` and
`seek=256`. Never write the whole `nvme-log.raw` over the boot disk.

Then:

1. PL confirms PuTTY serial capture is logging to
   `C:\Users\<user>\neoplutarch-serial.log`, clears/archives the prior capture, and watches the
   OptiPlex framebuffer live.
2. Coder triggers the one-shot boot: `sudo efibootmgr -n 0001 && sudo reboot`.
3. PL waits for the composited Arthur greeting, types one short nonempty question, and presses Enter.
   Do not paste control characters and do not inject a virtual/QEMU key.
4. Let the forward finish. A short-prompt 0.5B run is normally about 3–4 minutes; progress is judged
   by breadcrumbs, not impatience. Use ten minutes without any new inference breadcrumb as the
   operator watchdog.
5. PL reads the composited answer live—no photo—and preserves the PuTTY log. Boot-select back to the
   Ubuntu USB as specified by the generic runbook. Coder also reads the backup sink:
   `sudo -n dd if=/dev/nvme0n1 bs=1M skip=256 count=16 2>/dev/null | strings`.

## Ordered serial breadcrumb map

Concurrent `neo-init` and `neo-arthur` writes may share a line. Match the token strings in order; do
not require every token to start on a fresh line.

| Order | Required breadcrumb | What it proves |
|---:|---|---|
| 1 | Limine `Loading module` for `neo-init`, `neo-worker`, `neo-shell`, `neo-arthur` | ISO module paths resolve; no model blob was put in Limine. |
| 2 | `[ kernel ] heap: ready (768 MiB` and framebuffer dimensions | GOP and the showcase heap exist. |
| 3 | AHCI attached, NVMe attached, `bare_metal_log_sink_selftest=ok` | SATA model path and independent durable sink are both live. |
| 4 | `SHOWCASE neofs mount=ok model_blob=out_of_band` | NeoFS mounted from the SATA model disk. |
| 5 | `SHOWCASE scanout gate=armed` | G1 is the only process authorized to scan out. |
| 6 | `SHOWCASE boot framebuffer=ready owner=g1-compositor` then `SHOWCASE compositor up` | G1 owns the inherited GOP framebuffer. |
| 7 | `SHOWCASE surface created role=TEXT client=arthur` | Arthur is a G1 client, not a framebuffer console. |
| 8 | `SHOWCASE bypass client_present=EPERM`, `SHOWCASE seam buffer=client-rw compositor-ro-nx`, then `SHOWCASE scanout compositor_present=allowed` | Client bypass is denied while the compositor can present the imported buffer. |
| 9 | `SHOWCASE greeting composited surface=arthur` | The complete non-silicon render seam is live. |
| 10 | `SHOWCASE memory stage=surface_resident ... used_bytes=...` | First OOM discriminator before the model becomes resident. |
| 11 | `vocab_load tokens=151936`, `model_open ... size=485489152`, `model_load_resident bytes=485489152` | Real vocab and exact real model loaded from NeoFS. |
| 12 | `SHOWCASE memory stage=model_resident ... used_bytes=...` and `SHOWCASE footprint discriminator=model-485MB-plus-compositor` | Named high-water resident combination exists. |
| 13 | `SHOWCASE model resident with_compositor=true`, `SHOWCASE focus activated surface=arthur no_snoop=true`, `SHOWCASE input focus=active client=arthur` | Model, compositor, and the split ACTIVATE/RECEIVE focus authority coexist. |
| 14 | `SHOWCASE keyboard loop=armed source=physical-usb-hid` | Silicon-only boundary is armed. In QEMU the same line ends with `qemu=unsupported fail_closed=wait-for-hid`; stop the smoke there. |
| 15 | One or more `SHOWCASE keystroke ascii=<n>` lines | Arthur itself received the decoded byte through `InputFocus`; this is both the per-key receive and route proof. |
| 16 | `SHOWCASE present commit=accepted damage=client-rect` while typing | Echo updates still flow through G1 commits; since Desktop S2 each commit carries only the band Arthur redrew (the full surface only on the first commit). |
| 17 | `SHOWCASE inference start source=input_focus` then `forward generate enter` | Enter handed the typed prompt from the focus loop into real inference. |
| 18 | `forward0 top5=...`, `sample step=...`, then `decode done tokens=...` | Real forward, generation, and decode make progress. |
| 19 | `reply="..."` then `SHOWCASE inference complete forward=real` | Exact answer bytes exist before presentation. |
| 20 | `SHOWCASE present commit=accepted`, then `SHOWCASE answer composited surface=arthur forward=real serial=0x4152544855520001` | The answer commit was accepted and G1 composited it. |
| 21 | `SHOWCASE surface destroyed`, `model_drop start`, `model_drop done`, `SHOWCASE slice=B pass` | Teardown completed without retaining the model or surface. |
| 22 | `SHOWCASE HID reports=... bytes=0 ep_errors=0 routed_to=input_focus`, pixel/bypass oracles, and terminal `NEO_TEST/1 event=PASS name=arthur_compositor_baremetal` | Physical HID, confinement, scanout, and the one terminal frame all passed. |

## Blind-hardware failure and discriminator bank

| Symptom / last good breadcrumb | Likely boundary | Action and discriminator |
|---|---|---|
| **Top risk: stop during model load or inference with memory pressure** | The 485 MB file, resident tensors, vocab, compositor surface, USB state, KV/scratch, and allocation rounding coexist. | Record the last `SHOWCASE memory` line and its `used_bytes`/`frames_used`. Before `model_load_resident` means file/model residency; after `model_resident` but before `inference start` means the combined idle set; after `inference start` means KV/scratch/forward pressure. Preserve any allocator/OOM/panic line. Do not misclassify a normal 3–4 minute forward as OOM. |
| G1/greeting is present, `model_open` fails or `vocab_load failed` follows | SATA image, wrong artifact, corrupt NeoFS, or missing sidecar—not rendering. | Re-run `neofs-mkfs verify`, compare the two artifact hashes above, and confirm `/dev/sda` identity and full 768 MiB write. |
| Greeting shows; typing does nothing; no `keystroke ascii` | USB enumeration/pump, focus activation, or routing. | If `focus activated` is absent, the focus seam failed. If it is present, look for `hid-keyboard: ready`/xHCI errors. A `keystroke ascii` line is emitted only after Arthur receives the routed byte; there is no separate ambiguous compositor key queue. |
| Printable keys echo, but Enter yields no `inference start` | Enter decode or prompt-buffer boundary. | Confirm an ASCII `10`/newline keystroke and the last echo commit. Retype a short nonempty question and Enter once; do not power-cycle before saving serial. |
| `inference start` then silence | Forward/KV path with compositor resident. | `forward generate enter` missing localizes the userspace-to-kernel handoff. No `forward0 top5` localizes prefill/first forward. A stalled `sample step=N` localizes decode step N. Apply the ten-minute no-progress watchdog and retain the last token line. |
| `inference complete` and `reply="..."` exist, but no `answer composited` | Arthur answer commit, G1 present, or answer serial discriminator. | If `present commit=accepted` is absent, inspect the SurfaceCap commit/reply seam. If present but the answer serial is absent, inspect the G1 serial/registry branch. This is not an inference failure. |
| First client-present attempt changes the framebuffer, bypass oracle fails, or scanout gate is absent | Security regression: Arthur bypassed G1 or G1 lacked `GpuSubmit`. | Red immediately. Do not accept a visually correct answer; preserve the poison/pixel oracle and terminal FAIL. |
| `no_log_sink_device_available` before scenario BEGIN | Wrong disk-controller export or attachment. | Manifest disk 0 must be AHCI and disk 4 NVMe. Confirm the sink self-test before proceeding. The rehearsal found and fixed this exact layout defect. |
| PuTTY stops after one userspace line while the framebuffer changes | Serial/NVMe sink or scheduler concurrency, not necessarily compositor failure. | Read the NVMe sink after returning to Ubuntu. If the sink continues, primary serial capture failed; if both stop at the same line, retain that boundary for CP-1/CP-16 lock/wake analysis. Do not infer success from the screen alone. |
| Kernel panic, page fault, TLB timeout, or terminal FAIL anywhere | Kernel/runtime failure. | Preserve the full primary serial log and sink before another boot. The first fault/timeout line, not the final reset, is the discriminator. |

## Invariants that remain armed

- **Kill switch:** only `arthur_compositor_baremetal` selects the integrated route. Standalone Arthur
  and Atrium scenarios remain unchanged; no model blob enters git or the default ISO.
- **I-14(a):** USB pump, `InputFocus`, SurfaceCap commits, and G1 presents take no AI-governance
  lock. `focus` → `inference start` → `commit` proves the boundary order.
- **I-14(b):** a stalled Arthur owns only its dedicated surface/session. G1 remains framebuffer
  owner and the bounded input queue remains independent; the operator watchdog makes a stall red.
- **I-14(c):** this integration adds no DMA aperture. USB uses its existing fenced path; the shared
  pixel buffer is CPU-mapped client-RW/compositor-RO-NX; SATA/NVMe retain their existing drivers.
- **Concurrency pitfall bank:** treat missing ordered tokens, foreign-lock wake failures, IRQ/IF-off
  waits, and device single-in-flight failures as localized by the table above. Concurrent log writers
  may byte-interleave, so evidence parsing searches token substrings while still requiring their
  semantic order.
