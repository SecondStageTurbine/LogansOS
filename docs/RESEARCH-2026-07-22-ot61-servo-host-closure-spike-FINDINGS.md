<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# OT-61 Servo host-closure spike — FINDINGS

**Date:** 2026-07-22
**Lane:** Janus
**Verdict:** **PROCEED, but only as a deliberately bounded native OS port.** Do not attempt a four-crate `no_std` extraction, and do not introduce a general POSIX ABI.

## 0. Decision first

The viable closure is a **per-surface mix**:

1. Build a browser-private LogansOS host runtime comprising:
   - a real Rust `std` platform implementation for a first-class LogansOS user target;
   - a small C/C++ runtime and SpiderMonkey OS port;
   - capability-backed host objects for files, clocks, process lifecycle, and IPC.
2. Keep Servo, Stylo, html5ever, WebRender, and SpiderMonkey upstream-shaped. Patch only target selection and embedder/provider seams.
3. Replace upstream's ambient surfaces rather than emulating them:
   - no renderer `NetDialCap` and no raw socket construction;
   - no ambient filesystem root, current directory, executable discovery, or inherited environment;
   - no runtime dynamic loader in the first closure;
   - fonts, packaged resources, cache, and downloads arrive as explicit handles or broker channels.
4. Keep the C++ and POSIX-like implementation details **behind the browser bundle boundary**. The kernel/user ABI remains libneo capabilities and syscalls; neither web content nor general LogansOS programs receive a POSIX personality.

This is closer to strategy A than B, but “`std`-lite shim” understates it. Rust crates that import `std` need a supported `libstd` platform layer with correct thread, TLS, wait, clock, filesystem, socket, and process semantics. SpiderMonkey additionally needs a native platform port. A maintained fork remains necessary only at the capability-sensitive provider seams. Rewriting core DOM/layout/style code to `no_std` is a REFUTE.

### The first prototype

Build and boot a statically linked `servo-host-smoke` process on a new `x86_64-unknown-neoplutarch` target. It must:

- spawn and join two native threads;
- prove per-thread TLS isolation and destructor execution;
- wake and time out a `Mutex` + `Condvar` pair using a monotonic clock;
- allocate, free, and regrow heap/VM storage;
- read one sealed resource supplied by handle;
- parse a fixed HTML fragment with upstream html5ever and run one upstream Stylo selector/style unit;
- fail closed when it tries ambient root/open, environment enumeration, or network dial without a broker capability.

**Buildable now: no.** The repository has no user-thread syscall, user TLS contract, blocking wait primitive, general monotonic/realtime clock API, VM unmap/protect operation, or Rust `std` target PAL. Those are the precise precursor. A host-only `cargo check` would prove dependency resolution, not the target surface.

### Dependency-ordered build ladder

| Cycle | Closure increment | Gate that ends the cycle |
|---|---|---|
| C0 | First-class target, static CRT entry, `libstd` PAL skeleton | `std-pal-smoke` prints, allocates, exits; unsupported APIs fail deterministically |
| C1 | user threads, TLS, mutex/condvar wait, monotonic clock, VM map/unmap/protect, stack bounds | contention, timeout, TLS destructor, guard-page, and W^X negative batteries pass in QEMU |
| C2 | sealed file/resource handles, entropy, process identity/crash containment; browser gets no dial capability | `servo-host-smoke` above passes, including all three ambient-authority refusals |
| C3 | static C++ runtime + exact SpiderMonkey, initially interpreter/no-JIT | one realm evaluates JS, GC compacts, helper threads join, OOM/crash kills only the content process |
| C4 | Servo script/DOM + Stylo + layout closure, font/resource providers, broker channels | fixed HTML + CSS + JS mutation produces the expected display-list snapshot |
| C5 | paint backend contract | delegated to the SWGL sibling; software pixels reach an Atrium surface without direct scanout authority |

Every cycle has a go/no-go discriminator. Failure at C1 means the Servo track is not ready; it does not justify building a generic POSIX layer. Failure at C3 after C1 succeeds means reassess the SpiderMonkey port or an explicitly bounded WASI-derived implementation, not the DOM/layout closure.

## 1. Source pins and method

Claims below were checked against actual Git worktrees, not summaries:

