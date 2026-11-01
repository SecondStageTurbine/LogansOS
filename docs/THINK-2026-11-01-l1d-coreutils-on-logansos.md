<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: L1d — uutils coreutils on LogansOS

**Date:** 2026-11-01 · **Author:** Dexter (Relay 239) · **Asked by:** the PM, from
`docs/THINK-2026-10-31-lived-in-os-lanes.md` §1 and §3.
**Status:** survey only; this relay changes no code. Citations to our tree are
`origin/main` @ `e9330a22`. Upstream is uutils/coreutils
`cac8be1f17667aa337a3c8951fc77d98ece9d0b5`, cloned and read on 2026-11-01.

**Bottom line.** The ten utilities are not ten independent porting jobs. Three facts decide the lane:

1. **Nothing runs correctly yet, because there is no argv.** `std::env::args` on
   `x86_64-unknown-neoplutarch` falls to std's `unsupported` arm, whose `args()` returns an
   *empty iterator* (`library/std/src/sys/args/mod.rs:57-58`,
   `library/std/src/sys/args/unsupported.rs:6,20-21`). The uutils `uucore` layer indexes
   `ARGV[0]` (`src/uucore/src/lib/lib.rs:392-395`), so `cp`, `mv`, `wc` and `tail` abort
   before parsing anything; `echo`, `ls`, `cat`, `mkdir` and `head` silently see zero
   operands instead. The kernel's `SYS_SPAWN` has no argv argument
   (`kernel/src/arch/x86_64/syscall.rs:1838`), so this is an ABI build, not a PAL stub.
2. **uutils does not know this OS**, and its cfg model is two-valued: `unix` or `windows`
   (plus WASI). On `os = "neoplutarch"` both are false, so uucore's `fs`, `pipes`,
   `entries`, `perms`, `safe-copy`, `safe-traversal` and `signals` modules are compiled
   out (`src/uucore/src/lib/lib.rs:65-66,112-150`). That is why `mkdir` and `head` fail
   on `uucore::fs`, and why `rm` "builds" to nothing.
3. **The file surface is open/read/write/sync/close only.** `std::fs::metadata`,
   `read_dir`, `create_dir`, `remove_file`, `rename`, `seek`, `truncate` and the link and
   permission calls are all `unsupported` in `fs_neoplutarch.rs` (lines cited in §2), while
   the kernel already has `SYS_READDIR` and `SYS_FTRUNCATE` and lacks `stat`, `mkdir`,
   `unlink`, `rename` and `lseek` entirely. coreutils reads metadata on nearly every path.

So the first build is **not a utility**: it is the argv ABI plus a `_start` shim (both
shared with L1c), then a small uutils `neoplutarch` platform arm. After that, `echo` lands
in one cycle as the plumbing gate; `cat`, `wc` and `head` are the first utilities that add
capability. `ls -l`, `cp` and `rm` wait on a stat syscall and on a decision about what a
POSIX mode bit means in a capability filesystem (§7.4).

## 1. Re-check on the day

Read from a shallow clone on 2026-11-01 (`git log -1`, `LICENSE`, workspace `Cargo.toml`):

| Item | Value |
|---|---|
| Repository | `https://github.com/uutils/coreutils` |
| Head | `cac8be1f17667aa337a3c8951fc77d98ece9d0b5` |
| Head date | `2026-10-31 19:59:12 +0100` |
| Head subject | `mknod: support non-UTF8 paths` |
| Licence | **MIT** — `LICENSE` (MIT text) and `Cargo.toml` `license = "MIT"`; per-utility crates inherit it (`license.workspace = true`) |
| Workspace version / edition / MSRV | `0.13.0` / 2024 / `1.88.0` |
| Per-utility crates | `src/uu/<name>`, package `uu_<name>`, each with its own `Cargo.toml` and `[lib] path = "src/<name>.rs"` |
| Published crates | crates.io carries `uu_<name>` at **0.12.0**, one minor behind this workspace (`cargo info uu_echo`). Do not mix the two (§7.7). |

The 2026-10-31 survey row recorded `cac8be1`; the head is the same a day later. Nothing
moved upstream between that survey and this relay.

## 2. What the target's platform layer is today

In this THINK, **"the PAL"** means the whole target platform layer, which is two pieces:

- `userland/neo-std-pal/src/lib.rs` — the C0 primitive crate (`write`, `exit`,
  `map_region`, `SyscallHeap`, and *named* stubs for everything not built);
- `userland/neoplutarch-std-overlay/std/*` — the `library/std/src/sys/*` arms that actually
  serve C1/C2 today. The file calls live here, not in the C0 crate: for example
  `neo-std-pal::fs_unsupported` (`lib.rs:340`) is no longer the fs path, while
  `fs_neoplutarch.rs` is.

The split matters when reading the table: a call can be "implemented" in the overlay while
`neo-std-pal` still carries the generic stub for the shape it replaced.

None of the ten needs `neo-crt`. Every `libc::...` in this document is the Rust `libc`
crate (a symbol namespace, and an empty one on this target), and `rustix`/`nix` are Rust
syscall wrappers. The OT-61 C runtime (`neo-crt`, musl + libc++) is irrelevant to L1d;
nothing here links C.

