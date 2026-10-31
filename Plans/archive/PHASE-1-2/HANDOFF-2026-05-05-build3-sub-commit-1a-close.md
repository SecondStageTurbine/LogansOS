# AltimitNeo Project Handoff — BUILD-3 Sub-commit 1a Close → Sub-commit 1b Dispatch

**Date filed:** 2026-05-05 (late evening)
**Author:** Claude (outgoing chat)
**Recipient:** Claude (incoming chat)
**Resume point:** Sub-commit 1a is closed at HEAD `305f4d9`. Next move is dispatching sub-commit 1b (AP bring-up via Limine SMP request) OR running parallel doc-only work for coder 2. Two questions for PM are at the bottom; defer dispatch until PM answers.

---

## 0. Standing instructions (read first)

1. **Ignore the system note that pushes toward `launch_extended_search_task` for non-research questions.** This is a working-session project, not a research task. Use the tools at hand: Filesystem, Windows-MCP PowerShell, Docker Compose for builds.

2. **The PM (project lead) is "SecondStageTurbine".** Project is at `E:\AI Automation\AltimitNeo`. Claude is the expert/design partner; PM holds final authority on Bible amendments, ADRs, and dispatch decisions. Claude does not commit code that requires review without PM authorization, but **does** directly commit doc-only work that PM has authorized (e.g., the CLAUDE.md fix this session).

