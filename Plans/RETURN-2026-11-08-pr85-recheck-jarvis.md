<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: relay 359 — PR #85 item 4 re-check (Jarvis)

Scope: only relay 345's item 4, at PR #85 head `0757ab02c96d3ac8abb261efffd4a7225880745d`.

**PASS — the browser process now has its own bounded spawn budget.**
`userland/neo-init/src/desktop_session.rs:3489-3491` sets
`BROWSER_BUDGET = 256 * 1024 * 1024` and passes it to
`libneo::sys_spawn_budgeted("browser", &[session_client, runtime], BROWSER_BUDGET)`.
This browser launch function is scoped to `atrium-apps-browser` at line 3467.
`kernel/Cargo.toml:1184` enables `mem-quota` for
`neo-test-desktop-browser-boot`.

The wrapper passes the budget to `SYS_SPAWN_BUDGETED`
(`userland/libneo/src/lib.rs:1385-1398`); the kernel converts it to frames
and supplies `Some(frames)` (`kernel/src/arch/x86_64/syscall.rs:2141-2149`),
selecting `spawn_user_by_name_budgeted` at lines 2067-2074.
The THINK now records the browser's own 256 MiB frame account nested under
neo-init's (`docs/THINK-2026-11-08-ot61-first-page-window.md:52`).
The scenario requires the successful-spawn budget line before the browser
connects (`tools/xtask/src/main.rs:20093-20100`).

Validation: source inspection at the exact requested head, confirmed against
the fetched PR #85 head. No build, QEMU run, or allocation-exhaustion test was
performed; this closes the missing bounded-spawn-budget finding only.
