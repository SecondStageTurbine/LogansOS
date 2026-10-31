# Dispatch: PHASE-2.md status update — roll forward through BUILD-3 sub-commit 1a

**To:** Jarvis (coder 2)
**From:** PM
**Date:** 2026-05-06
**Parent commit:** `305f4d9` (currently HEAD on `main`)
**Type:** Doc-only. Single file. Single commit on `main`.
**Estimated surface:** ~150–250 lines net added to `docs/phases/PHASE-2.md`. No code changes.
**Parallel work note:** Cerebro (coder 1) is concurrently dispatched on BUILD-3 sub-commit 1b (kernel code). 1b touches `kernel/src/{main.rs,arch/x86_64/smp.rs,arch/x86_64/mod.rs,arch/x86_64/per_cpu.rs}`, `kernel/Cargo.toml` (maybe), and `tools/xtask/src/main.rs`. **No file overlap with this dispatch.**

---

## 0. Standing instructions (read in full before starting)

1. **Commit on `main`, not detached HEAD.** Run `git status` and `git branch --show-current` before your first commit. The detached-HEAD landing of ANSPEC-002 last session was a recovery scenario; let's not repeat it.

2. **Run `git pull --ff-only origin main`** before starting. Cerebro's 1b commit may land while you're working; if so, rebase your work in progress and proceed. If 1b lands AFTER you commit, your 1a-perspective entry is still correct (you're documenting the 1a close, not 1b). PM will handle a follow-on PHASE-2.md entry for 1b after both close.

3. **Doc-only.** No `.rs` changes. No xtask changes. No ADR changes. If you find something in PHASE-2.md that contradicts an ADR or the Bible, **note it in your close report** but do not silently fix the ADR/Bible — those are governance artifacts requiring separate amendment processes.

4. **Citations.** Where the new entries reference commit hashes, ADR numbers, or test-scenario names, those have to be verifiable. Use the patterns already in the file: `commit \`abc1234\`` for hashes, `ADR-013 §Q1` for ADR sections, `\`smp_per_cpu_scheduler_smoke\`` for test names. **Do not invent commit hashes.** Read `git log` and use the actual ones.

5. **Voice and style.** Match the existing PHASE-2.md tone — past-tense for completed work ("BUILD-1 lands the capability foundation"), present tense for what an entry asserts ("BUILD-2 is the buddy + slab allocator"), structured per-criterion blocks with PASS/FAIL markers and trace evidence. Don't introduce new section conventions.

---

## 1. What needs to be added

PHASE-2.md was last updated when BUILD-1 (capability foundation) closed. Since then, three things have happened that the file does not reflect:

- **BUILD-2 — Buddy + slab allocators.** Closed 2026-05-04 with sub-commits 1–4 plus a post-close summary in ADR-012. Last commit `f5a4984`.
- **BUILD-3 sub-commit 0 — ADR-013 + ADR-011 supersede + GLOBAL fixup.** Closed 2026-05-04. Commits `fe5bc3d` (initial), `b4b3ac9` (remediation), `57fd235` (lint cleanup).
- **BUILD-3 sub-commit 1a — Per-CPU infrastructure refactor.** Closed 2026-05-05 at `305f4d9`. APs do not exist yet; that's sub-commit 1b (in flight, not in scope for this update).

You are adding three new sections to PHASE-2.md, plus updating the header table. The file's existing BUILD-1 section is correct and stays; you're appending after it.

---

## 2. Section-by-section guidance

### 2.1 Header table

Currently reads:

```
| **Status** | In progress |
| **Started** | 2026-05-04 |
| **Closed** | - |
| **ADRs landed** | 010 (capability system foundation); 011 amended |
| **Commits** | BUILD-1 in this commit |
| **Next build** | BUILD-2 design conversation: allocator or SMP sequencing, lead call |
```

Update to reflect current state through BUILD-3 sub-commit 1a:

```
| **Status** | In progress (BUILD-3 active; sub-commit 1a closed, sub-commit 1b in flight) |
| **Started** | 2026-05-04 |
| **Closed** | - |
| **ADRs landed** | 010 (capability system foundation); 011 amended (then superseded by 013); 012 (buddy + slab allocator foundation); 013 (Phase 2 SMP retrofit, supersedes 011) |
| **Commits** | BUILD-1: see below; BUILD-2: see below; BUILD-3 sub-commits 0+1a: see below |
| **Next build** | BUILD-3 sub-commit 1b: AP bring-up via Limine MP request |
```

Match the existing table syntax exactly (single pipe + leading/trailing whitespace as in the original).

### 2.2 New section: `## BUILD-2 — Buddy + Slab Allocators`

Append after the existing `## Course Corrections` block at end of BUILD-1 — but before the existing `## Verification Log` section. Or append at the document's logical end and add cross-references; structure is whatever reads best alongside the existing BUILD-1 section. Read the file end-to-end before deciding section placement.

The BUILD-2 section needs:

- One-paragraph summary of what landed.
- **Sub-commit 1 — Buddy frame allocator.** Commit `[hash]`. ADR-012 §Q1 + §Q2. Buddy allocator over Phase 1's bump+free-list. Two new test scenarios: `buddy_split_coalesce`, `buddy_max_order`, `buddy_alloc_free_alloc_returns_same_frame`, plus `expect_pass_but_alloc_fail` deliberate-failure scenario.
- **Sub-commit 2 — Slab allocator infrastructure.** Commit `[hash]`. ADR-012 §Q3. `SlabCache<T>` trait, `Slab<T>`, `Magazine<T>`, `GlobalCache<T>`, `SlabPerCpuState` shape with per-CPU + global cache pattern. `THREAD_CACHE`, `PROCESS_CACHE`, `CHANNEL_CACHE` declared static. New scenarios: `slab_local_to_global_promotion`, `slab_global_to_buddy_overflow`.
- **Sub-commit 3 — Channel migration to slab (REVERTED).** Commit `[hash]` for the revert. The Channel cache migration was reverted because `Arc::from(Box<T>)` routes Drop to the global allocator rather than the slab's free path. Test `slab_channel_migration` retained in `tools/xtask/src/main.rs` as `ignored: true` regression pin — when Arc-aware slab integration lands in a future BUILD, flip to `ignored: false` and it should PASS.
- **Sub-commit 4 — Thread migration to slab.** Commit `f5a4984`. ADR-012 §Q3. Thread allocation on `THREAD_CACHE.alloc()`, free via the cache. `slab_thread_migration` scenario. Thread is unaffected by the Arc-final-owner pattern because Thread is not Arc-shared (only one scheduler queue holds it at any time, modulo the rendezvous-block pattern).
- Course corrections: ADR-012 BUILD-2 post-close summary records the Arc-vs-slab finding plus the three known paths (custom Drop, `Arc::new_in()` with custom allocator, refactor away from Arc final-owner). Reference it.
- Verification Log entry for BUILD-2: `cargo k` green, all default `slab_*` tests PASS (excluding `slab_channel_migration` which is ignored), Phase 1.5b/1.5c anti-regression PASS, frame leak audit clean.
- Anti-criteria: Arc-aware slab integration deferred to future BUILD. Per-CPU sharding deferred to BUILD-3 (when actual SMP lands).

To get the actual commit hashes, run:

```
cd "E:\AI Automation\AltimitNeo"
git log --oneline f5a4984~5..f5a4984
```

Use the literal hashes that returns. Don't invent.

For the `slab_channel_migration` ignored state, verify by reading `tools/xtask/src/main.rs` directly — find the scenario's `ignored:` field and confirm it's `true`. Quote the literal source comment block in the entry; the existing comment is the canonical explanation.

### 2.3 New section: `## BUILD-3 sub-commit 0 — ADR-013 + ADR-011 Supersede + GLOBAL Fixup`

The first BUILD-3 piece. Three commits because of one remediation cycle and one lint cleanup pass.

- **Initial commit `fe5bc3d`.** ADR-013 lands at `docs/ADR-013-phase-2-smp-retrofit.md` with all nine load-bearing decisions (Q1–Q9). ADR-011 acquires `Superseded by: ADR-013` metadata. Post-boot fixup walks PML4 entries 256-511, sets the GLOBAL bit on kernel-half leaf entries.
- **Remediation commit `b4b3ac9`.** First boot revealed the `paging::sanity_check` was running pre-fixup (so its `global=true` claim was always false against reality). Remediation: drop the pre-fixup sanity check; capture the actual real serial output post-fixup. Specifically observed: `paging: sanity OK - kernel .text sample virt VirtAddr(0x...) -> phys PhysAddr(0x...) (global=true)` at a KASLR-relocated address.
- **Lint cleanup commit `57fd235`.** `cargo k` warnings: explicit `unsafe` blocks added per Rust 2024 `unsafe_op_in_unsafe_fn`, `unused mut` removed in GLOBAL fixup. Final warning count 22 at this commit (PM-verified by clean-tree `cargo k`).
- Verification: post-fixup `paging::sanity_check` reports `global=true` on kernel `.text` sample. CR4.PGE explicitly enabled (Limine did not set it). Frame leak audit clean. All BUILD-2 anti-regression tests PASS.

Reference: ADR-013 itself, ADR-011 (now superseded), the GLOBAL fixup discussion in ADR-013 §Q7.a.

### 2.4 New section: `## BUILD-3 sub-commit 1a — Per-CPU Infrastructure Refactor`

The most substantial piece since BUILD-2 close. Cerebro landed it 2026-05-05.

Content per the actual close record (which you can recover from the handoff at `Plans/HANDOFF-2026-05-05-build3-sub-commit-1a-close.md` §2 and §3):

- Commit `305f4d9`. ADR-013 §Q1 + §Q2 + Amendment 1 + Amendment 2.
- `MAX_CPUS = 64` constant; `PER_CPU` becomes `[PerCpu; 64]` array. New fields: `cpu_id`, `active_pmap_cr3`, `scheduler_ptr`, `tss_ptr`.
- Per-AP GDT (Option A from ADR-013 Amendment 1): `static GDTS: [Once<PerCpuGdt>; MAX_CPUS]`.
- Per-AP TSS + IST stacks: **static storage**, not heap-backed. This is a deviation from the original dispatch (which assumed heap was up by trap-init time); the actual boot order has `init_traps()` at line 233 and `mm::init()` at line 288 of `kernel/src/main.rs`. Coder 1 caught the deviation, used `static IST_STACKS: [PerCpuIstStacks; MAX_CPUS]` and `static TSS_STORAGE: [TssSlot; MAX_CPUS]` with `MaybeUninit` and lazy `initialize_per_cpu_tss(cpu_id)`, and recorded the correction in ADR-013 Amendment 1's "Implementation correction" subsection.
- `init_traps` splits into `init_traps_bsp()` + `init_traps_ap(cpu_id)`. `per_cpu::init` splits into `init_bsp()` + `init_ap(cpu_id)`. `syscall::init` splits into `init_bsp()` + `init_ap()`. All four MSRs (EFER, STAR, LSTAR, SFMASK) per-CPU.
- `idt::load_for_ap()` — IDT shared (exception handlers stateless, IST indices resolve via current TSS); each AP loads but doesn't init.
- `gdt::set_kernel_rsp()` writes via `current_per_cpu().tss_ptr` with `debug_assert!` null-check.
- `mm::slab::init_per_cpu(cpu_id)` exposed. Called from `ap_entry` in sub-commit 1b.
- Compatibility shim `pub fn init_traps()` retained as alias for `init_traps_bsp`; `main.rs` unchanged in 1a.
- 7 files modified: `kernel/src/arch/x86_64/{gdt,idt,mod,per_cpu,syscall}.rs`, `kernel/src/mm/slab.rs`, `docs/ADR-013-phase-2-smp-retrofit.md`. Net ~600 lines (395 insertions, 324 deletions per `git diff --stat 2f6d969..305f4d9`).
- **APs do not exist in 1a.** No Limine MP request. No `ap_entry`. Single-CPU boot only.
- Verification: `cargo k` warning count 18 at `305f4d9` (down from 22 at `57fd235`); four warnings retired by `#[allow(dead_code)]` on asm-load-bearing offset constants (`KERNEL_RSP_OFFSET`, `USER_RSP_SAVE_OFFSET`, `SLAB_STATE_OFFSET`, `star_user_base()`). Constants remain in source as documentation of asm layout invariant.
- Boot log literals from `boot-log-1a.txt` (file currently in project root, untracked):
  - `[ kernel ] gdt: cpu=0 loaded (tss_sel=0x30)`
  - `[ kernel ] per_cpu: PER_CPU[0] at 0xffffffffa6ccfd40 → IA32_GS_BASE (BSP)`
  - `[ kernel ] paging: GLOBAL fixup complete - 4K leaves: 4575, 2M leaves: 251, 1G leaves: 0`
  - `[ kernel ] paging: sanity OK - kernel .text sample virt VirtAddr(0xffffffffa6c22ab0) -> phys PhysAddr(0x1f1f3ab0) (global=true)`
  - `[ kernel ] frame: leak audit clean (baseline=1026 final=1026)`
  - `[ kernel ] phase 0 boot complete`
  - `[ kernel ] no scheduler yet — halting forever`
- 4K leaf count 3626 → 4575: 64 statically-allocated IST stack arrays of 20 KiB each. Frame leak audit clean confirms static memory isn't tracked by frame allocator (correct — it's reserved at link time).
- 23/23 default scenarios PASS reported by coder. `expect_pass_but_fail` correctly detected by meta-test.

