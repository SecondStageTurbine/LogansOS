<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: relay 345 — PR #85 review (Jarvis)

Scope: PR #85's L5 changes, reviewed against its #81 base (`4000afd0`).

1. **PASS — `desktop_browser_boot` checks every client pixel except the WM pointer, against an independent hand-derived frame.** `tools/xtask/src/main.rs:22419` skips only the 12×18 pointer sprite; the expected black border and white remainder are at `tools/xtask/src/main.rs:22399-22405`. The frame is independently stated in `userland/layout-smoke/src/browser.rs:104` and `docs/THINK-2026-11-08-ot61-first-page-window.md` §3.

2. **PASS — the browser buffer hash is independently checked.** `tools/xtask/src/main.rs:22414` feeds hand-derived expected pixels into the FNV-1a computation, independent of the renderer and browser buffer; the logged browser hash is compared at `tools/xtask/src/main.rs:22485`. That frame yields `abeb310785ba4095`. The browser-side buffer hash is at `userland/layout-smoke/src/browser.rs:115`.

3. **PASS — the browser joins as an ordinary WM client and does not present from std.** It sends `CREATE_SURFACE`, imports a client buffer, and commits damage in `userland/layout-smoke/src/browser.rs:190, 205-221, 297`; the WM receives the buffer with `MAP_READ | QUERY | PRESENT` at `userland/layout-smoke/src/browser.rs:214-219`, and the manifest says commit damage does not present (`tools/ot61-pal-manifest.toml:83`). The browser gets the session channel plus a derived `BrowserRuntime` restricted to `USE | GRANT` and `THREAD_SPAWN` in `userland/neo-init/src/desktop_session.rs:3476-3484`; it grants the WM only `ACTIVATE` for its input activator at `userland/layout-smoke/src/browser.rs:232-236`.

4. **FAIL — the WebRender/browser process has no bounded scenario-scoped spawn budget.** `userland/neo-init/src/desktop_session.rs:3484` calls `sys_spawn_client`, which follows ordinary `SYS_SPAWN`; it does not call `sys_spawn_budgeted`. The child therefore shares its caller's memory account rather than receiving the bounded budget claimed in `docs/THINK-2026-11-08-ot61-first-page-window.md` §4-5. Concrete input: add a browser-side allocation loop that repeatedly grows a `Vec<u8>`; the scenario has no child quota that makes the allocation fail at a set limit.

5. **PASS — features are off by default and the browser client is opt-in.** `userland/neo-init/Cargo.toml:34` has `default = []`; `atrium-apps-browser` is an explicit feature at `:94`, while the kernel's `neo-test-desktop-browser-boot` opt-in is defined at `kernel/Cargo.toml:1183`. With these features off, the browser slot and launch allowlist additions are cfg-gated in `userland/neo-init/src/desktop_session.rs:3445, 4079`; the THINK §5 records the default-image kill switch.
