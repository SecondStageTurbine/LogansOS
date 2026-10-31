<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: memory-quota leaf re-check, relay #184 / #228

Reviewer: Jarvis. Branch: `codex/mem-quota-leaf-recheck-jarvis`.
**CLOSED: the three dispatched findings on merged #228.** Reviewed squash
`0769274cb3f660c854780e9a5dc20e43cfbb7b1c`, an ancestor of fetched main
`bf2fc858`. All code references below are **at the squash**. This replaces
this file's earlier pre-merge assessment and clears its pending-target status.

1. **CLOSED — refused mappings free their unattached leaf.**
   `kernel/src/mm/pmap.rs:461–475` returns an armed `UnmappedFrame` after
   charge/allocation, or releases the charge if allocation itself fails.
   `UnmappedFrame::drop` (:1275–1280) physically frees an armed frame
   before releasing its charge. A Pmap borrow keeps the owner alive.
   Every production allocation caller keeps the guard until successful
   installation: ELF segments `kernel/src/loader/elf.rs:211–248`, stacks
   :293–306, ordinary regions `pmap.rs:804–817`, guarded regions :868–880,
   and intermediate tables :637–654. `?` and other early returns drop an
   armed guard; successful installation alone calls `mapped()` (:1269).
   Intermediate tables linked before a later refusal remain reachable and
   charged until Pmap teardown; they do not become unattached leaks.
   The feature-off allocator-exhaustion behavior is outside this fix.
2. **CLOSED — a replaced owned leaf is freed once after
   shootdown.** `pmap.rs:521–525` captures the old owned leaf under the
   same guard that replaces its PTE. Borrowed leaves are excluded by
   `owned_leaf_locked` (:560–563), and an identical old/new frame is
   excluded at :522. The guard is dropped (:529), the replacement is
   flushed (:530–531), then the detached old frame is freed and its charge
   released (:534–536). Pmap Drop only walks the new reachable leaf, so
   it does not free or release the detached old leaf again. Its remaining
   frame frees precede `release_all` (:1345–1361).
3. **CLOSED for the real-frame discriminator; conditional
   global-counter risk, not a demonstrated four-CPU flake.**
   `kernel/src/test_harness.rs:42027–42041` compares `frames_in_use()`
   around 32 refused maps (region, guarded region, stack, ELF) and 8
   replacements after a warm-up round. The backing global counter is
   `FRAMES_IN_USE` (`kernel/src/mm/frame.rs:71–75`), incremented by real
   frame allocation (:633–635) and decremented by successful physical free
   (:509–512), independently of memory-account release.

   Independently ran the squash in Docker with QEMU 10.2.3:
   `NEO_SMP=4 NEO_TEST_ATTEMPTS=1 cargo neo-test mem_quota --no-meta`.
   **PASS:** 1,028 physical frames before and after all 32 refused maps
   and 8 replacements, `root_used=0 underflows=0`.

   Repeated the physical-leak negative control on this exact squash:
   keep the ledger releases, omit the physical frees in
   `UnmappedFrame::drop` and the replaced-leaf branch. **FAIL** at
   `failed_map_frames_lost`, before=1033, after=1073: exactly 32 refused
   leaves + 8 replaced leaves lost. It still reported
   `root_used=0 underflows=0`, so account equality alone would miss it.
   Both runs booted four CPUs and used one attempt. The earlier head's
   empty-guard control also failed `failed_map_leaf_still_charged`; its
   source is unchanged, but that control was not rerun in this update.
   Original source bytes were restored; the isolated squash checkout is
   clean. No implementation changes are included.

   An unrelated CPU retaining one newly allocated frame between the two
   reads would change N to N+1 and falsely accuse this leg; an unrelated
   free could also mask a leak. Warm-up alone does not prevent either.
   However, this fixture calls the leg after `spawn_neo_init` has yielded
   and reaped until **no user process remains** (`kernel/src/main.rs:519–532`),
   and after the deferred-drop leg. AP initialization is complete before
   tests (`kernel/src/arch/x86_64/smp.rs:174–209`); AP bootstrap loops do
   not start allocating work merely because `NEO_SMP=4`
   (`kernel/src/sched/scheduler.rs:470–500`), and reaping is BSP-only
   (:2684–2686). No concurrent allocating AP workload is introduced by
   this scenario. Thus four CPUs alone do not establish the suggested
   flake. If the fixture gains concurrent workers/background allocation,
   require explicit quiescence or a scoped physical-frame witness before
   retaining this exact global equality assertion.

RAG health was healthy (`ollama=true`, `qdrant=true`). The graft discovery
result contained a stale pre-fix `map_user`; conclusions use the pinned Git
source, not that snippet. The allocation/map/guard bodies, ELF loader,
frame allocator, failed-map leg and scenario entry block match the previously
reviewed head `4c880925` exactly. Subsequent main changes in Pmap concern
borrowed-mapping lifetime and do not alter the reviewed owned-leaf paths.
Only this RETURN is included in the update.
