<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# OT-61 browser C0 — the libstd PAL skeleton + host de-risk — FINDINGS

**Date:** 2026-07-22
**Lane:** Cerebro (Claude family; authored the cross-family check this builds on)
**Status:** C0 BUILT (the ladder's first rung) — host de-risk + first-class target + PAL skeleton + the A4 build-breaking manifest gate. All six cross-check amendments folded.
**Builds on:** `docs/RESEARCH-2026-07-22-ot61-browser-closure-crosscheck-FINDINGS.md` (the amended ladder, RATIFY-AFTER-AMENDMENT) + the two Janus spikes (host-closure, SWGL).

---

## 0. What C0 delivers (and the one honest scope line)

C0 is the **gating precursor** for the whole browser program: nothing downstream (SWGL floor, Servo, SpiderMonkey) can start until a first-class Rust target + a `libstd` platform layer exists. This build lands, **host-de-risk-first (A5)**:

1. **The authoritative PAL surface manifest** — `tools/ot61-pal-manifest.toml`, the definitive falsifiable inventory of what a hosted `std` (+ SpiderMonkey pthread, A3) demands, grounded in the pinned toolchain's real `library/std/src/sys/*` set AND the concrete `-Z build-std=std` probe.
2. **The SWGL↔Atrium byte-contract oracle** — `tools/swgl-atrium-oracle`, hand-laid XRGB8888_LE bytes (ADR-028) proving the render seam BEFORE any kernel work (6 host tests green).
3. **A first-class target** — `userland/x86_64-unknown-neoplutarch.json` (`os = "neoplutarch"`, static, panic=abort, R+W+NX).
4. **The `libstd` PAL skeleton** — `userland/neo-std-pal`, the *real* platform-primitive tier: print/heap/exit wired to raw libneo syscalls; every C1+ surface a **deterministic loud-abort stub** carrying its symbol name.
5. **The A4 build-breaking manifest gate** — `userland/neo-std-pal/build.rs`: a raw syscall the PAL references outside the manifest **fails the build** (proven: clean-pass + `SYS_KILL2`-injection fails).
6. **The `std-pal-smoke` gate** — `userland/std-pal-smoke`, a target-native binary proving print+alloc+exit on-target and the loud-absence property.

**The honest scope line (the C0-completion sub-rung).** A **real-`std`-linked** smoke requires a bounded rust-src `library/std/src/sys/*` overlay (§4). This build proves the target, the PAL primitives (raw-syscall-verified in the on-target ELF disasm), the loud-absence property, and the A4 guard — the *primitive tier*. The std-linked tier is the next sub-rung, and its **exact remaining surface is enumerated** here (§4) — not papered over. Per the dispatch, a bigger-than-modeled surface would be a cheap early finding; **the finding is the opposite: the surface is exactly as the FINDINGS modeled, and the overlay is bounded.**

---

## 1. Host de-risk (A5) — done first, zero kernel

### 1.1 The PAL surface manifest (`tools/ot61-pal-manifest.toml`)

Grounded in **two independent sources this build read directly:**
- the pinned toolchain's real platform-module set — `library/std/src/sys/` = `{fs, io, net, os_str, path, personality, pipe, process, random, stdio, sync, thread, thread_local, time, pal}` (the PAL was refactored out of `sys/pal/` in rustc 1.97; the real surface is `sys/*`);
- the concrete `-Z build-std=std,panic_abort` probe for `target_os="neoplutarch"`, whose undefined-symbol output pinned the exact gaps (recorded per-row as `probe = "..."`): `sys::io::{decode_error_kind,errno,error_string,is_interrupted}`, `sys::thread_local::key::{Key,LazyKey,get,set}`, `sys::random::fill_bytes`, the `sys::pal` `imp` alias, and a `panic_unwind` arm.

Each surface is annotated `closed-c0 | c1 | c2 | c3 | reserved-a1 | reserved-a2 | compile-out`, grouped by gap-table surface, with the **A3** SpiderMonkey/pthread consumer the same C1 primitive must also serve. **Result: C0 closes exactly 3 primitives (stdio/alloc/process-exit); everything else is a named C1/C2/C3 rung.** The surface is not larger than modeled.

### 1.2 The SWGL↔Atrium byte-contract oracle (`tools/swgl-atrium-oracle`)

Hand-laid XRGB8888_LE bytes (independent of any SWGL/Atrium code — the ADR-028 rule) pinning the render seam's four silent-corruption modes: **channel order** (SWGL BGRA `[B,G,R,A]` ↔ Atrium `[B,G,R,X]`, no swap — caught with distinct R≠B), **padded stride** (row 1 lands at the padded offset, not tight), **the ignored high byte** (nontrivial A doesn't change displayed RGB), and **`get_color_buffer(_, flush=true)`** delayed-clear resolution. 6 tests green; pins `FORMAT_XRGB8888_LE` to the real kernel constant. This proves the seam contract before a line of kernel work and **seeds A6's real-corpus GL gate** (the future `AtriumSwglRenderingContext` must satisfy these exact bytes).

