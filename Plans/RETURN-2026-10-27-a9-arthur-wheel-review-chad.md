<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: relay #104, A9-C2 mouse-wheel / touchpad review

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. Cross-family review of Claude-authored pointer ABI, queue, and IRQ-safe delivery code.

**Overall: FAIL.** Two confirmed wheel-specific defects: forwarded wheels can evict keystrokes, and a combined press/wheel report can deliver a new press through a stale grab before RESYNC. Recommend **revert-first** for #162 (`9a1c35bb`), then re-land with these cases covered. This return changes documentation only; no repair or revert was performed.

- Reviewed: `9a1c35bba3f98ecfbab4b22d4dc06eca4b077ab7`, PR [#162](https://github.com/SecondStageTurbine/LogansOS/pull/162).
- Worktree: `E:\AI Automation\workers\chad`; branch `codex/a9-arthur-wheel-review-chad`, based on the reviewed SHA as dispatched.
- Read the bundle and #104 dispatch from `origin/main`, local AGENTS.md and full CLAUDE.md, and CP-37/52/54/56. Applied rust-best-practices and systematic-debugging; investigated/reproduced before making recommendations.
- RAG: healthy (`ollama=true`, `qdrant=true`); ran the dispatch's `pointer route wheel mint_records input queue coalesce` query. Direct source reads and exact-symbol searches checked the reviewed tree, rather than treating the main-tree RAG as reviewed-SHA evidence.

## Checklist

| # | Verdict | Evidence |
|---|---|---|
| 1 | PASS | Eight-byte layout remains unchanged; signed kind-2 `code` is preserved by `kernel/src/atrium/input_logic.rs:40` and `userland/libneo/src/lib.rs:2711`. Exact searches for `InputEventV1`, `INPUT_EVENT_POINTER`, event receive/decode, and numeric kind checks found no kind-2 reader requiring `code=0`. PR table covers the active pointer/queue and WM/Arthur/Worker consumer families. Additional generic transport/key-only readers (syscall encoder `kernel/src/arch/x86_64/syscall.rs:7915`, SAK `kernel/src/atrium/sak_logic.rs:63`, repeat `kernel/src/atrium/key_repeat_logic.rs:69`, watchdog `kernel/src/watchdog.rs:182`, and neo-init key/probe loops) preserve it or filter by kind first. Independent literal byte arrays pin kind 2, kind-4 WHEEL, and kind 5 at `tools/atrium-logic-tests/src/pointer_route_kats.rs:51` and `:71`. |
| 2 | FAIL | Raw wheels are excluded from motion coalescing and the superseded-transition scan (`kernel/src/atrium/input_queue_logic.rs:125`, `:211`), and an incoming raw wheel with unchanged buttons drops on a key-filled queue (`:168`). However forwarded **kind 5** is absent from both `is_pointer` (`:38`) and `carries_wheel` (`:48`), so it reaches the key-eviction fallback (`:173` through `:188`). F1 below reproduces actual-capacity keystroke loss. No-wheel predicates and kind-3 behavior are unchanged; key-only KAT passes (`tools/atrium-logic-tests/src/input_queue_kats.rs:99`). |
| 3 | FAIL | In isolation WHEEL has `prev==buttons`, cannot be a transition, and never takes RESYNC; pending RESYNC waits for a motion (`kernel/src/atrium/pointer_route_logic.rs:162`, `:189`). The integration fails CP-52: `kernel/src/atrium/focus.rs:474` eagerly mints both records before either enqueue; the second mint/enqueue can lose an earlier release after the new press's flags are already fixed. F2 below proves the missed release and stale-client forwarding. The existing isolated-ring KAT (`tools/atrium-logic-tests/src/pointer_route_kats.rs:102`) misses this queue/ring interaction. |
| 4 | PASS | Same `DELIVERY` guard covers minting and consumption (`kernel/src/atrium/focus.rs:70`, `:383`, `:389`, `:548`, `:568`). `IrqSafeMutex::lock`/`try_lock` mask before acquiring, and drop unlocks before restoring IF (`kernel/src/sync/irqsafe_mutex.rs:44`, `:63`, `:134`). No new lock or caller context. The existing PRIMARY/device acquisition discipline is unchanged by the two driver edits; no production path acquires DELIVERY with IF enabled. CP-54 capped routes remain 32 clients plus the pointer route (`kernel/src/atrium/focus.rs:43`, `:438`). Test-only control acquisition remains deliberately gated. |
| 5 | PASS | Wheel branch forwards only when an offered activator and `ctx.client.contains` both match (`userland/neo-init/src/desktop_session.rs:4202`); otherwise it consumes/drops. Local subtraction is inside that rectangle, hence already bounded to client dimensions (`:4204`); pointer integration clamps to screen (`:4230`). Home/Lens-only paths supply default contexts with no target (`:358`, `:3778`, `:5084`); escaped/disabled Arthur supplies no activator (`:436`). No wheel branch changes keyboard focus, buttons, or grab. This checks the implemented session contexts; it makes no claim that a future lock-screen implementation exists. |
| 6 | PASS | USB signed byte (including -128) is already i8; absent fourth byte is zero (`kernel/src/drivers/usb/hid_logic.rs:67`). I2C extracts signed descriptor fields and clamps to its existing [-127,127] decoded range, absent wheel zero (`kernel/src/drivers/i2c_hid/hid_logic.rs:545`, `:595`). Drivers pass those exact decoded values, or zero when disabled (`kernel/src/drivers/usb/hid_mouse.rs:282`, `kernel/src/drivers/i2c_hid/touchpad.rs:1359`). Kind-2 writer sign-extends into i16 (`kernel/src/atrium/input_logic.rs:48`). This passes through the touchpad mouse collection's wheel; it does not implement a PTP two-finger gesture decoder. |
| 7 | PASS | Kind 5 decodes signed i16 (`userland/neo-arthur/src/showcase_logic.rs:85`); delta is `-notches * 3` in i64 and each scroll call clamps to kept/bottom bounds, resetting `view_top=None` on reaching bottom (`userland/neo-arthur/src/showcase_text.rs:327`, `:331`, `:335`). Multi-notch forward/back rendering equality passes (`tools/arthur-logic-tests/src/showcase_s2_tests.rs:468`). The prior #100 finding about a saved view after later transcript-cap eviction is inherited from #159 and is separate from these wheel-specific failures. |
| 8 | PASS | Both producers force wheel zero when `atrium-pointer-wheel` is off (driver citations above), yielding today's kind-2 bytes and one motion record. WM without `desktop-wheel` drops WHEEL records (`userland/neo-init/src/desktop_session.rs:4218`). Kernel dependency and scenario pairing are explicit (`kernel/Cargo.toml:697`, `:820`); product enables the two together (`tools/xtask/src/main.rs:5269`, `:5683`), checked by `:18199`. |
| 9 | PASS | Real HMP mouse inputs are +1 then -1 (`tools/xtask/src/main.rs:10574`); script waits on Arthur's first reply and scroll witnesses (`:432`). Checker requires both WM-to-Arthur forwards and exactly `view_top=11` then follow (`:8555`), and is invoked by scenario validation (`:13647`). A dropped wheel/wrong target fails or times out. This happy path does not fill either queue and therefore does not cover F1/F2. |
| 10 | PASS | PR #162 contains the pitfall table, driver trap account, and survey with dates/SHAs/licenses; no upstream implementation was copied in the diff. Independently checked Wayland mirror commit `cd0d1543c093` date 2025-07-20 and Smithay `76118ad7796e` date 2026-10-26 via GitHub commits API, and raw MIT/Expat license files linked below. evdev is GPL-2.0 and explicitly principle-only with its failed-clone limitation disclosed. Smithay supports focus-based axis delivery, though its public handle also delegates through grabs; the PR's hover-regardless-of-grab rule is this project's policy, not a claim that every Smithay grab does so. |

## F1: a forwarded wheel evicts a real key

Actual client capacity is 64 (`kernel/src/atrium/focus.rs:37`). While Arthur is busy and not draining, queue 64 ordinary key events, HID usages 4 through 67. Hover over Arthur and forward wheel +1. `forward_record` creates kind 5 (`kernel/src/atrium/focus.rs:571`) and calls the same `enqueue` (`:582`). The queue classifies it as neither pointer nor raw wheel. With no motion or superseded transition, it removes the first non-pointer, which is key 4. Result: `EvictedKey`, first remaining key 5. A wheel has displaced a keystroke.

The identical queue with an incoming raw kind-2 wheel instead returns `DroppedIncomingMotion` and retains key 4. This isolates the missing forwarded-wheel classification. The existing wheel KAT constructs only kind 2 (`tools/atrium-logic-tests/src/input_queue_kats.rs:323`), so it stays green.

Minimal expectation test, using the existing mounted-module imports in `input_queue_kats.rs`:

```rust
let mut q = InputQueue::with_capacity(64);
for c in 4..68 { q.enqueue(InputEventV1::key(c, 0, true)); }
let w = InputEventV1 { kind: 5, flags: 0, code: 1, x: 0, y: 0 };
// FAIL: actual is EvictedKey, and the first remaining code is 5.
assert_eq!(q.enqueue(w), Enqueued::DroppedIncomingMotion);
assert_eq!(q.pop_front().unwrap().code, 4);
```

Separate retention observation: 64 raw wheel reports followed by a key leave 63 wheels; the fallback `unwrap_or(0)` (`kernel/src/atrium/input_queue_logic.rs:187`) removes one when no key/transition exists. It was **not** evicted by the superseded-transition scan. This contradicts the stronger source/PR wording "never evicted", but F1's verdict does not depend on prioritizing wheels above incoming keys. A full queue dropping an **incoming** wheel is permitted. Also checked a sole release plus 63 kind-5 wheels: another kind-5 wheel retains that release and evicts an older wheel; that is not a claimed production release defect.

## F2: press plus wheel can outrun RESYNC and reuse a stale grab

Concrete sequence, no concurrency timing needed beyond the WM not draining during the batch:

1. Mint and consume left-down tag 1 over Arthur. WM now has `buttons=1`, `Grab::Client(Arthur)`.
2. Mint left-up tag 2, but leave it unread in the WM queue. Kernel ring now has buttons 0.
3. Mint/enqueue 62 wheel-only reports, tags 3 through 64, buttons 0. WM queue has 63 records: `[release2, wheel3, ..., wheel64]`. Wheel handling never runs the WM button decision.
4. Receive a new left-down **with wheel +1** (optionally dy=-127 so the pointer moves off the old client). `focus.rs:474` eagerly mints motion 65 and wheel 66. Motion 65 overwrites the already-consumed slot 1: flags are 1, no RESYNC. Wheel 66 overwrites the live release slot 2: sets pending RESYNC, but WHEEL deliberately cannot carry it.
5. Enqueue motion 65: queue fills to 64. Enqueue wheel 66: kind-4 records use the generic queue policy, evicting release 2 at the head. `note_lost` happens only at `focus.rs:486`, after both records' flags are fixed. The whole operation holds the same DELIVERY guard (`:383`/`:389`), so RECV cannot drain between these mints/enqueues.
6. WM drains wheels 3..64 without changing its old buttons/grab, then motion 65. `pointer_grab::decide(1, 1, true)` returns all false (`userland/neo-atrium/src/pointer_grab.rs:27`): no cancellation and no release. The old Client grab receives tag 65 (`userland/neo-init/src/desktop_session.rs:4315`); kernel consumes it as `PHASE_DOWN`, because its ring predecessor had buttons 0. RESYNC appears only on the next motion, tag 67, too late to prevent this new press reaching the old target.

Essential combined-module expectation test (same imports as `pointer_route_kats.rs`):

```rust
let mut ring = RecordRing::new();
let mut q = InputQueue::with_capacity(64);
let down = ring.mint(0, 0, 1).unwrap();
ring.consume(down.code).unwrap();
q.enqueue(ring.mint(0, 0, 0).unwrap()); // release tag 2
for _ in 0..62 { q.enqueue(ring.mint_wheel(1).unwrap()); }
let records = [Some(ring.mint(0, -127, 1)), Some(ring.mint_wheel(1))];
for r in records.into_iter().flatten() {
    if !matches!(q.enqueue(r.unwrap()), Enqueued::Appended | Enqueued::Coalesced) {
        ring.note_lost();
    }
}
let press = std::iter::from_fn(|| q.pop_front())
    .find(|r| r.flags & WHEEL == 0).unwrap();
assert_eq!(press.code, 65);
// FAIL: actual flags=1, with the old grab still live.
assert_ne!(press.flags & RESYNC, 0);
```

The isolated ring's "wheel never carries RESYNC" property holds; the integration ordering makes it insufficient. The regression discriminator must exercise both records, ring overwrite, queue overflow, and the WM grab decision together.

## Validation and evidence limits

- Fresh local `cargo test -p atrium-logic-tests -p arthur-logic-tests -p xtask --offline`: **PASS**, Atrium 137, Arthur 88, xtask 141, zero failures; doc tests also pass. These existing tests do not contain F1/F2.
- Compiled a temporary standalone Rust harness under ignored `target/review104/`, mounting the exact reviewed `input_logic.rs`, `input_queue_logic.rs`, `pointer_route_logic.rs`, and `pointer_grab.rs`. Assertions confirmed the actual F1/F2 outcomes above, the raw-wheel control, and the retained-release negative control. No repository source/test files were edited.
- Independently queried [batch-7 gate run 40465526041](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40465526041): **success**, host/product/SPDX and all four QEMU shards green. Run head is `0b9298b5313f42a50e17e13d64ae20d94f7273bc`, not the squash SHA and not its descendant; fetched it and verified no differences against the reviewed SHA in the queue, focus, record ring, desktop session, or Arthur text implementation. This is existing gate evidence, not a fresh QEMU rerun or proof against F1/F2. PR body separately records `desktop_wheel_boot` and related scenarios on its implementation head.
- Prior-art licenses read from raw pinned sources: [Wayland COPYING](https://raw.githubusercontent.com/gitlab-freedesktop-mirrors/wayland/cd0d1543c093/COPYING), [Smithay LICENSE.txt](https://raw.githubusercontent.com/Smithay/smithay/76118ad7796e/LICENSE.txt). Commit dates independently matched the survey. No GPL code copied.
- Review discovery: one `graft ask ... --source`, reported **~224,351 tokens saved** total for this review.

**Disposition:** FAIL items 2 and 3; revert-first recommendation for the PM/PL. Preserve both overflow discriminators when re-landing. Items 1 and 4-10 pass within the stated scope.
