# OBSERVE: phil-opp Writing an OS in Rust Survey

| | |
|---|---|
| Date | 2026-05-20 |
| Author | Cerebro |
| Dispatch | `Plans/DISPATCH-2026-05-20-phil-opp-writing-an-os-in-rust-observe-cerebro.md` |
| Type | OBSERVE-class retrospective cross-reference |
| Source under review | Philipp Oppermann, *Writing an OS in Rust*, `https://os.phil-opp.com` |
| Project substrate | NeoPlutarch Phase 0-3 as of the 2026-05-20 end-of-day handoff addendum |

---

## 1. Substrate State At Start Of Research

This pass starts from the state recorded in
`Plans/HANDOFF-2026-05-20-end-of-day-phase-3-substantially-complete.md` and
the addendum that dispatched this OBSERVE lane.

Relevant substrate summary:

- Phase 0 through Phase 2 are complete.
- Phase 3 is substantially complete in QEMU and awaits BUILD-5b bare-metal
  validation for formal Bible Article 16.3 closure.
- The kernel is a Rust `#![no_std]` monolith, with unsafe isolated at hardware
  boundaries per Bible Article 4.2 and Article 15.2.
- The boot path is Limine, not GRUB, not a custom bootloader, not `bootimage`,
  per Bible Article 4.4.
- Display output is a Limine-provided framebuffer plus serial logging, not VGA
  text mode, per Bible Article 9.8.
- x86_64 bring-up has GDT, IDT, TSS, IST, double-fault handling, timer paths,
  APIC/SMP infrastructure, and per-CPU state.
- Memory management includes Limine HHDM page-table access, kernel-half/user-half
  split, per-process PML4s, KASLR-aware heap placement, buddy frames, slab
  object pools, and an IRQ-safe global heap wrapper.
- Phase 3 storage has a synchronous `BlockDevice` trait and a `MountedFs` VFS
  line that compose across NeoFS, FAT32, ext4, virtio-blk, AHCI, and NVMe.
- Foreign filesystems are read-only in Phase 3 and default to `noexec`; foreign
  type import coerces regular file content to `Regular`, never `Model`.
- The canonical test path is `cargo neo-test` through the docker-compose `neo`
  service, with QEMU serial/debug-exit integration.

The current blog root was checked during this pass. It still presents the
second-edition series as the live series and lists `Async/Await` as the latest
post. It also exposes the first edition as no longer updated. One URL nuance:
the current second-edition double-fault post is
`https://os.phil-opp.com/double-fault-exceptions/`; the shorter
`https://os.phil-opp.com/double-faults/` resolves to the first-edition post.

---

## 2. TL;DR

- No NeoPlutarch gap was found against phil-opp's series. Gap count: 0.
- Cross-reference count: 15 rows total: 5 convergent, 5 divergent-intentional,
  4 substrate-extended, 1 not applicable.
- The largest intentional divergences are already Bible/ADR-grounded: Limine
  over `bootimage`/GRUB/custom boot, framebuffer over VGA text mode, `cargo
  neo-test` over the `bootimage` runner, and synchronous-blocking kernel I/O
  over a kernel async executor.
- phil-opp remains a useful pedagogical reference for early Rust OS mechanics,
  but NeoPlutarch's current substrate has moved beyond the tutorial in SMP,
  capability security, storage, filesystems, and AI-native type policy.

---

## 2.1.A Edition 2 Post Survey

