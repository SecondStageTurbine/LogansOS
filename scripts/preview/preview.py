#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""The downloadable preview VM (Plans/DISPATCH-2026-11-01-preview-vm-image.md).

One place holds the QEMU command line the Windows package boots with, so the
`.bat` files J.c. double-clicks and the CI boot check that blocks a broken
preview cannot drift apart. Subcommands:

  package     lay the zip's tree out from the `cargo xtask vm-image` disks
              (converted to qcow2 by the caller), the firmware, the licences
              and the generated `.bat`/README, then write the zip.
  boot-check  unzip a package into a scratch directory and boot it exactly as
              `Start LogansOS.bat` does (same arguments; only the window and
              the accelerator differ: `-display none`, `-accel tcg`), and pass
              when the desktop session reaches its ready marker.
  qemu-args   print the shared arguments (one per line), for reading.

The machine matches `cargo xtask vm-run` (OT-87 R1): q35, `-cpu max`, 4 GiB,
4 CPUs, `-vga std`, the pinned OVMF as pflash (code read-only, a fresh copy
of the variable store each start), the boot disk then the data disk on
virtio-blk, and the desktop's USB keyboard and mouse on qemu-xhci
(`desktop_input_devices` in tools/xtask). The disks are qcow2 here instead of
raw. QEMU's user networking is attached as the dispatch asks; the preview
kernel has no driver for it, so the VM is offline (the guide says so).
"""

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
import time
import zipfile
from pathlib import Path

# Paths inside the package, relative to its folder (the `.bat` `cd`s there,
# so QEMU never sees the user's own path, spaces and all).
BOOT = "disks/boot.qcow2"
DATA = "disks/data.qcow2"
OVMF_CODE = "firmware/edk2-x86_64-code.fd"
OVMF_VARS = "firmware/edk2-i386-vars.fd"
VARS_RUN = "firmware/vars-this-run.fd"
SERIAL_LOG = "logansos-serial.log"

# The session's ready marker: the desktop session (the WM) has drawn the
# first-boot account screen. A fresh data disk has an empty `.system/`, so
# this is the first thing a new user sees. Same marker the login scenarios
# hold the VM image to (tools/xtask/src/login_scenarios.rs).
READY_MARKERS = ["LOGIN first-boot shown"]
# Lines that fail the check at once (the kernel's panic banner, a Rust
# panic message), and lines that must never appear on a fresh disk.
FATAL_MARKERS = ["KERNEL PANIC", "panicked at"]
FORBIDDEN_MARKERS = ["LOGIN skipped"]

ZIP_LIMIT = 1900 * 1024 * 1024  # GitHub's release-asset cap is 2 GiB.


def machine_args():
    """QEMU arguments shared by every way the package is booted."""
    return [
        "-machine", "q35",
        "-cpu", "max",
        "-m", "4096M",
        "-smp", "4",
        "-vga", "std",
        "-no-reboot",
        "-monitor", "none",
        "-serial", "file:" + SERIAL_LOG,
        "-drive", "if=pflash,format=raw,unit=0,readonly=on,file=" + OVMF_CODE,
        "-drive", "if=pflash,format=raw,unit=1,file=" + VARS_RUN,
        "-drive", "if=none,id=vmboot,format=qcow2,file=" + BOOT,
        "-device", "virtio-blk-pci,drive=vmboot,bootindex=0,disable-legacy=on,disable-modern=off",
        "-drive", "if=none,id=vmdata,format=qcow2,cache=writeback,file=" + DATA,
        "-device", "virtio-blk-pci,drive=vmdata,disable-legacy=on,disable-modern=off",
        "-device", "qemu-xhci,id=xhci0",
        "-device", "usb-kbd,bus=xhci0.0",
        "-device", "usb-mouse,bus=xhci0.0",
        "-nic", "user,model=virtio-net-pci",
        "-name", "LogansOS preview",
    ]


WINDOW_ARGS = ["-display", "gtk,zoom-to-fit=on"]
FAST_ACCEL = ["-accel", "whpx,kernel-irqchip=off", "-accel", "tcg"]
SAFE_ACCEL = ["-accel", "tcg"]
CHECK_ARGS = ["-display", "none"] + SAFE_ACCEL


def bat_quote(arg):
    """One argument on a cmd.exe line. None of ours holds a quote, a `%` or
    a caret; quote the ones with a space."""
    assert not any(c in arg for c in '"%^&|<>'), arg
    return '"' + arg + '"' if " " in arg else arg


def bat_line(args):
    return " ".join(bat_quote(a) for a in args)


QEMU_URL = "https://www.qemu.org/download/#windows"

BAT_FIND_QEMU = r"""@echo off
setlocal
rem LogansOS preview: boots the two disks in this folder with QEMU.
cd /d "%~dp0"
title LogansOS preview

