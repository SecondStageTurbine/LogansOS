<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ACPI table corpus (OT-86, ACPI/AML rung 0)

One directory per machine/firmware. Each directory holds one file per ACPI table. Tables are identified by
their header signature, not their file name, so the Windows `acpidump -b` (`dsdt.dat`, `ssdt1.dat`), Linux
`/sys/firmware/acpi/tables` (`DSDT`, `SSDT1`) and `tools/acpi-aml-grade/extract/qemu_extract.py` layouts all
load. Each directory also holds:

- `provenance.json`: machine, firmware/BIOS version, extraction tool and OS, date, and per-table SHA-256.
- `SHA256SUMS`: `sha256sum` format. The gate test verifies it against the bytes.

**This public tree carries the QEMU sets only** (`qemu-q35-ovmf-smp1`, `qemu-q35-ovmf-smp4-viommu`,
`qemu-q35-seabios-smp1`). The real-machine sets (the OptiPlex 7080, Zenbook S14, MSI MS-7E57, HP EliteDesk
800 G6 Mini and Lenovo ThinkCentre M75q/M910q dumps) are kept in the private repo. The tests that read a
real-machine set skip with a note when it is absent, and whole-corpus totals (the rung-0 rebaseline, the
committed `tools/acpi-aml-grade/report/`) are checked only when every set is present; the QEMU sets are
always checked in full. The committed report and ratchet were generated from the full corpus. The rest of
this README describes the full corpus.

**Only machines the project controls, or QEMU.** No third-party dumps: provenance is the point of a corpus.
Every machine the PL owns is in scope, one set per machine, plus a second set where a machine was re-dumped
from another OS (the OptiPlex 7080: `optiplex-7080-1.37.0` from Windows, `optiplex-7080-linux-1.37.0` from
Linux). Both stay, side by side, for comparison. A Linux set's runtime-loaded `dynamic/` SSDTs are flattened
to `dynamic-SSDTn` and marked `runtime_loaded` in provenance. Linux exports no XSDT, so its completeness is
counted against a same-firmware XSDT (`ingest_set.py --xsdt-reference`).

**Delivery is closed, but the corpus is not complete:** the M710q was not delivered; the HP, M910q and MSI
Windows dumps lack XSDT-listed tables; and the OptiPlex Linux set accounts for 22 of its 24 XSDT entries.
Every set's `provenance.json` records its own count.

**Never commit MSDM or SLIC** (the OEM Windows licence key / licensing table). Delivered sets go through
`tools/acpi-aml-grade/extract/ingest_set.py`, which drops them by signature and records the drop in
`provenance.json`. The loader refuses them too, and the gate test fails while one is present.
Extraction instructions, and how to add a set, are in `docs/RESEARCH-2026-10-22-acpi-aml-corpus-FINDINGS.md`
§8. After adding a set, run `CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade` (PowerShell:
`$env:CARGO_PROFILE_DEV_PANIC='unwind'; cargo run -p acpi-aml-grade`) from the repo root and commit the regenerated
`tools/acpi-aml-grade/report/`.
