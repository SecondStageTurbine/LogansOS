<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 370: #93 design review (C3c-1 step 4, SpiderMonkey green on target)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:**
`Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md` §370.

**Reviewed:** PR #93, branch `claude/ot61-c3c1-think-step4`, at the dispatched head
`21b56b1656b41432f930e957689149ec1fc793e1` ("THINK C3c-1 product build: step 4 on target (mozjs_smoke green;
the SyscallHeap alignment fix)"). The range `e3d39dbdb..21b56b16` is one commit, one file, +13/-0:
`docs/THINK-2026-11-08-ot61-c3c1-product-build.md`. `origin/main` at review time is
`5f5b8136c8222d17fd66cfe6c6c68b571a2abfad`; the PR's base is `e3d39dbdbd9bfa112175f11ae782a075922d8c3d`.
The change is documentation only.

The claims in §5.4 name a run on the try branch `claude/ot61-c3c1-mozjs-try` at
`38d2c4381c9846d2a530b816271d9d57d4eadf5c`, so the code claims are checked at that head.

**Overall: PASS, all three items.** The cited run is green on all four QEMU shards; `mozjs_smoke` is
hard-gated by xtask on every `ok(n)` line; and the C3c-1 scope in the THINK (JIT off, one thread, the
product-crates PR still to come) matches the code. Two non-blocking wording points are in §Notes.

| # | Dispatch item | Verdict | Enforcing refs |
|---|---|---|---|
| 1 | Run `42556683335` is green on all four QEMU shards and `mozjs_smoke` passed in it | **PASS** | run API: head `38d2c4381`, conclusion `success`, jobs `128964111421`/`128964111375`/`128964111396`/`128964111275` all `success`; PR body's shard-1 line; `kernel/src/test_harness.rs:177,3098,3117,3137`; `tools/xtask/src/main.rs:2519,2522,19680-19700,22787-22804`; `ci.yml:1052-1053` |
| 2 | xtask requires every `ok(n)` line it claims; a missing line fails the scenario | **PASS** | `tools/xtask/src/main.rs:19680-19700` (10 `ok(n)` + `PASS`), `:22787-22804` (missing needle -> `Err`); `userland/mozjs-smoke/src/main.rs:47-84,133-137,153-157,166-172,174-181,184` |
| 3 | What the THINK says is still missing for C3c-1 matches the code | **PASS** | THINK:152-162,168,186; `generated/obj/js/src/js-confdefs.h:52` (`JS_CODEGEN_NONE`); `patched/mozjs-sys/build.rs:80-86`; `generated/patched/mozjs-sys/mozjs/js/src/vm/Runtime.cpp:63-67,176`; no product-crates PR (branch `2761bb71b` only) |

## 1. The cited run is green on all four QEMU shards — PASS

The public Actions API for run `42556683335` reports `head_sha = 38d2c4381...`, `event =
workflow_dispatch`, `conclusion = success`. The four QEMU jobs are all `completed / success`:

```
128964111421  QEMU Integration Tests (shard 1/4)  success
128964111375  QEMU Integration Tests (shard 2/4)  success
128964111396  QEMU Integration Tests (shard 3/4)  success
128964111275  QEMU Integration Tests (shard 4/4)  success
```

The other executed jobs in that run are green too (Host Tests, Format & Lint; Build Checks; Product kernel
boot; Production RNG seed boot; Decoder Fuzz); `Batch report` is `skipped` because this was not a batch run.
That is the "green on all four QEMU shards, plus host tests, format, lint and build checks" claim in the PR
body.

For `mozjs_smoke` specifically, the PR body records shard 1, job `128964111421`, `mozjs_smoke 520.1 … 1
pass`, i.e. the scenario appears once and passed. The job id is the shard-1 job above, and its run head is the
try head. I also confirmed the scenario was scheduled rather than skipped: its `TestScenario` entry is
`ignored: false` (`tools/xtask/src/main.rs:2519-2523`), and the run's Host job passed the blocking
`neo-test shard coverage (4-way partition is complete + disjoint)` gate (`ci.yml:1052-1053`), so the 4-way
partition covers it exactly once. A shard job fails when `cargo neo-test` returns an error
(`tools/xtask/src/main.rs:10915-10931`), so a green shard containing `mozjs_smoke` is a scenario pass.

The THINK's failure narrative is also consistent with the record: the first try run `42548550310` (head
`ee40505ee`) failed, and its only red QEMU job is shard 1 (job `128939026436`); after the `grow_need` fix at
`e5b7aabd7` the second run's shard 1 is green. The `grow_need` code and its three host tests are at
`userland/neo-std-pal/src/lib.rs:207-217,437-467`; the smoke's check 12 is at
`userland/mozjs-smoke/src/main.rs:133-137`.

**Evidence-access note.** GitHub's log-archive and artifact endpoints refuse anonymous downloads
(`403 "Must have admin rights to Repository"`), so I could not independently open the raw job log from this
worker. I verified the run, the four shard jobs, the exact shard-1 job id, the scenario's non-ignored entry and
the blocking coverage gate through the public API and the repository; the per-scenario line above is the PR
body's own log citation. The run cannot be green with a failed `mozjs_smoke` under the xtask gate, so the
claim is supported; the literal `[ mozjs-smoke ] ok(12)` line is the one artifact I could not paste.

