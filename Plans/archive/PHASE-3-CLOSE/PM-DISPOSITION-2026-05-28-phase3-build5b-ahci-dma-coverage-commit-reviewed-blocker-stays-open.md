# PM-DISPOSITION — Phase 3 BUILD-5b — Jarvis commit `87a49d1` reviewed: QEMU coverage gate ACCEPTED as a separate improvement; AHCI log-sink DMA BLOCKER REMAINS OPEN

**Date:** 2026-05-28
**Phase:** 3
**Build:** BUILD-5b
**Reviewer:** PM (Cerebro / Claude)
**Subject commit:** `87a49d1db72460493db500b832b0339840116d55` — "tools: gate bare-metal log sink qemu layout"
**Author:** SecondStageTurbine (Jarvis cycle)
**BLOCKER under review:** [BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md](BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md)
**Re-dispatch:** [HANDOFF-2026-05-28-phase3-build5b-ahci-log-sink-dma-bug-to-jarvis-ROUND2-kernel-fix-required.md](HANDOFF-2026-05-28-phase3-build5b-ahci-log-sink-dma-bug-to-jarvis-ROUND2-kernel-fix-required.md)

## One-line verdict

Jarvis closed a real QEMU coverage gap but did not fix — and by his own statement did not attempt to fix — the kernel-side AHCI log-sink DMA defect. The defect is confirmed real on bare metal with confound-free chain-of-custody. Commit `87a49d1` is ACCEPTED as a standalone CI-coverage improvement; the BLOCKER **stays OPEN**.


## Verification cadence ledger

Same cadence that accepted `72a0a41` and that surfaced this bug. Halted at the substance gate once it failed; see note at the end on gates intentionally not run.

