<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# OT-61 browser C0.5: the bounded rust-src std overlay, completed (FINDINGS)

**Date:** 2026-10-21 · **Seat:** S3 (Claude family) · **Branch:** `claude/ot61-c05-std-overlay`
**Base measured against:** `origin/main` @ `2b3b5094`
**Authority:** ADR-078 §3 (C0.5); C0 record `docs/RESEARCH-2026-07-22-ot61-c0-libstd-pal-FINDINGS.md`;
plan `Plans/NOTE-2026-10-21-ot61-c05-plan-s3.md`.
**Merge:** PL-reserved. This branch is delivered for gate and review only.

**Status: built. All four close-gate items hold locally, with evidence below.** The 4-shard CI gate
(PM-dispatched) and the cross-family Codex review are pending; their URLs and verdicts are recorded in §6
once they exist.

---

## 0. Result in one paragraph

A real `std`-linked program now compiles, links, and runs on-target with **no silently-wrong behaviour on
any surface it touches**:

- `std::println!` and `eprintln!` both reach the console.
- A heap `Vec`/`String` grows across many pages.
- LogansOS errnos decode to the right `io::ErrorKind` and message.
- Every surface the overlay does not implement (fs, net, process, thread, stdin, cwd) fails with
  `ErrorKind::Unsupported`.
- `std::process::exit(0)` exits cleanly.
- A **panic** prints its message on serial and then aborts the process with **exit 134**.

Before this rung, a std panic **hung the whole machine**: std's stock abort is `ud2`, and the kernel
answers a ring-3 `#UD` with `halt_forever()`. The overlay still issues only `SYS_WRITE`, `SYS_MAP_REGION`
and `SYS_EXIT`. The A4 gate now scans every file that can carry a syscall, and committed negative tests
prove it bites. The default kernel and ISO are byte-identical to `main`.

## 1. What the scaffold already had (verified on `main`, not trusted)

The LogansOS history starts from an import commit (`a17e615c`). That import carried most of a prior
C0.5 attempt (NeoPlutarch `79d8075e`). `origin/claude/ot61-c05-restage` is that same commit on the old
history. It has no merge-base with `main`, and its `neo-std-pal/build.rs` predates `main`'s numeric-rax
hardening, so it is superseded.

| Piece | State found | Disposition |
|---|---|---|
| C0 target JSON, `neo-std-pal`, manifest, `std-pal-smoke` | Present. `std-pal-smoke` is build-only (no QEMU scenario). | Re-verified: builds default and `--features unsupported-demo`. |
| `apply-overlay.sh` + `stdio`/`alloc` overlay modules | Working, but with silent-wrong stubs (§2) and an exit arm carrying its syscall inside the shell script. | Rewritten (§3). |
| `std-real-smoke` + `std_real_smoke` scenario | Working scaffold: print/alloc/exit only, already scenario-gated (no default-build staging). | Extended (§3.4). |
| A4 gate (named + numeric-rax over PAL src and overlay `.rs`) | Present; blind spots (§2.6). The negative proof was prose only. | Hardened; negative tests committed (§4.2). |
| io-errno table | Absent: routed to std `generic` (everything `Uncategorized`, every message "operation successful"). | Built (§3.2). |
| Panic/abort arm | Absent (§2.1). | Built (§3.3). |

## 2. Defects found and fixed

1. **A std panic halted the machine.** The chain is `panic_abort::__rust_start_panic` →
   `std::rt::__rust_abort` → `sys::pal::unsupported::abort_internal` → `core::intrinsics::abort()`
   (`ud2`). The kernel's ring-3 `#UD` handler (`idt.rs`, `invalid_opcode`) is `dump(); halt_forever()`.
   *Fixed* for std (§3.3). The underlying kernel containment gap is logged as **OT-83**.
2. **Panic messages went nowhere.** `panic_output()` returned `Some(Vec::new())`, so std formatted the
   message into a throwaway `Vec`. Now it returns `Stderr`.
