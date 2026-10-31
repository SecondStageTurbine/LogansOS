<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-34 threaded-matmul / SMP-deadlock — prior-art trap-mining → Prometheus

**Date:** 2026-07-19
**From:** PM (Claude)
**To:** Prometheus (Qwen open-weight — research / trap-mining seat; this is squarely in scope: research + prior-art, doc output, NO kernel, NO QEMU gate)
**Status:** DISPATCHED — a **research/trap-mining** dispatch. Output is additions to the concurrency-pitfall bank (`docs/concurrency-pitfalls.md`), not code. It **pre-inoculates** the future OT-34 fix (which is a separate main-tier, kernel-touching dispatch) so that build spends its iterations on our genuinely-novel seams, not on classes the field already solved.

## The concrete failure this mines against
On the 12-core bare-metal scheduler smoke test (Cerebro's OT-28 bare-metal run, 2026-07-19), enabling threaded-matmul (OT-34) **deadlocked**: the **AP idle-hook parks APs before cross-CPU scheduling finishes → the run hangs at the spawned hog**. Cerebro tried it and reverted. This is a real scheduler/AP-parking race at our SMP seam, NOT a matmul-math bug. The fix is future main-tier work; your job is to make sure that fix arrives already-armed against every *documented* trap in this class.

## What to mine (read for PRINCIPLE, harvest the trap + its discriminator)
The PL's standing insight (2026-07-12): these SMP bugs are largely already found and squashed in other systems — we *do* have the answer out there at the level of **pattern/discipline** (not drop-in code). Mine the documented field gotchas for **this exact class** — AP bring-up / idle-hook parking / cross-CPU scheduler hand-off / IPI wakeup / run-queue vs idle-loop races — from primary sources:
- **Intel SDM** Vol.3 (MP init / INIT-SIPI-SIPI, inter-processor interrupts, HLT/MWAIT wake semantics, memory ordering around IPIs).
- **Linux** scheduler + SMP bringup (`kernel/sched/idle.c`, `smpboot.c`, `kernel/smp.c`, the `cpu_idle` / `sched_ttwu_pending` / `resched_curr` paths — read for the *ordering discipline*, the "why", the barriers).
- **BSD** (FreeBSD `sched_ule` / `ipi` / `cpu_idle`, illumos idle-thread/`poke_cpu`) for a second independent lineage.
- **OSDev** wiki (SMP, APIC, "sleeping/waking CPUs", the classic "idle CPU misses a wakeup IPI" pitfall) and seL4/kernel-mailing-list write-ups on scheduler-wake races.

**Hard rule — MPL kernel hygiene (CLAUDE.md, OT-1):** read these for **principle only**. Do **NOT** copy GPL/copyleft (Linux/BSD) code, structure, or comments into our MPL-2.0 kernel — not now (doc) and not as a template for the future fix. Cite the source for the *idea*; express the trap in our own words, mapped to our own primitives.

## The deliverable — new CP-bank entries (`docs/concurrency-pitfalls.md`)
Extend the normative bank (currently CP-1…CP-18) with the mined traps — number them consecutively (CP-19, CP-20, …). For **each** trap, one entry with:
1. **The trap** — the race/hazard, in our own words (e.g. "an idle AP that HLTs after the scheduler marks it idle but before the wakeup IPI is delivered sleeps forever if the IPI-vs-idle-flag ordering is wrong").
2. **Where it bites us** — map it to OUR seams: the AP idle-hook, the cross-CPU spawn/schedule hand-off (the exact place Cerebro's hang lives), the `resched`/park path, the IPI wake. Name the file/function if you can locate it (RAG/grep the scheduler).
3. **The discriminator** — the observation that tells you THIS trap is firing vs another (the breadcrumb / state to capture on a hang — e.g. "target AP's idle flag set AND no pending-wake bit AND HLT entered = the missed-IPI race").
4. **The guard / discipline** — the ordering or barrier that prevents it (the principle, from prior art), stated so the future fix can implement it in our primitives.
5. **Source** — the primary reference for the principle (SDM section, or the concept from Linux/BSD/OSDev), cited for the *idea*, not copied.

Also add a short **framing note** at the top of the new block: the OT-34 concrete failure (AP-parks-before-cross-CPU-schedule-finishes) and which of the new CP entries directly cover it, so the future fix-dispatch reads the map at a glance.

## Bounded honesty (state this in the RETURN)
The answer is out there for the **principle**, not drop-in code (prior-art fixes are welded to their host's primitives; our bug lives at *our* spawn/idle seam). Patterns-known ≠ correctness-proven (the seL4 lesson) — the bank **shortens the search**, it does not replace the future fix's QEMU gate / AAG. Flag honestly which traps you're confident map to our seam vs which are "possible, verify at fix-time."

## Gate + close (research/doc — NO QEMU gate)
- This is a **docs/`Plans/` change** (`docs/concurrency-pitfalls.md`) — CI paths-ignores `docs/**`, so **no push-CI, no QEMU gate** fires, and none is needed. The close is: the CP-bank additions committed + pushed, the RETURN listing the new CP numbers + which cover the OT-34 failure + the honesty flags.
- SPDX: the doc already carries its CC-BY header; keep it.
- **Disk reclaim:** doc-only, no `target/` to reclaim; standard branch cleanup after the PM confirms merge.

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-19-ot34-smp-deadlock-trapmining-prometheus.md`. Also read the existing bank to match its shape: `git show origin/main:docs/concurrency-pitfalls.md` (CP-1…CP-18), and `git show origin/main:docs/complexity-governance.md` (I-14 subsystem fault-containment context). PM artifacts live on `origin/main`; your tree is on a feature branch, so `git show`, not a local file-open.
- **One-line:** Mine the **documented** AP-parking / idle-hook / cross-CPU-scheduler-handoff / IPI-wake traps (Intel SDM + Linux/BSD/OSDev — **read for principle, NEVER copy GPL into our MPL kernel**) and add them to the CP bank as CP-19+, each with trap + where-it-bites-our-seam + discriminator + guard + source, framed against Cerebro's concrete OT-34 failure (AP idle-hook parks APs before cross-CPU scheduling finishes → hang at the spawned hog). This pre-inoculates the future OT-34 fix; it is NOT the fix. Bounded honesty: principle-known ≠ correctness-proven; flag confident-vs-verify-at-fix-time mappings.
- **RAG:** use it to locate OUR scheduler seams for the "where it bites us" mapping — health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first; queries e.g. `AP idle hook park`, `cross-CPU schedule handoff spawn`, `reschedule IPI wake`, `idle loop HLT`. If down/degraded (it was `ollama:false` earlier), note it in your RETURN and use grep/Read — never a blocker; the PL owns RAG startup.
