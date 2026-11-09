<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 361: #87 re-check (GPL cut, pinned toolchain, dtoa record)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:**
`Plans/DISPATCH-2026-11-08-relays-359-362.md` §361.

**Reviewed:** PR #87, branch `claude/ot61-c3c1-mozjs-vendor`, at the **latest head**
`bc1b154633a9a84303fdfc9bce8b9ca195f5e65a` ("OT-61 C3c-1 generator: record jsapi/jsglue's compiles
(glue_commands.json)"). The relay named `4f7c4e75`; the dispatch moved the head to `bbdd374c1` and says to check the
latest head. `bc1b15463` is one commit past `bbdd374c1` and three past `4f7c4e75`. Merge-base
`e6b3d665cb509c45b63a0fdce0f53e737f62fe8b`; 46 commits, 10,064 files, +3,897,849. `origin/main` at review time is
`1fb76e0140dc3d977827b15660b0d522c2a3220c`.

**Overall: PASS, all three items** at the latest head. The pin and the `VENDORED.md` rows are identical to the
dispatched head: `git diff bbdd374c1 bc1b15463 -- .github/workflows/ot61-mozjs-generate.yml
third_party/ot61-mozjs/VENDORED.md` touches only the `glue-cc.log` lines.

| # | Dispatch item | Verdict | Enforcing refs (at `bc1b15463`) |
|---|---|---|---|
| 1 | The three GPL-only files are cut, and listed in the cut-list | **PASS** | `third_party/ot61-mozjs/VENDORED.md:126`; `.../third_party/python/yamllint/` (0 files); `.../js/src/util/make_unicode.py` (absent); `.../js/src/devtools/rootAnalysis/run_complete` (absent) |
| 2 | The generator's toolchain is pinned | **PASS** (one residual, §2) | `ot61-mozjs-generate.yml:38,64-65,71,73-75`; the `$NEO_LLVM_BIN` uses at `:96,151,213,243-244,259`; `rust-toolchain.toml:10`; `ot61-mozjs-generate.yml:317` |
| 3 | `dtoa.c` is recorded under the dtoa / Lucent notice, which J.c. allowed 2026-11-08 | **PASS** | `VENDORED.md:147-148`; `CLAUDE.md:917` (on `main`); `.../js/src/dtoa.c:1-18`; `.../js/src/util/DoubleToString.cpp:70` |

## 1. The three GPL-only files are cut, and listed — PASS

All three are absent from the vendored tree at the latest head, under
`third_party/ot61-mozjs/vendor/mozjs-138c9470/mozjs-sys/mozjs/`:

| Path | Files at head | Notes |
|---|---|---|
| `third_party/python/yamllint/` | **0** | the whole GPL-3.0 package is gone |
| `js/src/util/make_unicode.py` | **absent** | GPL-3.0-or-later header at the old copy |
| `js/src/devtools/rootAnalysis/run_complete` | **absent** | GPL-3.0-or-later (Sixgill, Stanford) |

A tree-wide `git ls-tree -r --name-only refs/pr/87` search for `yamllint/`, `make_unicode.py` and `run_complete`
returns nothing, so there is no second copy elsewhere in the tree.

They are in the recorded cut-list at **`third_party/ot61-mozjs/VENDORED.md:126`**: the row names
`third_party/python/yamllint` (GPL-3.0), `js/src/util/make_unicode.py` and
`js/src/devtools/rootAnalysis/run_complete` (GPL-3.0), alongside `pylru` (GPL-2.0+), `tooltool.py` (GPL-2.0) and
setuptools' `_vendor/autocommand` (LGPL-3.0). The deletion commit is `90a4decdc` ("drop the GPL/LGPL files from
the vendored SpiderMonkey tree", 59 files deleted); `bbdd374c1` adds the two LGPL-2.1 PGO pages to the same row.

The cut did not break the generator: run
[42536660879](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42536660879) at `bbdd374c1`, which is
after both cut commits, is green through `freeze the outputs, and check them against the committed copy` and
`normalizer_glue.h against cbindgen`.

Not one of the three, so not folded into the verdict: `third_party/python/setuptools/setuptools/_vendor/autocommand-2.2.2.dist-info/` still holds six metadata files (its `LICENSE` and `METADATA` are the LGPLv3 text)
although the code directory is gone. It is metadata/licence text, not code; if the PM wants zero LGPL text in the
tree, that dist-info is the remaining crumb.

## 2. The generator's toolchain is pinned — PASS, with one residual

The compiler/LLVM side that shapes the frozen set is pinned, checked against the Dockerfile, and used everywhere:

| Pin | Line |
|---|---|
| `NEO_CLANG_MAJOR: "19"` | `ot61-mozjs-generate.yml:38` |
| asserts `Dockerfile` has `ARG NEO_CRT_CLANG_MAJOR=19` | `:64-65` |
| installs `clang-19`, `lld-19`, `llvm-19`, `libclang1-19` when absent | `:71` |
| asserts `clang-19 -dumpversion` major == 19 | `:73` |
| exports `NEO_CLANG=clang-19`, `NEO_LLVM_BIN=/usr/lib/llvm-19/bin` | `:74-75` |
| host configure `CC`/`CXX` from `$NEO_LLVM_BIN` | `:96` |
| neoplutarch configure `HOST_CC`/`HOST_CXX` from `$NEO_LLVM_BIN`, `NEO_CLANG=$NEO_CLANG` | `:151` |
| `llvm-nm` from `$NEO_LLVM_BIN` | `:213` |
| bindgen's libclang from `$NEO_CLANG -print-resource-dir` | `:243-244` |
| `llvm-ar` from `$NEO_LLVM_BIN` | `:259` |
| Rust nightly pinned (`nightly-2026-04-16`) | `rust-toolchain.toml:10` |
| cbindgen pinned (`--version 0.29.4 --locked`) | `ot61-mozjs-generate.yml:317` |

The same `Install clang (the Dockerfile's pinned major)` step is **success** in run
[42536660879](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42536660879) at `bbdd374c1`, and
[42533574513](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42533574513) at `2515288c1` is green too.
`VENDORED.md:111-114` records that the committed set is from run `42532935334` at `4f7c4e75`, the first with the
pinned clang 19, and that going from the runner's clang 21 to 19 changed exactly two files (`-Wno-character-conversion`
dropped from all 167 compile entries, and `jsapi.rs`), which is the drift the pin prevents.

