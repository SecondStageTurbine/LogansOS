<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: L1b — kibi on LogansOS

**Date:** 2026-11-01 · **Author:** Dexter (Relay 240) · **Asked by:** the PM, from
`Plans/DISPATCH-2026-11-01-l1b-kibi-survey-dexter.md`, section "Relay 240".
**Status:** survey only; this relay changes no code. It vendors nothing and commits only this THINK and
its RETURN.
**Citations.** Our tree is `origin/main` @ `c97bb856` (the Relay-240 dispatch commit; its only change
since `e9330a22` is `Plans/`, so every code line below is the code at `e9330a22` too). The terminal
design is PR #12 @ `ae7a9985` (`docs/THINK-2026-10-31-l1a-terminal-window.md`, revision 2). The std-PAL
method is my L1d survey, PR #38 @ `0da7a9f3` (THINK `a18955f1`). Upstream is
`ilai-deutel/kibi` @ `d42f6e34c7d0ea717aacc64cb2f2da4171f7b2b4`, cloned and read on 2026-11-01.

**Bottom line.**

1. **kibi is a clean vendor candidate.** MIT OR Apache-2.0, thirteen source files, one portable runtime
   dependency (`unicode-width 0.2.2`; `libc` only under `cfg(unix)`, `winapi` only under `cfg(windows)`), no
   `unsafe` outside its `unix.rs`/`windows.rs` backends, and a
   per-OS `sys` module that is exactly the seam a `neoplutarch` backend needs.
2. **Its declared MSRV is two minors ahead of ours, but the bump is a lint, not a language feature.**
   kibi says `rust-version = "1.99"`; our pinned toolchain is `nightly-2026-04-16` (`rustc
   1.97.0-nightly`). Cargo refuses the build. The 1.99 bump (commit `ccfcde5`) only adds the
   `unnecessary_rest_pattern` clippy lint; with `--ignore-rust-version` the source compiles on 1.97.
   Vendor with an explicit `rust-version` patch, or bump the OS toolchain — not both.
3. **It cannot build for our target unmodified.** `src/lib.rs:22` declares `mod sys` with `path`
   attributes for `windows`/`unix`/`wasi` only; on `target_os = "neoplutarch"` the module has no file.
   Adding a `neoplutarch` arm and copying `unix.rs` fails because `libc` is empty on this target (no
   `termios`, `ioctl`, `sigaction`, `winsize`, `VMIN`, `VTIME`). A std-only backend compiles; I built it,
   and with a `_start` shim plus `SyscallHeap` the ELF gets a real `e_entry` (`0x21aef0`) instead of `0`.
4. **The terminal layer maps onto #12, but #12 is missing the input primitive kibi actually needs.**
   Raw mode and size are the term page; DSR 6n is the answerback. But kibi's `VMIN=0/VTIME=1` 100 ms
   read timeout is not in #12's proposed `size`/`set_mode`/`is_terminal` API, and without it kibi cannot
   tell a lone Esc, cannot poll for a resize, and cannot distinguish a closed terminal from an idle one.
   #12's prose names `SYS_WAIT_ANY` as the mechanism; L1a.1/L1a.2 must expose it as a concrete call and
   define timeout-versus-EOF.
5. **The file layer needs L1d's shared PAL work, not a kibi fork.** At head, kibi cannot open an
   existing file (the unconditional `seek(End(0))` is `Unsupported`) and cannot save any file
   (`File::create` is refused for `truncate`; if truncate were allowed, `SYS_OPEN(OPEN_CREATE_REGULAR)`
   is create-new-only, so save-over-existing still fails). Its argv is empty, so it opens no file at
   all when launched with one. These are L1d P0/P1/P2/P8, not editor bugs.
6. **Recommended shape:** kibi stays a std-only program; a new `src/neoplutarch.rs` implements the `sys`
   seam on the std term extension and plain `std::fs`; no libc, no signals, no raw syscalls in kibi.
   Vendor under `third_party/kibi` at `d42f6e3`. The first runnable editor waits on L1a.1 → L1a.3 plus
   L1d P0/P1/P2/P8. That is a real dependency chain, not a small port, but it is the same chain L1c and
   L1d already need.

## 1. Re-check on the day

Read from a fresh clone on 2026-11-01 (`git log -1`, `Cargo.toml`, the licence files, the CI matrix):

| Item | Value |
|---|---|
| Repository | `https://github.com/ilai-deutel/kibi` |
| Head | `d42f6e34c7d0ea717aacc64cb2f2da4171f7b2b4` |
| Head date | `2026-10-27 21:46:18 +0000` (author date `2026-10-28 01:46:18 +0400`) |
| Head subject | `Add Ctrl-O to open a file (#651)` |
| Version | `0.3.3` (`Cargo.toml`; `CHANGELOG.md` dates 0.3.3 to 2026-01-01) |
| Licence | **MIT OR Apache-2.0** — `Cargo.toml` `license`, `LICENSE-MIT`, `LICENSE-APACHE`, `COPYRIGHT`, `REUSE.toml` (`MIT or Apache-2.0`) |
| MSRV | **`rust-version = "1.99"`** (`Cargo.toml:9`); CI matrix pins `1.99` and `nightly` (`.github/workflows/ci.yml:32,51`) |
| Edition | **2024** (`Cargo.toml:12`) |
| Source size | 13 `.rs` files under `src/` (2,784 lines); `src/editor.rs` is 1,619 of them |
| Runtime deps | `unicode-width 0.2.2`; `libc 0.2.186` only under `cfg(unix)`; `winapi`/`winapi-util` only under `cfg(windows)` |
| Dev deps | `rstest`, `tempfile` (not needed for the target build) |
| Tests | `tests/cli.rs` (host CLI, needs a terminal); unit tests inside `src/editor.rs`, `src/config.rs`, `src/row.rs`, `src/syntax.rs` |
| Archived? | Not checked (no API from this seat). The lane re-checks it when the build starts, as the survey rule says. |

The 2026-10-31 survey row recorded the same head (`d42f6e3`) and the same licence. Nothing moved
upstream in the day between that survey and this relay.

Two repository details matter to a vendored copy:

- **`build.rs` runs `git describe` on the host** (`build.rs:7-12`, `Command::new("git").args(["describe",
  "--tags", "--match=v*"])`). A vendored `third_party/kibi` sits inside the LogansOS git repository, so
  `git describe` would describe **our** tags, not kibi's. Drop `build.rs` from the vendored copy and let
  `CARGO_PKG_VERSION` be `0.3.3` (upstream already excludes `build.rs` from its published crates, per
  the 0.3.3 changelog).
- **`syntax.d/` is 52 small INI files (212 KB).** Vendor it, or the editor starts with no syntax
  definitions. `assets/` (2 MB of screenshots and casts), `tests/`, `fuzz/`, `xtask/` and `scripts/` are
  not needed.

## 2. Our toolchain against its MSRV

`rust-toolchain.toml` pins `channel = "nightly-2026-04-16"`. The pinned compiler is:

```
rustc 1.97.0-nightly (e8e4541ff 2026-04-15)
cargo 1.97.0-nightly (eb94155a9 2026-04-09)
```

kibi declares `rust-version = "1.99"`, so a plain build stops before compiling anything:

```
error: rustc 1.97.0-nightly is not supported by the following packages:
  kibi@0.3.3 requires rustc 1.99
  kibi@0.3.3 requires rustc 1.99
  kibi@0.3.3 requires rustc 1.99
  kibi@0.3.3 requires rustc 1.99
```

The 1.99 bump is commit `ccfcde57ed8a0b04fdc1d9ace4d4e8595bb6d50f` ("Bump MSRV to 1.99",
2026-10-27). Its whole `Cargo.toml` change is `rust-version = "1.98"` → `"1.99"` plus one new clippy
lint, `unnecessary_rest_pattern = "deny"`; the CI matrix moves with it. It adds no language feature to
the source. With `--ignore-rust-version`, the same 1.97 compiler compiles `src/editor.rs`, `row.rs`,
`syntax.rs`, `config.rs` and the scratch backend (build evidence in §5). Edition 2024 has been stable
since 1.85, so it is not the barrier either.

