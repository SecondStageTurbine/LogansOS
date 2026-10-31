<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — OT-61 C0.5 cross-family review (Chad)

**Date:** 2026-10-21 · **Reviewer:** Chad (Codex family) · **Reviewed head:** `459d36e3d3ae60eca2af6f4df5e7f70167f84aa4` · **Base:** `origin/main` @ `2b3b509443dd8bc00de9838a64bd8bd4937a487b`

## 0. Verdict: BOUNCE

One concrete, security-relevant gate bypass remains: A4 G5 can accept a numeric syscall number at a method-qualified `syscall3` call site. This means the claim that an off-manifest syscall addition necessarily fails the build is false. I found no other concrete failure in the seven requested claims. The committed negative tests cover the direct-call form but not the qualified-call form.

## 1. Unsupported `sys/*` paths — PASS, with scope note

Checked `userland/neoplutarch-std-overlay/apply-overlay.py` lines 197–245 and the overlay route modules. Its fallback audit names 14 untouched modules and requires `_ =>` to select std's `unsupported` or `no_threads` module. The routes for stdio, exit, abort, errno, alloc, random, and thread-local are explicitly installed; the smoke exercises unsupported filesystem/network/process/thread/stdin and related paths (FINDINGS §4.1). The unsupported random path panics and reaches the replacement abort path. Heap deallocation intentionally leaks until process exit; that is a documented resource limitation, not a successful false result.

The fallback audit is structural and checks only the last `_ =>` arm for a module-name substring. This is a toolchain-drift guard, not a semantic proof. I found no concrete current false-success path among the audited modules. `args`/environment being empty and terminal detection being false rely on the target's no-argv/no-env and console-only model, as documented in FINDINGS.

## 2. PAL errno mapping — PASS

Checked `std/io_error_neoplutarch.rs` lines 29–53, 66–76 and the host KAT in `tools/xtask/src/ot61_c05_tests.rs`. The table contains all 23 kernel errno values and the committed KAT compares kernel names/numbers and the mapped kinds against an independent expected list. Relevant retry/control-flow entries are `EAGAIN → WouldBlock` (line 35), `ETIMEDOUT → TimedOut` (line 49); there is no `Interrupted` row and `is_interrupted` is constant false (lines 66–69). `EBADF`/`EFAULT → InvalidInput` (lines 34, 38), `EALREADY → AlreadyExists` (line 50), and `ECANCELED → Uncategorized` (line 52) match the documented LogansOS semantics. Unknown values become `Uncategorized`, and errno zero is described as no error.

## 3. Panic abort path — PASS

Checked `std/neoplutarch_rt.rs` lines 32–36 and 72–91, plus the std overlay patch to `sys/pal/unsupported/common.rs`. The target-gated `abort_internal` calls the overlay abort routine; that prints a diagnostic and invokes `SYS_EXIT(134)`. The `not(target_os = "neoplutarch")` branch retains `core::intrinsics::abort()`. The real-std panic smoke and kernel harness require the panic program to be reaped with exit 134, and xtask also checks serial output for the panic message and abort line (FINDINGS §4.1, §4.3). I found no concrete bypass in the installed target path; general user-mode `#UD` containment remains the separately documented OT-83 issue.

## 4. A4 manifest gate — FAIL

Concrete G5 bypass in `userland/neo-std-pal/a4_gate.rs` lines 157–179: it computes `is_ident_tail` from the byte immediately preceding the substring `syscall3(`, then only rejects a non-`SYS_*` first argument when both `!is_decl` **and** `!is_ident_tail` hold. A method call therefore passes the guard. Reproducible source edit to a scanned Rust file:

```rust
struct Gate;
impl Gate {
    unsafe fn syscall3(num: u64, _a0: u64, _a1: u64, _a2: u64) -> i64 {
        core::arch::asm!("syscall", in("rax") num, lateout("rax") _);
        0
    }
}
fn rogue() { unsafe { Gate.syscall3(41, 0, 0, 0); } }
```

