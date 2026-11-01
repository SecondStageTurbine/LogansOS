<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Dispatch: a downloadable preview VM image of LogansOS (Seat G)

**Asked by J.c. (2026-11-01):** "Set up option 2, VM first": GitHub builds a ready-to-boot LogansOS VM image and
publishes it as a download on this public repo, so J.c. can try what's been built without building anything on his
PC. He uses Windows and isn't a Linux person: the result must be one download and a few clicks.

## What to build

1. **A workflow, `.github/workflows/preview-image.yml`**, on GitHub-hosted runners (never self-hosted):
   - triggers: `workflow_dispatch`, and a daily schedule that **skips** when `main` hasn't changed since the last
     published preview (compare the SHA recorded in the release);
   - builds the product image the way `cargo xtask vm-image` does (UEFI boot disk + NeoFS data disk), from `main`;
   - packages a Windows zip (below) and publishes it to a **rolling GitHub Release named `preview`** (tag `preview`,
     marked pre-release), replacing the previous asset; the release notes name the `main` SHA, the build date, and
     what's in this build (the merged PR titles since the previous preview);
   - permissions: `contents: write` for this workflow only; nothing else gets a write token.
2. **The Windows package, `LogansOS-preview-<shortsha>.zip`:**
   - the boot disk and the data disk (convert to `qcow2` if it saves space);
   - OVMF firmware files (edk2, BSD-2-Clause-Patent: include its licence text);
   - `Start LogansOS.bat`: finds QEMU for Windows (default install path, or `qemu-system-x86_64.exe` on PATH; a clear
     message with the download link if missing), and boots the disks with **the same machine, devices and options our
     desktop scenarios use** (so what J.c. sees is what the gate tests), in a window, with WHPX acceleration if
     available and a fallback to TCG with a note that it'll be slower;
   - `README.txt`: three steps in plain language (install QEMU from qemu.org once, unzip, double-click), what to try
     (the desktop, Home, talking to Arthur, the session surviving a restart), the keys that matter, and how to quit.
   - **Do not bundle QEMU** (GPL; J.c. installs it once from qemu.org).
3. **Arthur's model:** include the default model if its licence allows redistribution. Arthur's stage-B model is
   **Qwen2.5-0.5B**: verify the licence **at the exact upstream revision you download** (expected Apache-2.0); pin the
   revision and a SHA-256 of the downloaded weights; convert to `.neom` the way the repo already does; put the
   model's licence and a NOTICE (source, revision, conversion) in the zip. If the licence isn't redistributable or
   the size makes the zip impractical (keep it well under 2 GB), publish without the model and say in README how
   Arthur behaves without it. No network fetch at boot: everything the VM needs is in the zip.
4. **A guide, `docs/HOWTO-try-logansos-in-a-vm.md`:** the same three steps with a little more detail, the "what to
   try" list, known limits in a VM (no Wi-Fi; networking only if QEMU's user networking is enabled, which the .bat
   should do), and where to report what you see.
5. **A check that the package actually boots:** a CI step that boots the packaged disks exactly as the .bat does (on
   Linux, same QEMU options) and asserts the desktop session reaches its ready marker, so a broken preview is never
   published. Reuse the existing desktop scenario markers.

## Rules

- Public repo: no personal details in anything you push (scan before every push; the pattern is in your session
  prompt). No real-machine ACPI tables, no inventories.
- The usual close-gate (`scripts/worker-close-gate.sh`), a PR, and a `PM:` comment with the run URL of a
  `workflow_dispatch` run of the new workflow that published a `preview` release, plus the release URL.
- The PM merges; then the PM tells J.c. where to download it.
