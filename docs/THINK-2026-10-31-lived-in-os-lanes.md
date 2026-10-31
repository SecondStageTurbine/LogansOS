<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: three lanes to make LogansOS feel lived-in (2026-10-31)

**Status:** proposed, for dispatch when the seats resume after the move to the public repo.
**Asked by:** J.c., after a GitHub survey of Rust projects that could round out the OS.
**Prior-art rule:** CLAUDE.md, "Unfamiliar features: survey open source before writing from scratch". The table in
section 2 is the survey; each lane's dispatch repeats the rows it uses and re-checks them on the day it starts.

## 1. What we already have (not lanes)

The survey listed several components LogansOS already carries. Checked on main, 2026-10-31:

| Survey item | Already in LogansOS |
|---|---|
| smoltcp (TCP/IP) | A kernel dependency (`kernel/src/net/smoltcp_wrap.rs`), alongside the kernel's own TCP, IPv4, IPv6, DHCPv4, DNS, ARP and NDP. |
| rustls (TLS) | In `libneo`, with a custom crypto provider. The fetch broker is public-only by policy, not for lack of TLS. |
| FAT (embedded-sdmmc) | `rust-fatfs` (MIT) is vendored in `third_party/fatfs`, but used only by the host image builder. Lane 3 below. |
| Fonts / text (ab-glyph, cosmic-text) | `neo-text` on `ttf-parser`. cosmic-text (shaping, bidi) is a later upgrade, not a gap. |
| 2D rendering (tiny-skia) | The browser renders through WebRender on SWGL (OT-61 rung 4). tiny-skia only matters for non-browser app canvases. |
| A std / libc layer | **Built by OT-61:** a real Rust `std` target (`x86_64-unknown-neoplutarch`, std overlay plus `neo-std-pal` on our syscalls) and `neo-crt` (musl plus libc++, #261). The relibc question is mostly answered: neo-crt is the C runtime, and a second libc would be duplication. |

## 2. Prior-art survey (checked 2026-10-31)

Last commit and license were read directly from each repository (shallow clone; `git log -1`; the license file and
`Cargo.toml`), not from a summary. Archived status was not checked (no API access from the PM seat); a lane re-checks
it when it starts.

| Project | Last commit | License (checked) | Fit for LogansOS | What we'd take |
|---|---|---|---|---|
| ilai-deutel/kibi (editor, ≤1024 lines) | `d42f6e3` 2026-10-27 | MIT OR Apache-2.0 | std plus a raw-mode terminal; tiny, so the port is mostly the terminal layer. Needs Rust 1.99 / edition 2024 (check our toolchain). | The code, vendored. |
| redox-os/sodium (vi-like editor) | `43feb13` 2024-12-03 | MIT | Designed for a Rust OS, but almost two years quiet: borderline-abandoned by the rule. | Read for design; vendor only with a named maintainer. |
| redox-os/ion (shell) | `e9b3ca4` 2026-09-07 | MIT | std, plus git dependencies on gitlab.redox-os.org (`calc`, `liner`, `small`) and termion. A real port, not a drop-in. | The code, vendored with its git deps pinned. |
| redox-os/termion (terminal) | `c784cec` 2025-11-21 | MIT | The raw-mode/ANSI layer under Ion and sodium; its Unix backend expects termios. | A LogansOS backend for it, or our own small equivalent. |
| uutils/coreutils | `cac8be1` 2026-10-31 | MIT | std-heavy (fs, process, env); per-utility crates, so we can take a few at a time. | The code, per utility, as our std PAL covers what each one needs. |
| Frommi/miniz_oxide (DEFLATE) | `7fc2dd4` 2026-10-29 | MIT (crate `Cargo.toml`) | `no_std` + `alloc`. Drop-in. | The code. |
| image-rs/image-png | `b2f10c6` 2026-09-23 | MIT OR Apache-2.0 | Decoder on top of miniz_oxide/fdeflate; alloc-friendly. | The code. |
| etemesi254/zune-image (JPEG etc.) | `2f9086b` 2026-10-24 | MIT OR Apache-2.0 OR Zlib | `zune-jpeg` is `no_std` + alloc. | `zune-jpeg` (and `zune-png` as an alternative to image-png). |
| image-rs/image (umbrella) | `075e65d` 2026-09-15 | MIT OR Apache-2.0 | Assumes std; pulls every codec. | Not the umbrella: take individual codecs. |
| rafalh/rust-fatfs | `38b3a79` 2026-10-16 | MIT | Already vendored at `38b3a795`. `no_std` + alloc. | Reuse the vendored copy in the OS. |

No candidate is GPL, LGPL or AGPL. Helix (MPL-2.0) and Slint (mixed terms) from the original survey are out of scope
for these lanes.

## 3. Lane 1: terminal, editor, shell, basic tools

The biggest day-to-day gap: there is no text editor, and `neo-shell` is a minimal capability-aware REPL.

**The real first step is a terminal surface.** Both kibi and Ion assume a Unix-style terminal: raw mode, ANSI
escapes, window size. LogansOS has a window manager, not a tty. So the lane is:

1. **L1a, a terminal app on the WM:** a window that renders an ANSI/VT text grid (`neo-text` for glyphs) and gives
   its client a byte stream with raw/cooked mode and a size query, through a small std-visible interface (a
   `std::os::neoplutarch::term` extension, or termion's backend implemented on it). Byte oracle for the escape-sequence
   subset we support. Kill-switch: the terminal app is a separate client; the WM is unchanged without it.
2. **L1b, kibi** on that terminal, vendored under `third_party/kibi` (license files kept, upstream SHA recorded).
   First real editor. Check the toolchain against its Rust 1.99 / edition 2024 requirement first.
3. **L1c, a shell:** decide between porting Ion (git deps pinned and vendored, termion on L1a) and growing
   `neo-shell`, with a short spike on each. Ion gets us scripting, pipelines and history; neo-shell keeps the
   capability model first-class. The decision goes to J.c. with the spike results.
4. **L1d, coreutils:** individual uutils crates (`ls`, `cat`, `cp`, `mv`, `mkdir`, `rm`, `echo`, `wc`, `head`,
   `tail` first), each gated on our std PAL supporting what it calls. Unsupported calls are listed, not stubbed
   silently.

Dependency chain: L1a → L1b; L1a → L1c; L1c → L1d (the tools need somewhere to run). L1a is the gating build.
Cross-family review on L1a (it adds a new client protocol) and a security read of the terminal's input path
(nothing may reach the terminal's client while the lock screen is up, the CP-97 lesson).

## 4. Lane 2: codecs (compression and images)

Small, `no_std`, low risk, and several features want them: gzip over HTTP for the fetch broker, PNG and JPEG for the
browser and for app icons and wallpapers, archives later.

1. **L2a, miniz_oxide** vendored, with host KATs: inflate known gzip and zlib streams fetched raw (the byte-oracle
   rule: test vectors by `git`/`curl`, never a summarizing fetch), plus a round trip.
2. **L2b, image decoding:** `png` (image-png) and `zune-jpeg`, vendored, behind one small `libneo`-level decode API
   (bytes in, RGBA plus size out), with host KATs on a few raw-fetched images (hash of the decoded pixels).
3. **Guardrails:** a decode size cap (width × height × 4 bounded) so a hostile image can't exhaust memory; fuzz each
   decoder on the host for a fixed budget in CI.

No kernel change. Host-only gate plus the CI build; this lane suits Prometheus (host-only builds) or a main-tier seat
between kernel windows.

## 5. Lane 3: FAT in the OS

`rust-fatfs` is already vendored (`third_party/fatfs` at `38b3a795`) but only the host image builder uses it. Using
it in the OS lets LogansOS read and write ordinary USB sticks and SD cards.

1. **L3a:** mount a FAT volume from a block device through the VFS mount table, read-only first. Mount needs the
   mount authority (#240); the volume is never mounted at boot by default.
2. **L3b:** read-write, behind a feature, through the write guard (`GuardedDevice`, #85/#88), so the Zenbook's
   internal Windows disk can never be written. Scenario: create, write, read back and remount on a FAT image.
3. **Kill-switch:** the feature off means no FAT code in the kernel.

Prior-art trap mining before L3a: FAT long-name edge cases, FAT12/16/32 detection, dirty-bit handling, and what the
Linux `vfat` driver warns about (read for principle only, never copied).

## 6. Order and sizing

Three independent lanes, so all three can start when seats are free:
- **Lane 2** first or in parallel: it's host-only, so it doesn't compete for QEMU gate slots.
- **Lane 1** is the most visible and the deepest: L1a is a real design and build (new client protocol, gate
  scenarios, cross-family review); L1b follows quickly; L1c starts with two spikes and a decision; L1d is many small
  builds.
- **Lane 3** is one to two gated builds on code we already vendor.

Seats: Lane 1 to a main-tier seat (the WM experience suggests seat B, who built the permission prompt and lock
paths); Lane 2 to Prometheus or Dexter; Lane 3 to seat C (NeoFS and the block layer).

## 7. Decisions for J.c.

1. Shell: port Ion, or grow `neo-shell`? Proposed: spike both in L1c, then decide.
2. Is a terminal window the intended direction for the desktop's command line, or should the shell live inside
   Arthur's surface? This THINK assumes a separate terminal app.
