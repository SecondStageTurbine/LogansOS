<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# How to try LogansOS in a virtual machine (Windows)

LogansOS publishes a ready-to-boot virtual machine as one download. You don't
build anything: install QEMU once, unzip, double-click. Nothing on your PC is
changed; LogansOS runs in a window.

The download is on the project's **`preview` release**:
<https://github.com/SecondStageTurbine/LogansOS/releases/tag/preview>. It is
rebuilt from `main` every day that `main` changes, and only published after the
packaged disks have booted to the desktop session's first screen in CI.

## The three steps

1. **Install QEMU, once.** QEMU is the free program that runs the virtual
   machine. Go to <https://www.qemu.org/download/#windows>, follow the link to
   the 64-bit Windows installers, download the newest `qemu-w64-setup-….exe`
   and run it. Keep the default folder, `C:\Program Files\qemu`, so the start
   file can find it. (QEMU isn't in the download: its licence is GPL, so it is
   installed from its own site.) QEMU from MSYS2 also works
   (`pacman -S mingw-w64-ucrt-x86_64-qemu`): the start file looks in
   `C:\msys64\ucrt64\bin` (and `mingw64`, `clang64`) too.
2. **Download and unzip.** On the release page, download
   `LogansOS-preview-<version>.zip` (around half a gigabyte). Right-click it and
   choose **Extract All**.
3. **Double-click `Start LogansOS.bat`** in the unzipped folder.

A LogansOS window opens. The first start takes a few minutes. If Windows asks
whether to run the file ("Windows protected your PC"), choose **More info**,
then **Run anyway**: the file only starts QEMU, and you can open it in Notepad
to read it.

**If the window stays black for more than ten minutes,** close it and
double-click **`Start LogansOS (compatibility mode).bat`** instead. The normal
start uses Windows' own virtualization (Windows Hypervisor Platform) when it is
turned on, and falls back to QEMU's built-in emulator by itself when it isn't.
Compatibility mode always uses the built-in emulator: slower, but it is exactly
how the build checks boot the preview.

## What to try

- **Make an account.** The first screen asks for a password. Write down the
  recovery code it shows you.
- **The desktop.** The frosted bar along the top, the Spine down the left side.
- **Home.** Type a question in the field in the middle and press Enter.
- **Talk to Arthur.** He opens and answers. In a virtual machine he is slow: an
  answer can take a few minutes (more in compatibility mode). The busy emblem
  means he's still thinking. **Esc** goes back to Home; Arthur stays open.
- **The session survives a restart.** Close the window and double-click
  `Start LogansOS.bat` again: your account and Arthur's memory are still there.
  They live in `disks\data.qcow2`. To start over, unzip the download again into
  a new folder.

Arthur's model is Qwen2.5-0.5B-Instruct (Apache-2.0). The release notes say
whether a build includes it; the zip's `licenses` folder carries its licence and
a NOTICE saying where it came from and how it was converted.

## Keys that matter

| Key | What it does |
|---|---|
| Click inside the window | LogansOS gets your keyboard and mouse |
| **Ctrl+Alt+G** | Get your mouse back from the window |
| **Ctrl+Alt+F** | Full screen on and off |
| **Esc** (in Arthur) | Back to Home |
| **Tab** | Move between Home's field and the Spine |
| **Windows key+L** | Lock the session (Windows may take it for itself first) |

## How to quit

Close the LogansOS window (the X in its corner). Nothing keeps running
afterwards.

## Known limits in a virtual machine

- **No Wi-Fi**, and in practice no network: the `.bat` attaches QEMU's user
  networking, but this build has no driver for the network card QEMU emulates,
  so LogansOS stays offline in the VM.
- **No graphics acceleration**: the screen is drawn by the processor, so moving
  windows is less smooth than on real hardware.
- **Arthur is slow** (see above), much slower than on a real PC.
- **Windows key shortcuts** may be caught by Windows before they reach the
  window.

## Where to report what you see

Open an issue at <https://github.com/SecondStageTurbine/LogansOS/issues>. Say
which preview you ran (its version is in the zip's name and at the top of
`README.txt`), whether you used the normal or the compatibility start, and what
you saw. Attach `logansos-serial.log` from the unzipped folder: it is
LogansOS's own log of that run.

## For developers

The workflow is `.github/workflows/preview-image.yml`: GitHub-hosted runners, a
daily schedule that skips when `main` hasn't moved since the published preview,
and `workflow_dispatch`. It builds the disks with
`cargo xtask vm-image --with-model` (the product image, as `cargo xtask vm-run`
boots it), converts them to qcow2 and packages the zip with
`scripts/preview/preview.py`. That script holds the one QEMU command line both
the `.bat` files and the boot check use; the boot check differs only in
`-display none` and `-accel tcg`, and passes when the serial log shows
`LOGIN first-boot shown` with no kernel panic. Only the publish job has a write
token.

The model is fetched from its pinned upstream revision and checked from raw
bytes before anything is built: the repository's `LICENSE` is present and is
the Apache License 2.0 file with a pinned SHA-256, the model card says
`apache-2.0`, and the GGUF's SHA-256 matches the pin. xtask then holds the
converted `.neom` and vocabulary to the kernel's own pins. A mismatch fails the
run and nothing is published.