| Edition 2 post | phil-opp topic | NeoPlutarch cross-reference |
|---|---|---|
| `A Freestanding Rust Binary` | `#![no_std]`, `#![no_main]`, panic handler, custom target, freestanding binary constraints. | Convergent. NeoPlutarch is Rust `#![no_std]`, has explicit panic paths, custom target/build orchestration, and treats OS-dependent Rust facilities as unavailable in-kernel. |
| `A Minimal Rust Kernel` | Boot process, BIOS/UEFI framing, target spec, `bootimage`, QEMU, first screen output. | Divergent-intentional. NeoPlutarch keeps the Rust kernel shape but uses Limine as the bootloader contract and does not use `bootimage`. |
| `VGA Text Mode` | Safe wrapper around VGA text memory, volatile MMIO, formatting macros, spinlock-protected global writer. | Divergent-intentional on device choice, convergent on encapsulated unsafe. NeoPlutarch targets Limine framebuffer plus serial, not VGA text mode. |
| `Testing` | `no_std` custom test framework, QEMU exit device, serial output, integration tests, timeouts. | Divergent-intentional on tooling. NeoPlutarch uses `cargo neo-test`, docker-compose, serial frames, scenario metadata, and QEMU debug-exit semantics rather than the `bootimage` runner. |
| `CPU Exceptions` | IDT setup, `extern "x86-interrupt"`, breakpoint/page-fault style handlers. | Convergent. NeoPlutarch has IDT handlers for CPU exceptions and serial diagnostics, with policy layered onto current Phase 1-3 needs. |
| `Double Faults` | Double-fault handling, IST/TSS/GDT interaction, stack-overflow resilience. | Convergent. NeoPlutarch has per-CPU GDT/TSS/IST storage and a double-fault IST path. It extends the model for SMP and additional IST users. |
| `Hardware Interrupts` | 8259 PIC, timer interrupt, keyboard input, deadlock/race exposure, `hlt`. | Substrate-extended. NeoPlutarch has early interrupt handling, timer paths, PS/2 graceful-degradation history, APIC/SMP/MSI paths, and written lock/interrupt discipline. |
| `Introduction to Paging` | Memory protection, segmentation, virtual memory, multilevel x86_64 paging. | Convergent. Bible Article 7 requires virtual memory, high-half kernel, per-process PML4s, and page-table enforcement rather than convention. |
| `Paging Implementation` | Techniques for accessing page tables, bootloader support, address translation, mapping, frame allocation. | Substrate-extended. NeoPlutarch uses Limine's HHDM and active CR3 tables, adds global-bit fixup, KASLR-aware heap placement, per-process PML4s, and SMP-aware TLB work. |
| `Heap Allocation` | `alloc`, `GlobalAlloc`, heap mapping, allocator crate, heap tests. | Convergent. NeoPlutarch uses `linked_list_allocator` in kernel and libneo, but the kernel wraps the heap with IRQ-safe discipline and later layers buddy/slab underneath. |
| `Allocator Designs` | Bump, linked-list, fixed-size block allocator designs, interior mutability through locks. | Substrate-extended. NeoPlutarch uses a bootstrap/buddy/slab split and makes scheduler heap-independence an Article 8.5/ADR-016 invariant. |
| `Async/Await` | Cooperative multitasking, Rust futures, executor, async keyboard task. | Divergent-intentional. NeoPlutarch does not add a kernel async runtime in Phase 3; ADR-021 Q1 chooses synchronous-blocking file/block I/O from filesystem callers. |

---

## 2.1.B Edition 1 Unique-Material Note

The first edition is explicitly marked by the source site as no longer updated.
It is still useful for identifying older material that Edition 2 either removes
or moves behind the bootloader crate.

Edition 1 material that matters for cross-reference:

- `A minimal Multiboot Kernel` and `Entering Long Mode` show GRUB/Multiboot,
  assembly-level long-mode entry, initial identity paging, and a hand-built
  GDT. NeoPlutarch intentionally delegates that boot-transition class to Limine
  and starts from the Limine boot protocol contract.
- First-edition paging posts (`Allocating Frames`, `Page Tables`, `Remap the
  Kernel`) expose frame allocation, recursive/table access, kernel remapping,
  and guard-page ideas. NeoPlutarch implements the corresponding concerns in a
  later and broader substrate: HHDM-backed page-table access, buddy frames,
  KASLR-tolerant heap placement, per-process PML4s, and TLB shootdown work.
- First-edition exception and double-fault material is mostly superseded by
  Edition 2, but the TSS/GDT/IST details remain directionally aligned with
  NeoPlutarch's current x86_64 implementation.
- Extra first-edition material such as cross-compiling binutils/libcore and
  GDB setup is not load-bearing for NeoPlutarch's current toolchain because the
  project uses its own Rust target, xtask, Limine ISO assembly, docker-compose,
  and `cargo neo-test` flow.

No first-edition-only topic creates a gap. The material either predates the
Limine decision, is already implemented through a different substrate, or is
tooling-specific to the tutorial.

---

## 2.1.C Cross-Reference Table