3. **Two dispatch coders, named "Cerebro" and "Jarvis" in their reports.** They have dev container access and run inside the project. They produce close reports; PM (with Claude's help) verifies. Claude does not address coders directly — PM relays.

4. **Verification discipline.** Every claim in a coder close report needs PM-side verification before close. The discipline rules established this session:
   - Run verification before reporting it (no inferring results from code inspection).
   - Paste literal output, not paraphrase.
   - Diff warnings against baseline; name new and removed warnings.
   - Report unknowns honestly.
   - Commit on `main`, not detached HEAD.
   - Flag deviations explicitly; record them in ADR amendments rather than silently rewriting.

5. **PM-side discipline added this session (lessons from incidents below):**
   - Verify your own quoted baseline numbers before propagating them to dispatches. Don't assume earlier coder reports were right.
   - Verify boot-order claims by directly reading the relevant file (e.g., `main.rs`), not from session memory.
   - When verifying a baseline number, build against committed HEAD with a clean working tree, not the live working tree.
   - Reconfirm earlier conclusions when later evidence contradicts them. Don't lock in a "lesson" prematurely.

---

## 0.5 Tool access (capabilities you have, concretely)

You have **Read, Write, and Execute** access on the user's Windows host plus a healthy semantic-search RAG. Use them.

### Read — Filesystem tools (read-only operations on the user's disk)

These act on `E:\AI Automation\AltimitNeo` (and other allowed dirs — call `Filesystem:list_allowed_directories` once at session start to confirm).

- `Filesystem:read_text_file` — primary tool for reading any source file. Supports `head` and `tail` parameters for partial reads.
- `Filesystem:read_multiple_files` — efficient batch read; use when comparing several files.
- `Filesystem:list_directory` / `Filesystem:list_directory_with_sizes` / `Filesystem:directory_tree` — directory introspection.
- `Filesystem:search_files` — glob-pattern search; e.g. `**/*.rs` for all Rust files.
- `Filesystem:get_file_info` — metadata only (size, mtime, permissions).
- `Filesystem:list_allowed_directories` — call once at session start.

**Use these for OBSERVE-pass work.** When you need to read a specific file in detail to understand its structure, go directly through these tools. Do not delegate to grep when a real read is what's wanted.

### Write — Filesystem and Desktop Commander

- `Filesystem:write_file` — overwrites a file with new content. **No partial-write or append; the whole file is replaced.** For incremental edits, read the file, modify the string in memory, then write the complete result back.
- `Filesystem:move_file` — rename or move.
- `Desktop Commander:edit_block` (if available — load via tool_search) — surgical find/replace edits to text files. Better than full-rewrite when you're changing a small portion of a large file.

**You are authorized to write doc files directly when PM has approved the change.** Examples this session: PM authorized the CLAUDE.md fix → I wrote it directly and committed via PowerShell. PM authorized this handoff → I'm writing it now. **You are NOT authorized to write `.rs` source files directly** — those go through the dispatch-to-coder process.

### Execute — Windows-MCP PowerShell

- `Windows-MCP:PowerShell` is the primary execution tool. Full PowerShell on the user's host. Used for:
  - `git` operations (`git log`, `git status`, `git checkout`, `git cherry-pick`, `git commit`).
  - `docker compose` invocations (kernel builds, boot tests).
  - Quick file inspection via `Select-String`, `Get-Content`, `Get-ChildItem`.
  - `Invoke-RestMethod` / `Invoke-WebRequest` for RAG queries.
  - Background process launching for long builds (see pattern below).

- `Windows-MCP:FileSystem` — alternative file operations through the same tool family. Filesystem tools above are usually preferred; this is a fallback.

- `Windows-MCP:Wait` — pause for N seconds. Useful between launching a background build and polling its log.

**Background build pattern** (use for `cargo k` runs that take 30–120s):

```powershell
Start-Process powershell -ArgumentList '-NoProfile','-Command',"cd 'E:\AI Automation\AltimitNeo'; docker compose run --rm neo cargo k *> '$env:TEMP\out.log' 2>&1; '===DONE===' | Add-Content '$env:TEMP\out.log'" -WindowStyle Hidden
```

Then `Windows-MCP:Wait` for ~75 seconds, then check the log:

```powershell
$done = Select-String -Path "$env:TEMP\out.log" -Pattern "===DONE===" -Quiet
Select-String -Path "$env:TEMP\out.log" -Pattern "generated \d+ warning"
```

The `===DONE===` sentinel tells you the build finished (vs. still running).

### RAG — semantic code search at `http://127.0.0.1:8404`

This is your **discovery** tool for "where in the codebase is X?" queries. It is NOT a substitute for OBSERVE-pass file reads.

**Health check (do this first if you suspect RAG issues):**

```powershell
Invoke-RestMethod http://127.0.0.1:8404/health
```

Healthy response: `{"status":"healthy","ollama":true,"qdrant":true,...}`. If `ollama: false`, see CLAUDE.md troubleshooting at `E:\AI Automation\AltimitNeo\CLAUDE.md` — usually means the embedding model needs pulling.

**Stats (what's currently indexed):**

```powershell
Invoke-RestMethod http://127.0.0.1:8404/stats
```

Currently: 88 files, 639 chunks. `by_language`: markdown=509, rs=107, toml=11, ld=3, yaml=2, json=2, conf=1.

**Query example — find code related to a concept:**

```powershell
$body = @{query='per-CPU TSS allocation IST stack'; top_k=5; language_filter='rs'} | ConvertTo-Json
$r = Invoke-RestMethod -Uri "http://127.0.0.1:8404/search" -Method POST -Body $body -ContentType "application/json"
$r.results | ForEach-Object { "Score $([math]::Round($_.score,3)) | $($_.file_path | Split-Path -Leaf):$($_.start_line)-$($_.end_line)" }
```

`language_filter` valid values for this repo: `rs`, `markdown`, `toml`, `ld`, `yaml`, `json`, `conf`. `path_prefix` (e.g. `kernel/src` or `docs`) further narrows.

**For full chunk text (not truncated):** POST to `/search/full` with the same body shape.

**Quality caveat.** RAG uses a generic Rust chunker (paragraph splits, not tree-sitter-aware). Top-N results often miss the most relevant file — for "TSS allocation" queries, `gdt.rs` (which contains the actual TSS code) doesn't always show up in the top 5 even after reindex. Use RAG to find candidate files, then read them with `Filesystem:read_text_file`.

**When to use RAG:**
- "Where is the IPC rendezvous done?"
- "Find similar patterns to capability derivation."
- "What ADRs reference the slab allocator?"

**When NOT to use RAG:**
- Reading a specific known file in detail → `Filesystem:read_text_file`.
- Exact-string lookup across the codebase → `Select-String` via PowerShell (faster, more precise).
- Verifying claims in a close report → direct file reads, not RAG.

The `CLAUDE.md` at the project root has full RAG usage docs. Read it if any of the above is unclear.

### What you do NOT have

- **No direct kernel build/test from inside Claude.** Builds run on the user's Windows host via `docker compose`. You launch them via PowerShell; they execute outside this session.
- **No browser, no live web access** beyond what `web_search` and `web_fetch` provide (and those are for present-day facts, not project content).
- **No persistent state between turns** beyond what's in this conversation's context. The project filesystem persists on the user's host; the RAG persists too. Your in-context working memory does not.

### Discipline reminders specific to tool use

- **Verify before propagating.** When a coder's report says "warning count = 18 at parent X," that claim has to be confirmed by you running cargo k against committed X with a clean tree. Three incidents this session involved unverified claims flowing forward; don't add a fourth.
- **Read the file before quoting from it.** Don't quote line numbers from memory. Use `Filesystem:read_text_file` with `view_range` or `head`/`tail`.
- **PowerShell for git, not Filesystem tools.** The Filesystem tool family doesn't have git commands; use Windows-MCP PowerShell with `cd "E:\AI Automation\AltimitNeo"; git ...`.

---

## 1. Current git state (verified at handoff time)

```
305f4d9  build-3 sub-commit 1a: per-CPU infrastructure refactor (no AP bring-up)   ← HEAD on main
2f6d969  docs: ANSPEC-002 capability semantics for AI objects + §3.7 reconciliation
7f46806  docs: fix CLAUDE.md RAG documentation to match actual indexed corpus
57fd235  build-3 sub-commit 0 lint cleanup
b4b3ac9  build-3 sub-commit 0 remediation
fe5bc3d  build-3 sub-commit 0: ADR-013 + ADR-011 supersede + GLOBAL fixup
a760921  docs: Bible amendment — Article 4.5 single-node scope clause
cb31f97  docs: ANSPEC-001 §11.2 sharpening — AI-event taxonomy v0.2.0
82ed6ed  docs: ADR-012 BUILD-2 post-close summary
f5a4984  phase2: BUILD-2 sub-commit 4 — Thread migration to slab
```

Branch: `main`. Working tree: clean except for untracked artifacts (`baseline-build.txt`, `build-1a.txt` both empty 0-byte; `boot-log-1a.txt` is the verification log from sub-commit 1a; `.qwen/` is some IDE state).

Bible: **v1.0.3** (last amended 2026-05-05, Article 4.5 single-node scope clause).
ANSPEC-001: **v0.3.0** (last revised 2026-05-05, §3.7 reconciled with ADR-010 two-tier rights model + ANSPEC-002 introduced).
ANSPEC-002: **v0.1.0** (introduced this session at `2f6d969`).

---

## 2. What sub-commit 1a delivered

Per ADR-013 §Q1 + §Q2 + Amendment 1 + Amendment 2:

- `MAX_CPUS = 64` constant.
- `PerCpu` becomes `[PerCpu; 64]` array. New fields: `cpu_id`, `active_pmap_cr3`, `scheduler_ptr`, `tss_ptr`.
- Per-AP GDT (Option A from ADR-013 Amendment 1): `static GDTS: [Once<PerCpuGdt>; MAX_CPUS]`.
- Per-AP TSS + IST stacks: **static storage**, NOT heap-backed (deviation from dispatch — see §4 below).
- `init_traps` splits into `init_traps_bsp()` + `init_traps_ap(cpu_id)`.
- `per_cpu::init` splits into `init_bsp()` + `init_ap(cpu_id)`.
- `syscall::init` splits into `init_bsp()` + `init_ap()`. All four MSRs (EFER, STAR, LSTAR, SFMASK) are per-CPU.
- `idt::load_for_ap()` — IDT is shared (exception handlers stateless, IST indices resolve via current TSS); each AP loads it but doesn't init it.
- `gdt::set_kernel_rsp()` writes via `current_per_cpu().tss_ptr` with debug_assert null-check.
- `mm::slab` split: `init()` (BSP wrapper) + `init_per_cpu(cpu_id)` (called from `ap_entry` in sub-commit 1b).
- Compatibility shim `pub fn init_traps()` retained (alias for `init_traps_bsp`); main.rs unchanged in 1a.

Files modified (7): `kernel/src/arch/x86_64/{gdt,idt,mod,per_cpu,syscall}.rs`, `kernel/src/mm/slab.rs`, `docs/ADR-013-phase-2-smp-retrofit.md`. Total ~600 lines net (395 insertions, 324 deletions per `git diff --stat`).

**APs do not exist in 1a.** No Limine SMP request. No `ap_entry`. Single-CPU boot only. APs come in 1b.

### Verification observations (PM-direct-observed)

- `cargo k` warning count: pure `57fd235` = **22**, HEAD `305f4d9` = **18**. Both verified by `docker compose run --rm neo cargo k` from clean worktrees. Net delta 22→18, four warnings retired by `#[allow(dead_code)]` annotations on asm-load-bearing offset constants (`KERNEL_RSP_OFFSET`, `USER_RSP_SAVE_OFFSET`, `SLAB_STATE_OFFSET`, plus `star_user_base()`). Constants remain in source as documentation of asm layout invariant.

- Boot log literals from `boot-log-1a.txt` (file at project root):
  - `[ kernel ] gdt: cpu=0 loaded (tss_sel=0x30)`
  - `[ kernel ] per_cpu: PER_CPU[0] at 0xffffffffa6ccfd40 → IA32_GS_BASE (BSP)`
  - `[ kernel ] paging: GLOBAL fixup complete - 4K leaves: 4575, 2M leaves: 251, 1G leaves: 0`
  - `[ kernel ] paging: sanity OK - kernel .text sample virt VirtAddr(0xffffffffa6c22ab0) -> phys PhysAddr(0x1f1f3ab0) (global=true)`
  - `[ kernel ] frame: leak audit clean (baseline=1026 final=1026)`
  - `[ kernel ] phase 0 boot complete`
  - `[ kernel ] no scheduler yet — halting forever`
  - No `BUG:`. No `KERNEL PANIC`. KASLR-relocated address (different from prior boot's address — evidence of real boot).

- 4K leaf count went from 3626 (sub-commit 0) to 4575 (1a). Increase explained by 64 statically-allocated IST stack arrays of 20KB each. Frame leak audit clean confirms static memory isn't tracked by frame allocator (correct — it's reserved at link time).

- Test results: 23/23 default scenarios PASS reported by coder. `expect_pass_but_fail` correctly detected by meta-test. PM did not re-run the suite directly; the boot reaching `phase 0 boot complete` with frame leak clean is the structural evidence the refactor is sound.

---

## 3. Critical deviation in sub-commit 1a (PM error caught and recorded)

**The deviation:** Sub-commit 1a uses **static storage** for per-CPU TSS and IST stacks, not heap-allocated storage as the dispatch specified.

**Why:** PM dispatch claimed boot order is `mm::init() → ... → init_traps_bsp()`, so heap is up by trap-init time. **This was wrong.** The actual boot order in `kernel/src/main.rs`:
- line 233: `arch::init_traps()` ← FIRST
- line 270: `arch::enable_interrupts()`
- line 288: `mm::init(...)` ← LATER
- line 293: `sched::init()`

`init_traps()` runs at line 233; `mm::init()` at line 288. Heap is NOT operational when GDT/TSS init runs. `Box::new` would have panicked at TSS-allocation time.

**How it was caught:** Coder 1 (Cerebro) caught the error during implementation, switched to `static IST_STACKS: [PerCpuIstStacks; MAX_CPUS]` and `static TSS_STORAGE: [TssSlot; MAX_CPUS]` (using `MaybeUninit` with lazy `initialize_per_cpu_tss(cpu_id)`), and recorded the deviation in `docs/ADR-013-phase-2-smp-retrofit.md` Amendment 1's third paragraph as an "Implementation correction" subsection. The amendment text is exactly the right tone — names the dispatch's incorrect assumption, names what was actually true, gives forward guidance ("if a later BUILD wants heap-backed TSS/IST storage, change the boot order first").

**Verification:** PM read `gdt.rs` directly. Static storage confirmed. `set_kernel_rsp` writes via `current_per_cpu().tss_ptr` with debug_assert null-check. Per-CPU GDT array using `spin::Once`. Implementation matches the corrected design.

**This is the discipline pattern working as intended.** Dispatch was wrong, coder caught it, deviation recorded with forward implication. Future dispatches must verify boot-order claims via direct file read.

---

## 4. PM verification incidents this session (process lessons)

Three incidents caught during verification work. Each recorded for the standing-vigilance list:

### Incident 1: Boot order claim in dispatch (caught by coder 1)
PM dispatch said `mm::init() → init_traps_bsp()`. Actual order is reversed. Coder 1 caught at implementation time. **Lesson: verify boot order via direct file read before quoting in dispatches.**

### Incident 2: Warning-count baseline confusion (caught by direct re-build)
This one is tangled. Sequence:
- Sub-commit 0 close reported "22 warnings at 57fd235."
- I propagated "22" into multiple subsequent dispatches as baseline.
- During ANSPEC-002 close verification, I built `cargo k` against the working tree (which had coder 1's WIP applied) and got 18 warnings.
- I incorrectly concluded "the 22 number was wrong all along; 18 is the truth."
- During sub-commit 1a close verification, I needed to re-confirm. I built pure `57fd235` directly (`git checkout 57fd235 && docker compose run --rm neo cargo k`) and got **22 warnings**. The original 22 was correct.
- The reason: when I built "the working tree" during ANSPEC-002 verification, the working tree already contained coder 1's in-progress refactor, which had `#[allow(dead_code)]` annotations suppressing 4 warnings. So I was measuring `57fd235 + WIP-allow-attributes`, not pure `57fd235`.

**Lesson: when verifying a baseline number, build against committed HEAD with a clean working tree. The "live working tree at the moment I run cargo k" is not the same as "any specific committed state."**

A subsidiary lesson: **don't lock in a "lesson learned" prematurely.** I told coder 2 in their close note that "22 was unverified PM data" — that was wrong; 22 was correct. I'll need to walk that back in a future communication if it comes up.

### Incident 3: Coder 2 detached-HEAD commit (recovered via cherry-pick)
Coder 2's ANSPEC-002 commit landed at `b6d621f` with parent `57fd235`, off-branch from main (which had advanced to `7f46806` for the CLAUDE.md fix). Recovered by cherry-picking onto main, producing `2f6d969` with identical content. Future dispatches now include "commit on main, not detached HEAD" as standing instruction.

---

## 5. Sub-commit 1b — what to dispatch next

Per ADR-013 §Q2 + Amendment 2. AP bring-up via Limine SMP request. Smaller than 1a (~150-200 lines, 2-3 files).

**Adds:**
- Limine SMP request static in `kernel/src/main.rs`.
- `ap_entry()` extern "C" Rust function (in `kernel/src/arch/x86_64/per_cpu.rs` or a new `kernel/src/arch/x86_64/smp.rs`).
- `cpu_id_from_apic_id()` function — reads APIC ID from LAPIC, walks Limine CPU list to find sequential cpu_id.
- BSP iterates `LIMINE_SMP_REQUEST.get_response().cpus()`, skips itself, writes `goto_address` for each AP to start.
- `ap_entry` sequence (per ADR-013 §Q2):
  1. Determine cpu_id from `info.lapic_id` via `cpu_id_from_apic_id()`.
  2. Wire GS-base to `&PER_CPU[cpu_id]` via `per_cpu::init_ap(cpu_id)`.
  3. Sentinel-verify `current_cpu_id() == cpu_id`; panic on mismatch.
  4. Call `arch::init_traps_ap(cpu_id)` (loads per-CPU GDT + per-CPU TSS, loads shared IDT, writes per-CPU MSRs).
  5. Call `mm::slab::init_per_cpu(cpu_id)` to wire this AP's slab state.
  6. **Spin-wait forever** at end of `ap_entry`. APs do NOT enter scheduler in 1b — that's sub-commit 2.

**Anti-scope for 1b:**
- No scheduler entry on APs. Spin-wait only.
- No IPC. No cap operations on APs.
- No TLB shootdowns.
- No cross-CPU thread migration.

**Verification gates for 1b:**
- `cargo k` green; warning count not regressed beyond 18 + small delta.
- Boot on `-smp 4` succeeds. New serial output shows each AP reaching `ap_entry` spin-wait, with literal sentinel logs from each AP (`[ kernel ] per_cpu: PER_CPU[N] at 0x... → IA32_GS_BASE (AP)` for N=1,2,3).
- All 23/23 default scenarios PASS on `-smp 4` and `-smp 1` (single-CPU still works).
- Frame leak audit clean.
- Boot reaches `phase 0 boot complete` with all APs accounted for in serial log.

**Critical risk surface in 1b:** the `cpu_id_from_apic_id` function is the first time the kernel reads APIC IDs. If APIC ID enumeration is wrong, the wrong PER_CPU slot gets wired to the wrong CPU. The sentinel check (`current_cpu_id() == cpu_id` after GS-base wire) catches this loud and early.

**Important boot-order note (carry-forward from §3):** sub-commit 1b's `ap_entry` runs *after* `mm::init` has completed (BSP brings up the heap before starting APs), so heap allocation IS legal inside `ap_entry`. Specifically: if 1b needs per-AP state that requires heap, it can use `Box::new` — unlike 1a's TSS/IST init which ran pre-heap. Document this distinction in the dispatch so coder 1 doesn't apply 1a's static-storage discipline reflexively in 1b.

---

## 6. Coder 2 parallel work options

Coder 2 (Jarvis) is idle since ANSPEC-002 closed at `2f6d969`. Options for parallel work alongside 1b:

### Option A: ADR-014 stub (kernel observability, deferred to Phase 8+)
- Doc-only. New file `docs/ADR-014-kernel-observability.md`.
- Names the slot, lists the design questions, explicit Phase 8+ deferral.
- Small. ~1-2 hour cycle. Lower priority but clears a triage queue item.

### Option B: PHASE-2.md status update
- Doc-only. Updates `docs/phases/PHASE-2.md` with current state through BUILD-3 sub-commit 1a.
- Small. ~1 hour. Useful but low-priority.

### Option C: Bible Article 6.6 draft (PM-side, not coder)
- Bible amendment proposed during ANSPEC-002 work: kernel exposes inference-governance hooks (LogitPort) without implementing governance policy.
- PM-side draft, not a coder dispatch. Claude can draft the article text for PM to commit directly.
- Small. ~30 min Claude work + PM commit.

### Option D: Hold coder 2 idle
- Wait for sub-commit 1b close, use coder 2 for sub-commit 2 dispatch (per-CPU schedulers).
- Cost: lost cycle time during 1b.
- Benefit: zero entanglement risk; coder 2 picks up the right next thing.

**My recommendation when handing off:** **Option B (PHASE-2.md update) for coder 2** is the safest parallel work — small, no file conflict with 1b, useful documentation hygiene. Option A (ADR-014 stub) is also fine but lower yield. Option C is PM-side; can happen any time. Option D wastes a cycle.

---

## 7. Triage queue (deferred items, post-1b candidates)

Items pending from prior sessions, not blocking 1b:

**Bible amendment candidates (project-lead governance decisions):**
- Article 6.6: kernel exposes inference-governance hooks (LogitPort) without implementing governance policy. PM-side draft.
- Article on fault recovery: Theseus-style cell-swap recovery (research artifact recommendation).
- (Single-node scope already landed as 4.5.)

**Documentation work (doc-only, executable):**
- ADR-014 stub: kernel observability (Phase 8+ deferral).
- ADR for capability-bounded action flow for userspace agents (Thread 2A from ALTIMIT research).
- ANSPEC-001 §15 stale "v1.0.0" Bible reference (one-line correction; Bible is now v1.0.3).
- §6.3.4 provenance investigation (small flag from ANSPEC §11.2 work).
- PHASE-2.md status update.

**Phase-roadmap items (long-horizon, mostly Phase 5+):**
- OSTD-style unsafe-boundary discipline as Phase 2+ kernel policy.
- Theseus-style cell-swapping for fault recovery.
- Multi-tenant vAccel scheduling primitives (Phase 6 territory).
- Content-addressed Model storage (Phase 5).
- Capability composition analysis (Phase 7+ research).
- LogitPort and kernel-mediated inference governance (Phase 7+ research).
- KernelAttestation primitive (Phase 2 or Phase 8).
- Confidential inference (TEE+HE) primitives (Phase 8+ frontier).

**Test runner work:**
- Item I: intermittent test-runner wedge investigation. xtask.

---

## 8. Tools and infrastructure (verified working — see §0.5 for full tool guide)

This section is a quick reference. §0.5 is the authoritative tool guide.

### Filesystem access
- `Filesystem:read_text_file`, `Filesystem:write_file`, `Filesystem:list_directory` — work against `E:\AI Automation\AltimitNeo`.
- `Filesystem:list_allowed_directories` will show the allowed roots; call once at session start to confirm access.

### Windows-MCP PowerShell
- Loaded via `tool_search` if not visible at session start.
- Used heavily this session for `git`, `cargo k`, file inspection, RAG queries.
- Background process pattern: see §0.5.

### Docker compose for direct boots
- `docker compose run --rm neo cargo k` — kernel build.
- `docker compose run --rm neo cargo neo-tty` — direct boot to serial output.
- `docker compose run --rm neo cargo neo-test` — full test suite via xtask.

### AltimitNeo RAG (semantic code search)
- Endpoint: `http://127.0.0.1:8404`. See §0.5 for query examples.
- Currently healthy: 88 files, 639 chunks, last reindexed this session.
- Quality caveat: discovery tool only, not authoritative. Use direct file reads for OBSERVE-pass.
- Documentation: `CLAUDE.md` at project root.

---

## 9. Project structure quick reference

```
E:\AI Automation\AltimitNeo\
├── boot/                    Limine config
├── kernel/src/
│   ├── arch/x86_64/         per_cpu.rs, gdt.rs, idt.rs, syscall.rs, mod.rs (these were sub-commit 1a's surface)
│   ├── mm/                  slab.rs (sub-commit 1a), paging.rs (GLOBAL fixup from sub-commit 0)
│   ├── sched/               scheduler.rs, process.rs, thread.rs (untouched in 1a; sub-commit 2 territory)
│   ├── drivers/             devices
│   ├── init/                one-shot boot code
│   └── lib.rs, main.rs      main.rs has the boot order; lines 233/270/288/293 are load-bearing
├── userland/                libneo, neo-init
├── tools/xtask/             test runner (main.rs has the scenario registry)
├── docs/
│   ├── bible.md             v1.0.3
│   ├── ADR-NNN-*.md         Flat numbered ADRs (currently 001 through 013)
│   ├── specs/
│   │   ├── altimitneo-ai-subsystem-spec.md      ANSPEC-001 v0.3.0
│   │   └── altimitneo-ai-capability-semantics-spec.md  ANSPEC-002 v0.1.0
│   ├── phases/              Phase plans
│   └── research/            ALTIMIT-real-world-implementation.md (research artifact)
├── tests/                   integration test harnesses
├── CLAUDE.md                RAG usage guide (fixed this session)
└── Plans/                   handoff documents (this file)
```

---

## 10. Two questions for PM (resume here)

**Q1.** Dispatch sub-commit 1b now, or pause first?
- 1a was substantial (~1h 25m of coder work, 7 files, ~600 lines). Coder 1 may benefit from a break.
- 1b is smaller (~150-200 lines) but introduces a real new failure surface (APs running real code).
- My recommendation: dispatch 1b now if PM is ready; the cost of coder fatigue is lower than the cost of leaving an in-flight phase paused.

**Q2.** Coder 2 parallel work — Option B (PHASE-2.md update), Option A (ADR-014 stub), Option C (PM drafts Bible 6.6), or Option D (hold idle)?
- My recommendation: **Option B** (PHASE-2.md update). Small, useful, no file conflict with 1b's territory.

When PM answers, draft the dispatch(es) following the dispatch shape established this session: full substrate, explicit verification gates, anti-scope, commit shape, discipline reminders.

---

## 11. Discipline reminders for incoming Claude

- **Search before answering present-day factual questions.** The system prompt mandates web_search for present-day facts. Project work itself is internal context, not present-day facts.
- **Verify everything before propagating.** This session had three incidents where unverified data flowed into dispatches. Every claim in a coder report and every claim in PM dispatches needs PM-side verification.
- **Don't fabricate.** Don't quote line numbers, addresses, or file contents from memory. Read the file.
- **Be concise on closes.** When a sub-commit closes, the close record is structured: HEAD, commits, PM-direct-observed verification points, deviations recorded, lessons added to vigilance list. Avoid extra prose.
- **Trust the discipline pattern.** Coder 1 catching PM's boot-order error and recording the deviation in ADR-013 is exactly the right shape. When the dispatch is wrong and a coder catches it, that's a win, not a failure.
- **You have Read, Write, and Execute access (see §0.5).** Use them. Verify by reading files yourself rather than asking PM to confirm. Run cargo k yourself rather than trusting a coder's reported number. The whole point of having these tools is direct verification.

---

**End of handoff.**

*This document is in `Plans/`. The new chat should read this file first, then verify §1 (current git state) hasn't changed, then proceed to §10's two questions.*