Course corrections record the boot-order deviation. The handoff §3 has the canonical narrative: name the wrong assumption (dispatch claimed `mm::init` precedes `init_traps`, file says reverse), name what was actually true (boot order verified at lines 233/270/288/293), give forward implication (future BUILDs needing heap-backed per-CPU state must change boot order first).

### 2.5 Update the existing `## BUILD-2 Transition Notes` section

The existing section reads like a forward-looking note from when BUILD-2 design was about to start. It should now say "Both BUILD-2 (allocators) and BUILD-3 (SMP) are in flight; design substrate is ADR-012 + ADR-013." Edit it minimally — don't rewrite the whole thing — to reflect that the design conversation it anticipated has happened. Either prepend a short "Update: this transition is complete; see BUILD-2 and BUILD-3 sections below." line, or reword the section name to "## Transition from BUILD-1" (past tense) and tighten the prose.

### 2.6 The forward-reference section

The existing `## Forward reference: async discipline for Phase 8+ libneo` section at the very end is fine as-is. Don't touch it.

---

## 3. What you do NOT do

- Don't update the Bible. v1.0.3 is correct.
- Don't update ADR-013 or ADR-012 or any other ADR. Their amendment-subsection patterns belong with the work that motivated them, not with PHASE-2.md.
- Don't update ANSPEC-001 or ANSPEC-002.
- Don't add a section for sub-commit 1b. That's in flight; PM will handle a follow-on PHASE-2.md entry after Cerebro's 1b close report is verified.
- Don't fix unrelated formatting drift in the existing BUILD-1 section. If you spot something, mention it in your close report.

