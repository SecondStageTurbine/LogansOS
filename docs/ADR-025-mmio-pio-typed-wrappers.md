# ADR-025: Typed volatile MMIO and port-I/O wrappers for driver register access

**Date:** 2026-05-29
**Author:** PM (Cerebro), drafting
**Status:** Ratified 2026-05-29 by project-lead approval. Implementation is a forward-marker (incremental, as drivers are written/refactored); most plausibly scheduled alongside Phase 6 accelerator-driver work, with opportunistic refactor of virtio-net/AHCI before then.
**ADR Number:** ADR-025.
**Linked to:** ADR-001 (monolithic kernel; drivers run in kernel mode), ADR-019 (lock-type discipline; this is the analogous hardware-access discipline), ADR-022 (fault-recovery forward-marker; companion forward-marker style).
**Bible articles potentially affected:** Article 4.2 (unsafe permitted only at hardware boundaries, reviewed line-by-line), Article 9.1 (uniform trait-based driver interface per device class), Article 15.2 (SAFETY comments on every unsafe block). Possible future Article 9 clause naming typed register-access wrappers as the canonical hardware-access primitive.
**Prior art / provenance:** Jeremy Soller (Jackpot51), Redox OS AMA, r/rust, 2015 — observations on broken 2015-era `volatile`, LLVM reordering of MMIO, and the need for typed `PIO8/PIO16/PIO32` wrappers to prevent wrong-width port access; community `Volatile<T>` wrapper pattern (dbaupp) using volatile intrinsics. Shared by JC 2026-05-29 for reference. This ADR modernizes that 2015 guidance for the current stable toolchain (see "Modernization" below).

---

## Rationale

Driver code in a monolithic kernel touches device registers through raw addresses (MMIO BARs) and, on x86_64, through I/O ports (PIO). Two hazard classes arise that are NOT caught by Rust's safety guarantees and NOT caught by code review reliably:

1. **Compiler elision/reordering/coalescing of device accesses.** A device register is not normal memory: reading it can have side effects, writing it triggers hardware actions, and the same address can return different values on successive reads. An ordinary `*ptr` access lets the optimizer assume normal-memory semantics — it may cache the value, drop a "redundant" store, or reorder accesses relative to each other. For a device register this silently corrupts driver behavior. The access must be `volatile` so the compiler emits exactly the loads/stores written, in order, with no fusing.

2. **Wrong-width port access.** x86 `in`/`out` come in 8/16/32-bit forms. Issuing a byte read against a register that must be read as a word (or vice versa) is a class of bug the type system can prevent but raw `inb`/`outl` helpers do not. Soller's AMA calls this out directly as motivation for typed port wrappers.

Today these accesses are written ad-hoc per driver, each with its own `unsafe` block and its own (possibly missing) volatile discipline. That scatters hardware-boundary `unsafe` across the driver tree, contrary to the concentration-of-unsafe principle (Bible 4.2; cf. the Asterinas OSTD "all unsafe in a small audited core" direction noted in the ALTIMIT research report).

The proposal: a small set of typed, audited register-access primitives that every driver uses, so the volatile discipline and the width discipline are structural rather than per-call-site — the same move ADR-019 made for IRQ-safe locking.

## Non-goal / critical clarification: this is NOT a synchronization primitive

Volatile access governs **how a single CPU talks to a device**. It is emphatically **not** a cross-CPU ordering or atomicity tool.

- `volatile` prevents the *compiler* from eliding/reordering/fusing accesses. It does NOT emit CPU memory barriers and provides NO happens-before guarantee between threads/CPUs.
- Cross-CPU synchronization (the wait/wake, queue, and scheduler paths) is the job of `core::sync::atomic` with explicit `Ordering`, and of `IrqSafeMutex` (ADR-019) — NOT volatile.

This distinction is load-bearing and is stated here precisely because the two are routinely conflated. A multi-waiter SMP race is an atomics/ordering/lock problem; reaching for `volatile` to "fix" it would be a category error. This ADR's wrappers must never be presented as a concurrency tool, and driver code must not use them as one.

## Modernization (what changed since the 2015 source)

The 2015 Redox observations were partly about a *broken toolchain*: `volatile` semantics were unreliable, and LLVM mis-handled some intrinsics. That era is over:

- The correct, stable primitive today is `core::ptr::read_volatile` / `write_volatile` (stable since Rust 1.9). These are well-defined and not the broken 2015 form.
- We therefore do NOT need to patch libcore or hand-roll intrinsics. We wrap the stable volatile functions in typed newtypes.
- The enduring, still-correct part of the 2015 guidance is the *pattern*: typed wrappers for MMIO and width-typed wrappers for PIO. That pattern is what this ADR adopts.