| # | Topic | Classification | NeoPlutarch state |
|---|---|---|---|
| 1 | Freestanding Rust kernel | Convergent | Rust `#![no_std]` kernel, custom target/build flow, explicit panic path. |
| 2 | Bootloader/tooling | Divergent-intentional | Bible Article 4.4 chooses Limine; `bootimage`, GRUB, and custom boot are intentionally outside the project path. |
| 3 | Screen output | Divergent-intentional | Bible Article 9.8 requires UEFI/Limine framebuffer from Phase 0; VGA text mode is not the display substrate. |
| 4 | Serial/test QEMU harness | Divergent-intentional | Bible Article 16.2 is satisfied through `cargo neo-test`, serial frames, and debug-exit integration. |
| 5 | IDT and exception handlers | Convergent | `kernel/src/arch/x86_64/idt.rs` has IDT entries and trap diagnostics. |
| 6 | Double fault, TSS, IST, GDT | Convergent | `kernel/src/arch/x86_64/gdt.rs` and `idt.rs` provide per-CPU TSS/IST and double-fault stack handling. |
| 7 | Basic hardware interrupts | Substrate-extended | NeoPlutarch goes beyond PIC/PIT pedagogy into APIC, SMP, MSI, and IRQ-safe lock discipline. |
| 8 | Virtual memory model | Convergent | Bible Article 7 mandates high-half kernel, lower-half userspace, and page-table enforcement. |
| 9 | Page-table access and mapping | Substrate-extended | Limine HHDM plus CR3 wrapping, global-bit fixup, per-process PML4, KASLR, and TLB shootdown extend the tutorial shape. |
| 10 | Frame allocation | Substrate-extended | The project moved from bootstrap allocation to buddy frames and slab object pools per Bible Article 7.4. |
| 11 | Heap allocation and `alloc` | Convergent | Kernel/libneo use `linked_list_allocator`; kernel adds IRQ-safe heap wrapper and scheduler decoupling. |
| 12 | Allocator experimentation | Substrate-extended | phil-opp surveys allocator designs; NeoPlutarch selected concrete boot, frame, slab, and heap disciplines. |
| 13 | Cooperative async executor | Divergent-intentional | ADR-021 Q1 rejects a Phase 3 async runtime; synchronous-blocking filesystem/block API is the chosen substrate. |
| 14 | First-edition GRUB/Multiboot long-mode path | Divergent-intentional | Superseded by the Limine boot protocol decision. |
| 15 | First-edition toolchain extras | Not applicable | Not load-bearing for the repository's Rust target, xtask, docker, and Limine flow. |

Gap rows: none.

---

## 2.1.D Gap Analysis

No genuine gap surfaced.

Items checked and ruled non-gaps:

- **Bootloader mismatch:** Not a gap. NeoPlutarch's Bible Article 4.4 explicitly
  chooses Limine because it provides HHDM, framebuffer, modules, and SMP support
  without legacy BIOS baggage.
- **VGA text mode absence:** Not a gap. Bible Article 9.8 requires the UEFI
  framebuffer path from Phase 0; VGA text mode is an intentional non-target.
- **`bootimage` runner absence:** Not a gap. Bible Article 16.2 requires QEMU
  integration tests, not phil-opp's runner. NeoPlutarch has a separate
  scenario-based `cargo neo-test` harness.
- **Kernel async executor absence:** Not a gap. ADR-021 Q1 explicitly says
  Phase 3 has no general async runtime and uses synchronous-blocking file I/O.
- **GRUB/Multiboot/hand long-mode code absence:** Not a gap. This is pre-Limine
  first-edition material and conflicts with the current bootloader doctrine.
- **Foreign filesystem semantic type import:** Not a phil-opp topic and not a
  gap. Bible Article 10.3 and ADR-021 Q9 require inode-level type policy;
  foreign filesystems are coerced to `Regular` and default `noexec`.
- **Phase 3 bare-metal validation still pending:** Real, but unrelated to
  phil-opp's tutorial coverage. It is already recorded as BUILD-5b hardware
  gating in the handoff.

Stop-condition check:

- No hard conflict with Bible/ADR found.
- No load-bearing gap for the current Phase 3 or near-term Phase 4 substrate
  found.
- No contradiction to a safety invariant found.
- No THINK-class design selection made in this artifact.
- Source state did not materially change in a way that invalidates dispatch.

---

## 2.1.E Opportunistic Pattern Analysis

This section is descriptive only.

**Pattern 1: bootloader as substrate boundary.** phil-opp's second edition
already moved work out of the tutorial kernel and into the `bootloader` crate.
NeoPlutarch takes the same broad shape further by making Limine the stable
boot-protocol line. The direction is convergent even though the concrete tool
choice differs.

**Pattern 2: isolate unsafe hardware details behind small modules.** The VGA,
IDT, port-I/O, paging, and allocator posts repeatedly put unsafe operations
behind narrow Rust interfaces. NeoPlutarch's driver, arch, mm, sync, and VFS
modules follow the same high-level discipline, with stronger Article 15.2
comment requirements.

**Pattern 3: tutorial single-core assumptions become explicit project
invariants or deferrals.** phil-opp often begins with a single-core kernel and
pedagogical global state. NeoPlutarch has already paid the Phase 2 cost:
per-CPU GDT/TSS/IST, per-CPU scheduler state, SMP wakeups, and written lock/IRQ
discipline.

