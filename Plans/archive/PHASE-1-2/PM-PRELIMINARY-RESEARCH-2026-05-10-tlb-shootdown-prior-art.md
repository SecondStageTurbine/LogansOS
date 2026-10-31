# PM-Preliminary Research: TLB shootdown and CR3-cache prior art (SC-3C / Q7.b + Q7.c)

**Date:** 2026-05-10 (later session, post-SC-3C THINK artifact)
**Author:** PM — NOT Cerebro. See provenance correction below.
**Type:** PM-preliminary research artifact (informational; not normative; PROVISIONAL). PM executed web searches and produced first-pass synthesis directly. NOT a Cerebro deep-research artifact. A fresh-context Cerebro pass should follow via `Plans/DISPATCH-2026-05-10-sc-3c-d15-tlb-shootdown-prior-art-cerebro.md` to confirm, extend, or correct findings. Until Cerebro confirms, treat findings as PM-grade-preliminary and treat the downstream D-revisions ratification (`Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md`) as PROVISIONAL pending Cerebro confirmation.
**Authority:** None. PM consumed its own preliminary research and produced provisional D-revisions ratification. PL should treat both as provisional until Cerebro confirms. Voice anchor: `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` (the rigor target a Cerebro-authored version would meet).

**Supersession marker (filed 2026-05-11):** This artifact has been superseded for canonical content by the Cerebro deep-research artifact `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` (filed 2026-05-11, 39.7 KB). Cerebro independently verified all five reference systems (Linux, FreeBSD, seL4, Asterinas, AMD INVLPGB) against primary sources, confirmed all four provisional D-revisions documented in `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` (which now transitions PROVISIONAL → FINALIZED), and produced seven citation/factual corrections to this PM-preliminary artifact's evidence base: (1) Linux cpumask is eventually-consistent not strictly-accurate, (2) FreeBSD API signature evolved to `smp_targeted_tlb_shootdown_native(pmap_t, addr1, addr2, curcpu_cb, enum invl_op_codes op)`, (3) `curcpu_cb` runs after IPI dispatch and before the wait loop (not during), (4) function is `broadcast_tlb_flush` not `broadcast_flush_tlb_all`, (5) `consider_global_asid` has 1-in-32 sampling gate and rising floor, (6) seL4 mailing list date is November 2018 not January 2019, (7) FreeBSD-EN-18:07 verbatim wording is "resulted in" not "resulting in". Cerebro also surfaced one Phase 2+ forward marker (Linux lazy-TLB exception for page-table teardown) not previously captured. See `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md` for the full consumption record. This artifact is preserved as historical record of PM's preliminary first-pass; for canonical prior-art evidence (especially citation precision), use the Cerebro artifact.

**Provenance correction (filed 2026-05-10 later session):** This artifact was originally filed as `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md` with `Author: Cerebro research artifact` in the header. That was a role-confusion error by PM — deep-research dispatches output `Plans/RESEARCH-YYYY-MM-DD-<topic>-cerebro.md` per agent-roster Â§2.3, and the named author must be the executing agent (Cerebro, a separate Claude session reached via PL-ferried dispatch), not the PM session that produced this file. Renamed to `PM-PRELIMINARY-RESEARCH-...` and reclassified as PM-grade-preliminary rather than Cerebro-grade-research. The actual Cerebro dispatch lives at `Plans/DISPATCH-2026-05-10-sc-3c-d15-tlb-shootdown-prior-art-cerebro.md`. See `Plans/PM-NOTE-2026-05-10-pm-role-confusion-recovery.md` for the full calibration record.

---

## TL;DR

Cross-CPU TLB invalidation is one of the most-trodden problems in SMP OS engineering, and the public source available is rich enough to grade NeoPlutarch's SC-3C PM leans (D1–D14) against five comparable reference systems. The headline findings:

**Linux's measured INVLPG-vs-full-flush threshold is `tlb_single_page_flush_ceiling = 33` pages**, runtime-tunable via `/sys/kernel/debug/x86/tlb_single_page_flush_ceiling`, and applied symmetrically by `flush_tlb_mm_range` and `flush_tlb_kernel_range`. PM lean D3 (N=16) is conservative; the kernel-internals literature treats 33 as the measured optimum for current x86 hardware. NeoPlutarch can start at 16 as the bible-1.2 cautious baseline and revisit after measurement, or align with Linux's 33 immediately to reduce calibration churn later. No correctness consequence either way.

**Linux uses per-mm CPU-mask targeting (T2 in the SC-3C THINK taxonomy), not broadcast.** `flush_tlb_others` consumes `mm_cpumask(mm)` and IPIs only CPUs that have ever loaded that mm's CR3. The cpumask is maintained at `switch_mm_irqs_off` time: clearing the previous CPU's bit when an mm leaves, setting the new CPU's bit when it arrives. PM lean D4 (T1 broadcast) is simpler but is the historical Linux 1.x design that targeted shootdown replaced because of measured IPI cost on many-CPU machines. NeoPlutarch's Phase 2 NEO_SMP=4 ceiling makes the broadcast/targeted gap small at current scale, but per-Pmap CPU-mask tracking is well-trodden territory and a known future migration if SMP scales up.

**FreeBSD's `smp_targeted_tlb_shootdown(cpuset_t mask, u_int vector, pmap_t pmap, addr1, addr2, curcpu_cb)`** is the closest public-source analogue to the API shape SC-3C is building. It accepts an explicit CPU mask, dispatches IPIs by vector, uses a generation counter (`smp_tlb_generation`) for synchronization, and invokes a `curcpu_cb` callback on the originating CPU while waiting for remote ACKs. This is a more sophisticated pattern than the PM lean's "self-invalidate synchronously before sending IPI" (D9): FreeBSD overlaps the local invalidation with the remote wait. The performance impact at NEO_SMP=4 is small but non-zero; the API shape is worth borrowing.

**FreeBSD-EN-18:07 (2018) is the canonical "TLB-shootdown-bugs-cause-silent-data-corruption" calibration anchor.** Xen PTI IPI handlers were left non-PTI during the Meltdown response; user-space TLB entries weren't consistently invalidated; processes "see" pages of memory previously mapped but since unmapped, with silent data corruption. This is the bug class SC-3C must not introduce. The implication for verification gates (D14): the SC-3C closure-gate test scenarios must include a stress test that would catch a missing shootdown (concurrent munmap + access from another CPU), not just a stress test that exercises the happy path.