---

## 4. Pre-implementation OBSERVE pass

Before writing, read:

1. `docs/phases/PHASE-2.md` — the file you're editing. Read the whole thing. Note the existing prose style, the "Course Corrections" pattern, the per-criterion verification block format with `✓ PASS` markers and trace evidence (this is the `PHASE-1.5b.md` style; PHASE-2.md is similar).
2. `docs/phases/PHASE-1.5b.md` — for style reference if PHASE-2.md is unclear on any point.
3. `docs/ADR-012-buddy-slab-allocator-foundation.md` — to ground the BUILD-2 section's references. Read the BUILD-2 amendment subsection in particular; it has the post-close summary findings on Arc-vs-slab.
4. `docs/ADR-013-phase-2-smp-retrofit.md` Amendments 1 and 2 — the canonical sources for sub-commit 1a's deviation and the 1a/1b split rationale.
5. `Plans/HANDOFF-2026-05-05-build3-sub-commit-1a-close.md` — §1 (current git state), §2 (what 1a delivered), §3 (deviation), §4 (incidents). The §2 and §3 narratives are exactly what your sub-commit 1a section in PHASE-2.md is summarizing; don't paraphrase from memory.
6. `tools/xtask/src/main.rs` — for the `slab_channel_migration` ignored-state confirmation and the BUILD-2 test scenario list.
7. `git log --oneline f5a4984..305f4d9` — the actual commit graph through current HEAD. Quote literal hashes.

