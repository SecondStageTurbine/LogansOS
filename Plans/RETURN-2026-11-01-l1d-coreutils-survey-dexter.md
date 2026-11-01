<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 239, L1d survey: uutils coreutils on LogansOS

**Seat:** Dexter. **Answers:** `Plans/DISPATCH-2026-11-01-l1d-coreutils-survey-dexter.md`, Relay 239.
**Branch:** `dexter/l1d-coreutils-survey`. **Base:** `origin/main` @ `e9330a22` (ff-clean at close).
**Head (the survey artifact; this RETURN's parent):** `a18955f1`.
**Kernel-touching:** no — this relay changes no code at all. The deliverable is one THINK.

## Delivered

| Item | Path | Notes |
|---|---|---|
| Survey | `docs/THINK-2026-11-01-l1d-coreutils-on-logansos.md` | Answers the dispatch's five points: upstream re-check, std/libc/nix/rustix call map vs the PAL, target build attempt with exact errors, the gap table with cycle estimates, and the recommended order + first utility + gate scenario |

No vendored crates were committed. The uutils clone and the overlaid `rust-src` copy live
under `/tmp` only. The build logs are `/tmp/buildlogs/<utility>.log` on this box.

## Upstream record (re-checked on the day)

- Repository `https://github.com/uutils/coreutils`
- Head **`cac8be1f17667aa337a3c8951fc77d98ece9d0b5`**, dated **2026-10-31 19:59:12 +0100**
  (`mknod: support non-UTF8 paths`)
- Licence **MIT** (`LICENSE`; workspace `Cargo.toml` `license = "MIT"`, per-utility crates
  inherit it)
- Workspace `0.13.0`, edition 2024, MSRV 1.88.0; per-utility packages are `uu_<name>`
- crates.io carries `uu_<name>` at **0.12.0** — one minor behind. The THINK recommends
  vendoring the workspace at `cac8be1`, not the registry crates.

The 2026-10-31 survey row recorded the same head: nothing moved upstream between that
survey and this relay.

## The answer in brief

1. **The lane is blocked before the first utility by argv.** `std::env::args` on
   `x86_64-unknown-neoplutarch` falls to std's `unsupported` arm, which returns an **empty
   iterator** (`library/std/src/sys/args/mod.rs:57-58`,
   `library/std/src/sys/args/unsupported.rs:6,20-21`). `uucore` indexes `ARGV[0]`
   (`src/uucore/src/lib/lib.rs:392-395`), so `cp`, `mv`, `wc` and `tail` abort 134 at
   startup; `echo`, `ls`, `cat`, `mkdir` and `head` silently see zero operands. `echo` in
   that state prints a bare newline and exits 0. `SYS_SPAWN` has no argv argument
   (`kernel/src/arch/x86_64/syscall.rs:1838`), so this is an ABI build (P0 in the THINK).
2. **No utility ELF built today is loadable.** `echo` and `wc` build, but both have
   `e_entry = 0x0` because there is no `_start`; the loader rejects an entry outside every
   loaded segment (`kernel/src/loader/elf.rs:253-261`). `std-real-smoke` (with `_start`)
   builds at `0x219240`, which is the control. A `_start` shim + `SyscallHeap` is P1.
3. **uutils' cfg model is `unix`/`windows`/WASI, and this target is none of them.** uucore's
   `fs`, `pipes`, `entries`, `perms`, `safe-copy`, `safe-traversal` and `signals` modules
   are compiled out (`src/uucore/src/lib/lib.rs:65-66,112-150`). `mkdir`/`head` fail on
   `uucore::fs`; `rm` is worse — its crate root is
   `#![cfg(any(unix, windows, target_os = "wasi"))]` (`rm.rs:8`), so `cargo build -p uu_rm`
   exits 0 and produces **no binary**. Three third-party crates also fail:
   `hostname 0.4.2` (`compile_error!`), `console 0.16.6` (via `indicatif`) and
   `filetime 0.2.29`. `libc` and `nix` compile but are empty on this target; `rustix` is
   not built at all.
4. **The fs surface is open/read/write/sync/close only.** `std::fs::{metadata, stat, lstat,
   read_dir, create_dir, remove_file, rename, seek, truncate, canonicalize, symlink,
   hard_link, read_link, set_permissions, set_times}` are all `unsupported`
   (`userland/neoplutarch-std-overlay/std/fs_neoplutarch.rs`, lines cited in THINK §2).
   The kernel already has `SYS_READDIR` and `SYS_FTRUNCATE`; it lacks `stat`, `mkdir`,
   `unlink`, `rename` and `lseek`. `SystemTime` is also unexposed even though the kernel
   answers `CLOCK_REALTIME` under `rtc-epoch`.
5. **There is no POSIX mode bit.** The VFS has capability rights, not mode/uid/gid/nlink/
   inode/mtime. `ls -l`, `cp -p` and `mkdir -m` need a named decision: synthesize a mode
   from rights (recommended v1), store real metadata, or refuse the columns. This is THINK
   §7.4 and the biggest open design question in the lane.

## Build results (target `x86_64-unknown-neoplutarch`, pinned `nightly-2026-04-16`)

Scratch clone at `/tmp/uutils`, target JSON copied from
`userland/x86_64-unknown-neoplutarch.json`, std built with `-Z build-std=std,panic_abort`
from the repo's overlay (`__CARGO_TESTS_ONLY_SRC_ROOT`), and
`RUSTC_BOOTSTRAP_SYNTHETIC_TARGET=1` (without it, every dependency fails with
`E0658 restricted_std`).

| Utility | Result | First error |
|---|---|---|
| `echo` | **built** (26.4 MB), `e_entry 0` | — |
| `wc` | **built** (28.5 MB), `e_entry 0` | — |
| `rm` | exit 0, **no binary** | crate-level `#![cfg(any(unix, windows, wasi))]` |
| `cat` | error | `E0432` `crate::platform::is_safe_overwrite` (`cat.rs:10`) |
| `mkdir` | error | `E0432` `uucore::fs` (`mkdir.rs:20`) |
| `head` | error | `E0433` `uucore::fs` (`head.rs:396`) |
| `ls` | error | `hostname 0.4.2` `compile_error!` (`lib.rs:67`) |
| `cp` | error | `console 0.16.6` (23 errors), then `filetime 0.2.29` (14) |
| `mv` | error | `console 0.16.6` (23 errors) |
| `tail` | error | `platform::{ProcessChecker, Pid}` + `uucore::fs` (8 errors) |

## Work packages and order

The THINK defines P0–P12 (argv; entry/allocator; stdin; readdir; stat; mkdir; unlink/rmdir;
rename; seek/truncate/append; links/perms/times; `SystemTime`; the uutils `neoplutarch`
platform arm; the shell handoff), with cycle estimates that exclude calendar time.

Recommended order: **P0 + P1 + P12 → `echo` → `cat` → `wc` → `head` → `mkdir` → `ls`
(bare, then `-l`) → `rm` → `mv` → `cp` → `tail`**. Steps 1–5 (through `head`) are ≈10–13
cycles from today; the whole ten are roughly 45–60 cycles, with stat (P4) and
links/perms/times (P9) the long poles.

**First utility: `echo`**, as a plumbing gate, not a capability win (the shell already has
an `echo` builtin). Gate `coreutils_echo`: spawn `/modules/echo` with
`["echo","-n","hello"]` → console frame **exactly `hello`**, exit 0; `["echo","a","b"]` →
`a b\n`; `["echo","--help"]` → contains `Usage: echo`; and a negative spawn with no argv
must fail **loudly**, not print `\n`. The first capability utility after it is `cat`
(`cat /hello.txt` through a handed-in `RootDirCap`), then `wc`.

## Verification

| Check | Result |
|---|---|
| `git fetch origin` + branch base | base `e9330a22` == `origin/main` tip — ff-clean |
| Diff vs `origin/main` | two files: the THINK and this RETURN; no code |
| Code changes | none — no `kernel/`, `tools/`, `userland/`, `Cargo.*` touched |
| SPDX | THINK and RETURN both start with `<!-- SPDX-License-Identifier: CC-BY-4.0 -->` |
| Build attempt | ten `cargo build -p uu_<name> -Zjson-target-spec` runs; logs at `/tmp/buildlogs/` on this box; scratch clone and overlaid rust-src not committed |
| `worker-close-gate.sh` | not run: there is no code to fmt/test/clippy; Step 1's rebase check was done manually (above). CI is path-ignored for `docs/**` and `Plans/**`. |

```
Branch:        dexter/l1d-coreutils-survey @ a18955f1 (THINK)
Base:         origin/main @ e9330a22  (ff-clean)
Kernel-touching: no
Push CI:       not applicable (docs/Plans only; path-ignored)
Self-stat:     THINK commit 1 file, 570 insertions(+); this RETURN adds 1 file
```

## Notes

1. **No CI URL.** The branch touches only `docs/` and `Plans/`, which `ci.yml` path-ignores;
   there is no run to cite, and no green-CI claim is made.
2. **RAG.** Not available on this box, as the dispatch says; used `grep`, direct reads and
   the upstream clone.
3. **No personal details.** The THINK and this RETURN contain no local paths, hostnames,
   addresses or emails; the build commands use `/tmp` and repo-relative paths only.
4. **Disk reclaim.** After the PM merges: `cargo clean` in `~/workers/dexter` if desired;
   do not delete the worktree. The `/tmp/uutils` clone is disposable.