**Recommendation: patch the vendored manifest, not the OS toolchain.** Set
`rust-version = "1.97"` in `third_party/kibi/Cargo.toml`, keep upstream's value verbatim in
`Cargo.toml.upstream`, and record the patch in `VENDORED.md`. A toolchain bump for one editor would
re-open every std-overlay anchor and the 4-shard gate for no language gain. If the PM will not carry a
manifest patch, the build wrapper can pass `--ignore-rust-version`; that is a global, silent bypass and
is worse than the explicit patch.

The vendored copy is excluded from the root workspace, like `third_party/miniz_oxide` and
`third_party/fatfs`, so the repo's fmt/clippy settings do not apply to upstream's style and the
workspace's `cargo test` does not build it for the host.

## 3. The terminal layer, call by call

All kibi terminal code is in `src/unix.rs`, `src/terminal.rs` and the `run`/`loop_until_keypress`
functions in `src/editor.rs`. The WASI backend (`src/wasi.rs`) is the closest existing model for a
"this platform has no termios" file, but it is not a starting point: it opens `/dev/tty`, which we do
not have.

### 3.1 Raw mode

| kibi call | Site | What it does | #12 mapping |
|---|---|---|---|
| `tcgetattr(STDIN_FILENO, &mut termios)` | `unix.rs:77` | snapshot the mode | **No getter.** #12 proposes `size()`, `set_mode(Mode)`, `is_terminal()`; not `mode()`. The backend must either assume the spawn mode is cooked+echo or #12 must add a getter. |
| `cfmakeraw(&mut term)` | `unix.rs:80` | clear `ICANON`, `ECHO`, `ISIG`, `IEXTEN`, `OPOST`, `ICRNL`, … | `set_mode(Mode { cooked: false, echo: false })`. Raw in #12 already means keys go straight to the pipe and Ctrl+C is `0x03`, so `ISIG` has no analogue. Output processing has no analogue, but kibi emits `\r\n` itself (`editor.rs:582`), so it does not need `OPOST`. |
| `term.c_cc[VMIN] = 0; term.c_cc[VTIME] = 1` | `unix.rs:83` | a 100 ms read timeout: return 0 bytes when no key arrived | **#12 has no child-facing timeout.** This is the one hard gap. The prose in #12 §3.3 names `SYS_WAIT_ANY` on the pipe item with a deadline as the mechanism, but the proposed API is only `size`/`set_mode`/`is_terminal`. Without a call, kibi's `BufRead` blocks forever on a lone Esc and never wakes to poll the size. |
| `tcsetattr(STDIN_FILENO, TCSADRAIN, &term)` | `unix.rs:69,84` | apply the mode | `set_mode` via the term page's `mode_req`/`mode_req_seq`/`mode_ack_seq`; the ack wait replaces `TCSADRAIN` (stdin has no output to drain). The backend must wait for the ack before returning from `enable_raw_mode`, as #12 §3.2 specifies (1 s deadline, then `TimedOut`). |
| `set_term_mode(&orig_term)` | `terminal.rs:43` (through `unix.rs:68-70`) | restore the saved mode | `set_mode(Mode { cooked: true, echo: true })` if no getter; `?1049l` after it. This is correct only if the terminal always spawns children in cooked+echo, which #12 §4 makes the default. A `mode()` getter removes the assumption. |

kibi reads no other termios field. A std-only `TermMode` of two booleans (`cooked`, `echo`) is a
complete replacement; there is no need to port `struct termios`, `TCSADRAIN`, `VMIN`/`VTIME` or the
`libc` crate at all.

### 3.2 Window size

| kibi call | Site | What it does | #12 mapping |
|---|---|---|---|
| `ioctl(STDOUT_FILENO, TIOCGWINSZ, &winsize)` | `unix.rs:31-37` | rows and columns from the tty; rejects `0×0` | The term page at offset 12/14 (`columns`, `rows`) and `term::size()`. #12 already has the page and both pixel fields; kibi ignores the pixels. |
| fallback `print!("{REPOSITION_CURSOR_END}{DEVICE_STATUS_REPORT}")` then `read_exact(b"\x1b[")`, `read_until(b';')`, `read_until(b'R')` | `terminal.rs:18-38` | move to the bottom-right with `ESC[999C ESC[999B`, send DSR `ESC[6n`, parse `ESC[row;colR` | #12 supports CUF/CUD, DSR 6n, and the `CSI row;col R` answerback (§5.4). **But `read_exact` has no deadline**, and #12 says answerbacks are written non-blocking and dropped if the stdin pipe is full. A dropped answerback with no keys queued blocks `read_exact` forever. The fallback needs the same read timeout as §3.1, or `size()` must be guaranteed and this path removed. |

`update_window_size` (`editor.rs:279-288`) uses the ioctl first and the DSR path only on error. With a
term page, the ioctl path is `term::size()` and the DSR path should be unreachable; the port should
still make it safe.

### 3.3 SIGWINCH

| kibi call | Site | What it does | #12 mapping |
|---|---|---|---|
| `sigemptyset(&sa_mask)` | `unix.rs:53` | empty the mask | No signals on LogansOS. Drop it. |
| `sa_flags = SA_SIGINFO`; `sa_sigaction = handle_wsize`; `sigaction(SIGWINCH, …)` | `unix.rs:44-58` | set a global `AtomicBool` (`WSC`) when the window changes | **#12 has no signal and no size-change wake.** The term page has `size_seq`, but the terminal does not `FUTEX_WAKE` on a configure, and the child-facing API does not expose the seq. |
| `has_window_size_changed()` → `WSC.swap(false)` | `unix.rs:41,65`; called at `editor.rs:235` | poll the flag at the top of each key loop | The backend can poll `term::size()` against a cached `(rows, cols)` and return whether it changed. That works **only because** §3.1's 100 ms read returns to the loop; with a blocking read, a resize is not noticed until the next key. |
| `update_window_size()` + `refresh_screen()` on change | `editor.rs:235-237` | resize and repaint | Same, once the poll returns true. |

There are two acceptable designs:

1. **Poll (recommended for L1b).** `has_window_size_changed` compares the current `term::size()` to a
   cached value. It needs no kernel work and no wake. The L1b gate resizes the window and waits up to
   one 100 ms read timeout plus one frame.
2. **Wake (optional later).** Add a `FUTEX_WAKE` on the term page's `size_seq` when the terminal
   configures its window, and let the child `FUTEX_WAIT` on it. This is a new term-page ordering row,
   not needed for the first editor.

### 3.4 The keys kibi expects

`loop_until_keypress` (`editor.rs:230-273`) parses bytes itself. It accepts:

