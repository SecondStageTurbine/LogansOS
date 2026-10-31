<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Runbook — Bare-metal Arthur on the OptiPlex 7080 (SSH + serial/sink capture)

**Status:** Operational runbook (PL/PM-maintained). **Created:** 2026-07-19.

This is the **authoritative access/boot/capture procedure for every bare-metal Arthur run** on the
OptiPlex 7080. Every bare-metal dispatch MUST reference it (standing rule, `CLAUDE.md` — "Bare-metal
dispatches reference the bare-metal run runbook"), so the run's mechanics are never re-derived under
fire. The *methodology* below (an SSH-reachable control OS on a live USB + the target OS on the
internal disk; `efibootmgr` one-shot to boot the target; serial captured to a fixed log path via PuTTY
on the PL's PC; an on-disk NVMe log sink as backup; run-to-completion; boot-select back; a **joint**
PL+coder run) **generalizes to any bare-metal target** — a different machine (e.g. the M710q Gen9 GPU
target for OT-63 Rung 1) supplies its own concrete parameters against this same shape.

**Target:** OptiPlex 7080 on the build LAN — **`<optiplex-ip>` as of 2026-08-11**. Dual-purpose:
- an **Ubuntu live USB** for control/SSH, and
- **NeoPlutarch** burned to the internal NVMe for the actual Arthur run.

> ⚠ **2026-10-21: the Kingston DataTraveler is no longer the Ubuntu control stick.** It was overwritten
> with the NeoPlutarch image for the Zenbook S14 target (`docs/runbook-baremetal-zenbook-s14.md`).
> An OptiPlex run now needs a freshly written Ubuntu live USB before the SSH steps below work, and the
> `Boot0002` entry below names a stick that no longer exists. Re-check `efibootmgr` after writing the
> new stick and correct the entry number here.

> ⚠ **The address is DHCP-assigned and HAS moved.** This runbook gave an older address from 2026-07-18
> until 2026-08-11, by which point it was wrong and had been copied into two sibling runbooks. Do not
> trust a hardcoded address here — **confirm it before every run** and correct this line when it moves:
> `ping -c1 <optiplex-ip>`, or find it from the PL's PC with
> `arp -a | findstr "<lan-prefix>"`, or from another LAN box with `nmap -sn <lan-subnet>/24`.
> Note the box answers on this address **only while booted into the live USB**; NeoPlutarch itself has
> no sshd, so an unreachable host mid-run usually means it is booted into NeoPlutarch, not that it moved.

## Access (coder, from the build box)
- `ssh -i ~/.ssh/id_ed25519 ubuntu@<optiplex-ip>` — user `ubuntu`, **passwordless sudo** (`sudo -n`).
  Key is `cerebro@<pc-hostname>` (2026-07-18). Reachable **ONLY** while the box is on the Ubuntu
  live USB (NeoPlutarch has no sshd). Host-key churn on a fresh live boot:
  `ssh-keygen -R <optiplex-ip>`.

## Disk layout
- `nvme0n1` = burned **NeoPlutarch boot ISO** (iso9660; EFI `p2` → `/EFI/Boot/BootX64.efi`).
- `sda` (Samsung 860 EVO) = **`NEOFSv1`** volume holding the real model. **On disk the file is
  `model-qwen2.5-0.5b.neom`** (485,489,152 bytes) with sibling `model-qwen2.5-0.5b.vocab`. The
  source GGUF is `qwen2.5-0.5b-instruct-q4_k_m.gguf`, and this runbook previously named the volume
  file after the *source* rather than the *staged* name — the staged name is what
  `xtask` writes and what `neo-arthur` opens (`/neofs/model-qwen2.5-0.5b.neom`), per
  `Plans/DISPATCH-2026-07-18-baremetal-arthur-optiplex-cerebro.md` ("staged as `model-qwen2.5-0.5b.neom`").
  Verified against the physical volume 2026-08-11.

  > ⚠ **`/model.neom` (4,100 B) and `/hello.txt` (37 B) are LOAD-BEARING — do not delete them.**
  > They look exactly like leftover `neofs-mkfs` scaffolding and are not. `validate_children`
  > (`kernel/src/fs/neofs/mount.rs`) **fails the mount** without a root entry literally named
  > `model.neom` of kind `Model` (`UnsupportedType`) or without `hello.txt` of kind `Regular`
  > (`NotFound`). The real 485 MB model does **not** satisfy the `model.neom` name check — the
  > 4,100-byte stub is the only thing that does. Removing either during disk prep kills rung 5
  > with an error that names neither file.
  >
  > Full root listing as of 2026-08-11 (`tools/neofs-inspect.py`): `hello.txt` (37),
  > `model.neom` (4,100), `model-qwen2.5-0.5b.neom` (485,489,152),
  > `model-qwen2.5-0.5b.vocab` (7,990,862), `subdir` (0). Extent 768 MiB, 535.6 MiB used,
  > all three superblocks valid at gen=1, `root_inode_hash` matches.
  > **No `.arthur/memory`** — that is the item-2 disk-prep question.
- **NVMe log sink** at `nvme0n1` **offset 256 MiB (`0x10000000`)**, magic `NEO_LOG_v1`, ~16 MiB —
  NeoPlutarch mirrors its serial here. Read back (after the box returns to Ubuntu):
  `sudo -n dd if=/dev/nvme0n1 bs=1M skip=256 count=16 2>/dev/null | strings`
  - **Since the Zenbook lane (2026-10-21) the sink writes ONLY over a pre-provisioned, sealed
    header.** The kernel reads the region first and refuses (`block-log-sink: REFUSED: no sealed
    NEO_LOG_v1 header ...`) when it is absent or unsealed, then runs on without a sink. The block write
    guard (`kernel/src/drivers/block/write_guard_logic.rs`) arms ONLY that 16 MiB region on this disk,
    and only because it sits in space no partition (the burned ISO's included) allocates.
  - **The header is v3 since the write-guard hardening (2026-10-21): the old 11-byte `printf` header no
    longer arms anything.** Re-provision once, from Ubuntu, **on this sink disk only**:
    1. on the build box: `cargo xtask log-sink-header neo-log-sink-header.bin` (4096 bytes: magic,
       16 MiB region size, version 3, BLAKE3 seal), and copy the file to the control OS. It writes only a
       regular file: it refuses a device path, and an existing file without `--force`;
    2. `sudo dd if=neo-log-sink-header.bin of=/dev/nvme0n1 bs=4096 seek=$((0x10000000 / 4096)) conv=notrunc,fsync`.

    An ISO reburn (~120 MB) ends far below 256 MiB, so the header survives it.

    The boot NVMe carries the ISO's GPT, so the sink sits in space that table leaves free. A sink on a disk
    with **no** partition table arms only if that disk's first MiB is blank (all zero). This guards
    against a sealed header copied onto someone's unpartitioned volume. A brand-new disk qualifies; a
    reused one does not, and there the header alone is refused with `table=none-start-in-use`.

## Boot control (efibootmgr)
`Boot0000`=Windows, **`Boot0001`=UEFI INTEL NVMe = NeoPlutarch**, `Boot0002`=Kingston USB=Ubuntu
(**stale since 2026-10-21**: the Kingston stick is now the Zenbook image; see the note above).
Trigger the run: `sudo efibootmgr -n 0001 && sudo reboot` (one-shot). BootOrder is `0000,0001,0002`,
so after the one-shot NeoPlutarch boot the box falls back to **Windows** unless boot-selected —
returning to Ubuntu/SSH needs a boot-select to the USB. **This makes the run JOINT** (the coder drives
SSH/analysis; the PL is physically at the box for the boot-select, the live on-screen read, and the
serial terminal).

## The deliverable — the SERIAL LOG, not a photo (standing correction, 2026-07-19)
**The durable proof artifact is the serial log (+ the NVMe sink backup) — there is NO framebuffer
photo. The PL does not take photos and never has** — do not write "photograph the screen" into a
bare-metal dispatch or ask for a photo in a RETURN. On-screen legibility is established by (a) the PL's
**live visual read** at the box during the joint run (eyes on the physical framebuffer) plus (b) the
serial log's `REPLY: <text>` line — and the framebuffer console renders the exact bytes it writes to
serial, so a legible serial reply on a proven-non-blank framebuffer IS the legibility proof. The log is
conclusive on its own.

## Serial capture (primary channel)
The PL runs **PuTTY as a serial terminal** off the OptiPlex COM port (on the **PL's** PC — that is why
the Ubuntu box has no `/dev/ttyUSB*`) and saves the session to
**`C:\Users\<user>\neoplutarch-serial.log`** on the build box. That file is the coder's analysis input
(**same path every run**) **and the deliverable**. The on-disk NVMe sink is the **backup** channel.

## Run procedure (joint PL + coder)
1. **PL:** at the OptiPlex, PuTTY serial connected + logging to the file; watching the screen.
2. **Boot NeoPlutarch:** coder `sudo efibootmgr -n 0001 && sudo reboot` over SSH, or PL `F12` → NVMe.
3. NeoPlutarch **auto-greets + auto-runs the forward pass** (no keyboard needed).
   **LET IT RUN TO COMPLETION — ~3-4 min** (0.5B, prefill ~22 tok + gen 32 × ~4 s/tok, scalar
   single-threaded). Watch serial: `INFER: pos=0..21` (prefill) → `sample step=0..31` → `REPLY: <text>`.
   *(Do not halt early — the ~3-4 min is expected linear generation, not a hang; see the OT-28 premise
   correction. A true hang is distinguished by the per-stage breadcrumbs stalling, not by wall-clock.)*
4. **PL:** confirm the full reply is legible on the framebuffer by **eye** (live — no photo) and **save
   the serial log** (the durable proof). The NVMe sink is the backup copy.
5. **Boot-select back to the Ubuntu USB;** coder reads the log/sink → confirms the full reply.

## Notes
- The run is **JOINT by construction** (boot-select + physical serial + the live on-screen read are the
  PL's; SSH control + log analysis are the coder's) — a bare-metal dispatch names both roles.
- The serial breadcrumbs + the NVMe sink are the blind-hardware discriminator channels: a bare-metal
  dispatch's failure-mode/discriminator bank maps its expected failures onto these two channels (screen
  = human-facing; serial/sink = diagnostic), per the blind-hardware standing rule.
- **Do not modeset** (inherit Limine's GOP, ADR-032) — the framebuffer is expendable during compute.
