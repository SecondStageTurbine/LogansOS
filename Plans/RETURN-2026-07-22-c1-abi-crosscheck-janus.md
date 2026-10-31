<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: C1 host-runtime ABI cross-model REFUTE (Janus)

**Date:** 2026-07-22

**Scope:** doc-only cross-check of the C1.1 futex/thread and C1.2 TLS freeze; no build, hardware, or C1.0 gate

**Verdict:** **REFUTE the ABI as currently frozen.** The five *primitive families* can remain, but P2 and P3 have real consumer requirements that the stated contracts do not satisfy. P1 converges at the kernel boundary after two freeze clarifications.

## Decision table

| Target | Verdict | Freeze impact | Required amendment |
|---|---|---|---|
| P1 futex | **CONVERGES** | C1.1 contract clarification; no new kernel primitive | Make the timeout an unambiguous monotonic absolute deadline, and include `RwLock`/`pthread_rwlock_*` in the userland surface and battery. |
| P2 thread/stack | **DIVERGES** | **Redo-forcing C1.1 freeze bug** | Expose the initial and created threads' usable stack base + extent back to userland, preferably through `thread_get_info`. |
| P3 TLS model | **DIVERGES** | **Redo-forcing C1.2 unless key-only is frozen explicitly** | Either force and gate a key-only Rust/C++ build, or build the static `PT_TLS` template/TCB path. An FS-base setter alone does not implement compiler TLS. |
| P3 destructors | **DIVERGES** | **Unconditional redo-forcing C1.2 semantic bug** | Implement POSIX destructor rounds, including a destructor that re-establishes a value. |
| P5 no-JIT bonus | **CONVERGES** | C3 configuration/gate note | Force the no-JIT build/backend-off path; that path does not initialize executable memory, native regexp, or wasm codegen. |

The result is not a sixth host-service family. P2 needs one more operation within the thread family; P3 can remain one kernel FS-base operation if the key-only contract is made structural. The claim that P3 serves both consumers "identically/verbatim" is nevertheless false.

## P1 — CONVERGES: wait-on-u32 + wake-N is sufficient

The exact consumers tolerate the semantics a bare private futex can provide:

- Pinned Rust `std` builds its mutex entirely from an atomic state word, `futex_wait`, and wake-one; every wake returns to a retry loop (`rust e8e4541ff:library/std/src/sys/sync/mutex/futex.rs:31-65,89-101`). Its condvar uses a notification sequence word, snapshots it before unlocking, waits only while that expected value remains, and then re-locks (`.../condvar/futex.rs:7-10,22-54`).
- Mozilla explicitly permits spurious condvar wakes and requires callers to recheck their predicate (`Firefox f8025617:mozglue/misc/PlatformConditionVariable.h:40-49`). Its POSIX implementation selects `CLOCK_MONOTONIC`, converts the relative duration to one absolute deadline, and accepts only success or timeout (`mozglue/misc/ConditionVariable_posix.cpp:30-33,68-84,110-151`).
- Mozilla's actual mutex variants are default, debug `PTHREAD_MUTEX_ERRORCHECK`, and an optional adaptive policy (`mozglue/misc/Mutex_posix.cpp:35-84,93-121`). Owner-ID/error checking and adaptive spinning are userland policy over the same futex; they do not require PI, robust-owner-death, requeue, or a fairness syscall.
- The synthesis omitted a real third synchronization surface: non-Windows Mozilla builds `RWLock_posix.cpp` (`mozglue/misc/moz.build:126-143`), which requires the ordinary pthread read/write lock operations (`mozglue/misc/RWLock_posix.cpp:18-63`). Rust's implementation proves the same two futex operations suffice: its state + writer-notify words wait at `rwlock/futex.rs:94-160,172-260` and wake one/all at `:264-325`.

**Amend before C1.1:** spell the kernel argument as `deadline_monotonic_ns`, use `u64::MAX` for infinite, and let `0` mean already expired. This matches Rust's one-time monotonic absolute conversion and absolute futex wait (`rust e8e4541ff:library/std/src/sys/pal/unix/futex.rs:24-40,70-92`) and avoids the current contradiction between "monotonic deadline" and the ambiguous `timeout_ns /* 0 = infinite */`. Add pthread/Rust RW-lock contention to the userland battery. Preserve the returned wake count; Rust uses it when deciding whether to fall back from a writer to readers (`rwlock/futex.rs:318-329`).

No exact SpiderMonkey-relevant use of process-shared, robust, PI, requeue, strict FIFO fairness, or pthread cancellation was found. Those are C2/C3 notes only if a later pinned consumer appears, not speculative C1 blockers.

## P2 — DIVERGES: creation input is not stack-bounds discovery

