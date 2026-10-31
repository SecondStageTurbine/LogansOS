# RETURN: ACPI/AML rung 0 review (OT-86)

**Reviewer:** Codex (GPT-6-Luna)
**Reviewed head:** `e79b38be962464c1d8bbb04c014fedf64ad77096`
**Result:** 2 FAILs; see 6a and 7a. The updated branch contains nine sets, so the dispatch's QEMU-only premise is stale.

## 1. Scope and kernel isolation

- **1a PASS** — `git diff origin/main...HEAD --stat` has no paths under `kernel/`, `userland/` or `tools/xtask/`; findings also lists only fixtures, the host tool, workspace metadata and docs (`docs/RESEARCH-2026-10-22-acpi-aml-corpus-FINDINGS.md:395-400`).
- **1b PASS** — `Cargo.lock` adds the grader's dependency graph and disambiguates `spinning_top` 0.2.5 from newly added 0.3.0; no pre-existing package version moved (`Cargo.lock:1579-1589`, findings `:401-403`).
- **1c PASS** — `kernel/Cargo.toml` has no `acpi` or `aml` dependency; the new workspace member is the host-only tool (`tools/acpi-aml-grade/Cargo.toml:17-18`).
- **1d PASS (reported, not rerun)** — findings records the same-path, separate-target comparison and matching ELF SHA-256 `042d8d40fc079f1a4fa9c7631ea4ebdea23c70a7d907f5c6518a7cf3ed12947e` (`docs/RESEARCH-2026-10-22-acpi-aml-corpus-FINDINGS.md:404-410`). I did not reproduce the kernel build in this review.
- **1e PASS** — exact crates.io versions are pinned; no patch/path/git source is present (`tools/acpi-aml-grade/Cargo.toml:17-21`; workspace diff has no `[patch]`).

## 2. Corpus integrity and provenance

- **2a PASS** — the corpus gate verifies every delivered `SHA256SUMS` line against an existing file (`tools/acpi-aml-grade/tests/corpus.rs:30-36`); that test passed after report regeneration.
- **2b PASS** — the gate sums each non-RSDP/FACS table through its declared length and compares with provenance (`tools/acpi-aml-grade/tests/corpus.rs:87-98`); all passed.
- **2c PASS** — each set's provenance is checked for set, machine, firmware/version, tool, OS and date, and the set name must match (`tools/acpi-aml-grade/tests/corpus.rs:42-61`). Provenance table checksums are compared by the same gate at `:74-98`.
- **2d PASS** — the SeaBIOS set is the default `neo-test` QEMU shape; `xtask` uses `q35`, the expected CPU, 512M fallback memory and `smp_count()` (`tools/xtask/src/main.rs:8074-8076`, `:8105-8123`, `:8402-8403`; findings `:120-121`). There is no `-bios` in that launch; `-boot d -cdrom` selects the default SeaBIOS firmware.
- **2e PASS** — the extractor accepts RSDPs only after v1 and (revision >= 2) v2 checksums, prefers the highest revision, and refuses differing same-revision candidates (`tools/acpi-aml-grade/extract/qemu_extract.py:70-92`, `:196-209`). It snapshots guest RAM through QMP `pmemsave` (`:177-187`).
- **2f PASS** — the nine sets are the three project QEMU setups and six project-owned machines; provenance documents their origins and says no third-party DSDT was used (`docs/RESEARCH-2026-10-22-acpi-aml-corpus-FINDINGS.md:63-78`, `:127-128`).
- **2g PASS** — ingestion excludes by four-byte table signature and records the drop (`tools/acpi-aml-grade/extract/ingest_set.py:35`, `:150-153`); the loader excludes both signatures (`tools/acpi-aml-grade/src/corpus.rs:23-25`, `:132-138`); the test rejects any set containing either (`tools/acpi-aml-grade/tests/corpus.rs:23-29`, `:157-164`). No MSDM/SLIC table file exists in the corpus.
- **2h PASS** — ingest refuses a nonempty destination, verifies source hashes before ingest, copies bytes unchanged, records each table hash, and writes checksums (`tools/acpi-aml-grade/extract/ingest_set.py:118`, `:135-153`, `:169-178`, `:241-244`).

