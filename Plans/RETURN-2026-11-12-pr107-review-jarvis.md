<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Jarvis review of #107 (OT-61 C3c-1 product build)

Reviewed head `6c5bc8b733905aa5089d462c170f610e4835e16a` (`claude/ot61-c3c1-product-crates`) against the dispatch and `docs/THINK-2026-11-08-ot61-c3c1-product-build.md` §§2, 4, 5.

1. **Build hermeticity — PASS.** `mozjs-sys/build.rs` uses only `std` (`third_party/ot61-mozjs/patched/mozjs-sys/build.rs:27-35`), states it runs no network or upstream generators (`:4-8,23`), and compiles the frozen compile/glue command sets using the pinned `neo-cc`/`neo-c++` wrapper (`:16-18,181-207`). Source directories are explicitly bounded (`:44-59`). `userland/mozjs-smoke/Cargo.lock` has no `source =` registry entries (path sources only).

2. **Workflows — FAIL.** Both workflows can run on self-hosted runners: `.github/workflows/ot61-closure.yml:60,447` and `.github/workflows/ot61-mozjs-generate.yml:30` use `fromJSON(vars.CI_RUNNER_LABELS || '"ubuntu-latest"')`. Setting `CI_RUNNER_LABELS` to `["self-hosted"]` routes the jobs to a self-hosted runner. Both workflows also use `actions/checkout@v4` without `persist-credentials: false` (`ot61-closure.yml:70,458`; `ot61-mozjs-generate.yml:42`), so the checkout token remains in the repository's git config while workflow code runs. A push to the matching `claude/ot61-**` branch (`ot61-closure.yml:23`; `ot61-mozjs-generate.yml:18`) runs that branch's code; for example, code can read the configured git extraheader. No explicit `permissions` block limits the token. Thus the hosted-only and no-secret-to-PR-code requirements are not enforced.

3. **Scenario — PASS.** The smoke evaluates JS and compares returned int32 values against explicit expected results (`userland/mozjs-smoke/src/main.rs:47-84,102-110,161-180`). The kernel emits its pass line only after exit success and memory-frame return (`kernel/src/test_harness.rs:3500-3514`), and xtask requires the individual evaluation, regexp, normalization, date and GC result lines plus `PASS` (`tools/xtask/src/main.rs:20423-20443`).

4. **PAL change — PASS.** The only allocator behavior addition computes growth from request size plus alignment padding, with saturating addition and the existing minimum (`userland/neo-std-pal/src/lib.rs:204-215,251-271`); the host tests cover small, page-aligned and 1 MiB-aligned requests (`:432-467`). The change addresses SpiderMonkey's 1 MiB aligned GC chunk. It adds no new syscall or panic path.

5. **Licences and prior art — PASS.** `VENDORED.md` records the upstream URL and exact Servo commit (`third_party/ot61-mozjs/VENDORED.md:11-19`), says the vendor tree is unmodified and documents the applied patches (`:23-29`), and records dependency licences and the GPL/LGPL sweep (`:136-150`). It also names the excluded bundled components and reasons (`:152-160`). The product design documents reusing the existing vendored closures and `neo-crt` (`docs/THINK-2026-11-08-ot61-c3c1-product-build.md:138-143,164-177`).

No tests were run; this was a code and workflow review.
