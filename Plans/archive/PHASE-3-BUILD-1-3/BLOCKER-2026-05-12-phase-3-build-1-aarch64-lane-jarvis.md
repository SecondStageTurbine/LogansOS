# Blocker: Phase 3 BUILD-1 aarch64 Barrier-Exposure Lane

Date: 2026-05-12
Owner: Jarvis
Branch: `codex/phase-3-build-1-jarvis`

## Verdict

BUILD-1 closure is blocked on the required aarch64 barrier-exposure lane.

The current repo can run the BUILD-1 block/VFS integration gates on x86_64, but it does not have enough aarch64 kernel, boot, interrupt, or QEMU test substrate to expose weak-memory virtqueue descriptor-publish and notification-ordering bugs honestly.

## Exact Missing Substrate

- Toolchain and target layout are x86_64-centered.
  - `rust-toolchain.toml` lists `x86_64-unknown-none` only.
  - `tools/xtask/src/main.rs` uses `KERNEL_TARGET = "x86_64-neoplutarch"` and `USER_TARGET = "x86_64-neoplutarch-user"`.
  - There is no aarch64 target spec under a repo `targets/` directory.

- Boot/runtime architecture surface is x86_64-only.
  - `kernel/src/arch/mod.rs` exposes only `pub mod x86_64`.
  - x86 GDT, IDT, APIC, PIC, PIT, syscall, SMP, per-CPU, CR3, TLB, and inline asm paths are compiled directly.
  - No aarch64 exception vectors, EL transition/bootstrap, timer, GIC, UART console, MMU/page-table, TLB invalidation, per-CPU, or scheduler bring-up path exists.

- Test runner is x86_64-only.
  - `tools/xtask/src/main.rs` launches `qemu-system-x86_64`.
  - There is no `qemu-system-aarch64` runner, `virt` machine configuration, aarch64 serial capture, aarch64 disk attach path, or aarch64 CI lane.

- Storage path cannot yet be exercised on aarch64.
  - The current virtio-blk lane is reached through the x86 PCI/QEMU path.
  - There is no aarch64 virtio-mmio or aarch64 PCI transport probe lane tied into the test harness.

## Compile-Only Probe

Command attempted:

```powershell
cargo build -p neoplutarch-kernel --target aarch64-unknown-none '-Zbuild-std=core,alloc' '-Zbuild-std-features=compiler-builtins-mem' --no-default-features
```

Result: FAIL.

Representative failures:

- `x86_64::instructions` is configured out when the target is not `x86_64`.
- `uart_16550::SerialPort` is gated to x86/x86_64 targets.
- `extern "x86-interrupt"` ABI is unsupported on aarch64.
- x86 inline asm references `eax`, `ecx`, `edx`, `ebx`, and `gs:[24]`, which are invalid on aarch64.
- x86 control-register/MSR APIs such as `Cr3::read`, `Cr3::write`, `Cr4::read`, `Efer::update`, `Star::write`, and `LStar::write` are unavailable.
- Limine MP fields differ under the target surface: code expects `bsp_lapic_id` and `lapic_id`, while the available fields include `bsp_mpidr` and `mpidr`.

The failed compile confirms that no aarch64 runtime barrier lane exists yet.

## Why x86-Only Tests Cannot Close This Gate

x86 TSO can hide descriptor-ring ordering bugs that aarch64 may expose. BUILD-1 specifically needs confidence that:

- descriptor writes are visible before the available-ring index publish;
- available-ring publication is visible before queue notification;
- completion observation and wake-token state use the correct acquire/release ordering;
- notification-suppression reads are separated from descriptor publication by the intended barrier class.

The x86_64 QEMU lane validates function, feature negotiation, write-through behavior, DMA lifetime, and wake-token composition, but it cannot prove the weak-memory ordering required by ADR-021. A passing x86-only lane is therefore not equivalent to the required aarch64 barrier-exposure lane.

## Smallest Viable aarch64 Lane Proposal

1. Add an aarch64 kernel target spec and toolchain target entry.
2. Split `kernel/src/arch` so x86_64 code is target-gated and an aarch64 module can compile independently.
3. Implement minimal aarch64 boot/runtime:
   - entry/bootstrap under QEMU `virt`;
   - exception vectors;
   - EL/MMU/page-table setup;
   - timer and GIC interrupt routing;
   - UART serial console;
   - per-CPU identity and scheduler entry;
   - TLB invalidation and memory-barrier primitives.
4. Add an aarch64 virtio transport path, preferably virtio-mmio for QEMU `virt`, or a tested PCI path if selected.
5. Extend `xtask` with a `qemu-system-aarch64` neo-test lane, serial logging, generated disk attachment, and CI wiring.
6. Add a targeted barrier-exposure scenario that drives the virtqueue descriptor-publish, available-ring publish, notification, completion, and wake-token path under aarch64.

## PM Decision Needed

Authorize an aarch64 substrate build lane before BUILD-1 closure, or explicitly keep BUILD-1 open with x86 integration PASS and aarch64 closure BLOCKED.

No BUILD-2 NeoFS work should begin under this dispatch.
