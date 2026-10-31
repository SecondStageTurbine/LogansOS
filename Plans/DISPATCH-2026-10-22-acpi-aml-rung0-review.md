<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family review of ACPI/AML rung 0 (OT-86)

MODEL: GPT-6-Luna / EFFORT: high. The change is contained: one host-only tool crate plus fixtures and docs.
It has no SMP core, no kernel code and no new syscall.

**For:** a Codex-family seat (the PM relays).
**Writer:** S5 (Claude), so the reviewer must be from a different family (CLAUDE.md §Cross-model diversity).
**Branch:** `claude/acpi-aml-rung0` (PR to `main`; the PL reserves the merge).
**WORKTREE:** `E:\AI Automation\workers\<your seat>`. It is yours alone; never another coder's folder, never the
repo root.
**Read:**
```
git fetch origin
git checkout -B claude/acpi-aml-rung0 origin/claude/acpi-aml-rung0
git diff origin/main...origin/claude/acpi-aml-rung0 --stat
git show origin/claude/acpi-aml-rung0:docs/RESEARCH-2026-10-22-acpi-aml-corpus-FINDINGS.md
```
**Run:** `cargo test -p acpi-aml-grade`, then `CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade` (PowerShell:
`$env:CARGO_PROFILE_DEV_PANIC='unwind'; cargo run -p acpi-aml-grade`) from the repo root. The second
must leave `git status` clean, because the committed report is already current.