The frozen `thread_create(entry,arg,stack_size,guard) -> tid` lets the kernel choose the mapping. `thread_self() -> tid` then gives the running thread no way to recover that mapping. SpiderMonkey needs it:

- On a POSIX worker, `GetNativeStackBaseImpl` calls `pthread_getattr_np(pthread_self())`, obtains the lowest address and length through `pthread_attr_getstack`, fatally asserts both calls, and returns the high end for downward-growing stacks (`Firefox f8025617:js/src/util/NativeStack.cpp:93-115`).
- `JSContext::init` stores that base (`js/src/vm/JSContext.cpp:116-118`), and `JS_SetNativeStackQuota` derives its native stack limit from it (`js/src/jsapi.cpp:1526-1536`). Helper-thread profiler registration also passes `GetNativeStackBase()` (`js/src/vm/InternalThreadPool.cpp:255-266`).
- The glibc main-thread fallback reads `__libc_stack_end` through `dlsym` and fatally asserts its presence (`js/src/util/NativeStack.cpp:67-90`), which is not available in the declared static/no-runtime-loader port.

The dispatch called this a conservative GC stack scan. I did **not** find that claim at the cited ESR seam: the demonstrated consumers are stack-limit enforcement and optional profiler registration. That correction does not remove the requirement; context initialization still needs the value and currently has no viable LogansOS source.

**Amend before C1.1:** add `thread_get_info(self-or-same-process-tid, out)` (or an equivalent stack-bounds call) returning at least `{tid, usable_low, usable_len, guard_low, guard_len}` with explicit downward-stack semantics. Populate it for the initial process thread as well as every `thread_create`; define validity through thread exit/reap; bind private-CRT `pthread_getattr_np`/`pthread_attr_getstack` to it. The kernel must retain this metadata: the current `Thread` carries no user-stack bounds (`kernel/src/sched/thread.rs:87-125`), and `Thread::new_user` accepts only `user_rsp` (`:309-315`).

The child runtime entry must be a TLS-free bootstrap trampoline: record/query bounds, install the C1.2 TCB/key block and per-thread errno, then call the application start routine. The exact engine does not require caller-supplied stacks or detach-at-create: it sets only stack size (`PosixThread.cpp:49-57`) and detaches later (`:75-79`). No `pthread_cancel` use was found. Per-thread `errno` is real in no-JIT GC VM code (`js/src/gc/Memory.cpp:260-271,894-907,1103-1108`), but a TCB slot serves it; it is not another thread syscall.

## P3 — DIVERGES: native TLS is reachable, and key destruction needs rounds

### Native compiler TLS versus key-only TLS

Core no-JIT SpiderMonkey code contains production TLS declarations. `MOZ_THREAD_LOCAL` expands to compiler-native `__thread` whenever `HAVE_THREAD_TLS_KEYWORD` is defined; pthread keys are only its fallback (`Firefox f8025617:mfbt/ThreadLocal.h:145-169,238-250`). Unguarded examples include `TlsContext` (`js/src/vm/Runtime.cpp:58`) and `TlsGCContext` (`js/src/gc/GC.cpp:291`). GC initialization initializes and sets the latter (`GC.cpp:928-940`), while process initialization initializes `TlsContext` before `InitializeJit` (`js/src/vm/Initialization.cpp:147-170`): disabling JIT does not remove this TLS.

The normal configure path does not know the LogansOS loader limitation. It defines `HAVE_THREAD_TLS_KEYWORD` from a link-only `static __thread` probe for all but a short exclusion list (`build/moz.configure/toolchain.configure:2472-2501`). A static ELF link can therefore succeed and carry TLS even though the runtime cannot instantiate it. Rust has the same fork: pinned `std` selects native TLS under `target_thread_local`, otherwise its heap/key implementation (`rust e8e4541ff:library/std/src/sys/thread_local/mod.rs:1-8,26-47`); the native macro emits actual `#[thread_local]` statics (`.../native/mod.rs:56-119`). The pinned Rust mozjs wrapper has production `thread_local!` state for the current context and GC root set (`rust-mozjs cd33b268:mozjs/src/rust.rs:161,311-317,372-382`; `mozjs/src/gc/trace.rs:25-32,39-70`).

An arbitrary block installed as FS.base serves pthread keys only if the private key runtime defines how to find that block. It does not serve compiler-assigned FS-relative offsets. The current loader explicitly excludes `PT_TLS` (`kernel/src/loader/elf.rs:12-13`) and skips every non-`PT_LOAD` program header (`:151-162`); it therefore never obtains the TLS template's file size, memory size, or alignment and never builds a private `.tdata`/`.tbss` instance for each thread.

