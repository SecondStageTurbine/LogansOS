<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: CP-50 syscall-exit interrupt-window cross-check — Janus (Codex)

**Verdict: MERGE**

Reviewed the merged CP-50 change on `origin/main` (`17dcf9ef1bde00413847eba98fd003d912ad9890`). I found no surviving correctness, discriminator, or production-impact finding.

## Findings

None.

## Adversarial checks

1. **Fix and exit ordering.** Both cfg-exclusive `syscall_entry` assembly variants place `cli` immediately before `syscall_exit_swapgs`, followed only by `swapgs` and `iretq` (`kernel/src/arch/x86_64/syscall.rs:743` and `kernel/src/arch/x86_64/syscall.rs:846`). A fresh kernel disassembly showed the exact bytes `fa 0f 01 f8 48 cf` at those boundaries. Before the `cli`, a tick still enters with the kernel GS active; after the GPR pops nothing consumes per-CPU state before the `cli`. Preemption during the restore/pops therefore cannot expose the user GS window. `iretq` restores the user IF from the saved frame. A faulting `iretq` after `swapgs` remains a CPL-0 `#GP` with user GS, but that is the documented non-canonical-return residual rather than a surviving maskable-interrupt path. `cli` does not mask NMI, `#MC`, or fault/debug exceptions; those are likewise correctly retained as residuals.

2. **IF/context-switch audit.** I traced the context-switch, yield/block/exit, timer-preemption, user-entry, interrupt-return, `enable_interrupts`, `with_interrupts_enabled`, `IrqSafeMutex`, and explicit `sti` paths. Context switching occurs with IF clear; state-restoring helpers restore the IF state they found. The production exception is `LocalShootdownQuiesce::resume()` (`kernel/src/mm/tlb_shootdown.rs:39`), reached by `spawn_user`, which can make the pre-`cli` syscall tail preemptible but cannot reopen the post-`swapgs` window. I found no second syscall-return route or interrupt-stub return that reaches its exit `swapgs` with IF set.

3. **Discriminator validity.** The real test-vector entry saves the ring-origin flag plus 15 GPRs and the frame anchor, making the naked shim's `lea rsi, [rbp + 136]` correct (`kernel/src/test_harness.rs:8782`). The body uses atomics, MSR reads, address checks, and LAPIC EOI without a GS-relative per-CPU dereference. Slot zero is a sentinel; each count gets a unique nonzero slot; the required exact `cli`/neighbor boundary coverage and all-slot-fired checks prevent late delivery or a never-fired timer from producing a pass. The test also validates that the byte at `syscall_exit_swapgs - 1` is `0xFA` (`kernel/src/test_harness.rs:9068`). Xtask applies `-icount shift=0,sleep=off` only to this scenario (`tools/xtask/src/main.rs:6147`), so the instruction-boundary sweep is actually enabled at `NEO_SMP=4`.

4. **Production impact.** The new vector, LAPIC helper, syscall, counters, probe, and harness are scenario/test cfg-gated. The only unconditional additions are the two zero-size global labels in the cfg-exclusive `syscall_entry` assembly definitions. Because exactly one definition is assembled, they neither duplicate symbols nor alter layout. Fresh `llvm-nm` output contained one address for each label.

5. **Cross-family / architecture check.** Intel's specified behavior supports the intended sequence: `CLI` clears IF for maskable interrupts, `SWAPGS` does not alter flags, and `IRETQ` restores RFLAGS from the return frame. This closes the maskable IRQ/IPI window only; it does not claim to make the sequence immune to NMI, machine check, debug/fault delivery, or a faulting return. No additional architecture-specific defect survived review.

## Verification

- RAG health was `healthy` with Ollama and Qdrant available; semantic search was used to locate the exit/audit paths, followed by direct source inspection.
- `NEO_SMP=4 NEO_TEST_ATTEMPTS=1 cargo xtask neo-test syscall_exit_irq_window`: all userspace and scenario kernel builds passed. The local run then stopped before QEMU because `/home/neo/limine` is absent in this Windows checkout, so I could not independently reproduce the runtime PASS/control FAIL here.
- Fresh `llvm-nm` / `llvm-objdump` inspection of the scenario kernel confirmed:
  - `syscall_entry` tail: `cli` at `syscall_exit_swapgs - 1`, then `swapgs`, then `iretq`;
  - test shim frame address: `lea 0x88(%rbp), %rsi`;
  - real interrupt entry's push layout matches that offset.

