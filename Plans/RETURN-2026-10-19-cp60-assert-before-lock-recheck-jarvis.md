<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: CP-60 assertion-before-lock re-check — Jarvis (Codex)

**Reviewed:** merged `013c39aa73e578472c437a35aa08b7c8eaaf8597` (#64) and its review head
`6d2a1123` on `claude/cp60-assert-before-lock`  
**Verdict:** **MERGE** — no findings.  
**Prior `08cd4184` F1:** **CLOSED.** `assert_pml4_not_live` is now the first executable statement in
`Pmap::drop`, before the read-only walk, shootdown, and every free, and it runs outside
`with_frame_alloc`.

## Numbered checks

1. **Panic-path lock safety — closed.** `assert_pml4_not_live` performs only the neo-test atomic witness
   updates, the captured-kernel-CR3 comparison, and per-CPU atomic active-CR3 reads. It runs before the
   `Vec` construction and before `with_frame_alloc`, so it holds neither `FRAME_ALLOC` nor a slab lock.
   It does not take the serial or block-log-sink locks. The normal and forced process teardown paths drop
   their registry/handle guards before the final process/thread references can release the `Pmap`; no
   `with_frame_alloc` closure contains a process, thread, or `Arc<Pmap>` release. I found no path that can
   invoke this final drop while already holding `FRAME_ALLOC` or another lock acquired by the reported
   panic path.

2. **Check-to-free window — sound.** The longer interval consists of a read-only user-half page-table
   walk followed by the teardown shootdown. `Drop` has exclusive access to the `Pmap`, and each
   schedulable user thread plus the owning process retains an `Arc<Pmap>`. Reaching the final `Arc` drop
   therefore proves that no runnable or parked thread can newly select this PML4 during that interval.
   An already-active stale CR3 is caught before any descendant frame is returned. Nothing in the walk or
   shootdown publishes the dying PML4 to a CPU.

3. **Witness — sound for the claimed property.** Both debug and release branches enter `run_locked`
   only after acquiring `FRAME_ALLOC`; `run_locked` sets the per-CPU flag immediately before calling the
   supplied closure and clears it immediately after the closure returns. Thus it marks exactly the
   closure lifetime in both branches. A panic inside the closure leaves the flag true, intentionally
   allowing `panic_failed` to report the offending state; the kernel's panic path is terminal and does
   not unwind or resume, so there is no later execution to poison. `LIVE_CHECKS` counts every live-PML4
   check and `LIVE_CHECKS_UNDER_FRAME_ALLOC` samples the flag before the hard assertions. The churn test
   requires at least 60 checks and zero under-lock samples, and xtask requires that exact witness line.

4. **Additional review — no findings.** The kernel-PML4 and every per-CPU active-PML4 assertion remain
   hard. The ordinary and lazy-control feature compositions both compile. The only limitation is that
   the held flag is a neo-test witness rather than production enforcement, which matches the dispatch
   and does not weaken the production assertions.

## Verification

- `cargo fmt --all -- --check` — PASS.
- `cargo check -p neoplutarch-kernel --features neo-test-token-park-teardown-churn` — PASS (12 existing
  dead-code warnings).
- `cargo check -p neoplutarch-kernel --features neo-test-token-park-teardown-churn,neo-test-kernel-cr3-lazy-control`
  — PASS (same warnings).
- QEMU was not rerun; the dispatch's supplied positive and negative runs were checked against the code
  paths and witness requirements.
- Semantic RAG health was healthy, but `last_indexed` was 2026-10-17 and therefore predates the reviewed
  commits; current source and exact-symbol searches were used for the final verification.
