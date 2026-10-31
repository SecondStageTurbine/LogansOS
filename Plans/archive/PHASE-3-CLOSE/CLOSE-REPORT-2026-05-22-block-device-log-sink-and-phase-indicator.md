# CLOSE-REPORT: Block-Device Log Sink + Framebuffer Phase Indicator (Jarvis Dispatch)

**Date:** 2026-05-22
**Recorder:** PM (filed on behalf of Codex per dispatch §9 — Codex shipped but could not file due to verification environment lacking Limine; PL ran the missing verification step locally)
**Closes:** `Plans/DISPATCH-2026-05-22-block-device-log-sink-and-phase-indicator-jarvis.md`
**Status:** CLOSED — substantially complete with C2/C4 deferred to first BUILD-5b execution

---

## 1. Summary

Codex implemented the dispatch's requested capabilities: the `BlockDeviceLogSink` kernel log destination with NEO_LOG_v1 versioned magic header and recursion-silencing, the 800×200 lower-right framebuffer phase indicator with 8×-scaled font, scenario harness integration via `--bare-metal-mode`, and the host-side `neo-extract-log` extraction tool. Codex's environment lacked Limine which blocked ISO packaging verification; PL ran the missing `docker compose` invocations locally and confirmed both bare-metal-mode ISOs build cleanly.

Five of seven acceptance criteria are fully discharged. C2 (framebuffer phase indicator runtime rendering) and C4 (bare-metal ISO runtime exercise against scenarios) remain runtime-unverified at the dispatch level and are deferred to first BUILD-5b execution, where they discharge as side effects of BUILD-5b's existing success criteria.

## 2. Files Delivered

Per Codex's report:

- `kernel/src/log/block_sink.rs` — `BlockDeviceLogSink` type, header framing, staging buffer, recursion guard, flush, degradation reporting.
- `kernel/src/scenario/phase_indicator.rs` — framebuffer phase indicator (800×200 lower-right, 8×-scaled font, color-coded phase/PASS/FAIL).
- `kernel/src/test_harness.rs` — bare-metal strict setup wired, phase transitions instrumented in AHCI/NVMe exemplar scenarios, PASS/FAIL flush hooks.
- `tools/xtask/src/main.rs` — `--bare-metal-mode` flag, disk pairing logic, manifest roles, log-sink feature selection.
- `tools/neo-extract-log/src/main.rs` — extraction tool with unit tests.
- `tools/neo-extract-log/README.md` — extraction documentation.

## 3. Verification Performed

### 3.1 In Codex's environment (compile + unit tests)

- `cargo test -p neo-extract-log` — PASS
- `cargo check -p xtask` — PASS
- `cargo check -Zjson-target-spec` — PASS
- Kernel checks for both bare-metal feature pairs — PASS
- Kernel checks for both non-bare-metal exemplar scenario features — PASS
- `git diff --check` — only LF/CRLF warnings reported

### 3.2 In PL's local environment (ISO packaging)

PL ran on 2026-05-22 from `E:\AI Automation\AltimitNeo`:

```
docker compose run --rm neo cargo neo-test --build-only phase3_ahci_neofs_mount_and_readwrite --bare-metal-mode
docker compose run --rm neo cargo neo-test --build-only phase3_nvme_neofs_mount_and_readwrite --bare-metal-mode
```

Both invocations produced the stable summary frame from dispatch §3:

```
NEO_VMWARE_SCENARIO/1 scenario=<scenario> mode=bare-metal iso=<path> manifest=<path>
NEO_VMWARE_DISK/1 scenario=<scenario> role=scenario qemu_index=<n> raw=<path> vmdk=<path> controller=<c>
NEO_VMWARE_DISK/1 scenario=<scenario> role=log-sink qemu_index=<n> raw=<path> vmdk=<path> controller=<c>
```

The scenario / log-sink disk pairing inverts correctly per scenario per the dispatch §4.3 design:
- `phase3_ahci_neofs_mount_and_readwrite`: scenario disk on SATA-AHCI (`device3`), log-sink on NVMe (`device4`).
- `phase3_nvme_neofs_mount_and_readwrite`: log-sink on SATA-AHCI (`device3`), scenario disk on NVMe (`device4`).