The asm remains inside a function named `syscall3`, satisfying G3; `num` remains the allowed G4 rax operand; the literal `41` is not a `SYS_*` token for G1/G2; and the `.` immediately before `syscall3(` makes `is_ident_tail` true, so G5 does not reject it. This is not merely the documented macro/include limitation: it is an ordinary Rust call in a scanned file. The manifest gate must require a named syscall constant at every call site regardless of callee qualification, and add this fixture to the negative tests. The current `a4_g5_refuses_a_literal_syscall_number` fixture changes a direct call only, so it misses this route.

## 5. No new syscall; heap and target NX — PASS (source review)

`git diff origin/main...origin/claude/ot61-c05-std-overlay -- kernel/src/arch/x86_64/syscall.rs kernel/src/errno.rs userland/libneo` is empty. The overlay runtime uses only `SYS_EXIT`, `SYS_WRITE`, and `SYS_MAP_REGION`; the PAL manifest allowlist and smoke are consistent with these. `kernel/src/mm/pmap.rs` defines `USER_DATA_RW` as PRESENT | WRITABLE | USER_ACCESSIBLE | NO_EXECUTE, and the allocator requests only map-region pages (alloc module). FINDINGS §4.3 records the two ELF program-header checks (no W+X or PT_TLS) and objdump finding only the two trampoline syscall instructions. I inspected the source and the reported evidence, but did not independently rebuild/readelf/objdump the ELFs in this review lane.

## 6. Default kernel/toolchain build identity — PASS, not independently reproduced

`tools/xtask/src/main.rs` sets `stage_std_real_smoke(false)` in `build_user()` and only applies the overlay/builds/stages the two real-std binaries when the scenario flag is set. The kernel change is confined to `kernel/src/test_harness.rs` under neo-test cfg; the default kernel syscall/errno/libneo diff is empty. FINDINGS §4.4 reports a pinned-build-ID comparison of all 12 staged ISO-tree files with matching hashes. I did not repeat both ISO builds here, so this is a source audit plus the branch author's recorded byte comparison, not an independent binary reproduction. The persistent rust-src overlay can affect a later non-scenario `-Z build-std` build only if its target cfg fails to isolate it; the overlay's added sys module is `#[cfg(target_os = "neoplutarch")]`, and abort replacement has an explicit inverse cfg for other targets.

## 7. No POSIX surface in kernel/user ABI; only narrow seams forked — PASS

The reviewed diff adds no syscall, errno, or libneo ABI entry. The only kernel change is the neo-test harness path that launches/reaps the real-std smoke programs. The PAL/overlay is in the userspace toolchain layer and uses existing libneo syscall numbers; the new `SYS_MAP_REGION` allocator remains NX. This matches ADR-078 §2.1 and the reviewed branch surface. No POSIX syscall/API surface is added to the kernel or user ABI.

## Commands and review method

- `git fetch origin`
- Read dispatch, findings, plan, and ADR-078 from the reviewed branch/main.
- `graft ask "Review browser rung C0.5 ..." --source` (repo graph probe; branch-specific files were then read directly).
- `Invoke-RestMethod http://127.0.0.1:8404/health` and `/search`; RAG was healthy (`ollama=true`, `qdrant=true`).
- `git diff origin/main...origin/claude/ot61-c05-std-overlay --stat`
- `git diff origin/main...origin/claude/ot61-c05-std-overlay -- kernel/src/arch/x86_64/syscall.rs kernel/src/errno.rs userland/libneo`
- Read `a4_gate.rs`, `ot61_c05_tests.rs`, overlay modules and script, xtask staging/build paths, `kernel/src/test_harness.rs`, and `kernel/src/mm/pmap.rs` at the reviewed head.
- No QEMU gate was run, per dispatch. No ISO comparison was rebuilt in this review lane.
