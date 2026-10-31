<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# OT-88 prep: prior-art survey for a terminal emulator

Draft for the OT-88 THINK (terminal, file manager, settings; `Plans/DECISION-2026-10-27-release-tiers-mvp.md`). This file covers the terminal only. It is input for the THINK, not a design.

## What the tree has today

- **No escape-sequence interpreter.** Arthur's transcript and the Home and WM text go through neo-text's `TextFlow`, which draws proportional text and knows `\n`, `\t`, backspace and nothing else. OT-57 (the VT100/ANSI interpreter, `docs/OPEN-THREADS.md`) is banked but not built.
- **No PTY.** Nothing in `kernel/` or `userland/` connects a child's stdin and stdout to another process as a terminal. `neo-shell` is the Arthur slice-1 shell, not a line-discipline host.
- **A monospace face exists.** neo-text builds with `plex-mono` (IBM Plex Mono), which a cell grid needs.
- **Keys arrive as HID usages** (kind-1 `InputEventV1`). A terminal has to encode them back into bytes (arrows → `CSI A`…, PgUp → `CSI 5 ~`), the reverse of what A9-C did for Arthur.

So a terminal is four pieces: a parser, a screen model (cell grid, attributes, scrollback), a renderer onto an Atrium surface, and a PTY seam with key encoding. Prior art covers the first two well, in licenses we can vendor. The renderer and the PTY seam are ours.

## How the facts were gathered

- Each head date and SHA comes from `git clone --depth 1 --filter=blob:none` of the default branch on 2026-10-27, and each license from the license file or `Cargo.toml` at that commit.
- `no_std` claims come from the crate's own `lib.rs` (`#![cfg_attr(not(feature = "std"), no_std)]`) and its `[features]`, not from docs.
- The GitHub REST API returns 403 from this container, so there are no issue or release counts.
- Nothing was copied.

## Candidates

| Candidate | License | Head (date, SHA) | What it is | Fit for us | What we'd take |
|---|---|---|---|---|---|
| **vte** (`alacritty/vte`) 0.15.0 | Apache-2.0 OR MIT | 2026-10-25, `89c9c22f5b8f` | The parser only: Paul Williams' DEC state machine, calling a `Perform` trait for print, execute, CSI, ESC, OSC and DCS. | **Good.** `no_std` when `std` is off; deps `arrayvec` and `memchr`, both `no_std`. The optional `ansi` feature adds a higher-level handler (it pulls `log`, `bitflags` and `cursor-icon`). | The parser layer, as a dependency or vendored. Alacritty and `vt100` both build on it (their `Cargo.toml` files at the heads above), so it's well exercised. |
| **vt100** (`doy/vt100-rust`) 0.18.1 | MIT | 2026-10-19, `fb0f54ec7222` | The parser plus a screen model: a cell grid with attributes, cursor, modes, scrollback, and diffing between screens. Renderer-agnostic. | **Best fit.** `no_std` + `alloc` when `std` is off (`lib.rs:36`, `extern crate alloc` at `:52`). Deps: `vte` (no default features), `itoa`, `unicode-width`, optional `embedded-io`. All compatible licenses. | The candidate to depend on or vendor under `third_party/vt100/`: it gives us parser and screen state, leaving only rendering and the PTY. |
| **alacritty_terminal** (`alacritty/alacritty`) 0.26.1-dev | Apache-2.0 | 2026-10-26, `bbb2cbf9470a` | A full terminal core: grid, selection, search, vi mode, and a PTY event loop. | **Poor to vendor.** std-only, tied to the host OS (`libc`, `polling`, `parking_lot`, `home`). | Design reference: grid storage and how the PTY loop feeds the parser. |
| **wezterm-escape-parser** (`wezterm/wezterm`) | MIT | 2026-10-26, `e68a8ec27727` | A parser (on `vtparse`) that decodes into a typed action enum: CSI, OSC and DCS as Rust types, not raw parameters. WezTerm's `term` crate holds the screen model. | Parser: `no_std` when `std` is off (`lib.rs:10`). `term`: std, with heavy deps (`image`, `terminfo`, `url`). | Possibly the typed decoding layer, if `vt100`'s coverage falls short. `term` is a reference for a modern feature set (kitty keyboard protocol, sixel). |
| **rio-vt** (`raphamorim/rio`) | MIT | 2026-10-26, `ec9ca5043e14` | "Embeddable terminal core extracted from Rio": VT state machine, grid, PTY driver, selection, search. | std (`parking_lot`, `regex`, `tracing`). A recent extraction, so its API may not be stable. | Design reference only. |
| **libghostty-vt** (`ghostty-org/ghostty`) | MIT | 2026-10-26, `68640f3049d5` | Ghostty's terminal core as a library: Zig, with a C API (`include/ghostty/vt.h`). | **No** for code: it needs the Zig toolchain or C FFI into our `no_std` userland. | Reference for behaviour. Its tests could supply conformance cases, fetched raw (the byte-oracle rule). |
| **libvterm** (`neovim/libvterm`) | MIT | 2025-11-20, `934bc2fbf218` | A C terminal library. This repo is a fork that its README says is **no longer used**: neovim bundles vterm in its own tree, and upstream is Bazaar at leonerd.org.uk (not fetched). | **No** (C, and this fork is dead). | Nothing beyond the principle. |

**Not a candidate:** zellij is a multiplexer, not a terminal core, and the clone under `a-kenji/` is a stale fork (2025-10-21), not `zellij-org`.

## Leaning, for the THINK to test

1. **Depend on or vendor `vt100`, which brings `vte`.** It's MIT, `no_std` + `alloc`, renderer-agnostic, and alive (a commit this month). It covers parser and screen state. That is most of OT-57's banked interpreter work, done upstream. The THINK should verify three things:
   - its sequence coverage against what the T1 shelf's crossterm apps emit;
   - its scrollback model;
   - that it builds on our userland target with `std` off.
2. **The renderer is ours:** `vt100`'s `Screen` cells drawn in Plex Mono through neo-text onto an Atrium surface, with damage from `vt100`'s screen diff.
3. **The PTY seam is ours, and the deepest piece:** how a child's stdout reaches the terminal and its keys go back. It's either a kernel PTY object or a userland pipe pair with a line discipline. It touches spawn and handle-passing (OT-61 std threads and exit_group), so it's the item most likely to need a kernel slice and the concurrency pitfall bank.
4. **Key encoding** (HID usage → bytes) is a small, table-driven module, with a hand-laid byte oracle for each key.

**Rework of OT-57's estimate in our units:** OT-57 banked "~500 lines, 1–2 person-weeks" for a from-scratch interpreter. With `vt100` the parser and screen are borrowed, so the dependency chain is: PTY seam → renderer → key encoding → a QEMU scenario driving a real TUI app. The PTY seam carries the uncertainty. The other three are mechanical.

## Licenses of the transitive deps `vt100` brings

| Crate | License |
|---|---|
| `vte` | Apache-2.0 OR MIT |
| `arrayvec` | Apache-2.0 OR MIT |
| `memchr` | Unlicense OR MIT |
| `itoa` | Apache-2.0 OR MIT |
| `unicode-width` | Apache-2.0 OR MIT |

All would be compatible with our MPL-2.0 tree. Only `vte`'s license was read from source here; the other four are as commonly published and **not yet verified**. Check each against its own `Cargo.toml` and license file at import.