---

## 2. The C0 target + PAL skeleton

- **Target** (`x86_64-unknown-neoplutarch.json`): distinct `os = "neoplutarch"` (so std can select a PAL), `static`, `panic=abort`, `has-thread-local=false`, `singlethread=true` — matching LogansOS reality (A1: R+W+NX; no thread runtime yet).
- **`neo-std-pal`** — the platform-primitive tier, **NOT a std-lite name shim** (the direction Chad/Janus/the cross-check REFUTEd): it issues **raw libneo syscalls** (the PAL sits *below* libneo, so it re-derives the raw ABI — `SYS_WRITE`/`SYS_MAP_REGION`/`SYS_EXIT`) and makes every unsupported surface fail **deterministically and loudly by name** (`unsupported_pal(symbol)` → stderr diagnostic + exit 90). The on-target ELF disassembly confirms the raw `syscall` wiring and the fail-closed `exit` spin.
- **`std-pal-smoke`** — builds cleanly on the first-class target (default + `unsupported-demo`), a static x86-64 EXEC. Default: print → alloc 1024×u32 + free → print → exit 0. `unsupported-demo`: calls a C1 stub → exit 90 + the named symbol. Both properties, one binary.

**A3 (co-design) is structural in the PAL:** each C1+ stub names the SpiderMonkey/pthread consumer the primitive must also serve (`thread_new_unsupported` ↔ `PosixThread.cpp`; `wait_unsupported` ↔ `ConditionVariable_posix.cpp`; `vm_protect_unsupported` ↔ GC `Memory.cpp`; `clock_unsupported` ↔ `TimeStamp_posix.cpp`; …) so C1 freezes **one** ABI serving both `std` and SpiderMonkey, not a std-only one.

---

## 3. A4 — the build-breaking PAL manifest gate (proven falsifiable)

`neo-std-pal/build.rs` scans the PAL's own source for every `SYS_*` constant and **fails the build** if any is not in `tools/ot61-pal-manifest.toml` `[gate].allowed_syscalls` (C0: exactly `SYS_WRITE`, `SYS_MAP_REGION`, `SYS_EXIT`). Proven both directions this build:
- **clean:** `A4 PAL-manifest gate OK — PAL syscalls [SYS_EXIT, SYS_MAP_REGION, SYS_WRITE] ⊆ allowed [...]`, build Finished;
- **negative:** injecting an off-manifest `SYS_KILL2` reference → `A4 gate FAILED`, build refused.

This is the structural anti-compat-island guard the cross-family check demanded: a new host-surface entry cannot sneak in — it must be added to the manifest **with its capability provenance in the same change**. The **tightest** discipline is reserved in the manifest for the eventual C++/libc++ surface (C3), the highest leak-risk vector.

---

## 4. The C0-completion sub-rung: the rust-src std overlay (exact surface enumerated)