| Source | Exact revision | Why this revision |
|---|---|---|
| Servo | `25b89d879cecef6e63203f70993ba22b5e7cd9ef` | revision pinned by the OT-61 cross-check |
| Stylo | `394aea737f0c9f85ee442898c29053de8abf68d8` | exact Servo workspace dependency at Servo `Cargo.toml:221,228,238-243` |
| rust-mozjs build port | `cd33b2687b39f365d043ce31ecf842c62ffa7df3` | `mozjs-sys` 140.12.0-2 source tag; Servo locks `mozjs` 0.20 and `mozjs-sys` 140.12.0-2 |
| SpiderMonkey / Firefox ESR source | `f8025617e815f21388b40baf189338d31a5f9a0a` | `FIREFOX_140_12_0esr_RELEASE`, matching the locked engine line |

The rust-mozjs source marker at `mozjs-sys/etc/COMMIT` is `7df86525c2c876c7c92320e49c3e0771f7a605c0`. Build-host Python, make, C/C++ compiler, and filesystem use are distinct from target runtime requirements: rust-mozjs invokes make at `mozjs-sys/build.rs:142-261`, chooses/links a C++ standard library at `:468-481`, and has a WASI target path at `:565-615`. Those tools run in CI; the resulting static engine still needs the runtime surfaces inventoried below.

RAG was healthy (`ollama=true`, `qdrant=true`; 16,732 indexed points) before discovery. Queries run as dispatched:

- `NetDialCap net dial socket egress`
- `libneo host services threads sync`
- `no_std target spec libneo`
- `atrium surface present seam`

RAG located the relevant LogansOS seams; all load-bearing claims were then verified in the files cited here.

## 2. Upstream host-demand inventory

This is not incidental test scaffolding. The useful engine closure is intrinsically hosted and concurrent.

### 2.1 Threads and TLS

- Stylo retains Rayon workers and join handles (`style/global_style_data.rs:75-78,202`) and uses traversal TLS (`style/bloom.rs:16`; `style/scoped_tls.rs:5-33`).
- Servo creates separate constellation, script, paint refresh, font, connector, and resource threads (`components/constellation/constellation.rs:618`; `components/script/script_thread.rs:489`; `components/paint/refresh_driver.rs:203-204`; `components/fonts/system_font_service.rs:89`; `components/net/connector.rs:451`; `components/net/resource_thread.rs:149`).
- Script rooting and ownership depend on native TLS (`components/script_bindings/root.rs:439`; `components/script/script_thread.rs:168`; `components/script/script_runtime.rs:1112,1130-1131`). This is correctness-critical GC state, not an optimization.
- Paint sizes a Rayon pool from `std::thread::available_parallelism()` and constructs it at `components/paint/painter.rs:209-214`. Net does the same for its async runtime at `components/net/async_runtime.rs:49`.
- SpiderMonkey's POSIX port creates, joins, detaches, names, and identifies pthreads in `js/src/threading/posix/PosixThread.cpp:39-131`. Its mutex ownership checker itself uses TLS (`js/src/threading/Mutex.cpp:13-16`; `Mutex.h:58`).

### 2.2 Synchronization and channels

- Constellation multiplexes crossbeam receivers and an IPC router (`components/constellation/constellation.rs:102,120-121`); script uses both crossbeam and `ipc_channel` (`components/script/script_thread.rs:36,53`). The refresh driver uses a timeout-capable crossbeam receiver (`components/paint/refresh_driver.rs:12,203`).
- Stylo keeps a `parking_lot` mutex/RwLock-protected global pool (`style/global_style_data.rs:13,61,75,131`).
- SpiderMonkey's Mozilla platform layer maps mutexes to pthread mutexes (`mozglue/misc/Mutex_posix.cpp:34-102`) and condition variables, including monotonic timed waits, to pthread condition variables (`mozglue/misc/ConditionVariable_posix.cpp:25-39,68-151`).

Correctness requires blocking waits with wake-one/wake-all, timeout, acquire/release ordering, thread exit cleanup, and no lost wakeups. A spin mutex alone is not a substitute.

### 2.3 Heap, virtual memory, stack, and executable memory

