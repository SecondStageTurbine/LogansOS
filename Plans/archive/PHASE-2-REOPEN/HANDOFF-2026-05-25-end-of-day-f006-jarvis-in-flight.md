# HANDOFF: 2026-05-25 end-of-day — F-006 Jarvis implementation in-flight

- **Status:** Jarvis working on F-006 implementation; PM idle pending return.
- **Repo HEAD:** `4a8cc5c` (`plans: F-006 Jarvis implementation dispatch`).
- **Workspace:** `E:\AI Automation\AltimitNeo\` (Windows, PL machine).
- **Repo:** `github.com/SecondStageTurbine/NeoPlutarch`.
- **Author:** PM (Cerebro/Claude).
- **Audience:** future PM instance picking up after compaction, or PM resuming next session.

---

## 0. Read-this-first

If you are a fresh PM instance reading this cold, the minimum-read set to reconstitute state is:

1. This handoff doc.
2. `Plans/NOTES-2026-05-25-F006-bare-metal-kernel-hhdm-coverage-short-of-module-placement.md` (defect file, includes §11 intermittence).
3. `Plans/CEREBRO-OUTPUT-2026-05-25-F006-diagnosis.md` (`c6dc59a`, the diagnosis you're acting on).
4. `Plans/PM-DISPOSITION-2026-05-25-F006-cerebro-diagnosis-close.md` (`d53c7c2`, your accept + open-dependency resolutions).
5. `Plans/DISPATCH-2026-05-25-F006-jarvis-implementation.md` (`4a8cc5c`, what Jarvis is currently doing).

Everything else in this handoff is context, narrative, and discipline. The five files above are the substantive state.

---

## 1. Immediate state

- **Jarvis is working** on F-006 code-bearing implementation per dispatch `4a8cc5c`. Time bound 4 hours from dispatch (16:42 local). Expected return window: ~20:42 local, plausibly later if blockers surface.
- **Phase 2 formal close is gated on F-006.** No other Phase 2 work is in flight.
- **Bare-metal hardware (OptiPlex 7080) is set up and verified** end-to-end. Serial chain works at 38400 baud. PROCEDURE doc filed at `081f35c`. Production ISO built (`boot/neoplutarch.iso`, SHA256 in PROCEDURE Appendix). PL can re-flash and re-boot the moment Jarvis lands the fix.
- **Untracked file:** `ai_digest.py` in repo root. Pre-existing, not Jarvis's. Cerebro noted it; leave it alone.
- **No outstanding PL ratifications.** F-006 fix is operational scope class (Cerebro §6, PM-DISPOSITION §6). PL is courier + acceptance ratifier on the eventual close report.

---

## 2. Today's session arc (commit-by-commit narrative)

Today was a single continuous arc: Phase 2 PLAN cadence completion → hardware verification execution → F-006 defect discovery + full triage chain. Commits in order:

### 2.1 Morning: Phase 2 PLAN cadence + Prometheus polish

- `8bdb4be` — PM accepted Cerebro THINK v0.2 substance, dispatched Prometheus polish.
- (intermediate Prometheus polish commits, see git log for details)
- `2fcad0e` — End-of-morning handoff filed (now superseded by this doc).
- `683d193` / `346008d` — Subphase 4 acceptance + hardware scenario media clarification.

By midday all Phase 2 software BUILD subphases 1-4 were CLOSED. Subphase 1 (`6587cd6`+`f87a26e`), Subphase 2 (`5aa5baa`+`950ece5`), Subphase 3 (`e7094a8`+`a202ed4`), Subphase 4 (`b7d27a6`+`39a8048`). See `Plans/CLOSE-REPORT-2026-05-25-phase-2-build-subphase-*.md` for the four close reports.

### 2.2 Afternoon: hardware verification execution

- **15:05** — `081f35c` PROCEDURE doc filed. Bare-metal verification recipe authored: serial chain setup, BIOS config, build sequence, per-scenario execution paths, evidence-path patterns, stop conditions, expected-panic exception for `q7c_shootdown_timeout_panic`, Appendix A hardware inventory.
- Hardware setup steps executed in parallel:
  - 6CC5P internal cable installed motherboard 10-pin SERIAL header → DB9 bracket.
  - Arnorin null modem cable connected.
  - DTECH FTDI USB-RS232 adapter to Windows COM3.
  - BIOS Serial Port set to COM1 on OptiPlex side.
  - PuTTY configured COM3 / **38400 baud 8N1** (NOT 115200 — the `uart_16550 = "0.3"` crate hardcodes 38400; PROCEDURE §3.3 still says 115200 and needs patching, see §7.6 below).
  - Ubuntu live-disc dmesg at 18:36 UTC proved chain end-to-end with ~500 clean lines confirming `ttyS0 at 0x3F8 IRQ 4 16550A`.
- Production ISO built: `boot/neoplutarch.iso`, 23.79 MB, El Torito with both BIOS (`limine-bios-cd.bin`) and UEFI (`limine-uefi-cd.bin`) boot entries. SHA256 captured in PROCEDURE.

### 2.3 Afternoon late: F-006 discovery + triage

- **15:55-ish** — Production ISO boot smoke on OptiPlex. Kernel booted cleanly through 12-CPU bringup + scheduler smoke test PASS (5 ping, 5 pong, hog reaped, 22 ticks). Then **PAGE_FAULT** at neo-init module load.
- `83e8112` — F-006 NOTES filed. Initial hypothesis: HHDM coverage falls short of where Limine placed the neo-init module. Evidence shows `hhdm_phys_end=0x47d175000` but `neo-init` mapped at `phys 0x47d4ba000` (~3.3 MiB above HHDM extent), CR2 fault at exactly that virt address, err=0x0 (supervisor read, page not present).
- `8aad054` — Evidence logs committed. Two serial logs:
  - `phase2-bare-metal-optiplex-7080-20260525-production-smoke-BLOCKER.serial.log` (kept in the private repo) (150KB clean 38400-baud capture with F-006 fault).
  - `phase2-bare-metal-optiplex-7080-20260525-production-smoke.serial.log` (45KB garbled 115200 baseline before baud rate corrected).
- `58640b3` — Cerebro dispatch original, asking diagnosis + alternative evaluation.
- **PL observed:** kernel splash (rendered in `kernel/src/init/boot_frame.rs` AFTER `spawn_neo_init` returns) appeared on boots 1+3, absent on boot 2 (the BLOCKER capture). Same ISO, same hardware. **F-006 is intermittent.**
- `e2dedf9` — F-006 NOTES updated with §11 intermittence finding. Limine's module-placement allocator picks varying physical addresses run-to-run; some land within HHDM coverage, some above. Acceptance bar sharpened: ≥10 consecutive successful boots, not one.
- `76f6586` — Cerebro dispatch patched with intermittence framing. Re-dispatched.
- **Cerebro returned** at `c6dc59a` (`Plans/CEREBRO-OUTPUT-2026-05-25-F006-diagnosis.md`). 181 lines, 15.6 KB. Hypothesis confirmed at source. Recommends option **D / refined B**: actual-span HHDM repair and assertion driven by Limine executable + module responses. Avoids A's memmap-type dependency and C's broad direct-map.
- `d53c7c2` — PM-DISPOSITION accept option D. Three open dependencies resolved operationally: (1) `BootHhdmRequirements` struct as third arg to `mm::init`; (2) executable span included in same helper; (3) test-only forced-unmap cfg hook for QEMU repair-path coverage. Operational scope class.
- `4a8cc5c` — Jarvis implementation dispatch filed. Code-bearing. 4-hour time bound. **CURRENT HEAD.**

---

## 3. F-006 full context (for fresh-instance reconstitution)

### 3.1 Defect

Bare-metal boot on OptiPlex 7080 PAGE_FAULTs when `spawn_neo_init` calls `module.data()` on the neo-init Limine module and the subsequent ELF parse dereferences a virtual address that has no HHDM mapping. Kernel hangs in trap handler.

**Faulting path (Cerebro-confirmed):**
- `kernel/src/main.rs:132-160` `spawn_neo_init` → calls `module.data()`.
- `module.data()` returns `&[u8]` at `hhdm_offset + module_phys` (Limine adds the offset).
- `spawn_user("neo-init", bytes)` → `kernel/src/sched/process.rs:917` calls `Elf::parse(elf_bytes)`.
- `kernel/src/loader/elf.rs:98-107` dereferences `bytes.as_ptr()` as ELF header.
- If `hhdm_offset + module_phys` was never mapped → PAGE_FAULT.

**Root cause (Cerebro-confirmed via source read):**
- `kernel/src/mm/mod.rs:66` is `mm::init(hhdm_offset, memmap)`.
- `kernel/src/mm/mod.rs:85` filters `e.type_ == limine::memmap::MEMMAP_USABLE` only.
- `kernel/src/mm/mod.rs:89` stores that result in `HHDM_PHYS_END`.
- Kernel does NOT build HHDM from scratch — Limine provides page tables, kernel wraps in `OffsetPageTable` (`paging.rs:34`).
- Bug: no boot-time contract check that Limine module spans are mapped before dereferencing. Stored extent (`HHDM_PHYS_END`) is too low because it's derived from MEMMAP_USABLE only, not module/executable spans.

### 3.2 Intermittence (NOTES §11)

Limine's module-placement allocator is **non-deterministic across boots**. Same ISO, same hardware: on boot 1+3 the kernel splash appeared (boot reached the post-`spawn_neo_init` framebuffer write); on boot 2 it didn't (the BLOCKER capture). Splash visible = module placed in mapped range = F-006 didn't trip. Splash absent = module placed above HHDM extent = F-006 tripped.

**Sharpens acceptance bar:** a single successful boot is variance, not fix. Need ≥10 consecutive successful boots from the same ISO.

### 3.3 Fix (Cerebro option D, accepted)

Derive required HHDM coverage from Limine's authoritative responses (executable address response + module file data response), not from memmap-type filtering. In `mm::init`, after `HHDM_OFFSET` is set and boot frame allocator is available:

1. Compute `required_hhdm_phys_end = max(usable_end, exec_end, max(module_end))`.
2. For each required span, walk page tables and check translation at `hhdm_offset + phys`.
3. For any missing leaf, install HHDM leaf mapping using **local boot frame allocator** (not `FRAME_ALLOC` — ADR-016 substrate discipline).
4. Store `HHDM_PHYS_END = required_hhdm_phys_end` (conservative).
5. Assert each required span's first and last page translate post-repair; panic with diagnostic on failure (boot unrecoverable).
6. Emit per-boot serial diagnostic line `[hhdm] required_end=0x... exec=[0x...,0x...) modules=[name=[0x...,0x...),...] repair_pages=N` BEFORE the assertion runs.

### 3.4 Why not A/B/C

- **A** (extend coverage by additional memmap types): Limine spec says `EXECUTABLE_AND_MODULES` memmap entries are "illustrative not authoritative" — the responses are the authoritative source. Cerebro §3 cites protocol lines 1114-1121, 1129-1137, 1143-1146.
- **B unrefined** (continuous max-fill from usable_end to highest required): unnecessary breadth; D narrows to explicit spans.
- **C** (full direct map up to highest memmap entry): on the OptiPlex high RAM end `0x47d800000`, ~17.96 GiB span. Even with 2 MiB leaves, 9196 leaf entries / ~76 KiB page tables. Broader policy decision, risks mapping reserved/MMIO with wrong memory type, larger unsafe budget.

### 3.5 Open dependencies resolved (PM-DISPOSITION §3)

1. **BootInfo plumbing:** introduce `BootHhdmRequirements` struct passed as third arg to `mm::init(hhdm_offset, memmap, boot_hhdm_reqs)`. Documented contract; extensible without re-touching call sites.
2. **Executable span:** include it. Marginal cost trivial; closes latent variant of the same bug class.
3. **QEMU repair-path test coverage:** test-only forced-unmap hook behind cfg gate, NOT synthetic page-table harness. Forced-unmap exercises the actual repair code path against real page tables.

---

## 4. In-flight worker dispatches

| Worker | Task | Dispatch | Time bound | Output expected |
|---|---|---|---|---|
| **Jarvis** | F-006 implementation per Cerebro option D | `Plans/DISPATCH-2026-05-25-F006-jarvis-implementation.md` (`4a8cc5c`) | 4 hours from 16:42 local | Code-bearing commit(s) on `main`; CI green; two new `neo-test` scenarios green |

No other workers in flight. Cerebro returned (`c6dc59a`). Prometheus idle since Phase 2 polish.

---

## 5. When Jarvis returns: triage flowchart

### 5.1 Jarvis lands code successfully (CI green, both scenarios pass)

1. **Read** Jarvis's commit messages + the actual diff. Verify against DISPATCH §2 (file targets), §3 (signatures), §4 (repair helper contract), §5 (serial diagnostic), §6 (tests).
2. **Verify constraints honored:**
   - All `unsafe` blocks have SAFETY comments naming the invariant (Bible 4.2).
   - Repair path uses local boot frame allocator, NOT `FRAME_ALLOC` (ADR-016).
   - No heap allocation in repair path.
   - Upper-half HHDM only.
   - Pre-AP-bringup single-threaded context preserved.
   - Forced-unmap hook is cfg-gated, default-off in normal builds.
3. **File PM acceptance:** `Plans/PM-DISPOSITION-2026-05-25-F006-jarvis-implementation-accept.md` (or close-report-style if Jarvis files a close report first; match the convention of subphase 1-4 close reports).
4. **Dispatch PL bare-metal re-verification.** Tell-PL text: build the new ISO (`docker compose run --rm neo cargo k` then `cargo iso`), flash USB via Rufus DD-image, run ≥10 consecutive OptiPlex boots, capture serial log per boot, verify `[hhdm]` line present each time with concrete values, zero PAGE_FAULTs. Evidence path pattern: `Plans/evidence/phase2-bare-metal-optiplex-7080-<YYYYMMDD-HHMMSS>-12cpu-f006-reverify-boot<N>.serial.log` for each boot.

### 5.2 Jarvis lands code but CI fails

- Read the CI output. Common failure modes for this kind of change:
  - Linker symbol naming mismatch (DISPATCH §3 noted "RAG-search for existing `__kernel_*` symbol references; if none exist, Jarvis introduces the linker-symbol pair").
  - `OffsetPageTable` API mismatch — Jarvis might have assumed a method that doesn't exist; check `kernel/src/mm/paging.rs` for actual API.
  - `neo-test` scenario harness pattern not matching existing scenarios — point Jarvis at the most recent Phase 2 scenario (`q7c_shootdown_timeout_panic`) for the canonical shape.
  - Forced-unmap cfg gate breaking default builds (cfg syntax wrong).
- Triage: small fix usually warrants direct PM patch-dispatch back to Jarvis ("the cfg gate should be `#[cfg(neo_test_force_unmap_module)]` not a feature; rest is fine; re-push"). Don't over-think.

