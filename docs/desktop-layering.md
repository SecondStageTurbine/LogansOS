<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# The desktop's layering: a fresh reader's map

*Desktop consolidation build (complexity-governance Rule 4), 2026-10-19. This note is the
fresh-reader test: everything in it is written from the module docs alone (the `//!` headers named
below), not from the THINKs or ADRs. If the note and a module doc disagree, the module doc is wrong
and gets fixed; the note is not where the truth lives.*

## Three tiers, one direction

```
kernel (ring 0)        atrium::buffer   atrium::focus   drivers/usb/hid_*   atrium::pointer_route_logic
   │  shared pixel buffers + fences      kernel-routed input focus          consume-once pointer records
   ▼
neo-init (the WM)      desktop_session.rs: the resident compositor / window manager
   │  owns the output buffer, lays out the clients, composes the whole scene, holds HANDOFF
   ▼
neo-atrium (pure)      lib.rs (normative "Scene specification v1.7"), scene, layout, blur,
                       backdrop, material, shape, motion, keys: no syscalls, host-tested
clients                Arthur (the one owned window); the Lens (one read-only instrument)
```

- **The kernel** contributes only two object kinds (`kernel/src/atrium/mod.rs`):
  - `SharedPixelBuffer`, the client's surface with its present fence (`buffer.rs`);
  - `InputFocus`, one global atomic word saying whose focus may consume input
    (`focus.rs`). Input security and input routing are the same comparison.

  HID keyboard and mouse reports are staged and delivered to that focus.

  The pointer is special. The WM receives every mouse report as a tagged record, chooses the target,
  and hands the tag back. The buttons a client finally sees come from the kernel's copy, never the WM
  (`pointer_route_logic.rs`). The SAK chord is recognized in the kernel (`sak_logic.rs`).
- **The WM** (`userland/neo-init/src/desktop_session.rs`) is `neo-init` staying resident:
  - It owns one full-screen buffer, paints the chrome, and lays clients out. A client never learns
    its position.
  - It recomposes only the damaged rectangle. Its base layer covers every pixel, so nothing an
    earlier owner drew survives the first frame.
  - It composites only its own copy of each client's committed pixels.
  - It holds Arthur's and the Lens's focus handles ACTIVATE-only and never reads their keys.
  - Home reads keys only from the WM's own focus.
  - Every change of owner is a `HANDOFF`: the route moves first, the new owner's queue is emptied,
    then the view is presented.
- **neo-atrium** (`userland/neo-atrium/src/lib.rs`) is the scene as pure integer code. The spec text
  in `lib.rs` is normative and is reproduced independently by `scripts/desktop/scene_oracle.py`
  (plus `field_oracle.py`, `label_oracle.py`, `lens_content_oracle.py` and
  `scripts/fonts/area_oracle.py`).

  Region blurs are cached at construction. Only a frame move or resize blurs again, never a frame
  of animation.

## The layers (each one a Cargo feature of `neo-init`)

Each feature adds one layer. With it off, the WM is the layer below, bit for bit. These are the
kill-switches, and the desktop scenarios hold them.

| Feature | Build | Adds | Held by |
|---|---|---|---|
| `desktop-session` | S1 | Flat chrome, Arthur as the one client | `desktop_session_boot` |
| `desktop-glass` | S3 | The neo-atrium scene: Threshold backdrop, cached frosted glass, Cap-Halo chrome, Plex labels. Or its Reduce-Transparency floor (`reduce-transparency`, or the preference file) | `desktop_glass_boot`, `desktop_glass_rt_boot` |
| `desktop-home` | DS-1a/b | Home and its intent field; Arthur held and resumed (`OP_YIELD_FOCUS`); the LaunchCap broker (`OP_LAUNCH`) | `desktop_home_boot` |
| `desktop-pointer` | DI-1a/b | The sprite, click-to-focus, the SAK, moving the window by its title band | `desktop_pointer_boot` |
| `desktop-lens` | DS-1c | The read-only Lens bloomed from Arthur's `list /`, switched to by its Spine chip | `desktop_lens_boot`, `desktop_lens_pointer_boot`, `desktop_lens_fault_boot` |
| `desktop-resize` | DI-1b | Edge and corner resizes through the client's configure mailbox | `desktop_resize_boot` |
| `desktop-full` | DL-1 | All of the above, the Lens docked to the window | `desktop_full_boot` |
| `desktop-motion` | DM-1 | Every view, Lens and focus change becomes a short transition. Its last frame is the unchanged motion-off present. Reduce Motion: `reduce-motion` or the preference file | `desktop_motion_boot`, `desktop_motion_rm_boot` |