- Servo and Stylo use normal `Box`, `Arc`, `Vec`, hash maps, allocator-aware image/font libraries, and malloc-size accounting. Servo's layout manifest directly depends on `malloc_size_of`, `rayon`, `parking_lot`, script, Stylo, and WebRender API (`components/layout/Cargo.toml:25-63`).
- SpiderMonkey GC reserves, commits, discards, unmaps, and changes protection on page-aligned regions (`js/src/gc/Memory.cpp:236-269,889-948,1057-1126`). JIT memory separately uses `munmap` and `mprotect` (`js/src/jit/ProcessExecutableMemory.cpp:534,581,604,984-991`).
- Servo already recognizes that SpiderMonkey will crash if JIT memory cannot be protected, and probes `mmap`/`mprotect` on OHOS (`components/script/init.rs:63-129`). This port should start with JIT disabled; GC still needs reclaimable VM and protection/guard semantics.
- SpiderMonkey also needs native stack bounds; its fallback probes libc state with `dlsym` (`js/src/util/NativeStack.cpp:77`). LogansOS should provide bounds directly at thread creation rather than reproduce that discovery trick.

### 2.4 Files, resources, cache, and fonts

- Servo's net component opens files asynchronously (`components/net/filemanager_thread.rs:5,701,871`) and its file protocol opens local files (`components/net/protocols/file.rs:5`).
- hosts-file discovery reads an environment-selected path (`components/net/hosts.rs:7-21`). This must be removed, not emulated.
- paint capture uses current/temp directories and creates directories (`components/paint/paint.rs:7-11,783-788`). Capture is a developer feature and can be compile-disabled on target.
- system fonts run behind a service thread (`components/fonts/system_font_service.rs:89`); the FreeType provider owns a process-global synchronized library (`components/fonts/platform/freetype/library_handle.rs:19,84-108`).

The browser requires packaged UA resources, font bytes, cache/cookie storage, selected upload/download files, and perhaps developer captures. It does **not** require an ambient namespace. Each category can be a sealed resource bundle, a delegated directory/file handle, or a broker protocol.

### 2.5 Network

- Servo's HTTP loader is Hyper/Tokio-based and measures connection/send events (`components/net/http_loader.rs:32-65,433-447,592-639`). Its async runtime and connector assume hosted threads.
- LogansOS already has the intended authority primitive: `sys_net_dial(cap, name, port)` at `userland/libneo/src/lib.rs:1352-1364`, backed by NetDialCap policy in `kernel/src/sched/net_dial_cap_logic.rs`.

The closure line is structural: the **network broker** holds a per-origin derived NetDialCap; renderer/script/layout processes hold only a request channel. Do not implement `std::net::TcpStream::connect` as an ambient direct syscall for those processes. A PAL implementation must return permission denied when no explicitly installed broker/provider exists.

### 2.6 Time and clocks

- Stylo traversal metrics use `Instant` (`style/context.rs:40,385-404`; `style/driver.rs:17,104`).
- Servo uses monotonic instants for document events, refresh timing, cache lifetimes, and network timing, and `SystemTime` for cookies and HTTP dates (`components/net/cookie.rs:10,38-40,93,276-287`; `components/net/http_loader.rs:10,592-639`).
- SpiderMonkey's POSIX timestamp layer uses monotonic and realtime clocks (`mozglue/misc/TimeStamp_posix.cpp:91,184-189,316`) and condition-variable deadlines use a monotonic clock.

The platform therefore needs separate monotonic and civil/realtime clocks with defined resolution and nondecreasing monotonic behavior. Kernel uptime counters are not a complete `std::time` contract.

### 2.7 Process, environment, and failure

- Servo's existing multiprocess sandbox path discovers its executable, creates child commands, forwards selected environment variables, and exits (`components/constellation/sandboxing.rs:145,165-166,233-264`). Servo core also uses process ID and exit (`components/servo/servo.rs:119,1473`).
- LogansOS `sys_spawn` launches a named, statically supplied user module with explicit inherited handles (`userland/libneo/src/lib.rs:801,977-991`; `kernel/src/sched/process.rs:1648`). It is not `fork`/`exec`.

Replace Servo's command construction with an embedder process factory: a fixed content-module identity plus an explicit handle manifest. Environment is an immutable allowlisted key/value object built by the launcher, not ambient kernel state. Process ID can be a non-authoritative diagnostic token. Exit/crash maps to process containment and parent notification.

### 2.8 Dynamic loading

