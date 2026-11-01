<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — #31 OT-58 tiered gate: re-check of relay 235's findings (Section 242)

**Seat:** Dexter (DeepSeek) · **Reviewed at:** #31 head `fb036204f3152315c3d314ffe2a9ee207ab7ab4c`
(the head when queued, unchanged)
**Base:** `origin/main` = `ded50d09de795b050b3a735c6cd34451b4ad581c`; diff base (merge-base) =
`c97bb8569f65936ec323f7c810e0fdc0b28aea3a`
**Branch:** `dexter/pr31-recheck` · **Answers:** `Plans/DISPATCH-2026-11-01-recheck-242-pr31.md` §242
**Scope:** static re-check of `docs/THINK-2026-11-01-ot58-tiered-gate.md` at the head (one file, +426; no
code in the PR). No CI/QEMU run was made for this review. RAG was not used: the question is a document
re-check, and every claim was checked against the tree directly.
**Read:** `git fetch origin +pull/31/head:pr31`; `git diff origin/main...pr31`; the THINK at the head;
the xtask `TestScenario` table, `kernel/Cargo.toml`, `docs/concurrency-pitfalls.md`, and the cited pass
records. **Line refs** are to the THINK at the head.

## Verdict

| # | My relay-235 FAIL | Now | THINK section |
|---|---|---|---|
| 1 | Escaped classes the quick tier misses | **PASS** | §2.2 (:99-137), §2.2.1 (:145-156) |
| 2 | One-action landing and prefix bisect | **PASS** | §2.4 (:258-269), §2.5 (:285-295) |
| 3 | The concurrency-core path filter | **PASS** | §2.3 (:158-203), §2.3.1 (:205-225) |
| 4 | The free-tier arithmetic | **PASS** | §3 (:297-328) |
| 5 | Rule changes stated in CLAUDE.md | **PASS** | §6 (:353-377) |

**Bottom line:** all five relay-235 FAILs are resolved in the THINK at this head; none needs another
design round. Two new items should be fixed in the S2 spec before it is built (below): the prefix
bisect has no input to carry the failing scenario names, and the kill-switch equivalence regression
that revision 1 had was dropped.

## 1. Escaped classes — **PASS**

§2.2 now lists `swapgs_irq_ring3` (26.8/18.3), `virtio_blk_io_gate_contention` (13.8/5.6) and
`lock_route_races` (12.9/4.1) as the escaped-defect group (:123-126), with `threads_futex` and
`kill_at_exit` under lifecycle (:120-121). It names the two over-budget discriminators,
`login_lock_park` (102.9/88.3) and `kill_ready_in_kernel` (63.3/3.5), in the full-tier-only table
(:130-137). That is the "with a note" resolution F1 asked for.

What I re-verified against the tree, not the prose:
- All 31 names exist as `TestScenario` entries, and all 31 are `ignored: false` (so they are in the
  default suite); `ring3_fault_ss` and `ring3_fault_xm` are `ignored: true`, as :114 says. The ring-3
  default set is exactly the five listed.
- The 31 wall times sum to **461.1 s** (the THINK says 461 s), and the max is **33.9 s**
  (`wait_any_core`), against the 35 s wall rule. The old 20 s QEMU rule is gone; the new rule is
  consistent with the numbers.
- The 12-minute check is on scenario wall from the job's own `timings.tsv` (:95-97), which is the
  right layer — a red, not a silent slowdown.
- `block_write_guard` is a real discriminator (xtask :4699); `block_write_guard_4k` exists at :4709
  but is not listed (see New, minor).

Residual (not a blocker): "every escaped-defect scenario" is still a manual claim. The S1 host tests
(:139-143) check that names exist, match the timing table and meet the cost rule; they do not check
that the list contains every historical escape. Adding the two I named closes this instance; a
mechanical guard would need a labelled escape registry, not a timing table.

## 2. One-action landing and prefix bisect — **PASS**