### 5.3 Jarvis files a blocker note

Per DISPATCH §8, Jarvis is instructed to STOP + file `Plans/JARVIS-BLOCKER-2026-05-25-F006-implementation-<topic>.md` rather than expand scope unilaterally.

- Read the blocker note.
- Three likely categories:
  - **Local boot frame allocator isn't actually reachable** at the point where `mm::init` needs it for HHDM repair. → PM evaluates: can we get it there with a small plumbing change, or does the fix need to move to a slightly later boot point? If later, does that still beat `spawn_neo_init`?
  - **Linker symbol for kernel image end doesn't exist and adding it is non-trivial.** → PM decides: drop executable span from required set (NOTES that the variant is latent but not currently observed), or add the symbol pair (probably a one-liner in the linker script).
  - **Something genuinely surprising** (e.g. `OffsetPageTable` doesn't expose page-table mutation in the shape needed, or the repair path can't be expressed in safe-Rust + narrow unsafe). → Stop and think before dispatching. May warrant a Cerebro micro-dispatch.
- Don't escalate to PL unless scope class changes (e.g. if blocker forces a substrate change that touches ADR-016 or a Bible-named invariant).

### 5.4 Jarvis is silent past the 4-hour bound

- Check `git log origin/main` — Jarvis sometimes pushes without surfacing.
- Check for any new files in `Plans/` — Jarvis may have filed a partial-progress note.
- If genuinely silent and no commits: 4 hours is a soft bound, not hard. Phase 2 close isn't bleeding. Note the slip in this handoff (revise §1 with updated time), continue waiting.

