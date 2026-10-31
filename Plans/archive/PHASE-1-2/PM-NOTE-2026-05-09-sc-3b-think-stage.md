# PM-NOTE (THINK stage): SC-3B scope for Phase 2 BUILD-3

**Date:** 2026-05-09
**Author:** PM
**Status:** THINK stage analysis. Not a dispatch, not an ADR amendment, not normative. Becomes input to the SC-3B PLAN-stage checkpoint conversation. Per Bible Article 14's four-stage cadence (OBSERVE → THINK → PLAN → BUILD/VERIFY), this is the THINK stage for the next BUILD-3 sub-commit.
**Authority:** Project lead. PM proposes; project lead ratifies scope.
**Triggered by:** Project lead's directive in conversation 2026-05-09: "Go ahead and Think stage. Bible + .hack real world os + AI ML built into the os from the Kernel up."

---

## 1. Substrate

**Landed (BUILD-3 to date):**
- Sub-commit 0: ADR-013 published, GLOBAL bit fixup (`fe5bc3d` / `b4b3ac9` / `57fd235`)
- Sub-commit 1a: per-CPU GDT/IDT/TSS/IST refactor (`305f4d9`)
- Sub-commit 1b: AP bring-up via Limine MP request (`3bfcfc8`)
- Sub-commit 2: per-CPU schedulers + §Q5 restore + AP GS-base rewire (`0b5ffb4`); §Q9 deferred indefinitely
- Wedge-fix arc: L (`4c6abf4`), Bible 8.5 (`8ef8cac`), ADR-016 ratified (`8a2311d`), A1 IRQ-safe heap (`aac0c11`), ADR-016 amendment (`1dbbfa7`), INDEX swept (`79b35f4`)

**ADR-013 work remaining:**
- **Q4** — wake-on-completer-CPU (cross-CPU IPC rendezvous wake)
- **Q6** — lock-order asm release for `block_current_into`
- **Q7.b/c** — CR3 cache, IPI-driven TLB shootdowns
- **Q8** — capability-table SMP composition (Q8.c is blocked-thread-kill walk)

**Pre-existing input artifact:** `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` scopes "SC-3A" as Q6 alone and analyzes Q6's composition with Q4 and Q8.c. No SC-3A code commit was ever produced; the pre-thinking note becomes input to whatever next sub-commit covers Q6.

---

## 2. Three lenses

### 2.1 Bible

**Article 8.3** ("SMP from Phase 2... global locks forbidden in hot paths; per-CPU structures and lock-free or fine-grained locking"). Sub-commit 2 made schedulers per-CPU; Q4 + Q6 are what make the IPC hot path actually SMP-safe under those per-CPU schedulers. Until both land, IPC rendezvous is single-CPU-only by virtue of ADR-011's superseded invariants still being in force.

**Article 8.5** (just ratified, v1.0.5): 8.5.4 — "No blocking while holding a spinlock... Operations that acquire mutexes or semaphores from spinlock-held context are forbidden." `block_current_into` currently holds the scheduler lock across a stack switch — structurally adjacent to violating 8.5.4. Q6's asm release on the new stack is precisely the structural mechanism that keeps the spinlock release atomic-with-context-switch. **Q6 is structural conformance with the bible amendment we just ratified.**

**Article 12.1** ("Capability-based security from Phase 2"). The capability-table SMP composition (Q8) is the direct delivery of this mandate. But Q8 builds on Q4 + Q6: capability operations under SMP need IPC handoff (which is how capabilities flow between processes) to be SMP-safe first.

**Article 1.2** (correctness > coherence > performance). Q4 + Q6 are correctness-critical. Q7 (TLB) and Q8 (capability SMP) are also correctness, but layered on top.

**Article 1.3** ("every commit is intended to ship"). A standalone Q6 sub-commit (the original SC-3A framing) would land the asm helper without any production caller — scaffolding, not behavior change. That's contrary to 1.3's spirit. The natural test for Q6's helper is the IPC retrofit; testing it in isolation is artificial.

**Article 14** (/simplify gate). The competing tension: a combined Q4+Q6 sub-commit is larger; a split SC-3A/SC-3B is two smaller commits. Simplify-gate prefers smaller commits IF they can cleanly split. The split here doesn't cleanly satisfy 1.3.

### 2.2 ALTIMIT real-world posture

From `Plans/ALTIMIT-real-world-implementation.md`:

**§A.5 capability-shaped authority** — the Twilight Bracelet pattern. The report identifies cap-handles as a posture this OS must structurally support, which directly maps to ADR-013 §Q8 (capability table SMP composition). But §Q8 builds on capability transfer being safe under SMP, which is what `sys_grant`/`sys_recv_grant` do, which is what Q4 + Q6 retrofit.