| Key | Bytes it accepts | #12's encoder emits | Verdict |
|---|---|---|---|
| printable ASCII / UTF-8 bytes | the bytes themselves | the bytes | ok |
| Enter | `0d` or `0a` (`editor.rs:658`) | `0d` | ok |
| Tab | `09` (inserted; `Row::update` expands it) | `09` | ok |
| Backspace | `7f` or `08` (`editor.rs:28,659`) | `7f` | ok |
| Esc | `1b`, or a truncated sequence | `1b` | ok **only with the 100 ms timeout** |
| Ctrl+Q/S/F/G/O/X/C/V/D/E/R/L/H | `11 13 06 07 0f 18 03 16 04 05 12 0c 08` (`editor.rs:14-27`) | C0 byte = letter & `1f` | ok; the oracle lists only Ctrl+A/C/D/U, so add every C0 byte kibi binds |
| Ctrl+/ | `1f` (`editor.rs:27`) | `1f` | add a row |
| Up/Down/Right/Left | `ESC [ A/B/C/D` | `ESC [ A/B/C/D` in normal mode; `ESC O A/B/C/D` under DECCKM | **ok only if DECCKM is reset.** kibi has no `ESC O A-D` arm: `(b'O', Some(b'A'))` falls to `_ => Key::Escape` (`editor.rs:244-270`). kibi never sends `CSI ? 1 h`, so #12 must keep DECCKM off by default and reset it per child. |
| Home/End | `ESC[H`, `ESC OH`, `ESC[F`, `ESC OF`, `ESC[1~`, `ESC[7~`, `ESC[4~`, `ESC[8~` | `ESC[H` / `ESC OH`, `ESC[F` / `ESC OF` | ok; the extra forms are accepted, not required |
| Delete | `ESC[3~` | `ESC[3~` | ok |
| PageUp/PageDown | `ESC[5~`, `ESC[6~` | `ESC[5~`, `ESC[6~` | ok |
| Ctrl+Right | `ESC[1;5C`, `ESC[5C`, `ESC O c` (lowercase) | `ESC[1;5C` (the table's only Ctrl-arrow row) | `ESC[1;5C` works; the other two are **not in #12's table** |
| Ctrl+Left/Up/Down | `ESC[1;5D`, `ESC[5D`, etc. | **not listed** | add them to the encoder/oracle |
| F1-F12, Insert, shifted arrows | mostly fall to `Key::Escape` (the parser consumes the sequence) | #12 emits them | harmless for kibi, but the terminal's encoder table should not claim kibi supports them |

The important negative is **DECCKM**: #12 lists it as a supported private mode, but kibi never asks
for it and cannot parse the SS3 arrows it produces. A terminal that enables DECCKM globally, or leaves
it enabled after another child, breaks every arrow key in kibi. The L1b gate must assert DECCKM is
reset.

### 3.5 The sequences kibi emits

From `src/ansi_escape.rs`, `src/syntax.rs:31`, `src/row.rs` and the draw functions:

| Sequence | Where | #12 status |
|---|---|---|
| `ESC[?1049h` / `ESC[?1049l` | `ansi_escape.rs:8,11`; `editor.rs:771`; `terminal.rs:43` | supported (alternate screen, save/restore cursor) |
| `ESC[H` | `ansi_escape.rs:20`; `editor.rs:619` | supported (CUP home) |
| `ESC[?25l` / `ESC[?25h` | `ansi_escape.rs:23,25`; `editor.rs:619,633` | supported |
| `ESC[K` | `ansi_escape.rs:28`; `draw_rows`, `draw_message_bar` | supported (EL 0) |
| `ESC[6n` | `ansi_escape.rs:31`; `terminal.rs:19` | supported (DSR 6n, answerback) |
| `ESC[999C ESC[999B` | `ansi_escape.rs:34`; `terminal.rs:19` | supported in principle (CUF/CUD), **not in the oracle**; add a clamp row |
| `ESC[{row};{col}H` | `editor.rs:633` | supported (CUP) |
| `ESC[m` | `ansi_escape.rs:14`; `row.rs:217` | supported as SGR 0, **not in the oracle** (the oracle uses `ESC[0m`) |
| `ESC[7m` | `ansi_escape.rs:17`; status bar | supported |
| `ESC[38;5;240m` | `editor.rs:556` | supported (`38;5;n`) |
| `ESC[31m`..`ESC[35m`, `ESC[39m`, `ESC[46m` | `syntax.rs:18-25` | supported (30-37, 39, 46) |
| `ESC[132m`, `ESC[134m` | `syntax.rs:21,23`; `Display` does `% 100` | resolve to `32m`/`34m`, supported |
| `\r\n`, plain text, `│` (U+2502) | `draw_rows`, `draw_left_padding` | supported (C0 and UTF-8) |

kibi emits **no** OSC, no title, no clipboard, no bracketed-paste markers, no `?7h`, no `?1h`. It is a
well-behaved consumer of the subset #12 promises.

### 3.6 What #12 is missing for kibi, in priority order

1. **A child read/wait with a deadline.** kibi's `VMIN=0/VTIME=1` is load-bearing three times: lone
   Esc, resize polling, and not hanging when the terminal is idle. #12 §3.3 names `SYS_WAIT_ANY` but
   the API list does not expose it. Add something like
   `term::wait_readable(timeout) -> io::Result<Readiness { Readable, TimedOut, Closed }>` (or
   `term::read(&mut buf, timeout)`), map it to the stdin pipe item + deadline in L1a.1, and define the
   return values. The backend can then build a `BufRead` whose `fill_buf` returns an empty slice on
   `TimedOut` and `Err(BrokenPipe)` on `Closed`.
2. **A timeout-versus-EOF contract.** This is not a detail. kibi's loop treats an empty read as "no
   second byte, so the Esc was alone" and loops. If EOF is also an empty read, a closed terminal makes
   kibi spin at 100% forever (`editor.rs:230-273`: `if let Some(a) = bytes.next()?` and no `else`).
   The terminal's pipe EOF must surface as an error the loop propagates (kibi then exits with
   `Error: Io`), or kibi needs a small patch to return from `run` on EOF. #12 §3.4 says the child reads
   EOF after the last writer closes; it does not say how `std::io::Read` distinguishes that from the
   new timeout. Name it.
3. **A mode getter or an explicit restore contract.** `enable_raw_mode` returns `TermMode`; `run`
   restores it on exit and on panic. #12 proposes `set_mode` but not `mode()`. Either add `mode()` or
   guarantee that every terminal child starts cooked+echo and say that the restore target is that
   constant.
4. **A size-change wake or an explicit poll contract.** The term page's `size_seq` is there, but the
   terminal does not wake the child. Polling is fine for L1b if (1) exists; write it down so the
   terminal app is not asked for a futex it does not need.
5. **Key-encoder coverage:** all C0 bytes a Ctrl+letter can produce, Ctrl+Left/Up/Down, and the
   shortened `ESC[5C` form. Keep the `ESC[1;5C` form kibi definitely parses, and keep DECCKM off.
6. **A `HandleKind` for the term page and for pipes.** The std `HandleKind` list
   (`userland/neoplutarch-std-overlay/std/os_neoplutarch.rs:68-111`) has `Buffer`, `Channel`, `File`,
   … but no `Pipe` and no `TermPage`. `std::os::neoplutarch::term::size()` needs to find the 4 KiB term
   page; a scan for `HandleKind::Buffer` is ambiguous (pixel buffers are also `Buffer`). Add
   `HandleKind::Pipe` and `HandleKind::TermPage`, or define a fixed spawn slot.
7. **`IsTerminal` plumbing.** #12 says `std::io::IsTerminal` answers from `is_terminal`, but our
   overlay has no `sys/io/is_terminal/neoplutarch.rs`; std's fallback returns `false` for everything
   (rust-src `library/std/src/sys/io/is_terminal/unsupported.rs`). kibi does not call `is_terminal`, but the
   backend should use it to fail loudly instead of falling into the DSR path.
8. **Oracle rows for kibi's exact output:** `ESC[m` (no parameter), `ESC[999C`/`ESC[999B` clamped at
   the edge, `ESC[?1049h` … `ESC[?1049l` round trip with the cursor, `ESC[?25l`/`h`, and
   `ESC[38;5;240m`. #12's grid oracle has CUP clamping and 1049, but not the 999C/999B form or `ESC[m`.

One more, not a protocol gap but a port constraint: **#12 routes stderr to the same pipe as stdout**
("stderr is the same pipe", §3). kibi's `Config::load` and `SyntaxConf::find` print warnings with
`eprintln!` (`config.rs:87,90`, `syntax.rs:68,72`) when a config or syntax file is malformed. Those bytes
would go into the terminal's **screen parser**, not a log, and could paint over the grid. The port must
either return empty `conf_dirs`/`data_dirs` for v1 (no config or syntax files to warn about), route
kibi's warnings to the status bar, or give the terminal a separate stderr log channel. Do not rely on
"the next repaint will cover it."