If anything contradicts this dispatch, **flag it in your close report** and either deviate (with a one-line note in your section) or surface to PM before continuing.

---

## 5. Verification gates (what the close report must show)

### 5.1 Doc-only confirmation

```
git diff --stat <parent>..HEAD
```

Expected: exactly one file changed (`docs/phases/PHASE-2.md`). If anything else shows up, that's a bug.

### 5.2 Markdown sanity

PHASE-2.md should still render as valid Markdown. The existing file uses GFM tables, fenced code blocks, and standard heading levels. Preserve those.

If you have a Markdown linter handy, run it; otherwise just confirm by-eye that:

- The header table at the top still renders.
- Section heading levels are consistent with existing usage (H2 for major BUILD sections, H3 for sub-commits within them).
- No broken links.

### 5.3 Internal consistency

Cross-references to ADRs should match actual ADR numbers (010, 011, 012, 013). Cross-references to commits should match actual commit hashes from `git log`. Cross-references to test scenarios should match actual scenario names from `tools/xtask/src/main.rs`'s `TEST_SCENARIOS` array.

### 5.4 No regression in existing content

The existing BUILD-1 section should be unchanged byte-for-byte (modulo if you rename the "BUILD-2 Transition Notes" section as suggested in §2.5, which is a tiny edit). Run `git diff` and confirm.

