<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH FINDINGS — Mine the known answers: a standing SMP/concurrency pitfall bank + discriminator seed

**Date:** 2026-07-12 · **Author:** Oracle (main-tier, Claude-family) · **Dispatch:** OT-62
(`Plans/DISPATCH-2026-07-12-smp-concurrency-pitfall-mining-oracle.md`) · **Branch:**
`codex/smp-concurrency-pitfall-mining-oracle` · **Type:** prior-art research, **HOST-ONLY, no code, no QEMU.**

**Primary deliverable:** `docs/concurrency-pitfalls.md` — the standing pitfall bank (CP-1…CP-17), the durable
checklist every future SMP-touching dispatch consults. **This file** is the research record: sources,
methodology, the cross-model verification, the exposure map, the two OT-59 worked examples, the
armed-discriminator recommendations, and the proposed CLAUDE.md rule extension for PM/PL ratification.

---

## 1. What was delivered (ISC roll-up)

| ISC | Ask | Status |
|-----|-----|--------|
| ISC-1 | Mine the documented pitfall corpus from **primary** sources (failure/mechanism/fix/citation) | ✅ 17 pitfalls, 7 categories; every entry primary-cited (§3) |
| ISC-2 | Cover the named categories (lock-order, IRQ-safe, TLB/IPI, barriers, sched/wake, atomics, publish-before-ready) | ✅ all seven, plus the publication-before-ready cross-cutting class as CP-17 |
| ISC-3 | Map each pitfall to OUR kernel (exposed/guarded/N-A) + a discriminator | ✅ exposure map with `file:line` evidence (§4); the two OT-59 races as worked examples |
| ISC-4 | The standing artifact + armed-discriminator recs + the CLAUDE.md rule extension | ✅ `docs/concurrency-pitfalls.md`; §5 recs; §6 rule text |

**Scope-guard compliance:** host-only, **no code changed**, no QEMU. **No GPL/copyleft code copied** —
Linux/FreeBSD read for principle and cited, re-derived in our primitives (§2). Load-bearing "the fix is X"
claims **3-vote cross-model verified** across three model families (§2.3). RAG was healthy
(`{"status":"healthy","ollama":true,"qdrant":true}`); exposure mapping used direct `Grep`/`Read` on the named
files (grep wins for exact symbols) plus the RAG queries in the dispatch.

---

## 2. Framing, licensing, and verification