## 4. File and environment calls against the PAL

The state names are: **implemented** (a real syscall path), **stub** (a deterministic
`Unsupported`/empty answer or a loud `neo-std-pal` exit), **missing** (no path at all). The std overlay
is the path that actually serves kibi; `neo-std-pal`'s generic stubs are below it and are only reached
where the overlay does not route. This is the same split I described in L1d §2.

| kibi use | kibi site | Our path | State | What kibi does with it |
|---|---|---|---|---|
| `std::env::args` | `main.rs:17,22` | `apply-overlay.py:346` audits `args/mod.rs` as `mod unsupported`; std's `args/unsupported.rs:6,20-21` returns an **empty** iterator | **missing, silently** | `main` sees `(None, None, 0)` and calls `run(None, …)`: a file argument is ignored and `--version` starts the editor. |
| `std::env::var` (`NO_COLOR`) | `editor.rs:769` | `env_neoplutarch.rs:35-37` `getenv` → `None` | **stub by policy** | `NO_COLOR` is never set, so `use_color` is always true. Harmless; the user cannot turn colour off. |
| `std::env::var` (`XDG_*`, `HOME`) | `xdg.rs:21,27` | same `env_neoplutarch.rs:35-37` | **stub by policy** | `Config` falls back to `/etc/xdg:/etc` and `SyntaxConf` to `/usr/local/share:/usr/share`; see the `read_dir`/`is_file` rows. |
| `io::stdin().lock()` and `BufRead::bytes` | `unix.rs:95`; `editor.rs:231-273` | `stdio_neoplutarch.rs:45-49`: `Stdin::read` returns `Unsupported` | **stub** | The first read exits with `Error: Io(Unsupported)`. This is the whole input path; L1a.1 must route fd 0 to the terminal pipe and L1d P2 must implement `Stdin::read`. |
| stdout `print!`, `flush` | `terminal.rs:20,45`; `editor.rs:634` | `stdio_neoplutarch.rs:58-64` over `neo-std-pal::write_fd` (`lib.rs:97`) | **implemented** | Writes go to the console fd; in the terminal they must go to the terminal's stdout pipe. |
| stderr `eprintln!` | `config.rs:87,90`; `syntax.rs:68,72`; panic hook `editor.rs:775` | `stdio_neoplutarch.rs:73-79`, `STDERR_FD = CONSOLE_FD` (`:25-26`) | **implemented, wrong channel** | These bytes land in the same stream as stdout; see §3.6. |
| `fs::metadata` (`is_file`, `is_symlink`) | `editor.rs:464`; `config.rs:59` | `fs_neoplutarch.rs:446-448` `stat` → `unsupported()` | **missing** | Returns `Err(Unsupported)`, so `is_ok_and` is false: the directory guard is skipped and `Path::is_file` is false. This *avoids* a blocker in `load`, but it also means no config file is ever seen. |
| `fs::File::open` | `editor.rs:467` | `fs_neoplutarch.rs:237-264`; needs `holds_root_dir_cap()` (`:240`, `neoplutarch_rt.rs:279`) | **implemented with a RootDirCap** | Opens read-only, `SYS_OPEN(OPEN_ROOT, FILE_READ)`. Without a cap it is `PermissionDenied` before the syscall. |
| `BufReader::split` / `read` | `editor.rs:474` | `fs_neoplutarch.rs:319-321` over `rt::read` (`neoplutarch_rt.rs:238`) | **implemented** | Reads the file while the cap is held. |
| `file.seek(SeekFrom::End(0))` | `editor.rs:481` | `fs_neoplutarch.rs:351-353` `seek` → `unsupported()`; there is **no `SYS_LSEEK`** in the kernel table (`kernel/src/arch/x86_64/syscall.rs:92-118`) | **missing** | **Every existing file fails to load cleanly.** The rows are pushed, but `?` returns before `update_all_rows`, `n_bytes`, `dirty` and the cursor reset; `open` only sets a status message. `Row::draw` uses the `render`/`hl` buffers that `update_all_rows` fills, so the grid shows the line-number gutters (`1 │`, `2 │`, `3 │`) but no text; the status bar size is `2B` and it says `Can't open …` for a file it could read. That half-initialized buffer is worse than a clean failure. |
| `fs::File::create` | `editor.rs:509` | `fs_neoplutarch.rs:237-264`; `OpenOptions` refuses `truncate` at `:248` | **missing/refused** | `File::create` is `write + create + truncate` (rust-src `library/std/src/fs.rs:638-640`), so it hits `unsupported()` before the syscall. No save at all, even to a new file. |
| (if `truncate` were allowed) `SYS_OPEN(OPEN_CREATE_REGULAR)` | — | `fs_neoplutarch.rs:262`; kernel `fs::open_root_temporary` (`kernel/src/fs/mod.rs:1487-1497`) and NeoFS `create_regular` (`kernel/src/fs/neofs/inode.rs:342-358`) | **create-new only** | `create_regular` returns `InvalidDirectory` when the name exists (`inode.rs:356-358`), mapped to `VfsError::Invalid`/`EINVAL` (`neofs/mount.rs:526-532`). Save-over-existing needs an open-or-create path, not just truncate. |
| `file.write_all` | `editor.rs:512-517` | `fs_neoplutarch.rs:335-337` over `rt::write` (`neoplutarch_rt.rs:102`) | **implemented** | Fine once a write handle exists. |
| `file.sync_all` | `editor.rs:519` | `fs_neoplutarch.rs:287-289` over `rt::fsync` (`neoplutarch_rt.rs:250`) | **implemented** | Fine. |
| `read_dir` (`syntax.d`) | `syntax.rs:61` | `fs_neoplutarch.rs:398-400` → `unsupported()` | **missing** | With the default `data_dirs`, this prints `Error iterating …: unsupported platform` to the screen for each directory, then uses the default syntax. With `data_dirs() == []`, it is never called. |
| `fs::read_to_string` (`config.ini`) | `config.rs:86` | `fs_neoplutarch.rs:237-264` + `:319-321` | **implemented if reached** | It is gated by `Path::is_file` (`config.rs:59`), which is false today, so it is never reached. With `stat` implemented it would read through the cap. |
| `Command::new(...).args(...).output()` (Ctrl-E) | `editor.rs:879` | `apply-overlay.py:348` audits `process/mod.rs` as `mod unsupported`; `neo-std-pal::process_spawn_unsupported` (`lib.rs:357`) is the direct-PAL twin | **missing** | Returns `Err(Unsupported)`, so kibi puts the error in its status bar. It does not crash. There is no pipe-capture spawn and no argv to give the child anyway (L1d P0). |
| `Instant::now`/`elapsed` | `editor.rs:142,610` | `time_neoplutarch.rs:18-24` over `SYS_CLOCK_GETTIME(CLOCK_MONOTONIC)` | **implemented** | Works when the scenario enables `desktop-session`/`browser-runtime`/`atrium-pointer-forward`; `SYS_CLOCK_GETTIME` is feature-gated (`kernel/src/arch/x86_64/syscall.rs:1045-1050`). |
| `std::panic::set_hook` | `editor.rs:774` | std's hook; `panic = "abort"`; hook runs before `abort_internal` (`neoplutarch_rt.rs:131`) | **implemented** | On panic, it restores the mode, prints `?1049l`, then aborts 134. |
| `IsTerminal` | not called | no `sys/io/is_terminal/neoplutarch.rs`; std fallback `unsupported.rs` returns false | **missing** | The backend should use it; kibi itself does not. |
| `env::current_dir` | only `wasi.rs:57` | `env`/`paths` fallback | **n/a for neoplutarch** | Not used by the Unix path. |