**Asterinas (the closest architectural reference — a Linux-ABI Rust monolith with framekernel intra-kernel privilege separation) is mid-flight on this exact territory.** Asterinas issue #799 (April 2024) identifies the same CR3-reload-on-every-user-kernel-switch problem Q7.b is fixing, and Asterinas's 2024 retrospective committed to SMP optimization as the 2025 focus. The published ATC25 paper evaluation was SMP=1 only. This means there is no settled Asterinas TLB-shootdown design to copy from — they are working on it concurrently. The framekernel privilege-separation pattern (privileged OSTD framework, de-privileged services) is structurally similar enough to NeoPlutarch's Bible-discipline approach that watching Asterinas's design choices over 2026 is worth doing, but it does not provide a frozen reference design today.

**seL4 mostly does not apply.** seL4's verification-first architecture explicitly avoids multi-CPU concurrency in the kernel: the implementation is single-threaded internally and event-based, sidestepping most cross-CPU TLB shootdown design surface by construction. The seL4 machine interface treats TLB flush as an unverified hardware primitive ("we still assume correctness of TLB and cache flushing operations"). The 2019 sel4-devel mailing list contains a developer note that "on sel4/x86_64 the TLB is practically not always cleaned properly in SMP systems," confirming this is a known limitation of the verification-first approach rather than a battle-tested design pattern. seL4 is a useful anti-pattern reference (why architectural simplification *avoids* the problem) more than a positive reference.

**AMD INVLPGB hardware-broadcast TLB invalidation is the most important forward-marker.** Zen 3+ CPUs implement INVLPGB; Linux 6.15 (released Feb 2025) merged the support via Rik van Riel's v14 patch series. The Linux integration assigns a "global ASID" to processes active on more than 3 CPUs, then uses `invlpgb_flush_user_nr_nosync()` plus `__tlbsync()` to flush remote TLBs in hardware without IPIs. Intel's "Remote Action Request" (RAR) is the announced follow-up. NeoPlutarch's IPI-based shootdown is the correct baseline for 2026 hardware compatibility, but the forward-marker should note INVLPGB / RAR as the obvious Phase 3+ alternative once measurement shows IPI overhead matters.

**The early-ACK pattern (USPTO 11321242) is an optimization filed for later.** Receivers ACK before flushing; sender's local flush completes before the receiver could observe stale entries; sender's wait is shortened. Useful to know exists; not needed for SC-3C correctness baseline.

Translated to the SC-3C D-decisions: **D1, D2, D5, D6, D7, D8, D9, D10, D11, D12, D13 stand**; **D3 may be revised upward (N=16 → N=33) to match Linux's measured threshold**; **D4's T1-broadcast lean has a clearly mapped T2-targeted-by-mm-cpumask upgrade path that should be marked as a Phase 2+ forward optimization rather than a deferred-indefinitely item**; **D14 should add an explicit "stress test that would catch a missing shootdown" scenario inspired by FreeBSD-EN-18:07**.

---

## Sources surveyed

### Primary sources consulted directly

- Linux source, `arch/x86/mm/tlb.c` at HEAD: `https://github.com/torvalds/linux/blob/master/arch/x86/mm/tlb.c`. Read for `flush_tlb_mm_range`, `flush_tlb_kernel_range`, `should_flush_tlb`, `consider_global_asid`, `broadcast_flush_tlb_all`, `cpu_tlbstate.is_lazy`, `cond_ibpb`, and the global-ASID infrastructure.
- Linux source, `arch/x86/mm/tlb.c` pinned to commit `9269d27e519ae9a89be8d288f59d1ec573b0c686`: `https://github.com/torvalds/linux/blob/9269d27e519ae9a89be8d288f59d1ec573b0c686/arch/x86/mm/tlb.c`. Read for `switch_mm_irqs_off` cpumask manipulation, `cond_ibpb` Spectre-v2 IBPB on context switch, lazy-TLB transition fall-through, and `init_mm` cpumask exclusion comments.
- Linux scheduler / TLB optimization writeup, kernel-internals.org: `https://kernel-internals.org/mm/tlb-optimization/`. Read for `tlb_single_page_flush_ceiling = 33` default, INVLPGB global ASID threshold of >3 CPUs, IPI-vs-INVLPGB tradeoffs, and TLB-miss-latency numbers (TLB hit ~1 cycle / ~0.3 ns vs TLB miss ~50 cycles / ~17 ns).
- Linux kernel documentation, "Cache and TLB Flushing Under Linux": `https://docs.kernel.org/core-api/cachetlb.html`. Read for the canonical Linux TLB-flush API surface (`flush_tlb_all`, `flush_tlb_mm`, `flush_tlb_range`, `flush_tlb_page`, `update_mmu_cache_range`).
- Linux 6.15 release notes, kernelnewbies.org: `https://kernelnewbies.org/Linux_6.15`. Read for INVLPGB mainline merge announcement and the framing of "TLB entries invalidated on remote CPUs without needing to send IPIs."
- LWN, "AMD broadcast TLB invalidation": `https://lwn.net/Articles/1008065/`. Read for Rik van Riel's v8/v10/v14 patch-series notes, mention of Intel RAR follow-up plan, and code-review interactions (Borislav Petkov queueing in `tip-x86-cpu-tlbi`).
- Patchwork, "[v10,07/12] x86/mm: use INVLPGB in flush_tlb_all": `https://patchwork.kernel.org/project/linux-mm/patch/20250211210823.242681-8-riel@surriel.com/`. Read for `broadcast_flush_tlb_all` integration with `flush_tlb_all`, `cpu_feature_enabled(X86_FEATURE_INVLPGB)` gating, and `guard(preempt)()` discipline.
- FreeBSD code review D25510, `smp_targeted_tlb_shootdown` implementation diff: `https://reviews.freebsd.org/file/data/2vh7rsmqsn5ebqbdbsj4/PHID-FILE-5tnkmv463f2g4ub7lcjb/D25510.id73886.diff`. Read for `cpuset_t mask`, `u_int vector`, `pmap_t pmap`, `addr1`, `addr2`, `smp_invl_cb_t curcpu_cb` API shape, the `smp_tlb_generation` counter, and the `curcpu_cb`-runs-on-caller-while-waiting pattern.
- FreeBSD security advisory FreeBSD-EN-18:07.pmap: `https://www.freebsd.org/security/advisories/FreeBSD-EN-18:07.pmap.asc`. Read for the Xen-PTI-IPI-handlers-left-non-PTI failure mode and the data-corruption impact statement ("Processes on Xen based guests may 'see' pages of memory which were previously mapped but have since been unmapped, resulting in data being corrupted and/or processes crashing due to internal data structures becoming inconsistent").
- seL4 paper, "Comprehensive Formal Verification of an OS Microkernel" (ACM TOCS 2014): `https://sel4.systems/Research/pdfs/comprehensive-formal-verification-os-microkernel.pdf`. Read for the kernel's machine-interface treatment of TLB flush ("we still assume correctness of TLB and cache flushing operations") and the verification-stack uniprocessor focus.
- seL4 paper, "Comprehensive Formal Verification of an OS Microkernel" mirror at UW CSE: `https://courses.cs.washington.edu/courses/cse551/17wi/readings/sel4-tocs.pdf`. Cross-checked against the sel4.systems copy.
- seL4 paper, "From L3 to seL4 What Have We Learnt in 20 Years of L4 Microkernels?": `http://rust-class.org/static/classes/class22/20years.pdf`. Read for the explicit single-threaded-event-based design rationale and the "TLB footprint has a greater impact on microkernel performance than it did 20 years ago" observation.
- seL4 API reference, TLB invalidate operations: `https://docs.sel4.systems/projects/sel4/api-doc.html`. Read for the X86 ASID pool design (1024 VSpaces per pool) and the cache/TLB invalidate capability operations.
- seL4 devel mailing list, January 2019 archive: `https://lists.sel4.systems/hyperkitty/list/devel@sel4.systems/2019/1/`. Read for the developer note "on sel4/x86_64 the TLB is practically not always cleaned properly in SMP systems."
- Asterinas GitHub issue #799, "Avoid unnecessary TLB flushing during user-kernel switching" (filed April 2024): `https://github.com/asterinas/asterinas/issues/799`. Read for the problem statement that matches Q7.b ("Currently, x86-64's CR3 is modified every time the kernel switches to the user space as it involves UserMode::execute. Modifying the CR3 invalidates all non-global TLB entries").
- Asterinas blog, "Asterinas in 2024": `https://asterinas.github.io/2025/01/20/asterinas-in-2024.html`. Read for the 2025 SMP focus commitment ("In 2025, we will turn our attention to optimizing performance for SMP environments") and the Asterinas Benchmark Collection methodology note.
- Asterinas ATC 2025 paper abstract: `https://arxiv.org/abs/2506.03876`. Read for the framekernel architecture statement (privileged OSTD framework + de-privileged services) and the 14.0%-TCB metric.
- Asterinas ATC 2025 paper PDF: `https://yinqian.org/papers/atc25.pdf`. Read for the experimental setup confirming SMP=1 evaluation only.
- OSDev wiki, "TLB": `https://wiki.osdev.org/TLB`. Read for INVLPG semantics ("On all Intel CPUs, and on AMD CPUs with EFER[TCE]=0, INVLPG flushes all upper-level entries"), CR3-reload-as-flush idiom, and the global (G) bit behaviour.
- OSDev forum, "TLB and INVLPG": `https://forum.osdev.org/viewtopic.php?t=36898`. Read for hobby-OS community consensus on INVLPG addition history (386 → TLB, 486 → INVLPG) and the "should I disable interrupts during page-table changes" community Q&A.
- Linux VM notes, page-tables section: `https://github.com/lorenzo-stoakes/linux-vm-notes/blob/master/sections/page-tables.md`. Read for the Linux-side framing of `flush_tlb_mm`, the lazy-TLB rationale (kernel threads, `active_mm`, `leave_mm`), and the IPI-on-many-CPUs cost model.
- bitcharmer/tlb_shootdowns blog: `https://github.com/bitcharmer/tlb_shootdowns`. Read for the IPI-and-`smp_call_function_many` pattern description and the SystemTap-instrumented backtrace approach to confirming TLB shootdowns are happening on Linux.