**§A.2 crash-immune by construction** — the Pluto's Kiss test. Q6's asm helper is precisely the kind of structural-rather-than-empirical mechanism this posture demands. The wedge fix (L) was empirical mitigation; A1 was structural at the heap layer; Q6 is structural at the lock-order layer. Continuing the same architectural pattern.

**§G.1 capability composition** — "Bible Article 8.3 (no global locks in hot paths) — A2 honors structurally; **the IPC retrofit (Q4+Q6) extends the same posture to capability transfer**." The report's Recommendation Stage 2 explicitly names "capability composition under SMP" as the project's research-frontier work. SC-3B is the on-ramp.

### 2.3 AI-native premise (ANSPEC-001)

**§3.7 capability handles on tensors** — "Userspace never holds a raw `TensorId`. It holds a capability handle... Capabilities are passed between processes through the standard cap-passing IPC, Phase 2 deliverable." ANSPEC-001 *literally* names Phase 2 cap-passing IPC as the foundation for tensor handle transfer. Q4 + Q6 are what build that foundation under SMP.

**§4.6 model sharing** — "Models are inherently shareable. A model loaded by process A may be used by process B if A grants a model capability handle to B (via the standard cap-passing IPC, Phase 2)." Same dependency.

**§5 inference contexts** — async inference uses tickets that are themselves capabilities (§5.2). Streaming inference holds a stream handle that can be passed between processes (§5.3). Each is Q4 + Q6's wake-on-completer-CPU + asm-release pattern applied to a different blocking primitive.

**§7.6 sibling scheduler** — the inference scheduler in Phase 7 is "a sibling to the main thread scheduler, not a subordinate. They communicate: a thread blocked on a synchronous inference call is parked by the main scheduler and woken by the inference scheduler upon completion." This is precisely Q4's wake-on-completer-CPU pattern applied across schedulers. **Q4 is the architectural precedent for inference-context wake.**

**§10.2 capability discipline** — "Capabilities are passed between processes through the standard cap-passing mechanism (Phase 2 deliverable). They are not forgeable; they cannot be guessed." The Phase 2 cap-passing mechanism that ANSPEC-001 cites is what Q4 + Q6 retrofit for SMP correctness.

**Convergence:** All three lenses point at Q4 + Q6 together. The Bible mandates SMP-safe IPC; ALTIMIT posture demands structural lock-order discipline; ANSPEC-001 names cap-passing IPC as the Phase 2 deliverable that AI primitives will reuse. They're not parallel concerns — they're the same concern viewed from three angles.

---

## 3. Map of remaining ADR-013 work

| Q | What it does | Depends on | Naturally before/after |
|---|---|---|---|
| **Q4** | Cross-CPU rendezvous wake on completer-CPU | Per-CPU schedulers (sub-commit 2 ✓), Q6 (parking safety) | Pairs with Q6 |
| **Q6** | Asm release of locks on new stack in `block_current_into` | Per-CPU schedulers (sub-commit 2 ✓) | Pairs with Q4 |
| **Q7.b** | CR3 cache to skip same-Pmap reloads | AP bring-up (sub-commit 1b ✓) | After Q4+Q6, before Q8 |
| **Q7.c** | IPI-driven TLB shootdowns | AP bring-up (sub-commit 1b ✓), Q7.b | After Q7.b |
| **Q8.a/b** | Capability table SMP composition | Q4+Q6 (cap transfer), Q7 (TLB for cap-walk) | After Q4+Q6+Q7 |
| **Q8.c** | Blocked-thread-kill walk through Channel EndpointState | Q4+Q6, Q8.a/b | After Q8.a/b |

The ordering is largely forced by dependencies. Q4+Q6 must come first; Q7 next; Q8 builds on both.

---

## 4. Three scope options for SC-3B

### Option (i): SC-3A then SC-3B (separated)

- **SC-3A:** Q6 asm helper alone, with a synthetic test scenario.
- **SC-3B:** Q4 + apply asm helper to IPC sites + cross-CPU IPC scenarios.
- Pro: Each commit smaller; /simplify gate happier per-commit.
- Con: SC-3A ships an unused primitive — violates Bible 1.3's "every commit is intended to ship." Synthetic test for Q6 in isolation is contrived.

### Option (ii): SC-3B = Q4 + Q6 + IPC retrofit, all together

- One sub-commit covering Q4 + Q6 + applying the helper to all `block_current_into` callers + cross-CPU IPC scenarios.
- Pro: Ships meaningful behavior change. Q6 has natural callers (the IPC paths). Q4's correctness is testable via real cross-CPU rendezvous scenarios.
- Con: Larger sub-commit. /simplify gate scrutinizes.
- Mitigation: split into multiple **commits** within the sub-commit boundary — e.g., commit 1 lands Q6 helper + plumbing; commit 2 wires Q4 wake-on-completer-CPU; commit 3 adds cross-CPU scenarios. Same sub-commit, three commits. Each individual commit is small.

