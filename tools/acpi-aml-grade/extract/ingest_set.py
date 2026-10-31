#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Turn a delivered ACPI table folder into a corpus set (OT-86).

Input: a folder as J.c. delivers it -- the `.dat` files `acpidump.exe -b` wrote
(or the files of a Linux /sys/firmware/acpi/tables copy) plus a `bios.txt`
and/or `machine.txt` (free-form `key: value` or `key=value` lines: manufacturer,
model, BIOS version, date, ...). Optional: `SHA256SUMS` hashed at the source,
`acpidump-version.txt`.

Output: fixtures/acpi/<name>/ with every table copied byte-for-byte, a fresh
SHA256SUMS, and provenance.json.

Rules enforced here, by table HEADER SIGNATURE (never by file name):
- MSDM and SLIC (the OEM Windows licence key / licensing table) are NEVER
  copied. If one arrives it is dropped and named in provenance.json.
- If the delivery carries a source-side SHA256SUMS, every listed file must
  still match it, or the ingest stops (transport damage).
- Table checksums are verified and recorded, never "fixed".

Usage:
  ingest_set.py --name msi-ms-7e57-<bios> --machine "MSI MS-7E57 (Ryzen 9 9900X)" \\
      --tool "acpidump.exe -b (ACPICA Windows binary tools)" --os "Windows 11" \\
      <delivered-folder> fixtures/acpi/msi-ms-7e57-<bios>
