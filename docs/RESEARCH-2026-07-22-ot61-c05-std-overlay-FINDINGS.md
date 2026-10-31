<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# OT-61 browser C0.5 — the rust-src std overlay + a real-std ELF on-target — FINDINGS

**Date:** 2026-07-22
**Lane:** Cerebro (Claude family; C0 + this rung's author)
**Status:** **C0.5 BUILT — clean runtime PASS.** A REAL `std`-linked ELF (real `std::println!` / `Vec` / `String` / `format!` / `std::process::exit`) compiles with `-Z build-std=std`, links to a static ELF, and **runs on-target in QEMU** through the existing `sys_spawn`/ELF-loader/process path. The first real-`std` ELF through that path — and it ran clean (no loader gap, no loud-stub fire, no TLS fault).
**Builds on:** ADR-078 §3 (C0.5) + the C0 record (`docs/RESEARCH-2026-07-22-ot61-c0-libstd-pal-FINDINGS.md`) + the PAL manifest (`tools/ot61-pal-manifest.toml`).

---

## 0. Result

C0.5 closes the C0 gate at the **runtime tier**: the C0 primitive-tier smoke stood on raw syscalls; this one uses **actual `std`**. The QEMU serial log is the proof (non-vacuous — the prints show real std ran, not just an exit code):

```
limine: Loading module boot():/modules/neo-std-real-smoke
[ kernel ] elf: std-real-smoke module ... size=6575128 bytes      # a 6.5 MB real-std ELF
[ kernel ] proc: spawned 'std-real-smoke' (pid=1) entry=0x2119a0
[ std-real-smoke ] std::println!: ok                              # real std::println!
[ std-real-smoke ] std heap Vec+String: len=1024 checksum=0x74621000   # real Vec+String+format!
[ std-real-smoke ] PASS: std println+alloc+exit
[ kernel ] proc: std-real-smoke exit code 0                       # clean std::process::exit(0)
NEO_TEST/1 event=PASS name=std_real_smoke ... i_am=real_std opens=c0.5
```

**The dispatch anticipated a possible loader/TLS/loud-stub finding; the finding is the opposite — it runs clean.** The reason the loader's known gaps didn't bite is *designed-in*, not luck (§3).

---

## 1. The bounded overlay (what it actually took)

`userland/neoplutarch-std-overlay/apply-overlay.sh` applies a **minimal** `neoplutarch` arm to the toolchain's rust-src `library/std/src/sys/*`, idempotently, reproducible in the ephemeral CI/close-gate container. The surface is exactly what C0 enumerated — **not larger**. Most `sys/*` modules already carry a `_ => unsupported` fallback (auto-loud); the overlay touches only:

| Seam | Overlay action | Why |
|---|---|---|
| `sys/stdio` | custom `neoplutarch.rs` → `SYS_WRITE` (fd 1/2) | the `_` arm is a *silent* `unsupported` (drops writes); real printing needs the override |
| `sys/exit` | `neoplutarch` arm → clean `SYS_EXIT` (anchored on `fn exit`, not `unique_thread_exit`) | the `_` arm is `intrinsics::abort()` (UD2 fault, not a clean exit) |
| `sys/io/error` | arm → the `generic` (no-OS-errno) impl | no `_` fallback; needs `decode_error_kind`/`errno`/`error_string`/`is_interrupted` |
| `sys/alloc` | custom `neoplutarch.rs` → `System` via `SYS_MAP_REGION` (leaky, **unused** — the smoke sets `#[global_allocator]`) | no `_` fallback; `System` must type-check |
| `sys/thread_local` | add `neoplutarch` to the **`no_threads`** (single-threaded statik) list | avoids the OS-key path entirely — matches `singlethread=true`/`has_thread_local=false`; **this is why there's no TLS at runtime** |
| `sys/random` | arm → `unsupported::fill_bytes` | the `_` arm is empty; `fill_bytes` is compiled-but-uncalled (no HashMap in the smoke) |

Syscalls the overlay issues: **`SYS_WRITE`, `SYS_EXIT`, `SYS_MAP_REGION`** — all three in the A4 PAL manifest. Two consumer-side flags complete it: `#![feature(restricted_std)]` (std's opt-in for an unofficial target) and `build-std-features = ["compiler-builtins-mem"]` (the `mem*` intrinsics). **Loud-absence preserved:** every `sys/*` the smoke doesn't use (`fs`, `net`, `process::Command`, `thread`, `sync`-beyond-single, `time`, …) stays a deterministic `unsupported` stub — the overlay implements only what links.

---

## 2. The real-std smoke + the on-target gate

`userland/std-real-smoke` (`#![no_main]` + `#![feature(restricted_std)]`, `#[global_allocator]` = `neo_std_pal::SyscallHeap`) uses real `std::println!`, a heap `Vec<u32>` + `String` via `format!`, and `std::process::exit(0)`, from a bare `_start` (the loader gives no argv, so no `rt::lang_start`; stdout initializes lazily). It builds to a **static EXEC ELF: 3 LOAD segments, no `PT_TLS`, no `PT_INTERP`**.

The `std_real_smoke` QEMU scenario (`neo-test-std-real-smoke`) has the **kernel `spawn_std_real_smoke`** load `/modules/neo-std-real-smoke` and spawn it through the same `process::spawn_user`/reap path as `neo-init`, PASS iff it exits 0. xtask applies the overlay + builds it (`-Z build-std=std`) + stages it; `limine.conf` loads it. **This is the first real-`std` ELF through the existing spawn/ELF-loader path — a genuine integration test of that path, and it passed.**

---

## 3. Why the loader's known gaps didn't bite (the integration finding)

The ELF loader handles **only `PT_LOAD`** — no `PT_TLS`, no FS-base setup, no argc/argv/auxv init stack (a bare `_start` contract, `kernel/src/loader/elf.rs`). A naïvely-built std ELF (with a `PT_TLS` segment + early `#[thread_local]` reads) **would fault**. It didn't, because the **target JSON forecloses the gap**: `has_thread_local=false` + `singlethread=true` ⇒ rustc emits **no `PT_TLS` segment** and the overlay routes `thread_local` to the `no_threads` statik path, so std never touches an OS TLS key or the FS base. The loader's TLS gap is real but **structurally never exercised** for this target. This is the load-bearing integration fact for C1: **the moment C1 adds real user threads (per-thread TLS), the loader will need `PT_TLS` + FS-base setup** — that work lands with C1's thread ABI, not before. (Bare `_start` + no argv also held: stdout inits lazily; no SysV crt0 contract is required for the smoke.)

---

## 4. A1 / A4 / kill-switch

- **A1 (no W^X):** the overlay introduces **no executable-memory mapping**. The target + heap are R+W+**NX**; `panic=abort` (no unwinder needing exec pages — `panic_abort`, and the loader refuses any W+X segment). JIT remains the future budgeted rung.
- **A4 (manifest armed over the overlay — genuinely).** The build-breaking gate (`neo-std-pal/build.rs`) scans **both** the PAL crate **and** the overlay's syscall surface: `for entry in walk_rs(&src_dir).chain(walk_rs(&overlay_dir))`. Confirmed **falsifiable both directions** — clean: `gate OK — PAL src + std overlay syscalls [SYS_EXIT, SYS_MAP_REGION, SYS_WRITE] ⊆ allowed`; and a bogus `SYS_SOCKET_CREATE` injected into `stdio_neoplutarch.rs` → **A4 gate FAILED** (build refused), reverted. *(AAG-bounce Finding 1: the first spin declared `overlay_dir` but never walked it — the guard was a claimed-but-absent no-op; now real.)*
- **Kill-switch / no-regression (build-side scoped — AAG-bounce Finding 2).** The overlay-apply + the `build-std=std` smoke build + the ISO module staging + its `limine.conf` line are **all gated behind the `std_real_smoke` scenario** (`stage_std_real_smoke()`, set only in `build_user_for_scenario` for that scenario; `false` on the default `build_user()` path). So a plain `xtask build` / production ISO is **genuinely untouched**: no shared-toolchain rust-src mutation, no 6.5 MB test ELF compiled or bundled, and (mirroring the mt7601u-fw conditional-module convention) the `neo-std-real-smoke` `module_path` is injected into the staged `limine.conf` only for its scenario — a default ISO never lists a module it does not stage. Binaries stay bit-for-bit unchanged; the ABI stays libneo capabilities (only the manifest-gated WRITE/MAP_REGION/EXIT). No new invariant; A2 untouched.

---

## 5. Scope forward (dependency-depth, no calendar)

C0.5 is the runtime close of C0. The DAG continues **C1** (user threads/TLS — which forces the loader `PT_TLS`/FS-base work §3, plus mutex/condvar waits, monotonic clock, VM map/unmap/**protect**, stack bounds, co-designed for `std` + SpiderMonkey pthread) → **C2** (sealed handles + no-dial + the A2 memory quota) → **C3** (SpiderMonkey no-JIT + the C++/libc++ CRT). **C1 is the depth bottleneck.** Every `sys/*` surface the overlay left a loud stub becomes a C1/C2 rung when a real consumer needs it — enumerable, not surprising.

---

## 6. Evidence + method

- **RAG** healthy (2026-07-22); discovery only — the std `sys/*` interface, the loader/spawn path, and the libneo ABI were read/probed directly.
- **Built + run this cycle (Docker toolchain):** the overlay compiles real `std` for `x86_64-unknown-neoplutarch`; `std-real-smoke` links to a static ELF (no PT_TLS); the `std_real_smoke` QEMU scenario **PASSes** with the real-std prints in the serial log (§0). A4 gate armed over the overlay.
- **No-human-time:** rung effort as dependency-depth/cycles/gates (§5).