### 3.3 Artifacts on disk (2026-05-22 ~23:00 EDT)

**AHCI scenario bundle** at `target\neo-test\vmware\phase3_ahci_neofs_mount_and_readwrite\`:

| File | Size | Role |
|------|------|------|
| `phase3_ahci_neofs_mount_and_readwrite.bare-metal.iso` | 24,690,688 B (24.7 MB) | bootable bare-metal-mode kernel |
| `device3.vmdk` | 262,144 B | VMDK pointing at scenario raw on SATA |
| `device4.vmdk` | 131,072 B | VMDK pointing at log-sink raw on NVMe |
| `device0..device2.vmdk` | 131,072 B each | qemu-filler (omitted in VMware/bare-metal) |
| `manifest.txt` | 3,079 B | structured manifest including `log_sink_offset=0x10000000`, `log_sink_size=16777216`, `vmware_minimum_attachment` hint, `runtime_source` description |

Companion raw disks at `target\neo-test\`:

| File | Size | Role |
|------|------|------|
| `phase3_ahci_neofs_mount_and_readwrite.ahci.raw` | 134,217,728 B (128 MiB) | NeoFS scenario image; provisioned onto Samsung 860 EVO via `dd` per BUILD-5b procedure |
| `phase3_ahci_neofs_mount_and_readwrite.nvme-log.raw` | 301,989,888 B (288 MiB) | Log sink target; size accommodates 256 MiB offset + 16 MiB log region with headroom; provisioned onto Intel E6100p NVMe via `dd` |

**NVMe scenario bundle** at `target\neo-test\vmware\phase3_nvme_neofs_mount_and_readwrite\`:

| File | Size | Role |
|------|------|------|
| `phase3_nvme_neofs_mount_and_readwrite.bare-metal.iso` | 24,694,784 B (24.7 MB) | bootable bare-metal-mode kernel |
| `device3.vmdk` | 131,072 B | VMDK for log-sink on SATA |
| `device4.vmdk` | 262,144 B | VMDK for scenario disk on NVMe |
| `device0..device2.vmdk` | 131,072 B each | qemu-filler |
| `manifest.txt` | 3,079 B | structured manifest |

Companion raw disks:

| File | Size | Role |
|------|------|------|
| `phase3_nvme_neofs_mount_and_readwrite.nvme.raw` | 134,217,728 B (128 MiB) | NeoFS scenario image; provisioned onto Intel E6100p NVMe via `dd` |
| `phase3_nvme_neofs_mount_and_readwrite.ahci-log.raw` | 301,989,888 B (288 MiB) | Log sink target; provisioned onto Samsung 860 EVO via `dd` |

## 4. Acceptance Criteria Status

| C | Item | Status | Evidence |
|---|------|--------|----------|
| **C1** | Block-device log sink module compiles, passes unit tests, wired into kernel log infrastructure | ✅ FULL | `cargo check` on both bare-metal feature pairs; `neo-extract-log` round-trip tests pass header framing |
| **C2** | Framebuffer phase indicator module compiles, renders a phase on a running kernel, observable in QEMU and VMware | ⚠️ COMPILE-ONLY | Module compiles; runtime rendering deferred — see §5 |
| **C3** | Scenario harness produces bare-metal-mode ISOs for both exemplar scenarios | ✅ FULL | Both ISOs present on disk per §3.3; `NEO_VMWARE_SCENARIO/1 mode=bare-metal` summary frame emitted per §3.2 |
| **C4** | Both exemplar scenarios run cleanly under VMware with the block-device sink enabled; extraction tool retrieves PASS log | ⚠️ COMPILE-ONLY | Scenarios compile with bare-metal feature; full runtime exercise deferred — see §5 |
| **C5** | `tools/neo-extract-log` builds and extracts a valid log region; refuses on invalid magic | ✅ FULL | `cargo test -p neo-extract-log` passes; unit tests cover both valid and invalid magic paths |
| **C6** | Documentation lands per dispatch §4.6 (excluding procedure update which PM owns) | ✅ FULL | `tools/neo-extract-log/README.md` present; manifest format documents `log_sink_offset`, `log_sink_size`, `vmware_minimum_attachment`, `runtime_source` |
| **C7** | No regressions in existing VMware scenario harness behavior when bare-metal-mode is not selected | ✅ FULL | `cargo check` on non-bare-metal exemplar features passes; no harness changes outside the new `--bare-metal-mode` code path |

**Summary:** 5/7 fully discharged. 2/7 (C2, C4) compile-verified, runtime-deferred.

## 5. Deferred Runtime Verification

C2 and C4 require booting a bare-metal-mode ISO with the scenario+log-sink disks attached and observing both the framebuffer phase indicator behavior and the post-run extraction of a valid log with PASS sentinel. PM accepts deferral of this verification to first BUILD-5b execution rather than running a redundant VMware verification session, because:

- The OptiPlex 7080 Micro bare-metal target is the actual verification surface BUILD-5b is designed for.
- A VMware-only verification session produces no information that BUILD-5b's own execution would not also produce.
- BUILD-5b's existing success criteria S1–S5 in `PROCEDURE-2026-05-22-phase-3-build-5b-bare-metal-optiplex.md` naturally exercise C2 and C4: S1 (serial capture working) maps onto block-device log sink working; S2/S3 (PASS sentinel in log) maps onto extraction tool retrieving a valid log with PASS sentinel; the procedure's evidence capture inherently observes the framebuffer phase indicator throughout.

Procedure update absorbing this discharge linkage lands as a separate artifact per dispatch §4.6 (PM-owned).

## 6. Hand-Off State

**To BUILD-5b execution:** the kernel and tooling are ready. PL holds a hardware session with:

- The OptiPlex 7080 Micro (boot path proven 2026-05-22 boot smoke, Service Tag D4SHKD3).
- Samsung 860 EVO 250 GB SATA SSD on the 2.5" bay (AHCI scenarios target).
- Intel E6100p Series 256 GB NVMe on M.2 SSD-3 (NVMe scenarios target).
- BIOS at 1.37.0, SATA mode AHCI, no other OS on disks (wipe acceptable).
- Two bare-metal-mode ISOs produced 2026-05-22 23:00 EDT.
- Two scenario raw disks + two log-sink raw disks ready for `dd` provisioning.
- A second USB stick with a Linux live ISO for the `dd` provisioning step (PL to procure if not already on hand).

The `neo-extract-log` tool extracts logs from the log-sink device post-run. Documentation at `tools/neo-extract-log/README.md`.

## 7. Forward-Marker Notes

The `BlockDeviceLogSink` and framebuffer phase indicator capabilities have applicability beyond Phase 3 BUILD-5b:

- **Phase 4 network stack BUILDs** may want the same log-sink infrastructure for bare-metal network-driver verification (forward-marker per `Plans/PHASE-4-THINK-2026-05-22-network-stack-design-direction.md` §3.5 `NetDevice` trait surface). No retrofit needed; the sink is reusable as-is.
- **Phase 5+ AI subsystem** runs on bare metal will likely want the framebuffer phase indicator pattern (kernel observability without serial) extended to AI-scenario-specific phases. Pattern is reusable; phase enum is extensible.
- **General kernel observability** (ADR-013 deferred) — the log-sink primitive is one of several capabilities a future ADR-013 may rest on.

These are notes, not commitments. ADR-013 remains deferred per its existing forward-marker.

## 8. Closure Boundary

This close-report discharges the dispatch's code-and-tooling deliverables. It does NOT:

- Close BUILD-5b. BUILD-5b runs against the OptiPlex when PL schedules it.
- Close Phase 3. Phase 3 closes via PM-DISPOSITION after BUILD-5b evidence lands.
- Open Phase 4 PLAN. Per `PHASE-4-THINK-2026-05-22-network-stack-design-direction.md` §6 P1, PLAN gates on Phase 3 closure.
- Discharge the procedure update obligation per dispatch §4.6. PM updates the procedure separately.

---

**End of close-report.**

*Codex's dispatch is substantially complete. C2 and C4 deferred to BUILD-5b. PM-DISPOSITION follows.*
