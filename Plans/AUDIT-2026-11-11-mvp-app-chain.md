<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# AUDIT: the MVP app chain against `origin/main` (2026-11-11)

**Seat:** Seat E. **Dispatch:** `Plans/DISPATCH-2026-11-11-seat-e-mvp-app-chain-audit.md`.
**Audited at:** `origin/main` `6c3c4dcd` (2026-11-11).
**Method:** code, not docs. For each rung I checked:
- the kill-switch feature in a `Cargo.toml`;
- the syscall numbers in `userland/libneo/src/lib.rs`;
- the crate under `userland/`;
- the scenario names in `tools/xtask/src/main.rs`;
- the commit that introduced each one (`git log -S`/`-G`).

**About the commits:** this repository's history starts at the public release `4a3f3ea1` (2026-10-31), which squashed the private history. So "landed in `4a3f3ea1`" means before the public tree; the private PR is named where the code says it.

## 1. State of every rung

Legend: **landed** (cited), **partial** (what is missing is named), **not started**, **in flight** (an open PR).

### OT-88 app model (`docs/THINK-2026-10-29-ot88-app-model.md` §7)

| Rung | State | Evidence on `origin/main` | Notes |
|---|---|---|---|
| A0 | landed | the THINK itself | |
| A0k | **landed** | `ipc-try-send` (`kernel/Cargo.toml:566-570`); `SYS_TRY_SEND` (`libneo/src/lib.rs:319`); `sys_try_grant` used by the WM's `Link`; scenario `ipc_try_send` | Came in with A1a, `4f8ffb6c` (2026-11-01, "carried over from private #220") |
| A1a | **landed** | `atrium-apps` (`neo-init/Cargo.toml:92`); the client table (`desktop_session.rs`, `apps::Apps`, `front: ARTHUR` at :1903); `MODULE_PROBE_APP = 8` (`atrium_launch.rs:37`); `neo-worker/src/probe_app.rs`; scenarios `desktop_apps_boot`, `_fault_`, `_hang_`, `_stall_`, `_drop_`, `_handoff_death_`, `_reuse_`, `_flood_`, `_grab_`, `_arthur_hang_`, `_arthur_yield_hang_boot` | `4f8ffb6c`. One window on screen at a time, as A1a specifies |
| A1b | **not started** | no frame per client, stacking order or side-by-side scenario; `front` is a single index | |
| A1c | **not started** | no per-window move, resize or snap; DI-1b's move/resize is Arthur's one window | |
| A3p | **not started** (its loop exists) | `ITEMS_MAX = 8` (`kernel/src/sched/wait_any.rs:46`). `desktop-park` exists (OT-22, `neo-init/Cargo.toml:140`; scenario `desktop_idle_parks`) but is not the image-A default | The rung is the 8 → 16 bound plus the image-A default |
| A2 | **not started** (#98 adds one launch) | no Spine app chips or `terminal`/`files`/`settings` Home intents on main. #98 adds `MODULE_TERMINAL = 4` and Arthur's `open terminal` (`claude/l1a3-terminal`) | #98 overlaps A2's launch path; see §3 |
| T1 | **landed** (as L1a.1) | `pipe` (`kernel/Cargo.toml:575-578`; `6a565270`, 2026-11-03, "L1a.1 step 2a"); `SYS_PIPE_*` and `sys_spawn_stdio` (`libneo/src/lib.rs:330-336`, `:3234`); the std overlay's `stdio_neoplutarch.rs`; scenarios `pipe_core`, `pipe_park_kill`, `pipe_stdio` (`b2d20587`, 2026-11-09, "pipe 2b") | The ledger names it L1a.1, not T1 |
| T2 | **partial / in flight** | the core, `userland/neo-term-core` (parser subset, key encoder, grid), is in `ea33980f` (#42, 2026-11-02, L1a.2). The window, `neo-terminal`, is #98 (open) | |
| K1b | **landed, in its scoped form** | `cap-rooted-open` (`kernel/Cargo.toml:111`, `4a3f3ea1`); `OPEN_SCOPE_DIR` (`libneo/src/lib.rs:935`); `ARTHUR_SCOPES` (`:1224`); scenario `desktop_arthur_scope` (in the product image's kernel features; xtask asserts it at `main.rs:27230`) | `open_at` was **not** built (OT-66 THINK, "K1b as built", lines 171-188). See §4 |
| F0 | **not started** | no `mkdir_at`, `unlink_at`, `rename_at` or `stat_at` syscall, no `neofs-dir-ops` feature | Its dependency (a directory handle with rights) is in place: `OPEN_SCOPE_DIR` handles carry READ/WRITE bounds |
| F1 | **not started** | no `neo-files` module or Files client | |
| S1 | **partial** | the store and `OP_SETTING` are landed: `atrium-settings-store` (`neo-init/Cargo.toml`, `4a3f3ea1`), `settings-core`, `OP_SETTING` (`atrium_protocol.rs:51`), live apply (`desktop_session/settings.rs`), the `login_settings_store` scenario and the `settings-cut-test` cut. The **settings app** is missing: `SETTINGS_APP_MODULE = "settings"` (`settings.rs:195`) names a module that doesn't exist, and Arthur stands in for it in the boot tests | The setup menu (S1-S5) writes the same store |
| S2 | **not started** (its pieces exist) | idle lock (`lock.rs`, `lock-idle-test`) and the login screens exist; Settings' "Idle lock" and "Change password" rows don't, because the app doesn't | Needs S1's app |

### OT-67 editor (`docs/THINK-2026-10-29-ot67-editor.md` §7)

| Rung | State | Evidence | Notes |
|---|---|---|---|
| E0 | landed | the THINK | |
| E1 | **landed** | `userland/neo-edit-core` (`gap`, `lines`, `document`, `history`, `cursor`, `format`, `wrap`), `4a3f3ea1`. Its 23 KATs cover the THINK's list: random edits against a model, undo and redo, grapheme stops on the vendored Unicode 17 test, word and line moves, find, the text test, CRLF round trip, wrap and its cache | Not built into any image yet, as the rung says |
| W1 | **landed** | `neofs-file-replace` (`kernel/Cargo.toml:1941-1943`, `4a3f3ea1`); `SYS_FILE_REPLACE = 24` and `sys_file_replace` (`libneo/src/lib.rs:165`, `:1304`); scenario `neofs_file_replace` (`main.rs:6983`, not ignored). OT-93, the teardown row it needed, is on main (`kernel/Cargo.toml:782-786`) | |
| C1 | **not started** | no `atrium-clipboard`, `OP_CLIP_SET` or `OP_CLIP_GET` | Needs A1a-A1c |
| E2 | **not started** | no `neo-editor` or `MODULE_EDITOR` | |
| E3 | **not started** | | Needs C1 |

### File viewer (`docs/THINK-2026-11-08-file-viewer-prior-art.md` §6)

| Rung | State | Evidence | Notes |
|---|---|---|---|
| V0-V4 | **not started** | no registry, `neo-view` or viewer crate | V0 and V1 also need **L2d's `neo-view`**, which isn't on main. V0 needs F1 and E2 too. None of the V rungs is on the path to the MVP's "open and save" |

## 2. The critical path to "Files opens a file in the editor and saves it"

That sentence is E2's gate: open a file from Files, type, save, reopen, and read back the bytes. E2 needs E1 ✔, W1 ✔, A1 (A1a ✔, A1b ✗, A1c ✗), A2 ✗ and F1 ✗. F1 needs A2 and F0, and F0 needs K1b ✔.

```
A1a ✔ ── A1b ── A1c ── A2 ──┐
                            ├── F1 ── E2   (goal)
K1b ✔ ── F0 ────────────────┘
E1 ✔, W1 ✔ ───────────────────────┘
```

- **Longest chain:** A1b → A1c → A2 → F1 → E2, five rungs in series. F0 runs beside the first three and must land before F1.
- **Gates:** six 4-shard gates (A1b, A1c, A2, F0, F1, E2), plus their bounce cycles.
- **AAG:** only F0 on the path touches the kernel, so it alone needs the AAG and the pitfall-bank pass. The other five are WM and app work.
- **A possible shortcut, which is J.c.'s call, not a recommendation.** The path is long because of D1, side-by-side floating windows (A1b, A1c). If J.c. accepted Files and the editor opening one window at a time for the first cut (A1a's model), then A2 → F1 → E2 could follow A1a directly. That would take A1b and A1c off the critical path, cutting it from five rungs to three after F0. A1b and A1c would then come back as their own lane. The THINK chose D1 deliberately, so this needs his decision.

## 3. The next buildable rungs

A rung is buildable if every dependency is landed and it doesn't conflict with a lane in flight: #98 (terminal), #100 (S4) and the setup menu, OT-100 R2 (USB write path), #81 and #85 (browser), and #102 (tick diagnostics).

**1. F0, NeoFS directory operations: recommended first.**
- **Dependencies, all landed:** K1b's scoped directory handles carry the rights F0 checks, and #180's commit barrier is on main (`kernel/src/fs/neofs/commit_logic.rs`, with the deferred free in `alloc_logic.rs`).
- **Off the WM:** it touches no WM code, so it doesn't conflict with #98 or #85.
- **#100:** it adds a syscall in `syscall.rs`, which at most means a textual merge at the syscall table.
- **OT-100:** R2 is the USB driver's write path. R3, the mount, is a later lane and may meet F0 at mount time, not before.
- **It is the only kernel rung on the critical path**, and it runs in parallel with the WM chain. Starting it now takes its AAG and Muse cycles off the end of the path.
- **Gate:** kernel-touching, so the **4-shard gate (full-alone)** with the §4.2 byte oracles and a crash cut after each write of each operation. Then the **AAG** with the pitfall-bank pass on the NeoFS commit path and handle rights. The cross-family lens is the PM's call: it is the NeoFS commit path, not the SMP core. **Muse** reads the rights checks.
- **Kill-switch:** `neofs-dir-ops`. Off means none of the four calls exists.

**2. A1b, N windows on screen: the head of the WM chain, best started once #98 lands.**
- **Dependency:** A1a is landed.
- **Overlap:** it rewrites the client presentation in `userland/neo-init/src/desktop_session.rs`, where #98 adds 111 lines (its launch path, `MODULE_TERMINAL`) and #85 adds 114 to `neo-init`. Starting it beside them means rebasing over both.
- **Gate:** not kernel-touching, so the **4-shard gate** for the WM scenarios in §7, plus the usual cross-family review. No AAG.
- **Kill-switch:** `atrium-apps`.
- **If J.c. takes the shortcut in §2,** A2 replaces A1b here, with the same sequencing behind #98, since both edit the launch path.

**Also buildable, but off the path:** A3p (kernel, `wait_any`'s bound; AAG, CP-14 and CP-61) has its dependency landed and conflicts with nothing in flight. It is worth a free seat; it doesn't move the MVP.

## 4. The records that disagree

- **K1b.**
  - **ADR-010** says "since K1b (2026-10-29)"; the **OT-66 THINK** and the ledger call K1b open.
  - **The code** has K1b as built in L1.1: scoped directory caps (`OPEN_SCOPE_DIR`, `ARTHUR_SCOPES`, `desktop_arthur_scope`, under `cap-rooted-open`). `open_at` was not built. The OT-66 THINK's "K1b as built" (lines 171-188) says exactly this, and that `open_at` "can still come later."
  - **So K1b is landed for everything the app chain needs.** F0 uses directory handles, and the rights rule already holds for them. The "open" entries refer to `open_at`, which no MVP rung needs. The ledger and the OT-66 THINK's §2 status lines (lines 31-37, 90) should say "K1b landed (scoped caps); `open_at` deferred".
- **Rungs landed under other names.** T1 is L1a.1 (pipe steps 2a and 2b), and T2's core is L1a.2 (#42). A tracker keyed on the THINK's rung names reads them as not started.
- **S1 is half done.** The store and `OP_SETTING` are landed, and the setup menu writes the same store. The settings app, which S2 also needs, doesn't exist. `SETTINGS_APP_MODULE` names a module that isn't in the tree.

## 5. Summary

| | Rungs |
|---|---|
| Landed | A0, A0k, A1a, T1 (L1a.1), K1b (scoped form), E0, E1, W1 |
| Partial | T2 (core landed, window in #98), S1 (store, no app), A3p (loop exists, bound and default not) |
| In flight | T2's window and a first launch path (#98) |
| Not started | A1b, A1c, A2, F0, F1, S2, C1, E2, E3, V0-V4 (V0 and V1 also wait on L2d) |
| Critical path | A1b → A1c → A2 → F1 → E2, with F0 in parallel (five rungs after F0; three if J.c. takes §2's shortcut) |
| Next | **F0** now (kernel: 4-shard, AAG, Muse); **A1b** after #98 lands (WM: 4-shard, cross-family review) |