set "QEMU=%ProgramFiles%\qemu\qemu-system-x86_64.exe"
if exist "%QEMU%" goto found
set "QEMU="
for %%Q in (qemu-system-x86_64.exe) do set "QEMU=%%~$PATH:Q"
if defined QEMU goto found

echo.
echo   QEMU is not installed (or not where this file looks for it).
echo.
echo   LogansOS runs inside QEMU, a free program. Install it once:
echo     1. Your browser opens the QEMU download page now.
echo     2. Under "Windows", follow the link to the 64-bit installers
echo        and run the newest qemu-w64-setup file.
echo     3. Keep the default install folder (C:\Program Files\qemu).
echo   Then double-click "Start LogansOS.bat" again.
echo.
start "" "{url}"
pause
exit /b 1

:found
if not exist "{boot}" goto missing
if not exist "{data}" goto missing
if not exist "{code}" goto missing
copy /y "{vars_win}" "{vars_run_win}" >nul
if errorlevel 1 goto missing
""".replace("{url}", QEMU_URL)

BAT_MISSING = r"""
:missing
echo.
echo   Some files of the preview are missing. Unzip the whole download
echo   into a folder first, then double-click this file inside that folder.
echo.
pause
exit /b 1
"""


def win(path):
    return path.replace("/", "\\")


def bat_header():
    return (
        BAT_FIND_QEMU.replace("{boot}", win(BOOT))
        .replace("{data}", win(DATA))
        .replace("{code}", win(OVMF_CODE))
        .replace("{vars_win}", win(OVMF_VARS))
        .replace("{vars_run_win}", win(VARS_RUN))
    )


def start_bat():
    fast = bat_line(machine_args() + FAST_ACCEL + WINDOW_ARGS)
    safe = bat_line(machine_args() + SAFE_ACCEL + WINDOW_ARGS)
    body = bat_header() + r"""
echo.
echo   Starting LogansOS. A window opens; the first start takes a few minutes.
echo   Click inside the window to type. Ctrl+Alt+G gives your mouse back.
echo   To quit, close the LogansOS window.
echo.
"%QEMU%" {fast}
if not errorlevel 1 exit /b 0

echo.
echo   QEMU stopped with an error using Windows' fast virtualization.
echo   Trying again with QEMU's own emulator (slower, but it works everywhere).
echo.
copy /y "{vars_win}" "{vars_run_win}" >nul
"%QEMU%" {safe}
if not errorlevel 1 exit /b 0
echo.
echo   QEMU stopped with an error: its message is above. LogansOS's own
echo   log is {serial} in this folder.
pause
exit /b 1
""".replace("{fast}", fast).replace("{safe}", safe).replace(
        "{vars_win}", win(OVMF_VARS)
    ).replace("{vars_run_win}", win(VARS_RUN)).replace("{serial}", SERIAL_LOG)
    return body + BAT_MISSING


def safe_bat():
    safe = bat_line(machine_args() + SAFE_ACCEL + WINDOW_ARGS)
    body = bat_header() + r"""