Dynamic loading is not required for the minimum browser. Servo's media path discovers GStreamer plugins beside the executable (`components/servo/servo.rs:95-119`), and SpiderMonkey's native-stack fallback uses `dlsym` (`js/src/util/NativeStack.cpp:77`). Compile media plugins out, link FreeType/ICU/SpiderMonkey/SWGL statically, and provide stack bounds explicitly. A target `dlopen`/`dlsym` implementation is therefore **out of scope for the first closure**.

### 2.9 GL device/context contract

Servo paint owns a `RenderingContext`; it asks that context for `Rc<dyn gleam::gl::Gl>` and passes it to WebRender (`components/paint/painter.rs:67-78,107-108,153-154,222-225`). Servo pins `gleam = 0.15`, `surfman = 0.13.0`, and WebRender 0.70 (`Cargo.toml:103,244,279-280`).

That interface is the right provider seam. It is **not** evidence that the browser should own a GPU or scanout capability. The SWGL sibling must implement the software provider and publish pixels through an Atrium client surface. Hardware GL later replaces the provider behind the same paint contract. Detailed GL/SWGL closure is intentionally deferred to the sibling FINDINGS.

### 2.10 Panic and unwind

- Servo uses unwind as a local recovery boundary for SVG image processing (`components/net/image_cache.rs:49,1117`) and generated JS bindings can resume a SpiderMonkey-recorded unwind (`components/script_bindings/import.rs:20`). Layout tests also use hooks and catch-unwind, though test-only use does not define the shipped target.
- The current LogansOS target sets `panic-strategy: abort` (`userland/x86_64-neoplutarch-user.json:10`).

Initial target policy should remain abort across the C++ boundary: a panic or MOZ crash terminates the confined content process and reports to the constellation/embedder. Patch the few recovery catches to return explicit errors or disable the optional path. Do not unwind Rust through C++. Supporting Rust unwind can be reconsidered only after a QEMU gate proves personality routines, TLS destructors, and FFI boundaries; it is not a C0 dependency.

## 3. Per-surface LogansOS gap table

| Surface | Upstream runtime need | Present LogansOS provision | Gap | Closure and grade |
|---|---|---|---|---|
| Threads | create/join/detach, named threads, stack sizing/bounds, parallelism count | kernel scheduler and process model exist; user process snapshot exposes `thread_count` | no user thread lifecycle ABI; current libneo heap comment says userspace is single-threaded | add cap-confined in-process thread syscalls and explicit stack/TLS bootstrap. **Required / high difficulty** |
| TLS | Rust `thread_local!`, destructors, SpiderMonkey/Mozilla TLS, GC roots | none for user thread-local storage; `libneo::tls` is network TLS | define target TLS ABI, per-thread TLS block, destructor run at exit, compiler/runtime hooks. **Required / high** |
| Sync/waits | mutex, RwLock, condvar, channels, timed waits | kernel has internal `SwitchMutex`; libneo offers process channels; heap uses spin mutex | no userspace futex/wait-on-address or condvar primitive | add bounded wait/wake key scoped to process plus monotonic timeout; build Rust/C++ sync on it. **Required / high** |
| Heap/VM | malloc/free, aligned allocation, reserve/commit/reclaim/protect, guard pages | `LibneoHeap` alloc/free over monotonic RW+NX `sys_map_region` (`libneo/src/lib.rs:1768-1900`) | maps persist until exit; 16 MiB call ceiling; no unmap/protect/reserve; contiguous-growth assumption | retain allocator for C0; add tracked VM regions with unmap/protect and W^X. **Required before SpiderMonkey / high** |
| Files/resources | packaged resources, fonts, cache, uploads/downloads | NeoFS file handles/rights and open/read/write/readdir wrappers (`libneo/src/lib.rs:872-929`) | `std::fs` path model is ambient; no preopen-aware PAL or browser resource broker | sealed bundles + delegated handles + cache broker; target `std::fs` accepts only installed preopens. **Required / medium** |
| Network | DNS, HTTP/TLS, async sockets, per-origin policy | sockets plus NetDialCap derive/dial (`libneo/src/lib.rs:1337-1405`) | Hyper/Tokio adapter; risk of direct socket bypass | network broker owns NetDialCap; custom AsyncRead/Write transport; deny ambient `std::net`. **Provision exists, adapter medium** |
| Time | monotonic, realtime, sleep, timeout | sysstat/observability expose uptime/ticks (`libneo/src/lib.rs:252,1042-1068`) | no stable clock/sleep API with conversion/resolution contract | monotonic/realtime syscalls or clock page; timed wait uses monotonic. **Required / medium** |
| Process | fixed content workers, exit/wait/crash, identity | named static `sys_spawn`, explicit handle inheritance, kill/wait/inspect (`libneo/src/lib.rs:977-1040`) | no Servo process-factory adapter; no `Command`; no thread/process distinction in upstream path | replace at embedder seam with fixed-module spawn manifest. **Good fit / medium** |
| Environment | limited logging/config/prefs | no ambient environment API | upstream probes current exe, dirs, variables | immutable launcher-supplied allowlist; everything else absent. **Fork/provider seam / low** |
| Dynamic loading | optional media/font/plugin discovery; stack fallback | none | no loader/relocations/symbol namespace | static link; compile optional plugins out; direct stack bounds. **Not required / kill for first closure** |
| GL context/device | `RenderingContext` + `gleam::gl::Gl` for WebRender | Atrium surfaces/buffers/present exist; no browser GL provider | WebRender GL provider and pixel handoff | SWGL provider sibling; browser commits client buffer, never scanout. **Required for pixels / sibling** |
| Panic/unwind | some catches, panic hooks, SpiderMonkey error transport | abort-only target and process exit | no Rust unwinder/personality contract | abort confined process; patch recovery sites to explicit result. **Viable / medium** |
| C/C++ runtime | atomics, threads/TLS, clocks, libc allocation/string/math, stack, crash | libneo is Rust syscall wrappers; no target libc/libc++ | SpiderMonkey cannot link/run directly | private static CRT/libc++ subset implemented atop the same native PAL, no exported POSIX ABI. **Required / high** |

