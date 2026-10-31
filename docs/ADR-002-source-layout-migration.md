# ADR-002: Source Layout Migration — drivers/, init/, loader/

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-05-02 |
| **Authority** | Project Lead |
| **Supersedes** | — |
| **Superseded by** | — |
| **Related** | Bible Article 17, Article 27 · ADR-001 (governance precedent) · ANSPEC-001 §4.3 · `kernel/src/` tree at commit `8cbca36` |
| **Amends** | Bible Article 17 (canonical source layout list + reading-strictness clarification) |

---

## Context

Bible Article 17 specifies a canonical source-tree layout:

```
altimitneo/
├── kernel/
│   └── src/
│       ├── arch/x86_64/
│       ├── mm/
│       ├── sched/
│       ├── drivers/
│       ├── ai/
│       ├── fs/
│       ├── net/
│       ├── sync/
│       └── lib.rs
└── …
```

Article 17 also says: "Layout changes require a bible amendment."

The kernel as of commit `8cbca36` (Phase 1.4.1) does not match. Specifically:

- `kernel/src/serial.rs` exists at the top level. Article 17 says drivers live in `kernel/src/drivers/`. Serial is a console driver.
- `kernel/src/fb/` exists at the top level. Same problem; the framebuffer is a driver.
- `kernel/src/boot.rs` exists at the top level. Renders the cyberpunk boot frame to the framebuffer; runs once after `fb::init` and before halt-forever. Not a driver, not a long-lived subsystem, not in Article 17's list.
- `kernel/src/elf.rs` exists at the top level. ELF64 parser + load-into-Pmap, new from Phase 1.4. Same problem.
- `kernel/src/panic.rs` exists at the top level. Single `#[panic_handler]`. Not in Article 17's list.

This ADR resolves all five placements and clarifies a structural ambiguity in Article 17.

## The structural ambiguity

Article 17's source-tree diagram lists subsystem directories (`mm/`, `sched/`, `drivers/`, etc.) plus `lib.rs`. It does not show top-level utility files like `panic.rs` or `main.rs`. Article 17 then says "Layout changes require a bible amendment" — but does not define what counts as a layout change.

Two readings are possible:

**Strict reading.** The diagram is the canonical layout. Any file or directory not shown is a layout change requiring amendment. Top-level `panic.rs` requires an amendment to bless it; future top-level files (`version.rs`, `build_info.rs`, etc.) each require their own amendments.

**Loose reading.** The diagram specifies the *subsystem* layout — which directories must exist and where. Small top-level utility files (`panic.rs`, `main.rs`, future `version.rs`) live at the top level by convention without per-file amendments. Only new subsystem directories (`init/`, `loader/`, eventually `ipc/`) trigger amendments.

The strict reading turns the Bible into a per-file ledger. The loose reading preserves Article 17's authority over the things that actually matter (subsystem placement) without forcing amendments for trivial additions.

This ADR adopts the **loose reading** and amends Article 17 to make the reading explicit.

## Decision

### Layout migration

Five placements, effective immediately:

1. `kernel/src/serial.rs` → `kernel/src/drivers/console/serial.rs`
2. `kernel/src/fb/` → `kernel/src/drivers/fb/`
3. `kernel/src/boot.rs` → `kernel/src/init/boot_frame.rs`
4. `kernel/src/elf.rs` → `kernel/src/loader/elf.rs`
5. `kernel/src/panic.rs` — **keep at top level.** Not a subsystem, no internal structure, not init code (panics fire at any time). Its lifetime is the kernel's lifetime; it has no natural home in any subsystem.

`kernel/src/main.rs` also stays at top level. The kernel is a `[[bin]]`; `main.rs` is the canonical binary entry point. Bible Article 17 lists `lib.rs` reflecting the more common case (a kernel structured as a library with a thin binary wrapper); for our `[[bin]]` structure, `main.rs` serves the same role. A future workspace split into `kernel-lib` + `kernel-bin` is possible and would be its own ADR.

