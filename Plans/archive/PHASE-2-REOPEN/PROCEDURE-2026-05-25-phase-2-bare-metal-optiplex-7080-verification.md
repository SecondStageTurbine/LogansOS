# PROCEDURE — Phase 2 bare-metal verification on Dell OptiPlex 7080

**Date filed:** 2026-05-25
**Author:** PM
**Type:** Manual procedure document for PL direct execution. Companion to `Plans/DISPATCH-2026-05-25-phase-2-hardware-optiplex-7080-verification.md` (the *what*); this document is the *how*. PL has bare-metal hardware availability and serial console infrastructure (verified 2026-05-25, see §3).
**Scope:** Real-hardware verification of Phase 2's complete software remainder (subphases 1-4 closed) on a Dell OptiPlex 7080 — the open Bible Article 13 bare-metal mandate gate. Sole acceptable substitute for the "≥1 bare-metal target" requirement. Discharges THINK v0.2 Q6.
**Status:** Procedure ready for execution. Serial chain infrastructure verified PASS 2026-05-25 18:36 UTC (see §3.4). Estimated 4-8 hours wall-clock for full scenario sweep + perf captures + report.

---

## 1. Why bare-metal verification

QEMU TCG (current per-commit gate) is software emulation; VMware (the 2026-05-11 hypervisor-VM gate) is hardware virtualization with VT-x. Both are valuable but neither is bare-metal:

- **Real LAPIC IPI delivery.** Bare-metal cross-CPU rendezvous (the SC-3B 200/200 surface) exercises the actual IPI hardware path — different latency profile, different ordering observables vs hypervisor mediation.
- **Real cache coherency.** SMP correctness under the real CPU's MESI/MOESI protocol, not the hypervisor's serialized model.
- **Real TSC and APIC timer.** Timing-sensitive race surfaces (Q7.c shootdown timeouts, Q8.b grant-vs-exit CAS) observe physical clock variance the emulators flatten.
- **AP bring-up.** The real AP startup sequence (SIPI, trampoline, GS-base sentinel) runs against actual silicon, surfacing any assumption the hypervisor masks.

Bible Article 13 names "bare-metal multi-CPU verification" as a Phase 2 close-gate. This procedure executes it.

## 2. Hardware and prerequisites

### 2.1 Target hardware

- **Dell OptiPlex 7080 SFF** (model 0RR02N or compatible). The 7080 has been the project's bare-metal target since the 2026-05-22 boot smoke.
- **CPU:** Intel i5-10500T (Comet Lake), 6 physical cores / 12 logical CPUs (verified 2026-05-25 via Ubuntu dmesg — see §3.4 evidence). Satisfies Q6(b) "≥4 cores + full count."
- **Storage:** Samsung SSD 860 EVO 250GB (SATA, AHCI mode) + Intel SSDPEKKF256G8L NVMe 256GB (PCIe). Both visible in BIOS.
- **Firmware:** Dell BIOS 1.37.0 (or later) in UEFI mode. Secure Boot must be DISABLED for the unsigned Limine path.

### 2.2 Serial console infrastructure

- **6CC5P Dell internal serial header cable** (the small adapter cable). The 10-pin connector plugs into the OptiPlex motherboard's internal serial header; the DB9-male emerges via a PCI-slot bracket.
- **Null modem DB9 F-F adapter** (e.g. Arnorin or equivalent). Crosses TX/RX so two DTEs can talk.
- **FTDI USB-RS232 adapter** (e.g. DTECH). DB9 to USB.
- **Receiving Windows machine** with PuTTY installed and the FTDI driver loaded.

Verified physical chain (PL setup 2026-05-25): 6CC5P internal cable → OptiPlex back-panel DB9 → null modem → FTDI USB-RS232 → Windows USB. FTDI enumerates as COM3.

### 2.3 Build/development host