---

## 6. After Jarvis lands: PL bare-metal re-verification protocol

This is the **acceptance step** for F-006 close. Don't skip or shortcut.

### 6.1 Build the new ISO

```powershell
cd "E:\AI Automation\AltimitNeo"
docker compose run --rm neo cargo k
docker compose run --rm neo cargo iso
```

Output: `boot/neoplutarch.iso`. Record SHA256.

### 6.2 Flash USB

Rufus, DD image mode, GPT partition scheme for UEFI, FAT32. Target USB stick already known-good from production smoke.

### 6.3 Boot loop

Per PROCEDURE §6.2 pattern, plus F-006-specific evidence capture. For each of ≥10 boots:

1. Power off OptiPlex.
2. Insert USB.
3. Power on. PuTTY at COM3 / 38400 / 8N1 / capture-to-file enabled.
4. Capture filename: `Plans/evidence/phase2-bare-metal-optiplex-7080-<YYYYMMDD-HHMMSS>-12cpu-f006-reverify-boot<N>.serial.log` where N is 1..10+.
5. Verify in each log:
   - `[hhdm] required_end=0x... exec=[0x...,0x...) modules=[...]` line present.
   - Concrete (non-zero) values in `required_end` and per-module spans.
   - `repair_pages` count consistent (probably 0 on most boots, ≥1 on placement-high boots).
   - Boot reaches scheduler-idle (post-`spawn_neo_init`, hog reaped, ticks running).
   - **Zero PAGE_FAULTs.**