## 3. Recording-only harness

- **3a PASS** — all `aml` handler reads/writes route through `Recorder`; the `acpi` handler does likewise, with fake clock and mutex operations (`tools/acpi-aml-grade/src/grade_aml016.rs:26-95`, `tools/acpi-aml-grade/src/grade_acpi6.rs:109-179`, `tools/acpi-aml-grade/src/recorder.rs:106-180`). No handler calls port I/O or maps real physical memory.
- **3b PASS** — `H6` maps leaked, aligned, padded copies for registered tables; unknown addresses receive zeroed heap scratch and are recorded (`tools/acpi-aml-grade/src/grade_acpi6.rs:39-105`).
- **3c PASS** — two same-host runs emitted 9 sets x 2 crates and 0 unclassified failures; `coverage.json` SHA-256 was identical both times: `DBE77DC14FACB6FDF761DFA3925993C142F0D8058117C06D6C4A6D4238159A70` (`tools/acpi-aml-grade/src/main.rs:25-40`).
- **3d PASS** — each read/write charges the per-evaluation budget; it explicitly drops the mutex guard before panicking when exhausted (`tools/acpi-aml-grade/src/recorder.rs:84-103`, `:106-117`, `:135-147`).
- **3e PASS** — both graders traverse the namespace and attempt each `_STA` and `_INI` (`tools/acpi-aml-grade/src/grade_aml016.rs:188-204`, `tools/acpi-aml-grade/src/grade_acpi6.rs:307-327`). QEMU aggregate counts match the report's per-set results (`tools/acpi-aml-grade/report/coverage.md:139-148`).

## 4. Classification

- **4a PASS** — the corpus test rejects `UNCLASSIFIED` and empty mode keys (`tools/acpi-aml-grade/tests/corpus.rs:115-132`); regenerated run reports zero unclassified failures.
- **4b PASS** — dropped-base diagnosis requires the full access to fit inside the region; bit-offset diagnosis requires an aligned multiple of eight and an in-range translated byte offset; the fallback names uncovered accesses (`tools/acpi-aml-grade/src/classify.rs:215-252`). Unit cases cover these diagnoses and fallback (`:539-566`).
- **4c PASS** — `EXT_OPS` and `OPS` cover ACPI 6.5 §20.3 operator tables, including the `0x5B01` Mutex entry (`tools/acpi-aml-grade/src/classify.rs:45-144`); compared with the pinned `acpi` 6.1.1 decode table (`C:\Users\<user>\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\acpi-6.1.1\src\aml\mod.rs`).
- **4d PASS** — `AML016_EXT` and `AML016_OPS` list the parser-supported byte values (`tools/acpi-aml-grade/src/classify.rs:146-156`); compared with all `ext_opcode`/`opcode` parser arms in pinned `aml` 0.16.4 (`C:\Users\<user>\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\aml-0.16.4\src\term_object.rs` and `term_arg.rs`).
- **4e PASS** — QEMU's CPU `_STA` path reaches `\_SB.CPUS.CSTA`, containing Acquire `5B 23`, for which the 0.16.4 parser has no arm; findings names the three QEMU occurrences (`docs/RESEARCH-2026-10-22-acpi-aml-corpus-FINDINGS.md:248-262`, `:322-360`; classifier list `:146-156`).

## 5. Ranked crate defects