| Surface | Where it is today | State for coreutils |
|---|---|---|
| stdout write | `neo-std-pal/src/lib.rs:97` (`write_fd`); stdio overlay | **implemented** |
| exit | `neo-std-pal/src/lib.rs:109`; `neoplutarch_rt.rs:115` | **implemented** |
| heap | `neo-std-pal/src/lib.rs:124,150` (`SyscallHeap`, linked-list, NX); std's own `System` (`alloc_neoplutarch.rs`) leaks one map per allocation | **partial**: a real allocator exists but uutils does not install it |
| argv | *no overlay module*; std falls to `sys/args/unsupported.rs` | **lacks**, and fails **silently** (empty iterator) |
| env | `env_neoplutarch.rs:31-35` | **stub by policy**: empty and immutable. Fine for `LANG`/`POSIXLY_CORRECT` reads |
| stdin | `stdio_neoplutarch.rs:46-48` | **stub**: `read` returns `Unsupported` |
| `File::open` / read / write / fsync / close | `fs_neoplutarch.rs:237,319,335`; `neoplutarch_rt.rs:221-250` | **implemented**, but only for a process holding a `RootDirCap` (`fs_neoplutarch.rs:240`) |
| `OpenOptions` append / truncate / create_new | `fs_neoplutarch.rs:248` refuses them | **lacks** |
| `File::seek` / `tell` | `fs_neoplutarch.rs:351` | **lacks**: the VFS `OpenFile` has an offset (`kernel/src/fs/mod.rs:193,434-437`), but there is no `lseek` syscall |
| `File::truncate` | `fs_neoplutarch.rs:315` | **stub**, but the kernel already has `SYS_FTRUNCATE` (`syscall.rs:115,6823`) — wire only |
| `File::metadata` / `fs::metadata` / `stat` / `lstat` / `exists` | `fs_neoplutarch.rs:283,446,450,430` | **lacks**: no stat in the kernel VFS either (`Inode` has `kind`/`byte_len` only, `fs/mod.rs:140-167`) |
| `fs::read_dir` / `DirEntry::file_type` | `fs_neoplutarch.rs:398` | **lacks** std wiring; the kernel has `SYS_READDIR` (`syscall.rs:108,1134,6580`) and libneo wraps it (`libneo/src/lib.rs:1267`) |
| `fs::create_dir` / `DirBuilder::mkdir` | `fs_neoplutarch.rs:387` | **lacks**: no kernel mkdir |
| `fs::remove_file` / `remove_dir` / `remove_dir_all` | `fs_neoplutarch.rs:402,422,426` | **lacks**: no kernel unlink/rmdir |
| `fs::rename` | `fs_neoplutarch.rs:406` | **lacks**: `SYS_FILE_REPLACE` (`syscall.rs:118,6844`) replaces *content*, not a name |
| `fs::read_link` / `symlink` / `hard_link` | `fs_neoplutarch.rs:434,438,442` | **lacks** |
| `fs::canonicalize` | `fs_neoplutarch.rs:454` | **lacks** |
| `fs::set_permissions` / `set_times` | `fs_neoplutarch.rs:367,371,410,414` | **lacks** |
| `std::time::Instant` | `time_neoplutarch.rs:2-7` over `SYS_CLOCK_GETTIME` | **implemented** |
| `std::time::SystemTime` | falls to std's `unsupported` | **lacks**; the kernel already answers `CLOCK_REALTIME` under `rtc-epoch` (`syscall.rs:233,7233-7252`) |
| `std::process::Command` | `neo-std-pal/src/lib.rs:357` | **stub** (not needed by these ten) |
| threads / futex / TLS | C1.3 overlay | **implemented** (not needed by these ten) |

Two overlay details are load-bearing for this lane:

- The fallback audit in `apply-overlay.py:346` accepts `args/mod.rs` because its `_ =>` arm
  names `mod unsupported`. It checks the *name*, not the behaviour. `sys/args/unsupported.rs`
  is **not loud**: it returns zero arguments. That violates the PAL's own "loud absence"
  property (`neo-std-pal/src/lib.rs:274-292`) and is the reason `echo` can exit 0 with the
  wrong output instead of failing.
- `SYS_READDIR` and `SYS_FTRUNCATE` are not in `tools/ot61-pal-manifest.toml`'s
  `allowed_syscalls`, because std does not issue them yet. Wiring them into the overlay adds
  two manifest rows with provenance, as the C1/C2 rows already do.

## 3. Per-utility survey

Every utility's `[[bin]]` is `uucore::bin!(uu_<name>)` (`src/uucore/src/lib/lib.rs:264-282`),
which takes `uucore::args_os()` and calls `std::process::exit`. On this target that path is
already missing its first and last pieces: the args are empty, and the resulting ELF has
`e_entry = 0` because there is no `_start` (see §4.3).

The call lists below are from the crate's own source; `(U)` marks a call that is under
`#[cfg(unix)]` and therefore not compiled for us today. That is not a free pass: the unix
arm is where the utility's *behaviour* lives (mode bits, symlinks, `stat` fields), so
porting it is a `neoplutarch` arm, not a cfg flip.

### 3.0 `cargo tree` on the target

`cargo tree -p uu_<name> --target userland/x86_64-unknown-neoplutarch.json
-Zjson-target-spec` was run for all ten. The target-resolved direct dependencies, with the
system-facing ones marked:

| Utility | Direct deps (target-resolved) | System-facing / non-porting |
|---|---|---|
| `ls` | `ansi-width`, `clap`, `fluent`, `glob`, `hostname`, `lscolors`, `rustc-hash`, `terminal_size`, `thiserror`, `uucore`, `uutils_term_grid` | `hostname` **fails to compile**; `terminal_size` returns `None` |
| `cat` | `clap`, `fluent`, `memchr`, `thiserror`, `uucore` (`rustix` is `cfg(unix)`, not built) | none at the target; `rustix` absent |
| `cp` | `clap`, `filetime`, `fluent`, `indicatif`, `libc`, `thiserror`, `uucore`, `walkdir` | `filetime` **fails**; `indicatif` → `console` **fails**; `libc` empty |
| `mv` | `clap`, `fluent`, `indicatif`, `nix`, `rustc-hash`, `thiserror`, `uucore` | `console` **fails**; `nix` empty |
| `mkdir` | `clap`, `fluent`, `uucore` | none |
| `rm` | none compiled — every dep is under `cfg(any(unix, windows, wasi))` | — |
| `echo` | `clap`, `fluent`, `uucore` | none |
| `wc` | `bytecount`, `clap`, `fluent`, `thiserror`, `unicode-width`, `uucore` (`libc`/`rustix` are `cfg(unix)`) | `rustix` absent |
| `head` | `clap`, `fluent`, `memchr`, `thiserror`, `uucore` | none |
| `tail` | `clap`, `fluent`, `memchr`, `thiserror`, `uucore`, `notify` (`libc`/`rustix` are `cfg(unix)`) | `notify` falls back to `PollWatcher` |

The three failing third-party crates (`hostname`, `console`, `filetime`) are what P11's
"dependency policy" row means: patch each with a `neoplutarch` arm (a no-op progress bar, a
`gethostname` that returns a fixed name, a no-op `utimes`), or drop the feature on this
target. Patching is cheaper than replacing them, and each is a small, isolated change.

### 3.1 `ls`

- `fs::read_dir` (`src/uu/ls/src/ls.rs:1225`, recursion at `:1458,1486`); `DirEntry` and
  `FileType`; `Path::metadata`/`fs::metadata` (`display.rs:258`, `colors.rs:346`);
  `fs::canonicalize` (`display.rs:796`); `SystemTime`/`UNIX_EPOCH`.
- `(U)` `std::os::unix::fs::{MetadataExt, FileTypeExt}` (`ls.rs:20`, `display.rs:14`) and
  `uucore::libc::{dev_t, major, minor}` (`display.rs:45`) — the whole `-l` column set
  (mode, nlink, uid/gid, size, inode, time).
- `hostname::get()` for dired (`display.rs:1227`); `terminal_size` for the grid (compiles,
  returns no size); `lscolors` only if `--color` is on.
- **Build:** fails in `hostname 0.4.2`: `compile_error!("Unsupported target OS!")` at
  `hostname-0.4.2/src/lib.rs:67` (the crate has unix/windows/wasi arms only). `libc`,
  `lscolors` and `terminal_size` compile.
- **Gaps:** readdir (P3); stat (P4) for `-l`; `SystemTime` (P10) for the time column;
  dired's `hostname` (P11). A plain `ls` (names plus dir/file kind from the dirent) needs
  **only P3**.

### 3.2 `cat`

- `File::open` (`src/uu/cat/src/cat.rs:396`), `stdin`/`stdout` with `is_terminal`
  (`:381-383`), `OpenOptions` (7 sites).
- `(U)` `rustix::fs::{OFlags, SeekFrom, fcntl_getfl}` and `is_safe_overwrite`
  (`src/uu/cat/src/platform/unix.rs:8,35-36`); `platform/mod.rs:6-18` has unix, windows and
  wasi arms only.
- **Build:** `error[E0432]: unresolved import crate::platform::is_safe_overwrite` at
  `cat.rs:10`, "no `is_safe_overwrite` in `platform`" with all three arms configured out.