### Option (iii): SC-3B = different work (Q7 or Q8)

- Skip Q4+Q6 to start with. Do Q7 (TLB) or Q8 (capability SMP) first.
- Pro: None apparent.
- Con: Q8 needs Q4+Q6 as a prereq (capability operations call into IPC). Q7 doesn't strictly need Q4+Q6 but the lock-order discipline of Q6 affects how IPI-driven shootdowns interact with scheduler locks. Going out of order forces awkward shape later.

### Recommendation

**Option (ii).** SC-3A and SC-3B fuse into one sub-commit, multi-commit if helpful. The SC-3A pre-thinking note becomes input directly to SC-3B's PLAN-stage checkpoint conversation. The "SC-3A" naming is retired — there's no SC-3A code commit, just the pre-thinking note that fed SC-3B.

**Reasoning:**
- Bible 1.3 forbids scaffolding-only commits. Q6 alone is scaffolding.
- The natural test for Q6 IS the IPC retrofit. Splitting them creates artificial test surface.
- Q4 cannot ship without Q6's mechanism backing it; the two are coupled by correctness, not just adjacency.
- All three lenses (Bible, ALTIMIT, AI-native) point at "make IPC SMP-safe" as one logical change.
- Multi-commit-within-sub-commit lets /simplify gate scrutinize per-commit while preserving the logical unit.

---

## 5. SC-3B scope under Option (ii)