What this table means for L1b:

- **The file authority is a spawn-time capability, not ambient.** `File::open` refuses without a
  `RootDirCap` (`fs_neoplutarch.rs:240`). `neo-init` gives the shell one (`neo-init/src/main.rs:137-139`,
  L1d §7.5), but the shell's `spawn_child` passes only a channel (`userland/neo-shell/src/main.rs:1156-1170`).
  kibi cannot open a file until L1d P12 hands it a cap.
- **The first editor needs four shared pieces, not a kibi rewrite:** argv (P0), `_start` + allocator
  (P1), stdin (P2), and the file semantics (P8: `SYS_LSEEK`, open-or-create, `File::truncate`). The
  terminal backend itself is small.
- **The only kibi-specific file work is choosing what to do about config and syntax.** Empty
  `conf_dirs`/`data_dirs` avoids `stat` and `read_dir` entirely for v1; a later rung can add a
  read-only config cap and a `stat` that works.

## 5. The build attempt

### 5.1 Method

The scratch clone and build tree are under `/tmp`; a local `l1b-scratch` branch holds the edits and was never committed or pushed. Nothing was committed to kibi or to this branch.
The target and std are the repo's own, with the L1d overlay-applied rust-src copy:

```
git clone --depth 1 https://github.com/ilai-deutel/kibi /tmp/kibi
cp -a /tmp/kibi /tmp/kibi-l1b
cp userland/x86_64-unknown-neoplutarch.json /tmp/kibi-l1b/
# .cargo/config.toml: target = the JSON; build-std = ["std", "panic_abort"];
# build-std-features = ["compiler-builtins-mem"]; the static-link rustflags
# from userland/std-real-smoke/.cargo/config.toml
RUSTUP_TOOLCHAIN=nightly-2026-04-16 \
__CARGO_TESTS_ONLY_SRC_ROOT=/tmp/neo-rust-src/library \
RUSTC_BOOTSTRAP_SYNTHETIC_TARGET=1 \
cargo build -Zjson-target-spec [--ignore-rust-version]
```

`/tmp/neo-rust-src/library` is the overlaid rust-src copy from the L1d run. The overlay is unchanged
between `e9330a22` and `c97bb856` (the latter touches `Plans/` only), so the provenance is the same as
L1d's. `RUSTC_BOOTSTRAP_SYNTHETIC_TARGET=1` is still required for every non-`core` dependency (the L1d
`restricted_std` finding).

### 5.2 Results

| Run | Tree | Result | First error |
|---|---|---|---|
| A | kibi as-is | **stopped** | `error: rustc 1.97.0-nightly is not supported by the following packages: kibi@0.3.3 requires rustc 1.99` |
| B | A + `--ignore-rust-version` | **stopped** | `error[E0583]: file not found for module 'sys'` at `src/lib.rs:22` |
| C | B + a `neoplutarch` `path` arm, `src/neoplutarch.rs` copied from `unix.rs`, `libc` added for the target | **stopped** | five `E0432` unresolved imports |
| D | B + a minimal std-only `neoplutarch.rs` (no libc) | **built** | — |
| E | D + `#![no_main]`, `#[global_allocator] SyscallHeap`, `_start` | **built** | — |

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

`libc 0.2.189` (the version in kibi's `Cargo.lock`) compiles for the target but is an **empty crate**:
its `unix` module (and therefore `termios`, `ioctl`, `sigaction`, `winsize`) is `cfg(unix)`, and this
target is not unix. That is why run C fails at the imports. `libc` is a false friend here, exactly as
L1d found for `nix`/`rustix`.

Run D's backend is 30 lines: `TermMode` is a `Copy` unit struct, `get_window_size` returns
`Err(InvalidWindowSize)`, the signal handler is a no-op, `has_window_size_changed` is a `false` static,
`set_term_mode`/`enable_raw_mode` are `Ok`, `stdin` is `io::stdin().lock()`, and `conf_dirs`/`data_dirs`
are empty. It proves the editor code itself compiles on 1.97 and that the target build has no other
language or dependency blocker.

### 5.3 The ELF

| Run | Size | `e_entry` | Loadable? |
|---|---|---|---|
| D (normal `fn main`) | 251 KiB | `0x0` | **No** — our loader rejects an entry outside every loaded segment (`kernel/src/loader/elf.rs:253-261`) |
| E (`_start` + `SyscallHeap`) | 1.4 MiB | `0x21aef0` | **Yes in principle** — the entry is inside the `R E` segment `[0x21a530, 0x2876a4)` |

Run E is the important one. It links `neo-std-pal` as a path dependency, installs `SyscallHeap` as the
global allocator, and has a `_start` that calls `kibi::run` and `neo_std_pal::exit`. It emits the A4
manifest gate warning (`neo-std-pal@0.0.1: A4 PAL-manifest gate OK — 15 files, 2 syscall4
trampoline(s), …`), which is expected: `neo-std-pal` runs its own manifest check at build time. This is
the entry shape the L1b build should use.

## 6. Recommended port shape

### 6.1 Vendoring

```
third_party/kibi/
  VENDORED.md              # upstream URL, SHA d42f6e3, date, licence, the exact patches
  Cargo.toml.upstream      # upstream's Cargo.toml, byte for byte
  Cargo.toml               # publish = false; rust-version = "1.97"; no build.rs
  Cargo.lock               # pinned (upstream's lock is 0.3.3's)
  LICENSE-MIT, LICENSE-APACHE, COPYRIGHT, REUSE.toml
  src/                     # upstream files + SPDX line if the close-gate needs it + neoplutarch.rs
  syntax.d/*.ini           # 52 files, 212 KB
  config_example.ini
```

Exclude it from the root workspace (`Cargo.toml` `exclude`), as `third_party/miniz_oxide` is. Keep the
upstream tests out (or under `tests/` but not built for the target). Add `third_party/kibi` to
`VENDORED.md`'s check block: `git -C /tmp/kibi checkout d42f6e3`, then compare each `src/*.rs` after
the SPDX line and after the documented patch.

### 6.2 The `neoplutarch` backend

kibi's module pattern is a `cfg_attr(path = …)` per platform (`src/lib.rs:19-22`). Add one arm:

```rust
#[cfg_attr(windows, path = "windows.rs")]
#[cfg_attr(unix, path = "unix.rs")]
#[cfg_attr(target_os = "wasi", path = "wasi.rs")]
#[cfg_attr(target_os = "neoplutarch", path = "neoplutarch.rs")]
mod sys;

// xdg.rs is unix/wasi only today; neoplutarch implements conf_dirs/data_dirs
// directly because it has no environment to read.
#[cfg(any(unix, target_os = "wasi"))]
mod xdg;
```

`src/neoplutarch.rs` is std-only and needs no `libc`:

```rust
use std::io::{self, BufRead};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use crate::Error;

/// `false` = raw/no echo; `true` = cooked/echo. #12's mode page bits 0 and 1.
#[derive(Copy, Clone)]
pub struct TermMode {
    pub cooked: bool,
    pub echo: bool,
}

/// Pack rows in the high 32 bits, columns in the low 32.
static LAST_SIZE: AtomicU64 = AtomicU64::new(0);

pub fn get_window_size() -> Result<(usize, usize), Error> {
    std::os::neoplutarch::term::size()
        .map(|(cols, rows)| (rows as usize, cols as usize))
        .map_err(|_| Error::InvalidWindowSize)
}

pub fn register_winsize_change_signal_handler() -> io::Result<()> {
    Ok(()) // no signals; has_window_size_changed polls the term page
}

pub fn has_window_size_changed() -> bool {
    let Ok((cols, rows)) = std::os::neoplutarch::term::size() else { return false };
    let now = ((rows as u64) << 32) | cols as u64;
    LAST_SIZE.swap(now, Relaxed) != now
}

pub fn set_term_mode(term: &TermMode) -> io::Result<()> {
    std::os::neoplutarch::term::set_mode(*term)
}

pub fn enable_raw_mode() -> io::Result<TermMode> {
    let orig = std::os::neoplutarch::term::mode()?;
    std::os::neoplutarch::term::set_mode(TermMode { cooked: false, echo: false })?;
    Ok(orig)
}

pub fn stdin() -> io::Result<impl BufRead> {
    // A BufRead over fd 0 whose fill_buf waits at most 100 ms and returns
    // Err(BrokenPipe) when the terminal's stdin pipe is closed. See §3.6.
    std::os::neoplutarch::term::stdin()
}

pub fn conf_dirs() -> Vec<String> { Vec::new() } // v1: no config file
pub fn data_dirs() -> Vec<String> { Vec::new() } // v1: no syntax.d

pub fn path(filename: &str) -> std::path::PathBuf { std::path::PathBuf::from(filename) }
```

The `std::os::neoplutarch::term` module is #12's proposed extension, not a new kibi-private ABI. It
should be `#![unstable(feature = "neoplutarch_ext")]`, like the rest of `std::os::neoplutarch`
(`os_neoplutarch.rs:15`), and kibi's bin needs `#![feature(neoplutarch_ext)]` (and
`#![feature(restricted_std)]`, as any custom-target std consumer does).