## 4. Strategy A versus B

### A. Bounded hosted runtime

**Viability: PROCEED.** It matches the intrinsic concurrency and native dependencies rather than fighting them. It also amortizes the hard correctness work across unmodified Rust dependencies.

**Charter fit: conditional.** It fits only if:

- it links into browser-bundle processes rather than becoming the public application ABI;
- every resource-producing operation is backed by an installed capability/preopen/provider;
- unsupported ambient operations fail closed;
- renderer/content manifests structurally omit NetDialCap, root FileCap, GpuSubmitCap, and direct present/scanout authority;
- tests prove those negative properties.

**Blast radius:** toolchain target/PAL, a small native runtime, and new kernel primitives for threads/waits/VM/clocks. These are deep seams, but sharply enumerable. The dangerous outcome is a growing Linux-compat island. Prevent it with a checked surface manifest and a rule that adding a syscall-shaped compatibility entry requires its capability provenance and negative gate in the same change.

### B. Rewrite/strip fork

**Viability: REFUTE for core engine code.** Threads, TLS, waits, time, heap, and process separation cut across Stylo, DOM bindings, GC rooting, paint, networking, and many transitive crates. Removing them is a second browser engine, not a port.

**Maintenance blast:** every Servo refresh would re-touch script-thread construction, generated bindings, GC-root TLS, layout parallelism, crossbeam/ipc plumbing, net runtime, and paint. Every SpiderMonkey ESR refresh would re-touch threading, GC VM, time, and stack platform code. Those are exactly the high-churn/high-correctness surfaces to avoid owning.

### The maintained patch line

Fork only these narrow areas:

- add/register the LogansOS Rust target and SpiderMonkey platform target;
- Servo embedder process factory replacing `Command` and environment discovery;
- network transport provider replacing ambient connect;
- sealed resource/font/cache providers replacing path discovery;
- GL `RenderingContext` provider;
- compile-time disabling of GStreamer/plugin discovery and other unsupported optional features;
- explicit abort/error behavior at the few unwind-recovery sites.

Do **not** fork html5ever parsing, Stylo matching/cascade, layout algorithms, DOM implementation, generated WebIDL bodies, WebRender display-list semantics, or SpiderMonkey GC/JS semantics. Prefer target-cfg patches suitable for upstreaming. Pin and re-audit the patch inventory on every Servo update and SpiderMonkey ESR update; a gate fails if new host calls appear outside the manifest.

## 5. Target mechanics