### 2.1 Why this is valuable AND bounded
The PL's insight — *"these SMP bugs have already been found and squashed in other systems; do we not have the
answer out there?"* — is **true for the PRINCIPLE, not for drop-in code.** Prior-art fixes are welded to their
host kernel's primitives; we made different (capability-secured, our-own-lock-type) choices, and our bugs
manifest at seams we built new (OT-59's second race lives at *our* spawn/ring-3 seam). So the bank is a
**checklist of documented traps + the discipline that avoids each + our-exposure mapping + a discriminator to
pre-seed** — never a code port. It shortens the search for the **known** classes; **it does not replace** the
adversarial-audit gate, the 4-shard QEMU gate, the byte-oracle, or the kill-switch (**patterns-known ≠
correctness-proven — the seL4 lesson**). The genuinely-ours seams still iterate and still get verified.

### 2.2 Hard licensing rule (stated per dispatch)
Linux/GPL and FreeBSD/BSD sources were **read for principle/understanding and cited**; **no code was copied**
into our MPL kernel. Each entry extracts the *pattern/discipline*, cites the exact source location, and
re-derives in our own primitives. This is a learn-the-answer pass, never a lift (consistent with ADR-026).

### 2.3 The 3-vote cross-model verification (the load-bearing claims)
Per the PL standing rule (*cross-model diversity for convergence-signal work* — a convergence signal is only
strong across **different** model families), the five load-bearing "the fix is X" claims were independently
adversarially verified by **two non-Claude reviewers**, giving a genuine **three-family panel**:

- **Oracle (author)** — Claude-family.
- **Forge** — Codex/GPT-5.4 family (prompted to *refute*, default-suspicious).
- **Anvil** — Kimi K2.6 / Moonshot family (prompted to find the missing precondition).

**Result: strong cross-model convergence — no claim refuted; all five are genuine documented hazards with
correct-direction fixes.** Both families *independently* surfaced the same precision boundaries, which is the
strong-signal case — and I folded every load-bearing correction into the bank:

| Claim (→ bank entry) | Panel verdict | Load-bearing correction folded in |
|---|---|---|
| C1 IF-off cross-CPU-wait deadlock (→ **CP-2**) | Confirmed, **fix necessary-not-sufficient** | Deadlock needs the awaited CPU to be a **target** (not "any CPU on the lock"); IF-on must be paired with **preemption-disabled + CPU-pinned + stable/serialized payload**; handler stays ACK-only/never takes sender lock. Both families raised this. |
| C2 mm_cpumask + freed-tables (→ **CP-4/CP-5**) | Confirmed, **precision** | Reason = **paging-structure caches + speculative interior walks** (SDM §4.10.3), not leaf TLB; "flush all" = **mm_cpumask CPUs with lazy-skip disabled, not machine-wide**; RCU-table-free is the modern alternative. |
| C3 publication-before-ready (→ **CP-8/CP-17**) | Confirmed, **sufficiency boundary** | A boolean is sufficient **iff readiness is monotonic ∧ ready⇒timely-serviceable**; revocable liveness or effect-proof ⇒ **round-trip required**. Diagnostic: persistent timeouts after the flag ⇒ residual is a CP-2 IF-off window. |
| C4 EOI-only reschedule IPI (→ **CP-15**) | Confirmed, **mechanism sharpened** | Real ingredient = **`need_resched` + a preemptible IRQ-return point**, not the handler body; EOI-only is by-design adequate at any honoring point; stranding is the **non-preemptible/no-timer busy-loop** case. |
| C5 park-under-lock (→ **CP-14**) | Confirmed | Invariant = **never (committed-to-sleep ∧ lock-free ∧ on-CPU)**; early release also risks **two-CPUs-on-one-stack** corruption; the **`on_cpu` load-acquire** is the cross-CPU refinement (our Box-move + take-once + release-on-new-stack is the equivalent). |

This adversarial panel is a *pre-seed of the known classes*, not a correctness proof of the OT-59 fixes — those
still go through the PM's adversarial-audit gate and the QEMU gate on their own branches (Jarvis/Cerebro).

---

## 3. The pitfall corpus (primary sources, read directly)

Full corpus with mechanism/fix/our-exposure/discriminator/citation is in `docs/concurrency-pitfalls.md`. The
17 entries, by category:

- **A — Lock ordering:** CP-1 ABBA / inconsistent global order (lockdep).
- **B — IRQ-safe locking:** CP-2 IF-off spin awaiting a resource this CPU must interrupt-service (**the OT-59
  #1 class**); CP-3 holding an IRQ-safe lock across a cross-CPU/blocking wait.
- **C — TLB shootdown / IPI:** CP-4 shooting a CPU not running the mm (`mm_cpumask`); CP-5 freed-page-table
  teardown must flush lazy CPUs (speculative-walk hazard); CP-6 INVLPG / paging-structure-cache & MOV-CR3
  ordering; CP-7 ACK/EOI ordering in the handler; CP-8 shootdown to a published-but-not-ready CPU (**OT-59 #2**).
- **D — Memory ordering:** CP-9 store→load reordering under x86-TSO; CP-10 compiler-fence vs CPU-fence.
- **E — Per-CPU + scheduler/wake:** CP-11 wrong `Ordering` on an atomic; CP-14 lost wakeup (park/unpark);
  CP-15 stranded wakeup (EOI-only reschedule IPI); CP-16 wake path under a foreign lock (reap→wake).
- **F — Atomics/RMW:** CP-12 ABA / slot reuse; CP-13 torn multi-word read.
- **G — Cross-cutting:** CP-17 publication-before-ready.

**Primary sources actually read/verified this pass** (exact locations in the bank's "Primary sources" section):
- **Intel SDM Vol 3A** — §4.10 caching translation info (§4.10.3 paging-structure caches, §4.10.4 invalidation,
  §4.10.2 global pages); §9.2 memory ordering (x86-TSO); §11.6/§11.6.1 issuing IPIs + ICR delivery-status;
  §11.8.4–§11.8.5 EOI/TMR/level-triggered. *(Section numbers edition-dependent — cited by title.)*
- **Linux** — `arch/x86/mm/tlb.c` (`native_flush_tlb_multi`, `should_flush_tlb`, `mm_cpumask`, `is_lazy`,
  `tlb_gen`, `freed_tables`); `Documentation/memory-barriers.txt` (section list + barrier taxonomy +
  ACQUIRE/RELEASE + `barrier()` vs `smp_mb` + UP-reduces-to-compiler-barrier — **fetched and quoted**);
  `Documentation/locking/lockdep-design.rst` (lock classes; "inverse order" + "hardirq-safe → hardirq-unsafe
  forbidden"; validate-once caching — **fetched and quoted**); `Documentation/atomic_t.txt`;
  `kernel/sched/core.c` (`try_to_wake_up`, `resched_curr`, `TIF_NEED_RESCHED`, `on_cpu`).
- **FreeBSD** — `sys/x86/x86/mp_x86.c` `smp_targeted_tlb_shootdown()` interrupts-enabled `KASSERT` (panic
  string `"smp_targeted_tlb_shootdown: interrupts disabled"` — **confirmed via freebsd-amd64 archive**);
  `curcpu_cb` self-invalidation; `smp_rendezvous`.
- **OSDev** — *TLB Shootdown*, *APIC* (ICR/EOI/IPI), *Spinlock*. (The TLB-Shootdown page returned HTTP 403 to
  the fetch tool this session; its content is standard and cross-covered by the SDM + Linux/FreeBSD sources.)
- **Academic** — Sewell/Sarkar et al., *x86-TSO* (CACM 2010).

---

## 4. Our-kernel exposure map (grep/RAG evidence)

| Pitfall | Exposure | Evidence (`file:line`) |
|---|---|---|
| CP-1 lock order | Guarded (partial) | `sync/switch_mutex.rs:49-119,178-202` (debug lockdep, per-thread mask, panic-at-acquire); order table `sync/mod.rs:21-35`; I-5. **Gap:** scheduler/NeoFS/`PROCESSES`/`waiters` locks are unordered (level 0). |
| CP-2 IF-off cross-CPU wait | **Was EXPOSED (OT-59 #1)** | `mm/tlb_shootdown.rs:24` `SHOOTDOWN_LOCK: IrqSafeMutex<()>`; `:162` sender takes it IF-off; `:234-265` spins for ACKs; `sync/irqsafe_mutex.rs:36-45` (disables IF on lock). |
| CP-4 mm_cpumask | Guarded (T1 broadcast) | `mm/tlb_shootdown.rs:211-221` `shootdown_ready_cpus_mask` (broadcast, not per-mm); T2 note `:141-160`. |
| CP-5 freed-tables | Guarded (no lazy-TLB yet) | `mm/tlb_shootdown.rs:97-107` (capture-before note; lazy-TLB out of scope, exception recorded). |
| CP-6 INVLPG ordering | Guarded | `mm/tlb_shootdown.rs:19` `INVLPG_THRESHOLD_PAGES=33`; `:308-327` per-page vs full; self-invalidate after dispatch `:136,179`. |
| CP-7 ACK/EOI | Guarded | `arch/x86_64/idt.rs:392-396` (handle_ipi → EOI); ACK `tlb_shootdown.rs:203-204` (`pending.fetch_sub(1, Release)`) before EOI. |
| CP-8 publish-before-ready | **Was EXPOSED (OT-59 #2)** | `mm/tlb_shootdown.rs:164-166,211-221`; readiness flag `arch/x86_64/per_cpu.rs:248-262` (Release after IDT/LAPIC/sched/IF up); proxy `sched/process.rs:1433` `any_user_process`; handshake counter `idt.rs:234-253`. |
| CP-9/CP-11 ordering | Guarded | Release/Acquire on `pending`, per-CPU slot `AcqRel` (`tlb_shootdown.rs:186,204,227,241`), `tlb_shootdown_ready` Release/Acquire (`per_cpu.rs:252,261`); Relaxed only on pure counters. |
| CP-12 ABA | Guarded (already armed) | Generational arena I-13 (`ai/arena.rs`, ADR-042); `ThreadWakeToken`/`TokenTakeResult::Stale` (`scheduler.rs:1535-1556`). |
| CP-14 lost wakeup | Guarded (I-11) | `sched/scheduler.rs:1435-1491` `block_current_with_token_and_guard` (token under lock, release-on-new-stack via `switch_context_with_release2`); waker `:1535`. |
| CP-15 stranded wakeup | Partial (OT-43/OT-49) | `apic.rs:89` `send_reschedule_ipi` → `idt.rs:386-389` EOI-only; OT-49 idle thread `7f6c2fb`. |
| CP-16 reap→wake lock order | Under diagnosis (OT-59 #2 sibling) | `scheduler.rs:1905-1958` `reap` (frees → shootdown) → `process.rs:1472-1527` `on_thread_reap` → `:1230-1265` `complete_process_waiters` (waiters lock dropped **before** `wake_thread` — good) → `scheduler.rs:1502-1507`. |
| CP-17 publish-before-ready | Primitive exists | `per_cpu.rs:248-262` (form-1 readiness flag done right); OT-59 #2 = FIX-2 round-trip (form-2). |

**Cross-lane coupling surfaced:** `reap()` frees memory → triggers TLB shootdowns (CP-2/CP-8), and the Arthur
family (Cerebro's lane) hangs *in* `reap()` (CP-16) — so Jarvis's CP-2 shootdown fix **may incidentally green**
some Arthur scenarios. The bank makes this explicit (re-test Arthur after the shootdown fix before concluding a
second independent defect exists — matches the OT-59 watch-item).

---

## 5. Armed-discriminator recommendations (Rule-5 paid-down debt)

Detail in `docs/concurrency-pitfalls.md` → "Armed-discriminator recommendations." Summary:

**Promote to permanently-armed discriminator / ledger invariant:**
1. **CP-2/CP-3** — the deterministic **two-CPU shootdown-lock probe** (`shootdown_lock_probe`) + timeout panic, armed at `NEO_SMP≥4`.
2. **CP-7** — keep the `pending` ACK counter; add a debug "exactly-one-EOI-per-vector" assertion.
3. **CP-8/CP-17** — arm the **`from_ring3` reschedule-handshake counter**; recommend a **new ledger invariant
   (candidate I-16, pending the Rule-2 one-page-fit re-check):** *"Cross-CPU actions gate on an explicit
   readiness signal published after the target is safe, never on a visibility proxy."*
4. **CP-15** — arm OT-49's **timer-token idle-entry probe (B4)** + a no-timer-AP dispatch-within-N-ticks regression.
5. **CP-16** — **extend I-5's ordered lockdep levels** to the scheduler-inner / NeoFS / `PROCESSES` / `waiters`
   locks (closes the CP-1 coverage gap so a reap→wake inversion panics at the acquire, not a silent stall).
6. **CP-11** — a **grep-gate**: `Ordering::Relaxed` on a control flag fails review unless on the audited pure-counter allowlist.
7. **CP-5** — **capture-before-you-need-it**: the freed-tables exception + its negative-control gate ship in
   the same build that first introduces lazy-TLB.

**Already armed (keep):** CP-12 (I-13 generation counter), CP-14 (I-11 park/retry balance + quiesce hook),
CP-1 (I-5 lockdep — extend per CP-16). **Review-checklist (not standing):** CP-4 (arms when T2 lands),
CP-6, CP-9/CP-10, CP-13.

**Note on candidate I-16 (Rule-2 discipline):** adding a ledger row requires the one-page-fit re-check. I-16 as
worded is cross-cutting (every future cross-CPU seam preserves it) and its guards already exist (the
shootdown-ready intersection + the `from_ring3` counter), so it is a *name-the-existing-guard* addition, not new
machinery — but the PM makes the page-fit call at ratification.

---

## 6. Proposed CLAUDE.md rule extension (for PM/PL ratification)

> ## Concurrency / SMP-touching dispatches require a concurrency-pitfall-bank consultation (PM dispatch standing rule)
>
> Any dispatch that adds or changes **cross-CPU / interrupt / lock / atomic** code — the concurrency analogue
> of the driver/blind-hardware prior-art trap-mining rule — MUST, before it can be marked DISPATCHED, include a
> **concurrency-pitfall-bank consultation**: for each pitfall class the change touches
> (`docs/concurrency-pitfalls.md`, CP-1…CP-17), state *introduces / guards / N-A* and name the discriminator
> that proves it. New pitfalls found on the gate are added back to the bank (it grows; it is never pruned).
> This pre-seeds the **known** classes so real iteration is spent on the genuinely-ours seams — it does **not**
> replace the adversarial-audit gate, the QEMU gate, the byte-oracle, or the kill-switch (patterns-known ≠
> correctness-proven; the seL4 lesson). Copyleft/GPL sources are read for principle and cited, never copied
> (ADR-026).

Placement suggestion: adjacent to "Driver / blind-hardware dispatches require a deep prior-art trap-mining
pass" (the sibling rule it extends), or under the connectivity/fault-containment cluster.

---

## 7. Close-gate + RETURN

- **Docs-only ⇒ no CI run exists.** Only `docs/concurrency-pitfalls.md` and this `Plans/` file are added; CI
  paths-ignores `docs/**`, `Plans/**`, `**.md`, so no push-CI fires (per the CI-gate rule; `[skip ci]`
  redundant-but-harmless — not used). **`scripts/worker-close-gate.sh` host set: skip-with-note** — no `.rs`/
  `Cargo.*`/`tools/` touched, so `fmt`/`clippy`/host-tests/kernel-clippy are trivially unaffected (nothing to
  compile). **Not kernel-touching ⇒ no `workflow_dispatch` QEMU gate required.**
- **SPDX:** both new docs carry `<!-- SPDX-License-Identifier: CC-BY-4.0 -->` as the first line (docs/ = CC-BY-4.0).
- **Rebase freshness:** branch cut from `origin/main@5d69b3c` this session; will `git fetch && git rebase
  origin/main` at close if `main` advanced, then push.
- **Diff-vs-artifacts:** the RETURN below is diffed against the two committed files (§8).
- **Disk reclaim:** after the PM merges this branch, its working tree/`target/` can be reclaimed (docs-only —
  no build artifacts of note, but the checkout is deletable once on `origin/main`).

## 8. RETURN (evidence block)

- **FINDINGS:** `Plans/RESEARCH-2026-07-12-smp-concurrency-pitfall-bank-FINDINGS.md` (this file).
- **Standing artifact:** `docs/concurrency-pitfalls.md` — 17-pitfall bank, 7 categories, our-exposure map,
  discriminators, two OT-59 worked examples, armed-discriminator recs, the proposed rule extension, primary
  sources.
- **Pitfall corpus (mechanism/fix/citation):** §3 + the bank body.
- **Our-kernel exposure map:** §4 (with `file:line`).
- **Two OT-59 worked examples:** the bank's "worked examples" section (CP-2/CP-1 shootdown-lock ABBA;
  CP-8/CP-17 spawn/ring-3 publication).
- **Armed-discriminator recommendations:** §5 + the bank.
- **Proposed CLAUDE.md rule extension:** §6.
- **Cited primary sources:** §3 + the bank's "Primary sources."
- **3-vote cross-model verification:** §2.3 — Oracle (Claude) + Forge (Codex) + Anvil (Kimi), 3 families,
  strong convergence, all corrections folded into the bank.
- **PM/PL ratification items:** (a) adopt `docs/concurrency-pitfalls.md` as normative; (b) ratify the §6
  CLAUDE.md rule extension; (c) rule on candidate ledger invariant **I-16** (Rule-2 page-fit check).
