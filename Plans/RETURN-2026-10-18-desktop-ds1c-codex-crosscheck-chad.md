<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Desktop DS-1c cross-family review — Chad

**Verdict:** **BOUNCE**

**Reviewed head:** `85f4f5ef1bb7d23126b4fa7b2667c84e9da94369` (`origin/claude/desktop-ds1c-lens`)

**Review base:** `b59c0447` (the candidate's merge-base with `origin/main`)

The normal Lens path is coherent: its authority is attenuated, its single committed frame is copied into WM-owned storage, keyboard and pointer switching preserve the focus route, the independent scene/content/label fixtures reproduce, and the feature builds lint cleanly. The failure path does not uphold the claimed one-bloom resource bound, however, and the root-action classifier does not implement its documented “no other path” rule.

## Findings

### F1 — Medium — A failed Lens bloom can be retried and accumulates unreclaimable WM resources

`launch::serve` rejects another Lens only while `lens.is_some()` (`userland/neo-init/src/desktop_session.rs:1295-1308`). That option is populated only after `lens::bloom` succeeds. Any failed attempt therefore leaves it `None`, returns `STATUS_SPAWN_FAILED`, and lets Arthur issue `list /` again from its normal input loop (`userland/neo-arthur/src/main.rs:850-905`).

This is not merely a wording issue around “one successful Lens”:

1. A failure after `sys_buffer_map(..., 1)` at `desktop_session.rs:2768` leaves a borrowed RO mapping in neo-init. The mapping is explicitly never unmapped; the kernel pins the buffer for the mapping's lifetime (`kernel/src/arch/x86_64/syscall.rs:5836-5865`). Failures after that point include WM-plane allocation/present failure and a Lens exit before the commit reply.
2. Every spawned attempt also installs a `ProcessHandle`. `LensSession::release` passes that value to `sys_channel_close` (`desktop_session.rs:2592-2605`), but `SYS_CLOSE` accepts only File and Channel handles (`kernel/src/arch/x86_64/syscall.rs:4947-4976`); a Process handle returns `EBADF` without being removed. `sys_wait` consumes the exit result, not the caller's handle-table entry.
3. The failure cleanup (`desktop_session.rs:2656-2674`) therefore kills/reaps the child and closes its channels, but cannot reclaim either resource above. Because the attempted-bloom state is not latched, the same session can repeat this until VA/pinned-buffer memory or the WM handle table is exhausted.

The current happy-path QEMU scenario proves exactly one successful bloom, but it does not exercise a failed attempt followed by another request. Fix by latching the Lens request before entering `bloom` (so success or failure spends the one attempt), or make failure cleanup genuinely reclaim mappings and Process handles before permitting a retry. Add a failed-then-second-bloom gate.

### F2 — Low — `wants_root_lens` accepts commands that name an additional relative path

The classifier rejects additional tokens only when they begin with `/` (`userland/neo-arthur/src/showcase_logic.rs:246-262`). Consequently:

```text
wants_root_lens(b"ls / tmp")          == true
wants_root_lens(b"list / ../scratch") == true
```

Both contradict the function and dispatch contract that `/` is named “with no other path.” The test covers `ls / /tmp`, but no relative-path case (`tools/arthur-logic-tests/src/lib.rs:41-62`). This does not widen the Lens's authority—the broker ignores command text and opens the constant `/`—but it silently shows the wrong directory for a command containing another target. Tokenize/parse the accepted grammar positively rather than treating only slash-prefixed tokens as paths, and add the two cases above.

## Numbered cross-check

### 1. WM liveness and teardown

- Arthur is synchronously blocked on its `OP_LAUNCH` reply, so it cannot issue a second protocol request while the WM serves the first Lens. Keys typed during a normal bloom remain on Arthur's active InputFocus queue and are consumed after the reply.
- At every Lens exit point, closing its inherited endpoint makes the WM's blocked/next peer operation return `EPIPE`. For a correct Lens, create → import → commit is strict alternation; after the commit reply it waits on the session endpoint.
- I found no additional deadlock beyond the recorded unbounded wait for a hung Lens. Normal teardown `show_lens(None) → close session → wait → release` wakes the Lens before waiting. Failure teardown `kill → wait → close → release` is also correctly ordered.
- Closing any duplicate closes that channel side globally. The resulting later closes are on distinct inherited/local handles and are harmless/idempotent at the channel-state level; nothing still needed is closed early. The Process-handle pseudo-close is not effective, which contributes to F1.

### 2. Authority

- The intended inventory is session channel at index 0, a deliberately closed placeholder at index 1, and `/` at index 2 with only `LOOKUP | READDIR`. The worker holds no InputFocus and receives no client-selected path.
- The LaunchBroker allowlist admits only probe and Lens, validates the LaunchCap proof/token, and the Lens broker path hard-codes `/`. Arthur cannot select another module or path through this request.
- A second successful Lens is refused, but a failed attempt is retryable (F1). The intent classifier's “no other path” rule is incomplete (F2).
- Ambient `sys_open` and direct `sys_spawn` remain wider than the inherited-cap story; this is the already-recorded OT-66 limitation, not a new DS-1c regression.

### 3. Compositing

- `Scene::present` includes the Lens plane only when `home.is_none()`. Home presents never carry it.
- `show_lens` repaints/presents the halo/frame region and chip-2 glow region separately; it does not create a bounding rectangle across Arthur's client. Window-view presents carry Arthur's client, so base cannot overwrite it.
- Hold/resume damage includes the Lens frame/halo and Spine. Destroy first presents the base without Arthur, then `lens::end` clears and presents all Lens regions, so no stale Lens frame survives.
- The WM maps the Lens buffer RO, snapshots through `SharedPixels`, and composites only the WM-owned `Plane`. No compositor layer retains a pointer into the Lens's writable pages.

### 4. Oracle independence

The NumPy scene oracle independently implements layout, blur, tint, dither, rounded coverage, halo dashes, and drawing order. It does not call neo-atrium. I checked:

- Halo dash: the Lens ring is `(1358,333,384,450)`; `(1358,333)` is an on-pixel because the top-edge offset is `0` and `0 mod 10 < 6`. Its pre-light wide RGB is `(6441,12814,29584)`, light wide RGB `(31823,35446,44982)`, and final RGB `(125,139,176)`.
- Chip 2: `(14,128,56,56)`, center `(42,156)`, samples the cached **Spine** blur. At the center, blur wide RGB `(2856,4102,10983)` becomes tinted `(11441,14547,22574)`; Bayer threshold `40` yields RGB `(44,56,88)`.
- Fit uses the specified `lens.x + lens.w + 30 <= screen.w`; glass and RT fixtures cover focused, selected, Home-held, and Home-chip-selected states.
- The scene mask is seven tight boxes: five label regions plus Arthur and Lens clients. Content and exact label pixels are validated by separate fixtures, not hidden by the scene digest.

### 5. Kill-switch

- With `desktop-lens` off, `BaseScene::with_home` has no Lens layout/blur/regions, the LaunchCap allowlist is probe-only, and Arthur has no Lens action. Existing Home/DI-1a digest KATs remain active and passed.
- `desktop-lens` implies `desktop-home`; with `desktop-home` off, the S3 construction path is unchanged.
- neo-worker enters the Lens role only under `cfg(feature = "desktop-lens")`; feature-off dispatch remains the prior multi-role worker. With the feature on and no handles, the Lens entry fails its first channel operation and retains the DS-1b probe exit behavior.

### 6. Switching and the content oracle

- Hiding a Lens clears `lens_selected`; repaint regions include the second chip's glow reach. `show_home` also clears selection, so selection cannot outlive the Lens.
- Chip presses are consumed by the WM. Lens selection hands focus to the WM route through `HANDOFF`; Arthur's chip hands it back to Arthur. No chip click is forwarded into Arthur's client.
- Tab/Enter from `lens::view` clears Lens selection and resumes Arthur; Escape clears it and returns through Home/hold. `lens::choose` enters only on an empty intent, so a stale selection cannot swallow typed text.
- `lens_content_oracle.py` uses fontTools exact-area outlines and a hard-coded boot-root listing, independent of neo-text and neo-worker. For `scratch`, the derived local glyph window is `(15,64,48,14)` at 14 px/baseline 76. The checker requires every pixel outside all five windows to remain the floor and checks every in-window pixel against ±2/255 bounds. `scratch → scrateh` changes glyph coverage well beyond that slack (and moves ink relative to the expected per-pixel table), so it cannot pass.

### 7. Gate gaps and priority

1. **Failed bloom followed by a second request:** must be next; it exposes F1.
2. **Second bloom after success:** worth a small broker test; the implementation currently refuses it correctly.
3. **No-room 1280×800:** the layout has no Lens and `bloom` fails before spawning/mapping. Low risk, but a host layout/broker refusal assertion would pin it.
4. **RT with Lens:** host digests cover it; add QEMU only if RT hardware-path parity is a release requirement.
5. **Pointer clicks in Home:** route logic is shared and reviewed, but this is the most useful next interaction-coverage rung after F1 because the current pointer capture switches only in the window view.

### 8. Other structural checks

- `LensSession::release`'s attempted Process-handle close is type-wrong, as described in F1. It is masked on the happy path because neo-init exits after this one desktop session.
- The candidate inherits the separate DS-1ab initial-launch present-before-handoff defect. That dependency is recorded as **STILL OPEN** in Chad's DS-1ab fix-forward RETURN and is not counted again as a DS-1c-specific finding here.
- RAG health was `healthy` with Ollama and Qdrant true. It was used for discovery; its results did not surface several current Lens spans, so the authoritative review used direct candidate reads and exact-symbol searches.

## Validation

- `cargo test -p neo-atrium -p atrium-logic-tests -p arthur-logic-tests` — PASS: 194 passed, 0 failed, 2 intentional ignores.
- `cargo clippy -Zjson-target-spec --bin neo-worker --features desktop-lens -- -D warnings` — PASS.
- `cargo clippy -Zjson-target-spec --bin neo-init --features desktop-lens-pointer-boot-test -- -D warnings` — PASS.
- `cargo clippy -Zjson-target-spec --bin neo-arthur --features desktop-lens,desktop-session-boot-test -- -D warnings` — PASS.
- `scene_oracle.py lens-fixture` — exact fixture reproduction after forcing UTF-8 stdout.
- `lens_content_oracle.py fixture` and `labels` — exact fixture reproduction after forcing UTF-8 stdout.
- QEMU scenarios were not run in this review lane.
- `cargo clean` — removed 2,152 build artifacts (563.8 MiB) after validation, per dispatch.
