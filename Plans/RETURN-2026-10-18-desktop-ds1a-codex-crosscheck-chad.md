<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Desktop DS-1a cross-family adversarial review (Chad)

**Verdict:** BOUNCE (post-merge: fix-forward required). PR #39 is merged as `5e3261ea`; this review used its current descendants on `origin/main` at `cfafff09`.

**Reviewed:** the merged DS-1a change, current focus routing, Home/Arthur handoff, the session-channel intent framing, `OP_YIELD_FOCUS`, relaunch lifetime, and the Home oracle. The semantic index was healthy (`ollama=true`, `qdrant=true`) and was used for discovery; exact evidence was re-read in source.

## Findings

### 1. HIGH — the view changes before the input route, so transition-time keys are queued for the wrong owner and can be replayed later

**Evidence:** `userland/neo-init/src/desktop_session.rs:488-512,690-704,794-824,1558-1568,1592-1608`; `kernel/src/atrium/focus.rs:88-121,137-153`.

`InputFocus::activate` is an atomic, lock-serialized replacement: there is no instant with two active keyboard routes. The WM ordering around that replacement is nevertheless wrong for the visible ownership transition:

- Initial launch calls `show_window` and presents the switch before it even spawns/handshakes with Arthur. Home remains the active route until after Arthur's first commit and focus-cap exchange. Keys typed at the visible window are queued on Home and are eventually discarded by `return_home`.
- Yield replies to Arthur, paints and presents the complete held-Home view, and only then activates Home. A key typed for the already-visible Home during that interval is queued on Arthur's focus. Arthur is blocked on the session channel, so the key waits and is delivered to Arthur after resume.
- Resume paints and presents the focused Arthur window while Home is still active, then activates Arthur. A key typed for the already-visible Arthur is queued on Home. Nothing drains Home on resume; at the next yield, `read_intent` can consume that stale key (and a stale Enter can submit an unintended intent).

The permanently retained Home RECEIVE handle does not bypass the kernel gate while Arthur is active: `try_recv` checks the active id. The violation is delayed replay after that same Home object becomes active again. This is therefore a real counterexample to the intended semantic “WM-blind to client keys,” not a dual-focus race.

**Repro/reasoning:** inject a key after `scene.present(...)` and before the following `sys_input_focus_activate(...)` in `home::resume`; yield again and observe that `home::read_intent` receives the injected key. The reverse schedule through `home::hold` queues a Home-intended key for Arthur. The 1080p switch repaints a large union, so this is not merely a single-instruction interval.

**Fix direction:** make the route swap the logical transition boundary. Keep Home visible while Arthur is prepared, then activate Arthur and switch the view; on yield, activate Home before doing the held-Home work; on resume, activate Arthur before exposing/accepting resumed input. If visual and input transitions cannot be coupled, explicitly quarantine/drain the losing route at each handoff. Add a deterministic transition-hook test proving a key injected on each side of every swap reaches only the intended destination and is never replayed on a later activation.

### 2. MEDIUM — the independent Home oracle masks the primary interaction entirely

**Evidence:** `fixtures/desktop-ds1/home_digests.rs:3-10`; `userland/neo-atrium/src/tests.rs:454-528,591-655`; `userland/neo-atrium/src/scene.rs:505-558`; `scripts/desktop/scene_oracle.py:52-72,373-395`.

All Home fixture digests hash the entire `field_text` rectangle as zero. That rectangle contains the placeholder, typed intent, scrolling behavior, and caret. The repaint tests compare neo-atrium against another neo-atrium render, so deleting or identically misplacing `paint_field_text` on both sides still passes. The focus-state test still has the field glow even if the caret disappears. The QEMU readback consumes the same masked fixture.

The later exact-label fix in PR #43 independently covers the shared bar/chip labels, but its five runs do not include the Home placeholder, typed text, or caret. Consequently, deleting the visible text/caret — the primary DS-1a interaction — leaves the independent Home oracle green.