- **5a PASS** — `acpi` 6.1.1's PCI_Config read/write path omits `region.base`, unlike the memory and I/O arms. The QEMU PIRQ OpRegion is base `0x60`, length `0x0C`; the report records `0x0` where `0x60` is expected (`docs/RESEARCH-2026-10-22-acpi-aml-corpus-FINDINGS.md:287-295`; `tools/acpi-aml-grade/report/coverage.md:47`).
- **5b PASS** — `aml` 0.16.4 documents `AmlValue::Field.offset` in bits and adds it directly to the region base in `read_region`; QEMU LNKC reports `0x70` instead of `0x62` (`docs/RESEARCH-2026-10-22-acpi-aml-corpus-FINDINGS.md:296-300`; report `:49`).
- **5c PASS** — source-read claims agree with the pinned crate sources: `_BBR` routing is at `acpi-6.1.1/src/aml/mod.rs:2633` (findings correctly contrasts `_BBN`); narrowed `u16` port arithmetic at `:2520` and `:2585`; unsupported custom spaces return `LibUnimplemented` at `acpi-6.1.1/src/address.rs:147`; Bank/Index field `todo!()` sites at `acpi-6.1.1/src/aml/mod.rs:2382-2387` and `:2442-2447`; noninteger `_STA` panic at `:256`; revision-gated FADT extended fields at `acpi-6.1.1/src/sdt/fadt.rs:136` and `:148`; and `aml` 0.16.4 Processor-level initialization omission at `aml-0.16.4/src/lib.rs:236-` (`docs/RESEARCH-2026-10-22-acpi-aml-corpus-FINDINGS.md:300-360`).

## 6. Findings honesty

- **6a FAIL** — the dispatch's QEMU-only / Zenbook-and-OptiPlex-missing premise no longer describes this head. Findings now calls the corpus “final” and lists nine sets (`docs/RESEARCH-2026-10-22-acpi-aml-corpus-FINDINGS.md:63-71`), although the M710q is unavailable and four real-machine dumps still have unexplained XSDT entries (`:83-101`); Linux runtime `dynamic/` SSDTs are also still pending (`:509`). This overstates corpus completeness. The report does correctly handle N sets, per set and aggregate (`:152-153`).
- **6b PASS** — the findings' §4/§5 numbers match regenerated `coverage.json`; the report-match integration test passed after regeneration (`tools/acpi-aml-grade/tests/corpus.rs:138-153`). The differences from the committed report were Windows path separators, not metric values.
- **6c PASS** — adaptations are explicitly described in findings §6 and preserve stock interpreter behavior: panic capture, exhaustive external namespace walk, fake handler and scratch mappings (`docs/RESEARCH-2026-10-22-acpi-aml-corpus-FINDINGS.md:362-390`; grader sources cited in §3 above).
- **6d PASS** — the instructions write dumps and metadata into the chosen output directory, do not modify firmware or target storage, and identify Intel's official ACPICA Windows binary tools (`docs/RESEARCH-2026-10-22-acpi-aml-corpus-FINDINGS.md:414-490`). Intel's download page describes the Windows ACPICA tools bundle, including AcpiDump and iASL: [Intel ACPICA Windows binary tools](https://www.intel.com/content/www/us/en/download/774881/acpi-component-architecture-downloads-windows-binary-tools.html).
- **6e PASS** — both standing constraints and candidate rungs L1–L10 are present in findings §9 and OT-86 (`docs/RESEARCH-2026-10-22-acpi-aml-corpus-FINDINGS.md:493-519`, `docs/OPEN-THREADS.md:96`). Findings and OT-86 also add an L11 oracle rung.

## 7. Host CI set

- **7a FAIL (Windows path portability)** — on the clean checked-out report, `cargo test -p acpi-aml-grade` failed at `committed_report_matches_the_harness`: regenerated panic locations used `acpi-6.1.1\src\aml\...` while committed files use `/` (`tools/acpi-aml-grade/tests/corpus.rs:138-153`; generated diff in `tools/acpi-aml-grade/report/coverage.md:26-45`). After regenerating the report, all 12 tests passed. `cargo run` then generated the same JSON on two runs, but modified both committed report files on Windows, so it did not leave the checkout clean. The documented `--config 'profile.dev.panic="unwind"'` spelling loses its TOML quotes in this PowerShell invocation; the equivalent `CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade` succeeded. I did not run the remaining fmt/workspace/clippy/SPDX checks in this review.
