<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Runbook: bare-metal M710q (Gen9 EU) — Slice-4 GPU silicon target

**Status:** authoritative for any bare-metal run on the **M710q** (the designated Gen9 EU GPU target). Sibling to `docs/runbook-baremetal-arthur-optiplex.md` (the Arthur archetype) — the *methodology* is identical; this doc supplies the M710q's concrete params. A bare-metal M710q dispatch references THIS runbook the way an Arthur dispatch references the OptiPlex one.

**Why this box exists:** it is the spare Gen9 iGPU (Intel HD 530, Gen9 — same EU-ISA family as the OptiPlex UHD 630) that we can afford to brick, so blind from-scratch EU bring-up (Slice-4 byte-equality) never risks the OptiPlex that carries Arthur + WiFi. Verified up 2026-07-27.

---

## 0. Rig facts (verified 2026-07-27 — re-inventory before each run; do NOT assume)
- **Model:** Lenovo ThinkCentre M710q, i5-6500T (Skylake, 4c/4t), **Intel HD 530 (Gen9)**, 16 GB.
- **Control OS:** Ubuntu 24.04 live USB (kernel 6.11), root is an overlay `/cow` (not the SSD). SSH: `ssh ubuntu@<m710q-ip>` with the PL's `C:\Users\<user>\.ssh\id_ed25519`. (Host-key churn on a fresh live boot: `ssh-keygen -R <m710q-ip>`.)
- **Firmware:** UEFI, efivars present. `BootCurrent=0007` = the Kingston live USB; `BootOrder` starts `0007,…` so a plain reboot returns to Ubuntu.
- **⚠ VT-d MUST BE ENABLED IN FIRMWARE — hard prerequisite for any GPU run (PM decision, 2026-08-01).** Since the Slice-4 I-14(c) fix, **GPU activation fails closed until the graphics IOMMU domain is armed**: no VT-d ⇒ containment cannot arm ⇒ the iGPU refuses bus mastering and stays decode-only. On an M710q with VT-d disabled there is **no GPU**, by design — that is I-14(c) working, not a bug. The boot still succeeds and the CPU compute floor still works (degrade, never fail), and the serial log carries a `ContainmentNotArmed` breadcrumb naming the matched device ID — but **the EU byte-equality run cannot happen**. **Check this before the run, not after:** in setup, enable `Intel Virtualization Technology for Directed I/O` (VT-d) — it is a *separate* switch from plain VT-x on Lenovo firmware and is frequently off by default. Verify from the control OS with `dmesg | grep -i 'DMAR\|IOMMU'` (expect `DMAR: IOMMU enabled`); an absent DMAR table means the firmware switch is off. Rationale: `Plans/DECISION-2026-08-01-slice4-i14c-fail-closed-cerebro.md`.
- **Disks:**
  - **`/dev/sda` — Samsung SSD 850 EVO 250 GB, SATA/AHCI (WWN `5002538d4070972f`).** This is the **NeoPlutarch target: boot ISO + log sink.** Currently a single scratch MBR/NTFS "New Volume" (~1.2 MB of leftover Windows `$RECYCLE.BIN`/`System Volume Information` metadata — **disposable**, PL-confirmed). We overwrite the whole disk.
  - **`/dev/sdb` — Kingston DataTraveler 3.0 USB (the live control OS).** **NEVER a dd target — dd'ing it destroys the control OS.**

> ### ⚠ DEVICE-NAME DANGER — INVERTED vs the OptiPlex. READ THIS.
> On the OptiPlex, `/dev/sda` is the precious **model volume** you must never touch, and the NVMe is the target. **On the M710q it is the opposite: `/dev/sda` IS the dd target and there is NO NVMe.** The protected disk here is `/dev/sdb` (the USB). Applying OptiPlex muscle-memory ("never dd sda") is exactly wrong here and applying it the other way ("dd sda freely") on the OptiPlex is catastrophic. **Always identity-check by model/WWN before any dd** (`lsblk -o NAME,MODEL,WWN,SIZE`), never by bare device name.