F9 is resolved in §2.4 (:258-269): the PM re-assembles the batch locally and compares its tree hash to
`batch-report`'s **before** pushing; the landing is one push; `scripts/revert-batch.sh <base_sha>`
reverts the merge commits in reverse into **one** commit. That is a one-action landing and a one-action
revert unit, which is what "main one action from green" needs. The post-landing diff (:264-266)
compares everything outside `Plans/`, `docs/` and `*.md`, so F28's environment files are covered.

F8/F13 are resolved in §2.5 (:285-295): prefix bisect is primary, `prefix=0` is the base alone,
`base_sha` and `prefix` are inputs, the owner is the first failing prefix's last member, and infra,
build failures and timeouts are classified before bisecting (:278-284). That is a mechanical bisect,
not the "whose head reproduces" heuristic.

Residual / new (see New #1): the input that carries the failing scenario names is missing, so the
`cargo neo-test <names>` leg and the `⌈log₂ N⌉ × ~5 min` cost estimate are not yet implementable as
written.

## 3. Concurrency-core path filter — **PASS**

§2.3 puts every path F14 named on the core list:
- `kernel/src/sched/{...wake_trace}.rs` (:166-167);
- `kernel/src/sync/**` (:168);
- all of `kernel/src/mm/**` except four named helpers, which covers `slab.rs`, `tlb_shootdown.rs`,
  `pmap.rs` (:169-174);
- `kernel/src/arch/x86_64/{smp,per_cpu,context,idt,apic,apic_logic,timer,lapic_timer,syscall,mod,gdt,fpu}.rs`
  and `kernel/src/main.rs` (:175-177);
- `ai/par_matmul.rs`, `ipc/mod.rs`, `watchdog.rs`, `drivers/block/virtio_blk.rs` (:178-182).

F15 is resolved by the totality rule (:193-203): every file under `kernel/src/` must be on the core or
non-core list, a host test fails otherwise, and any unlisted `kernel/**` path is full-alone at run time.
That is the fail-safe direction F15 asked for. F16 is item 6 (:183-187); F17 is 2.3.1 (:205-225),
defined exactly with fixtures and explicitly deferred until the PM answers question 3.

What I re-verified: the THINK's claim that "every kernel file the bank names twice or more is on it"
holds against `docs/concurrency-pitfalls.md` — `scheduler.rs` (13), `idt.rs` (11), `tlb_shootdown.rs`
(8), `process.rs` (5), `arch/x86_64/mod.rs` (5), `smp.rs`, `per_cpu.rs`, `par_matmul.rs` (3 each),
`lock_owner.rs`, `syscall.rs`, `gdt.rs`, `context.rs`, `apic.rs` (2 each) are all core under the list.
The `mm` group has exactly three `*_logic.rs` helpers plus `hhdm_requirements.rs` — the four the THINK
says it excepts.

Residual (not a blocker): the non-core enumeration is illustrative ("helpers like `rtc.rs` and
`pit.rs`"): `arch/mod.rs`, `cpufreq.rs`, `ioapic.rs`, `pic.rs`, `refclock.rs` and their logic files
are not named. The S1 totality test will force them onto one list, and an unlisted path is full-alone,
so this is safe — but the THINK's list is not yet the total map it claims to describe.

## 4. Free-tier arithmetic — **PASS**

§3 (:297-328) corrects every number in F19-F23:
- one seat is **3** jobs, not 4 (no separate `plan`; `gate-plan` is a step in each job, :79-84);
- a batch is **2 + K shards = 8** at K=6, not K+4 = 10 (:304);
- 4 seats + a batch K=6 = **20, exactly** (:310); 5 seats + K=6 = 23 (:311); 5 + K=5 = 22 (:312);
- `main`'s post-landing run is counted (2 jobs after S2 folds it, :305) and its row says the batch has
  finished before it lands (:313);
- the scheduling rule is stated: dispatch a batch only when at most 4 quick tiers run (:316-318), and
  `ot61-closure.yml` is named as a fifth consumer (:319-322);
- the batch wall uses the measured largest shard (51 min + ~5 setup, :324-326), not the old 255/6
  estimate.

I re-checked the arithmetic: 4×3+8 = 20; 5×3+8 = 23; 5×3+7 = 22; 12+2 = 14. All rows are correct.

Residual (minor): "20, exactly" has no margin for the **previous** landing's `main` push run (2) or an
`ot61-closure.yml` run overlapping the next batch dispatch. The "two never overlap" line is true for
the same batch (its run ends before it lands), not for batch N+1 dispatched during batch N's `main`
run: 4 quick + 2 main + 8 batch = 22. The dispatch predicate counts quick tiers only. Fix: count the
`main` run in the predicate, or cap at 3 quick tiers while a `main` run is in flight.

## 5. Rule changes stated in CLAUDE.md — **PASS**

§6 (:353-377) is now a per-rule table with today vs with-batches for every standing rule F24-F30
named: the merge bar (:359), the close-gate evidence contract (:360), the AAG plus a batch-composed
AAG (:361), the escaped-defect/revert unit (:362), the 2-3 self-hosted QEMU stagger superseded for
GitHub-hosted runs (:363), and tree identity (:364). The "Unchanged" list (:366-373) is explicit, and
the new invariant is named with its guards (:375-377). That is the disclosure F24-F30 asked for.

Residual / new (see New #2): revision 2 dropped the kill-switch equivalence regression that revision 1
had ("a host test pins `--shard i/4` without `--tier` to today's partition"). §2.4 keeps the mode
(:273) but no regression, and §6 does not name the kill-switch. The complexity-governance rule wants a
regression proving disabled ≡ the previous layer; the S2 gate "defaults reproduce today's gate" is not
that test.

---

## New in revision 2

1. **The prefix bisect has no input to carry the failing scenario names (fix before S2).** §2.4's input
   list is `batch`, `base_sha`, `prefix` (:229-232); §2.5 says a prefix dispatch runs "only the failing
   scenarios (`cargo neo-test <names>`, in one job)" (:286). Revision 1 had
   `scenarios: "<names>"`; revision 2 dropped it. As written, S2 cannot pass `<names>`: it either runs
   the whole suite per prefix (~56 min/round, not the claimed ~5 min + failing scenarios) or must read
   the prior `batch-report` artifact, which is not stated. Fix: restore `scenarios` (or a
   `from_run <run_id>` input) in §2.4 and say which. This is the one functional gap in the otherwise
   sound bisect design.
2. **The kill-switch equivalence regression was dropped (restore it, or state the substitute).**
   Revision 1 pinned the no-`tier` default to today's 4-shard partition with a host test; revision 2
   keeps the kill-switch mode (:273) without that regression, and §6's "Unchanged" list omits it. The
   standing rule requires a regression proving disabled ≡ the previous layer, bit-for-bit. Fix: keep
   the host test, or make the S2 gate's "defaults reproduce today's gate" a host/CI assertion and name
   it in §6.
3. **Minor, non-blocking:**
   - `block_write_guard_4k` (xtask :4709, the 4K-sector guard witness) is neither in the 31 nor in the
     excluded table. The primary `block_write_guard` covers the guard path; naming the 4K variant or
     saying why not would complete F5.
   - "The 5 slowest | 90 of the 307 min" (:64) does not add up: the five listed values sum to 5,181 s =
     **86.4 min**. The per-shard sums (75/89/65/77) are **306**, not 307. Reconcile or round both.
   - §3's `4 seats + a batch = 20 exactly` needs the margin note in item 4 above.

**What I checked, so the PM can separate this from a bare re-read:** the 31 scenario names and their
`ignored` flags in `tools/xtask/src/main.rs`; the 461.1 s sum / 33.9 s max; `_ss`/`_xm` ignored flags;
the four excepted `mm` helpers; the pitfall-bank twice-named set; the §3 arithmetic rows; the
`scenarios`/`shards` inputs present in revision 1 and absent in revision 2; and the kill-switch
regression present in revision 1 and absent in revision 2. No commit in the PR changes code — the only
file is the THINK — so no CI/QEMU evidence was expected for this review.