### Console-class driver placement

Console-class drivers live under `kernel/src/drivers/console/<name>.rs`, with `kernel/src/drivers/console/mod.rs` re-exporting public items. The first instance is `drivers/console/serial.rs`. Future console-class drivers (PS/2 keyboard for the kernel debug shell, eventual VT100-style console multiplexing) live alongside it without requiring a per-driver reorg.

Rationale: a flat `drivers/serial.rs` would force a reorg the moment a second console-class driver lands. The nested form costs one path segment now and saves churn later.

### Why each non-driver placement

**`init/boot_frame.rs`** — `boot.rs` is a one-shot post-init renderer. It consumes the framebuffer driver but is not part of it; drivers don't own their consumers, the same way a network driver doesn't contain `boot_motd.rs`. Top-level placement makes the top-level a junk drawer over time. `init/` future-proofs cleanly: `init/log.rs` (kernel banner/version line), `init/splash.rs`, `init/probe.rs` (early hardware enumeration before drivers are fully online) all have a natural home.

**`loader/elf.rs`** — ELF parsing + segment-loading is its own concern. It uses `mm` (allocates frames, maps pages via Pmap) but isn't part of `mm` any more than the network stack is part of `mm` because it allocates buffers. `loader/` is forward-looking, not speculative: ANSPEC-001 §4.3 specifies `sys_model_load` which parses `.neom` files — structurally identical to what ELF loading does (parse binary format, validate, allocate storage, bind into a process). Phase 5 lands `loader/neom.rs`; possibly `loader/common.rs` for shared validation/mapping helpers. Two separate top-level `elf.rs` and `neom.rs` would duplicate the framing.

**`panic.rs` at top level** — see "Decision" item 5. The Bible's silence on `panic.rs` reflects that Article 17 enumerates subsystems, not every file. A one-line comment at the top of `panic.rs` records the placement decision: `// See ADR-002: panic.rs lives at the kernel-src top level.`

### Macro path strategy

`serial_print!` and `serial_println!` are kept at the crate root via re-export:

```rust
// in kernel/src/main.rs (or a small kernel/src/console.rs facade)
pub use crate::drivers::console::serial::{serial_print, serial_println};
```

This preserves all existing call sites with no churn at use-sites.

The `_print` helper that the macros expand to is updated at its definition: any `$crate::serial::_print` in the macro body becomes `$crate::drivers::console::serial::_print`. Macros are expanded at use-sites with the path embedded, so the path inside the macro body is what matters — re-exporting at the crate root does not fix this. The macro body update is mechanical (one or two grep-and-replace edits) and is included in the implementation checklist below.

### Article 17 amendment

Bible Article 17 is amended in the same commit as this ADR. The amendment text follows.

#### Amendment 1: source-tree diagram

The diagram in Article 17 is replaced with:

```
altimitneo/
├── boot/                  # Limine config, boot artifacts
├── kernel/                # The kernel crate (#![no_std])
│   └── src/
│       ├── arch/x86_64/   # Architecture-specific code
│       ├── mm/            # Memory management
│       ├── sched/         # Scheduler, threads, processes
│       ├── drivers/       # Device drivers by class
│       │   ├── console/   #   serial, future PS/2 keyboard, VT mux
│       │   └── fb/        #   framebuffer driver
│       ├── init/          # One-shot boot-time code (boot frame, banner)
│       ├── loader/        # Binary-format parsers (ELF; .neom in Phase 5+)
│       ├── ai/            # AI/ML subsystem (Phase 5+)
│       ├── fs/            # Filesystem (Phase 3+)
│       ├── net/           # Network stack (Phase 4+)
│       ├── sync/          # Locks, atomics, RCU
│       └── main.rs        # Binary entry point (the kernel is a [[bin]])
├── userland/
│   ├── libneo/            # Userspace runtime (#![no_std])
│   ├── neo-init/          # PID 1
│   └── tools/             # Userspace utilities
├── tools/
│   └── xtask/             # Build orchestrator
├── docs/
│   ├── bible.md           # This document
│   ├── phases/            # Per-phase plans and verification logs
│   ├── specs/             # Technical specifications (ANSPEC-NNN, etc.)
│   └── adr/               # Architecture Decision Records
└── tests/                 # Integration test harnesses
```

