<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: CP-49 `with_user_access` nesting cross-check (Jarvis / Codex)

**Reviewed:** `origin/claude/adoring-mendel-kgbzsi` @ `912b0a31`  
**Verdict:** **MERGE**  
**RAG:** healthy (`ollama=true`, `qdrant=true`) at review time.

## Findings

No merge-blocking or nit-level correctness findings survived re-verification.

The guard is deliberately not a complete containment mechanism for a closure that
blocks while AC=1: a successor that never opens another user-access window can run
with SMAP suspended. The branch states that limitation accurately in CP-49 and does
not claim that the new entry check replaces a future scheduler-side AC assertion.

## Adversarial checks

1. **False positives.** I traced the live-AC state from every installed entry.
   `enable_sse` clears AC before publishing `SMAP_ACTIVE` (`kernel/src/arch/x86_64/mod.rs:155-158`), and each
   AP calls it before meaningful Rust (`kernel/src/arch/x86_64/smp.rs:213-219`). SYSCALL masks AC in SFMASK;
   `swapgs_irq_entry`, both timer variants, and `set_exception_entry!` mask AC before Rust. The exception
   macro covers NMI and #DF as well as #PF (`idt.rs:77-136`), including their IST configuration. The ring-3
   #PF kill path therefore reaches `scheduler::exit()` with live AC clear. A first-run kernel thread inherits
   the switcher's live flags, but a correct switch site has AC=0; a first-run user thread's `iretq` uses its
   constructed flags. The only production `popfq` sites are the entry masks, and they clear AC. I found no
   correct path that calls the helper with live AC set.

2. **False negatives.** `SMAP_ACTIVE` is Release-published after CR4.SMAP and AC clearing, and Acquire-read by
   the helper. AP bring-up enables CPU-local SMAP before entering the rest of AP initialization. The entry AC
   read detects a direct nest before its `stac`. A leaked AC reaches the next window and is detected unless an
   intervening entry mask or `clac` clears it; the documented blocking-hole case can remain undetected when no
   later window is opened, but that is explicitly out of scope rather than a false claim. All current callers
   were rechecked: their closures are bounded copies, scalar reads/writes, or volatile accesses; none nests,
   blocks, yields, or allocates.

3. **Ordering.** `x86_64::registers::rflags::read_raw()` is `pushfq; pop` with
   `options(nomem, preserves_flags)`. Rust inline asm is side-effecting unless marked `pure`; neither this read
   nor `stac`/`clac` is `pure`. The local `stac`/`clac` wrappers also intentionally omit `nomem`, so they are
   compiler memory barriers (`mod.rs:316-333`). The compiler cannot merge the two entry reads or sink one past
   the corresponding side-effecting `stac`.

4. **Preemption.** Both FXSAVE and XSAVE timer stubs copy the interrupted `iretq` frame onto the interrupted
   thread's kernel stack, clear only live AC before Rust, and leave the saved frame untouched. A timer-driven
   switch therefore runs the next thread with live AC=0; when the old thread is eventually resumed, its own
   timer epilogue restores its saved frame, including the window's AC=1. The generic IRQ stub has the same
   clear-live/preserve-frame structure. An interrupt-context user copy can consequently open and close its own
   window without closing the interrupted one.

5. **Discriminator.** `run_user_access_nesting` rejects SMAP-off execution, maps a present USER page, verifies a
   successful single window and AC=0 afterward, then arms both the expected CR2/#PF control and the panic flag
   only after the outer window is open. `panic_failed` accepts only a panic whose location is
   `arch/x86_64/mod.rs` and whose message contains the guard marker. With the guard removed, the nested inner
   `clac` makes the final outer access fault at the separately expected `user_va + 2`; the success path cannot
   pass via a different panic or via SMAP being absent.

6. **CP-49 text and Linux comparison.** The stated uncovered blocking cases agree with the switch mechanics:
   cooperative `context_switch` does not save RFLAGS, while timer preemption preserves flags in the per-thread
   interrupt frame. The Linux raw sources support the comparison: x86 `user_access_begin/end` expand through
   `stac`/`clac` (`arch/x86/include/asm/uaccess.h`), and objtool rejects both `recursive UACCESS enable` and a
   call to a non-UACCESS-safe function while UACCESS is enabled (`tools/objtool/check.c`). The text does not
   overstate this as runtime containment.

7. **Other structural checks.** Clearing AC before the panic avoids running formatting/panic machinery with
   SMAP lifted. The check is gated by the same `active` snapshot as the later `stac`/`clac`, so one invocation
   cannot observe inconsistent enablement. The panic is an availability response to an already-invalid kernel
   state, not a new user-triggerable condition on a correct entry path.

## Verification

- `cargo xtask neo-test user_access_nesting`: the userspace and kernel scenario compile successfully.
- Runtime execution was unavailable in this Windows seat because xtask expects Limine at
  `/home/neo/limine`; all four launch attempts stopped at that environment prerequisite. This is not a source
  failure and does not contradict the dispatch's reported 4/4 local PASS/control results.