6. Power off. Wait 5 seconds. Repeat.

Don't power-cycle between boots to "reset" anything — placement variation comes from Limine, not from system state. Successive boots from the same USB on the same hardware are the test.

### 6.4 Acceptance criteria

- ≥10 consecutive boots all passing all per-boot checks → F-006 close-eligible.
- Any boot with PAGE_FAULT → fix incomplete. Capture the failing log, return to Jarvis with the specific module physical span that failed.
- Any boot missing the `[hhdm]` line → fix incomplete (diagnostic not emitting before the assertion path → indicates a code-path issue).
- Boots 11+ are confidence margin; not required but encouraged if there's time.

### 6.5 Close

If acceptance passes:
- PM files `Plans/PHASE-2-BARE-METAL-OPTIPLEX-7080-VERIFICATION-<YYYY-MM-DD>.md` per PROCEDURE pattern, with F-006 close section.
- PM files `Plans/CLOSE-REPORT-2026-05-25-or-26-f006-bare-metal-hhdm-coverage.md` (date depends on when re-verification actually runs).
- PM files `Plans/CLOSE-REPORT-2026-05-XX-phase-2-bare-metal-verification-complete.md`.
- Phase 2 formally CLOSED. PM updates Bible ledger Article 13 ("Phase 2 — COMPLETE") via amendment PR.