echo.
echo   Starting LogansOS in compatibility mode (QEMU's own emulator, no
echo   Windows virtualization). Slower, but the same as the build checks.
echo   Click inside the window to type. Ctrl+Alt+G gives your mouse back.
echo   To quit, close the LogansOS window.
echo.
"%QEMU%" {safe}
if not errorlevel 1 exit /b 0
echo.
echo   QEMU stopped with an error: its message is above. LogansOS's own
echo   log is {serial} in this folder.
pause
exit /b 1
""".replace("{safe}", safe).replace("{serial}", SERIAL_LOG)
    return body + BAT_MISSING


def readme(sha, ref, date, with_model):
    short = sha[:12]
    if with_model:
        arthur = (
            "Arthur (the assistant) has his language model in this build:\n"
            "Qwen2.5-0.5B-Instruct, Apache-2.0 (see licenses\\MODEL-NOTICE.txt).\n"
            "Inside a virtual machine he is slow: an answer can take a few\n"
            "minutes, longer in compatibility mode. The little busy emblem\n"
            "means he is still thinking."
        )
    else:
        arthur = (
            "This build has NO language model for Arthur (the assistant). You\n"
            "can still open him and type: he answers that he can hear you but\n"
            "has no model. Everything else works."
        )
    text = f"""LogansOS preview
================

Built from {ref} @ {short} on {date}.
This is a test build of an operating system that is still being made.
It runs inside a virtual machine: nothing on your PC is changed.


Three steps
-----------

1. Install QEMU once (free). Go to https://www.qemu.org/download/#windows,
   follow the link to the 64-bit Windows installers, and run the newest
   "qemu-w64-setup" file. Keep the default folder, C:\\Program Files\\qemu.

2. Unzip this download into a folder (right-click the zip, "Extract All").

3. In that folder, double-click "Start LogansOS.bat".

A LogansOS window opens. The first start takes a few minutes; later starts
are quicker. If the window stays black for more than ten minutes, close
it and double-click "Start LogansOS (compatibility mode).bat" instead.


What to try
-----------

- The first screen asks you to make an account. Choose a password and
  write down the recovery code it shows you.
- The desktop: the bar along the top, the Spine down the left side.
- Home: type a question in the field in the middle and press Enter.
- Talking to Arthur: he opens and answers your question.
{arthur}
- Close the window and double-click "Start LogansOS.bat" again: your
  account and Arthur's memory are still there (they live in
  disks\\data.qcow2). To start over, unzip the download again.


Keys that matter
----------------

  Click inside the window   LogansOS gets your keyboard and mouse
  Ctrl+Alt+G                get your mouse back from the window
  Ctrl+Alt+F                full screen on / off
  Esc                       in Arthur: back to Home
  Tab                       move between Home's field and the Spine
  Super+L (Windows key+L)   lock the session (Windows may catch it first)


How to quit
-----------

Close the LogansOS window (the X in its corner). Nothing else keeps running.


If something goes wrong
-----------------------

- "QEMU is not installed": do step 1, then try again.
- Very slow: normal in compatibility mode. Turning on "Windows Hypervisor
  Platform" (Windows Features) can make the normal start faster.