- The NeoPlutarch source tree at `E:\AI Automation\AltimitNeo\` on the Windows machine.
- Docker Desktop (Linux containers) for the `docker compose run --rm neo ...` build path.
- A USB stick (≥4 GB) for ISO write. Rufus or `dd` equivalent (Win32DiskImager) to write the ISO.

## 3. Serial chain setup and verification

### 3.1 Physical assembly (one-time)

1. Power off the OptiPlex; open the case.
2. Locate the internal serial header on the motherboard (silkscreen "SERIAL2" / "SER1" / "KB_MS_SERIAL", typically a 10-pin keyed header near the rear I/O cluster).
3. Plug the 6CC5P's small end onto the header. Confirm firm seating — these are tight; takes a confident press.
4. Mount the 6CC5P bracket (DB9-male emerging) into an empty PCI slot at the chassis rear; screw the bracket down so the DB9 doesn't flex when connectors are attached.
5. Close the case.
6. On the chassis exterior: 6CC5P DB9 → null modem F-F → FTDI USB-RS232's DB9. Plug the FTDI's USB into the Windows machine.

### 3.2 BIOS configuration (one-time, plus verification per session)

Power on the OptiPlex, tap **F2** to enter BIOS setup.

1. Navigate to **System Configuration → Serial Port**.
2. Set **Serial Port** to **COM1** (this is the *OptiPlex's* COM1 — fixed I/O 0x3F8 / IRQ 4, the universal x86 default). Not COM3 — that's the FTDI's Windows enumeration, unrelated.
3. Apply and exit (Save Changes).

Note: Dell OptiPlex 7080 consumer BIOS does NOT expose a "Console Redirection" or "BIOS Serial Redirection" option (removed from this product line). The serial port hardware is enabled; the BIOS just won't echo POST messages to it. Kernel-side serial (Limine + NeoPlutarch) drives the port directly, which is what matters.

### 3.3 Receiving terminal setup (per session)

On the Windows machine, before powering on the OptiPlex for any verification run:

1. Open Device Manager → Ports (COM & LPT). Confirm "USB Serial Port (COM3)" is present (FTDI). Note the actual COM number if different.
2. Launch PuTTY.
3. Connection type: **Serial**.
4. Serial line: **COM3** (or the actual FTDI port).
5. Speed: **38400** baud, 8 data bits, no parity, 1 stop bit (`8N1`). NeoPlutarch's current `uart_16550` initialization emits readable kernel logs at 38400 on this chain; if a future build changes this, update both ends.
6. Session → Logging:
   - Session logging: **All session output**.
   - Log file name: `Plans/evidence/phase2-bare-metal-optiplex-7080-<YYYYMMDD-HHMMSS>-<cpu-count>-<scenario>.serial.log` (paths per dispatch §7). On the Windows side place the file under `E:\AI Automation\AltimitNeo\Plans\evidence\…` so the project tree gathers it for the report and git commit.
   - On existing file: **Always append**.
7. **Open** the connection. The terminal will be blank until the OptiPlex boots.

### 3.4 Chain verification reference run (already executed)

A reference Ubuntu live-disc dmesg dump confirmed the chain end-to-end on 2026-05-25 at 18:36 UTC. Evidence in `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-183600-12-serial-chain-baseline.serial.log` (Ubuntu 7.0.0-14-generic, Intel i5-10500T 12-logical-CPUs, Samsung 860 EVO + Intel NVMe both enumerated, ttyS0 at 0x3F8 IRQ 4 base_baud 115200 16550A confirmed, ~500 lines clean text, zero garbling).

For every subsequent session, capture a fresh Ubuntu (or production NeoPlutarch boot smoke) serial dump as the in-session baseline before scenario execution. Garbled output here means the cable, baud rate, or driver state is wrong — fix before proceeding to scenarios.

## 4. Build sequence

Per `DISPATCH-2026-05-25-phase-2-hardware-optiplex-7080-verification.md` §4. Execute on the Windows host with Docker running.

### 4.1 Pre-build state capture

```powershell
cd 'E:\AI Automation\AltimitNeo'
git status --short --branch
git rev-parse HEAD
git log --oneline -5
```

Record the exact HEAD commit. The hardware verification report names this commit as the source of truth for the run. Required state per dispatch §3: on `main`, at or after `39a8048`, no uncommitted source changes (evidence files OK).

### 4.2 Production ISO build

```powershell
docker compose run --rm neo cargo k
docker compose run --rm neo cargo iso
docker compose run --rm neo xorriso -indev /workspace/boot/neoplutarch.iso -report_el_torito plain
```

Record:
- Build result (success/fail; any warnings)
- ISO path (typically `boot/neoplutarch.iso` in repo root)
- ISO SHA-256 hash: `(Get-FileHash boot\neoplutarch.iso -Algorithm SHA256).Hash`
- El Torito report output (BIOS + UEFI catalog entries present)

### 4.3 Per-scenario ISO builds

The dispatch §6 scenario list has ~40 scenarios. Each runs from its own scenario ISO produced by the test harness:

```powershell
docker compose run --rm neo cargo neo-test --build-only <scenario>
```

For each scenario the harness writes `target/neo-test/vmware/<scenario>/<scenario>.iso`. Record SHA-256 per scenario.

Do NOT use `--bare-metal-mode` for Phase 2 scenarios — that mode is currently reserved for Phase 3 AHCI/NVMe storage exemplars. Phase 2 scenarios run from the standard VMware-target ISO; the only difference between VMware execution and bare-metal execution is the physical boot target, not the build flag.

Build all scenarios up front for the session, or build-then-execute per scenario at PL's preference. Up-front-build minimizes context switches.

### 4.4 USB media write

For the production ISO:
1. Insert a USB stick (≥4 GB, will be overwritten).
2. In Rufus: Device = USB stick, Boot selection = `boot\neoplutarch.iso`, Partition scheme = GPT, Target system = UEFI (non CSM), File system = FAT32. Click START → "Write in DD Image mode" → OK.
3. After write, verify the USB shows `EFI/BOOT/BOOTX64.EFI` (Windows Explorer or `dir <USB-drive>:\EFI\BOOT\`).

For per-scenario ISOs: same procedure, but the scenario name in the filename helps avoid mix-ups. Label each stick or rewrite the same stick per-scenario, recording which ISO is on the stick before each boot.

## 5. BIOS pre-boot configuration (per-session)

Power on the OptiPlex; tap **F2** to enter BIOS.

1. **Secure Boot:** confirm DISABLED. The unsigned Limine path will fail at Secure Boot otherwise. If you cannot disable Secure Boot for any reason, stop and report — do NOT treat a Secure Boot rejection as a kernel failure (dispatch §4 explicit).
2. **System Configuration → Serial Port:** confirm COM1 (per §3.2).
3. **Boot Sequence:** ensure UEFI USB boot is enabled and prioritized appropriately. F12 at boot also gets the one-time boot menu if you prefer.
4. Apply, exit, power off.

Insert the USB stick (production ISO for the first run) before powering on for verification.

## 6. Execution sequence

Execute in the dispatch §5 order. For each step, the PuTTY terminal must be already connected and logging to the per-scenario evidence file BEFORE power-on.

### 6.1 Production ISO boot smoke

1. PuTTY logging to `…-<HHMMSS>-12-production-smoke.serial.log` (12 = full CPU count, expected for production smoke).
2. Power on OptiPlex; tap F12 if needed to select USB.
3. Capture full serial output from POST through to NeoPlutarch's idle/shell sentinel.
4. PASS conditions: kernel reaches its post-boot stable state; no `KERNEL PANIC`; no `BUG:`; AP bring-up completes for all logical CPUs (look for per-AP GS-base sentinel lines); frame-audit-clean at end-of-boot (the frame audit prints to serial — should show 0 leaked / 0 dirty).
5. Power off cleanly when sentinels confirmed.

### 6.2 Targeted scenario pass at ≥4 logical CPUs

Per dispatch §6 the full scenario set runs at "≥4 logical CPUs." On a 12-logical-CPU 7080, "≥4" is satisfied by booting the scenarios with the kernel's CPU-count limit at 4 (the scenarios should accept this via kernel cmdline or scenario-ISO build configuration — confirm in the test harness output).

For each scenario in dispatch §6's list:

1. Write the scenario ISO to USB (or insert a pre-written stick).
2. PuTTY logging to `…-<HHMMSS>-4-<scenario>.serial.log`.
3. Power on, boot.
4. Capture serial output until the scenario emits `event=PASS` or `event=FAIL` sentinel (or until panic/hang, see §8).
5. Power off.
6. Move on to next scenario.

For automation: PL may write a per-session script that rewrites the USB and prompts for power-cycle between scenarios. Out-of-scope for this document; the procedure assumes manual cadence.

### 6.3 Targeted scenario pass at full logical CPU count

Identical to §6.2 but with kernel CPU limit unrestricted (12 logical CPUs on the 7080). Per-scenario evidence files: `…-<HHMMSS>-12-<scenario>.serial.log`.

Order: PL preference. Recommend running the most SMP-stress-sensitive scenarios first (the `q7c_*` shootdown set, `smp_cross_cpu_*`, `cap_revoke_cross_process`, `q8a/b/c_*`) so any AP-bringup or coherency issue surfaces early.

### 6.4 Full non-ignored test-harness sweep (optional, time-permitting)

Per dispatch §5 item 4. If the hardware window permits, boot a full-suite scenario ISO that runs all non-ignored tests in sequence. Same evidence-file conventions. May take 30-90 minutes depending on harness configuration.

### 6.5 Hardware perf captures

Per dispatch §5 item 5. Five measurements; each typically a single scenario boot:

1. **Boot time:** measured by timestamp difference between the kernel's first serial output and the post-boot stable sentinel. Capture from the production smoke run (§6.1) — no separate boot required.
2. **Syscall latency smoke:** the harness's syscall-latency micro-bench scenario. PASS = numeric output present and within sanity range (record raw lines; the Q4 perf harness will gate against them later).
3. **Context-switch latency or count-derived scheduler timing:** the corresponding harness scenario (likely `phase2_perf_proxy_baseline` per dispatch §6). Record raw lines.
4. **Scheduling fairness:** the harness's fairness scenario, if present. If not, record the absence in the report.
5. **`irqsafe_mutex_bench`:** the ADR-019 50ns native bench scenario (now folded into the Q4 perf harness per THINK v0.2 Q4(d)). Record raw nanosecond numbers.

## 7. Evidence capture conventions

Per dispatch §7. Every run must preserve:

- Full serial log at the canonical path `Plans/evidence/phase2-bare-metal-optiplex-7080-<YYYYMMDD-HHMMSS>-<cpu-count>-<scenario-or-sweep>.serial.log`.
- Firmware mode (UEFI), Secure Boot state (disabled), BIOS version (capture from `dmidecode` if Ubuntu-cross-checked, or from the Dell BIOS splash).
- CPU count used for the run (4 or 12, or per-scenario harness configuration).
- Source commit (exact SHA) — record in the report, not per-log.
- ISO hash (per-scenario or production).
- PASS/FAIL table (one row per scenario per CPU-count).
- Raw perf-line excerpts from the serial logs.
- Frame-audit result (frame leak count, dirty frame count) — extract from the end-of-run frame-audit serial output.

Commit all evidence files to `Plans/evidence/…` at the end of the session. Per `a434992`'s CI path filter, evidence commits don't run CI.

## 8. Stop conditions and triage

Per dispatch §8. STOP and file blocker evidence immediately on:

| Event | Action |
|---|---|
| Unexpected `KERNEL PANIC` | Stop run. Capture full serial. File `Plans/evidence/…-BLOCKER-<scenario>.serial.log` + a brief NOTES doc naming the panic site. PM triages. |
| Unexpected `BUG:` | Same as panic. |
| Frame leak / dirty frame audit | Same. Frame-audit lines at end of run should show 0/0; any non-zero is a leak. |
| GS-base sentinel mismatch | Same. The per-AP `gsbase_check pid=N expected=X actual=Y` lines should all show `expected == actual`. |
| AP bring-up failure | Same. Look for `smpboot: CPU N failed to bring up` or missing per-AP sentinel lines. |
| Boot hang after serial progress (no output for >60s when kernel was still printing) | Same. Capture, power off, file evidence. |
| Media not visible as UEFI boot target | Diagnose USB layout — should expose `EFI/BOOT/BOOTX64.EFI`. If layout is correct but UEFI doesn't see it, re-write the USB with Rufus DD-image mode. If still failing, stop and report. |
| Scenario `event=FAIL` in a canonical final log | Stop the per-CPU-count batch this scenario belongs to. Other CPU-count batches may continue if PL chooses. File evidence. |

**Expected-panic exception:** `q7c_shootdown_timeout_panic` is expected to emit `KERNEL PANIC` because its purpose is to verify shootdown-timeout panic discipline. PASS for this scenario requires BOTH the panic AND an `event=PASS expected_panic=tlb_shootdown_timeout` sentinel in the same serial log. Treat the panic as evidence-of-PASS only when paired with the explicit expected_panic sentinel.

### 8.1 If hardware access is lost mid-session

Report `hardware_unavailable` in the session log. Do NOT improvise with QEMU or VMware to "finish" the run; the dispatch explicitly forbids substitution. Pick up at the next available hardware window.

## 9. Report generation

After execution, file `Plans/PHASE-2-BARE-METAL-OPTIPLEX-7080-VERIFICATION-<YYYY-MM-DD>.md` per dispatch §9. Sections required:

1. **Frontmatter:** date executed, executor (PL), source commit, ISO hash(es), procedure reference (this doc), dispatch reference.
2. **Hardware and firmware:** OptiPlex model, CPU model + logical count, storage devices enumerated, BIOS version, UEFI mode, Secure Boot state.
3. **Media layout evidence:** USB partition layout, presence of `EFI/BOOT/BOOTX64.EFI`, El Torito report.
4. **Serial log paths:** the directory of evidence files plus a manifest table.
5. **Targeted scenario table for ≥4 logical CPUs:** scenario × PASS/FAIL × evidence-file pointer. All scenarios from dispatch §6.
6. **Targeted scenario table for full logical CPU count:** same shape, full count.
7. **Full sweep verdict** (if run).
8. **Hardware perf captures:** the five measurements from §6.5 with raw values.
9. **Serial-log scan output:** automated grep for `KERNEL PANIC`, `BUG:`, `event=FAIL`, frame-audit non-zero, GS-base mismatch. Report any hits.
10. **Expected-panic exception handling:** explicit treatment of `q7c_shootdown_timeout_panic`.
11. **Conclusion:** one of `PASS` / `PASS-WITH-INCIDENTAL` / `FAIL`. `PASS-WITH-INCIDENTAL` is permissible for non-blocking incidents (e.g. one scenario flaked once, passed on retry) — describe each incidental.

## 10. Cleanup

After the session:

1. Power off OptiPlex.
2. PuTTY: close session (the log file is auto-saved per the logging config).
3. Move evidence files into `Plans/evidence/…` if they're not already there.
4. `cd 'E:\AI Automation\AltimitNeo' ; git add Plans/evidence Plans/PHASE-2-BARE-METAL-OPTIPLEX-7080-VERIFICATION-*.md ; git commit -m "evidence: Phase 2 bare-metal OptiPlex 7080 verification <YYYY-MM-DD>" ; git push`.
5. If the conclusion is PASS or PASS-WITH-INCIDENTAL, the open Bible Article 13 close-gate discharges and Phase 2 formal close becomes filable. PM closes the gate via PM-DISPOSITION accepting the verification report.

## Appendix A — Verified hardware inventory (2026-05-25)

Captured from Ubuntu 7.0.0-14-generic live disc dmesg, reference run at 18:36 UTC. Source: `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-183600-12-serial-chain-baseline.serial.log`.

| Item | Value |
|---|---|
| Chassis | Dell OptiPlex 7080/0RR02N |
| Firmware | American Megatrends UEFI 2.7; Dell BIOS 1.37.0 (2025-12-01) |
| SMBIOS | 3.2.0 |
| CPU | Intel Core i5-10500T @ 2.30GHz (family 0x6, model 0xa5, stepping 3) |
| Topology | 1 socket / 6 cores / 12 logical CPUs (HT enabled) |
| Memory | 2/2 DIMM slots populated; 16 GB total (per dmesg "Memory: 13957684K/16484524K available") |
| Serial | ttyS0 at I/O 0x3F8, IRQ 4, base_baud 115200, 16550A UART |
| Storage (SATA) | Samsung SSD 860 EVO 250GB, firmware RVT04B6Q, max UDMA/133, NCQ depth 32, AHCI mode |
| Storage (NVMe) | Intel SSDPEKKF256G8L at PCI 0000:01:00.0, class 0x010802, 12/0/0 default/read/poll queues |
| Network | Intel I219-LM (e1000e) eth0 at 0000:00:1f.6 |
| Wireless | Intel Wi-Fi 6 AX201 160MHz (iwlwifi) at 0000:00:14.3 |
| Bluetooth | Intel ibt-19-0-4 |
| Audio | Intel HDA + ALC3246 codec |
| Power | Intel HWP enabled; P-state driver active |
| Virt | VT-x available; DMAR (IOMMU) active in x2apic mode |
| Mitigations | eIBRS, IBPB conditional, IBRS, BHI SW BHB clearing — all auto-enabled |

The 12-logical-CPU count satisfies Q6(b) "≥4 logical CPUs minimum plus the target's full count" trivially.

## References

- `Plans/DISPATCH-2026-05-25-phase-2-hardware-optiplex-7080-verification.md` — the dispatch this procedure executes.
- `Plans/PM-DISPOSITION-2026-05-25-phase-2-build-subphase-4-close.md` — the close gate this verification discharges.
- `Plans/PROCEDURE-2026-05-11-vmware-hypervisor-vm-verification.md` — predecessor (hypervisor-VM, not bare-metal). Shape reference.
- `Plans/PHASE-2-THINK-v0.2-2026-05-24.md` Q6 — the ratified protocol questions this document answers operationally.
- `Plans/PL-RATIFICATION-2026-05-24-phase-2-think-v0.2.md` — ratifications including Q6(d) OptiPlex required + AMD stretch, Q6(e) PL executes written procedure.
- Bible Article 13 — the bare-metal mandate this discharges.

---

**End of procedure.**