---

## 1. Capture channels — adapted (no serial, no NVMe)
The M710q has **no serial port** and **no NVMe**, so neither OptiPlex capture channel exists natively. Adaptation:
- **Primary (durable): the disk log sink on `/dev/sda`** at offset **256 MiB** (`NEO_LOG_v1` magic). The sink is disk-agnostic raw block writes; NeoPlutarch reaches SATA via its existing **AHCI** driver (the same driver that hosts the Arthur model volume on the OptiPlex). Read it back from the live USB: `sudo dd if=/dev/sda bs=1M skip=256 count=16 2>/dev/null | strings`.
- **Secondary (the "glass" proof): framebuffer, PL live eyes-on.** Slice-4 EU is a *glass GPU* proof, so the composited on-screen output is a real part of the deliverable. As on Arthur: **NO framebuffer photo** — the PL reads legibility live; the durable PASS/FAIL + discriminator counters come from the sink.

> **Build implication (goes in the Slice-4 dispatch):** the M710q ISO's **log-sink disk role must target the AHCI/SATA disk** (`/dev/sda`), not NVMe. Confirm in the build manifest (`manifest.txt`: the log-sink disk role = the AHCI disk, `log_sink_offset=0x10000000`, `size=16777216`).

---

## 2. Stage the ISO + sink to `/dev/sda` (from the live USB, over SSH)
```
# identity-check FIRST — confirm sda is the Samsung SSD, sdb is the Kingston USB:
lsblk -o NAME,MODEL,WWN,SIZE,TRAN
# clear the sink region BEFORE staging:
sudo dd if=/dev/zero of=/dev/sda bs=1M seek=256 count=16 conv=notrunc,fsync ; sync
# copy the artifacts up (scp -O — the live session lacks SFTP):
scp -O <build-host>:/path/<scenario>.bare-metal.iso <build-host>:/path/<scenario>.disk-log.raw ubuntu@<m710q-ip>:/tmp/
# write the ISO to the WHOLE disk, then the sink at 256 MiB:
sudo dd if=/tmp/<scenario>.bare-metal.iso of=/dev/sda bs=4M conv=fsync status=progress
sudo dd if=/tmp/<scenario>.disk-log.raw of=/dev/sda bs=1M skip=256 seek=256 count=16 conv=notrunc,fsync status=progress
sync
```

---

## 3. Create the UEFI boot entry (first stage only) + one-shot boot
The Limine hybrid ISO carries an EFI System Partition with the bootloader. After dd, discover it and register a one-shot-able entry:
```
# discover the ESP the dd'd ISO exposes on sda (the FAT/EFI partition + the .EFI path):
lsblk -f /dev/sda                        # find the vfat/ESP partition number (e.g. sda2)
sudo mount -o ro /dev/sda2 /mnt && sudo find /mnt/EFI -iname '*.efi'; sudo umount /mnt   # confirm the EFI binary path
# create the boot entry (fill -p <espN> and -l with the confirmed path, e.g. \EFI\BOOT\BOOTX64.EFI):
sudo efibootmgr -c -d /dev/sda -p <espN> -L "NeoPlutarch-EU" -l '\EFI\BOOT\BOOTX64.EFI'
# efibootmgr -c prepends the new entry to BootOrder — RESTORE the USB (0007) to first so a post-run reboot returns to Ubuntu:
sudo efibootmgr -o 0007,<newid>,0005,0006,0001,0000
```
Then, every run (steady state):
```
sudo efibootmgr -n <newid> && sudo reboot     # one-shot: boots NeoPlutarch once, then BootOrder[0]=USB on the next reboot
```
Note the assigned `<newid>` from `efibootmgr -v` and reuse it. `BootNext` (`-n`) is consumed after one boot, so **the machine returns to the Ubuntu live USB automatically** on the reboot NeoPlutarch triggers at scenario end — no boot-select-back needed (unlike the OptiPlex).