"""

import argparse
import datetime
import hashlib
import json
import os
import re
import sys

EXCLUDED = {b"MSDM": "OEM Windows licence key (MSDM)", b"SLIC": "OEM licensing table (SLIC)"}
METADATA = {"bios.txt", "machine.txt", "acpidump-version.txt", "SHA256SUMS", "provenance.json",
            "README.md", "PROVENANCE.md"}


def read_text(path):
    """Metadata as Windows writes it: PowerShell 5 `Out-File` defaults to
    UTF-16LE with a BOM; everything else is UTF-8/ASCII."""
    raw = open(path, "rb").read()
    if raw[:2] in (b"\xff\xfe", b"\xfe\xff"):
        text = raw.decode("utf-16")
    else:
        text = raw.decode("utf-8-sig", errors="replace")
    return text.replace("\r\n", "\n").strip()


def parse_table_blocks(text):
    """PowerShell `Format-Table` output: per block a header row, a row of dash
    runs, then a value row. Column boundaries come from the dash runs (values
    contain spaces, e.g. "American Megatrends International, LLC."). Returns
    one {lower_header: value} dict per block, in order."""
    blocks = []
    lines = text.splitlines()
    for i in range(1, len(lines) - 1):
        dash = lines[i]
        if not dash.strip() or set(dash.strip()) - {"-", " "}:
            continue
        spans = [m.start() for m in re.finditer(r"-+", dash)]
        head, val = lines[i - 1], lines[i + 1]
        block = {}
        for j, a in enumerate(spans):
            b = spans[j + 1] if j + 1 < len(spans) else None
            key = head[a:b].strip().lower()
            value = (val[a:b] if b else val[a:]).strip()
            if key and value:
                block[key] = value
        blocks.append(block)
    return blocks


def parse_meta(text):
    """bios.txt / machine.txt in either shape: `Format-Table` blocks
    (Win32_BIOS has SMBIOSBIOSVersion, Win32_ComputerSystem has Model) or
    `key: value` / `key=value` lines."""
    out = {}
    blocks = parse_table_blocks(text)
    if blocks:
        for b in blocks:
            if "smbiosbiosversion" in b:
                out["bios_vendor"] = b.get("manufacturer")
                out["bios_version"] = b.get("smbiosbiosversion")
                out["bios_release_date"] = b.get("releasedate")
            elif "model" in b:
                out["system_manufacturer"] = b.get("manufacturer")
                out["model"] = b.get("model")
            else:
                out.update(b)
        return {k: v for k, v in out.items() if v}
    for line in text.splitlines():
        m = re.match(r"\s*([^:=]+?)\s*[:=]\s*(.*?)\s*$", line)
        if m and m.group(2):
            out[m.group(1).strip().lower().replace(" ", "_")] = m.group(2)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--name", required=True, help="corpus set name, e.g. msi-ms-7e57-<bios version>")
    ap.add_argument("--machine", required=True, help="human machine label")
    ap.add_argument("--tool", required=True, help="extraction tool as the delivery says")
    ap.add_argument("--os", required=True, help="OS the tables were dumped from")
    ap.add_argument("--date", default=None, help="extraction date (default: from bios/machine.txt or today)")
    ap.add_argument("--pre-dropped", default="",
                    help="comma-separated signatures removed from the delivery before it reached us "
                         "(e.g. MSDM,SLIC), recorded in provenance")
    ap.add_argument("--note", action="append", default=[], help="free-text provenance note (repeatable)")
    ap.add_argument("--pre-dropped-reason", default="present at extraction, dropped by the PM before commit",
                    help="who removed the --pre-dropped tables, recorded with them")
    ap.add_argument("--xsdt-reference", default=None,
                    help="an XSDT from another dump of the SAME firmware, used for the completeness "
                         "accounting when the delivery has none (Linux /sys/firmware/acpi/tables does "
                         "not export the XSDT); recorded in provenance as the accounting's source")
    ap.add_argument("src")
    ap.add_argument("out")
    a = ap.parse_args()

    if os.path.exists(a.out) and os.listdir(a.out):
        sys.exit(f"{a.out} exists and is not empty; refusing to overwrite a corpus set")

    names = sorted(n for n in os.listdir(a.src) if os.path.isfile(os.path.join(a.src, n)))
    # Linux copies carry runtime-loaded SSDTs in dynamic/; flatten them with a
    # prefix so they stay distinguishable from the XSDT's own tables.
    dyn = os.path.join(a.src, "dynamic")
    if os.path.isdir(dyn):
        names += sorted(os.path.join("dynamic", n) for n in os.listdir(dyn)
                        if os.path.isfile(os.path.join(dyn, n)))
    meta_text = {}
    for m in ("bios.txt", "machine.txt", "acpidump-version.txt"):
        p = os.path.join(a.src, m)
        if os.path.exists(p):
            meta_text[m] = read_text(p)

    # Source-side sums, if delivered, must still hold.
    src_sums = {}
    sp = os.path.join(a.src, "SHA256SUMS")
    if os.path.exists(sp):
        for line in read_text(sp).splitlines():
            if line.strip():
                h, f = line.split(None, 1)
                src_sums[f.strip().lstrip("*")] = h.lower()
        for f, h in src_sums.items():
            fp = os.path.join(a.src, f)
            if os.path.exists(fp) and hashlib.sha256(open(fp, "rb").read()).hexdigest() != h:
                sys.exit(f"{f}: does not match the delivered SHA256SUMS (transport damage?)")

    tables, dropped, skipped = [], [], []
    for n in names:
        if n in METADATA:
            continue
        data = open(os.path.join(a.src, n), "rb").read()
        sig = data[:4]
        if sig in EXCLUDED:
            dropped.append({"file": n, "signature": sig.decode(), "reason": EXCLUDED[sig] + " - never committed"})
            continue
        # Signatures are four printable ASCII bytes (e.g. "ASF!"), not just [A-Z0-9_].
        if len(data) < 36 and data[:8] != b"RSD PTR ":
            skipped.append({"file": n, "reason": "too short for an ACPI table"})
            continue
        if data[:8] != b"RSD PTR " and not all(0x21 <= c <= 0x7E for c in sig):
            skipped.append({"file": n, "reason": "not an ACPI table (no signature)"})
            continue
        tables.append((n, data))
    if not any(d[:4] == b"DSDT" for _, d in tables):
        sys.exit("no DSDT in the delivery")

    os.makedirs(a.out, exist_ok=True)
    records = []
    for n, data in tables:
        n = n.replace(os.sep, "-")
        with open(os.path.join(a.out, n), "wb") as fh:
            fh.write(data)
        sig = "RSDP" if data[:8] == b"RSD PTR " else data[:4].decode()
        rec = {"file": n, "signature": sig, "length": len(data), "sha256": hashlib.sha256(data).hexdigest()}
        if n.startswith("dynamic-"):
            # Linux dynamic/: installed at runtime by the firmware's own AML
            # (Load/LoadTable), never listed in the XSDT.
            rec["runtime_loaded"] = True
        if n in src_sums:
            rec["matches_source_sha256sums"] = src_sums[n] == rec["sha256"]
        if sig not in ("RSDP", "FACS") and len(data) >= 36:
            length = int.from_bytes(data[4:8], "little")
            rec["checksum_ok"] = sum(data[:length]) & 0xFF == 0 and length == len(data)
            rec["oem_id"] = data[10:16].decode("ascii", "replace").rstrip()
            rec["oem_table_id"] = data[16:24].decode("ascii", "replace").rstrip()
            rec["revision"] = data[8]
        records.append(rec)

    for sig in filter(None, (x.strip().upper() for x in a.pre_dropped.split(","))):
        dropped.append({"file": None, "signature": sig,
                        "reason": f"{a.pre_dropped_reason} - never committed"})

    # XSDT completeness. The XSDT lists physical addresses only, so a table the
    # dump left out cannot be named, only counted. FADT is in the XSDT; DSDT
    # and FACS are reached through the FADT, never listed. Never papered over:
    # the counts and any SSDT numbering gap go into provenance as found.
    xsdt = next((d for _, d in tables if d[:4] == b"XSDT"), None)
    xsdt_source = "this delivery"
    if xsdt is None and a.xsdt_reference:
        xsdt = open(a.xsdt_reference, "rb").read()
        if xsdt[:4] != b"XSDT":
            sys.exit(f"{a.xsdt_reference}: not an XSDT")
        xsdt_source = f"reference XSDT (same firmware): {a.xsdt_reference}"
    xsdt_check = None
    if xsdt is not None:
        entries = (int.from_bytes(xsdt[4:8], "little") - 36) // 8
        static = [(n, d) for n, d in tables if not os.path.dirname(n)]
        listed = [n for n, d in static if d[:4] not in (b"XSDT", b"DSDT", b"FACS", b"RSDT")
                  and d[:8] != b"RSD PTR "]
        ssdt_nums = sorted(int(m.group(1) or 0) for n, d in static if d[:4] == b"SSDT"
                           for m in [re.match(r"ssdt(\d*)(\.dat)?$", n.lower())] if m)
        gaps = [f"ssdt{i}" for i in range(1, max(ssdt_nums, default=0) + 1) if i not in ssdt_nums]
        pre = len([d for d in dropped if d["file"] is None]) + len([d for d in dropped if d["file"]])
        xsdt_check = {
            "xsdt_entries": entries,
            "delivered_xsdt_tables": len(listed),
            "excluded_by_rule": pre,
            "missing_from_delivery": entries - len(listed) - pre,
            "ssdt_numbering_gaps": gaps,
            "xsdt_source": xsdt_source,
            "runtime_loaded_tables_not_in_xsdt": len(tables) - len(static),
        }

    kv = {}
    for t in meta_text.values():
        kv.update(parse_meta(t))
    date = a.date or kv.get("date") or datetime.date.today().isoformat()
    bios = (kv.get("bios_version")
            or " ".join(v for k, v in kv.items() if "bios" in k and "version" in k)
            or "see machine_metadata")
    prov = {
        "set": a.name,
        "machine": a.machine,
        "firmware": f"{kv.get('bios_vendor') or kv.get('manufacturer') or 'vendor'} UEFI firmware",
        "firmware_version": bios,
        # As Windows rendered it: local time of the SMBIOS date, so a UTC-midnight
        # date can read as the previous evening.
        "firmware_release_date": kv.get("bios_release_date") or kv.get("bios_date"),
        "machine_metadata_parsed": kv,
        "extraction_tool": a.tool,
        "extraction_os": a.os,
        "date": date,
        "machine_metadata": meta_text,
        "source_sha256sums_delivered": bool(src_sums),
        "excluded_tables_dropped": dropped,
        "non_table_files_skipped": skipped,
        "xsdt_completeness": xsdt_check,
        "notes": a.note,
        "ingested_by": "tools/acpi-aml-grade/extract/ingest_set.py",
        "tables": records,
    }
    with open(os.path.join(a.out, "provenance.json"), "w") as fh:
        json.dump(prov, fh, indent=2)
        fh.write("\n")
    with open(os.path.join(a.out, "SHA256SUMS"), "w") as fh:
        for r in sorted(records, key=lambda r: r["file"]):
            fh.write(f"{r['sha256']}  {r['file']}\n")
    bad = [r["file"] for r in records if r.get("checksum_ok") is False]
    print(json.dumps({"set": a.name, "tables": len(records), "dropped": [d["file"] for d in dropped],
                      "bad_checksums": bad, "xsdt": xsdt_check}))


if __name__ == "__main__":
    main()
