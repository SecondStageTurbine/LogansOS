# CLOSE REPORT: F-006 bare-metal HHDM module coverage

- **Defect:** F-006, bare-metal kernel HHDM coverage short of Limine module placement
- **Date:** 2026-05-25
- **Author:** PM
- **Status:** CLOSED
- **Fix commit:** `59443de` (`F-006: repair HHDM module coverage`)
- **PM acceptance pre-reverify:** `43209d3` (`plans: accept F-006 Jarvis implementation for reverify`)
- **Current HEAD during evidence packaging:** `43209d359283aa01bbb7abab2db3dc7b02a4451d`
- **Test target:** Dell OptiPlex 7080, 12 logical CPUs, UEFI USB boot, Secure Boot disabled
- **ISO:** `boot/neoplutarch.iso`
- **ISO SHA256:** `83B21F8E614FD8A53E9427F1696F967AD768344801F586F1798404AB0B0D3D9E`
- **Serial:** COM3, 38400 baud, 8N1

---

## 1. Disposition

F-006 is closed.

The original failure was an intermittent bare-metal page fault while reading Limine module bytes through the HHDM. The accepted fix derives the required HHDM coverage from the executable and Limine module extents, repairs missing HHDM leaves before module bytes are dereferenced, and emits a boot-time `[hhdm]` report.

PL executed the required re-verification floor: 10 consecutive OptiPlex 7080 boots from the rebuilt production ISO. All 10 boots reached the framebuffer splash and the serial log's stable post-`neo-init` state. All 10 logs contain the `[hhdm]` coverage report, scheduler smoke pass, `neo-init exit code 0`, clean frame leak audit, and boot sequence rendered line. No boot shows `PAGE_FAULT`, panic, `FAIL`, `ERROR`, or assertion text.

This report closes F-006 only. It does not claim full Phase 2 bare-metal scenario-sweep completion, hardware perf completion, storage validation, Secure Boot support, or the separate GitHub Actions `sched_ready_queue_capacity_pressure` flake.

---

## 2. Evidence Set

Evidence directory:

```text
Plans/evidence/
```

Boot log files:

```text
Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-12cpu-f006-reverify-boot01.serial.log
Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-12cpu-f006-reverify-boot02.serial.log
Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-12cpu-f006-reverify-boot03.serial.log
Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-12cpu-f006-reverify-boot04.serial.log
Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-12cpu-f006-reverify-boot05.serial.log
Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-12cpu-f006-reverify-boot06.serial.log
Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-12cpu-f006-reverify-boot07.serial.log
Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-12cpu-f006-reverify-boot08.serial.log
Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-12cpu-f006-reverify-boot09.serial.log
Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-12cpu-f006-reverify-boot10.serial.log
```

PL also confirmed framebuffer splash visibility for each of the 10 boots during execution.

---

## 3. HHDM Coverage Evidence

All 10 boots emitted the same required coverage envelope:

```text
[hhdm] required_end=0x47d7ffbe8 exec=[0x66b5d000,0x670b2000) modules=[module0=[0x47d4ba000,0x47d7ffbe8),module1=[0x47d175000,0x47d4b9dc8)] repair_pages=N
```

Observed repair behavior:

| Boot | repair_pages | Result |
|---|---:|---|
| 01 | 0 | PASS |
| 02 | 838 | PASS |
| 03 | 0 | PASS |
| 04 | 0 | PASS |
| 05 | 838 | PASS |
| 06 | 0 | PASS |
| 07 | 838 | PASS |
| 08 | 838 | PASS |
| 09 | 0 | PASS |
| 10 | 838 | PASS |

Interpretation:

- 10/10 boots proved the kernel computed a required HHDM extent covering both Limine modules.
- 5/10 boots exercised the active repair path on real hardware (`repair_pages=838`).
- 5/10 boots exercised the already-covered path (`repair_pages=0`).
- Both paths completed through `neo-init`, final frame leak audit, and framebuffer boot rendering.

---

## 4. Serial Scan

PM scanned the 10 evidence logs for required pass markers:

```powershell
$logs = Get-ChildItem -Path "E:\AI Automation\AltimitNeo\Plans\evidence" `
  -Filter "phase2-bare-metal-optiplex-7080-20260525-12cpu-f006-reverify-boot*.serial.log" |
  Sort-Object Name
```

Summary:

| Check | Count |
|---|---:|
| Logs scanned | 10 |
| `[hhdm]` report present | 10 |
| `smoke test passed` present | 10 |
| `neo-init exit code 0` present | 10 |
| `leak audit clean` present | 10 |
| `boot sequence rendered` present | 10 |
| `PAGE_FAULT` hits | 0 |
| `panic` / `FAIL` / `ERROR` / `assert` hits | 0 |

Representative terminal summary:

```text
Summary: count=10 all_hhdm=10 all_sched=10 all_exit0=10 all_leak=10 all_rendered=10 page_faults=0 panic_fail_error=0
```

Boot 10 tail confirms the full success path:

```text
[ kernel ] sched: smoke test passed (5 ping, 5 pong, hog reaped, ticks=21)
[ kernel ] proc: neo-init exit code 0
[ kernel ] frame: leak audit clean (baseline=1039 final=1039)
[ boot  ] boot sequence rendered (1920x1080)
[ kernel ] phase 0 boot complete
```

---

## 5. Prior Non-Hardware Gates

PM previously accepted the F-006 implementation for this hardware reverify after local targeted checks:

```powershell
docker compose run --rm neo cargo k
docker compose run --rm neo cargo neo-test bare_metal_hhdm_covers_modules --no-meta
docker compose run --rm neo cargo neo-test bare_metal_hhdm_repair_path --no-meta
```

Accepted local results:

- `cargo k`: PASS, with existing warning noise.
- `bare_metal_hhdm_covers_modules`: PASS.
- `bare_metal_hhdm_repair_path`: PASS, including forced-unmap repair evidence.

Jarvis also reported:

- helper unit test: 5 passed;
- `cargo k`: passed;
- both new `neo-test` scenarios passed;
- `phase2_perf_proxy_baseline`: passed;
- full `NEO_SMP=4 cargo neo-test all --no-meta`: passed.

The GitHub Actions failure on `sched_ready_queue_capacity_pressure` remains classified as a separate scheduler pressure-test flake, not an F-006 regression.

---

## 6. Scope Boundary

Closed by this report:

- F-006 bare-metal HHDM coverage defect.
- F-006 10-consecutive-boot acceptance floor on OptiPlex 7080.
- Hardware evidence that both `repair_pages=0` and active HHDM repair complete cleanly.

Not closed by this report:

- Full Phase 2 bare-metal scenario sweep.
- Hardware perf captures.
- AHCI/NVMe storage validation.
- Secure Boot support for unsigned Limine media.
- CI flake remediation for `sched_ready_queue_capacity_pressure`.

---

## 7. Final Call

F-006 is CLOSED.

Recommended next step: resume the Phase 2 OptiPlex 7080 hardware scenario sweep from `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md` section 6.2 when PL is ready. The previous F-006 blocker no longer halts that sweep.

---

**End of close report.**