**Residual, stated plainly:** `runs-on` is still `ubuntu-latest` unless `vars.CI_RUNNER_LABELS` overrides it
(`:30`), and `python3` / `make` are the runner's — the "Tools present" step only **prints** them (`:80-81`), it does
not pin them. The pin my earlier FAIL asked for was the compiler, and that is now in place; the byte-exact drift
check (`:290-303`) means a Python/make bump reddens the job loudly rather than changing the committed set silently,
and CLAUDE.md explicitly allows the host's `python3` for build time. If the PM's bar is that every tool be
version-pinned, python3/make remain the narrow gap; the frozen ABI's inputs are clang/LLVM and are pinned.

## 3. `dtoa.c` is recorded under the dtoa / Lucent notice — PASS

`third_party/ot61-mozjs/VENDORED.md:147-148` now splits the row and records it:

- `:147` — `js/src`, `js/public`, `mfbt`, `mozglue/misc` are "MPL-2.0, except `js/src/dtoa.c` (next row)".
- `:148` — "`js/src/dtoa.c` | David M. Gay's dtoa / Lucent permissive notice (the file's header). On CLAUDE.md's
  list since J.c.'s ruling of 2026-11-08. Compiled through `js/src/util/DoubleToString.cpp:70`".

The claim holds against the files: `.../js/src/dtoa.c:1-18` is the David M. Gay / Lucent notice ("Copyright (c)
1991, 2000, 2001 by Lucent Technologies … Permission to use, copy, modify, and distribute this software …"), and
`.../js/src/util/DoubleToString.cpp:70` is `#include "dtoa.c"`. `dtoa.c` occurs exactly once in the tree, at that
path. `CLAUDE.md:917` on `origin/main` lists "the dtoa / Lucent permissive notice (David M. Gay's `dtoa.c`, as
SpiderMonkey links it; PL, 2026-11-08)" (commit `da68b78f2`). The PR branch's base predates `da68b78f2`, so the
line is on `main` rather than in this branch's `CLAUDE.md`; the record's reference to it is correct. THINK #10
(`docs/THINK-2026-10-31-ot61-c3c-spidermonkey.md:70`, on `refs/pr/10` at `10258cc1`) carries the matching dtoa row
and cites `da68b78f`, so the full table `VENDORED.md` defers to agrees.

## Notes on the latest head (not item verdicts)

- **The latest head is one commit past the frozen set, and its generator run fails at the freeze step.**
  `bc1b15463` adds `freeze.py`'s `glue_commands.json` output but does not commit
  `third_party/ot61-mozjs/generated/glue_commands.json` (the `generated/` tree at this SHA has the 2 bindings,
  `compile_commands.json`, `dist_include.txt`, 96 `obj/` files, 30 `patched/` entries and `system_wrappers.txt`; no
  glue file). Run
  [42540844398](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42540844398) at `bc1b15463` completed
  **failure** at `freeze the outputs, and check them against the committed copy`; every step through bindgen is
  green. The new glue file is the only added output and it is not committed here, so this is the normal
  emit-then-commit state (or a glue-log extraction miss) until the next frozen-set commit — not a regression in
  items 1–3.
- **No green run at the exact latest head.** The last green generator runs are `42536660879` at `bbdd374c1` and
  `42533574513` at `2515288c1`; both are after the GPL cut.
- The dispatched head `bbdd374c1` is green (`42536660879`), so items 1 and 2 are backed by a green run at the head
  the dispatch names, and by the same pin/VENDORED lines at the actual latest head.

## What I did not check

- I did not re-run the generator (it needs the CI toolchain, `unshare -n` and the full job), so the run evidence is
  the GitHub step lists, not a re-execution.
- I did not re-audit upstream's file contents, and I did not repeat relay 354's full vendor hash comparison; this
  re-check is the three-file cut and its list, the pin, and the dtoa record.
- I did not re-review the original #87 items outside §361 (network-off, secrets/writes, product image), and I did
  not verify the two PGO pages' contents beyond their absence from the tree.
