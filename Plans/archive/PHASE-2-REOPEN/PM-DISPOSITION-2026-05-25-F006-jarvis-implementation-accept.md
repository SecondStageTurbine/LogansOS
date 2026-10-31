# PM-DISPOSITION: F-006 Jarvis implementation acceptance

- **Defect:** F-006, bare-metal HHDM coverage short of Limine module placement
- **Implementation commit:** `59443de` (`F-006: repair HHDM module coverage`)
- **Authority:** `Plans/PM-DISPOSITION-2026-05-25-F006-cerebro-diagnosis-close.md` (`d53c7c2`)
- **Dispatch:** `Plans/DISPATCH-2026-05-25-F006-jarvis-implementation.md` (`4a8cc5c`)
- **Author:** PM
- **Date:** 2026-05-25
- **Disposition:** ACCEPT FOR BARE-METAL RE-VERIFICATION

---

## 1. Disposition

PM accepts Jarvis's F-006 implementation at `59443de` for the next acceptance gate: OptiPlex 7080 bare-metal re-verification with at least 10 consecutive successful boots.

The implementation satisfies the operational design accepted in `d53c7c2`: derive required HHDM coverage from Limine executable/module responses, repair missing upper-half HHDM leaves inside `mm::init` using the local boot allocator, assert coverage before module bytes are dereferenced, and expose deterministic QEMU repair-path coverage via a forced-unmap hook.

Phase 2 is not formally closed by this disposition. F-006 close remains gated on the bare-metal re-verification evidence described in Section 6.

---

## 2. Implementation Review

Commit `59443de` changed:

- `kernel/src/main.rs`
  - Builds `BootHhdmRequirements` before `mm::init`.
  - Computes executable span from Limine executable physical base plus kernel image size.
  - Reads Limine module address/size metadata without constructing `module.data()` slices before HHDM repair.
  - Passes requirements into `mm::init(hhdm_offset, memmap, &boot_hhdm_requirements)`.

- `kernel/src/mm/hhdm_requirements.rs`
  - Adds `PhysExtent`, `BootModuleExtent`, `BootHhdmRequirements`, `HhdmBootReport`.
  - Adds `required_hhdm_phys_end(...)` helper and five source-level unit tests.

- `kernel/src/mm/mod.rs`
  - Computes `required_end = max(usable_end, executable.end, module.ends...)`.
  - Runs optional forced-unmap under `neo-test-hhdm-force-unmap`.
  - Repairs required spans using `repair_required_hhdm_spans(...)`.
  - Stores conservative `HHDM_PHYS_END = required_end`.
  - Emits `[hhdm] required_end=... exec=[...) modules=[...] repair_pages=N`.
  - Asserts first/last address coverage after repair.

- `kernel/src/test_harness.rs`, `kernel/Cargo.toml`, `tools/xtask/src/main.rs`
  - Adds `bare_metal_hhdm_covers_modules`.
  - Adds `bare_metal_hhdm_repair_path`.
  - Wires repair-path scenario to `neo-test-hhdm-force-unmap`.

---

## 3. Constraint Review

Accepted:

- **Actual Limine-owned spans:** yes. The implementation derives executable and module spans from Limine responses, not memmap type guesses.
- **No early `module.data()` dereference for requirement collection:** yes. The implementation reads the Limine file metadata prefix before HHDM repair.
- **Local boot allocator:** yes. HHDM repair receives the local `BuddyAllocator` inside `mm::init`; it does not use `FRAME_ALLOC`.
- **Pre-AP / pre-scheduler context:** yes. Repair happens before storage init, AP bring-up, scheduler init, and `spawn_neo_init`.
- **Upper-half HHDM only:** yes. Repair maps `hhdm_offset + phys` pages.
- **Unsafe comments:** yes. New unsafe blocks have adjacent SAFETY comments naming the invariant.
- **Forced-unmap hook default-off:** yes. The hook is gated by `neo-test-hhdm-force-unmap`, which is only enabled by the repair-path scenario feature.

Accepted caveat:

- The `[hhdm]` diagnostic labels modules as `module0`, `module1`, rather than Limine path names. This is weaker than the dispatch's ideal named-module diagnostic, but it still provides concrete per-module spans and repair count for the 10-boot acceptance loop. It is not a blocker for bare-metal re-verification.

---

## 4. PM Verification Performed

PM verified current repo state:

- `main` and `origin/main` point at `59443de`.
- Working tree is clean except pre-existing untracked `ai_digest.py`.
- `git diff --check HEAD~1..HEAD` passes.
- Limine `File` layout in local crate `limine 0.6.3` matches the prefix used by `limine_file_data_extent`: `revision`, `address`, `size`.

PM reran targeted gates:

```powershell
docker compose run --rm neo cargo k
docker compose run --rm neo cargo neo-test bare_metal_hhdm_covers_modules --no-meta
docker compose run --rm neo cargo neo-test bare_metal_hhdm_repair_path --no-meta
```

Results:

- `cargo k`: PASS, with existing warning noise.
- `bare_metal_hhdm_covers_modules`: PASS.
  - Serial evidence: `[hhdm] required_end=0x1fea5000 exec=[0x1ebd0000,0x1f10b000) modules=[module0=[0x1e879000,0x1ebbebe8),module1=[0x1e530000,0x1e874dc8)] repair_pages=0`
  - PASS line: `required_end=0x1fea5000 repair_pages=0 module_count=2`
- `bare_metal_hhdm_repair_path`: PASS.
  - Serial evidence: `[hhdm] forced_unmap_page ...`
  - Serial evidence: `[hhdm] required_end=0x1fea5000 exec=[0x1ebca000,0x1f106000) modules=[module0=[0x1e873000,0x1ebb8be8),module1=[0x1e52a000,0x1e86edc8)] repair_pages=1`
  - PASS line: `required_end=0x1fea5000 repair_pages=1 module_count=2`

PM did not independently count Jarvis's reported full-sweep success; Jarvis reported:

- helper unit test: 5 passed;
- `cargo k`: passed;
- both new `neo-test` scenarios passed;
- `phase2_perf_proxy_baseline`: passed;
- full `NEO_SMP=4 cargo neo-test all --no-meta`: passed.

Direct PM attempt to run the helper tests through the kernel package is not a valid local gate because `kernel/Cargo.toml` marks the binary `test = false`; forcing `--bin kernel` hits the expected no_std duplicate `panic_impl` conflict with `std`.

---

## 5. CI Status

GitHub CI for `59443de` is red:

- Run: `26422315191`
- Workflow: `CI`
- Job: `QEMU Integration Tests`
- Failure: `sched_ready_queue_capacity_pressure` failed after 4 attempts with QEMU debug-exit status 35.

PM classifies this as the same scheduler pressure-test flake Prometheus diagnosed for the earlier doc-only failure at `39a8048`, not as an F-006 implementation regression:

- The failure scenario is unrelated to HHDM repair.
- The same scenario failed on the documentation-only commit `39a8048`, where code was unchanged from a previously green commit.
- The two F-006-specific QEMU scenarios pass locally, including forced repair.

CI flake remediation remains a separate lane. It does not block starting PL's F-006 bare-metal re-verification, but it must be resolved or explicitly dispositioned before Phase 2 formal close is claimed.

---

## 6. PL Bare-Metal Re-Verification Dispatch

PL should run the F-006 OptiPlex 7080 re-verification now.

Build from repo root:

```powershell
cd "E:\AI Automation\AltimitNeo"
docker compose run --rm neo cargo k
docker compose run --rm neo cargo iso
Get-FileHash -Algorithm SHA256 .\boot\neoplutarch.iso
```

PM local ISO hash after targeted QEMU scenario builds:

```text
ADA27C458906E5998AA6A41A71C7B644DD057FD78243B6A959FC4C2CF2796E5C
```

If PL rebuilds before flashing, use PL's fresh hash in the evidence, not the PM local hash above.

Flash:

- Rufus DD image mode.
- GPT/UEFI.
- Secure Boot disabled.
- Serial capture at COM3 / 38400 / 8N1.

Run at least 10 consecutive OptiPlex boots from the same ISO.

Evidence filename pattern:

```text
Plans/evidence/phase2-bare-metal-optiplex-7080-<YYYYMMDD-HHMMSS>-12cpu-f006-reverify-boot<N>.serial.log
```

For each boot, verify:

- `[hhdm] required_end=0x... exec=[0x...,0x...) modules=[...] repair_pages=N` appears.
- `required_end`, exec span, module spans, and `repair_pages` have concrete values.
- Boot reaches post-`spawn_neo_init` / scheduler-idle territory.
- Zero `PAGE_FAULT`.
- Zero kernel panic.

Acceptance floor:

- 10 consecutive passing boots: F-006 close-eligible.
- Any PAGE_FAULT, panic, or missing `[hhdm]` line: F-006 remains open; capture failing log and return to PM/Jarvis with the failing boot number and serial evidence.

---

## 7. Tell-PL Text

PL - F-006 implementation is accepted for bare-metal re-verification at `59443de`. Build a fresh production ISO from repo root with `docker compose run --rm neo cargo k` then `docker compose run --rm neo cargo iso`, record SHA256, flash via Rufus DD-image mode, and run at least 10 consecutive OptiPlex 7080 boots with PuTTY COM3 / 38400 / 8N1 capture. Save each log as `Plans/evidence/phase2-bare-metal-optiplex-7080-<YYYYMMDD-HHMMSS>-12cpu-f006-reverify-boot<N>.serial.log`. Each boot must show the `[hhdm] required_end=... exec=... modules=... repair_pages=N` line, reach post-neo-init/scheduler-idle, and show zero PAGE_FAULTs. One success is not acceptance; 10 consecutive is the floor. GitHub CI is currently red on the known unrelated `sched_ready_queue_capacity_pressure` flake, so do not treat that as an F-006 blocker for hardware re-verification.

---

## 8. Final Call

ACCEPT `59443de` for F-006 bare-metal re-verification.

Do not close F-006 until PL returns 10 consecutive passing OptiPlex boot logs.