- **Gaps:** a nine-line `#[cfg(not(any(unix, windows, wasi)))]` arm returning `false`
  (the wasi arm's policy), P2 stdin for the no-file case, P1 shim. File reads already work
  through `fs_neoplutarch.rs`.

### 3.3 `wc`

- `File::open` (`src/uu/wc/src/wc.rs:696,775`), `fs::metadata` (`:284,740`), implicit
  stdin (`Input::Stdin`, `:153,178`), `uucore::util_name()` in `uu_app` (`:395`).
- `(U)` `rustix::fs::fstat` + `libc::S_IFMT` (`wc.rs:301-302`, `count_fast.rs:68-111`) —
  the splice/fast path; the non-unix arm returns `false` (`wc.rs:304-310`).
- **Build:** **succeeds**; produces `target/neo-target/debug/wc` (28.5 MB), `e_entry 0`.
- **Gaps:** argv (P0), stdin (P2), shim (P1). The `util_name()` call aborts today on the
  empty `ARGV[0]`; the metadata fast path degrades to an `Unsupported` error and the slow
  path still counts bytes/lines/words.

### 3.4 `head`

- `File::open` (`src/uu/head/src/head.rs:510`), `seek` (`:334,350,389-390,468`), `metadata`
  (`:394,415,536`), implicit stdin (`:455`), `uucore::fs::sane_blksize` (`:396`).
- `(U)` `std::os::unix::ffi::OsStringExt` (`:700`); the `#[cfg(unix)]` stdin-clone arm
  (`:458-470`) is not compiled; the `not(unix)` arm just locks stdin (`:475-487`).
- **Build:** `error[E0433]: cannot find fs in uucore` at `head.rs:396`, gated out at
  `uucore/src/lib/lib.rs:65-66`.
- **Gaps:** P2 stdin; either enable `uucore::fs` with a `neoplutarch` arm for
  `sane_blksize` (the `not(unix)` arm already returns `DEFAULT` 512) or drop that one
  call; P8 seek only for `-c -N` (all-but-last-N bytes).

### 3.5 `mkdir`

- `std::fs::create_dir` (`src/uu/mkdir/src/mkdir.rs:264`), `remove_dir` for the rollback
  (`:320`), `uucore::fs::dir_strip_dot_for_creation` (`:20`), `uucore::mode::get_umask` /
  `parse_chmod` (`:15,70`).
- `(U)` `std::os::unix::fs::DirBuilderExt` (`:255`); the `not(unix)` arm calls plain
  `std::fs::create_dir` (`:263-265`), which is exactly our case.
- **Build:** `error[E0432]: unresolved import uucore::fs` at `mkdir.rs:20`.
- **Gaps:** P5 (a new mkdir syscall; there is none), P11 (`uucore::fs` arm). `-m` is
  accepted today but there is no stored mode to set; see §7.4.

### 3.6 `rm`

- Removal goes through `platform::{safe_remove_file, safe_remove_empty_dir,
  safe_remove_dir_recursive}` (`src/uu/rm/src/rm.rs:33`); `fs::remove_dir` (`:125,740`);
  `read_dir` (`:532,628`); `symlink_metadata` (`:342,515`); `metadata`;
  `uucore::fs::{normalize_path, path_is_root_dir, path_ends_with_terminator}`;
  `uucore::safe_traversal` (`:8`); `indicatif` progress.
- `(U)` `libc` mode-bit logic (`platform/unix.rs:31-47`), `std::os::unix::ffi::OsStrExt`.
- **Build:** **exit 0, but no binary.** The crate root is
  `#![cfg(any(unix, windows, target_os = "wasi"))]` (`rm.rs:8`), so `uu_rm` is an empty
  library; the `[[bin]]` is skipped by `required-features = ["uucore/default"]`
  (`src/uu/rm/Cargo.toml`). Only `libuu_rm.rlib` is produced. This is the worst build
  result in the set because it looks green.
- **Gaps:** P6 unlink/rmdir; P11 crate gate plus `uucore::fs`/`safe_traversal` arms; P4
  lstat and mode bits for `-i` and the read-only prompt (basic `rm` can try unlink then
  rmdir and use the dirent kind for recursion).

### 3.7 `echo`

- `args.skip(1)` (`src/uu/echo/src/echo.rs:123-125`), `std::env::var_os("POSIXLY_CORRECT")`
  (`:138`), stdout. No filesystem call at all.
- **Build:** **succeeds**; `target/neo-target/debug/echo` (26.4 MB), `e_entry 0`.
- **Gaps:** P0 argv, P1 shim. Today the empty iterator means `echo` prints a bare newline
  and exits 0 — a silent wrong answer, not an error.
- **Note:** `neo-shell` already has an `echo` builtin (`userland/neo-shell/src/shell_logic.rs:133`),
  so this is a *plumbing* gate, not a capability win.

### 3.8 `cp`

- `fs::metadata`/`symlink_metadata` (`src/uu/cp/src/cp.rs:1508-1519,1827`),
  `set_permissions` (`:1837`), `fs::copy`, `File::open`/`create`, `seek`, `set_len`,
  `symlink` (`:2105`), `hard_link`, `create_dir_all`, `read_link`, `read_dir`,
  `canonicalize`, `env::current_dir`.
- `(U)` `nix::sys::stat::{mknod, Mode, SFlag}` (`:34`), `libc` mode bits
  (`:1406,1673-1674`), `rustix`/`std::os::unix::fs` in `platform/linux.rs`.
- **Build:** fails in `console 0.16.6` (via `indicatif`) with 23 errors, then in
  `filetime 0.2.29` with 14: `cannot find unix in os` (`src/unix/mod.rs:5`), unresolved
  `libc` (`src/unix/mod.rs:2,47-49`), `as_bytes` missing (`utimes.rs:43`).
- **Gaps:** P4 stat, P8 seek/truncate/append, P9 links/perms/times, P11
  `filetime`/`console` policy and the `fs`/`perms`/`safe-copy` arms. `-p` needs mode bits
  and times; without them it must be refused loudly, not silently ignored.

### 3.9 `mv`

- `fs::rename` (`src/uu/mv/src/mv.rs:962`), `fs::copy`, `remove_file`/`remove_dir`/
  `remove_dir_all`, `read_link` (`:1042`), `symlink` (`:1046`), `hard_link`,
  `read_dir` (`hardlink.rs:237`), `metadata` (`:833`), `canonicalize`, `env::current_dir`.
- `(U)` `rustix::fs::renameat_with` (`:660`), `nix::unistd::mkfifo` (`:1025`),
  `std::os::unix::fs::{MetadataExt, PermissionsExt}`.
- **Build:** fails in `console 0.16.6` with the same 23 errors. `nix 0.31.3` *compiles*
  but is an empty crate on this target (all its modules are `cfg(unix)`), so it is a false
  friend in the dependency tree.
- **Gaps:** P7 rename, P11 `console` policy and the `fs` arm, P9 for the `-s`/`-l` modes.
  Basic same-filesystem `mv` is rename-only; cross-filesystem `mv` degrades to `cp`+`rm`
  and inherits `cp`'s gaps.

### 3.10 `tail`

- `File::open`, `seek` (26 sites; `chunks.rs:55`), `metadata` (`follow/files.rs:135`),
  implicit stdin, `env::current_dir`, `notify` for `-f`.
- `(U)` `rustix::fs`/`libc::O_NONBLOCK` (`tail.rs:226-235`), `rustix::process`
  (`platform/unix.rs:9`), `rustix::fs::fstat` (`args.rs:351`).
- **Build:** 8 errors: `platform::ProcessChecker` (`watch.rs:548`), `platform::Pid`
  (`args.rs:150`), `platform::supports_pid_checks` (`args.rs:340`), `uucore::fs`
  (`tail.rs:174`).
- **Gaps:** P2 stdin, P8 seek, P4 metadata for size/time in `-f`, P11 `fs` and `platform`
  arms. `notify` 8.2.1 selects `PollWatcher` for unknown OSes
  (`notify-8.2.1/src/lib.rs:437`), so `-f` can work without inotify once seek exists.

## 4. The build attempt

### 4.1 Method

The scratch clone is under `/tmp`; no vendor tree is committed. Target and std are the
repo's own:

```
git clone --depth 1 https://github.com/uutils/coreutils /tmp/uutils
cp -a "$(rustc --print sysroot)/lib/rustlib/src/rust/library" /tmp/neo-rust-src/library
NEO_STD_LIBRARY=/tmp/neo-rust-src/library \
  bash userland/neoplutarch-std-overlay/apply-overlay.sh
```

`/tmp/uutils/.cargo/config.toml` sets `target = "neo-target.json"` (a copy of
`userland/x86_64-unknown-neoplutarch.json`), `build-std = ["std", "panic_abort"]`,
`build-std-features = ["compiler-builtins-mem"]`, and the static-link rustflags from
`userland/std-real-smoke/.cargo/config.toml`. Each utility then built with:

```
RUSTUP_TOOLCHAIN=nightly-2026-04-16 \
__CARGO_TESTS_ONLY_SRC_ROOT=/tmp/neo-rust-src/library \
RUSTC_BOOTSTRAP_SYNTHETIC_TARGET=1 \
cargo build -p uu_<name> -Zjson-target-spec
```

`RUSTC_BOOTSTRAP_SYNTHETIC_TARGET=1` is not optional. Without it every dependency fails:

```
error[E0658]: use of unstable library feature `restricted_std`
   --> rustc-hash-2.1.3/src/lib.rs:25:1
   = help: add `#![feature(restricted_std)]` to the crate attributes to enable
```

That is the same mechanism `xtask` uses for the Servo closure
(`tools/xtask/src/main.rs:7511-7525`); a vendored uutils tree will need the same treatment,
or `#![feature(restricted_std)]` in every crate (not practical for third-party sources).

Toolchain: `rustc 1.97.0-nightly (e8e4541ff 2026-04-15)`.

### 4.2 Results

| Utility | Result | First error |
|---|---|---|
| `echo` | **built** (26.4 MB) | — |
| `wc` | **built** (28.5 MB) | — |
| `rm` | exit 0, **no binary** | crate-level `#![cfg(any(unix, windows, wasi))]` |
| `cat` | error | `E0432` `crate::platform::is_safe_overwrite` (`cat.rs:10`) |
| `mkdir` | error | `E0432` `uucore::fs` (`mkdir.rs:20`) |
| `head` | error | `E0433` `uucore::fs` (`head.rs:396`) |
| `ls` | error | `hostname 0.4.2` `compile_error!` (`lib.rs:67`) |
| `cp` | error | `console 0.16.6` (23), then `filetime 0.2.29` (14) |
| `mv` | error | `console 0.16.6` (23) |
| `tail` | error | `platform::{ProcessChecker, Pid}` + `uucore::fs` (8) |

Representative exact errors:

```
error[E0433]: cannot find `fs` in `uucore`
   --> src/uu/head/src/head.rs:396:33
note: found an item that was configured out
  --> src/uucore/src/lib/lib.rs:65:26
   | #[cfg(all(feature = "fs", any(unix, windows, target_os = "wasi")))]
```

```
error: Unsupported target OS! Create an issue: https://github.com/djc/hostname/issues/new
  --> hostname-0.4.2/src/lib.rs:67:9
```

```
error[E0433]: cannot find `unix` in `os`
  --> filetime-0.2.29/src/unix/mod.rs:5:14
  = note: found an item that was configured out (std/src/os/mod.rs:36)
```

`rustix` is not built at all for this target (no `Compiling rustix` line in any log);
`libc 0.2.191` and `nix 0.31.3` compile, but as essentially empty crates because their
APIs are `cfg(unix)`. A green link therefore says nothing about POSIX behaviour.

### 4.3 The ELF is not loadable either

Both successful binaries have `Entry point address: 0x0`. They are normal `fn main`
Rust binaries; on this target there is no `crt1.o`/`_start` to set the entry, so the
linker leaves it at zero. Our loader jumps to `e_entry` and rejects an entry outside every
loaded segment (`kernel/src/loader/elf.rs:253-261`,
`ElfError::EntryNotInLoadedSegment`). For comparison, `std-real-smoke` (which is
`#![no_main]` with `#[unsafe(no_mangle)] extern "C" fn _start`) builds with
`e_entry = 0x219240`.

So a vendored uutils binary needs a `_start` shim that calls `uumain` directly (uutils'
`bin!` macro ends in `std::process::exit`, so the shim is short), or a `uucore` macro that
emits `#![no_main]` + `_start`. This is P1 in §5.

## 5. Work packages and the gap table

A **cycle** is one build → QEMU gate → diagnose loop on the pinned toolchain, including
the test that closes the rung. Ranges are uncertainty, not calendar time: a new syscall
touching three filesystems (`neofs`, `ext4`, `fat32`) is wider than a std arm.

| ID | Work package | Where | Cycles |
|---|---|---|---|
| **P0** | **argv ABI**: `SYS_SPAWN` variant carrying argv (or an argv handle), loader stack/handle setup, new `library/std/src/sys/args/neoplutarch.rs`, `uucore` unchanged | kernel + loader + std overlay | 3–5 |
| **P1** | **entry + allocator**: `#![no_main]` `_start` shim for a uutils bin, `neo-std-pal::SyscallHeap` as `#[global_allocator]`, `restricted_std` build plumbing | uutils vendor + `userland/neo-std-pal` | 1–2 |
| **P2** | **stdin**: `std::sys::stdio::neoplutarch::Stdin::read` over `SYS_READ(0)` (the kernel reserves fd 0 and routes it to serial/HID under `serial-stdin`/`usb-hid`); scenario feature `serial-stdin` | std overlay | 1–2 |
| **P3** | **readdir**: `std::fs::readdir`/`ReadDir`/`DirEntry` over `SYS_READDIR`, decoding `DirentV1`; `DirEntry::file_type` from the kind | std overlay (kernel exists) | 1–2 |
| **P4** | **stat**: new `SYS_STAT`/`SYS_FSTAT`; `Inode::stat`; neofs/ext4/fat32 impls; `FileAttr` (size, kind, mode/rights, nlink, ino, dev, times); `FileType`; `Permissions`; byte oracle | kernel VFS + 3 FS + std overlay | 4–6 |
| **P5** | **mkdir**: new `SYS_MKDIR`; VFS `create_dir`; neofs/ext4/fat32 | kernel | 2–3 |
| **P6** | **unlink/rmdir**: new syscalls; VFS `unlink`/`rmdir`; neofs/ext4/fat32 | kernel | 2–3 |
| **P7** | **rename**: new `SYS_RENAME` (two paths); VFS; neofs/ext4/fat32. `SYS_FILE_REPLACE` is content-only and cannot stand in | kernel | 2–3 |
| **P8** | **seek/truncate/append**: new `SYS_LSEEK` over the existing `OpenFile.offset`; wire `SYS_FTRUNCATE` into `File::truncate`; implement `OpenOptions` append/truncate/create_new | kernel (one call) + std overlay | 1–2 |
| **P9** | **links/perms/times**: `SYS_SYMLINK`, `SYS_LINK`, `SYS_READLINK`, `SYS_CHMOD` (or a rights-set call), `SYS_UTIMES`; std arms | kernel + std overlay | 3–5 |
| **P10** | **wall clock**: `SystemTime` arm over `CLOCK_REALTIME` (kernel already answers under `rtc-epoch`) | std overlay | 1 |
| **P11** | **uutils platform arm**: `target_os = "neoplutarch"` arms in `uucore` (`fs`, `pipes`, `entries`, `perms`, `safe-copy`, `safe-traversal`, `signals`) and per-crate arms (`cat` platform, `tail` platform, `rm` crate gate); dependency policy for `hostname`/`console`/`filetime` | vendored uutils | 3–5 |
| **P12** | **shell handoff**: pass argv + the shell's RootDirCap handle (`libneo::root_dir_cap()`, `libneo/src/lib.rs:1125-1137`; the shell already inherits one, `neo-init/src/main.rs:137-139`) + stdin to `sys_spawn` | `userland/neo-shell` | 1–2 |

The dispatch's table, with each utility's **incremental** cost from today's tree (shared
packages counted once in the order of §6; the column is not additive):