The current custom JSON is an `x86_64-unknown-none` static, aborting target. It tells LLVM how to emit code; it does not supply `std` implementations. `-Z build-std=std,panic_abort` will compile only after Rust knows how `sys::pal` implements this OS and after the required libc/runtime symbols exist.

The concrete route is:

1. Introduce a first-class target triple such as `x86_64-unknown-neoplutarch` with a distinct `target_os`, static relocation, abort initially, and the current no-redzone/data-layout constraints.
2. Carry a small Rust toolchain patch adding a LogansOS `library/std/src/sys/pal/neoplutarch` implementation. Reuse semantics and tests from an unsupported bounded target such as Hermit or WASI where they match, but call libneo/native runtime functions—not POSIX symbols merely for naming convenience.
3. Implement thread/TLS/wait/clock/VM primitives below both Rust `std` and the private C++ runtime. One implementation must define the semantics; avoid separate Rust and C++ schedulers.
4. Cross-build Servo and SpiderMonkey statically in the Linux CI container. Build scripts may use host Python/make/filesystem. Target artifacts may not assume those facilities.
5. Link a fixed browser module image launched by `sys_spawn` with an explicit handle manifest. There is no `fork`, runtime linker, PATH, cwd, or inherited environment.
6. Maintain a machine-readable allowlist of PAL operations. Each operation declares authority source, target process roles allowed to call it, and a negative test.

An alternative WASI route is useful as source material, not as an answer by itself. rust-mozjs already recognizes `wasm32-wasip1`, but Servo's native threading/TLS, process partitioning, native C++ engine, and SWGL path still need a capable host. Importing WASI wholesale would also obscure the capability provenance that OT-61 requires.

## 6. Go/no-go contract and maintenance gates

### Go if

- C1 proves correct threads/TLS/waits/VM under contention and failure injection;
- the browser-specific PAL remains a finite audited surface;
- C2 proves all ambient-authority negatives;
- C3 runs exact pinned SpiderMonkey with no JIT and process-confined failure;
- upstream deltas stay confined to the patch line above.

### Stop or redesign if

- `std` compatibility requires global filesystem, unrestricted sockets, fork semantics, or a loader exposed to content;
- SpiderMonkey cannot run without executable writable memory despite the no-JIT configuration;
- GC requires VM semantics the kernel cannot provide without weakening W^X or memory ownership;
- Servo refreshes repeatedly force changes in core DOM/layout/style rather than provider/platform seams;
- the renderer obtains NetDialCap, GpuSubmitCap, root FileCap, or a direct present/scanout handle.

### Per-refresh audit

For each pinned upstream refresh:

1. diff the target/platform patch set;
2. regenerate the host-call inventory;
3. fail on a new ambient path, environment, socket, process, loader, or executable-memory call;
4. run PAL semantic tests and capability-negative tests;
5. run one DOM/style/layout display-list fixture;
6. after the sibling lands, run one software-render pixel fixture through Atrium.

This measures the port by dependency depth and verification cycles. No calendar or staffing estimate is needed to decide feasibility.

## 7. Explicit divergences from prior browser studies

1. **Divergence from the original small-subset framing:** useful Servo layout is not separable as four `no_std` crates. `servo-layout` directly depends on script, fonts, net traits, paint API, Stylo, Rayon, and WebRender API. Chad's refutation is confirmed.
2. **Tightening of “bounded `std`-lite”:** it must be a real target PAL plus a native C++ port, not a libneo crate that shadows a few `std` names.
3. **No direct renderer network adapter:** even though LogansOS already has socket syscalls, the renderer does not receive the capability. The broker boundary is structural.
4. **No JIT in the first SpiderMonkey gate:** GC VM closure is already substantial; executable memory is a later, separately authorized decision.
5. **No dynamic loader in the first closure:** optional plugin/media discovery is compiled out and core native dependencies are static.
6. **GL is a provider, not authority:** the browser paint closure supplies pixels to Atrium; it never gains direct scanout. The exact SWGL contract belongs to the sibling spike.

## 8. Bottom line

Servo remains viable on LogansOS, but the gating artifact is an OS port, not a crate-selection exercise. The clean-slate advantage is that this port can be **less ambient than Unix**: upstream engine code receives familiar thread/time/allocation semantics while all authority-bearing operations terminate in explicit LogansOS capabilities and brokers. That is new infrastructure worth building only if C0-C3 stay finite and their negative gates remain structural.
