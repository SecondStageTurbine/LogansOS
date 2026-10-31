# Research: Phase 3 BUILD-1 aarch64 CI lane feasibility

| | |
|---|---|
| **Date** | 2026-05-12 (fresh-context Cerebro session) |
| **Author** | Cerebro research artifact |
| **Type** | Research artifact (informational; not normative). Feasibility assessment. |
| **Authority** | None. PM consumes to decide whether aarch64 lane is a BUILD-1 prerequisite, a parallel workstream, or a documented deviation at BUILD-1 close. |
| **Scope** | Five questions: substrate gaps, compile-only value, smallest viable runtime lane, aarch64 barrier primitives, formal blocker language. |
| **Time budget actual** | ~45 minutes |

---

## 1. Executive Summary

**The aarch64 lane is viable, well-scoped, and should be a parallel workstream running alongside BUILD-1 — not a prerequisite that blocks the Jarvis BUILD-1 dispatch.** The barrier-exposure value is real and irreplaceable: on x86-64 with TSO, `compiler_fence(Ordering::Release)` and `fence(Ordering::Release)` are indistinguishable at runtime (both emit no hardware instruction), so a missing `fence` in the virtio ring submission path will never manifest as a bug on x86 regardless of how much stress testing is run. On aarch64, `fence(Ordering::Release)` emits `dmb ish` while `compiler_fence` emits nothing — making the missing-barrier bug immediately observable as a DMA corruption or hang.

**The substrate gap is well-defined.** Three categories of missing work: (1) arch/aarch64/ kernel module with a minimal boot stub, (2) Cargo/toolchain configuration for `aarch64-unknown-none`, and (3) virtio-blk MMIO transport code (smaller than PCI transport, equivalent barrier-exposure coverage). The x86-specific crate dependencies (`x86_64`, `raw-cpuid`, `uart_16550`) must be feature-gated or replaced. The existing Limine boot path should be abandoned for the aarch64 lane — direct `-kernel` ELF load via QEMU is simpler, removes the UEFI firmware requirement, and is the consensus pattern for minimal aarch64 QEMU lanes.

**Compile-only evidence is too weak.** It catches structural errors (wrong asm!, missing `#[cfg(target_arch)]` guards) but cannot catch the missing-barrier bug class. The whole point of the lane is runtime execution — a compile-only gate would pass a driver with `compiler_fence` everywhere and miss the correctness defect entirely.

**The polling harness catches step-3 bugs fully. It catches step-5 bugs only if the harness explicitly negotiates VIRTIO_F_EVENT_IDX and takes the notification-suppression decision branch.** A trivial "always notify" polling harness misses step-5 entirely. The correct BUILD-1 lane must exercise the notification-suppression path with a hang-detection deadline.

---

## 2. Q1 — What exact substrate is missing for a real aarch64 QEMU virtio-blk lane?

### 2.1 Substrate survey (from direct source read)

NeoPlutarch is fully x86_64-only at HEAD. The following substrate items are confirmed absent:

**Architecture module:**
- `kernel/src/arch/aarch64/` does not exist. `arch/mod.rs` line 1: *"Architecture-specific code. For now, only x86_64."* It dispatches only to `pub mod x86_64`. `arch/x86_64/mod.rs` line 8: *"Phase 5+ may add aarch64 if we target the Pi 5; everything in this module is x86-only and will get gated behind `#[cfg(target_arch = "x86_64")]` when that lands."*

**Target configuration:**
- `kernel/aarch64-neoplutarch.json` does not exist. Only `kernel/x86_64-neoplutarch.json` is present.
- `rust-toolchain.toml` specifies `targets = ["x86_64-unknown-none"]`. No aarch64 target.
- `kernel/.cargo/config.toml` sets `target = "x86_64-neoplutarch.json"` and has only `[target.x86_64-neoplutarch]` rustflags. No aarch64 entry.