| Utility | Blocking gaps | PAL work each gap needs | Cycles |
|---|---|---|---|
| `echo` | no argv; no entry; leaky default allocator | **P0** argv; **P1** `_start` + `SyscallHeap`; no fs | 3–5 |
| `cat` | no stdin; `is_safe_overwrite` has no arm | **P2** stdin; **P11** one `not(any(unix,windows,wasi))` arm; file reads already work | 2 |
| `wc` | no argv; no stdin | **P0/P2**; `util_name()` abort fixed by P0; builds today | 1–2 |
| `head` | no stdin; `uucore::fs` absent | **P2**; **P11** `fs` arm (or drop the `sane_blksize` call); **P8** only for `-c -N` | 2 |
| `mkdir` | no mkdir syscall; `uucore::fs` absent | **P5** mkdir; **P11** `fs` arm; mode bits deferred (§7.4) | 3–4 |
| `ls` | readdir absent in std; **`-l`** needs stat/time; `hostname` dep | **P3** readdir; **P4** stat; **P10** SystemTime; **P11** hostname policy. Bare `ls` = P3 only | bare 2–3; `-l` +5–7 |
| `rm` | no unlink/rmdir; crate compiled out; safe-traversal absent | **P6** unlink/rmdir; **P11** crate gate + `fs`/`safe_traversal` arms; **P4** for `-i`/mode prompts | 4–6 |
| `mv` | no rename; console dep; links absent | **P7** rename; **P11** console policy + `fs` arm; **P9** for `-s`/`-l`; **P4** optional | 4–6 (same-FS); +3–5 cross-FS |
| `cp` | no stat/seek/truncate; filetime + console deps; perms/times/links absent | **P4** stat; **P8** seek/truncate/append; **P9** links/perms/times; **P11** filetime/console + `fs`/`perms`/`safe-copy` arms | 8–12 |
| `tail` | no stdin/seek/metadata; platform arms absent | **P2**; **P8** seek; **P4** metadata for `-f`; **P11** `fs` + `platform` arms; `notify` already falls back to `PollWatcher` | 4–6 |

