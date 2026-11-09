<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 363: #10 item 3 re-check (§3.1/§3.2 vs the enforced single thread)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:**
`Plans/DISPATCH-2026-11-08-relays-359-362.md` §363.

**Reviewed:** PR #10 head `10258cc1ee3f5acf42f0d78fecb28eb540045ca0` ("THINK C3c: §3.1/§3.2 match the
enforced single thread (relay 355)"), one file `+13/−10` from `65da764c8`:
`docs/THINK-2026-10-31-ot61-c3c-spidermonkey.md`. `origin/main` at review time is
`1fb76e0140dc3d977827b15660b0d522c2a3220c`. This is a narrow re-check of the relay-355 item 3 FAIL only;
items 1, 2 and 4 passed at `65da764c8` and are not re-reviewed.

**Overall: PASS.** §3.1 and §3.2 now prescribe the same selection §4 enforces: the POSIX threading sources
on C3b's pthreads in both steps, with helper threads off in C3c-1 (`gCanUseExtraThreads = false`, patch
0003) and on in C3c-2. The "WASI first" / "no-op threads" / "today's `neo-crt`" prescriptions are gone; the
remaining "no-op" mentions describe the WASI port or explicitly reject it.

## Checklist

| # | Dispatch item | Verdict | Enforcing refs |
|---|---|---|---|
| 3 | §3.1 and §3.2 now match the enforced single thread (no "WASI no-op backend" or "today's `neo-crt`" left that contradicts the new paragraph) | **PASS** | THINK:111, 115-117, 122-124, 130-136 against THINK:171-178 |

## 1. §3.1 — PASS (`THINK:111`, `115-117`)

The threads row at THINK:111 now reads: "**POSIX**, on C3b's pthreads, in both steps. WASI's no-op
backend makes `JS_NewContext` fail on x86_64. C3c-1 runs **without helper threads**: patch 0003 sets
`gCanUseExtraThreads = false`, so the pool never starts (§4, "Single-threaded, enforced"). C3c-2 turns them
on." The WASI column still lists `noop/NoopThread.cpp` / `noop/CpuCount.cpp` as the *WASI port's* files —
factual about upstream — and the "Ours" cell names that backend only to say why it is not used.

The paragraph at THINK:115-117 replaces the old "WASI first / no-op / today's `neo-crt`" text: "C3b has
since landed, so neo-crt has pthreads, and C3c-1 builds with the POSIX threading backend. The one-thread
rule now comes from helper threads being off (`gCanUseExtraThreads = false`, patch 0003), not from a no-op
backend. C3c-1 needs the neo-crt in #88 …". That is the §4 selection, not the one relay 355 failed.

## 2. §3.2 — PASS (`THINK:122-124`, `130-136`)

- **C3c-1** (THINK:122-124): "no JIT, no shared memory, no intl, POSIX threads with no helper threads
  (`gCanUseExtraThreads = false`), GC memory through `posix_memalign`." The old line's "no-op threads" is
  gone; threads are POSIX with the helper pool off.
- **C3c-2** (THINK:130-132): "Turn `gCanUseExtraThreads` back on, so the helper pool starts on the POSIX
  backend C3c-1 already uses (C3b's pthreads, libc++ with threads)."
- **Order** (THINK:135-136): "**C3a → C3b → #88 → C3c-1 → C3c-2**", and the split's remaining job is
  "turning helper threads on, which needs its own CP table and AAG."

## 3. Against §4 and the code — the prose is the real build selection

- §4 at THINK:171-178 says exactly the same: WASI's no-op threading backend "is *not* used"; the target
  keeps the POSIX backend; patch 0003 sets `gCanUseExtraThreads = false` for `__neoplutarch__`
  (`vm/Runtime.cpp:63`); `JSRuntime::init` initialises the pool only `if (CanUseExtraThreads())`
  (`vm/Runtime.cpp:176`).
- The enforcement is real at #87's current head `3b85ae501` (the branch has moved past the dispatch's
  `bbdd374c`): `third_party/ot61-mozjs/patches/0003-neoplutarch-source-choices.patch:458-462` still sets
  `gCanUseExtraThreads = false` under `__neoplutarch__`, and the patch does not touch
  `js/src/threading/moz.build`.
- Upstream `js/src/threading/moz.build` selects `noop/…` only when `OS_ARCH == "WASI"`, and
  `posix/CpuCount.cpp` / `posix/PosixThread.cpp` for every other `OS_ARCH`; patch 0002 sets `OS_ARCH` to
  `NeoPlutarch` (`0002-configure-neoplutarch-target.patch`). POSIX in both steps is therefore the real
  selection, not just wording.
- Grep over the whole head for the stale prescriptions: no "WASI first" and no "today's `neo-crt`" anywhere.
  The "no-op"/"noop" hits are THINK:110 ("page protections are no-ops", GC memory, unrelated), THINK:111
  (the WASI port column's `noop/` files, and the sentence saying that backend fails `JS_NewContext`),
  THINK:116 ("not from a no-op backend"), and THINK:171 ("is *not* used"). None selects the no-op backend.

## Nits (non-blocking)

- THINK:122 still says "`mozjs` with the WASI-shaped configuration"; in context this is the feature set
  (no JIT, no shared memory, no intl), and the next line says POSIX threads. If the PM wants zero ambiguity
  in a future edit, "the WASI-derived feature configuration" would say it. It does not contradict §4.
- THINK:124's continuation line is not indented under the list item; it renders as a lazy continuation, so
  this is cosmetic only.

## Also in this commit (outside the item; observed, not re-reviewed)

The same commit adds the two LGPL-2.1 PGO pages (`build/pgo/js-input/3d-thingy.html` and
`…/sunspider/date-format-xparb.html`) to §2.2's cut-list row (THINK:86), which answers the relay-355 §2.2
note. I did not re-review items 1, 2 or 4.

## Verification performed

- `git fetch origin`; `origin/main` = `1fb76e014`; PR #10 fetched with
  `git fetch origin +pull/10/head:refs/pr/10`, `git rev-parse refs/pr/10` = `10258cc1`; `git show --stat
  10258cc1` and `git diff 65da764c8..10258cc1 -- docs/THINK-2026-10-31-ot61-c3c-spidermonkey.md` (one
  file, `+13/−10`).
- Read the full THINK at `10258cc1`; cited lines are the head's line numbers.
- Checked `origin/claude/ot61-c3c1-mozjs-vendor` at `3b85ae501`: patch 0003's `gCanUseExtraThreads` hunk,
  patch 0002's `OS_ARCH`, and the vendored upstream `js/src/threading/moz.build`.
- RAG was not available in this worker (`http://127.0.0.1:8404/health` refused); direct reads and `grep`
  supplied the evidence.
- No source, code or other doc changed: this commit is the RETURN only. The pre-existing untracked
  `pr59-review/` directory is not staged and is not part of this branch.
