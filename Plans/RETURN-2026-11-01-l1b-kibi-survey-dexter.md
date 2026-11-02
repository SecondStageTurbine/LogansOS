<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 240, L1b survey: kibi on LogansOS

**Seat:** Dexter. **Answers:** `Plans/DISPATCH-2026-11-01-l1b-kibi-survey-dexter.md`, section "Relay 240".
**Branch:** `dexter/l1b-kibi-survey`. **Base:** `origin/main` @ `c97bb856` (ff-clean at close).
**Head (the survey artifact; this RETURN's parent):** `4945b5e054323890548e40a3a322d050d4a31f41`.
**Kernel-touching:** no — this relay changes no code at all. The deliverable is one THINK.

## Delivered

| Item | Path | Notes |
|---|---|---|
| Survey | `docs/THINK-2026-11-01-l1b-kibi-on-logansos.md` | Answers the dispatch's five points: the upstream re-check and toolchain comparison, the terminal call/escape map against PR #12 with the gaps, the file/env map against `neo-std-pal` with `file:line`, the exact target-build errors, and the recommended std-only `neoplutarch` backend, build order and `kibi_open_edit_save` gate |

No vendored crate was committed. The kibi clone and the scratch target build live under `/tmp` only;
the scratch edits are on a local `l1b-scratch` branch that was never committed or pushed. Build logs are
`/tmp/buildlogs/kibi-*.log` on this box.

## Upstream record (re-checked on the day)

- Repository `https://github.com/ilai-deutel/kibi`
- Head **`d42f6e34c7d0ea717aacc64cb2f2da4171f7b2b4`**, committed **2026-10-27 21:46:18 +0000**
  (author date 2026-10-28 01:46:18 +0400), subject `Add Ctrl-O to open a file (#651)`
- Licence **MIT OR Apache-2.0** (`Cargo.toml`, `LICENSE-MIT`, `LICENSE-APACHE`, `COPYRIGHT`,
  `REUSE.toml`)
- Version 0.3.3; **`rust-version = "1.99"`** (`Cargo.toml:9`); **edition 2024** (`Cargo.toml:12`);
  CI pins `1.99` and `nightly` (`.github/workflows/ci.yml:32,51`)
- Thirteen `.rs` files (2,784 lines); portable runtime dependency `unicode-width 0.2.2`, `libc` only
  under `cfg(unix)`, `winapi` only under `cfg(windows)`.

Our pinned toolchain is `nightly-2026-04-16`, `rustc 1.97.0-nightly (e8e4541ff 2026-04-15)`. kibi's
1.99 is **two minors ahead**. The 1.99 bump (`ccfcde5`) only adds the `unnecessary_rest_pattern` clippy
lint; it is not a language feature. With `--ignore-rust-version` the source compiles on 1.97 (proved by
runs D/E below).

## The answer in brief

1. **kibi is a clean vendor candidate.** Its per-OS `sys` module is the seam; a `neoplutarch` arm is a
   small std-only file, not a libc port. Vendor at `d42f6e3` under `third_party/kibi`, keep the licence
   files and `syntax.d`, drop `build.rs` (its `git describe` would read LogansOS's tags), and patch
   `rust-version` to 1.97 with the patch recorded in `VENDORED.md`.
2. **The terminal layer maps onto PR #12, but #12 is missing the input primitive kibi needs.** Raw mode
   is the term page's `cooked`/`echo` bits; size is the page's `columns`/`rows`; DSR 6n is the
   answerback. **Missing:** a child read/wait with a deadline (kibi's `VMIN=0/VTIME=1` 100 ms timeout),
   a timeout-versus-EOF contract (a closed terminal otherwise makes kibi spin), a mode getter or an
   explicit restore contract, a size-change wake or a written poll contract, `HandleKind::{Pipe,TermPage}`,
   `IsTerminal` plumbing, full C0/Ctrl-arrow key coverage, and oracle rows for `ESC[m`, `ESC[999C`/
   `ESC[999B` and `?1049` round trips. DECCKM must stay reset: kibi cannot parse `ESC O A-D`.
3. **The file layer is blocked by shared PAL work, not by the editor.** `std::env::args` is empty
   (`apply-overlay.py:346`; std's `args/unsupported.rs`), so a file argument is ignored. `File::open`
   works through a `RootDirCap` (`fs_neoplutarch.rs:237-264`), but the unconditional
   `seek(SeekFrom::End(0))` (`editor.rs:481`) is `Unsupported` (`fs_neoplutarch.rs:351-353`), so every
   existing file half-loads (gutters, no text, status `2B`/`Can't open`). `File::create`
   (`editor.rs:509`) is refused for `truncate` (`fs_neoplutarch.rs:248`); if truncate were allowed,
   `SYS_OPEN(OPEN_CREATE_REGULAR)` is create-new-only on NeoFS (`neofs/inode.rs:356-358`), so
   save-over-existing still fails. These are L1d **P0/P1/P2/P8**, not kibi bugs. Config/syntax can be
   avoided in v1 with empty `conf_dirs`/`data_dirs`; otherwise `stat`/`read_dir` are missing and
   `eprintln!` warnings land in the terminal's screen parser because stderr is the same pipe as stdout
   (`stdio_neoplutarch.rs:25-26`).
4. **The target build fails exactly as recorded, and a scratch std-only backend proves the port.**
   As-is: the MSRV gate. With `--ignore-rust-version`: `E0583` no `src/sys.rs` at `src/lib.rs:22`.
   A `neoplutarch` arm copied from `unix.rs` plus `libc`: five `E0432`s because `libc` is empty on this
   target (`termios`, `ioctl`, `sigaction`, `winsize`, `VMIN`, `VTIME`). A std-only backend builds;
   a `_start` + `SyscallHeap` shim gets `e_entry = 0x21aef0` inside the `R E` segment instead of `0`.
5. **Recommended shape and order.** kibi stays std-only: `src/neoplutarch.rs` on
   `std::os::neoplutarch::term::{size, set_mode, mode, stdin}` and plain `std::fs`; no libc, no signals,
   no raw syscalls. Order: B0 vendor + host-shim backend (can start now), B1 std term extension over
   L1a.1, B2 shared L1d P0/P1/P2/P8, B3 target build, B4 launch/read via L1a.3 + a `RootDirCap`, B5
   save, B6 the `kibi_open_edit_save` gate. The lane is roughly 10–16 cycles after L1a.1; the terminal
   app is the long pole. The gate types `X`, arrows, `End`/`Home`, `Ctrl+Right`, `Home`/`Delete`/`X`,
   `Ctrl+S`, a resize, lone `Esc` + `E`, `Ctrl+F`/`hello`, `Ctrl+E` (negative) and `Ctrl+Q`, and checks
   the whole 80 × 25 grid plus the saved bytes `Xhello\nworld\n` exactly, with separate legs for EOF,
   the lock screen, `--version` and no-argv.

## Build results (target `x86_64-unknown-neoplutarch`, pinned `nightly-2026-04-16`)

Scratch clone `/tmp/kibi`, build tree `/tmp/kibi-l1b`, target JSON copied from
`userland/x86_64-unknown-neoplutarch.json`, std built with `-Z build-std=std,panic_abort` from
`/tmp/neo-rust-src/library` (the L1d overlay-applied copy; the overlay is unchanged between
`e9330a22` and `c97bb856`), `RUSTC_BOOTSTRAP_SYNTHETIC_TARGET=1`:

| Run | Tree | Result | First error |
|---|---|---|---|
| A | kibi as-is | **stopped** | `error: rustc 1.97.0-nightly is not supported by the following packages: kibi@0.3.3 requires rustc 1.99` |
| B | A + `--ignore-rust-version` | **stopped** | `error[E0583]: file not found for module 'sys'` at `src/lib.rs:22` |
| C | B + `neoplutarch` path arm, `src/neoplutarch.rs` copied from `unix.rs`, `libc` added | **stopped** | five `E0432` unresolved imports |
| D | B + a minimal std-only `neoplutarch.rs` (no libc) | **built** | — (251 KiB, `e_entry 0`) |
| E | D + `#![no_main]`, `#[global_allocator] SyscallHeap`, `_start` | **built** | — (1.4 MiB, `e_entry 0x21aef0`, inside the `R E` segment `[0x21a530, 0x2876a4)`) |

Representative exact errors:

```
error[E0583]: file not found for module `sys`
  --> src/lib.rs:22:1
   |
22 | mod sys;
   | ^^^^^^^^
   = help: to create the module `sys`, create file "src/sys.rs" or "src/sys/mod.rs"
```

```
error[E0432]: unresolved import `libc::termios`
  --> src/neoplutarch.rs:15:9
15 | pub use libc::termios as TermMode;
   |         ^^^^^^^^^^^^^^ no `termios` in the root

error[E0432]: unresolved imports `libc::SA_SIGINFO`, `libc::STDIN_FILENO`, `libc::STDOUT_FILENO`,
              `libc::TCSADRAIN`, `libc::TIOCGWINSZ`, `libc::VMIN`, `libc::VTIME`
  --> src/neoplutarch.rs:16:12

error[E0432]: unresolved imports `libc::c_int`, `libc::sigaction`, `libc::sighandler_t`,
              `libc::siginfo_t`, `libc::winsize`
  --> src/neoplutarch.rs:17:12

error[E0432]: unresolved import `crate::xdg`
  --> src/neoplutarch.rs:20:16

error[E0432]: unresolved import `crate::sys::conf_dirs`
  --> src/config.rs:12:5
```

`libc 0.2.189` (kibi's `Cargo.lock`) compiles for the target but exposes no unix surface there, so it
is a false friend; the backend must be std-only.

## Verification

| Check | Result |
|---|---|
| `git fetch origin` + branch base | base `c97bb856` == `origin/main` tip at start — ff-clean |
| Diff vs `origin/main` | two files: the THINK and this RETURN; no code |
| Code changes | none — no `kernel/`, `tools/`, `userland/`, `Cargo.*` touched |
| SPDX | THINK and RETURN both start with `<!-- SPDX-License-Identifier: CC-BY-4.0 -->` |
| Build attempt | five `cargo build -Zjson-target-spec` runs; logs at `/tmp/buildlogs/kibi-*.log`; the scratch clone and branch are not committed |
| `worker-close-gate.sh` | not run: there is no code to fmt/test/clippy; Step 1's rebase check was done manually (above). CI is path-ignored for `docs/**` and `Plans/**`. |

```
Branch:        dexter/l1b-kibi-survey @ 4945b5e0 (THINK)
Base:          origin/main @ c97bb856  (ff-clean)
Kernel-touching: no
Push CI:       not applicable (docs/Plans only; path-ignored)
Self-stat:     THINK commit 1 file, 801 insertions(+); this RETURN adds 1 file
```

## Notes

1. **No CI URL.** The branch touches only `docs/` and `Plans/`, which `ci.yml` path-ignores; there is no
   run to cite, and no green-CI claim is made.
2. **RAG.** Not available on this box, as the dispatch says; used `grep`, direct reads and the upstream
   clone.
3. **No personal details.** The THINK and this RETURN use repo-relative and `/tmp` paths only; no
   hostnames, addresses or emails.
4. **Disk reclaim.** After the PM merges: `cargo clean` in `~/workers/dexter` if desired; do not delete
   the worktree. The `/tmp/kibi`, `/tmp/kibi-l1b` and `/tmp/neo-rust-src` trees are disposable.
5. **The L1b build is not a "vendor and run" job.** It shares L1d's argv/entry/stdin/file packages and
   L1a's pipe/term page/terminal app. The first host-only step (B0) can start now; the first runnable
   editor waits on that chain.