## 6. Recommended order and the first utility

The order is by dependency depth, not by the shell's builtin set:

1. **P0 + P1 + P12** — argv, `_start`, allocator, shell handoff. Shared with L1c; without
   them no utility can be tested end to end. (≈5–8 cycles.)
2. **`echo`** — the plumbing gate (below). (1 cycle after step 1.)
3. **`cat`** — first file utility; proves `File::open` + read through a `RootDirCap`, and
   the stdin path.
4. **`wc`** — first utility that adds a capability the shell does not have; already builds.
5. **`head`** — same stdin/read set, plus `seek` only for the negative-count forms.
6. **`mkdir`** — first mutation syscall (P5).
7. **`ls`** — bare first (P3), then `-l` once P4/P10 land.
8. **`rm`** — P6, with the crate gate and safe-traversal arm (P11).
9. **`mv`** — P7; same-FS first, cross-FS later on `cp`+`rm`.
10. **`cp`** — last of the mutation tools because it needs P4+P8+P9 together.
11. **`tail`** — read+seek is cheap, but `-f` wants P4 and a poll-watcher story; schedule
    it with P8, not before.

Steps 1–5 (through `head`) are ≈10–13 cycles from today. The whole ten are roughly
**45–60 cycles**, with P4 (stat) and P9 (links/perms/times) the two packages most likely to
run long. P0 and P4–P9 are kernel-touching, so they carry the 4-shard QEMU gate and the
AAG; P1–P3, P10–P11 and P12 are std-overlay/host/shell work and can run in parallel with a
kernel package.

