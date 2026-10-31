<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Desktop DS-1a fix-forward cross-check (Chad)

**Verdict:** BOUNCE / PARTIAL. **F1: STILL OPEN. F2: CLOSED.** Reviewed candidate `a409b5eea3b959955023fa7abe90747b71163250` from `origin/claude/desktop-s1-session-boot` against the pre-fix line and the original Chad return `a93f6342`.

The semantic index was healthy (`ollama=true`, `qdrant=true`) and was used for discovery; exact evidence was re-read from this worktree.

## Finding verdicts

### F1 (HIGH) — STILL OPEN: launch still presents the window before the route swap

**Evidence:** `userland/neo-init/src/desktop_session.rs:503-522,655-720,1813-1837`; `tools/atrium-logic-tests/src/handoff_kats.rs:89-163`; `tools/xtask/src/main.rs:245-282,10055-10084`; `docs/THINK-2026-10-18-ds1-home-spine-shell.md:89-108`.

The hold, resume, return-to-Home, pointer escape and pointer refocus paths now use `HANDOFF = [Swap, Discard, Present]`, and Arthur's `D` acknowledgement makes its discard complete before the WM presents. Those changes close the replay schedules on the hold/resume path.

The initial launch path, however, still performs a view change outside `HANDOFF`:

1. Home receives Enter in `home::read_intent`.
2. `run` immediately calls `scene.show_window()` and presents the whole view (`desktop_session.rs:507-513`). Home's focus is still the active keyboard route.
3. Only after spawning Arthur, completing create/grant/import/attach/commit/focus handshakes and receiving the activator does `arthur_session` run the launch `HANDOFF` (`:691-720`).

A key typed after the present at `:511` and before the swap at `:697` is therefore queued on Home although the window view is already visible. Arthur's discard cannot remove it because only the active route can be received from and Arthur drains only its own focus queue. The suspended Home loop does not read it. On Arthur's eventual exit, `home::return_home` activates Home and discards the key at `:1820-1823`. Thus it is not replayed into a later Home turn, but it is assigned to the wrong owner and lost. This directly contradicts the fix-forward's claims that launch uses the three-step transition and that, once the new view is on screen, every key reaches its owner.

**Why the gates miss it:** `handoff_kats` begins each modeled transition inside `hand_over`; it cannot observe an earlier `show_window`/present outside that abstraction. Its ADR-074 rule that only the live route may be drained is faithful, and its negative controls genuinely distinguish the old hold/resume order. The QEMU `Esc + zz + Backspace + Backspace` injection is also a real discriminator for the old hold order, but it injects only during hold, not during the long initial launch preparation window.

**Required closure:** keep Home visible while Arthur is prepared, then perform the launch's swap, Arthur discard/ack, and window present inside the one `HANDOFF`; alternatively add an explicit state whose visible ownership and routing contract is tested. Add a launch-boundary discriminator that types after the first window present and proves the input reaches Arthur rather than being discarded on return to Home.

### F2 (MEDIUM) — CLOSED: the Home field now has an independent exact oracle

**Evidence:** `scripts/desktop/field_oracle.py`; `fixtures/desktop-ds1/field_text.rs`; `userland/neo-atrium/src/lib.rs:86-106`; `userland/neo-atrium/src/scene.rs:505-560`; `userland/neo-atrium/src/tests.rs:798-893`; `kernel/src/test_harness.rs:2558-2590`; `tools/xtask/src/main.rs:10031-10035`.

`field_oracle.py` does not call neo-text or neo-atrium. It uses fontTools outline geometry through the independent exact-area renderer, independently measures advances/kerning, and places/blends those results over `scene_oracle.py`'s Home background. The checked fixture covers empty focused, typed focused, typed unfocused, and long scrolled focused states. The digest masks only the state's tight run/caret window plus the already separately checked bar/chip label boxes; every other field pixel remains pinned to the independent background.

For the long state at 1920x1080 I re-derived `T=(664,419,676,40)`, advance `w=727.596`, `room=672`, `x0=664+672-727.596=608.404`, and caret `floor(x0+w+1)=1337`, at y=427. The resulting checked window is `(664,426,676,26)`, matching the generated fixture.

The per-pixel bounds are tight enough to reject the supplied similar-letter mutation (`hello desktop` to `hello desktoq`) even with coverage slack of +/-2/255. A caret shifted one pixel necessarily leaves exact Electric Blue missing at an expected pixel and/or present where the digest/bounds expect background. The typed-unfocused state and the chip-selected negative control both reject a stray caret. Masking the bar/chip label boxes here is acceptable because the exact label oracle separately pins those shared runs; it no longer hides any part of the primary field interaction.

## Dispatch checklist

1. **F1 route/discard ordering:** all DS-1 route swaps found by exact `sys_input_focus_activate` inspection are inside `HANDOFF` except initial focus creation and the feature-off S3 path. The production hold/resume/return and pointer paths order swap before discard before present. The launch has the pre-handoff present described above, so F1 remains open.
2. **F1 later-turn schedules:** Arthur discards on every launch/resume intent and acknowledges before present. Home discards after becoming the route on hold/return/escape. Refocus intentionally does not discard because it resumes the same Arthur turn; keys while escaped route to Home. No second replay schedule survived re-verification.
3. **F1 discriminators:** the host model accurately represents active-route receive and rejects all four bad abstract orders. The QEMU injection would fail DS-1a's old hold order, but neither gate covers the initial launch pre-present.
4. **F2 independence and geometry:** re-read the Python renderer, font outline oracle, Rust field painter, fixture, negative mutations, and empty-field QEMU readback. Re-derived the scrolled origin/caret above. F2 is closed.
5. **Kill-switch:** with `desktop-home` off, the Home state/loop/handoff code is cfg-elided and the existing window-view bit-identity KAT (`home_build_keeps_the_window_view_bit_identical`) passes. The non-Home S3 activation order remains its prior direct activate/present/K path. No kernel change is part of F1/F2 beyond #48's separately reviewed staging epoch.
6. **What the gate does not cover:** launch-time input between the early window present and Arthur's route swap; field states other than empty in QEMU (the host exact KAT covers them); nondeterministic scheduler/USB timing beyond the deterministic handoff model.

## Validation

- `cargo test -p atrium-logic-tests -p neo-atrium` — PASS: 98/98 atrium logic tests; 23 passed / 2 ignored neo-atrium tests.
- `python scripts/desktop/field_oracle.py fixture` — generated content semantically matches `fixtures/desktop-ds1/field_text.rs` (only Windows stdout encoding/newline representation differs in a raw byte comparison).
- `python scripts/desktop/scene_oracle.py home-fixture` — generated content semantically matches `fixtures/desktop-ds1/home_digests.rs` under the same Windows encoding normalization.
- QEMU scenarios were not run; the surviving F1 schedule is established directly by the visible present, delayed route swap, and focus-queue ownership, and is outside the existing scenario's injection point.