---

## 4. Run + capture (JOINT — PL at the box)
1. **PL:** monitor on the M710q for the live glass read (framebuffer legibility).
2. `sudo efibootmgr -n <newid> && sudo reboot`. NeoPlutarch boots from `/dev/sda`, runs the scenario, writes the sink, reboots → back to Ubuntu.
3. **Let it run to a bounded terminal** — do not power-cycle because a stage looks stalled; the scenario self-terminates with diagnostics.
4. **Back on the live USB (auto), read the sink:** `sudo dd if=/dev/sda bs=1M skip=256 count=16 2>/dev/null | strings` → the `NEO_LOG_v1` record with the `NEO_TEST/1 event=PASS/FAIL`, the byte-equality result, and the discriminator counters.

---

## 5. What a Slice-4 EU PASS looks like (acceptance)
The sink must show the full chain, not just a terminal verdict:
`EU compute path executed (BLUR_WALKER_KICKS>0) → BLUR_COMPLETIONS==KICKS (EU retired, no hang) → output byte-identical to the CPU floor → fence_refusals=0 (every EU DMA buffer went through dma_arena::fence under IOMMU containment)`, ending with `NEO_TEST/1 event=PASS name=<slice4 scenario>`. **A green byte-equality with `BLUR_WALKER_KICKS==0` is VACUOUS** (the EU never ran; it silently fell back to CPU) — reject it. The framebuffer glass read corroborates but the sink is conclusive.

---

## 6. Failure-mode / discriminator bank (blind hardware — the run must be conclusive pass OR fail)
| Observation | Cause | Discriminator |
|---|---|---|
| Sink empty / no `NEO_LOG_v1` | ISO didn't boot, or sink wrote to the wrong disk | re-confirm the boot entry fired (framebuffer showed NeoPlutarch), and the manifest's log-sink role = the AHCI disk |
| `BLUR_WALKER_KICKS==0`, output still matches | EU never dispatched — silent CPU fallback | vacuous pass — the kick counter MUST be nonzero AND equality hold |
| `BLUR_WALKER_KICKS>0`, byte-mismatch | real EU compute bug | capture the first divergent byte offset |
| `KICKS>0`, `COMPLETIONS<KICKS` | EU dispatched, didn't retire (hang) | capture walker/DMA-fence state |
| `fence_refusals>0` | an EU DMA buffer bypassed `dma_arena::fence` | I-14(c) containment signal — a governance event, not an EU bug |
| Black screen, sink has PASS | framebuffer/display init issue, compute fine | sink PASS on a blank screen ⇒ display path, not compute |
| `candidates=0` | the iGPU's PCI ID is not in the accel allowlist | the scan now prints `Intel display device NOT in the Gen9 accel allowlist … device=0x….` — read the ID off that line (run 1 lacked this and the ID had to be recovered by hand) |

### 6a. Skylake-vs-Gen9.5 traps (prior-art mine, i915 + Intel compute-runtime, raw source)

The M710q is **HD 530 `[8086:1912]`, Skylake Gen9**; every prior silicon run was **UHD 630, Coffee/Comet
Lake Gen9.5**. What actually differs, mined from raw upstream source (not summarized):

**What is provably identical — do not spend a cycle here.**
- **GT config.** compute-runtime `shared/source/dll/devices/devices_base.inl` @ the pinned finalizer
  `24.26.30049.6`: `0x1912 → SklHw1x3x8`; `0x9BC8`/`0x3E92 → CflHw1x3x8`. **Both `1x3x8` = 24 EU**, and
  both compute `ThreadCount = EUCount * 7` (`hw_info_skl.cpp` / `hw_info_cfl.cpp`). Our hardcoded
  `MEDIA_VFE_STATE` max-threads **167** (bias-1 of 168) is therefore correct on **both**.