**First utility to land: `echo`.** It is not the most useful tool — the shell already has
an `echo` builtin — but it is the smallest end-to-end proof of spawn → argv → stdout →
exit, and it catches the silent-empty-argv defect with a byte-exact frame.

**Gate scenario `coreutils_echo`** (QEMU, xtask scenario; stages `/modules/echo` like the
std-real modules at `tools/xtask/src/main.rs:6456-6470`):

1. Spawn `echo` with argv `["echo", "-n", "hello"]`; the console frame must be exactly
   `hello` — no trailing newline, exit 0.
2. Spawn with `["echo", "a", "b"]`; frame exactly `a b\n`, exit 0.
3. Spawn with `["echo", "--help"]`; frame contains `Usage: echo`, exit 0.
4. Spawn with `["echo"]`; frame exactly `\n`, exit 0. (This alone does not prove argv;
   step 1 does, because the missing newline proves `-n` was parsed.)
5. **Negative:** spawn with the pre-P0 shape (no argv). It must fail **loudly** with the
   named unsupported-args message and a non-zero exit, not print `\n`. This is the
   "unsupported calls are listed, not stubbed silently" discriminator, and it is the test
   that would have caught the current behaviour.

Byte oracle: the frame is compared byte-for-byte, so `hello` vs `hello\n` is a real
assertion. The scenario needs `desktop-session`/`serial-stdin` features only if the shell
is in the loop; a direct kernel spawn with argv keeps it to the coreutils scenario.

After `echo`, the next gate should be `cat` with a handed-in `RootDirCap` and a file whose
content is known (`cat /hello.txt` → exactly its bytes), because that is the first test of
the fs authority handoff, not just argv.

## 7. Cross-cutting findings and traps

### 7.1 The argv failure is silent, and the overlay's fallback audit does not catch it

`apply-overlay.py:346` lists `args/mod.rs` among the "all loud unsupported" fallbacks, but
the module it routes to returns an empty iterator (`args/unsupported.rs:6,20-21`). A
utility that gets zero arguments can exit 0 with wrong output (`echo`), or print an error
about a missing operand. The target's own rule — every unsupported surface fails
deterministically and loudly (`neo-std-pal/src/lib.rs:274-292`) — is violated at the std
layer. Fix this *before* any utility lands: either implement P0 or add a
`sys/args/neoplutarch.rs` that aborts with a named message. The overlay's audit should
assert behaviour, not the `mod unsupported` spelling.

### 7.2 `ARGV[0]` is a panic on the error path

`UTIL_NAME` (`uucore/src/lib/lib.rs:394-396`) indexes `ARGV[base_index]`; with an empty
`ARGV` that is an index panic, i.e. abort 134 under `panic=abort`. `cp`, `mv`, `wc` and
`tail` call `util_name()` in `uu_app()`, so they die before anything useful. `rm` also
reaches it on its prompt/error paths. `ls`, `cat`, `mkdir`, `head` and `echo` hard-code
`Command::new("...")`, so they do not panic at startup; they just see zero operands. P0
removes the whole class; until then, do not read a 134 from a coreutils module as "the
utility ran and aborted".

### 7.3 The cfg model is binary; pick an arm, do not flip `unix`

Adding `"target-family": ["unix"]` to the target JSON would make `cfg(unix)` true and pull
in uutils' unix code, `std::os::unix`, and the real `libc`/`rustix`/`nix` surfaces — which
this target does not have. That trades a bounded set of `neoplutarch` arms for an unbounded
POSIX-compatibility claim. The recommended shape is a `neoplutarch` arm per module (P11),
modelled on the existing `not(unix)`/WASI arms in `uucore::fs` and `mkdir`. The target spec
should stay non-unix.

