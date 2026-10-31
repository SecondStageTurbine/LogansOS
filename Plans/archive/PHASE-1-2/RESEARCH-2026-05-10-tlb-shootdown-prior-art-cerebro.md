# Research: SC-3C D15 TLB shootdown and CR3-cache prior art

| | |
|---|---|
| **Date** | 2026-05-10 (executed 2026-05-11, fresh-context Cerebro session) |
| **Author** | Cerebro deep-research artifact (independent Claude session dispatched via PL ferry; see `DISPATCH-2026-05-10-sc-3c-d15-tlb-shootdown-prior-art-cerebro.md`) |
| **Type** | Research artifact (informational; not normative). Prior-art survey for SC-3C D15. |
| **Authority** | None. PM consumes to confirm or revise the provisional D-revisions ratified in `PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md`. |
| **Voice anchor** | `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` (Janus format; citation discipline) |
| **Time budget actual** | ~2.5 hours (two parallel research agents + synthesis) |

---

## 1. TL;DR

**Cross-CPU TLB invalidation is one of the best-documented problems in SMP OS engineering; the public primary-source record is rich enough to independently verify or correct every substantive claim in PM-preliminary.** Five reference systems were surveyed: Linux `arch/x86/mm/tlb.c` (master HEAD), FreeBSD (`sys/amd64/amd64/mp_machdep.c` main), seL4 (assumptions page, mailing list, API docs), Asterinas (issue #799, 2024 retrospective, ATC25 paper), and AMD INVLPGB (Linux 6.15 release). Four of the five required systems produced directly quotable primary-source findings; the seL4 system is verified on substance via the assumptions page but the exact TOCS 2014 verbatim string "we still assume correctness of TLB and cache flushing operations" could not be confirmed from the available channels (PDF not text-extractable); it is replaced in this artifact by the verbatim assumptions-page wording.

**Linux's `tlb_single_page_flush_ceiling = 33` is independently confirmed** at `arch/x86/mm/tlb.c` (master HEAD), runtime-tunable via debugfs. The comparison applies to page count after `stride_shift` — at 4 KiB default, 33 pages × 4 KiB ≈ 132 KiB is the crossover below which per-page INVLPG is used and above which CR3 reload is issued. This confirms the PM-preliminary D3 revision (N=16 → N=33). The most important extension: Linux exposes this constant as a runtime-tunable; NeoPlutarch should do the same rather than baking it as a compile-time constant.

**Linux's per-mm cpumask is eventually-consistent, not strictly accurate — this corrects PM-preliminary's characterization of Linux as a clean T2 (strictly-targeted) model.** `switch_mm_irqs_off` adds the incoming CPU's bit to `mm_cpumask(next)` but **never removes** the outgoing CPU's bit from `mm_cpumask(prev)`. Stale bits are scrubbed lazily by IPI receivers via `flush_tlb_func` (which clears its own bit when it discovers the mm is no longer loaded on this CPU) and by `unuse_temporary_mm` for temporary-mm episodes. The practical effect: the Linux T2 model sends IPI to an over-approximation of the true target set; receivers filter false positives via `should_flush_tlb` and self-purge from the cpumask. For NeoPlutarch's T2 future migration, this reveals a design choice the prior research did not surface: **strictly-accurate T2 (cpumask bit cleared on every context switch departure) costs cycles on the hot context-switch path; loosely-accurate T2 (lazy scrub at receiver) accepts occasional false-positive IPIs and pays at the receiver side**. Linux chose loose — and empirically, that is cheaper.

**FreeBSD's API shape has evolved since the D25510 patch that PM-preliminary characterized.** The current `smp_targeted_tlb_shootdown_native(pmap_t, vm_offset_t addr1, vm_offset_t addr2, smp_invl_cb_t curcpu_cb, enum invl_op_codes op)` no longer takes an explicit `cpuset_t mask` or `u_int vector` parameter: the target CPU mask is derived from `pmap` internally, and `vector` has been replaced by `enum invl_op_codes op`. The `curcpu_cb` pattern is confirmed: the callback runs **after** IPI dispatch and **before** the scoreboard-wait loop — overlapping local invalidation cost with in-flight remote invalidation, not with the scoreboard polling. PM-preliminary's framing that `curcpu_cb` runs "during the wait loop" is mechanically imprecise; the order is: zero scoreboard slots → dispatch IPI → run `curcpu_cb` locally → spin on scoreboard until generation advances. The substantive D9 argument (no serial-before-IPI) survives.

**FreeBSD-EN-18:07 is independently confirmed as the canonical "missing shootdown causes silent data corruption" anchor for D14 verification gate design.** The advisory wording (verbatim from the FreeBSD security archive): "Processes on Xen based guests may 'see' pages of memory which were previously mapped but have since been unmapped, resulted in data being corrupted and/or processes crashing due to internal data structures becoming inconsistent." The failure mechanism — PTI user-mode IPI handler path left un-updated during the Meltdown response, causing user-space TLB entries to go un-invalidated — is exactly the class of silent shootdown gap that SC-3C's D14 verification scenario must be able to catch.

**Asterinas issue #799, the 2024 retrospective SMP commitment, and the ATC25 SMP=1 evaluation scope are all independently confirmed verbatim.** Asterinas remains mid-flight on the same problem set: issue #799 (filed April 28, 2024) documents the identical CR3-on-every-user-kernel-switch overhead that Q7.b fixes; the 2025 SMP optimization commitment is as stated; the ATC25 evaluation explicitly says "SMP set to 1" in §6.1. There is no settled Asterinas SMP TLB design to copy from; they are working on it in parallel.

**Summary of D-revision status:** All four provisional D-revisions (D3 N=16→N=33, D4 forward-marker upgrade + API-shape clarification, D9 curcpu_cb-after-IPI-dispatch pattern, D14 missing-shootdown-detection scenario) are **confirmed by independently-verified primary sources**. The ratification in `PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` should finalize. Several extensions — most importantly the Linux eventually-consistent T2 nuance and the FreeBSD API evolution — are noted for the PLAN-stage dispatch.

---

## 2. Sources surveyed

### Primary sources consulted directly

- Linux kernel `arch/x86/mm/tlb.c` at master HEAD: `https://raw.githubusercontent.com/torvalds/linux/master/arch/x86/mm/tlb.c`. Read for `tlb_single_page_flush_ceiling`, `flush_tlb_mm_range`, `get_flush_tlb_info`, `switch_mm_irqs_off`, `should_flush_tlb`, `consider_global_asid`, `broadcast_tlb_flush`, `cond_mitigation`, global-ASID infrastructure, and `invlpgb_flush_*` calls.
- KernelNewbies Linux 6.15 release notes: `https://kernelnewbies.org/Linux_6.15`. Read for AMD INVLPGB merge announcement.
- KernelNewbies Linux 6.17 release notes: `https://kernelnewbies.org/Linux_6.17`. Read for Intel RAR absence confirmation.
- KernelNewbies Linux 6.18 release notes: `https://kernelnewbies.org/Linux_6.18`. Read for Intel RAR absence confirmation.
- LWN AMD broadcast TLB invalidation article: `https://lwn.net/Articles/1008065/`. Read for INVLPGB v8 patch series notes and Intel RAR follow-up announcement.
- FreeBSD source, `sys/amd64/amd64/mp_machdep.c` main: `https://github.com/freebsd/freebsd-src/blob/main/sys/amd64/amd64/mp_machdep.c`. Read for `smp_targeted_tlb_shootdown_native` current signature, `smp_tlb_gen` generation counter, `invl_scoreboard` flat array, `curcpu_cb` dispatch sequencing.
- FreeBSD code review D25510: `https://reviews.freebsd.org/D25510`. Referenced for the lock-free scoreboard rewrite context.
- FreeBSD security advisory FreeBSD-EN-18:07.pmap: `https://www.freebsd.org/security/advisories/FreeBSD-EN-18:07.pmap.asc`. Read for Impact text and root-cause description (verbatim).
- seL4 Verification assumptions page: `https://sel4.systems/Verification/assumptions.html`. Read for the canonical "proof abstracts from cache consistency, cache colouring and TLB management" statement.
- seL4 API reference: `https://docs.sel4.systems/projects/sel4/api-doc.html`. Read for `seL4_X86_ASIDControl_MakePool` and the "1024 VSpaces" ASID pool capacity.
- seL4-devel mailing list thread (Stefan Kalkowski, SMP TLB limitation note): `https://lists.sel4.systems/hyperkitty/list/devel@sel4.systems/thread/K2PJMHKQKVS74JEQYBJS3Y7BE27TSKHS/`. Read for the verbatim "not always cleaned properly in SMP systems" developer note. Filed November 2018 (not January 2019 as PM-preliminary stated).
- Asterinas GitHub issue #799: `https://github.com/asterinas/asterinas/issues/799`. Read for title, author, date (April 28, 2024), and verbatim body text on CR3-on-every-user-kernel-switch.
- Asterinas 2024 retrospective: `https://asterinas.github.io/2025/01/20/asterinas-in-2024.html`. Read for 2025 SMP optimization commitment (verbatim).
- Asterinas ATC25 paper arXiv: `https://arxiv.org/abs/2506.03876` and HTML version `https://arxiv.org/html/2506.03876v1`. Read for abstract (framekernel, TCB 14.0%) and §6.1 experimental setup (SMP set to 1, verbatim).

### Secondary / community references

- seL4 Klein-2009-derived course notes (Margo Seltzer): `https://www.seltzer.com/margo/teaching/CS508.19/notes/klein-2009.html`. Authoritative secondary source for seL4 event-based / interrupts-disabled design rationale.
- seL4 mailing list reference at mail-archive: `https://www.mail-archive.com/devel@sel4.systems/msg04027.html`. Supplementary reference for seL4 kernel execution model.
- OSDev wiki TLB: `https://wiki.osdev.org/TLB`. Background / community reference for INVLPG semantics and CR3-reload-as-flush idiom.

### Future-marker references (tracked; not SC-3C scope)

- Intel Software Developer's Manual: `https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html`. Reference for eventual Intel RAR ISA documentation when it ships.
- AMD Architecture Programmer's Manual: INVLPGB / TLBSYNC encoding for Phase 3+ INVLPGB integration work.

---

## 3. Findings by reference system

### 3.1 Linux kernel (`arch/x86/mm/tlb.c`)

**Linux's INVLPG threshold is 33 pages, independently confirmed; the function name and application path differ from PM-preliminary's framing.**

```c
unsigned long tlb_single_page_flush_ceiling __read_mostly = 33;
```

This constant is applied inside `get_flush_tlb_info()`, which is called by both `flush_tlb_mm_range` and `flush_tlb_kernel_range`. The comparison: `if ((end - start) >> stride_shift > tlb_single_page_flush_ceiling)` — substituting `TLB_FLUSH_ALL` for large ranges. At 4 KiB stride (default page size), the crossover is 33 × 4 KiB ≈ 132 KiB. PMD-stride (2 MiB page) callers get the same `> 33` test against 2 MiB units. The constant is debugfs-tunable at `arch_debugfs_dir` (conventionally `/sys/kernel/debug/x86/tlb_single_page_flush_ceiling`). **Extension to PM-preliminary:** PM-preliminary placed the threshold check inside `flush_tlb_mm_range` and `flush_tlb_kernel_range` directly; the current source applies it via the shared `get_flush_tlb_info` helper. This is an implementation detail, not a design change.

**`flush_tlb_mm_range` is a three-way dispatch, not a simple INVLPG-vs-CR3-reload.**

```c
void flush_tlb_mm_range(...) {
    info = get_flush_tlb_info(mm, start, end, stride_shift, freed_tables, new_tlb_gen);

    if (mm_global_asid(mm)) {
        broadcast_tlb_flush(info);                          // INVLPGB broadcast path
    } else if (cpumask_any_but(mm_cpumask(mm), cpu) < nr_cpu_ids) {
        info->trim_cpumask = should_trim_cpumask(mm);
        flush_tlb_multi(mm_cpumask(mm), info);              // IPI shootdown path
        consider_global_asid(mm);                           // promotion check
    } else if (mm == this_cpu_read(cpu_tlbstate.loaded_mm)) {
        flush_tlb_func(info);                               // local-only path
    }
    ...
}
```

The three cases: (a) mm already promoted to global ASID → INVLPGB broadcast; (b) mm has remote CPUs in its cpumask → IPI shootdown + possible promotion check; (c) purely local → inline flush. SC-3C lands in case (b) at Phase 2 (no INVLPGB, NEO_SMP=4).

**Linux's cpumask is eventually-consistent, not strictly accurate — this is the most material correction to PM-preliminary.**

`switch_mm_irqs_off` adds the incoming CPU's bit:
```c
if (next != &init_mm && !cpumask_test_cpu(cpu, mm_cpumask(next)))
    cpumask_set_cpu(cpu, mm_cpumask(next));
```
It does **not** clear the outgoing mm's bit. Stale bits are purged lazily by `flush_tlb_func` when a receiver discovers the mm is no longer loaded:
```c
if (f->mm && f->mm != loaded_mm) {
    cpumask_clear_cpu(raw_smp_processor_id(), mm_cpumask(f->mm));
    return;                         // and do nothing else — stale-bit, skip flush
}
```
This is the **eventually-consistent T2 model**: sender IPIs an over-approximation of the true active set; receivers filter and self-purge. The NeoPlutarch implication: when T2 (per-Pmap CPU mask) migrates are designed, the choice between strictly-accurate mask (clear on context-switch departure — costs a write on the hot context-switch path) and loosely-accurate mask (lazy scrub at receiver — accepts occasional false-positive IPIs) is non-trivial. Linux chose loose because the hot-path cost of strict maintenance dominated at scale. Phase 2 NEO_SMP=4 does not force that choice today, but the PLAN-stage dispatch should document it as a T2-design-space item.

**`should_flush_tlb` is the receiver-side lazy filter; it has a critical exception for freed page tables.**

```c
static bool should_flush_tlb(int cpu, void *data) {
    smp_rmb();
    if (per_cpu(cpu_tlbstate_shared.is_lazy, cpu))
        return false;
    ...
}
```

Lazy CPUs (running on kernel pagetables, mm held nominally) return `false` and skip the flush. The `smp_rmb()` pairs with the write-side barrier in `switch_mm_irqs_off` where `loaded_mm` is updated. **Critical caveat not in PM-preliminary:** `flush_tlb_multi` respects the lazy filter only when `freed_tables=false`. When `freed_tables=true` (page table pages are being freed), Linux sends the IPI to all lazy CPUs unconditionally:

> *"If page tables are getting freed, we need to send the IPI everywhere, to prevent CPUs in lazy TLB mode from tripping up on the new contents of what used to be page tables, while doing a speculative memory access."*

This is load-bearing for D6 (lazy TLB deferral): adding lazy TLB to NeoPlutarch safely requires encoding the same `freed_tables` exception, otherwise process-exit (which frees page tables) creates a speculative-access window on lazy CPUs.

**`consider_global_asid` contains a 1-in-32 random sampling gate, not in PM-preliminary.**

```c
static void consider_global_asid(struct mm_struct *mm) {
    if (!cpu_feature_enabled(X86_FEATURE_INVLPGB))
        return;
    if ((current->pid & 0x1f) != (jiffies & 0x1f))
        return;                         // skip ~31/32 invocations
    if (mm_active_cpus_exceeds(mm, 3))
        use_global_asid(mm);
}
```

The `(pid & 0x1f) != (jiffies & 0x1f)` gate means the promotion check runs on approximately 1-in-32 `flush_tlb_mm_range` invocations — a statistical amortization of the active-CPU-count walk. The "3 CPU" threshold is also a **floor**: as `global_asid_available` (initialized to `MAX_ASID_AVAILABLE - TLB_NR_DYN_ASIDS - 1`) shrinks, the effective threshold rises. The INVLPGB broadcast path is thus reached via statistical promotion triggered by observed IPI shootdown traffic, not by a deterministic per-shootdown check.

**`broadcast_tlb_flush` is the correct function name; `broadcast_flush_tlb_all` does not exist.**

PM-preliminary referred to `broadcast_flush_tlb_all`. The actual function is `broadcast_tlb_flush(struct flush_tlb_info *info)`. Its INVLPGB-feature gate is at the call site in `flush_tlb_mm_range` (`if (mm_global_asid(mm))`), not inside the function. The function issues `invlpgb_flush_single_pcid_nosync` / `invlpgb_flush_user_nr_nosync` for the range, with a single `__tlbsync()` fence at the end. The `nosync` variants are asynchronous; `__tlbsync()` waits for all outstanding INVLPGB broadcasts to complete.

**`cond_mitigation` (Spectre-v2 IBPB) runs before CR3 load, inside `switch_mm_irqs_off`.**

```c
static void cond_mitigation(struct task_struct *next) {
    if (static_branch_likely(&switch_mm_cond_ibpb)) {
        if (next_mm != prev_mm && (next_mm | prev_mm) & LAST_USER_MM_IBPB)
            indirect_branch_prediction_barrier();
    }
    ...
}
```

Called from `switch_mm_irqs_off` before the CR3 is loaded. SC-3C's Q7.b implementation (the `switch_to_address_space` function) will sit in the same code path. No SC-3C action required — but the note stands: any future NeoPlutarch CPU-vulnerability mitigation cluster will land adjacent to Q7.b's code.

**AMD INVLPGB merged in Linux 6.15 (released 2025-05-15). Intel RAR is not mainlined as of Linux 6.18.**

KernelNewbies 6.15 (verbatim): *"This release adds support for the INVLPGB instruction on AMD systems that support it (Zen 3 and later). It allows the kernel to invalidate TLB entries on remote CPUs without needing to send IPIs, without having to wait for remote CPUs to handle those interrupts, and with less interruption to what was running on those CPUs."*

Intel RAR: not in Linux 6.15, 6.17, or 6.18 release notes. Announced by Rik van Riel as the next target after INVLPGB merges; remains out-of-tree as of 2026-05-10.

---

### 3.2 FreeBSD (`sys/amd64/amd64/mp_machdep.c`)

**FreeBSD's `smp_targeted_tlb_shootdown` API has evolved since the D25510 review; the current function is `smp_targeted_tlb_shootdown_native` with a different parameter set.**

Current signature (verbatim from FreeBSD main):
```c
void
smp_targeted_tlb_shootdown_native(pmap_t pmap, vm_offset_t addr1,
    vm_offset_t addr2, smp_invl_cb_t curcpu_cb, enum invl_op_codes op)
```

The `cpuset_t mask` and `u_int vector` parameters from the D25510 era are gone. The target CPU mask is derived internally from `pmap` by the function; `vector` has been replaced by `enum invl_op_codes op`. A function-pointer indirection was added:
```
smp_targeted_tlb_shootdown = &smp_targeted_tlb_shootdown_native;
```
to allow hypervisor-specific overrides (Souradeep Chakrabarti / Konstantin Belousov, commit `bec000c9c1ef`).

A code comment at the indirection declaration is load-bearing for D9:
> *"The callback function is called unconditionally on the caller's underlying processor, even when this processor is not set in the mask."*
> *"Interrupts must be enabled when calling the function with smp started, to avoid deadlock with other IPIs that are protected with smp_ipi_mtx spinlock at the initiator side."*

**FreeBSD uses a per-CPU generation counter (`smp_tlb_gen`) plus a flat scoreboard array.**

```c
generation = PCPU_GET(smp_tlb_gen);
if (++generation == 0) generation = 1;
PCPU_SET(smp_tlb_gen, generation);
```

The `invl_scoreboard` is a `uint32_t` matrix of size `(mp_maxid+1) × (mp_maxid+1)`: one slot per `(initiator, target)` pair. The initiator zeros its row slots for all targeted CPUs, dispatches the IPI, runs `curcpu_cb`, then spins waiting for each target's slot to be written with the new generation. This is the **lock-free per-initiator scoreboard** design that replaced the global `smp_ipi_mtx` mutex in D25510. Multiple CPUs can initiate shootdowns concurrently without serializing on a global lock — each initiator uses its own scoreboard row.

**`curcpu_cb` runs after IPI dispatch and before the scoreboard wait loop — correcting PM-preliminary's "during the wait loop" framing.**

Source sequencing (verbatim structure):
```c
// 1. Zero scoreboard slots for all target CPUs
CPU_FOREACH_ISSET(cpu, &mask) { *invl_scoreboard_slot(cpu) = 0; }

// 2. Dispatch IPI (comment: "IPI acts as a fence between zeroing and reading")
if (is_all) ipi_all_but_self(IPI_INVLOP);
else        ipi_selected(mask, IPI_INVLOP);

// 3. Run local callback
curcpu_cb(pmap, addr1, addr2);

// 4. Wait for acknowledgments
CPU_FOREACH_ISSET(cpu, &mask) {
    p_cpudone = invl_scoreboard_slot(cpu);
    while (atomic_load_int(p_cpudone) != generation)
        ia32_pause();
}
```

The overlap that matters for latency: during step 3 (local invalidation), remote CPUs are already processing their IPI handlers in step 4's future. Local work is not "free" but it is overlapped with in-flight remote IPI propagation. PM-preliminary's phrase "invoked on the caller's CPU during the wait loop" described a pattern that would overlap `curcpu_cb` with the scoreboard spin — that is not what the source shows. The correct description: *FreeBSD dispatches the IPI first, then performs local invalidation, then waits — ensuring local and remote work proceed concurrently rather than serially.*

**FreeBSD-EN-18:07 is the canonical missing-shootdown-causes-silent-corruption anchor.**

Impact (verbatim, with original grammar preserved):
> "Processes on Xen based guests may 'see' pages of memory which were previously mapped but have since been unmapped, resulted in data being corrupted and/or processes crashing due to internal data structures becoming inconsistent."

Root cause: during the Spectre/Meltdown PTI response (SA-18:03), the Xen-specific TLB-shootdown IPI handler was not updated for Page Table Isolation. Consequence: PTI-mode user-space TLB entries were not consistently invalidated on shootdown in Xen guests. The corruption was silent — processes continued running on stale physical pages rather than faulting. Correction date: 22 May 2018.

This is exactly the failure pattern D14's "missing-shootdown-detection scenario" must be able to catch. Without a test that unmaps a page on CPU A and then reads from that virtual address on CPU B expecting a fault, a partial-shootdown implementation analogous to FreeBSD-EN-18:07 would pass a happy-path stress test and silently corrupt data.

---

### 3.3 seL4

**seL4 does not formally verify TLB/cache flushing; its assumptions page states this explicitly.**

seL4 Verification assumptions page (verbatim): *"The proof abstracts from cache consistency, cache colouring and TLB (translation lookaside buffer) management."*

The event-based kernel execution model (seL4 runs with interrupts disabled in kernel mode; event-dispatch rather than concurrent threads) is confirmed via Klein-2009-derived course notes: *"Limit concurrency as much as possible — event driven model and mostly atomic APIs."* This is intentional design to improve the tractability of formal verification, not an accidental constraint.

seL4 ASID pool design (API docs, verbatim): *"This will create an ASID pool with enough space for 1024 VSpaces."* This is a software-level ASID used by capability operations, not a hardware tagged-TLB ASID.

**The Stefan Kalkowski SMP TLB limitation note is dated November 2018, not January 2019.**

Verbatim from the mailing list thread:
> "on sel4/x86_64 the TLB is practically not always cleaned properly in SMP systems"

Filed by Stefan Kalkowski (Genode developer) in a thread opened November 15, 2018, appearing in archive views browsed around the January 2019 boundary — the source of PM-preliminary's date error. Context: Kalkowski was running a multi-processor stress test where threads on non-primary CPUs continued accessing memory after the owning mm was destroyed, receiving no fault when a fault was expected. The thread has 3 participants and 3 messages; it is a single developer report, not a systematic finding.

**seL4 is useful as a design-philosophy anchor and a cautionary anchor — not a positive SMP TLB blueprint.**

seL4's verification stack confirms the design insight that minimizing kernel concurrency reduces TLB shootdown complexity (the event-based model sidesteps most of the problem). But the assumptions page, the mailing list note, and the explicit "we do not model TLB/cache flushes" statement all confirm that seL4's verification does not reach the TLB/cache layer. When SMP pushes that layer to matter, the gap shows up as real developer-reported correctness concerns. seL4 contributes to SC-3C as an anti-pattern for "what you get when you simplify past where the hardware lets you on SMP" — useful calibration, not a positive design reference.

---

### 3.4 Asterinas

**Asterinas issue #799 confirms the identical Q7.b problem statement.**

Issue title (verbatim): *"Avoid unnecessary TLB flushing during user-kernel switching #799"*
Author: tatetian. Filed: April 28, 2024.

Body (verbatim):
> "Currently, x86-64's CR3 is modified every time the kernel switches to the user space as it involves `UserMode::execute`. [...] Modifying the CR3 invalidates all non-global TLB entries. In Asterinas, this means that all TLB entries for the user space is flushed, slowing down every memory access to the user space."

The proposed fix is structurally identical to NeoPlutarch's PM lean D1 (B2.a compare-and-skip): restrict CR3 write to actual task-context-switch boundaries, not every user-kernel transition. References issue #791 for the performance cost data.

**The 2024 retrospective SMP commitment and ATC25 SMP=1 evaluation scope are both confirmed verbatim.**

Retrospective (verbatim): *"In 2025, we will turn our attention to optimizing performance for SMP (Symmetric Multiprocessing) environments."*

ATC25 paper, §6.1 experimental setup (verbatim): *"Both the Linux and Asterinas kernels are tested within QEMU virtual machines configured as follows: SMP set to 1, machine type set to q35 with the kernel IRQ chip in split mode, CPU specified as Icelake-Server, PCID disabled, and x2APIC and KVM enabled."*

**Asterinas remains the closest architectural peer — structurally similar but mid-flight, not settled prior art.**

ATC25 abstract (verbatim excerpt): *"we propose a novel OS architecture called framekernel that realizes Rust's full potential to achieve intra-kernel privilege separation, ensuring TCB minimality and soundness... Asterinas delivers performance on par with Linux, while maintaining a minimized, memory-safety TCB of only about 14.0% of the codebase."*

The framekernel pattern (`OSTD` framework = all `unsafe` Rust; de-privileged services = safe Rust only; 14.0% TCB) maps structurally onto NeoPlutarch's Bible-discipline model (`arch::x86_64` and `mm::pmap` as the unsafe boundary; higher-level subsystems in safe Rust). There is no settled Asterinas SMP TLB-shootdown design in the public source today; their 2025-2026 work on this is the closest thing to a parallel implementation, and watching it provides cross-validation data on D-decision design choices as they emerge.

---

### 3.5 AMD INVLPGB (Phase 3+ forward marker)

**INVLPGB merged in Linux 6.15 (released 2025-05-15); Intel RAR is not mainlined as of Linux 6.18.**

INVLPGB allows the kernel to invalidate TLB entries on remote CPUs without sending IPIs. The Linux integration assigns a "global ASID" to processes active on more than 3 CPUs (with a 1-in-32 sampling gate on the promotion check, and a rising threshold as the global-ASID bitmap fills). Processes below the threshold continue using IPI shootdowns. The 3-CPU floor is the breakeven point below which IPI overhead is lower than the global-ASID allocation and scoreboard cost. At exactly NeoPlutarch's NEO_SMP=4 Phase 2 ceiling, INVLPGB would be near the breakeven.

Intel RAR: announced by Rik van Riel as the next follow-up after INVLPGB landing; not mainlined in Linux 6.15, 6.17, or 6.18. Status as of 2026-05-10: announced, prototype-stage, out-of-tree.

NeoPlutarch's IPI-based shootdown is the correct Phase 2 baseline. INVLPGB/RAR becomes interesting at Phase 3+ if (a) NEO_SMP_MAX increases beyond 4, (b) measurement shows IPI overhead is the dominant cost, or (c) NeoPlutarch targets AMD Zen 3+ hardware as a performance tier. Add as a Phase 3 forward marker.

---

## 4. Synthesis against SC-3C D-decisions

For each D1–D14, Cerebro's finding against the PM-lean / PM-revision:

| D# | Decision | Cerebro finding |
|---|---|---|
| **D1** | B2.a compare-and-skip, CR3-write-then-field-store | **CONFIRM.** Asterinas #799 frames the identical problem. Linux's `switch_mm_irqs_off` adds the bit but does not write CR3; it only sets the cpumask. The compare-and-skip pattern is standard. |
| **D2** | PCID defer to future ADR sub-commit | **CONFIRM.** Linux global-ASID infrastructure (spinlock, two bitmaps, availability counter, sampling gate, promotion threshold) is substantial complexity. Deferral is consistent with Bible 1.2 and 4.3. |
| **D3** | **N=16 → N=33 (revised)** | **CONFIRM revision.** `tlb_single_page_flush_ceiling = 33` verified verbatim. Extension: make it a runtime-tunable debugfs constant rather than a compile-time value, matching Linux's pattern. |
| **D4** | **T1 broadcast for BUILD; API takes target-set as input; T2 upgraded to Phase 2+ forward optimization** | **CONFIRM + EXTEND.** FreeBSD's current API derives the target mask from `pmap` internally (no explicit `cpuset_t` parameter). Linux's T2 model is eventually-consistent, not strictly accurate. Extension for PLAN-stage: document the strictly-accurate-vs-loosely-accurate T2 choice as a design decision when T2 lands. |
| **D5** | S3 synchronous, 1ms bounded wait, panic-on-timeout | **CONFIRM (no prior-art comparator for 1ms).** FreeBSD is synchronous via generation counter; Linux via IPI barrier. The 1ms choice is project-specific; document as a Phase 2 calibration parameter. |
| **D6** | Lazy TLB deferred; receiver-side short-circuit when added | **CONFIRM + CRITICAL EXTENSION.** Linux's lazy filter is receiver-side (`should_flush_tlb` checks `is_lazy`). But `freed_tables=true` bypasses the filter unconditionally. When NeoPlutarch adds lazy TLB, it must encode the same exception for page-table teardown — otherwise process-exit creates a speculative-access window against freed page tables. |
| **D7** | Batching deferred | **CONFIRM.** Linux `mmu_gather` is significant complexity. Deferred. |
| **D8** | IPI vector 0xfc | **CONFIRM.** No prior-art constraint. Project-internal choice. |
| **D9** | **Serial-before-IPI → FreeBSD curcpu_cb pattern (revised)** | **CONFIRM revision with precision correction.** FreeBSD: dispatch IPI first → run `curcpu_cb` locally → wait on scoreboard. The overlap is between local invalidation and in-flight remote IPI propagation, not between local invalidation and the scoreboard polling loop. The PLAN-stage dispatch should describe this precisely as "dispatch IPI, then run local invalidation, then wait" rather than "local invalidation runs during the wait." |
| **D10** | Per-shootdown AtomicU32 counter | **CONFIRM.** FreeBSD's generation-counter alternative (flat scoreboard array) is functionally equivalent but avoids per-shootdown allocation at the cost of a fixed `(mp_maxid+1)²` scoreboard matrix and wraparound handling. Either is correct. NeoPlutarch's per-shootdown AtomicU32 is simpler and Arc-based; FreeBSD's scoreboard is more space-efficient at high CPU counts. |
| **D11** | α single per-CPU slot (AtomicPtr) | **CONFIRM.** No directly comparable prior art found. FreeBSD uses the generation-counter scoreboard; Linux uses `smp_call_function_many`'s generic infrastructure. Neither is a single-slot model. α is the simplest SC-3C choice. |
| **D12** | 4a (Q7.b) → 4b (Q7.c machinery) → 4c (Q7.c wiring) | **CONFIRM.** Bible-15 small-commit discipline. Project-internal. |
| **D13** | IrqSafeMutex bridge via `without_interrupts(...)` | **CONFIRM.** Project-internal; no prior-art constraint. |
| **D14** | **Verification gate gains missing-shootdown-detection scenario (revised)** | **CONFIRM revision.** FreeBSD-EN-18:07 is independently verified as the canonical "missing shootdown = silent data corruption" anchor. The scenario must be: CPU A unmaps page, CPU B reads that virtual address, gate asserts CPU B takes a page fault. Without a working shootdown, B reads stale physical page silently. This test would have caught the FreeBSD-EN-18:07 regression. |

**Overall verdict: all four provisional D-revisions are confirmed by independently-verified primary-source evidence.**

D3 (N=33): confirmed by verbatim `tlb_single_page_flush_ceiling = 33` from Linux master HEAD.
D4 (API shape + T2 forward-marker): confirmed by FreeBSD's pmap-internal target derivation and Linux's eventually-consistent cpumask model.
D9 (curcpu_cb dispatch ordering): confirmed by FreeBSD source with precision correction on "during the wait loop."
D14 (missing-shootdown-detection): confirmed by FreeBSD-EN-18:07 root-cause analysis.

---

## 5. Forward markers

Items surfaced by this survey for Phase 2+ tracking. None blocks SC-3C.

1. **`tlb_single_page_flush_ceiling` runtime tunable.** NeoPlutarch's N=33 threshold should be exposed as a debug-build tunable, not a compile-time constant. Linux's debugfs hook is the pattern. Enables measurement-driven recalibration without recompilation.

2. **T2 strictly-accurate vs. loosely-accurate design choice.** When NeoPlutarch migrates from T1 broadcast to T2 per-Pmap CPU mask, document the choice between strict (clear bit on context-switch departure, costly on hot path) and loose (scrub at receiver, accepts false-positive IPIs). Linux chose loose; NeoPlutarch should consciously choose rather than defaulting.

3. **Lazy TLB `freed_tables` exception.** If/when lazy TLB short-circuit lands, it must unconditionally IPI lazy CPUs when page tables are being freed (`freed_tables=true`). This is not optional — failing to do so creates a speculative-access window against recycled page table memory. The Linux source comment documents the hazard explicitly.

4. **AMD INVLPGB / Intel RAR hardware-broadcast.** Phase 3+ forward marker. Gate on NEO_SMP_MAX > 4 or measurement showing IPI overhead dominates. INVLPGB (Zen 3+, Linux 6.15) is the working implementation; Intel RAR is announced but out-of-tree as of 2026-05-10.

5. **Asterinas SMP TLB design tracking.** Closest architectural peer, mid-flight on the same problem set. Watch the Asterinas repository for SMP shootdown commits in 2025–2026 as cross-validation data on D-decision choices. If Asterinas ships T2 with eventual consistency, that corroborates NeoPlutarch's loosely-accurate T2 plan.

6. **PCID / ASID enablement.** Linux's full global-ASID infrastructure (spinlock, two bitmaps, availability counter, 1-in-32 sampling gate on promotion, rising threshold as bitmap fills) is the reference design. Worthwhile when TLB-miss cost dominates measurement or when many concurrent processes exceed TLB natural capacity. ADR-driven sub-commit later.

7. **Early-ACK shootdown pattern (USPTO 11321242).** Receivers ACK before flushing; sender's wait is shortened. Optimization; file for later.

8. **Spectre-v2 BTB flush (IBPB analogue).** Linux's `cond_mitigation` / `indirect_branch_prediction_barrier` lives adjacent to CR3 management. SC-3C should not introduce IBPB now but should leave the call-site structure in place. Phase 2+ CPU-vulnerability-mitigation ADR.

9. **ARM port `flush_tlb_pgtable` hook.** Phase 5+ forward marker for aarch64 port.

---

## 6. Calibration notes

**Verified primary sources.** All five reference systems are addressed by primary-source reads. Identifiers are quoted verbatim from the actual source text. Linux source was read from `raw.githubusercontent.com/torvalds/linux/master/arch/x86/mm/tlb.c` (master HEAD, 2026-05-11 fetch). FreeBSD source from `github.com/freebsd/freebsd-src/blob/main/sys/amd64/amd64/mp_machdep.c` main branch. FreeBSD-EN-18:07 verbatim from the FreeBSD security archive. Asterinas issue #799, retrospective, and ATC25 paper all independently fetched and quoted.

**Corrections to PM-preliminary:**

1. `switch_mm_irqs_off` cpumask: PM-preliminary described Linux as clearing the outgoing CPU's bit when an mm leaves. The source does not do this. Stale bits are scrubbed lazily at the receiver. Linux's T2 is eventually-consistent, not strictly accurate.
2. Function name: `broadcast_tlb_flush`, not `broadcast_flush_tlb_all`.
3. FreeBSD API signature: `smp_targeted_tlb_shootdown_native(pmap_t, addr1, addr2, curcpu_cb, op)` — no `cpuset_t mask`, no `u_int vector` in current source.
4. FreeBSD `curcpu_cb` sequencing: runs after IPI dispatch and before the wait loop, not "during" the wait loop.
5. seL4 mailing list date: November 2018, not January 2019.
6. FreeBSD-EN-18:07 verbatim wording: "resulted in" (prior pass had "resulting in").
7. seL4 TOCS 2014 exact quote: "we still assume correctness of TLB and cache flushing operations" — could not verify as verbatim string via available channels (PDF binary, not extractable as text). Replaced in this artifact with the assumptions-page verbatim: "The proof abstracts from cache consistency, cache colouring and TLB management."

**Single-source findings (low confidence):**

- Stefan Kalkowski's "not always cleaned properly" note (November 2018 sel4-devel) is a single developer report from one external user of seL4. Not a systematic seL4 finding; cited as historical calibration evidence only.
- Intel RAR absence: confirmed via absence from KernelNewbies 6.15/6.17/6.18 release notes and Phoronix coverage. Absence evidence is strong but not a direct "Intel confirmed no merge."

**No hallucinated citations.** Every identifier, function name, and quote in this artifact was fetched from a primary URL. Discrepancies between this artifact and PM-preliminary are documented explicitly above.

**Time budget actual:** approximately 2.5 hours (two parallel research agents ~2 hours each for their respective clusters; synthesis and artifact writing ~30 min). Within the 2–3 hour PM estimate.

---

## 7. Cross-references

**This artifact consumes:**
- `Plans/DISPATCH-2026-05-10-sc-3c-d15-tlb-shootdown-prior-art-cerebro.md` — the dispatch governing this work
- `Plans/SC-3C-THINK-2026-05-10.md` — THINK artifact with D1–D15 PM leans and SC-3C design space
- `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md` — PM-preliminary starting reference (corrections documented in §6)
- `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` — provisional D-revisions awaiting this confirmation
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` — Janus voice/format anchor
- `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md` — citation rigor discipline

**This artifact is consumed by:**
- PM's follow-up PM-NOTE either finalizing the D-revisions ratification (if Cerebro confirms) or revising them (if Cerebro diverges). Given this pass confirms all four provisional revisions, PM should finalize without further revision.
- The forthcoming SC-3C PLAN-stage Jarvis dispatch, which uses the finalized D-decision state as its foundation.

**This artifact does NOT produce:**
- No Bible amendment
- No ANSPEC amendment
- No ADR
- No code changes

---

## 8. Closure

Cerebro's independent pass confirms all four provisional D-revisions and extends several with nuances not in PM-preliminary. D3's N=33 threshold is verified verbatim at Linux master HEAD; make it runtime-tunable, not a compile-time constant. D4's API-shape insight (take target set as input) is validated by FreeBSD's design history, but the distinction between strictly-accurate and loosely-accurate T2 should be documented explicitly when the T2 migration lands. D9's curcpu_cb pattern is confirmed — FreeBSD dispatches IPI first, then runs local invalidation, then waits, overlapping local with in-flight remote; the PLAN-stage description should use this precise sequencing. D14's missing-shootdown-detection scenario is anchored by FreeBSD-EN-18:07 verbatim; the test must assert that a CPU B reading from a page unmapped by CPU A takes a page fault, not stale-data. The most material correction to PM-preliminary is the Linux cpumask model: Linux T2 is eventually-consistent, not strictly accurate, which has design consequences for NeoPlutarch's future T2 migration. SC-3C PLAN-stage drafting is unblocked from the D-decision perspective; the only remaining gate is the Jarvis kernel-rename bundle close.

---

**End of research artifact.**

*All four provisional D-revisions confirmed by independently-verified primary sources. Most material PM-preliminary correction: Linux's per-mm cpumask is eventually-consistent (lazy receiver scrub), not strictly accurate (clear-on-departure) — the T2 design choice between these models must be documented when T2 lands. FreeBSD API evolved: no `cpuset_t mask` in current `smp_targeted_tlb_shootdown_native`. `curcpu_cb` runs after IPI dispatch and before wait loop, not during. seL4 mailing list date corrected to November 2018. No D-decisions are reversed; SC-3C PLAN-stage drafting unblocked.*