## Decision context

### Current state

- MMIO register access in drivers (virtio-net landed Phase 4 BUILD-1a; AHCI/block from Phase 3; future GPU/accel BARs in Phase 6) is done via raw pointer reads/writes inside per-driver `unsafe` blocks.
- PIO access (serial/COM1, PIC, PIT, and similar Phase-1 bringup hardware) uses ad-hoc `in*/out*` helpers.
- No type-level distinction enforces volatile-ness or port width.

### Proposed state

Two primitive families, owned centrally (proposed locations; final paths set at implementation):

- **`kernel/src/sync/mmio.rs` (or `arch`-neutral `hal/mmio.rs`):**
  - `Mmio<T>` — a typed wrapper over a register address. `read()` -> `read_volatile`; `write(v)` -> `write_volatile`. `T` constrained to the integer register widths (`u8/u16/u32/u64`). Read-only and write-only variants (`MmioR<T>`, `MmioW<T>`) for registers that must not be written/read, so misuse is a type error.
  - A `RegisterBlock` helper pattern (offset-typed fields over a base) so a driver declares its register map once and accesses fields by name, not by raw offset arithmetic.
  - Where device-observable ordering beyond per-access volatile is required, an explicit `fence()` helper (arch memory fence) is provided and documented — separately from the volatile access, never implied by it.

- **`kernel/src/arch/x86_64/pio.rs`:**
  - `Pio8`, `Pio16`, `Pio32` — width-typed port wrappers. Each owns a `u16` port number; `read()`/`write()` emit `inb/outb`, `inw/outw`, `inl/outl` respectively. Mixing a width against the wrong register is impossible because the width is in the type.
  - PIO is x86-specific; on aarch64/riscv64 (forward-marker arches) the PIO module is absent and only MMIO applies. This is intentional and matches the universal-substrate posture.

Every wrapper concentrates its `unsafe` into one audited file with a single SAFETY justification per primitive, replacing N scattered per-driver unsafe blocks. Drivers then contain little or no raw `unsafe` for register access.

### Migration

Forward-marker / incremental, not a big-bang rewrite:
- New drivers (Phase 6 accel/GPU, future NICs) use the wrappers from day one.
- Existing drivers (virtio-net, AHCI/block, Phase-1 PIO hardware) are refactored opportunistically when next touched, not in a dedicated sweep, to avoid churn risk on working code.
- Clippy/CI may later gain a lint flagging raw `read_volatile`/`write_volatile`/`in*`/`out*` outside the wrapper modules (deferred; not part of initial adoption).

## Concrete proposal (API sketch — illustrative, not final)

```rust
// hal/mmio.rs
pub struct Mmio<T> { addr: *mut T }
impl<T: RegisterInt> Mmio<T> {
    /// SAFETY (single, audited): addr is a valid device-register address for the
    /// lifetime of this wrapper; access is volatile so the compiler emits exactly
    /// these accesses in order. Not a cross-CPU ordering primitive.
    pub fn read(&self) -> T { unsafe { core::ptr::read_volatile(self.addr) } }
    pub fn write(&self, v: T) { unsafe { core::ptr::write_volatile(self.addr, v) } }
}

// arch/x86_64/pio.rs
pub struct Pio8(u16);
impl Pio8 {
    pub fn read(&self) -> u8 { /* inb */ }
    pub fn write(&self, v: u8) { /* outb */ }
}
// Pio16 -> inw/outw ; Pio32 -> inl/outl
```

## Consequences

**Positive:**
- Volatile and width discipline become structural; whole bug classes (elided/reordered MMIO, wrong-width PIO) are unrepresentable in driver code.
- Hardware-boundary `unsafe` is concentrated and auditable (Bible 4.2 honored by construction; OSTD-style discipline).
- Self-documenting register maps; fewer raw-offset errors.
- Consistent with ADR-019's "make the invariant a type" philosophy.

**Negative / costs:**
- Some boilerplate to declare register blocks.
- Incremental migration means two styles coexist until old drivers are refactored.
- The volatile-vs-atomic distinction must be taught and enforced in review so the wrappers are never misused as a sync tool.

**Neutral:**
- No runtime cost: wrappers are zero-overhead; volatile compiles to the same instruction as the raw access, just without the optimizer freedoms.

## Open questions (for ratification discussion)

