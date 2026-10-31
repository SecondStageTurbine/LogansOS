<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# FINDINGS: ACPI/AML rung 0: grading the stock Rust AML interpreters against a firmware corpus (OT-86)

**Author:** S5 (Claude). **Date:** 2026-10-22. **Branch:** `claude/acpi-aml-rung0` (PL merge reserved).
**Thread:** OT-86 in `docs/OPEN-THREADS.md`.
**Review:** `Plans/DISPATCH-2026-10-22-acpi-aml-rung0-review.md` (cross-family; the writer is Claude).
**Evidence:** `tools/acpi-aml-grade/report/coverage.json` (machine-readable, every number below) and
`coverage.md` (the same, rendered). A test fails if either file drifts from what the harness produces.

**The rung measures and builds nothing.** No kernel file, kernel dependency or kernel feature changed. The
default kernel build is bit-for-bit identical (§7). The crates are graded stock: nothing is patched, forked
or vendored.

## Summary

- **What was graded.** Two stock crates, neither patched: `aml` 0.16.4 (the crate the brief names) and its
  successor `acpi` 6.1.1. Delivery is closed: 3 QEMU sets plus all six machines J.c. has (Zenbook S14,
  OptiPlex 7080, MSI MS-7E57, ThinkCentre M75q and M910q, EliteDesk 800 G6). The M710q was not delivered.
  The OptiPlex has two sets, its Windows dump and a Linux re-dump (2026-10-23), so the corpus is 10 sets.
- **Real-hardware result.**

  | Crate | DSDTs that load | `_STA`/`_INI` reached | completing |
  |---|---|---|---|
  | `aml` 0.16.4 | 0 of 6 | 14 | 10 |
  | `acpi` 6.1.1 | 3 of 6 | 397 | 305 (76.8%) |

  `acpi` 6.1.1 is the only viable base.
- **What blocks it.** Six things, three of them panics, which in our `panic = "abort"` kernel are a dead machine:
  - field-unit type confusion (71 failures, all six machines; it also fails the OptiPlex DSDT load);
  - IndexField/BankField `todo!()` (17 panics, 4 machines);
  - a PCI_Config base-drop that misaddresses every PCI-config field;
  - Buffer compares longer than 4 bytes (every UUID `_DSM`);
  - unimplemented I2C/GPIO resource descriptors and Field `Connection()`.

  The last two block the Zenbook touchpad and touchscreen. The keyboard and EC evaluate.
- **The corpus has gaps, and they are recorded.** Delivery is final for the machines J.c. owns, but the corpus
  is not complete:
  - the M710q was not delivered;
  - three Windows dumps are partial: their XSDT lists tables the dump lacks (HP 3, M910q 2, MSI 1), so they
    are graded as delivered;
  - **the OptiPlex now has a near-complete Linux set** (`optiplex-7080-linux-1.37.0`): all 6 static SSDTs,
    the 7 runtime-loaded `dynamic/` SSDTs and FACS. Against its XSDT (24 entries), 22 are delivered and **2 are
    in neither the Linux nor the Windows dump**; they cannot be named from the XSDT (§2). The Windows
    OptiPlex set stays in the corpus for comparison;
  - the other machines have no Linux `dynamic/` SSDTs (Windows `acpidump -b` cannot see them).
- **The OptiPlex DSDT load failure is a crate bug, not a missing table.** The Linux DSDT is byte-identical
  to the Windows one, and `acpi` 6.1.1 fails it the same way with every SSDT present (§4b).
- **Rung-0 rebaseline (§4b).** With the Linux OptiPlex set added, the stock grade on real hardware is
  `acpi` 6.1.1 3/7 DSDTs, 335/446 = 75.1% `_STA`/`_INI` (was 3/6 and 305/397 = 76.8%), and 83
  `ObjectNotOfExpectedType` (was 71). Fix 1 is measured against these numbers.
- **Decision (J.c., 2026-10-23, relayed by the PM): "Acpi is a go."** `acpi` 6.1.1, not `aml` 0.16.4, is the
  Loganfy base (§9 L2, decided). #96 has his go-ahead to merge once the cross-family review passes and host CI
  is green. The next rungs are L3, then L4.

## 0. The stop condition did not fire

The rung's stop condition asks whether the stock crate can be driven from outside with a recording handler,
a namespace walk and `_STA`/`_INI` evaluation. Both crates can, through public API only:

| Need | `aml` 0.16.4 | `acpi` 6.1.1 (`acpi::aml`) |
|---|---|---|
| Recording handler | `AmlContext::new(Box<dyn aml::Handler>, …)`: every SystemMemory/IO/PCI access is a trait call | `Interpreter::new(H: acpi::Handler, …)`: same, plus `map_physical_region` |
| Load tables | `AmlContext::parse_table(&[u8])` | `Interpreter::load_table(&[u8])` |
| Namespace walk | `pub namespace: Namespace` and `Namespace::traverse` | `pub namespace: Spinlock<Namespace>` and `Namespace::traverse` |
| Evaluate `_STA`/`_INI` | `AmlContext::invoke_method(&AmlName, Args)` | `Interpreter::evaluate(AmlName, Vec<…>)` |
| The crate's own init | `initialize_objects()` | `initialize_namespace()` |

## 1. Which crate was graded, and why two

The brief names "the current published `aml` crate", and `AmlContext` is that crate's type. Its newest
published version is **`aml` 0.16.4**. Upstream (rust-osdev/acpi) has since moved AML into the **`acpi` crate
itself**. **`acpi` 6.1.1** ships a rewritten interpreter (`acpi::aml::Interpreter`) behind its default `aml`
feature and does not depend on the `aml` crate at all. Grading only 0.16.4 would grade a line upstream has left.
Grading only 6.1.1 would not grade what the brief names. **Both are pinned exactly (`=0.16.4`, `=6.1.1`) and
graded side by side**, and every number below is per crate. Both are MIT/Apache-2.0.

Which one to Loganfy is the next decision (§9).

## 2. The corpus: ten sets, seven of them real-hardware dumps of six machines. Delivery is closed, but the corpus is not complete

| Reference machine | Present | Sets |
|---|---|---|
| QEMU q35 (always available) | yes | `qemu-q35-seabios-smp1`, `qemu-q35-ovmf-smp1`, `qemu-q35-ovmf-smp4-viommu` |
| ASUS Zenbook S14 UX5406AA (Intel Panther Lake) | yes | `zenbook-s14-ux5406aa-303`: AMI BIOS UX5406AA.303; 49 tables, 27 SSDTs |
| Dell OptiPlex 7080 (Comet Lake; the bare-metal Arthur target) | yes, twice | `optiplex-7080-1.37.0`: Windows dump, Dell BIOS 1.37.0; 21 tables, 3 SSDTs (see XSDT completeness below). `optiplex-7080-linux-1.37.0`: Linux re-dump of the same machine and BIOS (2026-10-23); 31 tables: 6 static SSDTs, 7 runtime-loaded `dynamic/` SSDTs, FACS |
| Lenovo ThinkCentre M710q (the Gen9 GPU target) | **not delivered** | J.c. does not have it (PM, 2026-10-22). Every machine he has is delivered, so **delivery is closed**, but the corpus is **not complete**: the M710q is missing, and four sets have unaccounted XSDT entries (below). Only the OptiPlex has a Linux set with runtime `dynamic/` SSDTs. |
| Other project-owned machines | yes | `msi-ms-7e57-1.a70`: MSI MS-7E57, AMD Ryzen 9 9900X, AMI BIOS 1.A70; 29 tables, 15 SSDTs, 443 KB DSDT. `lenovo-thinkcentre-m75q-gen1-m2fkt21a`: AMD, BIOS M2FKT21A; 23 tables. `hp-elitedesk-800-g6-mini-s21-02.27.00`: Intel Comet Lake, HP BIOS S21 02.27.00; 25 tables. `lenovo-thinkcentre-m910q-m1akt5aa`: Intel Kaby Lake, BIOS M1AKT5AA; 24 tables, 7 SSDTs. |

