<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: the first page in a LogansOS window (OT-61, the window rung)

**Seat D, 2026-11-08.** This is the last rung of `docs/THINK-2026-11-01-ot61-first-page-layout.md` §5, called "L4" there: "the browser app renders L3 into its Atrium buffer and the compositor shows it". In #81, L4 is the text check, so this rung is called **L5** in code and serial lines.

It builds on:
- #81: L0-L4, Servo's layout without script rendered by WebRender on SWGL into a caller buffer;
- #78: SWGL into an Atrium buffer;
- #79: the read-only grant to a compositor;
- #80: `channel::try_recv` and `input` in std.

## 1. Who presents

The browser never presents. `SYS_FB_PRESENT` has no std trampoline and is kept out of the A4 manifest on purpose (`tools/ot61-pal-manifest.toml`, rung 4 S3's note). The rendering-stack THINK (`docs/THINK-2026-10-31-ot61-r4-swgl-floor.md` §3, "What it leaves to rung 6") gives presenting to the WM: neo-init's desktop session.

So in this rung the browser joins the session as **one more client in the A1a client table**, the way OT-88's design B describes an app (`docs/THINK-2026-10-29-ot88-app-model.md` §2.3). It is its own binary, launched by module id. It gets the session channel as its only handle, and it does the same handshake as the probe app (`userland/neo-worker/src/probe_app.rs`):
1. `CREATE_SURFACE`;
2. the surface channel arrives by grant;
3. the client creates its buffer and grants it with `USE` and `MAP_READ | QUERY | PRESENT`;
4. a full-surface first commit;
5. `FOCUS_REQUEST`, with an ACTIVATE-only activator granted to the WM;
6. per frame: `commit_damage`, then `OP_COMMIT_DAMAGE` on the surface channel.

The WM copies the committed rect into its scene and presents its own full-screen plane. That part is unchanged.

## 2. The pieces

| Piece | Where | Change |
|---|---|---|
| Module id | `libneo/src/atrium_launch.rs` | `MODULE_BROWSER = 5` (4-7 are the apps' ids; 4 is the terminal's in L1a), name `"browser"`, with its own surface namespace and buffer provenance. The kernel already resolves `/modules/neo-<name>` (`sched/process.rs` `spawn_user_by_name`), so the kernel needs no change. |
| WM client table | `neo-init/src/desktop_session.rs` `apps` | One more slot, behind a new neo-init feature `atrium-apps-browser`: `index_of(MODULE_BROWSER)`, a spawn like `spawn_probe_app`, and the import keyed by the browser's namespace and provenance. The geometry stays the client size the WM already accepts: **640 x 400**, `ROLE_TEXT`. Resizing is a later rung, so this rung changes no WM rule. |
| The browser binary | `userland/layout-smoke`, a second bin `neo-browser` | Same crate and lock as `layout-smoke`, so the closure is built once. It renders the L3 oracle page at a 640 x 400 viewport into the mapped Atrium buffer (SWGL's default framebuffer at the buffer's stride). It speaks the client protocol in std (`channel`, `atrium`, `input`), then keeps answering the WM until the session channel closes. |
| Protocol bytes | the browser | The NMC1 records are re-encoded in std code (libneo is `no_std` with raw syscalls, which A4 forbids in a std bundle). A byte oracle backs them: hand-laid `CREATE_SURFACE` / `IMPORT_ATTACH_BUFFER` / `COMMIT_DAMAGE` records from ADR-073's tables, asserted in a host test against both our encoder and libneo's, so a drift on either side fails. |
| Launch | neo-init, test feature | The scenario's neo-init opens the browser at session start, the way `desktop_apps_boot`'s Arthur launches the probe app. |
| Scenario | xtask `DESKTOP_SCENARIOS` row `desktop_browser_boot` | Stages `/modules/neo-browser` (the 8-module limit holds: neo-init, shell, arthur, worker, browser). A screendump is taken after the WM logs the browser's first commit. The check reads the capture at the client rect `atrium_desktop::layout(w, h, 640, 400)` gives. |

## 3. The oracle

The L3 frame at 640 x 400 is still hand-derived. The oracle page has no backgrounds and one border: `#b`'s 2 px black ring on its border box x 8..122, y 90..144, which doesn't depend on the viewport width (CSS 2.1 §10.3.3, as worked in `neo-static-page/src/oracle.rs`). Everything else is white.

In the capture, every pixel of the client rect must equal that frame. XRGB in the buffer becomes RGB in the PPM; WebRender's white is `ff ff ff` and the border `00 00 00`. Then the rect's hash is recorded as a regression check, never as the oracle.

The browser also logs the frame's SHA-256 over its own buffer before it commits. The capture's client rect must hash the same: the THINK's "compositor's readback hash == L3's hash".

## 4. Failure modes and discriminators

| Likely failure | Discriminator |
|---|---|
| The WM refuses the handshake: size, role, rights or provenance | The WM logs which step refused (the existing `connect` error path), the browser logs the reply code, and the scenario's serial needles name the step. |
| The frame is right in the buffer and wrong on screen (stride, offset, XRGB vs RGB order) | The buffer hash next to the capture rect hash. Equal hashes mean the scene and copy are right and any mismatch is in the oracle; unequal means the copy or position. The capture check prints the first wrong pixel in both client and screen coordinates. |
| The capture is taken before the commit lands | The screendump is keyed to the WM's commit log line, not to the browser's. |
| Memory: WebRender in a 640 x 400 window inside the session's budget | The browser's spawn budget is set by the scenario. An allocation failure aborts with exit 134 and the WM logs the client's death (peer death is treated as destroy, ADR-073). |
| Timing: the WM's bounded tries time out while the browser is still starting WebRender | The handshake is done **before** the first render (`CREATE_SURFACE` … import), and the first commit follows the render. If the WM's commit wait is bounded below the render time, the WM's log says so. Then the render happens before `CREATE_SURFACE`, and nothing is sent until the frame is ready. |

## 5. Kill-switch and the default build

- Everything is behind `atrium-apps-browser` (neo-init) and the scenario's features.
- The default image doesn't stage `neo-browser`, and the WM's table without the feature is byte-for-byte today's.
- The `apps` module is compiled with and without the feature in CI's neo-init clippy list.

## 6. Prior art and pitfalls

- **Prior art:** none new to borrow. This joins our own WM protocol (ADR-073, OT-88), as the probe app does.
- **Concurrency:** the browser's render thread and its protocol loop are one thread, so the new code adds no cross-CPU, lock or atomic code. The pitfall bank's classes are N/A apart from the existing channel rendezvous, whose bounded tries the WM already owns (CP bank, OT-88 entries).

## 7. Order

1. libneo ids, and the byte oracle (host test).
2. The browser binary's protocol + render, against a host-side fake WM in the same test.
3. The WM slot behind `atrium-apps-browser`.
4. The scenario and capture check.
5. Close-gate, quick tier, PR stacked on #80 and #81.