3. **Every stdio write error was swallowed.** `is_ebadf()` returned `true` for every error, and std's
   `handle_ebadf` turns an EBADF on a standard stream into success. Now it matches `EBADF` (9) only.
4. **`Stdin::read` returned a fake EOF.** Now it returns `Err(Unsupported)`.
5. **stderr was fd 2, which is not a console on LogansOS.** Found on-target: the `eprintln!` line never
   appeared. `sys_write_impl` treats only fd 1 as the console and resolves every other fd through the
   process file-handle table. So an fd-2 write failed `EBADF`, std swallowed it as success (item 3's
   mechanism), and the panic message and abort line were lost. Had the process held a file at handle 2,
   stderr would have written *into that file*.
   Fix: overlay stdout and stderr both use `CONSOLE_FD = 1`. **The same bug was in C0's PAL**
   (`neo-std-pal::STDERR_FD = 2`, used by every loud-stub message), and is fixed there too.
6. **A4 gate blind spots:**
   - (a) The exit arm's `in("rax") 0u64` lived in `apply-overlay.sh`, which was never scanned.
   - (b) `const X: u64 = 41; in("rax") X` bypassed both scans.
   - (c) `inout("rax")` was not matched.
   - (d) An allowed *name* bound to another syscall's *number* (`const SYS_WRITE: u64 = 41`) passed.
7. **The overlay was not re-application-safe.** `insert_arm` skipped any already-patched file, so on a
   persistent or cached toolchain an edited arm silently never landed. (CI restores `.rustup-home` from
   cache, so this is reachable there.)
8. **cargo `-Z build-std` does not fingerprint rust-src edits.** Found on-target: after the fd fix, the
   binary still carried the old std, because `libstd` was not rebuilt. A stale overlay std is silently
   reused.

## 3. What was built

### 3.1 The overlay: one syscall seam, every other surface loud

`userland/neoplutarch-std-overlay/`:

**`apply-overlay.py`** (`apply-overlay.sh` is a thin wrapper that xtask calls) does four things, in order:

1. **Pristine restore.** Every patched rust-src file is restored from `<file>.neo-pristine` before
   patching. The backup is made on first run. A pre-backup legacy patch is reverted structurally
   (§4.5).
2. **Copy** the four overlay modules into `std/src/sys/`.
3. **Patch the seams.** Each arm only names a module or calls `sys::neoplutarch_rt`; no arm carries code
   of its own.
4. **Fallback audit.** The 14 `sys/*` modules the overlay does not touch must still have their `_ =>`
   fallback on std's loud `unsupported` (or single-threaded `no_threads`) module. A toolchain bump that
   changes one fails the apply.

