<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #55): boot-keys cut and startup timeline (#118)

**Reviewed SHA:** `911a75bc61601a8ffbe73476c789c929240739f8`
**Base:** `5f31dbf2`
**RAG:** healthy (`ollama=true`, `qdrant=true`).
**Build:** not run, per dispatch. The merge gate reported 23/23.

## Checklist

1. **PASS — the cut is one-shot.** `InputFocus::activate` calls the hook for each successful route activation ([focus.rs:200-216](../kernel/src/atrium/focus.rs#L200)). The i8042 hook captures `HEAD` only with `compare_exchange(CUT_UNSET, ...)`; once applied, the cut becomes `CUT_DONE`, so Arthur activation or a handoff back to Home cannot replace it ([i8042.rs:96-118](../kernel/src/drivers/ps2/i8042.rs#L96)).

2. **PASS — cut arithmetic is wrap-safe and bounded by HEAD.** `tail_after_cut` compares wrapping distances and returns the cut only when it lies in the inclusive wrapping interval `tail..=head`; otherwise it returns the existing tail ([i8042_logic.rs:96-112](../kernel/src/drivers/ps2/i8042_logic.rs#L96)). The host KAT covers `u32` wrap and an out-of-interval cut ([i8042_tests.rs:291-306](../tools/xtask/src/i8042_tests.rs#L291)). The pump reads only while `tail != HEAD` and advances tail one byte at a time ([i8042.rs:445-454](../kernel/src/drivers/ps2/i8042.rs#L445)); the producer enforces the 256-byte occupancy bound before writing ([i8042.rs:237-244](../kernel/src/drivers/ps2/i8042.rs#L237)). A cut older than the retained interval resolves to `tail`, so it cannot advance beyond HEAD or skip post-cut bytes.

3. **PASS — reset covers decoder state and delivery state.** Under the pump lock, applying the cut advances the tail, then resets `Decoder`, modifiers, SAK filter, input stage, and (when enabled) autorepeat before decoding remaining bytes ([i8042.rs:120-145](../kernel/src/drivers/ps2/i8042.rs#L120)). The A5 host KAT models Enter followed by a held A with no break, then verifies the post-cut A press and release reach a fresh decoder ([i8042_tests.rs:308-353](../tools/xtask/src/i8042_tests.rs#L308)). The replay omits the break but does not fill all 256 ring slots; it validates the lost-break state, not physical ring saturation.

4. **PASS — lock and IRQ context.** `apply_boot_cut` is called while `STATE.lock()` is held ([i8042.rs:492-505](../kernel/src/drivers/ps2/i8042.rs#L492)). The `dropped N` message is emitted after leaving that scope and inside `enter_stage_only`/`leave_stage_only` ([i8042.rs:545-550](../kernel/src/drivers/ps2/i8042.rs#L545)). The IRQ handler remains the lock-free `try_drain_port` path and the reviewed diff adds no ISR lock ([i8042.rs:261-270](../kernel/src/drivers/ps2/i8042.rs#L261)).

5. **PASS — legacy kill-switch path.** The check is direct control-flow inspection: with `ps2-boot-keys-legacy`, `note_route_activated` returns before loading or recording a cut ([i8042.rs:111-118](../kernel/src/drivers/ps2/i8042.rs#L111)); `apply_boot_cut` then returns `None` for `CUT_UNSET`, and the ordinary pump still decodes from tail through HEAD ([i8042.rs:125-129](../kernel/src/drivers/ps2/i8042.rs#L125), [i8042.rs:503-509](../kernel/src/drivers/ps2/i8042.rs#L503)). This preserves the prior backlog delivery sequence. There is no dedicated legacy-feature test or `.text` identity check in this cut.

6. **PASS — timeline hooks and overlay width.** The route/key/receive/Arthur/model hooks use relaxed atomic loads, stores, increments, or compare-exchange and contain no lock, allocation, or wake ([watchdog.rs:164-218](../kernel/src/watchdog.rs#L164)). The watchdog module and timeline module are both compiled only with `freeze-watchdog` ([main.rs:89-92](../kernel/src/main.rs#L89)); new call sites are similarly feature-gated in focus and syscall code ([focus.rs:213-216](../kernel/src/atrium/focus.rs#L213), [syscall.rs:982-987](../kernel/src/arch/x86_64/syscall.rs#L982)). The overlay inserts the line into its paint list ([watchdog.rs:397-424](../kernel/src/watchdog.rs#L397)); the host KAT checks the maximum line fits 180 columns and that its glyphs exist ([lib.rs:213-235](../tools/fb-progress-logic-tests/src/lib.rs#L213)).

7. **PASS — gate coverage identified.** `ps2_keyboard` exercises QEMU PS/2 IRQ/decode input, `ps2_key_repeat` exercises held-key repeat, and `desktop_home_boot` exercises the desktop route and handoff flow ([main.rs:3154-3174](../tools/xtask/src/main.rs#L3154), [main.rs:2301-2303](../tools/xtask/src/main.rs#L2301)). The host KATs separately check cut arithmetic and the held-key lost-break replay ([i8042_tests.rs:291-353](../tools/xtask/src/i8042_tests.rs#L291)). These gates do not establish real silicon/EC i8042 timing or firmware behavior; QEMU and host logic tests cannot cover those.

## Overall verdict

**PASS.** The one-shot cut, wrapping bounds, state reset, lock context, legacy path, and gated timeline hooks satisfy the checklist. The legacy path was verified by source control flow; it lacks a dedicated test or text-identity check. QEMU does not validate real i8042/EC timing.
