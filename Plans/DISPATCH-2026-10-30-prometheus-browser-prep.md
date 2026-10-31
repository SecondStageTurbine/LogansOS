<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH — Prometheus (Qwen): browser prep, running ahead of seat D

**Why:** J.c. (2026-10-30) asked whether the local Qwen could help with the browser. Per CLAUDE.md, Prometheus takes
research, design and host-only work, not QEMU-gated builds. So it prepares the browser ladder's **next** rungs before
seat D reaches them (`docs/ADR-078-ot61-browser-c-ladder.md`). Seat D is on C2.3b (#245).

**Delivery for every section:** one branch per section named `prometheus/ot61-<slug>`, off `origin/main`
(`git fetch origin && git checkout -B prometheus/ot61-<slug> origin/main`). Commit the file the section names, push
the branch, and reply with the commit SHA. **Never push to main and never merge.** Exact bytes (versions, SHAs, license
text) come from the raw sources via `git` or `curl`, never a summarizing fetch. GPL code is read for the principle
only.

## Section 200: the Stylo / layout dependency closure. Slug `stylo-closure`
The pattern is `docs/RESEARCH-2026-10-29-ot61-c2.3b-html5ever-stylo-closure-SPIKE.md` (#200), which did html5ever and
selectors. Do the same for the next rung, Servo's `style` (Stylo) and `layout` crates:
- Every crate in their dependency closure, at the versions Servo pins: name, version, license, last commit date, upstream
  URL, and whether it has a `build.rs` or proc-macro (which runs at build time).
- Which ones are already vendored under `third_party/` by C2.3b.
- What each needs from the OS: threads, atomics, the clock, the filesystem, `std` features. Compare against what
  LogansOS's std overlay provides (`userland/neoplutarch-std-overlay`, `userland/neo-std-pal`).
- Anything incompatible with our MPL-2.0 tree.

File: `docs/RESEARCH-2026-10-30-ot61-stylo-layout-closure.md`.

## Section 201: the SpiderMonkey port study (rung C3). Slug `spidermonkey-port`
For ADR-078's C3 row (a static C++ runtime plus SpiderMonkey, interpreter only, no JIT):
- How SpiderMonkey is built standalone (`js/src`, mozjs crate or Servo's `mozjs`): toolchain, configure flags for
  no-JIT, and the minimum it needs from the OS: pthreads, mmap/mprotect, TLS, the clock, and signals.
- Map each need onto what LogansOS has (`docs/SDK-ABI.md`, `kernel/src/arch/x86_64/syscall.rs` and its `SYS_*`, the C1
  threads/futex, the VM map/protect primitives) and list the gaps.
- Trap-mine other projects' ports of SpiderMonkey to a new or embedded OS: known pitfalls, each with how we'd detect it.

File: `docs/RESEARCH-2026-10-30-ot61-spidermonkey-port.md`.

## Section 202: host-side HTML/CSS known-answer fixtures. Slug `html-css-fixtures`
- A small set of HTML and CSS test inputs, each with its expected parse tree (for html5ever) and selector-match
  results (for selectors).
- Take them from the **html5lib-tests** and **web-platform-tests** raw files (via `git`, not a summary; record the
  upstream SHA and license), or hand-lay them.
- Put them under `tools/ot61-fixtures/`, as data plus a README. Seat D wires them into the on-target smoke.

File: the fixtures directory plus `tools/ot61-fixtures/README.md`.