#### Amendment 2: clarifying paragraph

The following paragraph is appended to Article 17 immediately after "Layout changes require a bible amendment":

> Article 17's source-tree diagram specifies the **subsystem** layout — which directories must exist and where. Small top-level utility files (`panic.rs`, `main.rs`, future `version.rs`, etc.) live at the top level by convention and do not require per-file Bible amendments. New subsystem directories require an ADR + Bible amendment via the standard Article 27 process. The distinction is whether a structural unit is being added (subsystem = amendment required) or a single file is being placed (utility file = no amendment required).

#### Bible version

The Bible version bumps from `1.0.0` to `1.0.1`. This is a minor amendment per Article 27.

## Consequences

### Positive

- **Article 17 is honored.** All driver code lives in `drivers/`. Boot-time renderer has a home in `init/`. Loader code has a home in `loader/`. Top-level is reserved for the binary entry, the panic handler, and small utility files.
- **Future Phase 5 work has a home.** `loader/neom.rs` slots in cleanly next to `loader/elf.rs`. The structural framing carries forward.
- **The loose reading is recorded.** Future ADRs (ADR-NNN: add `init/probe.rs`, etc.) don't need to debate whether they're "layout changes." The amended Article 17 says only new subsystem directories trigger amendments.
- **`drivers/console/` is established** as the home for console-class drivers. Future work (PS/2 keyboard for the kernel debug shell, eventual VT100-style console multiplexing) lives there.

### Negative

- **Touches a lot of files.** Every reference to `crate::serial::*`, `crate::fb::*`, `crate::boot::*`, `crate::elf::*` updates. Macro bodies that hardcode `$crate::serial::_print` update too. Estimated ~30 import sites + macro paths.
- **Phase 1.4.x diff is large but mechanical.** Reviewers must distinguish "real change" (none) from "import-path churn" (most of the diff). Mitigation: this commit contains only the move, the new module declarations, the import updates, and the Bible/ADR text. No behavioral changes.
- **`init/` and `loader/` exist with one file each at first.** Looks empty for a phase or two. Acceptable; the framing is forward-looking by design.

### Neutral

- **`fb/` keeps its internal layout** (`mod.rs`, `font.rs`) under the new `drivers/fb/` path. No change to the framebuffer driver's internals.
- **No code behavior changes.** The boot trace produced by the kernel after this commit is identical to the boot trace before, except for path-related diagnostic messages if any exist (none currently).

## Alternatives Considered

### Alternative 1: Adopt the strict reading instead

Rejected. Under the strict reading, every top-level file (`panic.rs`, `main.rs`, future `version.rs`, etc.) requires an Article 17 amendment to bless its existence. This makes the Bible a per-file ledger. Article 27 amendments are not casual; using them for trivial top-level files devalues the amendment mechanism. The loose reading is what kernel-engineering practice actually expects from a layout document.

### Alternative 2: Move `panic.rs` to `init/panic.rs` for consistency

Rejected. Panic handlers fire at any time, not just at init. Putting `panic.rs` in `init/` would mislead future readers about when it runs.

### Alternative 3: Move `boot.rs` into `drivers/fb/`

Rejected. Inverts the dependency direction. `boot.rs` consumes the framebuffer driver; the driver does not own its consumer. The same logic that says a network driver doesn't contain `boot_motd.rs` says the framebuffer driver doesn't contain `boot_frame.rs`.

### Alternative 4: Move `elf.rs` into `mm/`

Rejected. ELF parsing is structurally distinct from memory management. The fact that the loader calls `Pmap::map_user` and `with_frame_alloc` doesn't make it part of `mm` — by that logic, the network stack would be part of `mm` because it allocates buffers. Loaders, drivers, and subsystems all use `mm`; that doesn't make them members of it.