A real-`std`-linked `std-pal-smoke` needs each `sys/*` `cfg_select!` to gain a `neoplutarch` arm. From this build's probe, the surface is **bounded and enumerable** (dependency-depth, not calendar):
- **Route to the existing `unsupported.rs`** (most modules): `fs, net, process(Command), pipe, thread, thread_local, sync, time, random, os_str, path, personality` — each gets a `target_os = "neoplutarch" => unsupported` arm. These are already the deterministic-failure surfaces C0's stubs mirror.
- **Three custom modules** (the wired primitives): `stdio` → `SYS_WRITE`, `alloc`(`sys::alloc`) → `SYS_MAP_REGION`, `process::exit` → `SYS_EXIT` — the exact three `neo-std-pal` already implements; the overlay lifts them into std's `sys`.
- **`sys::io`** error-decode: a small LogansOS-errno → `io::ErrorKind` table (probe-named 4 fns).
- **`panic_unwind`**: a `neoplutarch` arm (target is abort → the abort path).
- **Mechanism:** an overlay-apply step (the ephemeral CI container's rust-src is writable) that copies the reviewed `neoplutarch` PAL files onto the toolchain's `library/std/src/sys/*` before `-Z build-std=std,panic_abort`. The overlay files are the reviewable deliverable; the apply is reproducible per-build.

This is the immediate next sub-rung (C0.5) — bounded, no unknowns; the primitive tier proves everything it inherits.

---

## 5. The reserved rungs — NAMED, not built (A1, A2)

- **A1 — W^X ⊗ JIT invariant-budget rung (reserved).** LogansOS user memory is R+W+**NX**; there is **no runtime-executable-code primitive**. C0 introduces **none** — the `SyscallHeap` is NX, no stub grants W^X, the target has no executable-writable path. The eventual JIT is the **first** such primitive = a **new cross-cutting invariant** (`docs/complexity-governance.md` budget): it must be named, justified, guarded, and **kill-switchable** (JIT-off ≡ interpreter, provably) in the build that adds it. Interpreter-first is honest for *correctness*; JIT gates the *usable* end-state. **Reserved in the manifest (`[reserved.a1_wx_jit]`); C0 does not build or approach it.**
- **A2 — per-content-process memory quota (reserved, required rung).** No per-process frame ceiling exists (`sys_map_region` caps only per-call); an adversarial page can DoS the whole system before its process dies. "OOM kills only that process" becomes deliverable only when a per-content-process quota trips before global exhaustion — a **kernel VM change landing around C1/C2** with the region tracker, NOT C0. **Reserved in the manifest (`[reserved.a2_memory_quota]`).**

---

## 6. Kill-switch / no-regression + invariant budget

- **Kill-switch (Rule-1):** C0 is **purely additive** — a new opt-in target JSON, two new userland crates (`neo-std-pal`, `std-pal-smoke`), one new host oracle crate, one manifest. The **default kernel/toolchain build is bit-for-bit unchanged**; no kernel file is touched; **no new syscall** is added (the PAL uses only the existing `SYS_WRITE`/`SYS_MAP_REGION`/`SYS_EXIT`). Nothing web-facing reaches the kernel/user ABI — it stays libneo capabilities, **no POSIX personality leaks** (the A4 gate enforces this structurally).
- **Invariant budget:** C0 introduces **no** new cross-cutting invariant. It *names* A1 (JIT) and A2 (quota) as future invariant-budget items. The ledger (I-1…I-16) is unchanged.

---

## 7. The C0 gate — `std-pal-smoke`

Janus's falsifiable go/no-go: prints, allocates+frees, exits cleanly, and unsupported PAL APIs fail deterministically by name. **Primitive tier: BUILT + verified** (on-target static ELF; raw-syscall wiring confirmed in disasm; A4 gate falsifiable; the byte oracle + all host checks green). The **on-target QEMU run** of the smoke ELF (spawned via the process harness) is the C0 **close step** — the ELF is ready; it needs the loader/scenario spawn wiring (mechanical; adds no syscall). The **std-linked** tier is the C0.5 overlay sub-rung (§4).

---

## 8. Evidence + method

- **RAG** healthy (2026-07-22). Used for discovery; every load-bearing surface (the real `sys/*` set, libneo's `SYS_*` ABI, the XRGB layout, the process/target facts) read directly from source / probed.
- **Built + verified this cycle (host, via the Docker toolchain):** `swgl-atrium-oracle` 6/6 tests; `neo-std-pal` builds + the A4 gate proven both directions; `std-pal-smoke` builds on `x86_64-unknown-neoplutarch` (default + `unsupported-demo`) → static ELF, entry `0x207670`, raw `syscall` wiring confirmed by `objdump`.
- **The `-Z build-std=std` probe** for `target_os="neoplutarch"` produced the exact PAL-gap symbol list (§1.1) — the enumeration technique the dispatch asked for.
- **No-human-time:** scope expressed as dependency-depth / rungs / gates (C0 → C0.5 overlay → C1 threads/TLS/waits/region-tracker[+A2] → C2 sealed-handles/broker → C3 SpiderMonkey/C++-CRT[+A1 JIT decision]); C1 is the depth bottleneck.