## 2. xtask requires every `ok(n)` line it claims — PASS

The smoke emits ten `ok(n)` lines and a final `PASS`:

- `ok(12)` and `ok(11)` (`userland/mozjs-smoke/src/main.rs:133-137,153-157`);
- `ok(1)` … `ok(6)` from the `CHECKS` table (`:47-84`, evaluated at `:160-164`);
- `ok(7)` and `ok(8)` (`:166-172,174-181`);
- `PASS` (`:184`).

`tools/xtask/src/main.rs:19680-19700` hard-codes exactly those eleven strings and calls
`require_serial_contains` for each. `require_serial_contains` reads the scenario's serial log and returns
`Err(... "serial did not contain ...")` on a miss (`:22787-22804`), which becomes a failed scenario and a red
shard. There is no optional or best-effort path. So a missing `ok(n)` line fails the scenario: **PASS**.

Non-blocking maintenance caveat: the list is a fixed array, not a scan for `[ mozjs-smoke ] ok(`. A future
`ok(13)` added to the smoke would not be required until the xtask array is updated. Today's list is complete.

## 3. The C3c-1 "still missing" scope matches the code — PASS

The THINK's C3c-1 scope is the interpreter with no JIT, one thread (no helper threads), and the product-crates
PR still to land (§2, §5.3, §6: THINK:51,95,152-162,186). The code at the cited try head matches each:

- **No JIT.** The frozen configure output defines `JS_CODEGEN_NONE 1`
  (`third_party/ot61-mozjs/generated/obj/js/src/js-confdefs.h:52`); the product `mozjs-sys` build script
  panics if the `jit` feature is enabled (`third_party/ot61-mozjs/patched/mozjs-sys/build.rs:80-86`), and
  `mozjs`'s feature list only forwards `jit` to that rejected feature
  (`third_party/ot61-mozjs/patched/mozjs/Cargo.toml`: `jit = ["mozjs_sys/jit"]`). The smoke does not enable
  it (`userland/mozjs-smoke/Cargo.toml:26`: `default-features = false, features = ["libz-sys"]`).
- **One thread.** The patched `vm/Runtime.cpp` sets `gCanUseExtraThreads = false` under `__neoplutarch__`
  (`third_party/ot61-mozjs/generated/patched/mozjs-sys/mozjs/js/src/vm/Runtime.cpp:63-67`; patch 0003), and
  `JSRuntime::init` starts the helper pool only `if (CanUseExtraThreads() && ...)` (`:176`). This is the same
  one-entry-point rule the THINK describes at THINK:186 and the original C3c THINK §4.
- **Product-crates PR still missing.** `origin/main` has no `third_party/ot61-mozjs` tree. The product crates
  exist on `claude/ot61-c3c1-product-crates` at `2761bb71bd3cb40b993a12a746e3fd5029e6c34f` (and are merged
  into the try head); there is no open PR whose head is that branch. #87, #88 and #81 are still open, which is
  exactly why the PR body says the product branch's PR waits for them and then rebases. The THINK places the
  product crates at step 3 "on a try branch" (THINK:152-162), not as landed work; the code agrees.

The specific step-4 details also match: the kernel spawns the smoke with 256 MiB
(`kernel/src/test_harness.rs:3098,3117`), requires exit 0 and the root's frames returned (`:3128-3138`), the
kernel feature is `neo-test-mozjs-smoke = [..., "browser-runtime", "mem-quota", "kernel-rng"]`
(`kernel/Cargo.toml:497`), xtask runs it under `-cpu max` (`tools/xtask/src/main.rs:15164,15216`) in `1024M`
(`:15738`) with the smoke as a boot module (`:7759`), and the smoke's dev profile strips debuginfo
(`userland/mozjs-smoke/Cargo.toml`, `[profile.dev] strip = "debuginfo"`), matching THINK:166-169.

## Notes (non-blocking)

- **"Merged in" is loose for #81.** The THINK:176-177 says the run was "on the product crates with #88 and
  #81's crates merged in". #88 is a real merge (`af52f9887`), but #81's `libc`, `libz-sys`, `pkg-config` and
  `vcpkg` are committed on the try branch as "try only" cherry-picks (`a10ee4e77`, `844730c0b`), not a merge
  of #81. The content is there and the run is valid; "with #81's crates" would be precise.
- **The xtask `ok(n)` list is hard-coded** (see §2). It is complete for the current smoke; a new check needs a
  matching array entry or it will not be gated.

## No CI run

#93 is one `docs/` file, so push CI's paths-ignore applies (`docs/**`, `*.md`). This review was read-only:
`git show`/`ls-tree`/`git log`, the public Actions API for the cited run and jobs, and `git grep`. No repo
build or test was run or needed.

## Verdict

**PASS, all three §370 items** at `21b56b16`: run `42556683335` is green on all four QEMU shards, with
`mozjs_smoke` in the green shard 1 job and hard-gated by xtask; xtask requires every `ok(n)` line the smoke
emits and fails on a miss; and the THINK's C3c-1 scope — JIT off, one thread, the product-crates PR still to
come — matches the code at the cited try head.