If acceptance fails: F-006 remains open; second iteration of fix dispatch to Jarvis.

---

## 7. Pending / deferred items (not blocking F-006)

### 7.1 PROCEDURE §3.3 baud-rate correction

PROCEDURE doc (`081f35c`) still says 115200 baud. Actual correct value is **38400** (confirmed by `uart_16550 = "0.3"` crate hardcode + Ubuntu live-disc verification). File a small PM-DISPOSITION patch: `Plans/PM-DISPOSITION-2026-05-XX-procedure-baud-rate-correction.md`, edit PROCEDURE §3.3, commit + push. Trivially small. Defer until after F-006 close so the PROCEDURE doc isn't churning during active hardware work.

### 7.2 NVMe FLUSH durability diagnosis (BUILD-5b)

Paused since 2026-05-23. Hypothesis: Intel E6100p NVMe doesn't honor FLUSH past the 10M-spin timeout (≈3ms). Three remediation candidates in `Plans/NOTES-2026-05-23-nvme-driver-flush-and-completion-review.md` (`46a0848`). Phase 3 work. Serial console now available for diagnosis, so technically unblocked, but not on critical path until Phase 3 work starts.

### 7.3 Phase 2 hardware scenario sweep

Currently HALTED at boot smoke. Per PROCEDURE §6.2 onward, the full scenario sweep includes:
- `default_smoke` (the one that hit F-006)
- `sched_smp` set (SMP basics)
- `sched_pin` (CPU pinning)
- `cap_reserved_bit` (capability reserved bit activation, BUILD-1 verification)
- `adr_016_ring` (scheduler ring, BUILD-2 verification)
- `adr_019_irq_safety` (IRQ-safe migration, BUILD-3 verification)
- `q7c_shootdown_timeout_panic` (expected-panic exception)
- (additional per PROCEDURE §6.2 — re-read before dispatching PL)