### 5.5 Commit shape

Single commit on `main`. Suggested message:

```
docs: PHASE-2.md status update through BUILD-3 sub-commit 1a

Adds entries for BUILD-2 (buddy + slab allocators) and BUILD-3 sub-commits 0
and 1a (ADR-013 + GLOBAL fixup, per-CPU infrastructure refactor). Updates the
header table to reflect current ADRs and commits. The existing BUILD-1
section is unchanged; the prior "BUILD-2 Transition Notes" section is
retitled to past tense ("Transition from BUILD-1") since the work it
anticipated has happened.

No code changes. No ADR or Bible amendments. Sub-commit 1b is in flight
and not covered by this update.
```

---

## 6. If you spot inconsistencies or stale claims

Note them in your close report under "Observations" so PM can triage:

- ANSPEC-001 §15 reportedly references "Bible v1.0.0" but the Bible is now v1.0.3 (per the handoff §7 triage queue). One-line correction; could be its own follow-on dispatch.
- ANSPEC-001 §6.3.4 is a flagged provenance investigation item from the §11.2 work. Not your problem here; surface.
- Anything in the existing PHASE-2.md BUILD-1 section that contradicts current ADR-010 or ADR-013 wording. Surface; don't fix.

---

## 7. If something blocks you

If `git log` shows a commit graph different from what this dispatch implies (e.g., a commit between BUILD-2 and BUILD-3 sub-commit 0 you weren't expecting), or if any quoted boot-log literal disagrees with `boot-log-1a.txt` in the project root, **stop and report**. The discipline rules from sub-commit 1a apply: name the wrong assumption, name what's actually true, propose forward.

---

## 8. Why this matters

PHASE-2.md is the canonical phase-status doc. Anyone joining the project needs to be able to read it and understand the state of Phase 2. Right now it tells the BUILD-1 story and stops. Three weeks of substantial work isn't reflected. This dispatch closes that gap through 1a; PM will do the 1b follow-on.

This is documentation hygiene, not architectural work. Get it right, get it concise, get it cited. Standard close report when done.

---

**End of dispatch.**