**The real-hardware sets** are J.c.'s `acpidump.exe -b` dumps under Windows, delivered on 2026-10-22. The PM
committed them to `claude/phase-4-continuation-TlW3j` `acpi-drop/` (up to `1026a5b`). They were ingested from there
with `extract/ingest_set.py`, and every table was checked against the drop's bytes by hash; `acpi-drop/` itself
is not on this branch. Each `provenance.json` carries the PL's `bios.txt` (Win32_BIOS + Win32_ComputerSystem)
verbatim and parsed. The ThinkCentre's MSDM and SLIC were present at extraction and removed by the PM before
commit, which its provenance records. No other set contained either.

The delivered `acpidump -b` sets contain no RSDP or FACS. `acpi` 6.1.1 then runs without a
global-lock mapping, which only matters for `Acquire(\_GL)`.

**The OptiPlex Linux set** (`optiplex-7080-linux-1.37.0`) is J.c.'s copy of `/sys/firmware/acpi/tables`
from an Ubuntu live session on the same machine and BIOS (1.37.0), delivered to `acpi-drop/` @ `1c137d1`. It
carries FACS, so `acpi` 6.1.1 gets a global-lock mapping on this set. It has no MSDM or SLIC. Compared with the
Windows set:
- **DSDT and every other shared static table are byte-identical,** except BGRT, which records per-boot logo
  status.
- **The Windows dump's names do not follow firmware order.** Its `ssdt`, `ssdt1` and `ssdt3` are Linux `SSDT1`
  (CpuSsdt), `SSDT2` (SaSsdt) and `SSDT4` (DELL_MFF). The Windows dump lacked `SSDT3` (Ther_Rvp), `SSDT5`
  (PtidDevc) and `SSDT6` (TbtTypeC). Its `ssdt2` "gap" was a naming artefact, not the missing table.
- **`dynamic/SSDT7`–`SSDT13`** are the CPU power-management tables (Cpu0Cst, Cpu0Ist, Cpu0Psd, ApIst, ApHwp,
  ApPsd, ApCst). The firmware's own AML installs them at runtime with `Load`/`LoadTable`, and the XSDT never
  lists them. They are ingested as `dynamic-SSDT7`…`13`, with `runtime_loaded: true` in provenance. The
  graders load them after the static SSDTs, which gives Linux's post-boot namespace.
- **Linux exports no XSDT or RSDP,** so this set's completeness is counted against the Windows set's XSDT
  (same firmware). `ingest_set.py --xsdt-reference` records that source in provenance.

**XSDT completeness: not papered over.** A Windows `acpidump -b` dump does not always contain every table the
XSDT lists. The XSDT holds physical addresses only, so a table the dump left out can be counted but not
named. `ingest_set.py` records, per set:
- the XSDT entry count;
- the number of delivered tables that belong in the XSDT;
- the tables excluded by rule (MSDM/SLIC);
- the unexplained remainder;
- any SSDT numbering gap.

| Set | XSDT entries | Delivered | Excluded by rule | **Missing, unexplained** | SSDT numbering gap |
|---|---|---|---|---|---|
| Zenbook S14 | 49 | 47 | 2 (MSDM, SLIC, deleted by J.c.) | 0 | none |
| ThinkCentre M75q | 23 | 21 | 2 (MSDM, SLIC, dropped by the PM) | 0 | none |
| MSI MS-7E57 | 28 | 27 | 0 | **1** | none |
| ThinkCentre M910q | 24 | 22 | 0 | **2** | none |
| EliteDesk 800 G6 | 26 | 23 | 0 | **3** | none |
| OptiPlex 7080 (Windows) | 24 | 19 | 0 | **5** | `ssdt2` (a naming artefact: the dump lacks Linux `SSDT3`, `SSDT5`, `SSDT6`) |
| OptiPlex 7080 (Linux) | 24 (the Windows XSDT, same firmware) | 22 | 0 | **2** | none (static `SSDT1`–`6`; 7 runtime-loaded `dynamic/` SSDTs are not XSDT-listed) |

The QEMU sets are complete by construction (extracted from guest RAM).

**Consequences:**
- **For three machines, the grade is of a partial table set.** A missing SSDT can turn into the
  `LevelDoesNotExist`/`ObjectDoesNotExist` failures counted below.
- **The OptiPlex's missing SSDTs were not the cause of its DSDT load failure.** The Linux set has all six,
  and the DSDT, byte-identical, fails the same way (§4b). That makes it a crate bug.
- **The OptiPlex's last 2 XSDT entries are in neither dump.** Linux `/sys/firmware/acpi/tables` exports every
  table the kernel installed, and these two are not among them. The XSDT names them by address only. The
  delivery note says "no MSDM/SLIC" without saying whether those were absent or removed. If removed, these two
  are excluded by rule rather than missing. Until that is known, provenance counts them as missing and notes
  the alternative.
- **Closing the gap for HP, M910q and MSI needs a Linux dump of each** (L1). J.c. may re-dump them from a
  Linux live USB. Each new set gets the same rebaseline-first treatment.

**The QEMU and real-hardware grades must be read separately (§4).** QEMU's AML is small (8.4 KB DSDT, no
SSDTs). The real sets' AML (DSDT + SSDTs) runs from 106 KB (Lenovo M75q) to 840 KB (Zenbook), 13–100× QEMU's,
and it uses much more of the language. The grade drops accordingly.

