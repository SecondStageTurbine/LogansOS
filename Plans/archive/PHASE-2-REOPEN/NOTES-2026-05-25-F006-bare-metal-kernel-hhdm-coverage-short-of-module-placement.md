# NOTES: F-006 — Bare-metal kernel HHDM coverage falls short of Limine module placement

- **Author:** PM
- **Date filed:** 2026-05-25
- **Status:** CLOSED by `Plans/CLOSE-REPORT-2026-05-25-f006-bare-metal-hhdm-coverage.md`. First real bare-metal-only defect. **INTERMITTENT** — see §11.
- **Last updated:** 2026-05-25 (closed after 10 consecutive OptiPlex 7080 F-006 reverify boots passed with HHDM coverage reports, scheduler smoke pass, `neo-init exit code 0`, clean frame leak audit, and zero PAGE_FAULTs)
- **Defect ID:** F-006 (next in F-00X sequence; F-001..F-005 all closed in prior audit chain)
- **Severity:** BLOCKER for Bible Article 13 bare-metal close-gate. Phase 2 formal close is now gated on F-006 resolution.
- **Class:** real-hardware-only — does NOT reproduce on QEMU TCG (the per-commit gate) or VMware Workstation (the 2026-05-11 hypervisor-VM gate). This is precisely the class of bug bare-metal verification was supposed to surface.
- **Discovered during:** Production ISO boot smoke on Dell OptiPlex 7080 per `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md` §6.1.
- **Repo state at discovery:** HEAD `081f35c` on `origin/main`. Production ISO SHA256 `53451602C37CC472BF60E84FB6E2A313932017D6F1DF2BBE495C4E350EB445F5`.

---

## 1. Symptom

OptiPlex 7080 production boot reaches the point where the kernel attempts to ELF-parse the neo-init module bytes, then page-faults reading those bytes, and hangs.

Final serial output before hang (full transcript captured to evidence file):

```
[ kernel ] elf: neo-init module at 0xffff80047d4ba000 size=3431400 bytes
[ frame ] phase=baseline-pre-spawn used=1038 delta=+1038
[ trap  ] PAGE_FAULT (#PF) cr2=0xffff80047d4ba000
[ trap  ] PAGE_FAULT err = PageFaultErrorCode(0x0)
[ trap  ] PAGE_FAULT rip=0xffffffff9323beed cs=0x8 rflags=0x10086 rsp=0xffff80006a43e3c0 ss=0x10
```

Everything before this point is healthy: 12 CPUs brought up, scheduler smoke test passed (5 ping, 5 pong, hog reaped, 22 ticks), per-AP GS-base sentinels all correct, slab init complete on all CPUs.

## 2. Diagnosis (hypothesis)

Earlier in the same boot:

```
[ kernel ] mm: hhdm_offset=0xffff800000000000, memmap_entries=70
[ kernel ] mm: hhdm_phys_end=0x000000047d175000
[ kernel ] mm: usable physical memory = 16068 MiB
```

The neo-init module is logged at virtual `0xffff80047d4ba000`. Subtracting `hhdm_offset` (`0xffff800000000000`):

- Module physical address: `0x47d4ba000`
- Kernel's HHDM covers up to: `0x47d175000`
- Module starts: `0x345000` (≈3.3 MiB) **above** the kernel's HHDM extent.

Page-fault CR2 = module virt = `0xffff80047d4ba000`. Error code `0x0` = supervisor read, page not present, no reserved-bit violation. The kernel attempts to read the ELF bytes via the HHDM mapping, but the HHDM does not actually cover that physical page.

**Root cause hypothesis:** the kernel's HHDM-build logic computes `hhdm_phys_end` by iterating Limine's memmap and tracking the highest **usable RAM** end. It does NOT consider that Limine may have placed the neo-init module (and neo-worker) in `KERNEL_AND_MODULES`-typed regions, which can be at higher physical addresses than the last `usable` entry the kernel iterates.