**Pattern 4: simple I/O first, production policy later.** phil-opp uses serial,
debug-exit, PIC, and VGA as simple mechanisms to make the kernel observable.
NeoPlutarch keeps the observability principle but replaces the mechanisms with
serial-frame scenarios, Limine framebuffer, APIC/MSI, and block-driver
composition.

**Pattern 5: type and capability policy are outside phil-opp's scope.** The
tutorial is about OS bring-up. NeoPlutarch's typed-inode, FileCap, mount-mask,
and AI model-file invariants are project-specific synthesis above that level.

No recommendation is made here.

---

## 2.1.F Intentional Divergence Record

| Divergence | Intentional basis |
|---|---|
| Limine instead of `bootimage`, GRUB, or custom boot | Bible Article 4.4; ADR-013 also relies on Limine MP for SMP bring-up. |
| UEFI/Limine framebuffer instead of VGA text mode | Bible Article 9.8; legacy BIOS is not a target per Article 9.3 and 9.6. |
| `cargo neo-test`/docker/QEMU scenario harness instead of `bootimage` runner | Bible Article 16.2 requires QEMU integration testing; repository tooling implements a project-specific harness. |
| Synchronous-blocking filesystem/block API instead of a kernel async executor | ADR-021 Q1: no general async runtime in Phase 3, single-thread-per-process invariant remains, drivers may complete through IRQ internally. |
| Kernel-managed OS threads, no kernel green-thread abstraction | Bible Article 8.4. Userspace may build coroutines, but the kernel sees OS threads. |
| APIC/MSI/SMP interrupt substrate beyond tutorial PIC model | Bible Article 8.3, Article 9.9, and Phase 3 BUILD-5a AHCI/NVMe needs. |
| Heap/allocator discipline beyond tutorial allocator examples | Bible Article 8.5 and ADR-016 require IRQ-safe heap behavior and scheduler heap-independence. |
| Native NeoFS plus read-only FAT32/ext4 interop instead of generic POSIX storage | Bible Article 10.1 and 10.4; ADR-021 Q9 covers foreign type import/noexec policy. |
| `.neom` type policy is inode-level, not filename- or xattr-trusted | Bible Article 10.3; ADR-021 Q9. |
| No embedded model or AI assistant behavior implied by OS bring-up | Bible Article 23.1 and 23.3. |

---

## References

Primary source material:

- `https://os.phil-opp.com/`
- `https://os.phil-opp.com/freestanding-rust-binary/`
- `https://os.phil-opp.com/minimal-rust-kernel/`
- `https://os.phil-opp.com/vga-text-mode/`
- `https://os.phil-opp.com/testing/`
- `https://os.phil-opp.com/cpu-exceptions/`
- `https://os.phil-opp.com/double-fault-exceptions/`
- `https://os.phil-opp.com/hardware-interrupts/`
- `https://os.phil-opp.com/paging-introduction/`
- `https://os.phil-opp.com/paging-implementation/`
- `https://os.phil-opp.com/heap-allocation/`
- `https://os.phil-opp.com/allocator-designs/`
- `https://os.phil-opp.com/async-await/`
- `https://os.phil-opp.com/edition-1/`
- `https://os.phil-opp.com/entering-longmode/`
- `https://os.phil-opp.com/double-faults/`

NeoPlutarch local references:

- `docs/design.md` Section 9, which acknowledges phil-opp as a project
  reference.
- `docs/bible.md` Articles 4, 7, 8, 9, 10, 14, 15, 16, 18, 23, and 26.
- `docs/ADR-013-phase-2-smp-retrofit.md`
- `docs/ADR-015-kaslr-tolerant-heap-placement.md`
- `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md`
- `docs/ADR-021-phase-3-design-substrate.md`
- `Plans/HANDOFF-2026-05-20-end-of-day-phase-3-substantially-complete.md`
- `Plans/HANDOFF-2026-05-20-end-of-day-phase-3-substantially-complete-ADDENDUM.md`
- `kernel/src/arch/x86_64/gdt.rs`
- `kernel/src/arch/x86_64/idt.rs`
- `kernel/src/mm/paging.rs`
- `kernel/src/mm/heap.rs`
- `kernel/src/drivers/block/mod.rs`
- `kernel/src/fs/mod.rs`
- `tools/xtask/src/main.rs`

---

> this artifact is observe-class per bible article 14; it maps terrain and
> does not choose routes. it is informational per bible article 18.1; it
> does not bind the project. phil-opp's blog is acknowledged as a project
> reference per design.md §9; this artifact records the formal
> cross-reference against current neoplutarch substrate. no follow-on
> action is implied; pm decides whether any surfaced item warrants
> dispatch.