RAG (semantic code search) is available. Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health`.
If it is down, note that in your RETURN and use grep/Read; it is never a blocker. Example query:
`$body=@{query='ACPI table parsing MADT FADT'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

**How to answer.** For each item, give **PASS** or **FAIL** and the `file:line` that shows it. For a FAIL, add
the concrete input or case. Say what you checked for every PASS. An uncertain point is a note, not a FAIL.

**Delivery.** Commit your answers as `Plans/RETURN-2026-10-22-acpi-aml-rung0-review.md` on the same branch and
push. A review delivered only in chat does not exist (CLAUDE.md §"Work is only real when it is on origin").

## 1. Scope and kernel isolation

- [ ] **1a** No file under `kernel/`, `userland/` or `tools/xtask/` changed (`git diff --stat`).
- [ ] **1b** `Cargo.lock` only gained entries. No existing package's version changed. The one changed existing
  line is a version disambiguation (`spinning_top` → `spinning_top 0.2.5`).
- [ ] **1c** `kernel/Cargo.toml` does not depend on `acpi` or `aml`.
- [ ] **1d** The default kernel build is identical with and without the branch's `Cargo.toml`/`Cargo.lock`, built
  from the same checkout path into separate target dirs. The findings §7 states the hash; reproduce it if
  you can.
- [ ] **1e** The crates are stock. `tools/acpi-aml-grade/Cargo.toml` pins `aml = "=0.16.4"` and
  `acpi = "=6.1.1"` from crates.io, with no `[patch]`, path, git or vendored source.

## 2. Corpus integrity and provenance

- [ ] **2a** Each `fixtures/acpi/<set>/SHA256SUMS` matches the files (`sha256sum -c`), and every listed file
  exists.
- [ ] **2b** Every table except RSDP and FACS has a valid checksum (byte sum ≡ 0 mod 256 over `length`).
- [ ] **2c** Each `provenance.json` names its set, machine, firmware and package version, QEMU arguments,
  extraction tool, OS and date, and its per-table SHA-256 values equal `SHA256SUMS`.
- [ ] **2d** `qemu-q35-seabios-smp1`'s QEMU arguments match what `cargo neo-test` uses by default: `-machine q35`,
  `-cpu qemu64,+rdrand,+smep,+smap`, `-m 512M`, `-smp 1`, no `-bios` (`tools/xtask/src/main.rs`, the
  `neo-test` QEMU launch and `qemu_memory_for_scenario`).
- [ ] **2e** `extract/qemu_extract.py` reads guest RAM only. Check its RSDP selection: v1 checksum, plus v2
  checksum when revision ≥ 2; newest revision preferred; same-revision copies must be byte-identical or it
  exits.
- [ ] **2f** The corpus holds no third-party table set.
- [ ] **2g** MSDM and SLIC can never be committed. Check three things:
  - `extract/ingest_set.py` drops them by header signature, not file name, and records the drop;
  - `src/corpus.rs` `FORBIDDEN` keeps them out of `tables`;
  - `tests/corpus.rs` fails while any is present (`licence_tables_are_refused_by_the_loader`).
- [ ] **2i** The six real-hardware sets are byte-identical to the PL's delivery. Compare each table's SHA-256
  with `git show 1026a5b:acpi-drop/<machine>/<file>` from `origin/claude/phase-4-continuation-TlW3j`. The
  ThinkCentre's `provenance.json` records MSDM and SLIC as dropped by the PM before commit, and no
  `acpi-drop/` directory is on this branch.
- [ ] **2j** XSDT completeness is recorded, not hidden. Each real set's `provenance.json` has `xsdt_completeness`:
  the entry count from the XSDT header length, the delivered XSDT tables, MSDM/SLIC as excluded by rule, and
  the unexplained remainder. Check by hand: the OptiPlex shows 24 entries, 19 delivered, 5 missing, gap `ssdt2`.
  The findings §2 table and `coverage.md` show the same numbers.
- [ ] **2h** `ingest_set.py` copies tables byte for byte, stops when a delivered `SHA256SUMS` doesn't match,
  records table checksums without altering them, and refuses to overwrite an existing set.

## 3. The harness grades without touching hardware

- [ ] **3a** Every `Handler` method in `grade_aml016.rs` and `grade_acpi6.rs` routes to `Recorder::read`/`write`,
  or to the fake clock or mutex counter. None performs port I/O, MMIO or a real physical mapping.
- [ ] **3b** `H6::map_physical_region` returns only leaked, padded heap copies of corpus tables, or zeroed scratch
  memory for non-table addresses, which it records.
- [ ] **3c** The scripted model is deterministic. Two runs produce the same `coverage.json`.
- [ ] **3d** Each evaluation's access budget is enforced in `Recorder::read`/`write`. The budget panic is raised
  only after the mutex guard is dropped (no poisoned lock).
- [ ] **3f** `src/main.rs` refuses a `panic = "abort"` build (the workspace `dev`/`release` profiles), so a crate
  panic can never silently end the run, and `catch_unwind` is the only panic path in the graders.
- [ ] **3e** The `_STA`/`_INI` walk evaluates every `_STA` and `_INI` in the namespace for both crates. Cross-check
  the counts in `coverage.json` against the QEMU DSDT (`iasl -d fixtures/acpi/qemu-q35-ovmf-smp1/dsdt.dat`, on
  your own machine: 17 `_STA`, 1 `_INI` for smp1; 20 and 1 for smp4).

## 4. Classification

- [ ] **4a** `tests/corpus.rs` fails if any failure is `UNCLASSIFIED` or has an empty mode key.
- [ ] **4b** Check the address-formation diagnosis (`classify.rs` `diagnose_access`):
  - "region base dropped" fires only when the access fits inside `[0, region.length)`;
  - "bit offset used as byte offset" fires only when `(addr - base)` is a multiple of 8 and `base + (addr - base)/8` is inside the region;
  - any other out-of-region access still gets a named mode.
- [ ] **4c** The ext-opcode table `EXT_OPS` and the operator table `OPS` match ACPI 6.5 §20.3. Compare against
  the spec or `acpi` 6.1.1's decode table (`src/aml/mod.rs`, the `0x5b01 => Opcode::Mutex` block).
- [ ] **4d** `AML016_EXT` and `AML016_OPS` match the parser arms in `aml` 0.16.4: every
  `ext_opcode(opcode::EXT_*)` and `opcode(opcode::DEF_*)` in its source.
- [ ] **4e** The `aml` 0.16.4 Acquire/Release finding holds: QEMU's CPU `_STA` reaches `\_SB.CPUS.CSTA`, which
  contains `Acquire` (0x5B 0x23), and `aml` 0.16.4 has no arm for 0x23.

## 5. The three ranked gaps are real crate defects

- [ ] **5a** `acpi` 6.1.1, `src/aml/mod.rs`, `do_native_region_read`/`do_native_region_write`: the `PciConfig` arm
  passes the field offset without `region.base`, while the SystemMemory and SystemIO arms add it. The QEMU PIRQ
  region is `PCI_Config` base 0x60, length 0x0C, and `LNKA`..`LNKH` read `PRQA`..`PRQH` at 0x60–0x63 and
  0x68–0x6B. The report shows reads at 0x0–0x3 and 0x8–0xB.
- [ ] **5b** `aml` 0.16.4: `AmlValue::Field.offset` is in bits (`value.rs`), and `read_region` (`lib.rs`) adds it
  to `region_base` unchanged in all three spaces. The report's 0x70 for `LNKC` is 0x60 + 0x10.
- [ ] **5c** The source-read claims in findings §5a and §6 are accurate as stated:
  - `_BBR` vs `_BBN` (`acpi` 6.1.1 `pci_address_for_device`);
  - the `u16` port sum;
  - `LibUnimplemented` for custom region spaces;
  - `todo!()` on Bank/Index fields;
  - the `initialize_namespace` `_STA` panic;
  - FADT extended fields gated on revision only;
  - `aml` 0.16.4 `initialize_objects` skipping `Processor` levels.

- [ ] **5d** Check the Zenbook device probes (findings §4a, `fixtures/acpi/zenbook-s14-ux5406aa-303/probes.json`):
  - the probe paths match the set's DSDT, and EC0 is the ECDT's EC_ID;
  - `HID_I2C_UUID` (`src/probe.rs`) equals `iasl`'s `ToUUID("3cdff6f7-4267-4555-ad05-b30a3d8938de")` and occurs
    in the Zenbook DSDT (test `hid_i2c_uuid_is_the_one_the_zenbook_firmware_uses`);
  - the `acpi` 6.1.1 causes cited are the actual lines:
    - `mod.rs:1889`: a Buffer compare copies `left` into `[u8; 4]`;
    - `resource.rs`: `unimplemented!("GenericSerialBus Connection Descriptor")` and `unimplemented!("GPIO Connection Descriptor")`;
  - the hand-decoded touchpad `_CRS` bytes are I2cSerialBusV2 0x15 @ 400 kHz plus Interrupt 0x69.

## 6. Findings honesty

- [ ] **6a** The corpus present/missing list in the findings matches `fixtures/acpi/`:
  - QEMU ×3; Zenbook, OptiPlex 7080, MSI MS-7E57, Lenovo M75q, HP EliteDesk and Lenovo M910q present; the M710q named as not delivered;
  - QEMU and real-hardware results reported separately, with each crate's "reached" count beside its
    percentages;
  - the harness handles N sets: per set first, then aggregate.
- [ ] **6b** Every number in findings §4 and §5 equals `coverage.json`.
- [ ] **6c** The harness adaptations (findings §6) are reported as findings, and none of them changes either
  crate's behaviour.
- [ ] **6d** The extraction instructions (§8) are read-only on the target machines and write output only to the
  chosen folder. The Windows route uses the official Intel ACPICA Windows binary tools.
- [ ] **6e** The two standing design constraints and the L1–L10 candidate rungs are recorded in the findings
  and in `docs/OPEN-THREADS.md` OT-86.

## 7. Host CI set

- [ ] **7a** `cargo fmt --all --check`, `cargo test --workspace --exclude neoplutarch-kernel`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and the SPDX check all pass on the branch head.

## Re-check of the two FAILs in `Plans/RETURN-2026-10-22-acpi-aml-rung0-review.md`

MODEL: GPT-6.1-Sol / EFFORT: medium. This is a narrow re-check of the reviewer's own two findings, and only
these two items changed. Commit it as `Plans/RETURN-2026-10-23-acpi-aml-rung0-recheck.md`, on the branch.

- [ ] **R-7a (Windows path portability).** On a clean Windows checkout of the new head:
  - `cargo test -p acpi-aml-grade` passes, including `committed_report_matches_the_harness`;
  - `$env:CARGO_PROFILE_DEV_PANIC='unwind'; cargo run -p acpi-aml-grade` leaves `git status` clean.

  The fix is that `recorder::short_path` normalizes `\` to `/` before trimming panic locations (test
  `panic_sites_are_host_independent`). The documented invocation is now the env var, which needs no TOML
  quoting.
- [ ] **R-6a (corpus completeness wording).** The findings summary and §2, OT-86 and the fixtures README now say
  that delivery is closed (every machine J.c. has) and that the corpus is **not complete**. Check that each of
  these is named as a gap:
  - the M710q (missing);
  - four sets with unaccounted XSDT entries;
  - no Linux runtime `dynamic/` SSDTs.

  No text may still call the corpus "final" or complete.