### In scope
- **Q6 implementation:** `context_switch_with_release` asm helper (per pre-thinking note §5.1's ABI candidates — PLAN stage picks). Conforms to Invariants I1–I6 from the pre-thinking note.
- **Q4 implementation:** rendezvous wake on completer-CPU. `sys_send`, `sys_recv`, `sys_grant`, `sys_recv_grant` enqueue woken threads on the *completer's* per-CPU scheduler, not the parker's.
- **`block_current_into` retrofit:** all production callers (the IPC handoff paths) use Q6's helper.
- **ADR-011 invariant supersession:** the inline comments in `kernel/src/sched/scheduler.rs` and `kernel/src/sched/process.rs` that say "single-CPU rendezvous invariant" or "Phase 2 SMP must replace this" get either deleted or upgraded to reference ADR-013 §Q4/§Q6 as superseding.
- **New scenarios:** cross-CPU IPC rendezvous tests at NEO_SMP=4. Cover sender-blocks-first and receiver-blocks-first cases. Cover capability grant cross-CPU. Likely 4–8 new test scenarios.

### Out of scope
- Q7 (TLB management beyond Q7.a) — next sub-commit
- Q8 (capability table SMP composition) — next-after-that
- Q9 (per-CPU reapers) — deferred indefinitely
- ADR-016 A2 (scheduler heap-decoupling) — deferred to Phase 2 BUILD-4 or later, separate from SC-3B
- Inference scheduling — Phase 7

### Anti-scope
- **Do not change the IPC API.** Userspace `libneo` syscalls (`sys_send`, `sys_recv`, `sys_grant`, `sys_recv_grant`) keep their existing surface. Q4+Q6 is internal kernel correctness work.
- **Do not change capability semantics.** Rights bits, derivation, revoke — all unchanged.
- **Do not start the inference scheduler.** Phase 7's sibling scheduler will reuse Q4's pattern; it is not implemented in SC-3B.
- **Do not solve A2.** Scheduler heap-decoupling is Phase 2 BUILD-4-or-later work; SC-3B does not touch the scheduler queues' allocation strategy.

---

## 6. Open questions for PLAN stage

These need resolution before BUILD dispatch. The PLAN-stage conversation answers each.

1. **Q6 ABI shape:** const-generic array (a), fixed-shape struct (b), or per-call-site specialization (c). Per pre-thinking note §5.1. Project lead's call.
2. **Memory ordering:** does the asm need `mfence` between lock-release stores, or is x86's natural store ordering sufficient for `spin::Mutex`-equivalent Release semantics? Pre-thinking note §5.2. Likely no fence; PLAN confirms against actual `spin::Mutex` impl.
3. **Panic-safety review:** the `mem::forget`-then-asm window. Pre-thinking note §5.3. PLAN audits the resulting code for any panicable operation in the window.
4. **Cross-process bookkeeping placement:** `cross_process_prep` and `cross_thread_kstack` ordering relative to asm-call. Pre-thinking note §5.4.
5. **Block-target enumeration:** confirmed call sites of `block_current_into` in production. Likely just IPC, but PLAN verifies via `git grep`.
6. **Test scenario set:** which cross-CPU IPC patterns to cover. Sender-first, receiver-first, capability-grant cross-CPU, multi-grant chains, kill-during-rendezvous. PLAN proposes; project lead ratifies.
7. **Verification methodology:** what's the cargo-driven verification loop? Mirror L's 200/200 + supplementary scenarios pattern at NEO_SMP=4.
8. **Composition with A1:** A1's IRQ-safe heap is now in main. Q6's asm release on the new stack happens with interrupts in whatever state they were; verify the asm helper doesn't accidentally re-enable interrupts during the release window. (Sketch in pre-thinking §5.2 has `pushfq` + `popfq` flow — looks correct, but PLAN audits.)
9. **Bible 8.5.3 advancement:** does Q6 advance Bible 8.5.3 conformance? Indirect yes (scheduler critical sections become structurally simpler), but the formal 8.5.3 conformance milestone is A2 at Phase 2 BUILD-4-or-later. PLAN may note the partial advancement.

---

## 7. AI-native and ALTIMIT alignment summary

SC-3B (Q4 + Q6) is the **on-ramp** for everything AI-native that comes later. ANSPEC-001 names cap-passing IPC as the Phase 2 deliverable that tensor handles, model handles, and inference tickets will all reuse. The ALTIMIT report names "capability composition under SMP" as research-frontier work; Q4+Q6 is the substrate that work builds on. Bible 8.3 mandates SMP-safe IPC; Bible 8.5.4 mandates the lock-order discipline Q6 delivers structurally.

The work doesn't require new philosophy. It cashes out philosophy that's already ratified into mechanism.

After SC-3B, the BUILD-3 forward sequence is:
- **BUILD-4 sub-commit (post-3B):** Q7.b/c (TLB management). Independent of capability layer; can be its own sub-commit.
- **BUILD-5 sub-commit (post-Q7):** Q8.a/b (capability table SMP composition). Builds on Q4+Q6's IPC handoff being SMP-safe.
- **BUILD-6 sub-commit (post-Q8):** Q8.c (blocked-thread-kill walk). Builds on Q8.a/b.
- **Phase 2 BUILD-4 or BUILD-5 sub-commit:** ADR-016 A2 (scheduler heap-decoupling). Independent of the SMP retrofit; aligns with the buddy+slab allocator phase work.

After Phase 2 closes, Phase 3 (NeoFS) and Phase 4 (network stack) come before the AI-native phases (5+). But the capability-passing-under-SMP mechanism that Phase 5 needs is fundamentally the work SC-3B delivers.

---

## 8. Recommended next moves

1. **Project lead ratifies scope:** confirms Option (ii) (SC-3B = Q4 + Q6 + IPC retrofit + scenarios), or specifies a deviation.
2. **PM drafts SC-3B PLAN-stage checkpoint dispatch:** addresses the eight open questions in §6 above. Cerebro produces a PLAN-stage analysis (likely a Plans/ artifact, not a commit) for project-lead ratification.
3. **Project lead ratifies PLAN:** confirms ABI shape, scenario set, verification methodology, etc.
4. **PM drafts BUILD dispatch:** Cerebro implements per the ratified plan. Probably 3-5 commits within the sub-commit boundary.
5. **Verification:** cargo-driven loop at NEO_SMP=4 covering the new scenarios + anti-regression on existing scenarios.
6. **Close report + INDEX sweep:** after verification.

Total elapsed estimate: 1-2 weeks from THINK ratification to sub-commit close. Pacing similar to BUILD-3 sub-commit 1b/2.

---

## 9. Cross-references

- `docs/ADR-013-phase-2-smp-retrofit.md` §Q4, §Q6, §Q7, §Q8 — substrate.
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` — input note, retitled implicitly as SC-3B input.
- `docs/bible.md` Articles 1.2, 1.3, 8.3, 8.5, 12.1, 14 — governing posture.
- `docs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001) §3.7, §4.6, §5, §7.6, §10.2 — AI-native primitives that reuse this work.
- `Plans/ALTIMIT-real-world-implementation.md` §A.2, §A.5, §G.1 — ALTIMIT posture analysis.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — commitment now met, no longer gating SC-3B.
- `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` — formal discharge.

---

**End of THINK-stage note.**

*Recommendation: SC-3B scope = ADR-013 Q4 + Q6 + IPC retrofit + cross-CPU scenarios, multi-commit within one sub-commit. SC-3A retired as a name; pre-thinking note becomes SC-3B PLAN input. Three-lens convergence (Bible, ALTIMIT posture, AI-native premise) points at the same scope. Awaiting project-lead ratification to draft the PLAN-stage checkpoint dispatch.*