- Q1. Module home: arch-neutral `hal/mmio.rs` vs `sync/mmio.rs` vs `drivers/common/`? (Leaning `hal/` to keep it driver-class-agnostic.)
- Q2. Do we want `RegisterBlock` derive-macro ergonomics now, or hand-written register maps first and macro later? (Leaning hand-written first; macro is a later convenience.)
- Q3. Should the CI lint against raw volatile/port access ship with adoption or as a follow-up? (Leaning follow-up, after the wrappers exist and a few drivers use them.)
- Q4. Fence policy: enumerate exactly which device interactions need an explicit arch fence beyond volatile (x86 UC memory is strongly ordered, but virtio rings and some BARs have ordering requirements). Needs a short per-device-class note at implementation.

---

**Status: RATIFIED 2026-05-29.** Adopted as docs/ADR-025 per Bible Article 18.1/27 by project-lead approval. The decision is now binding; implementation is scheduled as a forward-marker (Phase 6 accelerator-driver work, with opportunistic refactor of virtio-net/AHCI before then). The four open questions are implementation-time details, not blockers to adoption.

---

## Addendum 2026-06-07 — aarch64 portability debt inventory

A Phase-6 audit of the driver layer (prompted by "are we holding the portability
invariant or accruing silent debt against the eventual aarch64 port?") found the
**hard** invariant held but a concentrated, partly-silent debt in the x86 platform
plumbing. This addendum records it so the debt is *tracked*, not silent — the
aarch64 bringup (post-Phase-5, per bible.md:133 / ADR-027) inherits this as its
substrate checklist. **No refactor is taken now**: abstracting against an unbuilt
second arch would be speculative generality. The goal here is visibility.

### Holding the invariant (no action needed)
- **Weak-memory ordering** — `drivers/block/virtio_barrier.rs` uses semantic
  `core::sync::atomic::fence(Ordering::Release/SeqCst)`, which lower to real `dmb`
  on aarch64 (not TSO-reliant compiler barriers). This is the hardest portability
  concern and it is correct; the QEMU aarch64 barrier-exposure CI lane
  (Phase 3, `phase3_aarch64_virtio_barrier_lane`) exists to validate exactly this.
- **Endianness** — all MMIO/descriptor access uses explicit `.to_le()/.from_le()`
  (never host-endian assumptions). Command encoding (`accel/intel_gen9_4a.rs`) is
  pure `.to_le_bytes()` — fully arch-neutral.
- **The Intel GPU driver proper** (`accel/accel_intel.rs`, `accel/intel_gen9_4a.rs`)
  pulls no `x86_64` crate types, no PIO, no TSC/PIT/MSR. Note also that this driver
  is *legitimately* platform-specific (integrated Intel-desktop hardware); an
  aarch64 board would run a different GPU driver entirely. Platform-specific drivers
  are not portability debt — only the **shared substrate** below is.

### Tracked debt (deferred to the aarch64 port)
| # | Coupling | Location | aarch64 replacement needed |
|---|---|---|---|
| 1 | PCI config via **CF8/CFC PIO** (`x86_64::instructions::port::Port`) — now guarded by a `compile_error!` tripwire | `drivers/pci.rs` | **ECAM** (MMIO config space, base from device tree/ACPI MCFG) |
| 2 | **MSI/MSI-X message address hardcoded `0xFEE00000`** (x86 LAPIC doorbell) | `drivers/pci.rs:396,437` | **GIC-ITS** doorbell address + device-ID/event-ID allocation |
| 3 | **Serial console via 16550 PIO** at COM1 `0x3F8` (`uart_16550::SerialPort`) | `drivers/console/serial.rs:18,28` | A PL011 (or platform UART) MMIO driver |
| 4 | **`x86_64`-crate address types** (`VirtAddr`/`PhysAddr`/`PhysFrame`/`Port`) — pervasive but shallow | `ahci`, `nvme`, `virtio_blk`, `virtio_net`, `virtio_accel`, `block/dma`, `console/serial` | An arch-neutral address newtype (or the kernel's own `mm` address types) |

### Enforcement
Debt item #1 now carries a `#[cfg(not(target_arch = "x86_64"))] compile_error!` in
`drivers/pci.rs`: a future non-x86 build fails with a pointed message (ECAM +
GIC-ITS needed) instead of an opaque `x86_64`-crate import error. Items #2–#4 are
recorded here only — they are no-ops on the sole current target (x86_64) and are
cheapest to address at port time, not speculatively now. When aarch64 bringup is
scheduled, this table is the starting work-breakdown.