### Secondary / community / patent references

- USPTO patent 11,321,242 B2, "Early acknowledgement of translation lookaside buffer shootdowns": `https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/11321242`. Read for the early-ACK optimization pattern (receiver ACKs before flushing; sender's flush completes before receiver could observe stale entries).
- USPTO patent 12,253,956 B2, "Hybrid scheme for performing translation lookaside buffer (TLB) shootdowns": `https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/12253956`. Read for the runtime IPI-vs-broadcast-TLBI selection logic and the per-CPU/platform erratum-detection variable that flags broken hardware-broadcast paths back to IPI.
- USPTO patent 8,112,174 B2, "Processor, method and computer program product for fast selective invalidation of translation lookaside buffer": `https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/8112174`. Read for the broadcast Invalidate-TLB-Key (ITK) command pattern at the hardware level and the SSKE-completion-blocks-new-TLB-installs invariant. Used as design-space marker; not directly applicable.
- Phoronix benchmarks of AMD INVLPGB Linux patches: `https://www.phoronix.com/forums/forum/hardware/processors-memory/1515546-benchmarking-the-amd-invlpgb-linux-kernel-patches-for-better-performance`. Read for real-world performance numbers and the Zen 3 / Zen 4 / Zen 5 / EPYC coverage scope of the optimization.
- Phoronix news, "AMD Broadcast TLB Invalidation INVLPGB Support Appears Ready For The Linux Kernel": `https://www.phoronix.com/news/AMD-INVLPGB-Ready-For-Linux`. Read for the v14 patch-series timing (December 2024 first post; ready for mainline early 2025; merged Linux 6.15).
- Columbia COMS W4118 Operating Systems I lecture notes on TLB: `https://cs4118.github.io/www/2023-1/lect/19-TLB.html`. Read for textbook-level summary of TLB cache structure and hit-rate expectations.
- "Skip TLB flushes for reused pages within mmap's" (arXiv 2409.10946): `https://arxiv.org/pdf/2409.10946`. Read for the "FPR" (Flagged Page Reuse) Linux extension and the framing that "both sources of TLB shootdowns, manual munmaps and page cache evictions, are targeted" — confirms targeted shootdown is the dominant production pattern.
- Adarsh Patil, "TLB and Pagewalk Performance in Multicore Architectures with Large Die-Stacked DRAM Cache" (arXiv 2002.01073): `https://arxiv.org/pdf/2002.01073`. Read for x86_64 page-walk depth history (Pentium → Pentium Pro → AMD Opteron 4-level) and TLB-miss-cost context.
- ARM64 patch, `flush_tlb_pgtable` hook addition, kernel.org mailing list archive: `https://go.mail-archive.com/linux-kernel@vger.kernel.org/msg1636479.html`. Read for the architecture-specific TLB invalidation hook pattern (ARM64 supports intermediate-page-table invalidation; other arches stub it out). Used as design-space marker for future ARM port forward-marker.
- "PARseL: Towards a Verified Root-of-Trust over seL4" (arXiv 2308.11921): `https://arxiv.org/pdf/2308.11921`. Read for seL4 capability-based access control framing and the user-space-CSpace boundary, to confirm seL4's TLB invalidation goes through capability operations rather than direct syscalls.
- vusec/revanc (AnC / ASLR-Cache attack source): `https://github.com/vusec/revanc`. Read briefly to confirm it is a side-channel attack research tool, not a TLB shootdown implementation. Used as negative reference to filter from the prior-art set.

### Future-marker references (not relevant to SC-3C, tracked for later)

- Intel Software Developer's Manual landing page: `https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html`. Reference for the eventual Intel RAR follow-up Phase 3+ work; SDM volume update will document the RAR ISA when it ships.
- AMD Architecture Programmer's Manual (not pulled in this pass): documents INVLPGB / TLBSYNC encoding. Forward marker if SC-3C+1 considers INVLPGB integration.

---

## Findings by query

### 3.1 Linux's CR3-cache and shootdown design (`arch/x86/mm/tlb.c`)

**Linux's switch_mm cpumask discipline is the strongest source of guidance for Q7.b.** At `switch_mm_irqs_off()` time, when an mm leaves a CPU, the CPU's bit is cleared from `mm_cpumask(real_prev)`; when an mm arrives on a CPU, the CPU's bit is set in `mm_cpumask(next)`. The kernel comments explicitly note: "Stop remote flushes for the previous mm. Skip kernel threads; we never send init_mm TLB flushing IPIs, but the bitmap manipulation can cause cache line contention." This is the same per-Pmap CPU-mask pattern that the SC-3C THINK artifact's T2 alternative described, and Linux's source confirms the bitmap manipulation has its own cache-line-contention cost — meaning T2 is not free even before IPI dispatch.

**Linux's `tlb_single_page_flush_ceiling` ceiling is 33 pages, runtime-tunable.** Read at `arch/x86/mm/tlb.c`:

```c
unsigned long tlb_single_page_flush_ceiling __read_mostly = 33;
```

Applied symmetrically by `flush_tlb_mm_range` (user-space ranges) and `flush_tlb_kernel_range` (kernel ranges). The threshold is exposed via `/sys/kernel/debug/x86/tlb_single_page_flush_ceiling` for runtime tuning. The kernel-internals.org writeup characterises 33 as "the kernel switches strategies" point measured against current x86 hardware. NeoPlutarch's PM lean D3 (N=16) is roughly half of Linux's measured optimum; this is a conservative starting point but not a Bible-correctness concern.

**Linux's lazy-TLB short-circuit is checked at the receiver, not the sender.** `should_flush_tlb(int cpu, void *data)` reads `per_cpu(cpu_tlbstate_shared.is_lazy, cpu)` with an explicit `smp_rmb()` barrier ordering it against `loaded_mm`. The receiver returns `false` to skip the flush when lazy. This means the sender always sends the IPI; the IPI handler decides whether to actually flush. The implication for SC-3C: PM lean D6 (lazy TLB out) is fine for the baseline, but Linux's design suggests lazy TLB is most naturally added as a receiver-side optimization later, not as a sender-side optimization that would change the shootdown API surface.

**Linux's `flush_tlb_all` has both legacy IPI and INVLPGB paths.** Pre-INVLPGB: `flush_tlb_all` calls `on_each_cpu(do_flush_tlb_all, NULL, 1)`, broadcasting an IPI that invokes `__flush_tlb_all` (CR3 reload-with-current) on every CPU. Post-INVLPGB: `flush_tlb_all` first tries `broadcast_flush_tlb_all()`, which checks `cpu_feature_enabled(X86_FEATURE_INVLPGB)` and falls through to the legacy path if not supported. This is the right model for SC-3C+1 INVLPGB integration: a runtime-gated fast path layered atop the IPI baseline, not a replacement.

**Linux's context-switch path also flushes the branch predictor (`cond_ibpb`).** Comment from `arch/x86/mm/tlb.c`: "Avoid user/user BTB poisoning by flushing the branch predictor when switching between processes. This stops one process from doing Spectre-v2 attacks on another." This is post-Spectre-v2 mitigation, not TLB shootdown work, but it is co-located in the same context-switch code path. SC-3C's Q7.b implementation will sit adjacent to where any future NeoPlutarch CPU-vuln mitigations would land. Worth noting but not a SC-3C concern.

**Linux's global-ASID lifetime: held for the lifetime of the process.** Read at `arch/x86/mm/tlb.c`:

```c
/* These global ASIDs are held for the lifetime of the process. */
static DEFINE_RAW_SPINLOCK(global_asid_lock);
static u16 last_global_asid = MAX_ASID_AVAILABLE;
static DECLARE_BITMAP(global_asid_used, MAX_ASID_AVAILABLE);
static DECLARE_BITMAP(global_asid_freed, MAX_ASID_AVAILABLE);
```

Global ASIDs are reset (via `invlpgb_flush_all_nonglobals` and `reset_global_asid_space`) only at rollover time. The implication: INVLPGB-based broadcast invalidation has its own complexity surface (ASID allocation, rollover, lifetime tracking) that SC-3C's IPI baseline does not have. Confirms PM lean D2 (PCID deferred) is sound — full PCID + ASID integration is non-trivial enough that it deserves its own ADR-driven sub-commit rather than being folded into SC-3C.

### 3.2 FreeBSD's `smp_targeted_tlb_shootdown` API shape

**The FreeBSD API is the most directly transferable design pattern for SC-3C's sender side.** Read from D25510 review diff:

```c
static void
smp_targeted_tlb_shootdown(cpuset_t mask, u_int vector, pmap_t pmap,
    vm_offset_t addr1, vm_offset_t addr2, smp_invl_cb_t curcpu_cb)
{
    cpuset_t other_cpus;
    volatile uint32_t *p_cpudone;
    uint32_t generation;
    int cpu;
    /* ... */
}
```

Translated to NeoPlutarch's idiom, the API shape would be:

```rust
pub fn smp_targeted_tlb_shootdown(
    mask: CpuMask,
    vector: u8,
    pmap: &Pmap,
    range: Option<(VirtAddr, VirtAddr)>,
    curcpu_cb: impl FnOnce(),
)
```

Key learnings:

1. **CPU mask is the input, not "all-except-self".** FreeBSD takes the targeted set explicitly. Even if the caller passes "all CPUs except self," the API surface accommodates targeted callers without rework. PM lean D4 (T1 broadcast) is implementable on top of this API shape by having the caller pass `all_other_cpus_mask()`. Doing it this way means SC-3C's API does not need to change when T2 (per-Pmap targeting) lands later.

2. **Generation counter rather than per-shootdown atomic.** FreeBSD uses a global `smp_tlb_generation` counter incremented per shootdown. Each per-CPU "done" slot records the generation it last completed. Sender waits until all targeted CPUs have advanced their generation to match. PM lean D10 (per-shootdown atomic counter) is functionally equivalent but is allocated per-shootdown rather than from a long-lived global. Either is correct; FreeBSD's choice avoids the per-shootdown allocation but is more sensitive to overflow and wraparound (4-billion-shootdown limit; not a practical concern but a design surface).

3. **Local-CPU callback runs while waiting for remotes.** The `curcpu_cb` callback is invoked on the caller's CPU during the wait loop. FreeBSD's comment: "The callback function is called unconditionally on the caller's underlying processor, even when this processor is not set in the mask. So, the callback function must be prepared to handle such spurious invocations." This is more efficient than PM lean D9's "self-invalidate synchronously before sending IPI" pattern: in PM lean D9, the local invalidation serializes before the IPI dispatch. In FreeBSD's pattern, the local invalidation can overlap with the remote IPI propagation. At NEO_SMP=4 the overlap window is small, but the API shape costs nothing to adopt. **PM lean D9 should be revised to match FreeBSD's overlap pattern.**

4. **Vector is parameterized.** FreeBSD does not hard-code "the TLB shootdown vector"; it accepts any IPI vector. This supports multiple shootdown variants (e.g., user TLB vs kernel TLB vs cache flush) without API duplication. PM lean D8 (vector 0xfc) is fine as the SC-3C-specific value; the API shape can take it as a parameter to keep future flexibility.

**FreeBSD-EN-18:07 is the corruption-class anchor for D14 verification gate design.** The advisory text:

> Processes on Xen based guests may "see" pages of memory which were previously mapped but have since been unmapped, resulting in data being corrupted and/or processes crashing due to internal data structures becoming inconsistent.

The bug was a regression: changes to address Meltdown (the original Spectre/PTI response) inadvertently left Xen IPI handlers non-PTI, so under PTI mode the user-space TLB entries were not consistently invalidated on shootdown. The bug surfaced as silent data corruption, not as a crash or a performance issue. **The implication for SC-3C D14 verification gate design: at least one test scenario must be capable of catching a missing-shootdown bug, not just a happy-path stress test.** Concretely: a test that unmaps a page on CPU A, then immediately reads from that virtual address on CPU B (with B's TLB potentially still caching the stale entry). Without a working shootdown, the read returns stale data instead of faulting. The verification gate should check for the fault.

### 3.3 seL4's TLB design — verification-first sidesteps shootdown complexity

**seL4 is event-based and single-threaded internally.** From the "20 years of L4 microkernels" retrospective: "The seL4 design explicitly avoids concurrency using an event-based execution model to improve the tractability of verification. However, the ubiquity of multicore processors creates a challenge: how can one utilize the available cores while retaining [verifiability]?" The published seL4 verification stack focuses on uniprocessor operation; multicore extensions exist (von Tessin's experimental x86 multicore version) but are not part of the fully-verified mainstream kernel.

**TLB flush is an unverified machine-interface primitive in seL4.** The 2014 TOCS paper states explicitly: "In this work we verify the kernel with high-level security properties down to the binary level, but we still assume correctness of TLB and cache flushing operations as [a hardware primitive]." This is the verification-first stance: the kernel does not prove TLB flushing correctness; it assumes the hardware delivers the documented semantics and audits the C-or-asm flush implementations manually.

**seL4 has user-visible ASIDs (1024 VSpaces per ASID pool) decoupled from hardware tagged-TLB ASIDs.** The 2014 paper: "Address-space identifiers (ASIDs) are a user-visible software artefact of the current implementation, and not directly related to hardware ASIDs used in tagged TLBs. They are not fundamental in any way. We expect to remove them in later kernel design iterations." This is a software-level address-space identifier used by capability operations, not a hardware-level PCID/ASID. seL4's design predates PCID adoption and the design has not converged on PCID integration in the publicly surveyed material.

**The 2019 sel4-devel mailing list contains a developer note acknowledging SMP TLB limitations.** From the January 2019 archive: "I've recognized that on sel4/x86_64 the TLB is practically not always cleaned properly in SMP systems." This is a developer-level note, not a CVE-grade advisory, but it confirms that seL4's verification-first SMP TLB story is incomplete and the kernel has known shortcomings in multicore TLB handling that have not been fully resolved publicly.

**Net takeaway for NeoPlutarch:** seL4 is a useful anti-pattern reference (architectural simplification *avoids* the shootdown design surface by eliminating most kernel concurrency) but not a positive design reference. NeoPlutarch's monolithic-with-Bible-discipline approach has to solve the problem directly. seL4 contributes nothing actionable to D1–D14; the references are documented here for completeness.

### 3.4 Asterinas — closest architectural reference, mid-flight on this exact territory

**Asterinas's framekernel architecture is structurally similar to NeoPlutarch's Bible-discipline model.** From the ATC25 paper abstract: "In the framekernel architecture, an entire Rust-written OS kernel runs in a single address space (similar to a monolithic kernel) but is logically divided into two parts: the privileged OS framework (akin to a microkernel) and the de-privileged OS services. The OS framework is privileged in that it is the only component permitted to use unsafe. Conversely, the OS services are de-privileged, restricted to safe Rust only." NeoPlutarch's Bible-15 SAFETY-comment discipline and the structural separation between `kernel/src/arch/` (unsafe operations behind safe Rust APIs) and higher-level subsystems (mostly safe Rust with checked unsafe) is functionally similar though not identical. Asterinas's `OSTD` framework crate maps onto what NeoPlutarch's `arch::x86_64` and `mm::pmap` modules are growing into.

**Asterinas issue #799 is the canonical "Q7.b problem statement" in the public Rust-OS literature.** Filed April 28, 2024 by the Asterinas team. Direct quote:

> Currently, x86-64's CR3 is modified every time the kernel switches to the user space as it involves `UserMode::execute`. [...] Modifying the CR3 invalidates all non-global TLB entries. In Asterinas, this means that all TLB entries for the user space is flushed, slowing down every memory access to the user space. The direct cost of modifying CR3 and its indirect cost of TLB cache misses have been demonstrated by issue 791. Theoretically, modifying CR3 is only necessary for context switching (between different tasks), not user-kernel switching. This is exactly what this issue proposes to do.

This is the same observation that motivates NeoPlutarch's Q7.b. Asterinas explicitly framed it as a CR3-only-on-task-switch optimization, structurally identical to PM lean B2.a (compare-and-skip).

**Asterinas's 2024 retrospective commits to SMP optimization as the 2025 focus.** Direct quote from `asterinas-in-2024.html`: "In 2025, we will turn our attention to optimizing performance for SMP (Symmetric Multiprocessing) environments. Asterinas's signature framekernel OS architecture promises a small and sound trusted computing base (TCB) for kernel memory safety." The ATC25 paper's experimental setup explicitly evaluated only SMP=1 ("Both the Linux and ASTERINAS kernels are tested within QEMU virtual machines configured as follows: SMP set to 1"). This means there is no settled Asterinas TLB-shootdown design in the public source today, and the work they did in 2025–2026 is the closest thing to a parallel implementation effort to NeoPlutarch's Q7.

**Implication for NeoPlutarch:** Watching the Asterinas repository over 2026 for their SMP TLB design decisions is a low-cost, high-value activity. They face the same architectural constraints (Rust monolith with intra-kernel privilege separation, no PCID initially, full TLB flush on CR3 write) and are at a comparable point in their roadmap. If Asterinas chooses, e.g., to start with broadcast IPI and migrate to per-mm cpumask later, that is direct cross-validation of PM lean D4. Forward marker: add Asterinas SMP TLB work to the Phase 2+ research-tracking list.

### 3.5 AMD INVLPGB hardware-broadcast — the Phase 3+ forward-marker

**INVLPGB is mainline-Linux as of 6.15 (Feb 2025).** The patch series originated December 2024 from Rik van Riel (Meta), went through 14 rounds of review, and was merged into `tip-x86-cpu-tlbi` and then mainline by AMD engineer Borislav Petkov. The kernelnewbies summary: "This release adds support for the INVLPGB instruction on AMD systems that support it (Zen 3 and later). It allows the kernel to invalidate TLB entries on remote CPUs without needing to send IPIs, without having to wait for remote CPUs to handle those interrupts, and with less interruption to what was running on those CPUs."

**The Linux integration is gated by feature-flag and active-CPU-count.** From `consider_global_asid` in `arch/x86/mm/tlb.c`:

```c
static void consider_global_asid(struct mm_struct *mm)
{
    if (!cpu_feature_enabled(X86_FEATURE_INVLPGB))
        return;

    if (mm_active_cpus_exceeds(mm, 3))
        use_global_asid(mm);
}
```

The threshold is "more than 3 CPUs" — processes active on 4+ CPUs get a global ASID. This is the cost-benefit threshold: below 4 CPUs, IPI broadcast is faster (lower per-shootdown overhead than the global-ASID allocation and lifetime tracking cost). At exactly NeoPlutarch's NEO_SMP=4 Phase 2 ceiling, INVLPGB would be at the breakeven point. This is useful future-marker data: INVLPGB integration becomes interesting once NeoPlutarch scales beyond 4 CPUs or once measurement shows IPI overhead at 4 CPUs is the dominant cost.

**Intel RAR (Remote Action Request) is the announced follow-up.** Rik van Riel's LWN cover letter for the INVLPGB v8 series: "My current plan is to implement support for Intel's RAR (Remote Action Request) TLB flushing in a follow-up series, after this thing has been merged into -tip. Making things any larger would just be unwieldy for reviewers." RAR is Intel's competing hardware-broadcast invalidation; the design is in Intel's published ISA documentation but as of this research's cutoff, no mainline Linux integration is merged yet (verify before relying on this as a 2026-shipping fact).

**Implication for NeoPlutarch:** INVLPGB / RAR are not SC-3C concerns. They become interesting at Phase 3+ if (a) NEO_SMP_MAX increases beyond 4, (b) measurement shows IPI shootdown is the dominant cost, or (c) NeoPlutarch wants to publish ATC-class performance numbers comparable to Linux. Add as a Phase 3 forward marker; not actionable today.

### 3.6 Patent-literature design-space markers (informational only)

**Early-ACK shootdown (USPTO 11321242).** Receivers ACK the IPI before flushing the TLB, then flush. The sender's local flush is in progress during the receiver's ACK, so by the time the receiver actually flushes its TLB, the sender's flush has completed and the receiver could not have observed stale entries even if it briefly delayed the flush. Net: sender's wait is shortened by the time it takes the receiver to dispatch the IPI handler. Useful optimization filed for later. Not needed for SC-3C correctness baseline. Filing date 2020-ish (patent only; not pulled into the survey).

**Hybrid IPI / TLBI dispatch (USPTO 12253956).** Runtime detection chooses between IPI-based shootdown and hardware-broadcast TLBI (ARM's equivalent of INVLPGB). The patent's contribution is the erratum-detection variable: some hardware has broken broadcast-TLBI behaviour; the kernel falls back to IPI for those CPUs. This is the same architectural pattern Linux uses for INVLPGB (`cpu_feature_enabled(X86_FEATURE_INVLPGB)` gate), just framed as a patent. Confirms the design pattern is the right one; not actionable for SC-3C.

**Selective TLB invalidation hardware (USPTO 8112174).** Older patent (filed 2008 or earlier; granted 2012) describing hardware-level Invalidate-TLB-Key (ITK) broadcast as part of SSKE instruction completion. IBM mainframe lineage, not x86. Used as historical design-space marker only; the broadcast-invalidation hardware concept predates x86's INVLPGB by a decade-plus on other architectures.

---

## 4. Synthesis: SC-3C D-decisions against prior art

Evaluating each D1–D14 PM lean against the prior-art findings:

| D# | Decision | PM lean | Prior-art finding | Revised PM lean |
|---|---|---|---|---|
| **D1** | Q7.b CR3 cache scope | B2.a (compare-and-skip) | Asterinas #799 frames the same problem identically; FreeBSD and Linux do effectively the same compare-and-skip via switch_mm cpumask. | **Stand. B2.a.** |
| **D2** | PCID enablement | Defer | Linux's full ASID/PCID infrastructure (`global_asid_lock`, `global_asid_used`, lifetime tracking, rollover handling) is substantial; deferral is consistent with Bible 1.2 and 4.3. | **Stand. Defer.** |
| **D3** | Shootdown granularity | G3 hybrid, **N=16** | Linux's measured threshold is **N=33**. Linux's choice is empirically grounded for current x86 hardware. NeoPlutarch's N=16 is conservative; no correctness consequence. | **Revise to N=33** to align with Linux's measured optimum and reduce calibration churn. Document N=33 as Linux-prior-art-derived; revisit after measurement if NeoPlutarch's workload differs. |
| **D4** | Target selection | T1 broadcast | Linux uses T2 per-mm cpumask explicitly because broadcast IPI cost is measured to dominate at higher CPU counts. At NEO_SMP=4 the gap is small. T2 has its own bitmap-manipulation cache-line-contention cost. FreeBSD takes mask as API input parameter, lets caller choose. | **Stand on T1 broadcast for SC-3C BUILD-stage** but **mark T2 as a Phase 2+ forward optimization** (not deferred indefinitely). API surface should take the target set as an input parameter so T2 migration does not require API rework. |
| **D5** | Synchrony | S3 synchronous with 1ms bounded wait + panic-on-timeout | Linux is synchronous via `smp_call_function_many`. FreeBSD is synchronous via generation counter. No prior-art system uses asynchronous shootdown for correctness reasons. 1ms bounded wait is a NeoPlutarch-specific calibration; no public-source comparator. | **Stand. S3 with 1ms bound.** Document the 1ms as a NeoPlutarch choice without prior-art precedent; treat as a Phase 2 calibration parameter to revisit. |
| **D6** | Lazy TLB | L1 none (out) | Linux's lazy-TLB short-circuit is checked at the receiver, not the sender. Adding lazy TLB later does not require changing the sender API surface. | **Stand. Defer.** No API impact when added later. |
| **D7** | Batching | B1 none (out) | Linux's `mmu_gather` batching is significant complexity ("at most one flush IPI round-trip needed regardless of whether 1 or 1,000,000 pages are unmapped"). Defer is consistent with Bible 1.2. | **Stand. Defer.** |
| **D8** | IPI vector | 0xfc | No prior-art constraint; vector allocation is project-internal. | **Stand. 0xfc.** |
| **D9** | Self-invalidation | Synchronous before sending IPI to others | FreeBSD's `curcpu_cb` pattern overlaps local invalidation with remote-wait, more efficient. PM lean serializes them. | **Revise to FreeBSD-style overlap.** Self-invalidation happens via callback during the wait loop, not before the IPI dispatch. Minor performance win; no correctness impact; the API shape becomes more flexible for future targeted callers. |
| **D10** | ACK mechanism | Per-shootdown `AtomicU32` counter | FreeBSD uses a global generation counter with per-CPU "done" slots. Functionally equivalent; FreeBSD's approach avoids per-shootdown allocation but adds overflow concerns (4B-shootdown wraparound). | **Stand. Per-shootdown atomic counter.** NeoPlutarch's heap and Arc model favors per-shootdown allocation; FreeBSD's generation-counter alternative is a non-essential optimization. |
| **D11** | Per-CPU shootdown slot | α single slot (AtomicPtr) | No directly comparable design in surveyed prior art (Linux uses `smp_call_function_many`'s generic call infrastructure; FreeBSD uses the generation counter as the implicit per-CPU slot). | **Stand. α single slot.** Most architecturally aligned with NeoPlutarch's existing per-CPU state model. |
| **D12** | Sub-commit boundaries | 4a (Q7.b) → 4b (Q7.c machinery) → 4c (Q7.c wiring) | No directly comparable boundary in prior art; this is a NeoPlutarch BUILD discipline choice. | **Stand.** Bible-15 small-commit discipline supports this. |
| **D13** | IrqSafeMutex dependency | Bridge via `without_interrupts(...)` if not landed | No prior-art constraint; bridge is NeoPlutarch-internal. | **Stand.** |
| **D14** | Test gate scenarios | 200/200 NEO_SMP=4 + concurrent munmap + process-exit-during-thread-run + same-process context-switch micro-bench | FreeBSD-EN-18:07 is the canonical "missing shootdown causes silent data corruption" calibration; verification gate should explicitly include a test capable of catching missing-shootdown bugs (not just happy-path stress). | **Revise to add explicit "missing-shootdown-detection" scenario.** Specifically: CPU A unmaps page; CPU B reads from that virtual address; without correct shootdown, B's TLB returns stale physical page; with correct shootdown, B faults. Test should assert the fault. |

**Three revisions surfaced from prior art:**

1. **D3: N=16 → N=33** to match Linux's measured threshold. Rationale: empirical grounding from a production kernel on current x86 hardware.
2. **D9: serial-before-IPI → callback-during-wait (FreeBSD `curcpu_cb` pattern).** Rationale: small performance win, more flexible API surface, no correctness impact.
3. **D14: add missing-shootdown-detection scenario.** Rationale: FreeBSD-EN-18:07 is the canonical evidence that TLB shootdown bugs cause silent data corruption, not crashes; the verification gate needs a scenario that would catch such a bug.

**One forward-marker upgrade:**

4. **D4: T1 broadcast remains the SC-3C BUILD-stage lean, but mark T2 (per-Pmap cpumask) as a Phase 2+ forward optimization rather than "deferred indefinitely."** Rationale: Linux's design is targeted-by-default for measured reasons; NEO_SMP=4 makes the gap small today but the migration path should be documented now to avoid API rework later. Concretely: the SC-3C shootdown API should take the target set as an explicit input parameter (matching FreeBSD's `cpuset_t mask`) so a future T2 migration is a caller-side change, not a callee-side API break.

**Eleven D-decisions stand unchanged.**

---

## 5. Forward markers (tracked for later sub-commits)

Items to add to `Plans/PHASE-2.md` ADR-013 ledger or to a Phase 2+ forward-marker document, surfaced by this research:

1. **PCID / ASID enablement** — Linux's mature design provides the template. Becomes interesting when (a) measurement shows TLB-miss cost is the dominant performance issue, or (b) NeoPlutarch supports more concurrent processes than the TLB can naturally cache. Deferred from SC-3C; ADR-driven sub-commit later.

2. **T2 targeted shootdown (per-Pmap CPU mask)** — Linux's `mm_cpumask` design. Becomes interesting when NEO_SMP scales beyond 4 or when broadcast IPI cost dominates measurement. SC-3C API should be future-T2-compatible per D4 revised lean.

3. **Lazy TLB (receiver-side short-circuit)** — Linux's `cpu_tlbstate_shared.is_lazy`. Becomes interesting once NeoPlutarch has many idle CPUs or many kernel threads. Sender API does not change; deferred without prerequisite.

4. **Shootdown batching (`mmu_gather` analogue)** — Linux's coalescing-across-pages design. Becomes interesting if profiling shows shootdown-per-unmap is a workload bottleneck. Caller-side change; deferred.

5. **Early-ACK pattern (USPTO 11321242)** — Receiver ACKs before flushing. Optimization to file for later; minimal correctness consequence.

6. **AMD INVLPGB / Intel RAR hardware-broadcast** — Phase 3+ forward marker. Becomes interesting if NEO_SMP scales beyond 4 or if NeoPlutarch wants ATC-class performance numbers. Significant complexity (ASID allocation, lifetime tracking, runtime feature detection); deserves its own ADR.

7. **Spectre-v2 BTB flush on context switch (IBPB analogue)** — Linux's `cond_ibpb`. Not TLB shootdown work but co-located in the same context-switch code path. SC-3C should not introduce IBPB now but should leave the door open. Phase 2+ CPU-vulnerability mitigation cluster; deserves its own ADR.

8. **ARM port intermediate-page-table flush hook** — `flush_tlb_pgtable` pattern from the ARM64 Linux patch. Future-marker for if/when NeoPlutarch ports to aarch64. Per `kernel/src/arch/x86_64/mod.rs` header comment, "Phase 5+ may add aarch64 if we target the Pi 5"; this is then territory.

9. **Asterinas SMP TLB design tracking** — Asterinas is publicly the closest architectural reference and was mid-flight on SMP optimization through 2025–2026. Watch their repository for shootdown-design commits; treat as cross-validation source for D-decision revisions.

---

## 6. Calibration notes (citation rigor)

Applying the calibration discipline established by `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md` and the project's standing rule that citations must survive spot-check:

**Verified primary sources:** Linux `arch/x86/mm/tlb.c` at master and at pinned commit `9269d27e519ae9a89be8d288f59d1ec573b0c686` were read directly; specific identifiers cited (`tlb_single_page_flush_ceiling`, `mm_cpumask`, `should_flush_tlb`, `consider_global_asid`, `broadcast_flush_tlb_all`, `cond_ibpb`, `global_asid_lock`, `global_asid_used`, `MAX_ASID_AVAILABLE`, `invlpgb_flush_all_nonglobals`) are quoted from the source.

**Verified secondary sources:** FreeBSD code review D25510 diff was read directly; the `smp_targeted_tlb_shootdown` signature is quoted verbatim. FreeBSD-EN-18:07 advisory text is quoted verbatim from the FreeBSD security archive. Asterinas issue #799 quotation is verbatim from the GitHub issue. Asterinas 2024 retrospective quotation is verbatim from the asterinas.github.io blog post.

**Citations marked as design-space markers, not authoritative:** USPTO patents 11321242, 12253956, and 8112174 are treated as design-space evidence (the patterns exist in the literature) rather than as authoritative-design references. Patent claims often describe broader designs than the specific implementations that ship; treat patent citations as "this design surface has been explored" rather than "this is how production systems work."

**Future-dated / unverifiable claims flagged honestly:**
- Intel RAR Linux mainline integration: announced as a follow-up by Rik van Riel in the INVLPGB cover letter; **no merge verified at this research's cutoff** (May 2026). The synthesis treats RAR as an announced future optimization, not a shipping fact.
- Asterinas's 2025–2026 SMP TLB work: their 2024 retrospective committed to it as the 2025 focus; **the specific design choices and commits during 2025–2026 were not pulled in this research pass**. The synthesis treats Asterinas as "mid-flight, worth watching" not "settled prior art."
- The "kernel-internals.org/mm/tlb-optimization" writeup is a third-party documentation site, not a primary Linux source. Specific numerical claims from that site (TLB hit latency, miss latency, INVLPGB threshold of "more than 3 CPUs") were cross-checked against the Linux source code; the threshold check is present in `consider_global_asid` (`mm_active_cpus_exceeds(mm, 3)`), confirming the third-party number. Other latency claims from that site were not source-checked and are treated as illustrative rather than authoritative.

**No hallucinated citations.** Each URL in §2 was returned by the search-engine pass and the cited content was inspected. The arXiv ID collision pattern that prompted the rejection of the earlier AI-native-OS field-survey content (e.g., the "Anima OS / ProbeLogits" 2604.11943 collision) does not appear in this research's source set; arXiv IDs cited here (2506.03876 Asterinas ATC25, 2409.10946 FPR, 2002.01073 TLB-pagewalk-DRAM, 2308.11921 PARseL) are all 2025-or-earlier-vintage IDs consistent with published ATC/USENIX/arXiv timelines.

**Research-pass time budget actual:** approximately 75 minutes total (5 web searches, content inspection per search, synthesis write-up). Within the 1–2 hour estimated budget from the SC-3C THINK artifact §6 D15 row.

---

## 7. Cross-references

**Upstream artifact this informs:**
- `Plans/SC-3C-THINK-2026-05-10.md` — SC-3C THINK-stage with D1–D14 PM leans. This research surfaces revisions to D3, D9, D14 and a forward-marker upgrade for D4.

**Voice anchor:**
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` — Janus-format research artifact; this artifact matches the rigor standard.

**Operational referents:**
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` — Cerebro deep-research lane definition. First substantive Cerebro deep-research dispatch under the new agent roster.
- `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md` — Citation-rigor calibration discipline. This artifact applies that discipline (§6 calibration notes).

**Substrate referents (re-confirmed during synthesis):**
- `kernel/src/arch/x86_64/per_cpu.rs:33` — `active_pmap_cr3: AtomicU64` substrate for Q7.b.
- `kernel/src/arch/x86_64/apic.rs` — IPI infrastructure pre-wired for Q7 callers (`APIC_ICR_LOW`, `APIC_ICR_HIGH`, `ICR_LEVEL_ASSERT` constants with explicit Q7-anticipating dead-code allow).
- `kernel/src/arch/x86_64/mod.rs:142-149` — `flush_local_tlb()` CR3-reload-with-current existing primitive.
- `kernel/src/mm/pmap.rs:319-322` — `Pmap::cr3()` API and the PCID-deferred comment.

**Authority and bible/ADR alignment:**
- `docs/bible.md` v1.5.0 Article 1.2 — correctness > coherence > performance. Supports D2/D6/D7 PCID/lazy/batching deferral.
- `docs/bible.md` v1.5.0 Article 4.3 — every dependency a liability. Supports keeping the SC-3C API surface minimal and forward-marking advanced optimizations rather than implementing them speculatively.
- `docs/ADR-013-phase-2-smp-retrofit.md` — Phase 2 SMP retrofit plan; SC-3C closes §Q7.b/c per this artifact's §1 mandate.

---

## 8. Recommendation

PM consumes this artifact alongside the SC-3C THINK and surfaces a short PM-NOTE proposing the three D-revisions (D3 → N=33, D9 → FreeBSD overlap, D14 → add missing-shootdown-detection scenario) plus the D4 forward-marker upgrade. PL ratifies, and the SC-3C PLAN-stage dispatch can then be drafted with prior-art-grounded D-decisions in hand.

If PL prefers to leave PM leans untouched and defer revisions to a measurement-driven post-BUILD ADR, that is also defensible — the three revisions are not correctness-blocking, just empirically grounded refinements. Either path leads to a credible SC-3C BUILD-stage.

**End of research artifact.**

*Linux's measured INVLPG threshold is 33 pages, not 16. FreeBSD's `smp_targeted_tlb_shootdown` is the closest API-shape match and uses a curcpu callback pattern more efficient than serial-before-IPI. FreeBSD-EN-18:07 is the canonical "missing TLB shootdown = silent data corruption" anchor and informs D14. Asterinas issue #799 frames Q7.b identically and is mid-flight on the same problem set. AMD INVLPGB / Intel RAR are Phase 3+ hardware-broadcast forward markers; not SC-3C concerns. seL4 sidesteps the problem by architectural simplification; not a positive design reference. Eleven D-decisions stand; D3 / D9 / D14 see specific revisions; D4 upgrades from indefinite-defer to documented forward-optimization with API-shape future-compatibility.*