Resume after F-006 close. PROCEDURE doc already covers the full sweep.

### 7.4 F-006 close report

Cannot file until bare-metal re-verification passes (§6 above).

### 7.5 Triage queue items from ALTIMIT research artifact

`docs/research/2026-05-05-altimit-realized-bringing-hack-os-vision-into-altimitneo.md` (or wherever it landed; check git log for the canonical path) flagged several pending Bible amendments, ANSPEC-002 draft items, and ADR drafts post-BUILD-2 sub-commit 2. None of these are blocking. Defer to next Phase 2-close planning session.

---

## 8. Operational discipline carry-forward

Notes accumulated today that should survive into the next session, even after compaction.

### 8.1 Trust PL's "small observations"

PL flagged the kernel-splash visibility difference between boots 1+3 vs boot 2. PM initially logged it as an aesthetic question. It turned out to be **the diagnostic signal that F-006 is intermittent**, which sharpened the entire fix-evaluation criteria. Carry forward: when PL surfaces an observation that doesn't have an obvious place in the current plan, take 60 seconds to actually evaluate it before continuing.

### 8.2 PM stop-suggestion pattern

Earlier today PM caught itself in a "do you want me to stop and..." pattern mid-session. PL explicitly called it out. After that point: just commit and move forward; don't add false stopping points. Carry forward.

### 8.3 Dispatch text vs dispatch document distinction

When PL says "do I give Cerebro the same dispatch?" mid-arc, the answer is sometimes no — the document on disk was authored before a substantive context shift, and the tell-worker text should be patched first. Today: dispatch patched at `76f6586` after the intermittence finding before re-issuing to Cerebro. Carry forward: when conditions change mid-dispatch, patch the doc + re-issue rather than relying on the worker to read the updated NOTES on their own.

### 8.4 Cerebro discipline

Cerebro returned (`c6dc59a`) with exactly what was asked: recommendation, hypothesis confirmation, alternative evaluation with rule-out reasoning, assertion + test design, scope class. No patch code. No ADR draft. No Bible amendment. This is the discipline boundary working as designed. Carry forward.

### 8.5 Baud rate gotcha

The `uart_16550 = "0.3"` crate hardcodes 38400 baud and there's no current Cargo override or runtime config. PROCEDURE §3.3 said 115200 because earlier docs assumed the default-default. **For any future bare-metal serial work, default to 38400** until/unless someone adds a baud-rate configuration knob to the kernel.

### 8.6 Production ISO build cadence