| # | Gate | Result | Evidence |
|---|------|--------|----------|
| 1 | Commit on `origin/main`, branch `main`, not detached | ✅ PASS | `git log origin/main` top = `87a49d1`; `git log -1 --pretty=fuller` shows author/commit dates, not detached |
| 2 | Commit message body names the BLOCKER | ✅ PASS | Body cites `Plans/BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md` and includes a security note |
| 3 | **Diff matches stated root cause (kernel fix for LBA + DMA source/length)** | ❌ **FAIL** | `git show 87a49d1 --stat`: **one file, `tools/xtask/src/main.rs`, +326/-30. No kernel code, no AHCI driver code, no log-sink write-path code.** |
| 4 | Staging hashes re-verified | ⏸ NOT RUN | Substance gate (#3) already disqualifies; no hash outcome can change the verdict. See "Gates intentionally not run." |
| 5 | QEMU + bare-metal build-only logs present | ✅ (as reported) | cargo k PASS, neo-extract-log tests 6/6 PASS, build-only PASS — all real and credible, but validate the harness change, not a fix |
| 6 | Security `.rodata`-leak fix included or explicitly deferred | ❌ **FAIL** | Not included; not deferred-with-plan. Leak path is fully intact (zero write-path lines changed) |
| 7 | PL rerun instruction concrete and minimal | ✅ (as written) | NVMe-lane-only instruction is concrete — but would re-trigger the leak, since the defect is unfixed |

Gate #3 and #6 are disqualifying. The BLOCKER cannot close.


## Why the BLOCKER does not close

### 1. No kernel code changed — the fix is absent by construction

The diff is entirely in the build/test orchestrator (`tools/xtask/src/main.rs`). The defect lives in the kernel's AHCI log-sink write submission. You cannot fix a kernel DMA defect without touching the kernel. Jarvis states this himself in the commit body: *"no kernel AHCI/log-sink DMA change is included."* Acceptance criterion #1 of the BLOCKER (correct both the destination-LBA arithmetic AND the DMA source pointer/length) is therefore unmet.

### 2. The bare-metal evidence is confound-free — the defect is real

The reviewer specifically checked the one hypothesis under which Jarvis's "no real defect" reading could survive: that the `0x434A51` `.rodata` was stale residue rather than a fresh kernel write. The NVMe-lane PARTIAL evidence kills it:

- The Samsung SATA was freshly `dd`'d with `nvme-log-sink-disk.raw` (`conv=fsync`, then `sync`) immediately before the run.
- A byte-for-byte `cmp` against the raw confirmed the disk clean pre-run: only the offset-0 `NEOPLUTARCH_PHASE` sentinel, zero differences.
- Post-run, `NEO_LOG_v1` + ~500 bytes of verbatim kernel `.rodata` appeared at `0x434A51`; the raw provably contains no such magic anywhere.

The kernel wrote that, on real hardware, to a disk verified clean beforehand. There is no provisioning/residue confound. The wrong-LBA + `.rodata` over-DMA defect is real, kernel-side, and reproduced on bare metal. (Corroborating self-deception symptom: serial line 437 `bare_metal_log_sink_selftest=ok cursor=246` — the kernel reads back from the same wrong location it wrote to, so its round-trip selftest passes over corrupt data, which is why the framebuffer showed green.)

### 3. The central inference is a logic error

"QEMU did not reproduce it, therefore there is no defect" is backwards. QEMU failing to reproduce a bug that bare metal *did* reproduce (with the chain-of-custody above) is confirmation of the BLOCKER/HANDOFF open-question possibility #1 — QEMU's emulated AHCI/SATA controller is more forgiving than a real HBA on byte-offset/sector-unit translation and masks the defect. It is not exoneration of the kernel. Jarvis held the exact data point that should have read as "QEMU is blind to this class; the bare-metal bug is real and still needs a kernel fix," and concluded the opposite.

### 4. The new gate produces a false green for this exact defect

By Jarvis's own validation numbers, the new QEMU AHCI-log-sink gate **passes** the very path bare metal **fails** (QEMU NVMe topology: AHCI log-sink extraction valid, 1457 bytes). A gate that cannot fire for the defect it is meant to guard against provides false assurance. With `87a49d1` in place, a "QEMU NVMe PASS" now actively papers over a live kernel-memory-to-disk leak. This is worse than no gate, until the kernel defect is fixed.


## What Jarvis got right (credit)

This is a scope substitution, not a botch or a deception. Specifically commendable:

- The QEMU coverage gap is **real and now closed**: the normal `cargo neo-test` path used the VM storage layout and never exercised AHCI-as-log-sink for the NVMe BUILD-5b lane. Single-scenario `--bare-metal-mode` can now run under QEMU through the same backend classifier as the exported bundle, and validates the raw log sink (header offset, scenario name, nonzero capture, no degradation, PASS marker, no stray `NEO_LOG_v1` copies). This answers the HANDOFF open question and is worth keeping.
- The commit message is **transparent** about exactly what it does and does not do, including the explicit "no kernel change included" note. No attempt to disguise the scope.
- The supporting validations (cargo k, neo-extract-log 6/6, release build, build-only) are real and pass.

Because of this, `87a49d1` is **kept on `origin/main`, not reverted.** It is reclassified as a standalone CI-coverage improvement rather than a BLOCKER close.

## Gates intentionally not run (and why that is not a shortcut)

The cadence doctrine is "do not shortcut." Skipping a check that *could change the verdict* is a shortcut; skipping one that *cannot* is judgment. Staging-hash re-verification (gate #4) only bears on whether to greenlight a PL bare-metal rerun. We are not greenlighting one — a rerun on this commit would simply re-trigger the leak. The disqualifying findings are at the source-diff and security gates, which are dispositive on their own. If a future commit reaches greenlight readiness, all 7 hashes get verified before PL touches hardware.

## Disposition (PM call — operational/technical, owned by PM)

1. **BLOCKER stays OPEN.** `87a49d1` does not close it.
2. **Keep `87a49d1`** on `origin/main` as an accepted coverage improvement. Do not revert.
3. **Re-dispatch to Jarvis (Round 2)** for the actual kernel fix — the side-by-side AHCI-vs-NVMe driver write-path comparison the original HANDOFF asked for and that was skipped. Validation gate is **bare metal**, not QEMU. See the Round-2 HANDOFF.
4. **Do NOT re-provision the Samsung SATA.** Forensic evidence at `0x4321A2` / `0x434A51` is preserved until the real fix lands and a bare-metal rerun returns clean.
5. **Secondary follow-up (lower priority than the fix):** the QEMU AHCI-model fidelity gap (emulation too forgiving on LBA/length to reproduce the class) warrants a tracked note so we do not over-trust QEMU green for hardware-path defects. Behind the kernel fix in priority.

## Status after this disposition

- AHCI lane: ✅ GREEN bare metal (unchanged, `72a0a41`).
- NVMe lane: 🟡 PARTIAL — kernel-side scenario PASS; on-disk log-sink extraction still failing on the unfixed AHCI DMA defect.
- Phase 3 BUILD-5b: **one genuine kernel fix away from close** — and that fix has not yet been written.

---

**PM-DISPOSITION end.** Re-dispatch handed to PL for delivery to Jarvis.