**Repro:** remove the call to `paint_field_text` in `paint_home_view`/`paint_home_rect`. The masked Home digests and full-vs-damage self-comparisons remain insensitive to the deletion (the existing label checks continue to cover only the static S3 runs).

**Fix direction:** add independent, tight field fixtures for at least empty/focused (placeholder + caret), a short typed intent, an unfocused/no-caret state, and a long left-scrolled intent. Keep the rest of the dynamic field masked from the scene digest, but make each named interaction fail independently when its ink or caret is removed or shifted.

## Dispatch checklist

1. **WM-blind:** ACTIVATE and RECEIVE remain disjoint, and the kernel's route replacement is exclusive. Finding 1 shows that the WM/client sequencing around the swap still assigns keys to the wrong logical owner and can replay them after reactivation.
2. **Return path:** after Destroy and `sys_wait`, the route remains on Arthur's focus until `return_home` activates Home. Because the WM retains the activator, this is a live object with an unreachable/dead-client queue, not an invalid pointer. The drain correctly runs after activation and is bounded at 256 receives; the focus queue is bounded at 64. It removes initial-launch type-ahead, but it does not repair the hold/resume replay in finding 1.
3. **`I` message:** the channel provides one synchronous message boundary. WM sends at most 121 bytes (`I` + `MAX_INTENT=120`); Arthur receives into 193 bytes, checks the tag, accepts empty as the chip action, and rejects non-printable bytes. The only sender is the peer endpoint and the message names no surface/peer, so this is not an INV-WM-1 identity-confusion issue. A dedicated ADR-073 verb would improve extensibility, but the present closed framing is unambiguous and not a finding.
4. **Yield/hold/resume:** opcode 7, payload 32, identity at 24..48 and non-zero LE serial at 48..56 match the hand-laid oracle. Registry lookup binds identity and provenance, rejecting a peer/foreign/dead surface. The rigid handshake rejects yield-before-attach. A pipelined double yield or commit blocks on the synchronous surface channel while Home owns the server loop, then is processed after resume; it cannot act for a peer, though serial freshness is not enforced. Finding 1 covers the wrong-route window around hold/resume.
5. **Relaunch:** the registry is per-session, handles are closed, and scene focus state is rebuilt sufficiently for a second cooperative round. The compositor's 640×400×4 read-only mapping has no unmap and its Pmap pin retains the old buffer frames, so every graceful round leaks 1,024,000 bytes of physical backing plus monotonically allocated VA until neo-init exits. The current production Arthur has no normal exit path, and the address range is very large, so I record this as a lifecycle note rather than another bounce finding; a future production close/relaunch path must add unmap/reuse first.
6. **Oracle:** the Python renderer independently reimplements the prose geometry/material arithmetic and regenerated all checked boxes/digests. Finding 2 is the important masked hole. PR #43 improves shared static-label coverage but does not close the dynamic field.
7. **Other:** the Home line editor is bounded, printable ASCII only, and its HID map rejects releases/non-key events. The 256-iteration return drain is intentionally larger than the 64-event queue and cannot spin forever. No additional INV-WM-1 or bounds defect survived re-verification.

## Validation performed

- `cargo test -p neo-atrium -p atrium-logic-tests` — PASS (neo-atrium 22 passed / 2 ignored; atrium-logic-tests 95 passed).
- `python scripts/desktop/scene_oracle.py home-fixture` — generated fixture data matched `fixtures/desktop-ds1/home_digests.rs` (banner excluded because PowerShell decoded the em dash differently).
- `NEO_SMP=4 cargo xtask neo-test desktop_home_boot` — NOT RUN TO QEMU: the build completed, but xtask stopped because `/home/neo/limine` is absent in this Windows worktree (`LIMINE_DIR` not configured). This is an environment prerequisite, not evidence against either static finding.
