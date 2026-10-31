#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Extract the complete ACPI table set QEMU hands a guest (OVMF or SeaBIOS).

ACPI/AML rung 0 corpus tool (host only). It boots the project's q35 machine
with no OS (OVMF to the UEFI shell, or SeaBIOS -- what `cargo neo-test`
uses -- to "no bootable device"), saves guest RAM over QMP `pmemsave`, finds
the RSDP the firmware installed, walks RSDP -> XSDT/RSDT -> every table (+ the
FADT's DSDT and FACS), checks each table's checksum, and writes one file per
table with a provenance record and SHA-256 sums.

Reads guest memory only; QEMU's own ACPI generator is the firmware under test.

Usage (the committed corpus sets were made with exactly these):
  qemu_extract.py --seabios --boot-wait 12 --name qemu-q35-seabios-smp1 \
      --out fixtures/acpi/qemu-q35-seabios-smp1
  qemu_extract.py --name qemu-q35-ovmf-smp1 --out fixtures/acpi/qemu-q35-ovmf-smp1
  qemu_extract.py --name qemu-q35-ovmf-smp4-viommu --smp 4 \
      --device intel-iommu,intremap=off,caching-mode=on --out fixtures/acpi/qemu-q35-ovmf-smp4-viommu
"""

import argparse
import datetime
import hashlib
import json
import os
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import time

OVMF_CODE = "/usr/share/OVMF/OVMF_CODE_4M.fd"
OVMF_VARS = "/usr/share/OVMF/OVMF_VARS_4M.fd"


def qmp(sock_path, commands):
    s = socket.socket(socket.AF_UNIX)
    s.connect(sock_path)
    f = s.makefile("rwb")

    def read_reply():
        while True:
            msg = json.loads(f.readline())
            if "return" in msg or "error" in msg:
                return msg

    json.loads(f.readline())  # greeting
    replies = []
    for cmd in [{"execute": "qmp_capabilities"}] + commands:
        f.write(json.dumps(cmd).encode() + b"\n")
        f.flush()
        replies.append(read_reply())
    s.close()
    return replies[1:]


def pkg_version(name):
    """Debian package version of the firmware blob, for provenance."""
    r = subprocess.run(["dpkg-query", "-W", "-f=${Version}", name], capture_output=True, text=True)
    return f"{name} {r.stdout.strip()}" if r.returncode == 0 else f"{name} (version unknown)"


def checksum_ok(b):
    return sum(b) & 0xFF == 0


def find_rsdp(mem):
    """Every 'RSD PTR ' whose v1 checksum holds (and v2 checksum, if rev >= 2).

    UEFI places the RSDP anywhere in EfiACPIReclaimMemory (no 16-byte
    alignment rule; OVMF's v2 RSDP sits at +0x14 behind its v1 copy), so the
    scan is byte-granular and the checksums decide. SeaBIOS publishes a
    revision-0 RSDP (RSDT only).
    """
    out = []
    pos = mem.find(b"RSD PTR ")
    while pos != -1:
        if pos + 36 <= len(mem):
            r = mem[pos:pos + 36]
            if checksum_ok(r[:20]) and (r[15] < 2 or checksum_ok(r[:36])):
                out.append(pos)
        pos = mem.find(b"RSD PTR ", pos + 1)
    return out


def table_at(mem, addr):
    if addr + 36 > len(mem):
        raise ValueError(f"table address {addr:#x} beyond saved RAM")
    length = struct.unpack_from("<I", mem, addr + 4)[0]
    return mem[addr:addr + length]


def walk(mem, rsdp_addr):
    rev = mem[rsdp_addr + 15]
    rsdp = mem[rsdp_addr:rsdp_addr + (36 if rev >= 2 else 20)]
    if rev >= 2:
        root_sig, root_addr, width = b"XSDT", struct.unpack_from("<Q", rsdp, 24)[0], 8
    else:
        root_sig, root_addr, width = b"RSDT", struct.unpack_from("<I", rsdp, 16)[0], 4
    root = table_at(mem, root_addr)
    if root[:4] != root_sig or not checksum_ok(root):
        raise ValueError(f"{root_sig.decode()} invalid")
    tables = [("RSDP", rsdp_addr, rsdp), (root_sig.decode(), root_addr, root)]
    fmt = "<Q" if width == 8 else "<I"
    entries = [struct.unpack_from(fmt, root, 36 + width * i)[0]
               for i in range((len(root) - 36) // width)]
    for addr in entries:
        t = table_at(mem, addr)
        tables.append((t[:4].decode("ascii"), addr, t))
        if t[:4] == b"FACP":
            dsdt32, facs32 = struct.unpack_from("<I", t, 40)[0], struct.unpack_from("<I", t, 36)[0]
            x_facs = struct.unpack_from("<Q", t, 132)[0] if len(t) >= 140 else 0
            x_dsdt = struct.unpack_from("<Q", t, 140)[0] if len(t) >= 148 else 0
            dsdt_addr = x_dsdt or dsdt32
            facs_addr = x_facs or facs32
            tables.append(("DSDT", dsdt_addr, table_at(mem, dsdt_addr)))
            if facs_addr:
                facs_len = struct.unpack_from("<I", mem, facs_addr + 4)[0]
                tables.append(("FACS", facs_addr, mem[facs_addr:facs_addr + facs_len]))
    return tables


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--name", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--smp", default="1")
    ap.add_argument("--mem", default="512M")
    ap.add_argument("--cpu", default="qemu64,+rdrand,+smep,+smap")
    ap.add_argument("--device", action="append", default=[])
    ap.add_argument("--boot-wait", type=float, default=25.0)
    ap.add_argument("--seabios", action="store_true",
                    help="boot QEMU's default SeaBIOS (what `cargo neo-test` uses) instead of OVMF; "
                         "SeaBIOS prints to VGA, not serial, so RAM is saved after --boot-wait seconds, "
                         "once it has fallen through to 'No bootable device'")
    args = ap.parse_args()

    mem_bytes = int(args.mem.rstrip("M")) * 1024 * 1024
    if mem_bytes > 2048 * 1024 * 1024:
        sys.exit("keep --mem <= 2048M so q35 low RAM is one contiguous range at 0")

    work = tempfile.mkdtemp(prefix="acpi-extract-")
    try:
        vars_fd = os.path.join(work, "vars.fd")
        shutil.copy(OVMF_VARS, vars_fd)
        qmp_sock = os.path.join(work, "qmp.sock")
        serial = os.path.join(work, "serial.log")
        cmd = ["qemu-system-x86_64", "-machine", "q35", "-cpu", args.cpu,
               "-m", args.mem, "-smp", args.smp, "-display", "none", "-no-reboot",
               "-net", "none",
               "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
               "-serial", f"file:{serial}",
               "-qmp", f"unix:{qmp_sock},server,nowait"]
        if not args.seabios:
            cmd += ["-drive", f"if=pflash,format=raw,readonly=on,file={OVMF_CODE}",
                    "-drive", f"if=pflash,format=raw,file={vars_fd}"]
        for d in args.device:
            cmd += ["-device", d]
        ready = None if args.seabios else b"Shell>"
        proc = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        deadline = time.time() + args.boot_wait + 60
        if ready is None:
            time.sleep(args.boot_wait)
        while ready is not None and time.time() < deadline:
            if os.path.exists(serial) and ready in open(serial, "rb").read():
                break
            time.sleep(1)
        else:
            if ready is not None:
                proc.kill()
                sys.exit(f"firmware never printed {ready!r} on serial")
        dump = os.path.join(work, "ram.bin")
        qmp(qmp_sock, [{"execute": "stop"},
                       {"execute": "pmemsave",
                        "arguments": {"val": 0, "size": mem_bytes, "filename": dump}},
                       {"execute": "quit"}])
        proc.wait(timeout=30)
        qemu_version = subprocess.run(["qemu-system-x86_64", "--version"],
                                      capture_output=True, text=True).stdout.splitlines()[0]
        mem = open(dump, "rb").read()
    finally:
        shutil.rmtree(work, ignore_errors=True)

    rsdps = find_rsdp(mem)
    good = []
    for r in rsdps:
        try:
            good.append((r, walk(mem, r)))
        except ValueError:
            pass
    if not good:
        sys.exit("no RSDP with a valid XSDT/RSDT found in guest RAM")
    # Prefer the newest revision (OVMF publishes a v1 RSDT copy beside the
    # v2 XSDT one). Same-revision copies must be byte-identical, or the
    # extraction is ambiguous; an RSDT view that differs from the XSDT view
    # is recorded in the provenance, not hidden.
    good.sort(key=lambda g: -mem[g[0] + 15])
    rsdp_addr, tables = good[0]
    top_rev = mem[rsdp_addr + 15]
    body = [t[2] for t in tables[2:]]
    root_views_differ = False
    for addr, tabs in good[1:]:
        same_rev = mem[addr + 15] == top_rev
        if [t[2] for t in tabs[2:]] != body:
            if same_rev:
                sys.exit("multiple RSDPs of one revision point at differing table sets; refusing to guess")
            root_views_differ = True

    os.makedirs(args.out, exist_ok=True)
    counts = {}
    records = []
    for sig, addr, data in tables:
        n = counts.get(sig, 0)
        counts[sig] = n + 1
        fname = f"{sig.lower()}{n if n else ''}.dat" if sig != "SSDT" else f"ssdt{n + 1}.dat"
        with open(os.path.join(args.out, fname), "wb") as fh:
            fh.write(data)
        rec = {"file": fname, "signature": sig, "phys_addr": f"{addr:#x}",
               "length": len(data), "sha256": hashlib.sha256(data).hexdigest()}
        if sig not in ("RSDP", "FACS"):
            rec["checksum_ok"] = checksum_ok(data)
            rec["oem_id"] = data[10:16].decode("ascii", "replace").rstrip()
            rec["oem_table_id"] = data[16:24].decode("ascii", "replace").rstrip()
            rec["revision"] = data[8]
        records.append(rec)
    prov = {
        "set": args.name,
        "machine": "QEMU q35 (virtual)",
        "firmware": ("SeaBIOS (QEMU default -bios)" if args.seabios
                     else f"OVMF (Ubuntu ovmf package) {OVMF_CODE}") + "; AML/tables generated by QEMU",
        "firmware_version": {
            "qemu": qemu_version,
            "firmware_package": pkg_version("seabios" if args.seabios else "ovmf"),
        },
        "qemu_args": {"machine": "q35", "cpu": args.cpu, "mem": args.mem, "smp": args.smp,
                      "devices": ["isa-debug-exit,iobase=0xf4,iosize=0x04"] + args.device},
        "extraction_tool": "tools/acpi-aml-grade/extract/qemu_extract.py (QMP pmemsave + XSDT/RSDT walk)",
        "extraction_os": "Linux host (container), guest parked at "
                         + ("SeaBIOS after --boot-wait (no bootable device)" if args.seabios
                            else "the OVMF UEFI shell"),
        "date": datetime.date.today().isoformat(),
        "rsdp_phys_addr": f"{rsdp_addr:#x}",
        "rsdp_copies_found": len(good),
        "rsdp_revision": top_rev,
        "rsdt_view_differs_from_xsdt_view": root_views_differ,
        "tables": records,
    }
    with open(os.path.join(args.out, "provenance.json"), "w") as fh:
        json.dump(prov, fh, indent=2)
        fh.write("\n")
    with open(os.path.join(args.out, "SHA256SUMS"), "w") as fh:
        for r in sorted(records, key=lambda r: r["file"]):
            fh.write(f"{r['sha256']}  {r['file']}\n")
    print(json.dumps({r["file"]: r["length"] for r in records}))


if __name__ == "__main__":
    main()
