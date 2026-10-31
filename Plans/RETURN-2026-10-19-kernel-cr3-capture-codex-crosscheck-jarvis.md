<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: CP-60 kernel-CR3 capture crosscheck — Jarvis (Codex)

**Reviewed:** merged `d62a494b3d48052c2333dca7526bb4e75af55ac6` and the tree-equivalent review
head `e53fb1e6837bca83fddc47c59d41990cf8d850d9`  
**Verdict:** **BOUNCE** — the CR3 capture fix is correct, but the new hard invariant can re-enter the
frame allocator from the panic path while already holding its lock. Move the assertions before
`with_frame_alloc` before treating the guard as safe in every supported test environment.

## Finding

### F1 — BLOCKER: `Pmap::drop` can panic while holding `FRAME_ALLOC`, and the bare-metal test panic path allocates DMA frames

**Sites:** `kernel/src/mm/pmap.rs:1040-1064`, `kernel/src/mm/mod.rs:410-449`,
`kernel/src/panic.rs:27-38`, `kernel/src/test_harness.rs:4028-4041,31578-31586`,
`kernel/src/log/block_sink.rs:187-239,297-324,380`, `kernel/src/drivers/block/dma.rs:69-80`.

Both new assertions execute inside the closure passed to `with_frame_alloc`, after that helper has
disabled interrupts and acquired the global `FRAME_ALLOC` spin mutex. A failing assertion enters the
kernel panic handler without releasing that guard (this kernel aborts; it does not unwind). In an
ordinary production panic, the handler prints and paints the framebuffer, neither of which acquires
`FRAME_ALLOC`; in an ordinary QEMU neo-test without a block log sink, `log::flush()` finds no sink.
Those paths therefore halt with the lock held but do not re-enter that lock.

The supported neo-test bare-metal path is different:

1. `on_panic` calls `test_harness::panic_failed`.
2. `panic_failed` calls `fail`, then `debug_exit`.
3. `debug_exit` calls `crate::log::flush()`.
4. With the bare-metal block log sink installed and staged bytes pending, `flush_locked` calls
   `write_bytes_at`, which creates `DmaBuffer`s.
5. `DmaBuffer::alloc_zeroed` calls `mm::with_frame_alloc` again on the same CPU.

In debug builds, the frame re-entry guard panics inside the panic handler; in a build without that
guard, the second `FRAME_ALLOC.lock()` spins forever with IF already clear. Either result defeats the
new invariant's purpose of failing loudly and preserving the first diagnosis. The assertion also runs
only after all collected lower-level page-table/data frames have already been returned to the buddy
allocator (`pmap.rs:1041-1043`), making its placement later than the condition it is meant to guard.

**Repro:** enable a neo-test bare-metal lane with its block log sink, leave a log line staged, then cause
the active-CR3 condition before the last `Arc<Pmap>` is dropped (the lazy-control condition is one way
to create it). The first assertion panic reaches the call chain above while `FRAME_ALLOC` is held.
The exact deadlock is deterministic in a non-debug build; the debug build recursively panics at
`frame_reentry_guard::enter`.

**Required repair:** compute `freed` and perform both the kernel-CR3 and per-CPU active-CR3 assertions
before entering `with_frame_alloc` (and therefore before freeing any descendant frame). `Drop` has
`&mut self`, and every schedulable thread owns an `Arc<Pmap>`, so once the final `Arc` reaches `Drop`, no
legitimate scheduler path can newly load this Pmap between the check and the free. Keep the assertions
hard: freeing an actually active PML4 is a fatal kernel invariant violation, not a debug-only concern.

## Required numbered checks

1. **Boot value — sound.** `mm::init` reads CR3 at `kernel/src/mm/mod.rs:170`, alongside the existing
   `KERNEL_PML4_PHYS` capture, before the first user Pmap is constructed (`main.rs:600` precedes user
   spawn). Later paging helpers mutate mappings and flush/reload the same CR3; none rebuilds or replaces
   the kernel PML4. APs are brought up only after `mm::init` and enter under Limine's shared kernel
   mappings. Their initially-zero software cache forces the first scheduler address-space transition
   through the canonical writer.

2. **Invariant — logically sound, placement unsafe.** Spawn-failure Pmaps were never loaded. User,
   same-pid, idle, bootstrap, and kernel-thread transitions all route through
   `scheduler::switch_to_address_space`, which performs `Cr3::write` before updating
   `active_pmap_cr3`. A live/runnable thread retains an `Arc<Pmap>`, preventing the final drop. The only
   direct CR3 writers outside that function are local-TLB reloads of the current CR3 and three terminal,
   test-only SMEP/SMAP probes; those probes never return to drop their Pmap. A hard assertion is the
   right severity, but F1 requires it to run outside the allocator critical section.

3. **Other lazy kernel state — none found.** Exhaustive `Lazy::new`, `Once`, CR3/CR4, GS, TSS, and
   rflags searches found no other lazy initializer that snapshots live per-process CPU state. The
   remaining Lazies construct owned registries/tables/constants; GDT/TSS state is explicitly per-CPU.
   The lazy CR3 under `neo-test-kernel-cr3-lazy-control` is intentionally the negative control.

4. **Exposure on pre-fix main — confirmed.** On the first user-to-kernel switch, the old lazy initializer
   read the outgoing user's still-live CR3. The cache already contained that same user value, so the
   canonical writer skipped the hardware write and kernel/idle work continued on neo-init's user half.
   Kernel-half mappings are shared, so normal kernel execution remained possible; any explicit
   SMAP-enabled user access could observe that process's user half. The
   `current_process_for_syscall` fallback would also resolve that cached CR3 to neo-init if reached with
   a mismatched PID, although normal user syscall entry already has the correct PID and ordinary kernel
   threads do not enter through SYSCALL.

5. **Discriminator — non-vacuous and exact.** Each round creates a fresh Pmap, registers a process,
   schedules a process-owned kernel thread on that Pmap, observes it run and park, terminates, reaps,
   verifies registry removal, and repeats 60 times. Under the control feature,
   `process::kernel_cr3()` reinstates exactly `Lazy::new(Cr3::read)`. Its first evaluation occurs on the
   user-to-kernel transition while the process PML4 is active; the cache equality then reproduces the
   skipped write. The normal and control feature compositions both compile.

6. **Additional structural checks.** The review head and merged commit have identical trees. The new
   CR3 capture has no kill-switch leakage outside the explicit negative control. The added docs have a
   trailing blank line (`git diff --check`), a non-blocking formatting nit.

## Verification

- `cargo fmt --all -- --check` — PASS.
- `cargo check -p neoplutarch-kernel --features neo-test-token-park-teardown-churn` — PASS (12 existing
  dead-code warnings).
- `cargo check -p neoplutarch-kernel --features neo-test-token-park-teardown-churn,neo-test-kernel-cr3-lazy-control`
  — PASS (same warnings).
- QEMU reruns were not repeated; the relay supplied fixed 3/3 PASS and control FAIL evidence. This
  review's blocker is a panic-path lock-order defect outside the ordinary QEMU configuration.
- Semantic RAG health was healthy but its index timestamp preceded these commits and queries returned
  no results; direct source reads and exhaustive exact searches were used.