### Alternative 5: Defer the migration until Phase 5

Rejected. The migration is needed before Phase 1.5 begins. Phase 1.5 lands multiple userspace processes + IPC + signals, all of which add files. Adding them under the wrong layout and migrating later costs more than migrating now.

### Alternative 6: Flat `drivers/serial.rs` instead of nested `drivers/console/serial.rs`

Rejected. Serial is a console-class driver. Future PS/2 keyboard and VT-mux work will also be console-class. A flat `drivers/serial.rs` forces a reorg the moment the second console driver lands. The nested form costs one path segment now and avoids that future churn.

## Implementation

This ADR is implemented by the migration commit landing alongside it:

- [ ] `kernel/src/serial.rs` → `kernel/src/drivers/console/serial.rs`
- [ ] `kernel/src/fb/` → `kernel/src/drivers/fb/`
- [ ] `kernel/src/boot.rs` → `kernel/src/init/boot_frame.rs`
- [ ] `kernel/src/elf.rs` → `kernel/src/loader/elf.rs`
- [ ] `kernel/src/panic.rs` — no move; one-line comment added at top of file: `// See ADR-002: panic.rs lives at the kernel-src top level.`
- [ ] New `kernel/src/drivers/mod.rs` declaring `pub mod console; pub mod fb;`
- [ ] New `kernel/src/drivers/console/mod.rs` declaring `pub mod serial;` and re-exporting public items
- [ ] New `kernel/src/drivers/fb/mod.rs` (preserves existing `fb/mod.rs` content under new path)
- [ ] New `kernel/src/init/mod.rs` with `pub mod boot_frame;`
- [ ] New `kernel/src/loader/mod.rs` with `pub mod elf;`
- [ ] `kernel/src/main.rs` mod declarations updated; macro re-exports added at crate root: `pub use crate::drivers::console::serial::{serial_print, serial_println};`
- [ ] All `use` paths in the kernel updated
- [ ] Macro bodies updated: any `$crate::serial::_print` → `$crate::drivers::console::serial::_print`
- [ ] `docs/bible.md` Article 17 amended per "Article 17 amendment" section above; Bible version bumped to `1.0.1`
- [ ] Boot-verified: ping/pong/hog smoke test + neo-init `hello from ring 3` produce identical serial trace before and after the move (diff the serial logs; expect zero differences)

## Notes

This ADR ratifies a discipline that was already implicit in Article 17. The amendment to Article 17 makes it explicit: subsystems are bible-controlled, top-level utility files are not.

Future ADRs that add subsystem directories (e.g., `ipc/` in Phase 1.5, `cap/` in Phase 2) follow this template: state the new directory, state what lives there, state what *does not* live there, amend Article 17 to add it to the canonical list.

### What this ADR does not do

This ADR addresses **deviation #2 only** (source layout). It does not address:

- **Deviation #3** (phase numbering offset between Bible and current commits) — handled by a separate small ADR or fix-up commit landing in the same Phase 1.5 prep window.
- **Deviation #4** (phase docs living off-tree) — handled by a follow-up that establishes `docs/phases/` and migrates the existing PRDs in. Mechanical, not architectural; does not need its own ADR.
- **Deviations #6, #7, #8** (Rust 2024 edition bump, KASLR enable, SMEP/SMAP enable) — separate work. The security ones (#7, #8) likely warrant their own ADR-003 covering the Phase 1 security debt and catch-up plan, since Article 12.3 says these are mandatory from Phase 1 and we are already in Phase 1.4.
- **Deviation #10** (no test infrastructure) — separate work, ideally before Phase 2 closes per Bible Article 16.

This ADR is one piece of a pre-Phase-2 cleanup sequence, not the catch-all.

---

**Decision recorded.** Per Bible Article 27, this ADR is amendable by superseding ADR.