**Preferred C1.2 amendment — make the intended key-only route structural:** keep Rust `has-thread-local=false`, change `singlethread` to false, replace the C0 overlay's forced `no_threads` route with a Neoplutarch OS-key/PAL route, force Mozilla `HAVE_THREAD_TLS_KEYWORD` off, and reject any final ELF containing `PT_TLS` or `SHF_TLS`. Define the private FS-based TCB/key-block locator without `__thread`, install it in the bootstrap trampoline before std/CRT/engine code, and reserve an errno cell. This lets full native TLS defer to C2/C3.

**Alternative:** if native TLS is admitted in C1.2, parse `PT_TLS`, copy `p_filesz`, zero through `p_memsz`, honor `p_align`, construct the x86-64 static TLS/TCB layout for the initial and every child thread, and install FS before its first user instruction. A static/local-exec-only contract can still defer a dynamic loader/DTV/general TLS relocations. No extra syscall beyond the FS-base operation is inherently required, but the loader and thread bootstrap work is required.

### Iterated key destructors

The userland-key design also understates exit semantics. Pinned Rust `std` creates keys with destructors (`rust e8e4541ff:library/std/src/sys/thread_local/key/unix.rs:23-31`). Its value destructor explicitly schedules runtime cleanup for a later destruction round (`.../os.rs:222-242`). On the key-based path, the cleanup callback first re-establishes its key as `RUN`, then calls `thread_cleanup` only when invoked in the next round (`.../guard/key.rs:33-57`). A single "run each registered destructor before exit" pass, and the proposed "destructor runs exactly once" battery, cannot satisfy that consumer.

**Amend before C1.2:** at exit, clear each non-null slot before calling its destructor; rescan keys when a callback establishes a new value; repeat up to the private runtime's `PTHREAD_DESTRUCTOR_ITERATIONS` contract (at least the POSIX four-pass behavior); then stop and enter `thread_exit`. Key deletion/reuse needs a generation check so a stale destructor cannot target a recycled key. Add (1) a destructor that re-arms its key and is called in a later pass and (2) a Rust cleanup-order test. "Exactly once" remains a valid battery only for a destructor that does not repopulate its slot.

## P5 bonus — CONVERGES under an enforced no-JIT configuration

The exact no-JIT path does not expose a hidden executable-memory consumer:

- rust-mozjs adds `--disable-jit` when its `jit` feature is absent (`rust-mozjs cd33b268:mozjs-sys/makefile.cargo:14-16`). Servo can also call `DisableJitBackend` before engine setup when `js_disable_jit` is true (`Servo 25b89d8:components/script/init.rs:162-175`).
- SpiderMonkey initializes process executable memory only when `HasJitBackend()` is true (`Firefox f8025617:js/src/jit/JitContext.cpp:118-131`). Native regexp is also gated by that predicate (`js/src/vm/RegExpShared.h:50-52`), and wasm reports no platform support without it (`js/src/wasm/WasmFeatures.cpp:245-252`).

This is a C3 configuration obligation, not a C1 divergence: omit the rust-mozjs `jit` feature (or force `DisableJitBackend` before `JS_Init`) and gate that the no-JIT smoke makes no executable-memory request. Baseline/native-regexp/wasm codegen and executable-memory support remain a later rung.

## Freeze ranking

1. **C1.1 redo-forcing:** add stack metadata + a self/same-process query before cutting thread syscalls. Creation-time stack size alone is insufficient.
2. **C1.2 redo-forcing:** implement iterated key destructors and replace the exactly-once-only battery.
3. **C1.2 redo-forcing unless constrained now:** freeze and artifact-gate key-only Rust + Mozilla TLS. Otherwise implement static `PT_TLS`/TCB initialization in C1.2.
4. **C1.1 freeze clarification, no new primitive:** make futex deadline/sentinel semantics exact; add RW locks to the private pthread/PAL surface and tests.
5. **C3 notes:** per-thread errno in the private TCB; no-JIT build/backend gate; process-shared/robust/PI/requeue/fairness only if a future pinned consumer proves a need. Full native/dynamic TLS may defer if the C1.2 key-only negative gate lands.

## Source/method boundary

Primary sources were read at Firefox ESR `f8025617e815f21388b40baf189338d31a5f9a0a`, rust-mozjs `cd33b2687b39f365d043ce31ecf842c62ffa7df3`, Servo `25b89d879cecef6e63203f70993ba22b5e7cd9ef`, and the `nightly-2026-04-16` manifest's Rust commit `e8e4541ff19649d95afab52fdde2c2eaa6829965`. RAG was healthy and used for discovery; every load-bearing claim above was checked in raw source. No build or runtime gate was run.