### 7.4 There is no POSIX mode bit, and that is a semantic decision

`ls -l`, `cp -p`, `mv` and `mkdir -m` assume a nine-bit mode, an owner uid/gid, an inode
number, a link count and an mtime. Our VFS has *rights* (`FileRights`: read, write, lookup,
readdir, execute; `kernel/src/fs/mod.rs:47-95`) and a kind, not those fields. Options:

- **(a) Synthesize a mode from rights** (e.g. owner rwx from read/write/execute, no group
  or other bits) and report uid/gid 0, nlink 1, ino 0. Cheap; `ls -l` becomes readable and
  `cp -p` is a no-op with a warning. This is the recommended v1.
- **(b) Store mode/owner/times on each inode** (a VFS/FS change across neofs, ext4, fat32).
  Honest POSIX semantics, but it contradicts the capability model and is much larger.
- **(c) Refuse the columns**: `ls -l` prints an error, `cp -p` fails loudly, `mkdir -m` is
  rejected. Cheapest, but `ls -l` is the form most people mean by `ls`.

Whichever is chosen, it must be a *named* decision in the build that adds stat, not a
silent mapping. This is the single biggest open design question in L1d.

### 7.5 Files are cap-rooted, not ambient

`File::open` refuses before the syscall unless the process holds a `RootDirCap`
(`fs_neoplutarch.rs:240`), and `SYS_OPEN`'s root form resolves through that cap. The shell
already inherits a handle (`neo-init/src/main.rs:137-139`) and can locate its RootDirCap
with `libneo::root_dir_cap()` (`libneo/src/lib.rs:1125-1137`), but `spawn_child` passes only
a channel (`neo-shell/src/main.rs:1156-1170`). Every external utility will start with no
file authority until P12 hands it the cap. That is a feature, not a bug — but it means a
utility's "current directory" is the cap, `..` cannot escape it, and `env::current_dir()`
returns `Unsupported` (there is no ambient cwd). Path normalisation must live in the
utility (`uucore::fs::normalize_path`, `dir_strip_dot_for_creation`) or in a small PAL
shim; do not expect `std::env::current_dir` to work.

### 7.6 The default allocator leaks

uutils does not depend on `neo-std-pal`, so it gets std's own `System` allocator, whose
`dealloc` is a no-op (`alloc_neoplutarch.rs:26-45`): one `SYS_MAP_REGION` per allocation,
never freed. `echo` will not notice; `ls` of a large directory, `cp` of a large file and
`wc` over many files will. P1 installs `SyscallHeap` (a real linked-list allocator,
`neo-std-pal/src/lib.rs:150-271`) as the bin's `#[global_allocator]`. This is cheap and
should be in the same build as the `_start` shim.

### 7.7 Do not mix the crates.io `uu_*` 0.12.0 with the 0.13.0 workspace

The registry crates lag the surveyed head by a minor version and carry their own `uucore`
dependency graph. A per-crate `uu_<name> = "0.12"` dependency would also make the uutils
patch (P11) a patch to a published crate, not to the tree under review. Vendor the
workspace at `cac8be1` under `third_party/uutils` with the `VENDORED.md` convention
(`third_party/<name>/VENDORED.md`, as `third_party/fatfs` and `third_party/miniz_oxide`
do), record the SHA and MIT licence, and take the per-utility crates by path. That keeps
one lockfile, one licence, and one place for the `neoplutarch` arms.

### 7.8 Kernel features are scenario-gated

`SYS_CLOCK_GETTIME` is compiled only for `browser-runtime`, `atrium-pointer-forward` or
`desktop-session` (`syscall.rs:7233-7238`), and `sys_read_stdin` only for `serial-stdin`
or `usb-hid` (`syscall.rs:6532`). A coreutils scenario that does not select one of those
gets `ENOSYS` from `Instant::now()` (a panic in the overlay) or from stdin. Name the
features in the scenario; do not let a green build imply the syscalls are present.

## 8. Decisions for J.c.

1. **Vendor the workspace or the registry crates?** Recommend the workspace at
   `cac8be1` under `third_party/uutils` (MIT, `VENDORED.md`, SHA pinned). The registry is
   a minor behind and splits the patch surface.
2. **`neoplutarch` arms in uucore, or a target-family change?** Recommend arms (P11); a
   `unix` target family would claim a POSIX surface the OS does not have.
3. **What is a mode bit?** §7.4(a) synthesize from rights for v1, (b) store real metadata
   later, or (c) refuse the columns. This gates `ls -l`, `cp -p`, `mv -p` and `mkdir -m`.
4. **Which authority does the shell hand a tool?** `cat`/`ls` need read; `cp`/`mkdir`/`rm`
   need write; `rm` is irreversible. The current model hands the shell's cap, or nothing
   at all. A per-tool cap (derived read-only for `ls`/`cat`, read-write for the mutators)
   is the capability-correct answer, and it is a shell policy decision, not a PAL one.
5. **Is `echo` worth landing first when the shell has a builtin?** Recommend yes as the
   plumbing gate; the first *capability* utility is `cat`, then `wc`. If the PL would
   rather not spend a cycle on a duplicate, start at `cat` and use a direct spawn as the
   argv smoke.
6. **How far does `tail -f` go?** The poll watcher is a fallback, not inotify. If `-f` is
   in scope, it needs a polling cadence and a stop condition; if not, land `tail -n` and
   refuse `-f` loudly.
