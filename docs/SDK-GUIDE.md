<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# SDK-v0 Quickstart Guide

Build your first LogansOS userland program from a development PC. This guide assumes you know Rust and have never touched this repo before.

## What you'll build

A minimal ELF64 binary that writes "Hello, LogansOS!" to stdout via the `sys_write` syscall, then exits. The same pattern scales to any userland application: static-link `libneo`, call syscalls, compile with the custom target spec.

## Prerequisites

- **Rust nightly** — the repo pins `nightly-2026-04-16` in `rust-toolchain.toml`. Running `cargo` from the repo root will auto-select this toolchain via rustup override.
- **llvm-tools-preview component** — required for the target spec. Install with `rustup component add llvm-tools-preview`.
- **QEMU** — to run the built ISO (optional for compilation-only work).
- **Docker** — used by the xtask build harness for hermetic builds.

## Step 1: Clone and verify the toolchain

```bash
git clone https://github.com/SecondStageTurbine/NeoPlutarch.git
cd NeoPlutarch
cargo --version  # should show nightly-2026-04-16
```

The `rust-toolchain.toml` file at the repo root tells rustup to use the pinned nightly. If you don't have it installed, rustup will fetch it on first `cargo` invocation.

## Step 2: Build the template app

The repo includes `userland/template-app/` — a minimal working example. Build it:

```bash
cargo build --package template-app
```

This produces an ELF64 binary at:
```
target/x86_64-neoplutarch-user/debug/template-app
```

You can verify it's the right format:
```bash
file target/x86_64-neoplutarch-user/debug/template-app
# Output: ELF 64-bit LSB executable, x86-64, statically linked
```

## Step 3: Understanding the build configuration

Every LogansOS userland binary needs three files beyond `src/main.rs`:

### `.cargo/config.toml` — selects the target and linker flags

```toml
[build]
target = "../x86_64-neoplutarch-user.json"

[unstable]
build-std = ["core", "compiler_builtins", "alloc"]
build-std-features = ["compiler-builtins-mem"]

[target.x86_64-neoplutarch-user]
rustflags = [
    "-C", "link-arg=-static",
    "-C", "link-arg=-no-dynamic-linker",
    "-C", "link-arg=-z",
    "-C", "link-arg=max-page-size=0x1000",
]
```

This tells Cargo to cross-compile for the `x86_64-neoplutarch-user` target (defined in the shared JSON spec at `userland/x86_64-neoplutarch-user.json`), build the standard library from source (`build-std`), and link statically with no dynamic linker.

### `build.rs` — injects the linker script

```rust
use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let linker_script = manifest_dir.join("linker.ld");
    println!("cargo:rerun-if-changed=linker.ld");
    println!("cargo:rustc-link-arg=-T{}", linker_script.display());
}
```

### `linker.ld` — defines the ELF layout

The linker script places the binary at virtual address `0x400000` (4 MiB into the user half), with three PT_LOAD segments (text, rodata, data). This is identical across all LogansOS userland binaries. See `userland/template-app/linker.ld` for the canonical copy.

## Step 4: Writing your own app

Copy the template directory as a starting point:

```bash
cp -r userland/template-app userland/my-app
```

Then edit `userland/my-app/src/main.rs`. The entry point is always `_start`:

```rust
#![no_std]
#![no_main]

use libneo::{sys_exit, sys_write};

pub extern "C" fn _start() -> ! {
    let msg = b"Hello from my app!\n";
    let _ = sys_write(1, msg.as_ptr(), msg.len());
    sys_exit(0);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    sys_exit(255);
}
```

Key rules:
- `#![no_std]` and `#![no_main]` are mandatory — there is no standard library or `main()` on LogansOS.
- The entry point is `pub extern "C" fn _start() -> !` (never returns).
- Call `sys_exit(code)` at the end — do not return from `_start`.
- Every binary needs a `#[panic_handler]` that calls `sys_exit`.

## Step 5: Getting your app into an ISO (contrib-module flow)

**Today's distribution model is "bake into the ISO at build time."** There is no install-after-boot mechanism (that's the H2 kernel build, gaps G1/G2 in the THINK). To run your app under QEMU:

1. Add your crate to the workspace by appending `"userland/my-app"` to `Cargo.toml`'s `members` list.
2. Add a build entry in `tools/xtask/src/main.rs` following the existing pattern (copy the `USER_PACKAGE`/`USER_BIN` constants for neo-init as a template).
3. Build and run:

```bash
cargo neo
```

This builds the kernel, all userland binaries (including yours), assembles the Limine ISO, and launches QEMU with GTK.

**To actually *spawn* your app from within the OS**, the running `neo-init` or `neo-arthur` process must call `sys_spawn("my-app", &[])`. The kernel resolves the name to `/neo-my-app` in the Limine module list. This means your binary's Cargo package name (minus the `neo-` prefix convention) is how it's referenced at runtime.

For a contributed app, the typical flow is: open a PR with your crate + xtask integration → maintainer merges → your binary is baked into the next ISO build.

## Available syscalls

`libneo` exposes 48 syscall wrappers (49 in the kernel; `SYS_TENSOR_WRITE` is kernel-only). The full table, with stability tiers and errno values, is in [`docs/SDK-ABI.md`](SDK-ABI.md).

The most commonly used syscalls for a starting application:
- `sys_write(fd, buf, len)` — write to stdout (fd 1) or stderr (fd 2)
- `sys_exit(code)` — terminate the process
- `sys_open(path, flags)` — open a file (returns a handle index)
- `sys_read_fd(fd, buf, len)` — read from an open file
- `sys_close(fd)` — close a file handle
- `sys_spawn(name, handles)` — spawn another process by name

## ABI stability — what can break

The SDK-v0 ABI is classified into four tiers. Read [`docs/SDK-ABI.md`](SDK-ABI.md) for the full contract:

| Tier | What it covers | Can it break? |
|------|---------------|---------------|
| **STABLE** | Syscall numbers, signatures, errno values | No — permanent |
| **STABILIZING** | Versioned ABI structs (e.g., `ProcessSnapshotV1`) | Layout may evolve with version bump; kernel rejects mismatched sizes |
| **EXPERIMENTAL** | AI (0x100+) and network (0x180+) syscalls | Yes — expect additions and changes |
| **BREAKS FREELY** | libneo internal helpers | Yes — implementation detail |

## Honest boundaries — what SDK-v0 does NOT offer

- **No install-after-boot.** Binaries are baked into the ISO at build time. Loading an ELF from NeoFS requires a new kernel syscall (gap G1, the H2 frontier).
- **No dynamic linking.** All binaries must static-link against `libneo`. There is no shared library loader.
- **Cross-compile only.** Building requires a host PC with the Rust nightly toolchain. There is no on-box compiler.
- **No POSIX personality.** This is not Linux. There are no `libc` shims, no `fork()`, no pipes — just capability-bounded syscalls.

These boundaries are intentional and named. The H2 developer platform (OT-44) will address install-after-boot and capability-native distribution when the multi-principal frontier (OT-22) opens.

## Next steps

- Read [`docs/SDK-ABI.md`](SDK-ABI.md) for the complete syscall table, errno values, and versioning policy.
- Read the THINK that produced this SDK: [`docs/THINK-2026-07-05-ot44-sdk-v0.md`](THINK-2026-07-05-ot44-sdk-v0.md).
- Study `userland/neo-shell/` for a more complex example (interactive REPL with file I/O).
- Study `userland/neo-arthur/` for an AI agent example (inference syscalls, capability grants, TLS networking).