`enable_raw_mode` uses `mode()`; if #12 does not add a getter, replace `let orig = …` with
`TermMode { cooked: true, echo: true }` and document that every terminal child starts cooked. The
getter is three lines in the term page and removes an assumption, so I recommend adding it.

### 6.3 The entry, allocator and manifest

kibi is a normal `fn main` binary today. On this target it needs the same entry shape as
`std-real-smoke` (`userland/std-real-smoke/src/main.rs:37-61`):

```rust
#![no_main]
#![allow(unsafe_code)] // `#[unsafe(no_mangle)]` on `_start` only
#![feature(restricted_std)]
#![feature(neoplutarch_ext)]

use neo_std_pal::SyscallHeap;

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let code = match kibi::main_entry() {
        Ok(()) => 0,
        Err(e) => { eprintln!("Error: {e:?}"); 1 }
    };
    neo_std_pal::exit(code)
}
```

Move the `main` body into `kibi::main_entry() -> Result<(), Error>` in `src/lib.rs` so both the host
`fn main` and the target `_start` can call it. `Cargo.toml` additions (target-only):

```toml
[target.'cfg(target_os = "neoplutarch")'.dependencies]
neo-std-pal = { path = "../../userland/neo-std-pal" }
```

No `libneo` dependency is needed if the term calls live in the std extension and the file calls stay
plain `std::fs`. That keeps kibi std-only and avoids pulling libneo's rustls closure into the editor.

The manifest patch is exactly: `publish = false`, `rust-version = "1.97"`, remove `build.rs`, and add
the target dependency. Record all four in `VENDORED.md`. Keep `Cargo.toml.upstream` unchanged for
diffing. License files unchanged; source files keep their upstream SPDX headers (they already have
`SPDX-FileCopyrightText` + `SPDX-License-Identifier: MIT OR Apache-2.0`; if the close-gate insists on
an `SPDX-License-Identifier` on line 1, add it above, as `third_party/miniz_oxide` does, and record it).

### 6.4 The PAL work L1b needs (and does not own)

These are L1d packages, shared, not kibi forks:

| Package | What L1b needs from it | Why it is shared |
|---|---|---|
| **P0** argv | `std::env::args` returns the spawn argv | Every non-builtin program |
| **P1** entry + allocator | `_start` + `SyscallHeap`; the `restricted_std` build plumbing | Every `std` userland bin; L1d's first utility |
| **P2** stdin | `Stdin::read` over fd 0 (the terminal pipe in L1b) | Every filter/editor |
| **P8** file semantics | `SYS_LSEEK`; `OpenOptions` open-or-create; `File::truncate` via the existing `SYS_FTRUNCATE` | coreutils `head`/`cp`/`mv`; the editor's load/save |
| **Term extension** | `std::os::neoplutarch::term::{size, set_mode, mode, stdin}`; `IsTerminal`; `HandleKind::{Pipe,TermPage}` | L1a.2/L1a.3 and any future TUI |

The `File::create` semantics are worth spelling out because they are two defects, not one:

1. `fs_neoplutarch.rs:248` refuses `truncate` before the syscall.
2. `SYS_OPEN(OPEN_CREATE_REGULAR)` is create-new-only (`neofs/inode.rs:356-358`), so even with truncate
   allowed, an existing file is `EINVAL`.

The fix belongs in the std overlay, not in kibi: try `OPEN_ROOT` first; on `-ENOENT`, try
`OPEN_CREATE_REGULAR`; then, if `truncate`, call `rt::ftruncate(handle, 0)` (add `SYS_FTRUNCATE` to
`neoplutarch_rt`; the kernel already implements it). `File::seek` then needs `SYS_LSEEK` over
`OpenFile.offset` (L1d P8). With those, kibi's `load` and `save` are unmodified.

If L1b must land **before** P8, the cheap escape is a two-patch fork of kibi:

- `load`: read the whole file into a `Vec<u8>` and check its last byte, instead of `seek(End(0))` +
  `bytes().next()`; this also fixes an upstream bug (§7.1).
- `save`: open with `OpenOptions::new().write(true).create(true).open()` (no truncate), then
  `libneo::sys_file_replace(handle, &bytes)` (`libneo/src/lib.rs:1255-1266`), which is the kernel's
  atomic whole-file replace (OT-67 W1, 4 MiB cap). It needs the `neofs-file-replace` feature and a
  write `RootDirCap`.

The first path (fix the PAL) is the better one: it serves coreutils and does not fork the editor's
logic. The second is a schedule escape, not the design.

### 6.5 What not to do

- **Do not add `"target-family": ["unix"]` to `x86_64-unknown-neoplutarch.json`.** That flips
  `cfg(unix)`, pulls in `libc`/`rustix`/`nix` and every `std::os::unix` arm, and claims a POSIX
  surface the OS does not have. It trades a bounded `neoplutarch` arm for an unbounded compatibility
  claim (the same finding as L1d §7.3).
- **Do not port `libc` or termios.** The target has no termios syscall and no signals. The term page is
  the interface.
- **Do not add SIGWINCH.** There is no signal delivery in this design. Poll the term page; add a futex
  only if profiling shows the 100 ms poll is too coarse.
- **Do not let kibi write warnings to stderr on the terminal's stdout pipe.** Use empty
  `conf_dirs`/`data_dirs` for v1, or add a log channel to #12.
- **Do not leave DECCKM set.** kibi parses `ESC[A-D` only; SS3 arrows become `Escape`.
- **Do not use the default std allocator.** `alloc_neoplutarch.rs:26-45` leaks one mapping per
  allocation; kibi allocates constantly (rows, `String` render buffers, `Vec` per line). `SyscallHeap`
  is not optional for an editor session.

## 7. Cross-cutting traps

### 7.1 kibi 0.3.3 always appends a trailing empty row

`load` says it checks "the last byte directly" (`editor.rs:478-483`), but it seeks to `End(0)`, i.e.
EOF, and then reads: `file.bytes().next()` is always `None`. The condition
`is_none_or(|b| b == b'\n')` is therefore always true, so **every loaded file gets an extra empty row**.
A host test of the exact code confirms it:

```
content="a"   rows=[[97]] last=None
content="a\n" rows=[[97]] last=None
content=""    rows=[]     last=None
content="\n"  rows=[[]]   last=None
```

The effect is that saving a file without a trailing newline adds one. For the L1b gate, seed
`/hello.txt` **with** a trailing newline (`hello\nworld\n`) so the round trip is stable; add a separate
leg for a no-trailing-newline file to catch the bug, and fix it in the vendored copy (read the bytes
and test the last one) if the PM wants byte-exact round trips. This is upstream's bug, not the port's;
record it in `VENDORED.md`.

### 7.2 A closed terminal makes kibi spin

`loop_until_keypress` (`editor.rs:230-273`) is:

```rust
loop {
    if sys::has_window_size_changed() { … }
    if let Some(a) = bytes.next().transpose()? {
        …
    }
}
```

There is no `else` for `None`. On a terminal, `VMIN=0/VTIME=1` makes `read` return 0 on timeout, which
`bytes.next()` turns into `None`, and the loop polls again after 100 ms — correct. On a **pipe EOF**,
`read` returns 0 immediately, so `bytes.next()` is `None` forever and the loop is a hot spin. The
terminal's stdin pipe closes when the terminal exits (#12 §3.4), so this is reachable. Fix at the
boundary: the backend's `fill_buf` returns `Err(BrokenPipe)` on EOF (so kibi exits with `Error: Io`), or
patch the loop to return from `run` on EOF. Do not let the gate call a spin a pass.

### 7.3 A lone Esc needs the timeout, and Esc followed quickly by a key is ambiguous

With `VMIN=0/VTIME=1`, `ESC` is read, then the parser immediately tries the next byte; if none arrives
within 100 ms, it returns `Key::Escape`. If the user types `Esc` then `i` within 100 ms, the parser
consumes `i` as the (invalid) sequence tail and returns `Escape`, losing the `i`. This is kibi's
Unix behaviour too, not a port regression, but the gate must type `Esc`, wait >100 ms, then `i`; and
the port must not make the timeout longer than 100 ms without re-checking this.

### 7.4 DSR 6n can be dropped

#12 §5.4 writes answerbacks non-blocking and drops them if the stdin pipe is full. kibi's fallback
`read_exact` (`terminal.rs:22`) has no deadline, so a dropped answerback with an otherwise idle pipe is
a hang. The term page makes the fallback unnecessary; if it is kept, give it the same 100 ms deadline
and treat a timeout as `CursorPosition` rather than blocking.

### 7.5 `eprintln!` is the screen

`STDERR_FD = CONSOLE_FD = 1` (`stdio_neoplutarch.rs:25-26`), and in L1a the child's stderr is the same
pipe as stdout (`THINK-2026-10-31-l1a-terminal-window.md` §3). kibi's config/syntax warnings
(`config.rs:87,90`, `syntax.rs:68,72`) are therefore bytes for the terminal's parser. Empty directories for
v1 are the cheap fix; a real config later needs a log channel or status-bar routing.

### 7.6 `build.rs` is a hermeticity hazard in a vendored tree

`build.rs` shells out to `git describe --tags --match=v*` and uses the first byte of stdout
(`build.rs:7-12`). In `third_party/kibi`, that is the LogansOS repository, not kibi's; if our repo ever
has a `v*` tag, kibi's `CARGO_PKG_VERSION` changes. Drop `build.rs` and use the manifest version. This
also removes a host tool dependency from a hermetic build.

### 7.7 The toolchain gate is a lint, not a language gate

Do not conclude "kibi needs 1.99 language features" from `rust-version = "1.99"`. The bump commit adds
one clippy lint. The source compiles on 1.97. The correct record is: *declared MSRV 1.99; the delta is
a clippy lint; vendored copy patched to 1.97 and verified by the L1b build.* If a future upstream tag
uses a real 1.99 feature, the build will fail loudly and the PM can revisit; do not pre-emptively bump
the OS toolchain.

## 8. Order of work in build cycles

A **cycle** is one build → gate → diagnose loop on the pinned toolchain. Ranges are uncertainty, not
calendar time. "Host" means no QEMU slot; "4-shard" means the QEMU gate.

| Step | Work | Depends on | Gate | Cycles |
|---|---|---|---|---|
| **B0** | Vendor `third_party/kibi` at `d42f6e3`: licences, `VENDORED.md`, `Cargo.toml.upstream`, `rust-version` patch, drop `build.rs`, `syntax.d`. Add the `neoplutarch` cfg arm and the backend skeleton against a **host** `term` shim so it compiles and its size/poll logic is host-tested. | nothing | host `cargo test`/`cargo check` in the vendored crate; no QEMU | 1 |
| **B1** | The std term extension: `size`, `set_mode`, `mode`, `stdin`, `IsTerminal`, `HandleKind::{Pipe,TermPage}`, over the term page. Host KAT for the page codec (the #12 §3.2 byte oracle, both directions) and for the `BufRead` timeout/EOF contract. | L1a.1 (pipe + term page) | host KATs; the term page is a syscall shape, so its wire bytes are a byte oracle | 2–3 |
| **B2** | L1d P0/P1/P2/P8 as needed by kibi: argv, `_start`/`SyscallHeap`, stdin, `SYS_LSEEK`, open-or-create, `File::truncate`. Most of P8 is std overlay; only `SYS_LSEEK` is kernel. | L1d packages; P8 can run before B1 | 4-shard for the kernel part; host tests for the overlay | 3–5 (shared) |
| **B3** | kibi target build: the real `neoplutarch.rs`, `_start`, allocator, `main_entry`; build in `third_party/kibi`; fix link/entry errors; stage `/modules/kibi` in xtask. | B0, B1, B2 | build only; entry must be inside a loaded segment | 1–2 |
| **B4** | Launch and read: the terminal app spawns kibi with argv and a `RootDirCap`; kibi opens a seeded `/hello.txt`; grid matches. This needs L1a.3 (the terminal app) and the spawn-cap/argv path (L1d P12/P0). | L1a.3, B3 | 4-shard `kibi_open` | 2–3 |
| **B5** | Save: `File::create` semantics (or the file-replace escape), save a changed buffer, read the bytes back, `sync_all`. | B4, B2 | 4-shard `kibi_save` | 1–2 |
| **B6** | The full `kibi_open_edit_save` gate: keys, resize, lone Esc, EOF, save, quit, lock screen, argv negative. | B5 | 4-shard | 1–2 |

The critical path is **L1a.1 → L1a.3 → B4 → B5 → B6**; B0 can start now and B1 can start as soon as
L1a.1's term page exists. B2 is shared with L1d, so it should be scheduled in L1d's order (P0+P1 first,
then P2, then P8), not as a private L1b prerequisite. The full lane is roughly **10–16 cycles after
L1a.1**, with the terminal app (L1a.3) the long pole; it is not a "vendor and run" job.

**First concrete build after this survey:** B0, because it is host-only, needs no QEMU slot, and
de-risks the one piece that is uniquely kibi's: the `neoplutarch.rs` seam and the `term` API shape.

## 9. The L1b gate scenario

Name: `kibi_open_edit_save` (xtask scenario; 4-shard QEMU). It runs the real terminal app with kibi as
its child, not through `neo-shell` (the shell cannot exec an external module until L1c). The scenario
selects the child module directly, the same spawn path the terminal uses for `neo-shell`; the production
path (kibi from the shell) is L1c work and is out of scope here.

### 9.1 Setup

- Terminal window 80 × 25; term page `columns=80`, `rows=25`, pixels 640 × 400.
- Seed `/hello.txt` with exactly `hello\nworld\n` (12 bytes) through the test's write cap.
- Spawn kibi with argv `["kibi", "/hello.txt"]`, a read-write `RootDirCap`, stdin/stdout pipes, and the
  term page. kibi's `ln_pad` is 3 (three rows, one digit); `screen_rows` is 23; `screen_cols` is 77.
- Enable `desktop-session` (so `SYS_CLOCK_GETTIME` answers) and `neofs-file-replace` if the save escape
  uses it; otherwise the P8 path.

### 9.2 What the scenario types and checks

| # | Typed / event | Expected |
|---|---|---|
| 1 | (startup) | Grid row 0 `1 │hello`, row 1 `2 │world`, row 2 `3 │` (the extra empty row kibi appends), row 3 `~ │`; status bar (0-based row 23, the 24th row) contains `hello.txt`, `12B`, and `1:1`; cursor at row 0, column 4 (1-based; `ln_pad` 3 + 1); alternate screen on, cursor visible. |
| 2 | `X` | Grid row 0 `Xhello`; status bar `(modified)`; cursor after `X` (column 5). This proves raw mode (no Enter needed) and the key path. |
| 3 | `Right`, `Right`, `Right` | Cursor moves right three cells, to column 8 (before the second `l` in `Xhello`); no text change. |
| 4 | `End`, `Home` | Cursor to after `Xhello`, then column 4; no text change. |
| 5 | `Down` | Cursor to row 1, column 4 (clamped to `world` length). |
| 6 | `Ctrl+Right` (`ESC[1;5C`) | Cursor to the end of `world` (no spaces); no text change. |
| 7 | `Up`, `Home`, `Delete`, `X` | Up to the first row, Home to its start, Delete removes the `X` (`hello`), then `X` restores `Xhello`; the net text is unchanged. This is the delete-path check. |
| 8 | `Ctrl+S` | No prompt (file name known); status bar `… written to /hello.txt`; the test reads `/hello.txt` and gets exactly `Xhello\nworld\n` (13 bytes). `sync_all` observed. |
| 9 | Resize to 100 × 30 | Within one 100 ms read timeout plus one frame, kibi repaints: status bar at 0-based row 28 (the 29th row), message bar row 29, line numbers still 3 wide, no stale rows below 30. The next key works. |
| 10 | `Esc`, wait 150 ms, `E` | The grid gains `E` (the buffer becomes `XEhello`); the Esc did not swallow it. (Do not type `E` within 100 ms; see §7.3.) |
| 11 | `Ctrl+F`, `hello`, Enter | Find completes; the match is highlighted (`SGR 46`); cursor on the match. |
| 12 | `Ctrl+E`, `echo hi`, Enter | A loud, non-crashing status-bar error (`Unsupported`/no process seam), **not** a silent no-op. The buffer is unchanged. If a later rung implements exec, this becomes an insert of `hi`. |
| 13 | `Ctrl+Q` | Exits (twice if the `E` left the buffer dirty); terminal writes `?1049l`; the terminal returns to the main screen and its prompt; the kibi process is gone; Arthur/WM still live. |
| 14 | separate leg: close the terminal / stdin pipe while kibi is idle | kibi exits (non-zero or zero with a clean restore), does **not** spin. Check the process's CPU time over 1 s. |
| 15 | separate leg: lock screen | With kibi running, `Super+L`, type `x` + Enter (to the lock), unlock, type `y`; kibi's buffer contains `y`, not `x`. This is #12 §6 inherited. |
| 16 | separate leg: `kibi --version` | Prints `kibi 0.3.3` to stdout and exits 0; with empty argv this would start the editor, so this is the P0 discriminator. |
| 17 | separate leg: no argv | Starts a blank buffer with `[No Name]`; does not panic; `Ctrl+Q` exits. |
| 18 | separate leg: save to a new path | `Ctrl+S` on a blank buffer prompts for a name; type `/new.txt` + Enter; bytes match. |

### 9.3 Byte oracle

The gate should not check "the grid contains `hello`". It should compare the **whole 80 × 25 grid**
against the hand-laid expectation, as #12 §5.3 does for its G-rows. kibi's startup script is exactly:

```
?1049h  ?25l  H  K  "1 │hello" \r\n  K "2 │world" \r\n  K "3 │" \r\n
K "~ │" \r\n …  K (status) \r\n  K (msg)  CUP(1,4)  ?25h
```

The expected grid rows are written from #12's grid oracle and kibi's code, not from what the terminal
happens to draw. Add oracle rows for:

- `ESC[m` (no parameter) = SGR 0;
- `ESC[999C` / `ESC[999B` clamp at the last column/row (the DSR fallback depends on it);
- `ESC[38;5;240m` then text then `ESC[m` (the line-number gutter);
- `?1049h` … `?1049l` round trip with the cursor (kibi saves/restores the main screen).

The saved file is compared byte-for-byte: `Xhello\nworld\n`, not a string containing it. This is the
same "byte oracle, not round trip" rule as ADR-028 and L1d's `coreutils_echo`.

### 9.4 What the gate does not cover

- No real hardware: the gate runs in QEMU, so it proves the protocol and the PAL, not a physical
  keyboard or a real WM resize. The bare-metal terminal gate is L1a.3's.
- No `neo-shell` launch: the direct-child selection is a test path. L1c must add the shell exec and a
  second gate leg.
- No large-file case: the 4 MiB `SYS_FILE_REPLACE` cap (if that path is chosen) fails loudly; test a
  file over the cap separately if the product needs it.
- No multi-window focus: L1a.3 covers focus; L1b only needs its own window.

## 10. Decisions for the PM / J.c.

1. **Toolchain:** patch the vendored `rust-version` to 1.97, or bump the OS toolchain to 1.99? Recommend
   the patch (§2). The 1.99 delta is a clippy lint; the source compiles on 1.97.
2. **File semantics:** wait for L1d P8 (`SYS_LSEEK`, open-or-create, `File::truncate`) or fork kibi's
   load/save for L1b? Recommend P8; it is shared with coreutils and keeps kibi unmodified. Fork only if
   the PM wants an earlier editor demo.
3. **Who gives kibi the file cap?** The terminal's child handle set in #12 has stdin/stdout/term page,
   not a `RootDirCap`; the shell has one but `spawn_child` passes only a channel. Recommend the shell
   exec path (L1c) with a per-tool cap, and use a direct-child test selection for the L1b gate.
4. **#12 delta:** add the timed read/wait and the timeout-vs-EOF contract before L1b; add `mode()`,
   `HandleKind::{Pipe,TermPage}`, all C0/Ctrl-arrow key rows, and the `ESC[m`/999C/999B oracle rows.
5. **Config and syntax:** empty dirs for v1, or implement `stat`/`readdir` plus a read-only config cap?
   Recommend empty dirs for v1; config/syntax is a later rung, and the warnings-on-stderr hazard
   disappears.
6. **Ctrl-E:** refuse loudly for v1, or implement a capability-checked exec with stdout capture?
   Recommend refuse; `std::process::Command` is an embedder seam, not an ambient API.
7. **Vendor at `d42f6e3` or a later tag?** `d42f6e3` is the checked head and the survey's row. Vendor it,
   record the SHA; re-vendor when a real 1.99 language feature or a bug fix lands.

## 11. Verification / method

| Check | Result |
|---|---|
| Upstream head / date / licence / MSRV / edition | read from a fresh clone on 2026-11-01; table in §1 |
| kibi source citations | every `file:line` is from the clone at `d42f6e34` |
| Our code citations | `origin/main` @ `c97bb856`; code subtrees unchanged from `e9330a22` |
| PR #12 citations | PR head `ae7a9985`, revision 2 |
| Build A/B/C/D/E | exact commands and errors in §5; logs under `/tmp/buildlogs/kibi-*.log` on this box |
| Scratch commits | none. `/tmp/kibi-l1b` is a local scratch tree; the target build is not committed. |
| Target build | run D: 251 KiB, `e_entry 0`; run E: 1.4 MiB, `e_entry 0x21aef0` inside the `R E` segment |
| RAG | not available on this box (as the dispatch says); used `grep`, direct reads and the upstream clone |
| Personal details | none: the THINK uses repo-relative and `/tmp` paths only, no hostnames, addresses or emails |
| Kernel-touching | no code changes in this relay. The L1b *build* later needs L1a.1 (kernel) and P8's `SYS_LSEEK`; the survey does not change either. |