Building `boot/neoplutarch.iso` via `docker compose run --rm neo cargo iso` is the canonical bare-metal artifact. Each F-006 fix iteration will need a fresh build + fresh USB flash. PL is set up for this; takes ~5 min including USB write.

### 8.7 Untracked `ai_digest.py`

Pre-existing untracked file. Cerebro touched nothing. Jarvis instructed nothing. Leave it alone; don't add to git unless PL surfaces intent.

### 8.8 RAG endpoint

`http://127.0.0.1:8404` — semantic code search over the NeoPlutarch repo. REST API: `/search`, `/search/full`, `/health`, `/stats`. Requires Ollama backend (`start-rag-ollama.ps1` on PL machine). Documented in `E:\AI Automation\AltimitNeo\CLAUDE.md`. Workers (Cerebro, Jarvis, Prometheus) are instructed to use it for navigation; PM should also use it when reading source.

### 8.9 Push status 1 is cosmetic

`git push` reliably returns exit code 1 due to a Windows / stderr peculiarity, even on successful pushes. Don't retry. Check `git log origin/main` to confirm the push landed.

### 8.10 Memory entries that persist

The following are in PM's long-term memory and should not need re-explanation in the next session:
- NeoPlutarch agent roster (Cerebro/Claude, Jarvis/Codex, Prometheus/Qwen3.6-27B; Janus retired).
- PM operational posture (PM owns operational + technical decisions; PL = courier + scope-class ratifier).
- RAG availability at 127.0.0.1:8404.

---

## 9. Key references

### 9.1 Today's commits in order

```
8bdb4be plans: accept Phase 2 THINK v0.2 substance + Prometheus polish dispatch
...     (Prometheus polish intermediate commits)
2fcad0e plans: morning end-of-day handoff
683d193 plans: accept subphase 4 + route hardware
346008d plans: clarify Phase 2 hardware scenario media
081f35c plans: file Phase 2 bare-metal OptiPlex 7080 verification procedure
83e8112 plans: file F-006 bare-metal kernel HHDM coverage short of module placement
8aad054 plans/evidence: capture OptiPlex 7080 production boot smoke serial evidence
58640b3 plans: dispatch Cerebro for F-006 diagnosis
e2dedf9 plans: F-006 NOTES update - intermittence finding from PL splash observation
76f6586 plans: patch F-006 Cerebro dispatch with intermittence framing
c6dc59a plans: add F-006 Cerebro diagnosis
d53c7c2 plans: F-006 PM-DISPOSITION accept Cerebro option D
4a8cc5c plans: F-006 Jarvis implementation dispatch          [HEAD]
```

### 9.2 F-006 artifact chain

| File | Commit | Purpose |
|---|---|---|
| `Plans/NOTES-2026-05-25-F006-bare-metal-kernel-hhdm-coverage-short-of-module-placement.md` | `e2dedf9` | Defect file (incl §11 intermittence) |
| `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-production-smoke-BLOCKER.serial.log` (kept in the private repo) | `8aad054` | F-006 fault capture (38400 baud, clean) |
| `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-production-smoke.serial.log` | `8aad054` | Pre-baud-correction baseline (garbled) |
| `Plans/DISPATCH-2026-05-25-F006-cerebro-diagnosis.md` | `76f6586` | Cerebro diagnosis dispatch (final form) |
| `Plans/CEREBRO-OUTPUT-2026-05-25-F006-diagnosis.md` | `c6dc59a` | Cerebro return: option D recommendation |
| `Plans/PM-DISPOSITION-2026-05-25-F006-cerebro-diagnosis-close.md` | `d53c7c2` | PM accept + open-dep resolutions |
| `Plans/DISPATCH-2026-05-25-F006-jarvis-implementation.md` | `4a8cc5c` | Jarvis implementation brief |

### 9.3 Hardware procedure

- `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md` (`081f35c`) — full procedure, Appendix A hardware inventory.
- Hardware: Dell OptiPlex 7080 SFF, i5-10500T 6c/12t, BIOS 1.37.0. Serial chain: 6CC5P internal cable → DB9 bracket → Arnorin null modem → DTECH FTDI USB-RS232 → Windows COM3 / PuTTY / **38400 8N1**.
- ISO: `boot/neoplutarch.iso` (regenerate per build).

