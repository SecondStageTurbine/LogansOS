# PM-DISPOSITION — Phase 3 BUILD-5b — Jarvis APIC-base init fix ACCEPTED

**Date:** 2026-05-28
**Closes:** the AHCI-lane APIC panic that blocked BUILD-5b on first attempt (kernel panic at `kernel/src/arch/x86_64/apic.rs:125` with `Captured bytes: 0`)
**Closed by:** Jarvis commit `72a0a41c8d6ef6d8bb6e7981dbc928c8cface40c` — `kernel: initialize bsp apic before storage probe`
**Verified on:** bare-metal Dell OptiPlex 7080 Micro (Service Tag D4SHKD3)

## Audit-trail gap acknowledgment

The previous PM session handed off the BUILD-5b investigation claiming the following artifacts had been filed:

- `Plans/BLOCKER-2026-05-28-phase3-build5b-ahci-apic-base-not-initialized.md`
- `Plans/HANDOFF-2026-05-28-phase3-build5b-ahci-apic-panic-to-jarvis.md`
- `Plans/evidence/phase3-build5b-optiplex-7080-20260528-ahci-run-fail-notes.txt`
- `Plans/evidence/phase3-build5b-optiplex-7080-20260528-ahci-run-log-fail-captured-bytes-0.txt`

**None of these files existed on disk** when the current PM took over (verified via repo-wide glob, untracked-file check, and orphan-commit scan across all branches). The prior PM either crashed before writing them or wrote them outside the repo. The data was preserved in two other places: (a) the human handoff message describing the failure, and (b) two files on Stick C (`ahci-run-fail-notes.txt` and `ahci-run-log-fail-captured-bytes-0.txt`) which have now been copied into `Plans/evidence/` (dated 2026-05-27 to reflect when they were created, not when filed). The other two claimed artifacts (BLOCKER and HANDOFF for the APIC panic) are not being reconstructed retroactively — the current PM did not author them and will not invent prior-PM intent. Instead, this PM-DISPOSITION subsumes their function: it documents the failure, the fix, and the close.

The lesson is for the takeover checklist: a new PM should re-verify on-disk artifacts before trusting handoff claims. The current PM did exactly this and surfaced the gap on first inspection.

## The blocker (what was wrong)

On the first BUILD-5b attempt at OptiPlex 7080 (2026-05-27 ~22:00 EDT, prior PM session), the AHCI-lane scenario kernel boot panicked:

```
panicked at kernel/src/arch/x86_64/apic.rs:125:5:
assertion `left != right` failed: apic: local APIC base not initialized
left: 0
right: 0
```

Extractor against the NVMe log sink afterward reported `Captured bytes: 0`, consistent with the kernel panicking before any scenario log entries could be written through the sink.

Provisioning had been verified correct via stable `/dev/disk/by-id` paths and byte-for-byte cmp against staged raws before boot — **not** a wrong-disk issue. The procedure correction "preboot log-sink raw is expected blank in the runtime log region; preboot proof is cmp, not NEO_LOG_v1 magic" was made during the prior session and remains in force.

PM diagnosis at the time: early APIC MMIO access before `smp::bring_up_aps()` initialized `APIC_BASE_PHYS` via `apic::init_bsp`. The panic occurred after AHCI/NVMe block devices registered (so MSI/MSI-X delivery was already plausible) and after framebuffer init, but before any nonzero log payload — pinpointing the EOI assertion in the early IRQ path.

## Jarvis's fix (what was right)

Commit `72a0a41c8d6ef6d8bb6e7981dbc928c8cface40c` (pushed to `origin/main` 2026-05-27 22:34 EDT):

```
kernel: initialize bsp apic before storage probe
```

Two files touched:

- `kernel/src/arch/x86_64/smp.rs` (+34): introduces `pub fn init_bsp_apic()` backed by atomic-guarded `init_bsp_apic_once()` (`AtomicBool` + `compare_exchange(false, true, AcqRel, Acquire)` for one-shot semantics); refactors `bring_up_aps()` to route through the same idempotent helper so the AP path is not broken
- `kernel/src/main.rs` (+11/−3): calls `arch::x86_64::smp::init_bsp_apic()` immediately after `mm::init(...)` and **before** `drivers::block::init()`, with an explanatory comment block

The fix has the exact shape the root cause required: BSP APIC ready before any storage interrupt source becomes live, with the existing AP startup ordering preserved.

## PM acceptance — 10-gate ledger (each gate enumerated literally, per the team's audit discipline)

