<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Showcase Slice B dress rehearsal and JOINT-run prep

**Date:** 2026-07-21
**From:** Jarvis
**To:** PM / PL
**Status:** **RUN-READY** — assembly, model staging, and the non-silicon QEMU prefix are proven. No bare-metal boot was performed.

## Result

The never-before-combined `arthur_compositor_baremetal` image compiles and links with the G1 compositor, physical USB-HID route, and Arthur real-inference path. The exact final ISO booted in QEMU through Arthur-as-G1-client creation, bypass denial, composited greeting, real vocab/model residency, and InputFocus activation. It then stopped at the explicit physical-HID-only fail-closed boundary, as intended.

The turnkey JOINT-run procedure is in `docs/runbook-sliceB-arthur-showcase-optiplex.md`. Do not boot the OptiPlex until the PL signals.

## Rehearsal defects found and fixed

1. The bare-metal bundle classifier exported every Slice-B disk as virtio-blk. The runtime therefore had no registered independent log sink and failed before scenario dispatch with `reason=no_log_sink_device_available`. Slice B now uses disk 0 as SATA/AHCI NeoFS and disk 4 as the NVMe log sink, matching the already-proven transformer bare-metal topology. A regression test freezes that mapping.
2. Required-real-model selection checked the `.neom` but not its adjacent vocabulary. A build could therefore assemble an image that failed only after boot. Assembly now rejects a missing, empty, or non-file `.vocab` sidecar.
3. The hardware-only keyboard loop had no explicit QEMU discriminator. It now emits `qemu=unsupported fail_closed=wait-for-hid` immediately before waiting for physical USB-HID input.

All changes are in `tools/xtask` or `userland/neo-init`; no kernel file changed. Per dispatch, the close gate and four-shard kernel QEMU gate are not required.

## Exact assembly and staged artifacts

Base: `01511c717c2a938b51164994eb0d10a7650683cf` (`origin/main` after the final pre-publication fetch and rebase).

```powershell
docker compose run --rm -v "E:\AI Automation\Models:/models:ro" -e NEO_STAGE_B_NEOM=/models/qwen2.5-0.5b-instruct-q4_k_m.neom -e NEO_SMP=4 neo cargo neo-test --build-only arthur_compositor_baremetal --bare-metal-mode --include-ignored
```

Result: PASS; all kernel and userspace modules compiled and linked.

Model source, intentionally outside git:

```text
qwen2.5-0.5b-instruct-q4_k_m.neom
  bytes  485489152
  sha256 b934a7149ede095d9b11e58e5aae9411a233d578e6d772f1eed205a54122ffc9
qwen2.5-0.5b-instruct-q4_k_m.vocab
  bytes  7990862
  sha256 24c573c024cb0552ddd503f5945e1b5b77351de94c1e280963b4427a9344e15a
```

The model and vocab are stored on the 768 MiB SATA NeoFS image, not copied into Limine or git. Limine carries only `neo-init`, `neo-worker`, `neo-shell`, and `neo-arthur`. `neofs-mkfs verify` passed with image size `805306368`, content hash `3a2c4f70978c5c3db6cf8dfb5e2500d8fa76c0308400ff85fbff23f7e2f847aa`, and entries `model-qwen2.5-0.5b.neom:Model` plus `model-qwen2.5-0.5b.vocab:Regular`.

Final QEMU-tested ISO:

```text
bytes  124258304
sha256 084540048fc4dff5aab997537b158103a66afea398a352a4e729af301e452afc
```

## Exact final QEMU smoke

The final ISO was booted with the production controller split (AHCI scenario disk plus NVMe evidence sink), 4096 MiB RAM, and one emulated CPU. One CPU is deliberate for the non-silicon prefix: an earlier four-vCPU TCG smoke attached both disks and initialized the sink but stopped producing serial after the first userspace boot line without panic/FAIL. That observation is recorded in the runbook's concurrency discriminator bank; it is not evidence about the four-core OptiPlex.

Final serial log: `target/neo-test/arthur_compositor_baremetal.qemu-smoke-main01511c71-20260721.serial.log`

```text
bytes  7328
sha256 1eaa8cfd85b2dc53c861dddbbadeec7ac5f5348396aee0f23be68c96741ed44b

SHOWCASE greeting composited surface=arthur compositor_event_bytes=0
vocab_load tokens=151936 bytes=7990862
model_open path=/neofs/model-qwen2.5-0.5b.neom size=485489152
model_load_resident bytes=485489152 ms=0
SHOWCASE memory stage=model_resident used_bytes=1651732480 frames_used=403255 cores=1
SHOWCASE footprint discriminator=model-485MB-plus-compositor source=qwen2.5-0.5b
SHOWCASE model resident with_compositor=true source=neofs
SHOWCASE focus activated surface=arthur no_snoop=true
SHOWCASE keyboard loop=armed source=physical-usb-hid qemu=unsupported fail_closed=wait-for-hid
```

There was no panic and no `NEO_TEST/1 event=FAIL`. QEMU was intentionally stopped after the final line. No virtual keystroke or inference was attempted; keyboard → real forward → composited answer remains the JOINT silicon capstone.

## Validation

- `docker compose run --rm neo cargo test -p xtask arthur_compositor_baremetal -- --nocapture` — 2 passed, 0 failed.
- Full integrated bare-metal build command above — PASS.
- `neofs-mkfs verify target/neo-test/arthur_compositor_baremetal.ahci.raw` — PASS.
- Exact final ISO QEMU boot-smoke — PASS to the physical-HID fail-closed boundary.
- `git diff --check` — PASS.

## JOINT-run handoff

Follow `docs/runbook-sliceB-arthur-showcase-optiplex.md` exactly. Its first risk is the named 485 MB model-plus-compositor memory footprint, and it contains the complete ordered breadcrumb map, blind-hardware failure/discriminator bank, kill switch, I-14(a/b/c), and concurrency-pitfall checks. The PL's signal authorizes the fresh rebuild and JOINT OptiPlex boot; this rehearsal does not.