Two details:
- Each layer's `-boot-test` feature adds the witnesses (the `DESKTOP …` log lines the xtask matches
  byte for byte) and ends the session after its scripted round.
- On the kernel side, each scenario is a `neo-test-desktop-*-boot` feature. Since this build they
  stack on two non-scenario bases:
  - `neo-test-desktop-home`: the desktop kernel with the Atrium test objects;
  - `neo-test-desktop-pointer`: that base plus pointer forwarding.

  Each scenario names its own `SCENARIO` and pass string.

## Where the proof lives

- **Host.** neo-atrium's KATs check the Rust against digest fixtures under `fixtures/desktop-*`. The fixtures come from the Python oracles, which share
  no code with the Rust. `scripts/desktop/check_fixtures.sh` regenerates all fourteen and `cmp`s
  them.
- **QEMU.** The xtask has one table, `DESKTOP_SCENARIOS` in `tools/xtask/src/main.rs`. Each row gives
  a scenario's:
  - HID script (built from named shared steps);
  - captures;
  - mid-transition capture;
  - timeout class;
  - band check.

  A unit test holds the table equal to the `desktop_*` entries of `TEST_SCENARIOS`. The captures
  are digested against the oracles.

## I-14 / CP-55 / CP-56 re-verification (this build)

Re-measured on `claude/desktop-consolidation` against `origin/main` @ `9ee63db7`. This build changes
nothing under `kernel/src/atrium`, `kernel/src/drivers` or `kernel/src/arch`: the diff stat is
empty. Its kernel changes are confined to `test_harness.rs` and the `neo-test-*` features in
`kernel/Cargo.toml`.

- **I-14(a), lock isolation on the DELIVERY / HID path.** Every lock on the path is an
  `IrqSafeMutex`, so it is IF=0 by construction:
  - `DELIVERY` (`kernel/src/atrium/focus.rs:70`);
  - both `PRIMARY` statics (`drivers/usb/hid_keyboard.rs:147`, `hid_mouse.rs:181`);
  - both devices' `inner` locks, ordered at `USB_DEVICE` (`hid_keyboard.rs:127`, `hid_mouse.rs:161`).

  A grep for `INFLIGHT` / `RECENCY` / `ARENA` / reservation / `TABLE` over `focus.rs` and
  `hid_*.rs` finds nothing, so no AI-governance lock is taken there. The guard is unchanged. This build
  touches no driver, so I-14(b) and (c) are as `main` left them.
- **CP-55, deferred work bound at completion.** A staged batch carries `ACTIVATION_EPOCH`, which is:
  - read at stage (`focus.rs:144`);
  - bumped on every route change (`:208`, `:225`, `:614`);
  - checked at flush (`:359`).

  A mismatch is dropped and counted in `HANDOFF_DROPPED`. Unchanged. The WM-side half is `HANDOFF`'s
  queue drain at every owner change, which is held by `desktop_home_boot` and the `discard_queued`
  witness (whose log text this build keeps byte for byte).
- **CP-56, IF discipline one lock out.** `DELIVERY`, `PRIMARY` and the device locks are all IF=0, so
  no outer lock in the chain is mixed IF=1/IF=0. The discriminators are still armed and unchanged:
  - the `hid_contention` mixed-IF and teardown legs;
  - the `DELIVERY_IF_ON` witness;
  - the `neo-test-hid-mixed-if-control` control.

**Re-measure (desktop surface, `origin/main` → this build):**

| File | `origin/main` | This build |
|---|---|---|
| `desktop_session.rs` | 4,587 lines | 4,457 |
| neo-atrium `scene.rs` | 974 | 964 |
| neo-atrium `layout.rs` | 573 | 563 |
| neo-atrium `tests.rs` | 1,875 | 1,837 |
| xtask `main.rs` | 14,210 | 14,076 |
| `test_harness.rs` | 33,236 | 33,192 |

- The test harness's `not(feature = "neo-test-desktop…")` ladder clauses went from 19 to 3.
- The per-scenario `desktop_*_boot` match arms in the xtask went from 19 to 2.
- Across the build: 997 insertions, 1,295 deletions.

No invariant row was added, relaxed or removed. No discriminator or control scenario was removed.
Every desktop fixture, oracle output and screenshot is byte-identical to `origin/main`.
