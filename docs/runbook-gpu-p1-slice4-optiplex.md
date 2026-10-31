<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Runbook — GPU P1 Slice 4 on the OptiPlex 7080

**Status:** Stage-A bundles built; JOINT PL+coder silicon run pending. **Created:** 2026-07-22.

This is the Slice-4 specialization of `docs/runbook-baremetal-arthur-optiplex.md`. That runbook remains authoritative for SSH, boot selection, PuTTY, and sink mechanics. This document pins the two GPU images, their order, their evidence, and their failure discriminators. The target is the OptiPlex 7080 at `<optiplex-ip>`, with Intel UHD 630 (Gen9.5). There is **no framebuffer photo**: the durable proof is PuTTY serial at `C:\Users\<user>\neoplutarch-serial.log` plus the NVMe sink readback.

## Roles and stop conditions

- **PL:** physically verifies the OptiPlex and disk identity, keeps PuTTY serial logging, observes the live console, selects the Ubuntu USB when returning from a run, and authorizes each internal-NVMe write.
- **Coder:** verifies hashes, drives SSH/build/log analysis, compares every required breadcrumb, and reports PASS or the first localized FAIL.
- Run 1 is the identity-domain EU proof. **Do not start run 2 unless run 1 reaches its exact PASS line.** A self-diagnosing FAIL is a valid return; do not hide it with another boot.
- Both scenarios intentionally quarantine the GPU in their terminal negative phase. Each scenario therefore needs a fresh boot. Do not attempt both in one kernel boot.

## Pinned local artifacts

Build from the repository root:

```powershell
docker compose run --rm neo cargo xtask neo-test-build accel_intel_blur_silicon --bare-metal-mode
docker compose run --rm neo cargo xtask neo-test-build accel_intel_blur_silicon_narrowed --bare-metal-mode
```

| Run | ISO | Bytes | SHA-256 |
|---|---|---:|---|
| 1 — identity | `target/neo-test/vmware/accel_intel_blur_silicon/accel_intel_blur_silicon.bare-metal.iso` | 121,976,832 | `73A6BDAA9C4718FE0CD6B5401C7770BEC40C2880D88C4780B6B8FC8501D9F519` |
| 2 — narrowed | `target/neo-test/vmware/accel_intel_blur_silicon_narrowed/accel_intel_blur_silicon_narrowed.bare-metal.iso` | 121,976,832 | `AAEE36058906EC4CF2C340F9C260B72D777699661796F27C680A3E68BCFA5BA7` |

The generated raw log disks are 301,989,888-byte zero templates with SHA-256 `79A9AC85947CC141DEC3284A82F59B21AE2B882DB43B3D22BC29657E5B998282`. On physical hardware the sink is the 16 MiB range beginning at internal NVMe offset `0x10000000` (256 MiB); the ISO ends below that offset.

Before transfer, re-run `Get-FileHash -Algorithm SHA256` on both ISOs and stop on any mismatch. The manifests beside the ISOs must say `mode=bare-metal`, `log_sink_offset=0x10000000`, and `log_sink_size=16777216`.

## Common preparation — repeat before each run

1. Boot the Ubuntu live USB. Coder connects with `ssh -i $env:USERPROFILE\.ssh\id_ed25519 ubuntu@<optiplex-ip>`; PL confirms this is the OptiPlex 7080.
2. PL starts PuTTY serial logging to exactly `C:\Users\<user>\neoplutarch-serial.log` and clears/truncates the old session log. Do not append run 2 to run 1.
3. Over SSH, inspect `lsblk -o NAME,SIZE,MODEL,SERIAL,TRAN,FSTYPE,MOUNTPOINTS` and `sudo -n efibootmgr`. PL must explicitly confirm the internal target is `/dev/nvme0n1` and `Boot0001` is the UEFI Intel NVMe. Stop if either differs.
4. Copy only the selected ISO to `/tmp` and verify its SHA-256 on Ubuntu. Preserve the run-specific ISO name so run 1 and run 2 cannot be confused.
5. With PL approval, write that verified ISO to the validated internal NVMe, without truncating the device, then clear only the dedicated sink range:

```sh
sudo -n dd if=/tmp/SELECTED.bare-metal.iso of=/dev/nvme0n1 bs=4M conv=fsync,notrunc status=progress
sudo -n dd if=/dev/zero of=/dev/nvme0n1 bs=1M seek=256 count=16 conv=fsync,notrunc status=progress
sync
```

These writes are destructive to the named internal target. Never substitute an unverified device, wildcard, environment variable, or broad path.

6. Coder arms the one-shot boot with `sudo -n efibootmgr -n 0001`; PL confirms PuTTY is capturing; then `sudo -n reboot`.
7. Let the scenario reach a terminal `NEO_TEST/1 event=PASS` or `event=FAIL`. Do not interrupt a bounded wait because the timeout/floor evidence is part of the gate.
8. PL boot-selects back to the Ubuntu USB. Read the sink without modifying it:

```sh
sudo -n dd if=/dev/nvme0n1 of=/tmp/gpu-slice4-sink.bin bs=1M skip=256 count=16 status=none
strings /tmp/gpu-slice4-sink.bin
```

9. Before another run can overwrite anything, copy the PuTTY log and sink to distinct run-specific names. Record ISO hash, git commit, boot/run label, terminal frame, and whether serial and sink agree.

## Run 1 — identity-domain EU and floor proof

Use `accel_intel_blur_silicon.bare-metal.iso`. Required terminal line:

```text
NEO_TEST/1 event=PASS name=accel_intel_blur_silicon accel_intel_blur_silicon=ok eu_dispatch=ELSP_LRC ppgtt=48bit_context h_only_mid=byte_exact two_pass_dst=byte_exact repeat=stable multigroup_edge=byte_exact groups=2x4 hw_threads=256 floor_masked=no kill_switch=inert forced_observer_timeout=degraded_to_floor device_quarantined=yes narrowed_run=required_separate_boot
```

The terminal line is necessary but retain the preceding discriminators:

- `blur-execlist-context` appears five times with five distinct non-zero `context_id` values, non-zero PML4 roots, `kernel_pages=2`, and `lrc_oracle=true`.
- `blur-walker-kick` reaches 4 before the disabled phase and does not advance while the kill-switch is off.
- H-only reports `mid_copied=yes dst_copied=false`; its host `dst` sentinel remains untouched.
- The next three untouched H→V submissions each retire a seqno and report byte-exact CPU-floor equality: two repeatable 8×8 runs, then the 33×31 partial-edge 2×4 walker grid. `compute_degrades=0` through the acceptance phase.
- The terminal forced-observer timeout has a fifth real ELSP kick, returns the exact CPU floor, increments degrade, retains the DMA working set, and says `device_quarantined=yes`.

Only this exact positive+negative sequence authorizes run 2.

## Run 2 — narrowed-domain positive and real fault negative

Use `accel_intel_blur_silicon_narrowed.bare-metal.iso`. Required terminal line:

```text
NEO_TEST/1 event=PASS name=accel_intel_blur_silicon_narrowed accel_intel_blur_silicon_narrowed=ok eu_dispatch=ELSP_LRC ppgtt=48bit_context h_only_mid=byte_exact two_pass_dst=byte_exact repeat=stable multigroup_edge=byte_exact groups=2x4 hw_threads=256 floor_masked=no graphics_vtd=translated positive_vtd_faults=0 real_igpu_dma=positive out_of_domain_weights=faulted fault_sid=igpu timeout_degrade=floor device_quarantined=yes
```

Retain evidence that:

- graphics narrowing is enabled before the first GPU-reachable allocation and the real iGPU requester is moved to the translated graphics domain;
- H-only, both 8×8 untouched two-pass runs, and the 33×31 multi-group run remain byte-exact under translation; the scenario then drains the hardware VT-d records and requires `positive_vtd_faults=0`, proving the complete positive working set is mapped before the negative is armed;
- the terminal submission allocates only `weights` outside the graphics arena, produces a real VT-d fault, and logs the iGPU source ID and a fault physical address outside the arena bounds;
- the faulting submission has a fifth real ELSP kick but no fifth blur completion, and the service returns the exact CPU floor with a recorded degrade/quarantine.

## Failure localization

| First durable symptom | Primary suspect / next action |
|---|---|
| No ELSP kick | probe, GGTT context mapping, or MMIO submit failure; use `blur-execlist-pre-elsp`, descriptor, and CSB breadcrumbs |
| ELSP kick, PPGTT `RING_FAULT` VA | classify the logged VA against ring/state/kernel/src/mid/dst/seqno/CURBE/weights; fix the missing/wrong leaf, not kernel math |
| No fault, seqno stuck, no output | compare IDD threads=8, walker width-max=7, VFE encoded URB regs=51, EOT/kernel-pages=2, CSB, and `INSTDONE` |
| H-only `mid` wrong | local-ID/GRF layout, lane geometry, pointer ABI, or Q16 kernel math; do not tune the inter-pass barrier yet |
| H-only `mid` correct, two-pass `dst` wrong | H→V visibility: verify the raw `0x7a000004/0x00100020` inter-pass packet and stateless MOCS 0 |
| First two-pass correct, repeat stuck | context identity/tail reuse; verify distinct context IDs and fresh LRC roots |
| Correct `dst` but any degrade or floor token | **not EU proof**; follow the rejected/timeout breadcrumb even though user-visible bytes are correct |
| Narrowed positive faults before terminal negative | graphics-domain inventory incomplete; fault address identifies the missing allocation class |
| Narrowed terminal negative has no VT-d fault | wrong requester/domain, weights accidentally arena-backed, or fault capture broken; do not claim I-14(c2) |

## Honest close statement

QEMU/host evidence validates Rust wiring and byte encodings only. Slice 4 closes only when both separate silicon logs and sink captures above are returned. Until then report: **Stage A built and QEMU-green; UHD 630 EU byte equality and real-iGPU narrowed VT-d containment pending JOINT hardware.**