- **Platform capabilities.** i915 `i915_pci.c`: `skl_gt2_info` and `cfl_gt2_info` are literally the same
  definition — `GEN9_FEATURES, .gt = 2` — differing only in the `PLATFORM()` enum tag. GGTT/PPGTT sizing,
  engine mask, and MMIO layout do not diverge. Our GTTMMADR (BAR0, 16 MiB, GGTT PTEs at +8 MiB), the
  render-ring base `0x2000`, and the Gen9 forcewake registers are Gen9-wide, not Gen9.5-only.
- **EU ISA.** The embedded kernels were compiled `ocloc … skl` (see `intel_gen9_kernels/README.md`) — the
  binaries are natively targeted at **this** SKU. The M710q is the *closer* target; the OptiPlex was the
  further one. Byte-equality is expected to transfer.
- **VT-d / I-14(c2).** `0x1912` is **absent** from the Linux `quirk_iommu_igfx` list
  (`drivers/iommu/intel/iommu.c`), which covers gen4 (`0x2a40`, `0x2e0x`) and Broadwell (`0x16xx`) only.
  Skylake iGPU DMA remapping is not quirked off, so the graphics VT-d domain is expected to behave as on
  the OptiPlex. `fence_refusals>0` remains a containment signal, not an EU bug.
- **`RING_R_PWR_CLK_STATE`** is only ever *read* by our driver, never programmed — so no slice/subslice/EU
  enable value is hardcoded to a GT config.

**The real deltas — first suspects if the M710q hangs where the OptiPlex did not.** We implement *none* of
i915's workarounds on either SKU, and Gen9.5 worked without them; these are the ones i915 applies to SKL
and **skips** on CFL/CML, so they are the only SKU-shaped hypotheses.

| i915 workaround (SKL) | Register | Why it is a suspect | Discriminator |
|---|---|---|---|
| `WaDisableKillLogic:bxt,skl,kbl` | `GAM_ECOCHK |= ECOCHK_DIS_TLB` | **The sharpest delta** — `gen9_gt_workarounds_init` applies it under `if (!IS_COFFEELAKE && !IS_COMETLAKE)`, i.e. exactly on SKL and *not* on our proven silicon. GAM/TLB path. | A ring/TLB fault or hang on M710q where the identical build passed on the OptiPlex ⇒ try this first. Capture `RING_FAULT` + `FAULT_TLB_DATA0/1` (already captured). |
| `WaInPlaceDecompressionHang:skl` | `GEN9_GAMT_ECO_REG_RW_IA` | Hang-class, gated `STEP_A0..STEP_H0` — effectively every SKL stepping. | Hang with no fault recorded; correlate with the newly-logged `rev=`. |
| `WaDisableGafsUnitClkGating:skl` | `GEN7_UCGCTL4` | Clock-gating stability; applied on SKL/KBL, not CFL/CML. | Intermittent/heat-dependent hang rather than a deterministic one. |
| `skl_tune_iz_hashing` | `GEN7_GT_MODE` | **Explicitly NOT a suspect** — it tunes Intermediate-Z hashing for the 3D rasterizer. Our path is GPGPU-only. Recorded so nobody chases it. | n/a |

**Stepping matters more on SKL than on Gen9.5** (i915 gates several Gen9 W/As on `IS_GRAPHICS_STEP`), which
is why the PCI scan now logs `rev=0x..` for the iGPU — a stepping-attributable failure is otherwise blind.

**Honest bound:** these are *hang/stability* workarounds, not functional prerequisites for a single GPGPU
dispatch, and none is known to be required for our minimal path. The trap-mine narrows where to look on a
failure; it does not predict one.

---

## 7. Post-run
- Save the sink readback text (the durable proof) to the PL's capture path.
- The box is already back on Ubuntu (auto). To re-stage, repeat from §2.
- **Restore note:** `/dev/sda` now holds the NeoPlutarch ISO, not the old NTFS volume — that's expected and fine (the box is dedicated GPU-target duty; the NTFS leftover was disposable).