| # | Gate | Status | Evidence |
|---|------|--------|----------|
| 1 | Root cause named in Jarvis's report | ✅ | "storage probing ran after interrupts were enabled but before smp::bring_up_aps() initialized the BSP local APIC; real AHCI/NVMe can deliver MSI/MSI-X during probe; the handler `apic::end_of_interrupt()` asserted because APIC_BASE_PHYS was still zero" |
| 2 | Commit hash present | ✅ | `72a0a41c8d6ef6d8bb6e7981dbc928c8cface40c` on `origin/main` (verified via `git log --oneline origin/main`) |
| 3 | `cargo k` PASS | ✅ | Jarvis claim accepted; corroborated by rebuilt staging hashes matching independently |
| 4 | `neo-extract-log` tests PASS | ✅ | Jarvis claim accepted; corroborated by `neo-extract-log` binary at staged SHA256 `dd75e1335c722d0cfbf078061244e8037111e7273d095947d14d4803d00ae9c7` functioning correctly on bare metal (9202-byte AHCI-lane extraction) |
| 5 | QEMU AHCI PASS | ✅ | Jarvis claim accepted |
| 6 | QEMU NVMe PASS | ✅ | Jarvis claim accepted (note: bare-metal NVMe lane surfaced a *separate* new bug; see BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md. That bug does not invalidate this gate or this fix.) |
| 7 | AHCI bare-metal build-only PASS | ✅ | Jarvis claim accepted; corroborated by 3/3 AHCI staging files hash-match |
| 8 | NVMe bare-metal build-only PASS | ✅ | Jarvis claim accepted; corroborated by 3/3 NVMe staging files hash-match |
| 9 | All 7 rebuilt staging SHA256 INDEPENDENTLY VERIFIED | ✅ | `Get-FileHash -Algorithm SHA256` MATCH on all 7 files (full list below) |
| 10 | PL rerun instruction present | ✅ | "Rerun AHCI lane only first. Do not run NVMe until AHCI passes." (Honored by PL.) |

**Independent diff audit (beyond Jarvis's stated claims):**

- Commit on `origin/main` (pushed), HEAD is `main` (not detached)
- Diff touches exactly the two files described, +45 lines, no scope creep
- APIC-init placement in `main.rs` is between `mm::init(...)` and `drivers::block::init()` — exactly where the root cause demands
- AP path refactored to route through the same idempotent helper — AP startup not broken
- Atomic guard uses correct memory ordering (`AcqRel` for write-side, `Acquire` for read-side)
- No surprise call sites: `apic::init_bsp` / `APIC_BASE_PHYS` / `init_bsp_apic` symbols live only in `apic.rs`, `main.rs`, `smp.rs`

**Staging SHA256 ledger (independently verified):**

```
f6a1f44f84cc7e7a1c270f1392d106cde8e8c648f2fd952764961c2602a4a411  BUILD-5b-staging/ahci/ahci-scenario.iso
77c4ef29e7184d49deec91db8a47588bdcdde1297d0372f13824efffe0db1083  BUILD-5b-staging/ahci/ahci-scenario-disk.raw
79a9ac85947cc141dec3284a82f59b21ae2b882db43b3d22bc29657e5b998282  BUILD-5b-staging/ahci/ahci-log-sink-disk.raw
8cbe1dfdefdfff7d107ad51c81ba995da8cb131cbe69284d2815fafbbda30929  BUILD-5b-staging/nvme/nvme-scenario.iso
77c4ef29e7184d49deec91db8a47588bdcdde1297d0372f13824efffe0db1083  BUILD-5b-staging/nvme/nvme-scenario-disk.raw
79a9ac85947cc141dec3284a82f59b21ae2b882db43b3d22bc29657e5b998282  BUILD-5b-staging/nvme/nvme-log-sink-disk.raw
dd75e1335c722d0cfbf078061244e8037111e7273d095947d14d4803d00ae9c7  BUILD-5b-staging/neo-extract-log
```

All 7 MATCH. (NVMe scenario-disk + log-sink raws intentionally share hashes with their AHCI counterparts — same raw layouts, different scenario ISOs.)

## Bare-metal proof of fix

PL re-ran the AHCI lane on the OptiPlex 7080 on 2026-05-27 23:15–23:29 EDT. Result: ALL FOUR ACCEPTANCE GATES GREEN. See [evidence/phase3-build5b-optiplex-7080-20260528-ahci-lane-rerun-PASS.md](evidence/phase3-build5b-optiplex-7080-20260528-ahci-lane-rerun-PASS.md). The same PuTTY serial log shows BOTH the OLD-kernel APIC panic (line 215) AND the NEW-kernel AHCI scenario PASS (line 397) in one continuous session — a smoking-gun comparison documenting exactly what the fix changed.

## Soft observations (non-blocking)

1. **Commit message** is title-only. Per `feedback_commit_message_deviation_closure.md` discipline, fix-commits closing tracked deviations should name the deviation in the commit body. The original AHCI panic was an active blocker rather than a numbered deviation, so this is a soft observation only — but future BLOCKER-closing commits (including the upcoming DMA-bug fix dispatched via HANDOFF-2026-05-28-phase3-build5b-ahci-log-sink-dma-bug-to-jarvis.md) should include explicit BLOCKER-ID references in commit message bodies.

2. **Local-only commit `5456d23`** (`chore: append latest irqsafe_mutex_bench serial log output`) sits ahead of `origin/main` by one commit. It is unrelated to BUILD-5b, was correctly identified by Jarvis as out-of-scope, and remains unpushed. No action required by this PM-DISPOSITION; it stays local pending PL's separate decision.

## Status

- **APIC-panic blocker:** CLOSED
- **Phase 3 BUILD-5b AHCI lane:** GREEN (bare metal)
- **Phase 3 BUILD-5b NVMe lane:** PARTIAL (kernel-side scenario PASS; on-disk extraction failed on a SEPARATE new bug — see [BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md](BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md))
- **Phase 3 BUILD-5b overall close:** PENDING the DMA-bug fix