**The three QEMU sets** come from `tools/acpi-aml-grade/extract/qemu_extract.py`. The extractor boots q35 with
no OS, saves guest RAM over QMP `pmemsave`, finds the RSDP the firmware installed, and walks RSDP →
XSDT/RSDT → every table (+ DSDT and FACS). Each table's checksum is verified. Each set directory has one
`.dat` per table, a `provenance.json` (machine, firmware and package versions, QEMU arguments, tool, OS, date,
each table's physical address and SHA-256) and a `SHA256SUMS`.

- **`qemu-q35-seabios-smp1`** is the firmware `cargo neo-test` actually boots (xtask uses `-boot d -cdrom`
  and no `-bios`, so SeaBIOS), with the project's default CPU, memory and SMP flags.
- **`qemu-q35-ovmf-smp1`** is the brief's QEMU/OVMF source. Its DSDT is the same size as SeaBIOS's but not
  byte-identical. The firmware programs the ECAM base, so the MMCONFIG/PCI-window `_CRS` values differ.
- **`qemu-q35-ovmf-smp4-viommu`** is the `vtd_fence` configuration: four CPUs (four `Processor` objects) and
  an `intel-iommu` (adds the DMAR table).

**Re-extraction is deterministic.** Running the extractor again over all three sets reproduced every table
byte for byte (SHA256SUMS unchanged). No third-party DSDT was used.

## 3. The harness (`tools/acpi-aml-grade`)

A host-only workspace member, following the `tools/*-logic-tests` pattern.

- **Recording handler** (`src/recorder.rs`). Every SystemMemory, SystemIO and PCI_Config access either crate
  makes is logged with its space, address (and PCI segment/bus/device/function), width, direction and value. It
  is answered from a **scripted model**: a zero-initialised, byte-granular write-back store per address space.
  A read returns the last value written there, or zero ("no device answers"). There is a fake clock (1 ms per
  query, so firmware timeouts end) and mutexes are always granted. **Nothing touches real hardware;** the
  handler cannot, by construction. Each evaluation has an access budget (200 000); spending it is classified as
  non-termination rather than hanging the harness.
- **Grading.** Per set and per crate:
  1. load the DSDT, then every SSDT in firmware order;
  2. walk the whole namespace and evaluate **every** `_STA` and `_INI` present, parent before child, whatever
     the device's status (the rung grades interpretation coverage, not boot policy);
  3. separately, run the crate's own init entry point on a fresh context.

  Each crate panic is caught (`catch_unwind`) and becomes a classified failure.
- **Region check.** After loading, the declared regions are every OpRegion in the interpreter's own namespace
  plus the FADT's fixed-hardware blocks. Every logged access must fall inside one of them.
- **Output.** `report/coverage.json` and `report/coverage.md`, regenerated by
  `CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade` (from the repo root; PowerShell:
  `$env:CARGO_PROFILE_DEV_PANIC='unwind'; cargo run -p acpi-aml-grade`; §6.3).
- **Scales to N sets.** One directory per machine. The report has one line per set and crate, then the
  aggregate and the ranked gaps across all sets. Sets that don't belong to a named reference machine are listed
  as "other project-owned machines".
- **Licence tables never enter the corpus.** MSDM (the OEM Windows licence key) and SLIC are refused by header
  signature, never by file name, in two places. `extract/ingest_set.py` drops them and names them in
  `provenance.json`. The Rust loader refuses to load one, and a gate test fails while any is present.
- **Gate tests** (`tests/corpus.rs`; they run in the normal `cargo test --workspace` host CI):
  - every set's SHA256SUMS verifies, the provenance keys are present, and each table's checksum agrees with
    what provenance recorded at extraction (a bad checksum on real firmware is a finding, not corpus damage);
  - no set contains MSDM or SLIC (and a planted MSDM/SLIC is refused by the loader);
  - every set is graded by both crates, with **zero `UNCLASSIFIED` failures**;
  - the committed report equals the harness's output.

  Unit KATs cover SHA-256 (FIPS 180-2 vectors), the address-formation diagnoser and the opcode call-path scan.

## 4. Headline metrics (9 sets: 3 QEMU, 6 real hardware)

| Group | Crate | DSDT parses clean | All DSDT+SSDT parse clean | `_STA`/`_INI` reached | completing | …and with no failure of any class |
|---|---|---|---|---|---|---|
| **real hardware (6)** | `aml` 0.16.4 | **0/6** | 0/6 | 14 | 10/14 | 8/14 |
| **real hardware (6)** | `acpi` 6.1.1 | **3/6 = 50%** | 2/6 | 397 | **305/397 = 76.8%** | 305/397 = 76.8% |
| QEMU (3) | `aml` 0.16.4 | 3/3 | 3/3 | 57 | 51/57 = 89.5% | 33/57 = 57.9% |
| QEMU (3) | `acpi` 6.1.1 | 3/3 | 3/3 | 57 | 57/57 = 100% | 33/57 = 57.9% |
| all (9) | `aml` 0.16.4 | 3/9 = 33.3% | 3/9 = 33.3% | 71 | 61/71 = 85.9% | 41/71 = 57.7% |
| all (9) | `acpi` 6.1.1 | 6/9 = 66.7% | 5/9 = 55.6% | 454 | 362/454 = 79.7% | 338/454 = 74.4% |

How to read the table:
- **"Reached" comes first.** A table that fails to load contributes no `_STA`/`_INI` to that crate's namespace.
  `aml` 0.16.4's 10/14 on real hardware is fourteen objects out of a namespace that never loaded, not a 71% grade.
  On real firmware it **cannot load a single DSDT**.
- **"Completing"** means the interpreter returned a value.
- **The last column** also excludes evaluations that completed but touched hardware outside every declared region.
- **On QEMU `acpi` 6.1.1 completes everything,** yet on 24 of 57 evaluations it reads the wrong hardware (§5 #2).

Per set, `acpi` 6.1.1 on real hardware:

| Set | DSDT | SSDTs that failed | `_STA` | `_INI` |
|---|---|---|---|---|
| HP EliteDesk 800 G6 Mini | clean | none | 35/39 | 30/32 |
| ThinkCentre M910q | clean | none | 88/108 | 31/38 |
| OptiPlex 7080 (Arthur target) | **failed** (`ObjectNotOfExpectedType` during load) | none of 3 | 7/13 | 22/30 |
| Zenbook S14 | clean | 3 of 27 | 53/78 | 14/19 |
| MSI MS-7E57 | **failed** | 2 of 15 | 21/36 | 4/4 |
| Lenovo M75q Gen 1 | **failed** | 3 of 7 | 0/0 | 0/0 |

The crates' own init entry points:
- `acpi` 6.1.1 `initialize_namespace()` **panics** on the HP (the predicted non-integer-`_STA` panic,
  `mod.rs:256`), on the MSI (IndexField `todo!()`), on the M910q (IndexField write `todo!()`) and on the OptiPlex (`todo!("Connect field :(")`, `mod.rs:1741`: a Field list with a `Connection()`).
- `aml` 0.16.4 `initialize_objects()` returns `Err` on the HP and the Zenbook.

## 4b. Rung-0 rebaseline (2026-10-23): the corpus with the OptiPlex Linux set

**Why a rebaseline.** Fix 1 changes the code. The corpus also changed when the OptiPlex Linux set arrived. The
stock crates are re-graded on the new corpus before any Fix 1 code, so each change gets its own clean read.
Fix 1's before/after is measured against this table, not against §4. §4 stays as graded at `048f64f`.

- **The harness and the judge are unchanged.** Same `aml` =0.16.4 and `acpi` =6.1.1, both unpatched.
- **The nine existing sets grade identically.** Every per-set figure in `coverage.json` is unchanged; every
  delta below comes from the one new set.

| Group | Crate | DSDT parses clean | `_STA`/`_INI` reached | clean, rung 0 (§4) | **clean, rebaseline** |
|---|---|---|---|---|---|
| real hardware | `acpi` 6.1.1 | 3/6 → **3/7** | 397 → 446 | 305/397 = 76.8% | **335/446 = 75.1%** |
| real hardware | `aml` 0.16.4 | 0/6 → **0/7** | 14 → 16 | 8/14 | **9/16** |
| QEMU (3) | `acpi` 6.1.1 | 3/3 | 57 | 33/57 | **33/57** (unchanged: the 24 PCI_Config misreads remain) |
| all | `acpi` 6.1.1 | 6/9 → **6/10** | 454 → 503 | 338/454 = 74.4% | **368/503 = 73.2%** |

| `acpi` 6.1.1 failure count | rung 0 | **rebaseline** | where the change is |
|---|---|---|---|
| `ObjectNotOfExpectedType` (`_STA`/`_INI` + table loads; the brief's "71") | 71 | **83** | +12, all on the OptiPlex Linux set |
| crate panics | 26 | **27** | +1 (`Connect field`), on the OptiPlex Linux set |

The OptiPlex, Windows dump against Linux dump, under `acpi` 6.1.1:

| Set | DSDT | SSDTs that failed to load | `_STA` | `_INI` | `ObjectNotOfExpectedType` |
|---|---|---|---|---|---|
| `optiplex-7080-1.37.0` (Windows) | **failed**: `ObjectNotOfExpectedType { expected: Reference, got: Package }` | none of 3 | 7/13 | 22/30 | 12 |
| `optiplex-7080-linux-1.37.0` (Linux) | **failed, identically** | 1 of 13: `SSDT6` (TbtTypeC) on `ObjectDoesNotExist(TBTS)` | 8/19 | 22/30 | 12 |

What this settles:
- **The OptiPlex DSDT load failure is a crate bug.** The DSDT is byte-identical, every SSDT is now present, and
  the failure is the same.
- **`SSDT6`'s failure is downstream of it.** `TBTS` is a field the DSDT declares late in its body (line 20552 of
  the disassembly). A DSDT load that stops early never reaches that declaration.
- **The extra SSDTs add 6 `_STA`s.** The 5 new failures among them are thermal-fan `\_TZ.FN0x._STA`
  (`InvalidOperationOnObject { LogicalOp, Uninitialized }`). Those PowerResources live in `SSDT3` (Ther_Rvp),
  which the Windows dump lacked. Each `_STA` does `Local0 = Acquire (FMT0, 0x03E8)` and then compares `Local0`.
  `acpi` 6.1.1 leaves `Acquire`'s result Uninitialized, where the spec has it return an Integer (True on
  timeout, §19.6.2). That is a new, named gap; it is not part of the 83.

## 4a. The primary target: Zenbook S14 UX5406AA (Panther Lake, BIOS UX5406AA.303)

**Set:** `zenbook-s14-ux5406aa-303` (49 tables: 27 SSDTs; DMAR, ECDT, NHLT, LPIT, SDEV, PHAT, HEST, DBG2, DTPR).

**Grade:**
- **`aml` 0.16.4: unusable.** The DSDT fails to load: `unimplemented!()` on a `Debug` target, then cascades. 21
  of 28 AML tables fail, and only 2 `_STA`s are reachable.
- **`acpi` 6.1.1: loads the DSDT clean.** 3 of 27 SSDTs fail:
  - `ssdt.dat`: `LevelDoesNotExist`;
  - `ssdt2.dat`: `NoCurrentOp`;
  - `ssdt18.dat`: `RefOf` store, `todo!()`.

  `_STA` completes 53/78 and `_INI` 14/19. 29 of those failures are the field-unit type confusion (§5 #1).

**The lane-deciding devices (PM ask): device probes.** `fixtures/acpi/zenbook-s14-ux5406aa-303/probes.json`
lists them. Each probe evaluates the named methods (`_DSM` with the HID-over-I2C UUID, rev 1, functions 0 and 1),
then runs each `_CRS` result through the crate's own resource decoder. The full table is in
`report/coverage.md`, "Device probes". `aml` 0.16.4 reaches none of these devices (its DSDT never loads).
`acpi` 6.1.1, under the zero model:

| Device | What evaluates | What fails |
|---|---|---|
| **EC** `\_SB.PC00.LPCB.EC0` (the ECDT's EC_ID; ports 0x66/0x62, GPE 0x6E) | `_HID` = PNP0C09; no `_STA` (present); `_CRS` decodes to I/O 0x62 and 0x66; `_GPE` = 0x6E, matching the ECDT | nothing probed fails. EC *OpRegion* access is unsupported in the crate (§5a), so EC-backed methods elsewhere will fail once reached |
| **Touchpad** `\_SB.PC00.I2C5.ETPD` (ASCE1206 / ASCF1206 / ELAN1010, chosen by NVS) | `_HID` (zero model picks ELAN1010); `_STA` = 0 (`TPIF != 1`); `_CRS` evaluates to `I2cSerialBusV2` @0x15, 400 kHz on `\_SB.PC00.I2C5` + Interrupt 0x69 (level, active-low, wake): hand-decoded from the returned bytes | **decoding that `_CRS` panics**: `unimplemented!("GenericSerialBus Connection Descriptor")`. **`_DSM` panics** (functions 0 and 1): Buffer comparison copies a 16-byte UUID into a 4-byte array (`mod.rs:1889`) |
| **Touchscreen** `\_SB.PC00.I2C4.TPL0` (ELAN9008) | `_STA` = 0 under the zero model | **`_HID` panics** (`unwrap` on `ObjectNotOfExpectedType {String, String}`, `mod.rs:813`, in `DerefOf(HIDS[TPLI])`); `_CRS` fails with `ObjectNotOfExpectedType`; **`_DSM` panics** (`mod.rs:1889`) |
| **Keyboard** `\_SB.PC00.LPCB.PS2K` (ATK3001, CID PNP030B) | `_HID` = ATK3001; `_STA` = 0x0F; `_CRS` decodes to I/O 0x60, 0x64 + IRQ 1 | none |
| **GPIO** `\_SB.GPI0/1/3/4/5` (`GHID`: INTC10BB on the PL's machine) | `_HID` (zero model picks INTC10BC: the `PCHS == PCHH` NVS branch); `_STA` = 0x0F | **every `_CRS` fails** (`ObjectNotOfExpectedType`) |

**What this decides for the lanes:**
- **Keyboard: unblocked by AML.** Its `_CRS` evaluates and decodes. PS/2 needs nothing AML can't do here.
- **EC: half there.** The device and its resources evaluate. EC OpRegion access (battery, lid, thermal methods)
  is unimplemented in the crate (L9).
- **Touchpad and touchscreen: blocked in the stock crate on two independent defects.**
  1. The decoder cannot decode an I2cSerialBus or GpioInt descriptor at all; it panics.
  2. Any `_DSM` that compares a UUID panics, so the HID descriptor address cannot be obtained.

  Both are L4-class fixes, and L4 now carries them.
- **GPIO controllers:** their `_CRS` (the MMIO windows a GPIO driver needs) fails on the field-unit type
  confusion (L3).

**Caveat: the zero model.** `_HID`/`_STA`/`_CRS` on this machine branch on NVS state (`TPDI`, `TPHI`, `TPLI`,
`PCHS`, `TPIF`, `SBFI`/`SBFG`) that the zero model leaves at 0. That is why the touchpad answers ELAN1010 with
`_STA` 0, and the GPIOs INTC10BC. The real values come from firmware NVS (the GNVS SystemMemory region) at boot.
A later rung can seed the scripted model from a live NVS dump. The panics and decode failures above do not
depend on those values.

## 5. The ranked gap taxonomy (every failure classified; 0 unclassified)

All 28 distinct failure modes are in `report/coverage.md`; the top of the list, with root causes:

| # | Crate | Class | Failure mode (named) | Occurrences | Sets |
|---|---|---|---|---|---|
| 1 | `acpi` 6.1.1 | type-semantics | **ObjectNotOfExpectedType**: type confusion on field-unit operands | 71 | 6 (all real) |
| 2 | `acpi` 6.1.1 | out-of-region | **PCI_Config field access drops the OpRegion base** | 24 | 3 (QEMU) |
| 3 | `aml` 0.16.4 | type-semantics | **LevelDoesNotExist** (mostly the cascade of a failed DSDT load) | 24 | 6 |
| 4 | `aml` 0.16.4 | out-of-region | **PCI_Config field bit offset added to the base as bytes** | 18 | 3 (QEMU) |
| 5 | `aml` 0.16.4 | out-of-region | **SystemMemory field bit offset added to the base as bytes** (HP `\_SB.PNVS`, M910q, OptiPlex) | 16 | 3 |
| 6 | `acpi` 6.1.1 | unimplemented | **IndexField read: `todo!()` at `mod.rs:2387` (panics)** | 15 | 4 |
| 7 | `aml` 0.16.4 | parse-error | UnexpectedEndOfStream | 8 | 2 |
| 8 | `aml` 0.16.4 | type-semantics | ValueDoesNotExist (cascade) | 8 | 4 |
| 9 | `aml` 0.16.4 | unimplemented | **no parser arm: Acquire, Release** (QEMU CPU `_STA`) | 6 | 3 |
| 10 | `acpi` 6.1.1 | type-semantics | InvalidOperationOnObject (ToBuffer on a BufferField; LogicalOp on Uninitialized) | 5 | 2 |
| 11 | `aml` 0.16.4 | unimplemented | `unimplemented!()` on a `Debug` target at `lib.rs:354` (panics) | 5 | 1 |
| 18 | `acpi` 6.1.1 | unimplemented | **`todo!("Connect field :(")` at `mod.rs:1741`**: Field lists with `Connection()` (GPIO/serial-bus-backed fields) | 1 | 1 (OptiPlex) |
| 23–28 | `aml` 0.16.4 | unimplemented | **no parser arm** while loading the MSI, M75q, M910q and OptiPlex DSDT/SSDTs: Event, Signal, Wait, Reset, Stall, Sleep, Timer, Acquire, Release, FromBCD, IndexField, BankField | 6 | 4 |

Further `acpi` 6.1.1 panics, one each:
- `RefOf` store: `todo!()` at `mod.rs:2318` (Zenbook SSDT load);
- IndexField write: `todo!()` at `mod.rs:2447` (2: M910q, MSI);
- an `unwrap()` on `ObjectNotOfExpectedType { expected: String, got: String }` at `mod.rs:813` (HP `\_SB.WMIS._INI`).

There is also one `NoCurrentOp` parse error, on a Zenbook SSDT.

Root causes, confirmed in the crate source unless marked:

1. **`acpi` 6.1.1: type confusion on field-unit operands (#1, #8). Root cause not yet traced to a line.**
   - **Cases checked by hand:**
     - Zenbook `\_SB.LNKA._STA` computes `PIRA & 0x80`. `PIRA` is an 8-bit field, and the crate reports
       `expected: Integer, got: Integer`.
     - Zenbook `\_SB.PC00.I2C0.HDC1._STA` computes `(I2SC == 0x02) && (CDIS != One)`. `I2SC` is a
       SystemMemory field, and the crate reports `expected: Reference, got: Package`.
   - **What it means:** both are field reads used as expression operands. The "Integer vs Integer" message shows
     the type check compares something other than the AML object type.
   - **Impact:** this mode alone fails 29 Zenbook, 23 M910q, 11 OptiPlex and 4 HP `_STA`/`_INI`s, and the OptiPlex DSDT load itself.
   - **Next step:** tracing it is the first job of the base rung (§9 L3).
2. **`acpi` 6.1.1 reads PCI config at the field offset, not at region base + field offset (#2).**
   - **What happens:** QEMU's `\_SB.PCI0.SF8.PIRQ` is `OperationRegion(PIRQ, PCI_Config, 0x60, 0x0C)`.
     `LNKA`–`LNKH._STA` read `PRQA`–`PRQH`, which live at bytes 0x60–0x63 and 0x68–0x6B. The crate issued the
     reads at 0x0–0x3 and 0x8–0xB, the LPC bridge's **vendor/device ID and class bytes**.
   - **Cause:** `do_native_region_read`/`_write` (`src/aml/mod.rs`) add `region.base` for SystemMemory and
     SystemIO, but pass only the field offset for `PciConfig`.
   - **Why it never shows on real hardware:** real firmware usually declares a whole-config-space region from
     offset 0 on the same device, so the misaddressed access still lands inside *a* declared region. The region
     check can't see that (L11).
3. **`aml` 0.16.4 adds a field's bit offset to the region base as if it were bytes (#4, #5), in every space.**
   - **What happens:** QEMU `LNKC` (bit offset 0x10) read 0x70 instead of 0x62. During an SSDT load on the HP,
     the `\_SB.PNVS` reads went to `0xb0e83270` instead of `0xb0e8304e`.
   - **Cause:** `AmlValue::Field.offset` is "in bits" (`value.rs`), and `read_region` adds it to `region_base`.
4. **`acpi` 6.1.1 IndexField/BankField are `todo!()` (#6).** A panic in a kernel. Real firmware uses IndexField
   for Super I/O and EC-adjacent registers. 9 MSI `_STA`s, 4 M910q evaluations, and the MSI and M75q DSDT loads
   die here.
5. **`aml` 0.16.4 lacks most of the extended-opcode set (#9, #23–28).** It cannot load the MSI, M75q, M910q or
   OptiPlex DSDT. Its errors (`UnexpectedByte(0x5B)`) name no opcode. The harness names the candidates by
   scanning the failing table or method body for extended opcodes the crate has no arm for.
6. **Cascades (#3 and part of #1).** After a DSDT fails to load, every SSDT that scopes into `\_SB.PCI0` fails
   with `LevelDoesNotExist`/`ValueDoesNotExist`. Those are consequences, not separate defects. After an `acpi`
   6.1.1 panic, its interpreter state (method context stack) is not reset, so later failures in the same set
   may be artifacts. The rankings count them as observed.

**Corpus-evidenced observations that no class column shows:**
- **Silent mis-addressing inside a region.** `aml` 0.16.4's QEMU `LNKB._STA` read PCI config 0x68 instead of
  0x61, which is still inside PIRQ, so the region check can't see it.
- **A stock init entry point that hides failures.** `aml` 0.16.4's `initialize_objects()` skips `Processor`
  levels, so on QEMU it returns `Ok` without reaching the failing CPU `_STA`s.
- **No parse error carries a byte offset** in either crate (L10).
- **No evaluation hit the access budget** (no non-termination under the zero model).
- **No table checksum was bad** in any delivered set.

### 5a. Gaps read from source

These were recorded from source before the real-hardware sets arrived. The ones the real-hardware run has
since **confirmed** are marked; the rest are still unexercised.

**`acpi` 6.1.1:**
- **Region spaces.** Every space other than SystemMemory, SystemIO and PCI_Config (EmbeddedControl, SMBus,
  CMOS, PciBarTarget, IPMI, GPIO, GenericSerialBus, PCC, OEM) fails with `LibUnimplemented`, *even with a handler
  installed* ("Custom region handlers aren't actually supported yet"). `install_region_handler` panics on a
  second install. Laptops (the Zenbook) need EC and GPIO/GSB for battery, lid and thermal.
- **PCI bus number.** The PCI address is derived from `_SEG`/**`_BBR`**/`_ADR`. `_BBR` is not an ACPI object;
  the base bus number is `_BBN`. The bus is therefore always 0, which misaddresses any device behind a root
  bridge with a non-zero base bus. `aml` 0.16.4 uses `_BBN` correctly.
- **I/O port overflow.** SystemIO addresses are computed as `region.base as u16 + offset as u16`. That
  overflows, which panics in debug builds and wraps in release.
- **Firmware-reachable panics.**
  - `todo!()` on Bank and Index field access (**confirmed**: MSI, Lenovo, HP).
  - `initialize_namespace()` panics if a `_STA` returns a non-integer (`let Object::Integer(..) = .. else { panic!() }`)
    (**confirmed**: HP).
  - The default `Handler::handle_fatal_error` panics.
  - In the AML module (`src/aml/`, grep counts): 36 `todo!`/`unimplemented!` sites and 76 `panic!` sites.
  - The module's own TODO list includes "loop timeouts", "method recursion depth" and "guarantee panic-free
    interpretation".
- **A second hardware path outside the Handler.** FADT fixed registers in SystemMemory are read and written by
  raw volatile pointer through `map_physical_region`, not through `Handler::read_*`/`write_*`. A mediation seam
  on the read/write methods alone would miss them.
- **No region identity at the Handler.** The Handler receives flat addresses only (`read_u8(address)`), never
  the OpRegion. A per-region capability check is impossible at today's seam.
- **Unimplemented features per its TODO list:** Load/LoadTable (dynamic SSDTs), Notify, DefMatch,
  DefDataRegion, `_REG`.

**`aml` 0.16.4:**
- **Opcode coverage.** No parser arm exists for these operators: Subtract, Multiply, Divide, Mod, Or, Nor, Nand,
  Xor, Not, Index, SizeOf, DerefOf, RefOf, Notify, Match, FindSetLeft/RightBit, ToBuffer, ToHexString,
  ToDecimalString, ToString, CopyObject and VarPackage. Nor for these extended operators: Acquire, Release,
  Event, Signal, Wait, Reset, Sleep, Stall, Load, LoadTable, Timer, FromBCD, ToBCD, IndexField, BankField and
  DataRegion. (Derived from the crate's parser arms; `classify.rs` `AML016_OPS`/`AML016_EXT`.) Real firmware
  uses most of these.
- **Panics.** `unimplemented!()` on any region space other than Memory/IO/PCI; `todo!()` on `Target::Null`/`Debug`
  reads. In total (grep counts over `src/`, test helpers included), 48 `todo!`/`unimplemented!` sites and
  14 `panic!` sites.

## 6. Harness adaptations: everything the grade needed beyond calling the crates

Gate item 4 requires these to be reported as findings rather than buried.

1. **Nothing was patched, forked or vendored.** `Cargo.toml` pins crates.io releases.
2. **`acpi` 6.1.1 needs more than AML to start.** `Interpreter::new` requires an `Arc<FixedRegisters>` built
   from a mapped FADT, and an optional mapped FACS (for the global lock).
   - How the harness supplies them: it maps the corpus FADT and FACS at synthetic physical addresses, in
     8-byte-aligned buffers padded to at least 4 KiB.
   - **Finding (source-read):** the crate gates the FADT's extended fields on `header.revision`, not
     `header.length`. Its own doc on `ExtendedField::access` says a bogus revision "may access uninitialised
     data". A FADT whose revision overstates its length is therefore read past its end. Also, `find_table`
     maps a table at its header length, and for QEMU's 244-byte revision-3 FADT that is smaller than
     `size_of::<Fadt>()` (276). That breaks the `Handler::map_physical_region` safety contract the crate itself
     states ("`size` must be at least `size_of::<T>()`").
3. **Both crates panic on firmware-reachable paths,** so the harness wraps every call in `catch_unwind`.
   - **That only works with unwinding, and this workspace's `dev` and `release` profiles set `panic = "abort"`**
     (kernel needs). On the first real-hardware run, the first `acpi` 6.1.1 `todo!()` therefore aborted the
     whole process, silently.
   - **The fix:** the binary now refuses an abort build and says how to run it
     with `CARGO_PROFILE_DEV_PANIC=unwind` (bash), or `$env:CARGO_PROFILE_DEV_PANIC="unwind"` (PowerShell).
     An environment variable needs no quoting. The equivalent `cargo run -p acpi-aml-grade --config
     'profile.dev.panic="unwind"'` also works in bash, but PowerShell strips its inner quotes.
     `cargo test`, where the gate lives, always unwinds.
   - **What it says about the kernel:** the kernel *is* an abort build. In LogansOS every one of those panics
     is a dead machine, not a failed method. That is the strongest single argument for L8.
4. **`aml` 0.16.4's own init skips `Processor` levels** (§5), so the harness writes its own exhaustive walk and
   reports the stock init separately ("stock init" in the per-set lines).
5. **Neither crate reports what it lacks.**
   - `aml` 0.16.4: `UnexpectedByte(0x5B)` / `Unimplemented`.
   - `acpi` 6.1.1: `LibUnimplemented`.
   - Neither error type carries an offset or a feature name. The harness recovers names mechanically (call-path
     opcode scan; panic site file:line). Failures it cannot name stay `UNCLASSIFIED` and fail the gate.

6. **ACPICA host tools were used, as tools, outside the build.** This is disclosed because the brief rules out
   "ACPICA in any form".
   - **Where:** the Ubuntu `acpica-tools` package (`iasl` 20230628), on the host only.
   - **What for:**
     - disassembling corpus tables to root-cause failures;
     - cross-checking the hand-decoded touchpad `_CRS`;
     - producing the HID-over-I2C UUID byte encoding (`ToUUID` compiled by `iasl`, then confirmed present in the
       Zenbook DSDT).
   - **What it never touched:** the harness, the grading, the build or the kernel; no ACPICA code is in the tree.
   - **Precedent:** the brief itself prescribes ACPICA's `acpidump.exe` for extraction.
   - **If the PL reads "in any form" as covering host analysis too:** later rungs can use hand-laid byte
     fixtures only (the L3 dispatch proposes exactly that).

## 7. Kernel untouched

- **No kernel changes.** The diff touches no file under `kernel/`, `userland/` or `tools/xtask`. Changed files:
  - new `fixtures/acpi/`;
  - new `tools/acpi-aml-grade/`;
  - one workspace `members` line in `Cargo.toml`;
  - `Cargo.lock`;
  - docs.
- **`Cargo.lock` only gained entries:** `acpi`, `aml`, `bitvec`, `funty`, `pci_types`, `radium`, `spinning_top`
  0.3.0, `tap`, `wyz`. One existing line was disambiguated: `spinning_top` → `spinning_top 0.2.5`, the same
  version as before. No existing package's version moved.
- **The default kernel binary is bit-for-bit unchanged.** `cd kernel && cargo build -Zjson-target-spec` was run
  with `origin/main`'s `Cargo.toml`/`Cargo.lock` and with this branch's, into separate clean target dirs from
  the same checkout path. The ELF SHA-256 matched, re-measured at branch head `a25a763`:
  `042d8d40fc079f1a4fa9c7631ea4ebdea23c70a7d907f5c6518a7cf3ed12947e` for both. (An earlier pair, built in
  another session, matched each other at `38339f5b…`. Only same-run pairs are compared.) (Builds from two different
  checkout paths differ only because a debug build embeds absolute paths. The same-path comparison is the
  controlled one.)
- **The harness uses its own SHA-256, not a RustCrypto crate,** so no shared crypto dependency or feature set
  can unify into the no_std members (OT-21).

## 8. Extraction instructions for J.c.: the Zenbook, the OptiPlex, and every other machine

**Every machine you own is in scope.** These are machines the project controls, so each one is a corpus set,
not only the two reference machines. The AMD desktop (MSI MS-7E57, Ryzen 9 9900X) is especially valuable:
it is the only non-Intel firmware so far. Most of these machines boot Windows, so the Windows route comes first.

**Everything here is read-only.** acpidump reads the firmware tables Windows already holds and writes only into
the folder you run it from. Nothing here involves LogansOS or the Zenbook's disk beyond an ordinary file save.
Put the output folder on a USB stick if you prefer.

**Delete `msdm.dat` and `slic.dat` before uploading** (MSDM carries the OEM Windows licence key). The pipeline
also refuses them by signature, so one that slips through is dropped and noted in provenance, never
committed. Deleting them at the source keeps the key off the upload too.

### Windows (preferred: every Windows machine)

1. **Download the official ACPICA Windows binary tools** from Intel's ACPICA page:
   ["ACPI Component Architecture Downloads (Windows Binary Tools)"](https://www.intel.com/content/www/us/en/download/774881/acpi-component-architecture-downloads-windows-binary-tools.html).
   It is a single zip containing `acpidump.exe`, `iasl.exe` and friends. Unzip it to, for example,
   `C:\acpica\`. Before unzipping, record the zip's SHA-256 for provenance:
   `Get-FileHash .\<zip> -Algorithm SHA256`.
2. **Open PowerShell as Administrator** and make an empty folder named for the machine and BIOS version.
   Run the commands below on each machine:
   ```powershell
   $bios = (Get-CimInstance Win32_BIOS)
   $cs   = (Get-CimInstance Win32_ComputerSystem)
   $name = "zenbook-s14-ux5406aa"        # OptiPlex: "optiplex-7080"; MSI desktop: "msi-ms-7e57"; others: "<maker>-<model>"
   $dir  = "$HOME\Desktop\$name-$($bios.SMBIOSBIOSVersion -replace '[^A-Za-z0-9.]','_')"
   New-Item -ItemType Directory $dir | Out-Null; Set-Location $dir
   C:\acpica\acpidump.exe -b             # one binary file per table: dsdt.dat, ssdt1.dat, facp.dat, ...
   C:\acpica\acpidump.exe -v > acpidump-version.txt
   Remove-Item msdm.dat, slic.dat -ErrorAction SilentlyContinue   # the OEM licence key never leaves the machine
   "machine=$($cs.Manufacturer) $($cs.Model)`nbios_vendor=$($bios.Manufacturer)`nbios_version=$($bios.SMBIOSBIOSVersion)`nbios_date=$($bios.ReleaseDate)`nos=$((Get-CimInstance Win32_OperatingSystem).Caption) $((Get-CimInstance Win32_OperatingSystem).Version)`ndate=$(Get-Date -Format yyyy-MM-dd)" | Out-File machine.txt -Encoding ascii
   Get-FileHash *.dat -Algorithm SHA256 | ForEach-Object { "$($_.Hash.ToLower())  $(Split-Path $_.Path -Leaf)" } | Out-File SHA256SUMS -Encoding ascii
   ```
3. **Send back the folder as-is** (zip it and upload it to the PM). Expected layout:
   ```
   zenbook-s14-ux5406aa-<BIOS version>/
     dsdt.dat  facp.dat  facs.dat  apic.dat  ssdt1.dat ... ssdtN.dat  (+ every other *.dat acpidump wrote)
     machine.txt            # model, BIOS vendor/version/date, OS, date (a bios.txt with the same facts works too)
     acpidump-version.txt   # the extraction tool's version
     SHA256SUMS             # hashed at the source, so transport damage is caught
   optiplex-7080-<BIOS version>/
     (same)
   ```
   `tools/acpi-aml-grade/extract/ingest_set.py` turns the folder into a corpus set:
   - it copies every table byte for byte, except MSDM/SLIC;
   - it checks the delivered `SHA256SUMS` (stopping on any mismatch);
   - it records each table's checksum;
   - it builds `provenance.json` from `bios.txt`/`machine.txt` (UTF-16 or UTF-8).

### Linux (alternative: needs a fresh Ubuntu live stick; the Kingston is now the image stick)

```bash
M=optiplex-7080            # or zenbook-s14-ux5406aa
D=/media/$USER/<usb>/$M-$(sudo dmidecode -s bios-version | tr -c 'A-Za-z0-9.\n' _)
sudo cp -r /sys/firmware/acpi/tables "$D"          # includes dynamic/ (runtime-loaded SSDTs)
{ echo "machine=$(sudo dmidecode -s system-manufacturer) $(sudo dmidecode -s system-product-name)";
  echo "bios_version=$(sudo dmidecode -s bios-version)"; echo "bios_date=$(sudo dmidecode -s bios-release-date)";
  echo "os=$(lsb_release -ds)"; echo "date=$(date -I)"; } | sudo tee "$D/machine.txt"
sudo rm -f "$D/MSDM" "$D/SLIC"                        # the OEM licence key never leaves the machine
cd "$D" && sudo find . -type f ! -name SHA256SUMS ! -name machine.txt -exec sha256sum {} + | sudo tee SHA256SUMS
```

Copy to the USB stick, never to an internal disk. On Linux the `dynamic/` subfolder captures SSDTs that firmware
loads at runtime. Windows' acpidump sees only tables in the XSDT. That difference will be visible in the corpus.

**Adding a delivered set** (the PM or the lane does this once the folder is uploaded):
```bash
python3 tools/acpi-aml-grade/extract/ingest_set.py --name msi-ms-7e57-<bios version> \
    --machine "MSI MS-7E57 (Ryzen 9 9900X)" --tool "acpidump.exe -b (ACPICA Windows binary tools)" \
    --os "Windows 11" <uploaded folder> fixtures/acpi/msi-ms-7e57-<bios version>
CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade   # regenerates tools/acpi-aml-grade/report/
cargo test -p acpi-aml-grade         # fails on any UNCLASSIFIED failure until it is classified
```
Commit the set together with the regenerated report. Tables are identified by their header signature, so the
Windows names (`dsdt.dat`), the Linux names (`DSDT`, plus `dynamic/` flattened to `dynamic-SSDT…`) and the
extractor's names all load.

## 9. The Loganfy delta (a list, not code), filed as OT-86's candidate rungs

Two **standing design constraints** bind every later rung (recorded here as the brief requires):

- **(i) In-kernel OpRegion access is capability-mediated and range-checked per access, at the handler seam.**
  Today neither crate's seam can carry this. The Handler sees flat addresses without region identity, and
  `acpi` 6.1.1 has a second, non-Handler path (`map_physical_region` + raw volatile for FADT registers). The
  seam must see `(space, region base, region length, offset, width, direction)` on every access, from one path.
- **(ii) AML executes in a preemptible or step-budgeted context, so hostile or broken firmware degrades a
  subsystem instead of hanging the kernel.** Neither crate has loop timeouts, recursion limits or a step
  counter. Both panic on firmware-reachable paths, and a kernel panic is the opposite of "degrade a subsystem".

The candidate rungs, in dependency order, re-ranked by the real-hardware run:

| # | Candidate rung | Evidence |
|---|---|---|
| L1 | **Corpus completion.** Delivery is closed for J.c.'s machines (the M710q is not available), but the corpus is incomplete. The OptiPlex now has a Linux set (static + runtime `dynamic/` SSDTs, FACS; 2 XSDT entries in neither dump). Still to do: Linux dumps of HP (3 missing), M910q (2) and MSI (1). Each new set is rebaselined (stock crates, unchanged judge) before any code change is measured against it (§4b). | §2, §4b, §8 |
| L2 | **DECIDED 2026-10-23 (J.c.: "Acpi is a go"). Base selection: `acpi` 6.1.1's interpreter, not `aml` 0.16.4.** On real hardware it loads 3/6 DSDTs against 0/6 and reaches 397 `_STA`/`_INI` against 14. It has the far larger opcode set and is where upstream works. Fork/vendor mechanics (into `tools/` first, host-only) are their own rung. | §4, §5 |
| L3 | **Field-unit operand type confusion** (the #1 real-hardware mode, 71 occurrences on all six real sets, and the OptiPlex DSDT load failure): trace to the line, fix, KAT on the Zenbook `LNKA`/`HDC1` cases. | §5 #1 |
| L4 | **Missing field kinds, semantics and resource descriptors.** IndexField/BankField read and write (`todo!()` panics: 17 occurrences on 4 of 6 real machines); Field `Connection()` (`todo!`, OptiPlex); Buffer comparison longer than 4 bytes (every UUID `_DSM` panics; `mod.rs:1889`); the resource decoder's GPIO Connection and GenericSerialBus Connection descriptors (`unimplemented!()`: the Zenbook touchpad's I2C + interrupt `_CRS`); `RefOf` store; then Load/LoadTable, Notify, Match, DataRegion. **The touchpad and touchscreen lanes need the first three.** | §4a, §5 #4, §5a |
| L5 | **Address-formation fixes, each with an independent byte oracle.** PCI_Config base drop; `_BBR`→`_BBN`; u16 port overflow. The QEMU PIRQ case is the ready-made fixture (expected bytes 0x60–0x63/0x68–0x6B). | §5 #2, §5a |
| L6 | **The mediated region seam (constraint i).** One access path carrying region identity; all three hardware paths (Handler read/write, `map_physical_region`, FADT GAS) routed through it; a per-access capability and range check; out-of-region = refused + logged, never performed. | §5a, §6.2 |
| L7 | **Step budget and preemption (constraint ii).** Op counter, loop timeouts, recursion depth; budget exhaustion returns a subsystem-level error. The harness's access budget is the host prototype. | §3, §5a |
| L8 | **Panic-free interpretation, and recovery after an error.** Remove the 36 `todo!`/`unimplemented!` + 76 `panic!` sites from the reachable surface (each becomes a typed error). Reset interpreter state after a failed method. Fuzz with the corpus as seeds. FADT extended fields gated on length as well as revision. | §5, §5a, §6.2–3 |
| L9 | **Region spaces real firmware needs.** EmbeddedControl first (the Zenbook ships an ECDT), then GPIO/GenericSerialBus, SMBus, CMOS, PCC; `_REG` execution. Not yet reached by the corpus: the evaluations that would need them fail earlier (L3/L4). Re-rank after those land. | §5a |
| L10 | **Diagnosable errors.** Parse errors with byte offsets; unimplemented errors that name the opcode or feature. | §5, §6.5 |
| L11 | **An independent field-offset oracle with PCI device identity.** An expected-access trace decoded from the tables by our own Field-list reader, compared against the recorded trace. It catches in-region mis-addressing (`aml` LNKB; `acpi` 6.1.1's PCI base drop landing inside a device's whole-config-space region on real firmware). | §5 |

Out of scope for every rung, per the PL's decision: ACPICA in any form in the kernel. (The host-side ACPICA
`acpidump` in §8 is only the extraction tool the brief prescribes; it never enters the build.)

## 10. Reproduce

```bash
# corpus (QEMU sets; needs qemu-system-x86, ovmf, seabios on the host)
python3 tools/acpi-aml-grade/extract/qemu_extract.py --seabios --boot-wait 12 --name qemu-q35-seabios-smp1 --out fixtures/acpi/qemu-q35-seabios-smp1
python3 tools/acpi-aml-grade/extract/qemu_extract.py --name qemu-q35-ovmf-smp1 --out fixtures/acpi/qemu-q35-ovmf-smp1
python3 tools/acpi-aml-grade/extract/qemu_extract.py --name qemu-q35-ovmf-smp4-viommu --smp 4 --device intel-iommu,intremap=off,caching-mode=on --out fixtures/acpi/qemu-q35-ovmf-smp4-viommu
# real-hardware sets: ingest a delivered folder (see section 8)
# grade + report (needs unwinding: the workspace profiles are panic = "abort"), then the gate tests
CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade
cargo test -p acpi-aml-grade
```