On the OptiPlex 7080 with 16 GB:
- Total RAM ends at `~0x47d800000` (per Ubuntu's dmesg evidence, 2026-05-25 reference run)
- Limine placed neo-init at `0x47d4ba000` — near the top of available memory
- Kernel's `hhdm_phys_end = 0x47d175000` — short of that by ~3.3 MiB

QEMU TCG's default test memmap is small (typically 128–512 MiB) and contiguous; module placement falls well within the usable RAM range. VMware similarly presents a clean contiguous memmap. Neither environment exercises the case where Limine places a module above the kernel's HHDM extent.

## 3. Why bare-metal exposes this and emulators do not

Bare-metal physical memory has real reserved/MMIO regions punched out of the address space. The 2026-05-25 Ubuntu reference dmesg shows the OptiPlex 7080's e820 memmap includes:

- Numerous `device reserved` ranges in the sub-4 GiB span
- Multiple `System RAM` segments split by reserved holes
- Bulk usable RAM in the `0x100000000`–`0x47d7fffff` band

Limine has to find a free contiguous chunk large enough for each module. On real hardware that chunk often ends up near the *top* of usable memory rather than near the bottom. The kernel's HHDM build, if it scans memmap entries by type and stops short of `KERNEL_AND_MODULES` entries (or stops at the last `usable` entry before a `reserved` hole), produces an HHDM that doesn't reach module placement.

This is **architectural** — not specific to the 7080. Any bare-metal target with a realistic e820 memmap will exhibit the same issue. Cross-vendor AMD verification (Q6(d) stretch goal) would also fail until F-006 is fixed.

## 4. Likely fix surfaces

Two design alternatives (deferred to Cerebro diagnosis dispatch; not chosen here):

- **(A) Extend HHDM-build to consider module-region entries.** The kernel iterates Limine's memmap and tracks the max physical address across `usable` + `bootloader_reclaimable` + `kernel_and_modules` types. Most permissive option; covers any region Limine has used.
- **(B) Compute HHDM end from Limine's executable + module responses directly.** After building from memmap, take `max(hhdm_phys_end, executable_phys_end, max(module_phys_end))` as the actual mapped extent. More targeted; only extends to cover real placements.

Either approach is a small, local change to `kernel/src/mm/*` (probably `kernel/src/mm/hhdm.rs` or wherever the HHDM build logic lives — Cerebro to confirm during diagnosis).

There's also a **(C) defensive option**: map the *entire* physical address space up to the highest memmap entry of any type. This is what Linux does for direct-map. It's the most robust but uses more page table memory; for AltimitNeo's 16 GB scale this is trivial overhead.

## 5. Reproduction

**On bare-metal:**
1. Build production ISO from any `origin/main` commit at or after `081f35c`.
2. Write to USB (Rufus DD-image mode, GPT/UEFI).
3. Boot Dell OptiPlex 7080 from USB with Secure Boot disabled.
4. Serial console at 38400 baud 8N1 (kernel uses `uart_16550` crate default; NOT 115200).
5. Observe page fault at neo-init load.

**On QEMU TCG:**  does NOT reproduce. The full Phase 2 scenario suite passes per `Plans/PM-DISPOSITION-2026-05-25-phase-2-build-subphase-4-close.md`. Module placement always falls within the kernel's HHDM extent on QEMU's small contiguous memmap.

**On VMware Workstation:** does NOT reproduce per the 2026-05-11 hypervisor-VM verification.

## 6. Procedure deviation

Per `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md` §8 (stop conditions): "Unexpected `KERNEL PANIC` → Stop run. Capture full serial. File `Plans/evidence/…-BLOCKER-<scenario>.serial.log` + a brief NOTES doc naming the panic site. PM triages."

Strictly speaking the kernel did not panic — it page-faulted and hung. Operationally identical to a panic for procedure purposes. Treating as a STOP condition.

**Bare-metal verification is HALTED.** The remaining scenario sweep (§6.2 onward) does not run until F-006 is resolved and a fresh boot smoke passes.

## 7. Evidence

- Full serial log of failed boot: `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-production-smoke-BLOCKER.serial.log` (kept in the private repo) (to be moved/saved from PL's PuTTY log file at the original PuTTY log path).
- Reference healthy run (Ubuntu live disc, same hardware, same chain): `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-183600-12-serial-chain-baseline.serial.log` (also to be moved from PL's Windows desktop into the evidence dir).
- Production ISO SHA256 used: `53451602C37CC472BF60E84FB6E2A313932017D6F1DF2BBE495C4E350EB445F5`
- Repo HEAD at boot: `081f35c`
- Kernel commit baseline: anything at or after `39a8048` (Phase 2 subphase 4 close)

## 8. Triage and next steps

Sequence:

1. **PL moves PuTTY logs into `Plans/evidence/`** and commits them with this NOTES doc. (Or PM does the move on PL's confirmation that the original PuTTY log is at a known path.)
2. **Cerebro dispatch — diagnosis + fix design.** Read `kernel/src/mm/*` to confirm hypothesis, evaluate the three fix alternatives in §4 against Bible 4.2 (`unsafe` budget), Bible 7.2 (memory model), and ADR-016 (heap-decoupling — does the fix need to allocate page tables under lock?). Recommend one alternative. Density per OBSERVE-equivalent dispatch.
3. **PL ratification** of the chosen fix alternative (if it's scope-class) or PM proceeds (if operational).
4. **Jarvis dispatch — implementation.** Code change in `kernel/src/mm/*`. New scenario `bare_metal_hhdm_covers_modules` added to the harness if a CI-testable assertion is possible (likely: assert `hhdm_phys_end >= max_module_phys_end` at HHDM-build time).
5. **PM-DISPOSITION accepting the fix.** Commit lands on `main`.
6. **PL re-verifies on OptiPlex.** Rebuild production ISO, rewrite USB, boot, confirm clean boot through neo-init load and into the rest of the production smoke sequence.
7. **Resume bare-metal scenario sweep** per the procedure §6.2 onward.
8. **F-006 close report** filed with evidence of the fix passing on bare-metal.

## 9. Honest scope framing

- F-006 does NOT invalidate Phase 2 subphases 1–4 (sys_wait/sys_inspect/process caps + ADR-016 ring + ADR-019 irq-safe + pre-hardware close gates). Those are all correctness-orthogonal to the HHDM-build issue.
- F-006 DOES block the bare-metal close-gate (Bible Article 13) and therefore Phase 2 formal close.
- This is exactly the bug class bare-metal verification was prescribed to catch. The procedure functioned as designed; the evidence trail is complete; the fix path is well-bounded.
- The 38400-baud discovery during chain verification is its own minor procedure correction (procedure §3.3 currently says "115200" — needs update to "38400" before next session). Filing that as a separate small PM-DISPOSITION patch alongside F-006 triage.

## 11. Intermittence (added 2026-05-25 post-initial-filing)

**The defect is intermittent across boots, not deterministic.** Empirical observation from PL across at least 3 bare-metal boots of the same ISO (SHA256 `5345160…445F5`):

- Boot 1: kernel splash rendered to framebuffer → implies `spawn_neo_init()` returned → F-006 did NOT trigger that boot
- Boot 2 (the BLOCKER capture): no splash, page fault at `0xffff80047d4ba000` → F-006 triggered
- Boot 3: kernel splash rendered → F-006 did NOT trigger

**Diagnostic significance.** `kernel/src/main.rs` calls `render_boot_sequence()` (the iconic boot splash) AFTER `spawn_neo_init()` returns. If F-006's page fault fires, the kernel hangs in the trap handler and the splash never renders. The splash's visibility is therefore a clean visual indicator of which path the boot took, independent of serial-console capture.

**Root cause refinement.** The intermittence is consistent with Limine's module-placement allocator varying across boots — same ISO, same hardware, same memmap, but Limine's free-region selection produces different module physical addresses on different boots. Sometimes the module lands within the kernel's HHDM coverage (`0x47d4ba000` would need to be < `hhdm_phys_end`); sometimes it lands above and triggers the fault.

**Implications for fix:**

- The fix MUST handle the worst-case placement (module above HHDM extent), not just the captured-evidence specific address. All three §4 alternatives (A/B/C) correctly handle this — they each ensure HHDM coverage reaches wherever Limine has placed things.
- The CI-testable assertion (§4 fourth point) should be `hhdm_phys_end >= max(module_phys_end)` evaluated at HHDM-build time — that catches the bug class regardless of which specific placement Limine chose on a given boot.
- Cerebro's recommendation evaluation should prefer alternatives that are robust to the FULL placement-variation surface, not just the captured failure address. This favors (B) and (C) over (A) — (A) only extends HHDM to all-usable-types but might still miss bootloader-reserved regions; (B) and (C) explicitly cover all known placements.
- The fix verification on bare-metal must be probabilistic: a single successful boot does NOT prove the fix works (might just be a boot where Limine happened to place modules low). Need multiple boots to gain confidence. Recommend at least 10 consecutive bare-metal boots all reaching scheduler-idle as the acceptance bar for the fix landing.

**Updated triage urgency.** Intermittence makes this MORE urgent to fix, not less:

- Reproducibility for debugging is worse (sometimes the bug "hides")
- A user shipping a Phase-2-close kernel could see boots succeed in staging and fail in the field with no code change
- Bible Article 1.2 ("a fast wrong answer is worse than a slow right one") applies directly: an intermittently-correct kernel is wrong by construction

## 12. References

- `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md` (`081f35c`) — procedure executed.
- `Plans/DISPATCH-2026-05-25-phase-2-hardware-optiplex-7080-verification.md` — dispatch governing this execution.
- `Plans/PM-DISPOSITION-2026-05-25-phase-2-build-subphase-4-close.md` — the close that this hardware session was meant to confirm.
- `Plans/PHASE-2-THINK-v0.2-2026-05-24.md` Q6 — bare-metal protocol ratified disposition.
- Bible Article 13 — bare-metal close-gate mandate.
- Bible Article 1.2 — correctness priority; F-006 is a correctness defect, not a performance one.
- `kernel/src/drivers/console/serial.rs` — uses `uart_16550 = "0.3"`, which hardcodes 38400 baud (procedure §3.3 baud correction context).
- Predecessor F-00X audit closes: F-001/F-002 PR #6 `4dfb13f`; F-003/F-005 `9a282f2`; F-004 absorbed into Phase 2 BUILD subphase 2 per audit close `199fb51`.

---

**Status: CLOSED. Cerebro diagnosis dispatched at `58640b3`; Jarvis fix landed at `59443de`; PM accepted reverify at `43209d3`; PL completed 10 consecutive OptiPlex 7080 F-006 reverify boots on 2026-05-25.**