| Seam | Routed to | Behaviour |
|---|---|---|
| `sys/mod.rs` | declares `neoplutarch_rt` | The **only** syscall seam: one `syscall3` trampoline plus `write`/`map_region`/`exit`/`abort`. |
| `sys/stdio` | `stdio/neoplutarch.rs` | stdout+stderr → `SYS_WRITE(fd 1)`; real errors; `is_ebadf` exact; stdin `Err(Unsupported)`; `panic_output` = stderr. |
| `sys/exit.rs` `fn exit` | `neoplutarch_rt::exit` | `SYS_EXIT(code)`. |
| `sys/pal/unsupported/common.rs` `abort_internal` | `neoplutarch_rt::abort` | One loud line, then `SYS_EXIT(134)`. Other targets keep `ud2` (the replacement is `cfg`'d). |
| `sys/io/error` | `io/error/neoplutarch.rs` | The errno table (§3.2). |
| `sys/alloc` | `alloc/neoplutarch.rs` | `System`: page-rounded `SYS_MAP_REGION` (NX), leaky dealloc. Overflow or over-alignment → null → `handle_alloc_error` → abort 134. |
| `sys/random` | `unsupported::fill_bytes` | Panics → abort 134. Entropy is a C2 capability seam. |
| `sys/thread_local` | `no_threads` statik lists | Matches the target (`singlethread`, no `has_thread_local` ⇒ no `PT_TLS`). |
| args, env, fs, net, process, pipe, thread, time, paths, pal, sync × 4 | **unchanged `_ =>` fallback (audited)** | `Err(Unsupported)`, or a panic → abort 134, or an accurate empty answer (`args`/`getenv`: the loader passes no argv/env; `is_terminal` false). |

"Route most `sys/*` to the PAL unsupported stubs" is implemented as std's own `sys/pal/unsupported`
tier. std sits *below* `neo-std-pal` and cannot link it, so std-internal `unsupported` *is* the stub tier
inside std. The deliverable was that each of these stubs is loud, and §4.1 proves that on-target.

### 3.2 The io-errno table

`ERRNO_TABLE` maps all 23 errnos in `kernel/src/errno.rs` to a `(name, ErrorKind, message)`.
`error_string` renders `"ENOENT: no such entry (LogansOS errno 2)"`. Unknown codes decode as
`Uncategorized` with their number.

**Load-bearing rule:** nothing maps to `Interrupted`, and `is_interrupted` is constant `false`. std's
`write_all`/`read_exact` retry on `Interrupted`, and LogansOS has no EINTR, so a failing syscall can
never make std spin.

Mapping choices where std's unix table has no entry:

| errno | Maps to |
|---|---|
| `ESRCH` | `NotFound` |
| `EBADF`, `EFAULT` | `InvalidInput` |
| `EALREADY` | `AlreadyExists` (LogansOS meaning: one-shot already done) |
| `EBADMSG` | `InvalidData` |
| `ECANCELED` | `Uncategorized` (the kernel never surfaces it to userspace) |

### 3.3 The panic arm

Under `panic=abort` the path is:

1. std's default hook writes the message to `panic_output()`, which is stderr, which is the console.
2. `panic_abort` → `__rust_abort` → `abort_internal`.
3. `abort_internal` writes `[ std ] neoplutarch abort_internal -> SYS_EXIT(134)` and exits **134**
   (128 + SIGABRT). That is distinct from a clean 0 and from the kernel fault-kill 139.

The same path also covers `rtabort!` and `handle_alloc_error`.

### 3.4 The smoke

`userland/std-real-smoke` now builds two binaries from the same overlay std:

- **`std-real-smoke`** runs 30 numbered checks: print (stdout, stderr, raw `write_all`), allocate (a
  16 Ki-element `Vec` growing across 16 pages, plus `String` growth), errno (8 kinds + unknown), and
  loud stubs (fs ×2, net ×2, process, thread, stdin, cwd, args, parallelism). It exits 0 on success, or
  with the failing check's number.
- **`std-real-panic`** triggers a real std panic (index out of bounds).

The kernel scenario `std_real_smoke` spawns both through the existing `spawn_user`/ELF-loader path. It
PASSes only if the smoke exits 0 **and** the panic arm exits 134.

Exit codes cannot tell a loud panic from a silent one, so **xtask additionally requires** these serial
lines:

- the stderr line;
- the smoke's PASS line;
- `panicked at src/panic.rs:`;
- the panic message;
- the abort line.

Sabotaging one needle made the scenario fail (§4.1), so the check is wired in.

### 3.5 xtask

- **Overlay stamp.** A BLAKE3 over every overlay file plus the sysroot path. When it changes, the
  smoke's target-triple dir is wiped so std rebuilds (defect 8).
- **Second ELF.** `std-real-panic` is built, staged as `/modules/neo-std-real-panic` and injected into
  `limine.conf`, all **only for the `std_real_smoke` scenario**. That gives 6 of the 8 boot modules.

## 4. Close gate, item by item

### 4.1 A real std-linked smoke runs on-target in QEMU: **PASS (local); CI gate pending (§6)**

Local QEMU 8.2.2, `cargo xtask neo-test std_real_smoke`, branch head `76888e6a`. Serial excerpt:

```
limine: Loading module `boot():/modules/neo-std-real-smoke`...
limine: Loading module `boot():/modules/neo-std-real-panic`...
[ std-real-smoke ] std::println!: ok
[ std-real-smoke ] std::eprintln! (stderr -> the console fd): ok
[ std-real-smoke ] std heap Vec+String: len=16384 checksum=0xc5330000
[ std-real-smoke ] ok: errno 2 -> NotFound "ENOENT: no such entry (LogansOS errno 2) (os error 2)"
[ std-real-smoke ] ok: fs::File::open -> Unsupported
[ std-real-smoke ] ok: net::TcpStream::connect -> Unsupported
[ std-real-smoke ] ok: thread::Builder::spawn -> Unsupported
[ std-real-smoke ] ok: io::stdin().read -> Unsupported (no fake EOF)
[ std-real-smoke ] PASS: std print+alloc+errno+loud-stubs+exit
[ kernel ] proc: std-real-smoke exit code 0
[ std-real-panic ] about to panic (expect exit 134)
thread '<unnamed>' (1) panicked at src/panic.rs:26:17:
index out of bounds: the len is 0 but the index is 3
[ std ] neoplutarch abort_internal -> SYS_EXIT(134)
[ kernel ] proc: std-real-panic exit code 134
NEO_TEST/1 event=PASS name=std_real_smoke std_real=ran print+alloc+errno+loud_stubs exit_code=0 panic_abort_exit=134 i_am=real_std opens=c0.5
```

The scenario is `ignored: false` in `TEST_SCENARIOS`, so the 4-shard gate runs it.

The gate was falsified locally three ways:

- The first on-target run **failed at check 49**. I had expected `available_parallelism` to be
  `Unsupported`; std's stub returns `UNKNOWN_THREAD_COUNT`. It is still loud, so the check now accepts
  any `Err`.
- The fd-2 stderr loss (defect 5) and the stale std (defect 8) were each caught by the serial content.
- A sabotaged serial needle failed the run: ``serial did not contain `index out of bounds: SABOTAGE` ``.

### 4.2 The A4 manifest gate still holds and has teeth: **PASS** (rebuilt after the review bounce, §7)

- **Positive** (real `neo-std-pal` build log):
  `A4 PAL-manifest gate OK — 7 files, 2 syscall3 trampoline(s), syscalls ["SYS_EXIT", "SYS_MAP_REGION", "SYS_WRITE"] ⊆ [gate].allowed_syscalls (G1-G7)`.
  The 7 files are the PAL `src/lib.rs`, the 4 overlay `std/*.rs` modules, `apply-overlay.sh` and
  `apply-overlay.py`.
- **The rules** (`userland/neo-std-pal/a4_gate.rs`). Rust sources are lexed first: comments are removed,
  and string/char literal contents are kept apart from code.

  | Rule | Enforces |
  |---|---|
  | G1 | Every `SYS_*` name in a file (comments included) is on the manifest. |
  | G2 | In code, a `SYS_*` identifier appears only as a value-checked `const`, or as a `syscall3` first argument declared in the same file. |
  | G3 | Exactly one brace-matched `fn syscall3` per trampoline file, none elsewhere. `asm!` appears only inside it, with a template of exactly `"syscall"`. Other asm macros are refused. |
  | G4 | The only rax operand is `in("rax") num`. |
  | G5 | Every other `syscall3` reference, under any qualification, is a call with a `SYS_*` first argument. |
  | G6 | No `include!`, `#[path]`, `#[link]` or `extern` blocks. |
  | G7 | Scripts never mention syscall or asm constructs. |

- **Negative, end to end.** A comment `// probe: would wrap SYS_SPAWN` was added to
  `stdio_neoplutarch.rs`, and the build refused:
  ```
  error: failed to run custom build command for `neo-std-pal v0.0.1`
    A4 gate FAILED — off-manifest syscall surface in the PAL or the std overlay.
      G1 …/neoplutarch-std-overlay/std/stdio_neoplutarch.rs: `SYS_SPAWN` is not in [gate].allowed_syscalls
  ```
  The probe was then reverted.
- **Negative, committed.** `tools/xtask/src/ot61_c05_tests.rs` runs in push-CI host checks and uses the
  same `a4_gate.rs` that `build.rs` runs. Result: **30/30 pass**. Each fixture below must be refused
  under the named rule, and the real tree must pass with exactly 2 trampolines.

  | Rule | Fixtures refused |
  |---|---|
  | G1 | off-manifest name |
  | G2 | `SYS_WRITE = 41`; `let SYS_WRITE = 41`; `static SYS_EXIT = 41`; `use libneo::SYS_WRITE` |
  | G3 | second `syscall` site; `global_asm!`; `int 0x80`; `sysenter`; **Chad's method-trampoline fixture, verbatim**; a second trampoline with named constants; a renamed wrapper holding its own asm; `"nop; syscall"` outside, and `"mov eax, 41; syscall"` *inside*, the trampoline; a block or line comment faking `fn syscall3`; a `macro_rules!` emitting asm; a trampoline file losing its trampoline |
  | G4 | literal rax in the trampoline; `inout("rax")` elsewhere |
  | G5 | literal number; aliased constant; **7 qualified-call forms** (`Gate.`, `Gate::`, `Self::`, `<Gate>::`, `crate::…::`, `self.`, `syscall3 (`); function pointer; `use … as` alias; turbofish; a renamed wrapper forwarding its number; a `"//"` string, or a raw string with `//`, before the call |
  | G6 | `include!`; `#[path]`; `extern` block; `#[link]` |
  | G7 | the replayed legacy exit arm (a script carrying syscall code) |

- **Backstop 1: what the script wrote** (`xtask verify_applied_overlay`, run on every `std_real_smoke`
  build). The 4 installed overlay modules must be byte-identical to the scanned sources, and every line
  `apply-overlay.py` added to a patched std file (diffed against its `.neo-pristine` backup) must pass
  the same checker.
  - Real run: `A4 backstop OK: 4 installed overlay modules == scanned sources; 8 patched std files' added lines pass the gate`.
  - Negative (temporary): the script was changed to write `"sys" + "call3(41, 0, 0, 0);"` into the exit
    arm, an obfuscation that G7 cannot see. The build refused:
    `A4 backstop FAILED — … G5 rust-src-arm:sys/exit.rs:2: syscall3(41, ..) — the number must be a named SYS_* constant`.
- **Backstop 2: ground truth from the ELF** (`xtask verify_std_real_syscall_sites`).
  `llvm-objdump -d --demangle` must show every `syscall`/`sysenter`/`int 0x80` inside
  `neo_std_pal::syscall3` or `std::sys::neoplutarch_rt::syscall3` (both now `#[inline(never)]`), with at
  most one each.
  - Real run: `2 syscall site(s) … all in sanctioned trampolines`, for both ELFs.
  - Negative (temporary): the overlay trampoline was made `#[inline(always)]`. The build refused:
    `A4 backstop FAILED: … has a syscall instruction in std::sys::neoplutarch_rt::map_region`.
- **Stated limits.** The checker is still lexical. It does not expand macros (a `macro_rules!` body is
  scanned as code, so emitting `asm!` or calling `syscall3` is caught, but token-pasting is out of
  scope). The two backstops cover what lexing cannot: the ELF check proves only the two trampolines
  execute `syscall`. What remains unproven is that the *value* reaching a trampoline is a manifest
  constant along every path. That is enforced lexically (G2 + G5), not by data flow.

### 4.3 No new syscall, no executable memory: **PASS**

- `git diff origin/main -- kernel/src/arch/x86_64/syscall.rs kernel/src/errno.rs userland/libneo` is
  **empty (0 lines)**. The kernel diff is `kernel/src/test_harness.rs` only (neo-test-only code).
- Program headers (`readelf -lW`), identical shape for both ELFs:

  ```
  LOAD  0x200000  R     (headers/rodata)
  LOAD  0x215110  R E   (text)
  LOAD  0x27dfb0  RW    (data/bss)
  GNU_STACK       RW
  ```

  There is no W+X segment and no `PT_TLS`. The loader refuses W+X anyway (`kernel/src/loader/elf.rs`).
- The heap: `SYS_MAP_REGION` maps `flags::USER_DATA_RW` = `PRESENT | WRITABLE | USER | NO_EXECUTE`
  (`kernel/src/mm/pmap.rs:106`). The overlay never asks for anything else (G1–G5).
- A1 (W^X × JIT) is untouched.

### 4.4 The default kernel and toolchain build is bit-for-bit unchanged: **PASS**

`cargo xtask iso` was run in the same directory at `origin/main` (`2b3b5094`) and at the branch
(`76888e6a`), with `NEO_BUILD_ID=ot61-c05-identity-probe` pinned (the kernel embeds `git HEAD` as its
build ID otherwise; `kernel/build.rs`). Then `sha256sum` was taken over the whole staged ISO tree.

**Result: all 12 files are identical:** `boot/kernel` (`064a1e43…de4ee`), `modules/neo-{init,worker,shell,arthur}`,
`boot/limine.conf`, the Limine boot files and the licenses. The default build applies no overlay,
builds no std smoke, and stages no std-real module.

**Re-proven after the bounce fix and the rebase onto `origin/main` @ `d4244be5` (CP-68 x2APIC).** With
the branch at `abcc60bf`, and the same method and pinned `NEO_BUILD_ID`, all 12 files are again
identical to `main`. The kernel hash is `41cbf9d8…a7c9`; it changed from the earlier run because
`main`'s kernel changed.

An earlier version of this branch edited `kernel/src/main.rs`; the kernel hash then **differed**, because
line shifts leak into the default binary through panic `Location`s. So `main.rs` was restored
byte-for-byte, and the panic-arm spawner lives in `test_harness.rs`, which is compiled only with
`neo-test`. (The prior C0.5 attempt's "bit-for-bit unchanged" claim carried a 38-line `main.rs`
insertion. It was not hash-verified, and by this mechanism it was very likely false.)

### 4.5 Supporting checks

- **Legacy-toolchain recovery.** A rust-src was patched with `main`'s old `apply-overlay.sh` and the
  backups were deleted. The new apply reverted the legacy patch in 6 files, and the result was
  **byte-identical** to a fresh pristine apply in all 8 patched files.
- **`std-pal-smoke`** (C0): builds default and `--features unsupported-demo`.
- **Close-gate ritual:** `scripts/worker-close-gate.sh`; the result is recorded in the RETURN / PR.

## 5. Boundaries held, ladder hygiene

- **No POSIX surface at the kernel/user ABI.** The only kernel change is neo-test harness code. The
  overlay lives in the browser bundle's toolchain and speaks the existing libneo ABI.
- **No forks** of html5ever, Stylo, layout, DOM, WebRender or the GC. Nothing web-facing is in this rung.
- **No later rung climbed.** Threads, TLS, time, entropy, fs, net, stdin and process stay deterministic
  loud stubs, banked per rung as **OT-84**.
- **Kill-switch (Rule 1).** The overlay is opt-in per scenario; disabled ≡ `main`, byte-for-byte (§4.4).
- **Invariant budget.** No new invariant. A1 and A2 are untouched; I-14 is improved for std programs
  (abort now kills the process, not the machine), and its general gap is OT-83.
- **Pitfall bank (CP-1…).** N/A by construction: no cross-CPU, IRQ, lock, atomic, TLB or scheduler code
  is added or changed. The overlay pins std to `no_threads` sync/TLS. The kernel harness addition is a
  sequential spawn-and-reap on the BSP, using the same calls as the existing `spawn_std_real_smoke`.
- **Byte oracle.** The errno KAT checks the overlay table against an independent hand-written
  expectation *and* against `kernel/src/errno.rs`. The smoke checks 8 entries on-target, from a third
  hand-written list.

**Logged to `docs/OPEN-THREADS.md`** (IDs provisional; the PM may renumber against concurrent lanes):

- **OT-83:** a ring-3 `#UD`/`#DE`/`#GP`/… halts the machine (an I-14 gap; kernel lane).
- **OT-84:** the overlay's loud surfaces, banked per rung. Notably, `HashMap::new()` aborts until the C2
  entropy seam exists, and there is one console fd.

## 7. Cross-family review bounce, and the fix

Chad (Codex family) returned **BOUNCE** on item 4. The RETURN is at
`2ff40845:Plans/RETURN-2026-10-21-ot61-c05-review-chad.md` on `codex/ot61-c05-codex-review-chad`. Six
of the seven items passed: stubs, errno, panic-abort, no new syscall/NX, default-build identity, and no
POSIX surface.

**What was reported.** A method-qualified call `Gate.syscall3(41, 0, 0, 0)`, next to a second
`fn syscall3` method holding the asm, would pass G5. The stated mechanism was that `.` before
`syscall3(` counts as an identifier tail.

**What reproduced.** That exact fixture, and its `::`/`Self::` variants, were in fact already refused
by the old G5: `.` and `:` are not identifier bytes. But the review's underlying point was right. G3
pinned the asm only to "a function named `syscall3`", and probing found **five real bypasses** of the
old checker:
1. a function pointer (`let f = syscall3; f(41, ..)`);
2. a multi-instruction template (`"nop; syscall"`) outside any trampoline;
3. a `"//"` string literal that made the line-comment stripper drop the rest of the line;
4. a `/* fn syscall3 */` block comment faking a trampoline;
5. a second `fn syscall3` anywhere.

The bounce is therefore **upheld in substance**.

**The fix** (§4.2):
- a lexer;
- one brace-matched trampoline per trampoline file;
- `asm!` confined to it, with an exact template;
- every `syscall3` reference checked, under any qualification;
- `SYS_*` bindings checked;
- side doors refused;
- scripts barred from syscall and asm text;
- two xtask backstops: the inserted-arm scan, and the ELF syscall-site check.

Chad's fixture is committed verbatim as a negative test, alongside every bypass above.

## 6. CI evidence and review

- **Push-CI on `074a90ae`: GREEN.** Run [39650092060](https://github.com/SecondStageTurbine/LogansOS/actions/runs/39650092060).
  - Attempt 1: "Host Tests, Format & Lint" died in `rustup toolchain install` on a runner DNS failure,
    before any cargo step ran (infra).
  - Attempt 2 (one re-run of the failed job): [green](https://github.com/SecondStageTurbine/LogansOS/actions/runs/39650092060/job/119422801196).
    The job's steps: fmt, workspace tests, shard-coverage, clippy `-D`, no_std hygiene, kernel clippy,
    neo-test-gated kernel clippy.
  - Every other job is green: SPDX, Product Feature Build, Monolithic Identity Guard, and all 12
    Feature-Gated Coverage arms.
  - The QEMU shards were skipped by design, since they run only on `workflow_dispatch`/schedule.
- **4-shard `workflow_dispatch` QEMU gate on `c4765beb`: GREEN.** Run
  [39677626303](https://github.com/SecondStageTurbine/LogansOS/actions/runs/39677626303). All 22 jobs
  succeeded, including QEMU shards 1/4 through 4/4. `std_real_smoke` is in shard 4/4, per
  `cargo xtask shard-coverage 4`
  ([shard 4](https://github.com/SecondStageTurbine/LogansOS/actions/runs/39677626303/job/119502382327)).
  That shard ran under CI's QEMU 10.2.3; local runs used 8.2.2. The earlier PM gate on `459d36e3` (run
  39666280960) was cancelled once the bounce fix superseded that head.
- **Cross-family review (Chad, Codex family):**
  - First pass: **BOUNCE** on item 4, the A4 gate's teeth. RETURN at
    `codex/ot61-c05-codex-review-chad` @ `2ff40845`. The other six items passed: stubs, errno,
    panic-abort, no new syscall/NX, default-build identity, and no POSIX surface. See §7.
  - Narrow re-check of the fix on `c4765beb`: **PASS on all four sub-items.** RETURN at
    `codex/ot61-c05-recheck-chad` @ `a810ed0b`. The sub-items are G5 call sites, the one trampoline with
    `asm!` only inside it, the lexer and side doors, and both backstops. By its own scope note it is a
    source and recorded-evidence re-check: Chad did not re-run the builds.

## 7. Cross-family review bounce, and the fix

Chad (Codex family) returned **BOUNCE** on item 4. The RETURN is at
`2ff40845:Plans/RETURN-2026-10-21-ot61-c05-review-chad.md` on `codex/ot61-c05-codex-review-chad`. Six
of the seven items passed: stubs, errno, panic-abort, no new syscall/NX, default-build identity, and no
POSIX surface.

**What was reported.** A method-qualified call `Gate.syscall3(41, 0, 0, 0)`, next to a second
`fn syscall3` method holding the asm, would pass G5. The stated mechanism was that `.` before
`syscall3(` counts as an identifier tail.

**What reproduced.** That exact fixture, and its `::`/`Self::` variants, were in fact already refused
by the old G5: `.` and `:` are not identifier bytes. But the review's underlying point was right. G3
pinned the asm only to "a function named `syscall3`", and probing found **five real bypasses** of the
old checker:
1. a function pointer (`let f = syscall3; f(41, ..)`);
2. a multi-instruction template (`"nop; syscall"`) outside any trampoline;
3. a `"//"` string literal that made the line-comment stripper drop the rest of the line;
4. a `/* fn syscall3 */` block comment faking a trampoline;
5. a second `fn syscall3` anywhere.

The bounce is therefore **upheld in substance**.

**The fix** (§4.2):
- a lexer;
- one brace-matched trampoline per trampoline file;
- `asm!` confined to it, with an exact template;
- every `syscall3` reference checked, under any qualification;
- `SYS_*` bindings checked;
- side doors refused;
- scripts barred from syscall and asm text;
- two xtask backstops: the inserted-arm scan, and the ELF syscall-site check.

Chad's fixture is committed verbatim as a negative test, alongside every bypass above.

## 6. CI evidence and review

- **Push-CI on `074a90ae`: GREEN.** Run [39650092060](https://github.com/SecondStageTurbine/LogansOS/actions/runs/39650092060).
  - Attempt 1: "Host Tests, Format & Lint" died in `rustup toolchain install` on a runner DNS failure,
    before any cargo step ran (infra).
  - Attempt 2 (one re-run of the failed job): [green](https://github.com/SecondStageTurbine/LogansOS/actions/runs/39650092060/job/119422801196).
    The job's steps: fmt, workspace tests, shard-coverage, clippy `-D`, no_std hygiene, kernel clippy,
    neo-test-gated kernel clippy.
  - Every other job is green: SPDX, Product Feature Build, Monolithic Identity Guard, and all 12
    Feature-Gated Coverage arms.
  - The QEMU shards were skipped by design, since they run only on `workflow_dispatch`/schedule.
- 4-shard `workflow_dispatch` QEMU gate: *PM-dispatched; URL recorded in the PM gate report.*
- Cross-family review: `Plans/DISPATCH-2026-10-21-ot61-c05-codex-review.md` (Chad; Codex family), RETURN
  on a `codex/` branch.