### 9.4 Bible / ADR carry-forward constraints (operative for F-006 fix)

- **Bible 4.2:** `unsafe` permitted at hardware boundary (page tables count). All `unsafe` blocks need SAFETY comments naming the invariant. CI fails on missing SAFETY.
- **Bible 7.2:** kernel upper-half / userspace lower-half split, enforced by page tables.
- **Bible 13:** phase-close discipline. Phase 2 close requires real boot verification on QEMU and ≥1 bare-metal target. F-006 blocks the bare-metal verification.
- **ADR-016:** scheduler ring / heap decoupling. Repair helper must use local boot frame allocator, NOT `FRAME_ALLOC`, NOT under scheduler context.
- **ADR-019:** IRQ-safe migration. F-006 fix runs pre-AP-bringup, pre-scheduler-start, so ADR-019 isn't directly invoked, but the discipline (no IRQ-unsafe operations once the scheduler is live) applies.

### 9.5 External references Cerebro cited

- Limine protocol spec: `https://github.com/limine-bootloader/limine-protocol/blob/trunk/PROTOCOL.md`
  - Lines 302-304: HHDM offset application.
  - Lines 383-388, 459-466: base-revision-3 HHDM coverage.
  - Lines 1114-1121, 1129-1137: memmap type definitions + executable/modules caveat.
  - Lines 1143-1146: sorted/alignment/overlap limits.

---

## 10. Action items table

| # | Action | Trigger | Owner | Status |
|---|---|---|---|---|
| 1 | Wait for Jarvis F-006 implementation | dispatch `4a8cc5c` filed | Jarvis | IN FLIGHT |
| 2 | PM consume Jarvis output (commit msgs + diff) | Jarvis returns | PM | PENDING-2 |
| 3 | PM file acceptance disposition | (2) passes review | PM | PENDING-3 |
| 4 | PL bare-metal re-verification (≥10 boots) | (3) filed | PL | PENDING-4 |
| 5 | PM file F-006 close report | (4) passes | PM | PENDING-5 |
| 6 | PM file Phase 2 bare-metal verification report | (5) filed | PM | PENDING-6 |
| 7 | PM file Phase 2 formal close report | (6) filed | PM | PENDING-7 |
| 8 | PM patch PROCEDURE §3.3 baud rate (115200 → 38400) | F-006 close done | PM | DEFERRED |
| 9 | Resume Phase 2 hardware scenario sweep (PROCEDURE §6.2+) | (7) filed | PL+PM | DEFERRED |
| 10 | NVMe FLUSH durability diagnosis | Phase 3 start | PM dispatch | PAUSED |
| 11 | ALTIMIT research triage queue items | next planning session | PM | DEFERRED |

---

## 11. If you must reduce this handoff to one paragraph

F-006 (bare-metal kernel HHDM coverage falls short of where Limine places the neo-init module, causing intermittent PAGE_FAULT at ELF-parse time) blocks Phase 2 formal close. Jarvis is currently implementing the fix per Cerebro option D (actual-span HHDM repair and assertion driven by Limine executable + module responses) under dispatch `4a8cc5c`. When Jarvis lands the code, verify the constraints (Bible 4.2 narrow unsafe, ADR-016 local boot frame allocator, pre-AP-bringup context, forced-unmap cfg gate), file acceptance, then dispatch PL for ≥10-consecutive-boot bare-metal re-verification on the OptiPlex 7080 (38400 baud serial, evidence path `Plans/evidence/phase2-bare-metal-optiplex-7080-*-f006-reverify-boot<N>.serial.log`). Single successful boot is not acceptance; placement-intermittence per NOTES §11 means the floor is 10.

---

**End of handoff.**

*HEAD at write time: `4a8cc5c`. Jarvis dispatched 16:42 local 2026-05-25, 4-hour soft bound.*