- Report what you saw on https://github.com/SecondStageTurbine/LogansOS/issues
  and attach logansos-serial.log from this folder (LogansOS's own log).

More detail: docs/HOWTO-try-logansos-in-a-vm.md in the project.
Licences of everything in this zip are in the "licenses" folder.
"""
    return text.replace("\n", "\r\n")


def cmd_package(a):
    """Lay out the package and write the zip."""
    out = Path(a.zip).resolve()
    # Files at the zip's root: Windows' "Extract All" already makes a folder.
    root = Path(tempfile.mkdtemp(prefix="preview-stage-"))
    (root / "disks").mkdir(parents=True)
    (root / "firmware").mkdir()
    (root / "licenses").mkdir()
    shutil.copyfile(a.boot, root / BOOT)
    shutil.copyfile(a.data, root / DATA)
    shutil.copyfile(Path(a.ovmf_dir) / "edk2-x86_64-code.fd", root / OVMF_CODE)
    shutil.copyfile(Path(a.ovmf_dir) / "edk2-i386-vars.fd", root / OVMF_VARS)
    for spec in a.license:
        name, src = spec.split("=", 1)
        shutil.copyfile(src, root / "licenses" / name)
    crlf = lambda s: s.replace("\r\n", "\n").replace("\n", "\r\n")  # noqa: E731
    (root / "Start LogansOS.bat").write_bytes(crlf(start_bat()).encode("ascii"))
    (root / "Start LogansOS (compatibility mode).bat").write_bytes(
        crlf(safe_bat()).encode("ascii")
    )
    (root / "README.txt").write_bytes(
        readme(a.sha, a.ref, a.date, a.with_model == "yes").encode("utf-8")
    )
    if out.exists():
        out.unlink()
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED, compresslevel=6) as z:
        for path in sorted(root.rglob("*")):
            if path.is_file():
                z.write(path, path.relative_to(root).as_posix())
    shutil.rmtree(root)
    size = out.stat().st_size
    print(f"PREVIEW_ZIP path={out} bytes={size}")
    if size > ZIP_LIMIT:
        sys.exit(f"the zip is {size} bytes, over the {ZIP_LIMIT}-byte limit")


def cmd_boot_check(a):
    """Unzip the package and boot it the way `Start LogansOS.bat` does."""
    work = Path(tempfile.mkdtemp(prefix="preview-check-"))
    with zipfile.ZipFile(a.zip) as z:
        z.extractall(work)
    pkg = work
    shutil.copyfile(pkg / OVMF_VARS, pkg / VARS_RUN)
    args = [a.qemu] + machine_args() + CHECK_ARGS
    print("BOOT_CHECK qemu: " + " ".join(args), flush=True)
    log = pkg / SERIAL_LOG
    start = time.monotonic()
    proc = subprocess.Popen(args, cwd=pkg)
    verdict, reason = "FAIL", "timeout"
    seen = 0
    try:
        while time.monotonic() - start < a.timeout:
            time.sleep(5)
            text = log.read_text(errors="replace") if log.exists() else ""
            if len(text) > seen:
                for line in text[seen:].splitlines():
                    print("serial| " + line, flush=True)
                seen = len(text)
            fatal = [m for m in FATAL_MARKERS + FORBIDDEN_MARKERS if m in text]
            if fatal:
                reason = "serial shows " + ", ".join(repr(m) for m in fatal)
                break
            if all(m in text for m in READY_MARKERS):
                verdict, reason = "PASS", "ready marker reached"
                break
            if proc.poll() is not None:
                reason = f"QEMU exited (status {proc.returncode}) before the ready marker"
                break
    finally:
        if proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(30)
            except subprocess.TimeoutExpired:
                proc.kill()
    elapsed = time.monotonic() - start
    if a.log_out:
        shutil.copyfile(log, a.log_out) if log.exists() else None
    shutil.rmtree(work)
    print(f"BOOT_CHECK verdict={verdict} elapsed_s={elapsed:.0f} reason={reason}")
    if a.summary:
        with open(a.summary, "w") as f:
            f.write(f"{verdict} {elapsed:.0f}\n")
    if verdict != "PASS":
        sys.exit(1)


def main():
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = p.add_subparsers(dest="cmd", required=True)
    pk = sub.add_parser("package")
    pk.add_argument("--zip", required=True)
    pk.add_argument("--boot", required=True, help="the boot disk, qcow2")
    pk.add_argument("--data", required=True, help="the data disk, qcow2")
    pk.add_argument("--ovmf-dir", required=True)
    pk.add_argument("--license", action="append", default=[], help="NAME=PATH")
    pk.add_argument("--sha", required=True)
    pk.add_argument("--ref", default="main")
    pk.add_argument("--date", required=True)
    pk.add_argument("--with-model", choices=["yes", "no"], required=True)
    bc = sub.add_parser("boot-check")
    bc.add_argument("--zip", required=True)
    bc.add_argument("--qemu", default="qemu-system-x86_64")
    bc.add_argument("--timeout", type=int, default=2400)
    bc.add_argument("--log-out")
    bc.add_argument("--summary")
    sub.add_parser("qemu-args")
    a = p.parse_args()
    if a.cmd == "package":
        cmd_package(a)
    elif a.cmd == "boot-check":
        cmd_boot_check(a)
    else:
        print("\n".join(machine_args()))


if __name__ == "__main__":
    main()
