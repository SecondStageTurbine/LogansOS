<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 336: #58 cross-window capture oracle re-check

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md` §336, running §316 of `Plans/DISPATCH-2026-11-08-relays-315-321.md`.
**Reviewed:** PR #58 head `9a8f014ecd637a1ec095e93c8553d7bccf82367e` (fix `9a8f014e`), against `origin/main` `93dee06723262afb50cfc4229b01877b8a42b138`. Earlier FAIL: `Plans/RETURN-2026-11-03-pr58-recheck-jarvis.md` (the missing cross-window capture oracle). Newest PM note: PR #58 comment, 2026-11-08T07:36:46Z.

**Overall: PASS.** The missing test now exists: the probe app (the second client) is put in the window, the window is dragged over its own client area and released, and the screen is captured at the WM's whole-screen recomposition witness and compared band by band. The old box damage fails the over-present oracle; the capture oracle's sensitivity is shown by the one-pixel band test. The scenario is boot-test only; production is unchanged.

## Checklist

| Check | Verdict | Enforcing lines |
|---|---|---|
| A window is dragged over the second client's pixels and the screen is captured | **PASS** | `tools/xtask/src/main.rs:618-631` (steps), `:1043-1053` (mid capture), `:3561-3566` (scenario entry) |
| The capture is checked against an independent whole-screen recomposition | **PASS** | `userland/neo-init/src/desktop_session.rs:4929-4983`; call `:674-678`; harness `tools/xtask/src/main.rs:22045-22101`; mid-capture machinery `:15039-15041` |
| The oracle fails under `drag-union-damage-control` | **PASS** (over-present oracle) | `tools/xtask/src/main.rs:19333-19338`, `:19378`, `:22012-22037`; control build `:7669-7674` |
| Anything new | **one line** | New scenarios are boot-test only; the existing `desktop_drag_perf_*` pair now also asserts `outside_px` (`tools/xtask/src/main.rs:19312`); production behaviour is unchanged (`Rect::uncovered_px` is only called from the boot-test code). |

## The scenario and the two oracles

**Scenario.** `desktop_drag_cross_boot` waits for `DESKTOP apps front client=1 via=launch`, then sends the 16-report `LONG_TITLE_DRAG`, then waits for `DESKTOP drag cross witness w=` and captures the screen before Super+Tab (`tools/xtask/src/main.rs:618-631,1043-1053`). The PM note records the shape of A1a: it shows one window at a time, so the probe app is the second client in that window (`DESKTOP apps front client=1`), and the window is dragged over that client's own pixels. That is the second-client case the earlier FAIL asked for; no second frame can exist on current `main`.

**Capture oracle.** After the release's move is presented, `drag_perf::witness` recomposes the whole screen from the scene with `scene.compose(true, screen)` — a full `1920x1080` recomposition, no damage tracking — and logs an 8-band FNV-1a digest (`userland/neo-init/src/desktop_session.rs:4929-4983`). It writes only the shadow output plane and does not present, so the display still holds whatever the drag's present left. The harness captures the display at that line and digests it with the same word/byte order (`tools/xtask/src/main.rs:22045-22101`); a stale pixel changes its band and the error names it. The unit test flips one pixel in band 5 and asserts only that band differs (`:24434-24449`).

**Failure under the control.** `drag-union-damage-control` presents the bounding box of the old and new outlines. That box is a superset of the fix's strips, so it leaves nothing stale and its capture matches — the PM note explains this, and it is the correct behaviour. Its fault is over-presenting unchanged pixels, so the oracle that must fail under it is `outside_px` (`require_drag_outside`, `tools/xtask/src/main.rs:22012-22037`), called with `fix = (name == "desktop_drag_cross_boot")` (`:19333-19338,19378`). The control scenario is green only if `outside_px > 0` (`:22030-22036`); the PM's run measured `1,981,107`. The pixel model is host-KAT'd in `tools/atrium-logic-tests/src/rect_uncovered_kats.rs:20-65`, and the oracle's passing/failing cases are pinned in `tools/xtask/src/main.rs:24452-24468`.

## Verification

- `cargo test -p xtask` (at `9a8f014e`): **197 passed, 0 failed, 1 ignored**.
- `cargo test -p atrium-logic-tests rect_uncovered` (at `9a8f014e`): **2 passed, 0 failed**.
- PR run on the exact head: https://github.com/SecondStageTurbine/LogansOS/actions/runs/42453074827 — `completed/success`, `full-alone`, all 4 QEMU shards plus Host, Build Checks, Decoder Fuzz and SPDX. (QEMU was not rerun locally; the review tree has no QEMU executable.)
