<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 365: #87's three commits after relay 361 (generator, leak check, glue_commands)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:**
`Plans/DISPATCH-2026-11-08-relays-359-362.md` §365.

**Reviewed:** PR #87, branch `claude/ot61-c3c1-mozjs-vendor`, at the dispatched head
`3b85ae5019002e359cb25942f1fb328ee0838945` ("OT-61 C3c-1: the frozen glue_commands.json (run 42541535589)").
The range is `bbdd374c1..3b85ae50`: `git diff --stat bbdd374c 3b85ae50 -- . ':!third_party/ot61-mozjs/generated'`
is four files (`ot61-mozjs-generate.yml`, `VENDORED.md`, `generator/freeze.py`, `generator/neo-cc`), and the
excluded fifth file is `generated/glue_commands.json`. Merge-base `e6b3d665cb509c45b63a0fdce0f53e737f62fe8b`;
39 commits from there. `origin/main` at review time is `145059d5b5fde15fa8790c88bea15c532371b190`.

**Overall: PASS, all three items** at the dispatched head. The commit after the dispatched head
(`2761bb71b`, the product build's patched crates) does not touch `freeze.py`, `neo-cc` or the glue step; its
generator run is green too (below).

| # | Dispatch item | Verdict | Enforcing refs (at `3b85ae50`) |
|---|---|---|---|
| 1 | Generator still runs network-off, on the pinned toolchain; drift check now covers `glue_commands.json` | **PASS** | `ot61-mozjs-generate.yml:249` (`unshare -n`), `:260` (`NEO_CC_LOG`), `:291` (`diff -r`); `generator/freeze.py:249-285`; `generated/glue_commands.json` (929 bytes) |
| 2 | `neo-cc` and the stricter leak check can't let a host path or host header into the frozen output | **PASS** (one residual, §2.3) | `generator/neo-cc:42-45` vs `:51,55-64`; `generator/freeze.py:63-79,249-283,315-331` |
| 3 | The three relay-361 items still hold | **PASS** | `VENDORED.md:129` (cut-list), `:150-151` (dtoa); pin at `ot61-mozjs-generate.yml:38,63-75`; `rust-toolchain.toml:10`; `generated/` tree |

## 1. Network off, pinned toolchain, drift check covers `glue_commands.json` — PASS

**Network off.** The new recording runs inside the already-network-off bindgen step, not beside it:
`NEO_CC_LOG="$obj/glue-cc.log"` is one of the `env` assignments passed to
`sudo unshare -n -- sudo -u "$(id -un)" env` (`ot61-mozjs-generate.yml:249-262`; the assignment is at `:260`).
The step also deletes a stale log first (`:247`). The earlier generation steps are unchanged: configure `:94`,
make export `:124`, the neoplutarch configure/export `:148`, the compile probe `:199`. The bindgen *tool* build
(`cargo build`, `:238`) still uses crates.io before the unshare, exactly as its comment says; every step that
produces the frozen set still runs with no network.

**Pinned toolchain.** The pin is untouched by this range. `NEO_CLANG_MAJOR: "19"` (`:38`), the Dockerfile
assertion and `clang-19`/`lld-19`/`llvm-19`/`libclang1-19` install (`:63-75`), libclang from
`"$NEO_CLANG" -print-resource-dir` (`:243`), `NEO_CLANG` passed into the bindgen env (`:251`), and the CXX is
`generator/neo-c++`, which resolves `clang=${NEO_CLANG:-clang}` (`neo-cc:20`). Rust is still
`nightly-2026-04-16` (`rust-toolchain.toml:10`) and cbindgen `0.29.4 --locked` (`ot61-mozjs-generate.yml:317`).
`git diff bbdd374c 3b85ae50 -- .github/workflows/ot61-mozjs-generate.yml` contains only the stale-log `rm` and
the `NEO_CC_LOG` assignment, so nothing in the pin moved.

**Drift check covers `glue_commands.json`.** `freeze.py` writes it when `obj/glue-cc.log` exists
(`freeze.py:249-285`), and the drift step compares the *whole* frozen directory with the committed one using
`diff -r "$committed" "$frozen"` (`ot61-mozjs-generate.yml:291`), so a missing, extra or changed
`generated/glue_commands.json` fails the job. Two runs make that concrete:

- run [42541535589](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42541535589) at `3fc087fb8`
  (the generator commit, before the file was committed): steps through bindgen are green, and
  `freeze the outputs, and check them against the committed copy` **fails**. The only delta between that
  commit and the next green one that can affect the freeze is the added `generated/glue_commands.json` (the
  other change is a `VENDORED.md` prose row), so the failure is the drift check seeing the new file the
  committed copy lacked; it is the coverage working, not an infra red. (The job log itself is not readable
  without repo admin rights, so this is the step result plus the one-file delta.)
- run [42542169317](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42542169317) at the reviewed
  head `3b85ae50`: same step **success**, with `generated/glue_commands.json` committed. So the committed copy
  reproduces from a fresh freeze, byte for byte, on the pinned clang 19.

The committed file is present in the tree (`git cat-file -s 3b85ae50:.../generated/glue_commands.json` = 929
bytes) and holds exactly the two expected compiles (`jsapi.cpp`, `jsglue.cpp`). If the log were ever absent,
freeze would write no glue file and the same `diff -r` would fail on the committed one, so the path is
fail-closed.

## 2. No host path or host header in the frozen output — PASS

### 2.1 `neo-cc` logs only the caller's arguments, before its own host headers

The log block is at `generator/neo-cc:42-45`: it records `os.getcwd()` and `"$@"`, then exits the block. The
wrapper's own flags are added *after* that point: `set -- --target=...` at `:51`, and the `inc=` string at
`:55-64`, which is where the host paths live — `$res/include` (clang's resource headers, `:59`),
`$crt/musl*`, `$crt/libcxx/include`, `${NEO_CXX_CONFIG}` and `$here/include`. Because `"$@"` is captured before
all of those, the wrapper's host header directories can never enter the log. That is the specific "host header"
hole and it is closed by ordering, not by filtering.

The caller's arguments themselves are the ones `mozjs-sys`'s `get_common_cc` builds
(`vendor/.../mozjs-sys/build.rs:548`): `-include` of `$build_dir/js/src/js-confdefs.h`, include paths from
`target.include_paths($build_dir)`, `-DSTATIC_JS_API`, and the source file. In the generator those are all
under `$obj` (the crate root's `OUT_DIR`) plus the source under the patched copy. They are not host system
paths.

### 2.2 `freeze.py` rejects every path it can see and fails on the rest

For the glue file (`freeze.py:249-283`) it deliberately drops the log's `directory` field and emits only
`{"cxx", "file", "arguments"}` (`:279`). Every absolute argument, every `./...` argument and every `-I`-joined
path is passed through `token()` (`:262-278`), which replaces `@SRC@` / `@OBJ@` / `@REPO@` / `@CRATES@`
(`:63-79`; `@CRATES@` is new in `3fc087fb8`, and `src` is longer so it is matched first). Entries whose source
is not `src/jsapi.cpp` or `src/jsglue.cpp` are dropped, which removes cc-rs's flag probes, and the code
`sys.exit`s unless the result is exactly the two files (`:280-281`).

Then the whole frozen output is scanned (`:315-331`): if any host root, `/home/runner/` or `/tmp/` survives in
any file, it prints `an absolute path survived in ...` and exits 1 (`:331`). This runs after the patched files
and generated files are written, so it covers `glue_commands.json` too.

**Checked against the committed result.** `generated/glue_commands.json` at `3b85ae50` contains only
`@OBJ@/dist/include`, `@OBJ@/js/src`, `@OBJ@/js/src/js-confdefs.h` and
`@CRATES@/mozjs-sys/src/{jsapi,jsglue}.cpp`; `compile_commands.json` (167 entries) has no value starting
`/home`, `/usr`, `/opt`, `/tmp` or `/github`.

**Mechanism test.** I ran `freeze.py` from the head against a synthetic `obj` (outside the repo) whose
`glue-cc.log` named host paths: the output tokenised them to `@OBJ@`/`@CRATES@`, dropped the log's
`directory`, dropped a probe compile, and exited 0. Replacing one argument with `/home/runner/...` made it
exit 1 (`an absolute path survived`).

### 2.3 Residual: the leak scan is a prefix deny-list, not a general absolute-path oracle

Stated plainly because it is a real limit of the check: a synthetic absolute path outside the four roots and
outside `/home/runner/` or `/tmp/` (I used `/opt/host/llvm/include`) is left in the output and the scan exits
**0**. So the scan is a backstop for the runner's temp/workspace paths, not a proof that "no absolute path can
survive". On the actual generator input no such path is reachable: `get_common_cc`'s absolute paths are `$obj`
and the patched crate root, and the wrapper's `$res`/neo-crt includes are added after the log line (§2.1). I
record it as a residual, not a FAIL, because the frozen file at the head was produced by that real input and
contains no host path; if the PM wants the stronger property, the check would have to flag *any* absolute path
outside the tokens, not extend the prefix list.

## 3. The three relay-361 items still hold — PASS

| Relay-361 item | Still true at `3b85ae50` |
|---|---|
| Three GPL-only files cut and listed | `git ls-tree -r --name-only 3b85ae50` matches **0** paths for `third_party/python/yamllint/`, `js/src/util/make_unicode.py`, `js/src/devtools/rootAnalysis/run_complete` (same at `2761bb71b`). The cut-list row is now `VENDORED.md:129` and still names all three (plus `pylru`, `tooltool.py`, `autocommand`, the ICU autoconf files and the two LGPL PGO pages). |
| Generator toolchain pinned | Unchanged by this range: `ot61-mozjs-generate.yml:38`, `:63-75`, `:243`, `:251`; `rust-toolchain.toml:10`; cbindgen `:317`. The range's only workflow edits are `:247` and `:260` (glue log). |
| `dtoa.c` under the dtoa / Lucent notice | `VENDORED.md:150-151` (line numbers shifted +3 by the `glue_commands.json` bullet, content unchanged); `js/src/dtoa.c:1-18` is the David M. Gay / Lucent notice; `js/src/util/DoubleToString.cpp:70` is `#include "dtoa.c"`; `CLAUDE.md:917` on `origin/main` still carries J.c.'s 2026-11-08 allowance. |

## Notes on the branch, not item verdicts

- **The PR head has moved past the dispatch.** `refs/pull/87/head` is now `2761bb71bd3cb40b993a12a746e3fd5029e6c34`
  ("the product build's patched crates"), one commit after `3b85ae50`. It adds
  `generator/check_patched_crates.py` and the `patched/` crate manifests/build.rs; its workflow diff adds one
  step (`patched crate copies against upstream`) and does not touch `freeze.py`, `neo-cc` or the glue/freeze
  steps. Run [42543689711](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42543689711) at that
  head is **success**, and the three relay-361 facts above also hold there (0 GPL-only matches; `VENDORED.md`
  dtoa rows at `:163-164`).
- The green run at the reviewed head is
  [42542169317](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42542169317): every step success,
  including `bindgen for jsapi and jsglue (network off)`, the freeze drift check, and the cbindgen header check.
- `VENDORED.md:114-116` says the frozen copy is "from run 42541535589 at `3fc087fb`". That run is the one that
  emitted the frozen set on drift (it failed only because the file was not yet committed); the commit that
  adds the file is `3b85ae50`, and its green run is what proves the bytes reproduce. The prose is terse, not
  wrong.

## What I did not check

- I did not re-run the generator in CI; the run evidence above is GitHub's step results, and the mechanism
  test in §2 was on a synthetic `obj`, not the full toolchain.
- I did not audit `2761bb71b`'s patched crates or `check_patched_crates.py`; that commit is outside the
  dispatched range and outside these three items.
- I did not re-check the original #87 items outside §361/§365 (network-off design, secrets/writes, product
  image), and I did not re-run relay 354's full vendor hash comparison.
- I did not verify the `frozen.tgz` sha256 in `VENDORED.md:116` against an artifact (no authenticated log/
  artifact access); the green drift check at `3b85ae50` is the stronger evidence that the committed tree
  reproduces.