**Linker script:**
- The existing `kernel/src/arch/x86_64/linker.ld` is x86_64-specific (x86 ELF sections, x86 start address assumptions). An aarch64 linker script is needed; RAM base is different (0x40000000 on QEMU virt machine vs x86's typical 0x100000).

**x86-specific crate dependencies** (would block `cargo check --target aarch64-unknown-none`):
- `x86_64` crate — entire CPU-register, GDT, IDT, CR3, interrupt API is x86-specific. Used throughout `arch/x86_64/`. Must be gated behind `#[cfg(target_arch = "x86_64")]`.
- `raw-cpuid` crate — x86 CPUID instruction. Used in `arch/x86_64/mod.rs` for SMEP/SMAP detection. Must be gated.
- `uart_16550` crate — x86 16550A serial UART (I/O port instructions `inb`/`outb`). aarch64 uses PL011 UART at MMIO address 0x09000000 on QEMU virt. Needs a separate PL011 driver or a driver-abstraction seam.

**Boot path:**
- Limine on aarch64 is **UEFI-only** — no BIOS path. Running it on `qemu-system-aarch64 -M virt` requires shipping `edk2-aarch64-code.fd` (AAVMF firmware) into CI. Additionally, there is an active edk2 regression (tianocore/edk2 issue #10942, February 2025) where the `202502` AAVMF build fails to detect virtio-blk-PCI on QEMU ARM/aarch64. **Recommendation: bypass Limine entirely for the aarch64 lane** and use QEMU's direct `-kernel` ELF load. This removes the UEFI firmware requirement and the regression risk. The x86_64 lane keeps Limine; the asymmetry is justified because Limine's primary value on x86 is BIOS+UEFI multiplexing, which has no analog on aarch64.

### 2.2 Summary gap table

| Gap | Category | Estimated size |
|---|---|---|
| `kernel/src/arch/aarch64/mod.rs` with `halt_forever`, interrupt on/off, UART init stubs | New code | ~60 lines Rust |
| `aarch64-neoplutarch.json` target spec | New file | ~15 lines JSON |
| aarch64 linker script | New file | ~40 lines |
| `rust-toolchain.toml` add `aarch64-unknown-none` target | Config change | 1 line |
| `kernel/.cargo/config.toml` add `[target.aarch64-neoplutarch]` with correct rustflags | Config change | ~5 lines |
| `arch/mod.rs` `#[cfg(target_arch)]` dispatch | Refactor | ~20 lines |
| `x86_64`, `raw-cpuid`, `uart_16550` crate feature-gating | Refactor | ~30 lines |
| PL011 UART driver (TX-only) at 0x09000000 | New code | ~40 lines Rust |
| aarch64 boot entry stub (stack init, BSS zero, branch to `_start`) | New code | ~30 lines asm |
| virtio-mmio probe (MMIO transport, not PCI) | New code | ~120 lines Rust |
| virtio-blk minimal driver (descriptor ring, polling, step-3/step-5 barriers) | New code | ~150 lines Rust |
| PSCI shutdown for clean CI exit | New code | ~10 lines Rust |
| CI lane QEMU invocation + pass/fail parsing | CI config | ~20 lines |

**Total estimated effort: 1.5–2 developer-days for a complete minimal lane.**

---

## 3. Q2 — Is compile-only aarch64 evidence useful, or too weak?

### 3.1 What compile-only proves

Cross-compiling NeoPlutarch's virtio driver to `aarch64-unknown-none` would verify:

- No x86-specific inline assembly (`asm!("mfence")`, `asm!("lock")`) is emitted unconditionally in the virtio driver code.
- `#[cfg(target_arch = "x86_64")]` gates are correctly placed; the build does not include x86 primitives on an aarch64 target.
- All Rust `fence()` and `compiler_fence()` calls compile syntactically — the types and Ordering variants are valid.
- The data structures (descriptor table, avail ring, used ring) have correct alignment and `repr(C)` layout on a 64-bit LE platform.

### 3.2 What compile-only does NOT prove

- **Whether `fence(Ordering::Release)` is used where `compiler_fence` would be wrong.** On aarch64, `fence(Release)` emits `dmb ish` and `compiler_fence(Release)` emits nothing. Both *compile* identically — the distinction is invisible to the compiler's type system. A driver written with `compiler_fence` everywhere compiles cleanly for aarch64 and contains a latent barrier bug that only manifests at runtime.
- **Whether the MMIO register writes happen in the correct order.** QEMU's device model may be more forgiving than real hardware (or may not be — the point is that runtime execution on the actual hardware memory model is the test).
- **Whether the polling loop correctly detects completion.** A compile check cannot verify that the driver actually completes a DMA transfer.

### 3.3 Verdict

**Compile-only is too weak for the stated goal ("barrier exposure") but is a useful first step for structural validation.** The correct ordering is:

1. **Phase 1 — Compile-only check:** add `aarch64-unknown-none` to the CI matrix; verify the codebase compiles without x86-isms leaking through. This is cheap and catches a different bug class (structural x86-coupling). Can land in BUILD-1 even before the runtime lane exists.
2. **Phase 2 — Runtime lane:** the QEMU lane described in Q3. This is the actual "barrier exposure" check. Should land at BUILD-1 close as an acceptance gate.

**Phase 1 alone should not be cited as evidence that barrier discipline is correct.** The BUILD-1 closure artifact must distinguish "compile-only aarch64 check passes" from "aarch64 runtime barrier test passes."

---

## 4. Q3 — What is the smallest viable CI/runtime lane?

### 4.1 Transport choice: MMIO over PCI

Use the **virtio-mmio MMIO transport** (`-device virtio-blk-device`), not the PCI transport (`-device virtio-blk-pci`).

**Why MMIO is the right choice:**
- The descriptor ring format (split virtqueue: descriptor table, avail ring, used ring, VRING_DESC_F_NEXT, VRING_DESC_F_WRITE) is **identical between MMIO and PCI transports** per OASIS virtio v1.2 §2.7. The barriers that matter (step-3 and step-5) are in the data-plane ring protocol, which is transport-agnostic.
- MMIO device discovery: walk 32 MMIO slots at `0x0a000000`–`0x0a004000`, read magic value `0x74726976` at offset 0x000 to confirm a virtio device, read DeviceID at offset 0x008 to confirm block device. ~60 lines of Rust.
- PCI discovery: PCI config space enumeration (BAR sizing, capability list walk, virtio capability structure parsing). ~300+ lines of Rust. No additional barrier coverage.
- **The MMIO lane is strictly equivalent to a PCI lane for barrier-bug coverage.**

**On QEMU virt machine:** virtio-mmio devices appear at fixed MMIO addresses described in the auto-generated device tree blob (FDT). The FDT base is at 0x40000000 (start of RAM) when booting ELF via `-kernel`. For a minimal CI harness, hardcoding the base address `0x0a000000` is acceptable; DTB parsing is not required.

### 4.2 Boot choice: direct `-kernel` ELF

Use **QEMU's direct `-kernel` ELF load**, bypassing Limine. QEMU reads the ELF entry point from `e_entry`, loads PT_LOAD segments at their specified physical addresses, and jumps to the entry point. The FDT/DTB is placed at RAM base 0x40000000 by QEMU.

Linker script: link the kernel ELF at 0x40080000 (0x40000000 RAM base + 0x80000 offset, above the DTB region). Entry symbol is the aarch64 boot stub that sets up the stack, zeros BSS, and branches to `_start`.

### 4.3 What the harness must do

**The harness must exercise the notification-suppression decision (step-5) explicitly, not just always-notify.** This is the part most minimal harnesses get wrong.

Required elements:

1. **Negotiate `VIRTIO_F_EVENT_IDX`** at feature-negotiation time. This enables the `avail_event` field in the used ring — the device writes the avail index it wants notified at, and the driver is permitted to skip the notify if `avail->idx <= avail_event` at submission time.

2. **Issue the notify conditionally,** consulting `avail_event` under the step-5 barrier:
   ```rust
   // Step-3: ensure descriptor writes are visible before idx update
   fence(Ordering::Release);   // emits dmb ish on aarch64
   avail_ring.idx = avail_ring.idx.wrapping_add(1);

   // Step-5: ensure idx write is visible before reading avail_event
   fence(Ordering::SeqCst);    // emits dmb ish on aarch64
   let event = used_ring.avail_event.load(Ordering::Relaxed);
   if wrapping_lt(old_idx, event.wrapping_add(1)) {
       // Notify device: write queue index to QueueNotify MMIO register
       mmio_write32(base + VIRTIO_MMIO_QUEUENOTIFY, 0);
   }
   ```

3. **Hang detection:** if `used_ring.idx` does not advance within N iterations (~10M spins at 100 MHz equivalent), print "HANG: step-5 notify not sent or not received" and exit with failure. This is what catches the missing-step-5-barrier bug.

4. **Verify data integrity:** after completion, read back the DMA buffer and compare against expected content. This catches step-3 bugs (data not visible to device → garbage written to buffer).

### 4.4 Concrete QEMU invocation

```sh
qemu-system-aarch64 \
  -M virt \
  -cpu cortex-a57 \
  -m 128M \
  -nographic \
  -kernel kernel.elf \
  -drive if=none,format=raw,file=test.img,id=hd0,cache=none \
  -device virtio-blk-device,drive=hd0 \
  -no-reboot \
  -d guest_errors \
  -semihosting-config enable=on,target=native
```

Key flags:
- `-nographic`: redirects PL011 serial to stdout; essential for CI.
- `-no-reboot`: kernel panic exits QEMU rather than restarting.
- `-d guest_errors`: logs guest-mode error events (MMU faults, illegal instructions) to stderr — invaluable for diagnosing barrier-related faults.
- `-semihosting-config enable=on,target=native`: enables ARM semihosting (`hlt #0xf000`), the canonical aarch64 bare-metal exit-with-status mechanism. Success: `mov w0, #0; mov w1, #0x18 (sys_exit); hlt #0xf000`. Failure: `mov w0, #1; ...`. QEMU exits with the status code, which the CI system reads.

**Comparison note:** x86 uses QEMU's `isa-debug-exit` device for clean exit; aarch64 uses semihosting. The difference must be accounted for in the CI harness's test runner.

### 4.5 Summary: smallest viable runtime lane

| Component | What it is | ~LOC |
|---|---|---|
| aarch64 entry stub (asm) | stack, BSS zero, branch to Rust | 30 |
| PL011 UART (TX only) | serial at 0x09000000 | 40 |
| virtio-mmio probe | magic + DeviceID + queue init | 80 |
| virtio-blk ring driver | descriptors + step-3/5 barriers + VIRTIO_F_EVENT_IDX + polling | 150 |
| DMA buffer allocation | in-kernel bump allocator or static region | 30 |
| Test harness + PSCI exit | verify data, print PASS/FAIL, exit | 30 |
| Linker script + target JSON | aarch64-neoplutarch.json + linker.ld | 55 |
| arch/aarch64/mod.rs stubs | halt, interrupts on/off (stubs only) | 40 |

**Total: ~455 lines of new code, 1.5–2 developer-days.**

This is the smallest harness that actually catches both bug classes. A simpler "always-notify, skip step-5" harness saves ~30 lines but misses the step-5 bug class entirely.

---

## 5. Q4 — Barrier primitives on aarch64 for descriptor publish vs notification suppression

### 5.1 The x86 vs aarch64 barrier table

The following table is the core finding. The "x86 emits" column is why the bug is invisible on x86; the "aarch64 emits" column is why the lane catches it.

| Primitive | x86-64 emits | aarch64 emits | Virtio use |
|---|---|---|---|
| `compiler_fence(Ordering::Release)` | *(nothing)* | *(nothing)* | WRONG for step-3; invisible failure |
| `compiler_fence(Ordering::SeqCst)` | *(nothing)* | *(nothing)* | WRONG for step-5; invisible failure |
| `fence(Ordering::Release)` | *(nothing — TSO provides store-release)* | **`dmb ish`** | CORRECT for step-3 |
| `fence(Ordering::Acquire)` | *(nothing — TSO provides load-acquire)* | `dmb ishld` | Load-side acquire |
| `fence(Ordering::SeqCst)` | `mfence` (or `lock xchg`) | **`dmb ish`** | CORRECT for step-5 |
| `fence(Ordering::AcqRel)` | *(nothing — TSO)* | `dmb ish` | Full barrier |

*Sources: Sewell et al., C/C++11 mappings (cl.cam.ac.uk); Rust std::sync::atomic::fence docs; Rust Atomics and Locks §7 (Mara Bos); duetorun ARM64 memory barriers.*

### 5.2 Step-3 (descriptor publish before avail→idx update)

**Correct primitive: `fence(Ordering::Release)`**

On aarch64 this emits `dmb ish` (Inner Shareable, full barrier). The architecturally minimal instruction for store-store ordering is `DMB ISHST` (Inner Shareable, Store-Store), but Rust/LLVM conservatively emits the stronger `DMB ISH` for any `Release` fence, which is also correct.

**The bug the lane catches:** if a developer writes `compiler_fence(Ordering::Release)` instead of `fence(Ordering::Release)` — motivated by a correct observation that "on x86, Release fences emit nothing, so maybe compiler_fence is enough" — the code compiles cleanly on both architectures but is silently broken on aarch64. On x86-TSO, all stores are ordered by default (no hardware reorder of stores past stores), making `fence(Release)` a no-op hardware-wise. On aarch64, stores CAN be observed out-of-order without an explicit barrier, so the descriptor fields may appear to the device after the idx update.

### 5.3 Step-5 (avail→idx visible before reading notification flags)

**Correct primitive: `fence(Ordering::SeqCst)`** (or `fence(Ordering::AcqRel)` — both emit `dmb ish` on aarch64 via LLVM)

This step requires a full store-load barrier: ensure the idx store is globally visible before reading the device-written `flags`/`avail_event` field. On aarch64, this is `DMB ISH`. On x86-64, `fence(SeqCst)` emits `mfence`; but `fence(Release)` emits *nothing* on x86 (TSO gives you store-release for free). So `fence(Release)` at step-5 is technically incorrect semantically (step-5 is a store-load ordering, not store-store), but happens to work on x86 because x86 doesn't reorder stores before loads in this pattern. On aarch64, the distinction matters: `fence(Release)` emits `dmb ish` which happens to cover the store-load case, but the *intent* should be `SeqCst` or `AcqRel`.

**The bug the lane catches:** `compiler_fence(Ordering::SeqCst)` at step-5 — emits nothing on both architectures, so the idx store is not guaranteed visible before the `avail_event` read. The driver reads a stale `avail_event` indicating suppression is active, skips the notify, but the device hasn't seen the idx update yet. Request hangs indefinitely. Caught by the hang-detection timeout in the harness.

### 5.4 Recommended barrier wrapper

Rather than scattering raw `fence(Ordering::X)` calls through the virtio driver, introduce a small wrapper module with named barrier functions:

```rust
// kernel/src/drivers/block/virtio/barrier.rs

/// Step-3 barrier: ensure all prior stores to the descriptor ring are
/// globally visible before updating avail->idx.
/// x86-64: emits nothing (TSO provides store ordering).
/// aarch64: emits DMB ISH.
#[inline(always)]
pub fn publish_descriptors() {
    core::sync::atomic::fence(core::sync::atomic::Ordering::Release);
}

/// Step-5 barrier: ensure avail->idx write is globally visible before
/// reading the notification-suppression flags (avail_event).
/// x86-64: emits mfence (or lock-prefixed op via LLVM).
/// aarch64: emits DMB ISH.
#[inline(always)]
pub fn before_notification_check() {
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
}
```

These names are human-auditable. A reviewer can grep for `publish_descriptors()` and `before_notification_check()` to verify every submission path has both barriers. The wrong primitive (compiler_fence) literally cannot appear under these names.

---

## 6. Q5 — If blocked, what should the formal blocker artifact say?

### 6.1 What "blocked" means in context

A BUILD-1 closure is blocked on the aarch64 lane if PM decides the lane is a BUILD-1 **acceptance gate** (the lane must pass before BUILD-1 closes). If PM decides the lane is a **parallel workstream** (the lane runs alongside BUILD-1 and becomes a gate at BUILD-2), then no blocker is filed — instead, a deviation is recorded at BUILD-1 close.

**Recommendation: parallel workstream.** The virtio driver for BUILD-1 will be written new; it is easier to write it correctly once (with the named barrier wrappers from §5.4) than to write it for x86 and then retroactively add aarch64 barriers. The lane should be started alongside the virtio driver implementation, with the intent that both are complete before BUILD-1 closes.

If the lane is not complete at BUILD-1 close, the following deviation record applies.

### 6.2 Formal deviation language for BUILD-1 closure (if lane incomplete)

```
## Deviation: aarch64 barrier-exposure CI lane not complete

**Type:** Test coverage gap (not a correctness defect in shipped code).
**Discovery:** Cerebro #2 dispatch (SC-4 D5) identified that x86-TSO masks
virtio-blk ring submission barrier bugs. An aarch64 QEMU lane with
VIRTIO_F_EVENT_IDX negotiation and hang-detection is required to provide
honest evidence of barrier correctness.

**Current state:** The BUILD-1 virtio-blk driver uses named barrier wrappers
(publish_descriptors(), before_notification_check()) throughout the
submission path. All barrier sites use fence(Ordering::X), not
compiler_fence. The wrappers emit DMB ISH on aarch64.

**What is missing:** A runtime aarch64 QEMU lane that actually executes the
virtio submission path under the aarch64 memory model. The barrier code
compiles correctly for aarch64 (Phase 1 compile check: pass) but has not
been verified at runtime.

**Risk:** Low. The named-wrapper discipline makes the wrong primitive
auditable; using compiler_fence instead of fence literally cannot appear
under these names. The primary residual risk is a logic error in the
conditional-notify path (step-5) that would require runtime execution to
detect.

**Forward marker FM-Build1-Aarch64:** aarch64 QEMU virtio-blk MMIO lane
(~455 LOC, ~1.5 dev-days) to be completed as a BUILD-2 prerequisite. See
Plans/RESEARCH-2026-05-12-phase-3-build-1-aarch64-lane-feasibility-cerebro.md
§4 for the exact QEMU invocation and §5.4 for the required harness elements.

**Acceptance gate for forward marker:**
  qemu-system-aarch64 -M virt -cpu cortex-a57 ... exits 0 (semihosting)
  with sentinel "PASS: virtio-blk step-3 OK, step-5 OK" on serial output.
  Test must exercise the notification-suppression (avail_event) branch.
```

### 6.3 What the artifact must NOT say

The blocker/deviation must not say:
- "compile-only check passes — barrier discipline verified." This is the key anti-claim; §3 of this artifact establishes that compile-only is too weak.
- "aarch64 barrier discipline is correct by code review." Code review cannot substitute for runtime execution under the aarch64 memory model. A reviewer cannot tell the difference between `fence()` and `compiler_fence()` at glance on a small screen; the named-wrapper pattern makes it easier, but the runtime lane is still necessary.
- "barrier bugs are unlikely given x86 testing." x86-TSO testing is orthogonal to aarch64-WMO exposure. The bug class is invisible on x86 regardless of test volume.

---

## 7. Calibration Notes

**Primary sources verified:**
- Rust `fence` and `compiler_fence` documentation (doc.rust-lang.org) — primary
- Sewell et al. C/C++11 processor mappings (cl.cam.ac.uk) — primary academic reference for compiler/architecture barrier mappings
- Rust Atomics and Locks §7 (Mara Bos, mara.nl) — primary for Rust-specific fence codegen
- OASIS virtio v1.2 §2.7 (docs.oasis-open.org) — primary for ring discipline identity across transports
- QEMU virt machine docs (qemu-project.gitlab.io) — primary for MMIO layout, RAM base, FDT placement
- Limine README and protocol docs (github.com/limine-bootloader) — primary for UEFI-only aarch64 constraint
- tianocore/edk2 issue #10942 — primary for the AAVMF regression (single-source; verify before citing in ADR)

**NeoPlutarch source confirmed by direct read:**
- `kernel/src/arch/mod.rs` — x86_64-only dispatch
- `kernel/src/arch/x86_64/mod.rs` — x86_64-specific primitives; Phase 5+ aarch64 comment
- `rust-toolchain.toml` — x86_64-unknown-none only
- `kernel/.cargo/config.toml` — x86_64-neoplutarch target only
- `kernel/x86_64-neoplutarch.json` — x86_64 target spec only

**Single-source findings:**
- tianocore/edk2 #10942 (AAVMF regression) — single GitHub issue; confirm before using as authority
- `DMB ISHST` as the minimal correct step-3 instruction — confirmed by duetorun ARM64 barriers table (independent secondary) and the Linux smp_wmb() → DMB ISHST codegen (broadly documented); treat as well-corroborated secondary

**DMB ISH vs DMBISHST distinction:**
- LLVM emits `dmb ish` (full ISH barrier) for `fence(Ordering::Release)`, not `dmb ishst` (store-store ISH). The full barrier is stronger than required for step-3 but always correct. The distinction matters only if performance is a concern; for BUILD-1 it does not.

---

## 8. Cross-References

**This artifact consumes:**
- `Plans/RESEARCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md` — established the aarch64 CI lane requirement (§10.c, ISC-S15 in the storage driver research)
- `Plans/DISPATCH-2026-05-11-phase-3-storage-driver-prior-art-cerebro.md` — origin of the "add aarch64 CI lane" requirement

**This artifact is consumed by:**
- PM decision on aarch64 lane scope (prerequisite vs parallel workstream)
- Phase 3 BUILD-1 PLAN dispatch for Jarvis — should carry the named-barrier-wrapper requirement and the aarch64 lane as a CI acceptance gate or deviation-at-close item

**Not produced:** no ADR, no Bible amendment.

---

## 9. Closure

The aarch64 BUILD-1 lane is achievable in roughly 1.5–2 developer-days. The implementation path is clear: aarch64 target JSON + entry stub + PL011 + virtio-mmio probe + minimal split-ring driver with named barrier wrappers + PSCI semihosting exit + QEMU `-M virt` CI invocation. Limine is bypassed; MMIO transport replaces PCI. The resulting harness catches both the step-3 (descriptor visibility) and step-5 (notification-suppression hang) bug classes — the two failure modes that are permanently invisible on x86-TSO. Compile-only evidence is a useful structural first step but must not be cited as barrier-correctness evidence. If the runtime lane is not complete at BUILD-1 close, the deviation record in §6.2 provides the correct language and formally defers the gate to BUILD-2.

---

**End of research artifact.**

*Substrate gaps: no arch/aarch64/, no aarch64 target JSON, no linker script, x86_64/raw-cpuid/uart_16550 crates need gating, Limine is UEFI-only on aarch64 (bypass it). Compile-only is too weak — catches structural errors, misses missing-fence bugs. Smallest runtime lane: MMIO transport + direct -kernel ELF + VIRTIO_F_EVENT_IDX negotiation + hang-detection, ~455 LOC. Step-3 primitive: fence(Ordering::Release) → dmb ish. Step-5 primitive: fence(Ordering::SeqCst) → dmb ish. Expose class: compiler_fence() instead of fence() — invisible on x86-TSO, broken on aarch64. Recommendation: parallel workstream, not prerequisite; named-wrapper pattern makes the wrong primitive human-auditable.*
