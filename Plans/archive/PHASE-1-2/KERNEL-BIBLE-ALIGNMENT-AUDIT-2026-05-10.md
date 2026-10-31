# Kernel-Bible-Alignment Audit (2026-05-10)

**Date:** 2026-05-10 (later session, post-Sub-step-6 close)
**Type:** Source-reconnaissance audit. Output of Cerebro Task A. Captures the substrate state of the kernel-side rename pending after the 2026-05-10 docs-side rename (commit `cca6889`). Authority record for the forthcoming Jarvis kernel-rename dispatch.
**Substrate:** Project HEAD `3a2cb70260cc69ef82ad1964c9d34dea300253b3`. Local `main` matches `origin/main`.
**Status:** Findings complete. Boot-identity sub-decision ratified by PL: **A3** (split identity — formal name + tagline byline). Below is the canonical target list for Jarvis to consume.

---

## 1. Scope

This audit checked the kernel-side substrate for:

1. **Legacy bible-misalignment patterns** that should have been corrected during the SC-3B / rename cycle but might have been missed in kernel source: `ALTFS`, `.neo-model`, `neo-inference`, `microkernel` (as positive descriptor), `AltimitNeo` (capital).
2. **Pending rename substrate** — every remaining occurrence of `altimit` (case-insensitive) across `kernel/`, `boot/`, `tools/`, `userland/`, `tests/`, and root build files. The 2026-05-10 rename three-commit dispatch text-replaced only under `docs/` and `README.md` per its negative-list discipline; the kernel-side rename was deferred.

Areas excluded from this audit:

- `docs/` — covered by the prior rename dispatch. (`docs/design.md` is in inconsistent post-text-replace state and is the subject of a separate Prometheus B1 surgical-update dispatch.)
- `Plans/` — internal PM-tier artifacts; preserved as historical record per Bible 18.1.
- `CLAUDE.md` — contains RAG/Qdrant configuration referring to an external indexer's collection name (`altimitneo-code`) and on-disk RAG paths (`E:\AI Automation\RAG\AltimitNeo`). These are **out of scope** for this audit; PL controls the RAG indexer separately.
- `target/` — build artifacts, regenerated from source on next build.
- Generated files — `Cargo.lock` updates automatically when package names change.

---

## 2. Legacy bible-misalignment patterns — all clean

| Pattern | Hits in `kernel/` | Verdict |
|---|---|---|
| `ALTFS` (case-insensitive) | 0 | Clean — Bible 10.2 `NeoFS` honored |
| `.neo-model` (literal) | 0 | Clean — Bible 6.2 `.neom` honored |
| `neo-inference` (literal) | 0 | Clean |
| `microkernel` (case-insensitive) | 0 | Clean — Bible 4.1 monolithic honored; no positive or negative occurrences anywhere in `kernel/` |
| `AltimitNeo` (case-sensitive PascalCase) | 0 | Clean |

No kernel-side carry-over of legacy-name baggage. The bible-faithful surface in kernel source is in good shape.

---

## 3. Pending rename substrate — file-by-file targets

The 2026-05-10 docs-side rename dispatch explicitly preserved kernel/, boot/, tools/, Plans/ from text-replace per its negative-list discipline. Those files remain at the pre-rename naming. This section enumerates each.

**Rename rules (per Bible 25.1 + 25.2):**

- Project name `AltimitNeo` → `NeoPlutarch`.
- Display brand `ALTIMIT NEO` / `ALTIMIT  NEO` → see §4 (split-identity decision).
- Package/identifier `altimit-neo` → `neoplutarch`.
- Target name `x86_64-altimit` → `x86_64-neoplutarch`; `x86_64-altimit-user` → `x86_64-neoplutarch-user`.
- `neo-` prefix preserved as established convention (Bible 25.2): `neo-init`, `neo-worker`, `libneo`, `.neom`, `NeoFS` all stay. Only `altimit`-containing identifiers change.

### 3.1 Cargo packages + target files

| File | Line(s) | Current | Target |
|---|---|---|---|
| `kernel/Cargo.toml` | 2 | `name = "altimit-neo-kernel"` | `name = "neoplutarch-kernel"` |
| `kernel/.cargo/config.toml` | 2 | `target = "x86_64-altimit.json"` | `target = "x86_64-neoplutarch.json"` |
| `kernel/.cargo/config.toml` | 12 | `[target.x86_64-altimit]` | `[target.x86_64-neoplutarch]` |
| `kernel/x86_64-altimit.json` | (filename) | — | rename file to `kernel/x86_64-neoplutarch.json` |
| `userland/x86_64-altimit-user.json` | (filename) | — | rename file to `userland/x86_64-neoplutarch-user.json` |
| `userland/neo-init/.cargo/config.toml` | 2 | `target = "../x86_64-altimit-user.json"` | `target = "../x86_64-neoplutarch-user.json"` |
| `userland/neo-init/.cargo/config.toml` | 10 | `[target.x86_64-altimit-user]` | `[target.x86_64-neoplutarch-user]` |
| `userland/neo-worker/.cargo/config.toml` | 2 | `target = "../x86_64-altimit-user.json"` | `target = "../x86_64-neoplutarch-user.json"` |
| `userland/neo-worker/.cargo/config.toml` | 8 | `[target.x86_64-altimit-user]` | `[target.x86_64-neoplutarch-user]` |
| `Cargo.lock` | 6 | `name = "altimit-neo-kernel"` | regenerates on next build; do not hand-edit |
| `Cargo.toml` (root) | 26 | `authors = ["ALTIMIT NEO Project"]` | `authors = ["NeoPlutarch Project"]` |
| `Cargo.toml` (root) | 28 | `repository = "https://github.com/yourname/altimit-neo"` | `repository = "https://github.com/SecondStageTurbine/NeoPlutarch"` (per hybrid git posture) |

### 3.2 xtask build orchestration

`tools/xtask/src/main.rs`:

| Line | Current | Target |
|---|---|---|
| 1 | `//! Build orchestration for ALTIMIT NEO.` | `//! Build orchestration for NeoPlutarch.` |
| 21 | `const KERNEL_PACKAGE: &str = "altimit-neo-kernel";` | `const KERNEL_PACKAGE: &str = "neoplutarch-kernel";` |
| 23 | `const KERNEL_TARGET: &str = "x86_64-altimit";` | `const KERNEL_TARGET: &str = "x86_64-neoplutarch";` |
| 31 | `const USER_TARGET: &str = "x86_64-altimit-user";` | `const USER_TARGET: &str = "x86_64-neoplutarch-user";` |
| 35 | `const ISO_OUTPUT_REL: &str = "boot/altimit-neo.iso";` | `const ISO_OUTPUT_REL: &str = "boot/neoplutarch.iso";` |
| 349 | `println!("ALTIMIT NEO build orchestration");` | `println!("NeoPlutarch build orchestration");` |
| 615 | `println!("ALTIMIT NEO QEMU integration tests");` | `println!("NeoPlutarch QEMU integration tests");` |
| 1044 | `eprintln!("[xtask]   ALTIMIT NEO ready.");` | `eprintln!("[xtask]   NeoPlutarch ready.");` |

### 3.3 Userland crates

| File | Line(s) | Current | Target |
|---|---|---|---|
| `userland/libneo/Cargo.toml` | 7 | description with "ALTIMIT NEO" | replace "ALTIMIT NEO" → "NeoPlutarch" |
| `userland/libneo/src/lib.rs` | 1 | `//! ALTIMIT NEO userland library.` | `//! NeoPlutarch userland library.` |
| `userland/neo-init/Cargo.toml` | 7 | description with "ALTIMIT NEO" | replace "ALTIMIT NEO" → "NeoPlutarch" |
| `userland/neo-init/src/main.rs` | 1 | `//! \`neo-init\` — the first userspace process under ALTIMIT NEO.` | replace "ALTIMIT NEO" → "NeoPlutarch" |
| `userland/neo-worker/Cargo.toml` | 7 | description with "ALTIMIT NEO" | replace "ALTIMIT NEO" → "NeoPlutarch" |

### 3.4 Boot config

`boot/limine.conf`:

| Line | Current | Target |
|---|---|---|
| 1 | `# ALTIMIT NEO — Limine bootloader configuration.` | `# NeoPlutarch — Limine bootloader configuration.` |
| 10 | `interface_branding: ALTIMIT NEO` | `interface_branding: NeoPlutarch` |
| 17 | `/ALTIMIT NEO` (entry name) | `/NeoPlutarch` (entry name) |

### 3.5 Kernel linker + Dockerfile + Compose

| File | Line(s) | Current | Target |
|---|---|---|---|
| `kernel/linker.ld` | 1 | `/* ALTIMIT NEO Kernel Linker Script` | `/* NeoPlutarch Kernel Linker Script` |
| `Dockerfile` | 1 | `# ALTIMIT NEO — development container.` | `# NeoPlutarch — development container.` |
| `docker-compose.yml` | 1 | `# ALTIMIT NEO — Docker Compose dev environment.` | `# NeoPlutarch — Docker Compose dev environment.` |
| `docker-compose.yml` | 28 | `image: altimit-neo-dev:latest` | `image: neoplutarch-dev:latest` |
| `docker-compose.yml` | 29 | `container_name: altimit-neo-dev` | `container_name: neoplutarch-dev` |
| `docker-compose.yml` | 45 | `image: altimit-neo-dev:latest` | `image: neoplutarch-dev:latest` |
| `docker-compose.yml` | 46 | `container_name: altimit-neo-vnc` | `container_name: neoplutarch-vnc` |

### 3.6 Tests README

| File | Line | Current | Target |
|---|---|---|---|
| `tests/README.md` | 1 | `# ALTIMIT NEO Integration Tests` | `# NeoPlutarch Integration Tests` |

### 3.7 Kernel source comments (mechanical, no displayed-string component)

These are doc-comments and code comments only; not user-visible. Pure search-and-replace from "ALTIMIT NEO" → "NeoPlutarch" or "altimit-neo" → "neoplutarch" as context dictates.

| File | Line | Current |
|---|---|---|
| `kernel/src/main.rs` | 1 | `//! ALTIMIT NEO Kernel — entry point.` |
| `kernel/src/main.rs` | 348 | `// Render the boot sequence. This is the iconic ALTIMIT moment.` |
| `kernel/src/init/boot_frame.rs` | 1 | `//! The ALTIMIT NEO boot sequence.` |
| `kernel/src/init/boot_frame.rs` | 38 | `/// Centered wordmark: "ALTIMIT NEO" stacked over a version line.` |
| `kernel/src/drivers/fb/mod.rs` | 36 | `// === ALTIMIT NEO palette ===` |
| `kernel/src/drivers/fb/mod.rs` | 76 | `"altimit-neo currently requires a 32-bpp framebuffer (got {})",` (error string — user-visible if hit; rename to `"neoplutarch currently requires..."`) |

The line 348 comment ("iconic ALTIMIT moment") captures design intent; the rename loses some of that flavor. Jarvis may rewrite to preserve the sentiment, e.g., `// Render the boot sequence. This is the iconic NeoPlutarch moment.` or keep the original phrasing as a historical-aesthetic note depending on PL preference. PM lean: simple text-replace; aesthetic preservation lives in `docs/design.md`'s vision narrative, not in code comments.

---

## 4. Boot identity — A3 ratified

**Decision (PL, 2026-05-10):** A3 — split identity. Formal project name `NEOPLUTARCH` + tagline byline `Intelligence as a kernel primitive.` rendered together at boot, reproducing the README's H1-over-blockquote visual hierarchy.

### 4.1 Framebuffer wordmark (`kernel/src/init/boot_frame.rs:40`)

**Current:**

```rust
let title = "ALTIMIT  NEO";
```

(Double-space deliberate; centers as a visually-split two-word wordmark.)

**Target:**

```rust
let title = "NEO  PLUTARCH";
let tagline = "Intelligence as a kernel primitive.";
```

Render `title` as the existing big wordmark; render `tagline` below it in smaller / lighter typography (whatever the existing rendering supports — italics, lower-intensity color, smaller scale). PM lean: preserve the double-space `"NEO  PLUTARCH"` to retain the cinematic split-word feel that `"ALTIMIT  NEO"` had. Alternative `"NEOPLUTARCH"` is acceptable if rendering constraints favor it; Jarvis decides during implementation.

If the existing rendering infrastructure doesn't support a subtitle line, Jarvis adds minimal support (one extra rendering call below the centered wordmark, same horizontal centering, smaller text). This is acceptable scope inside the rename dispatch.

### 4.2 Serial banner (`kernel/src/main.rs:254`)

**Current:**

```rust
serial_println!("  ALTIMIT NEO  v{}", env!("CARGO_PKG_VERSION"));
```

**Target:**

```rust
serial_println!("  NEOPLUTARCH  v{}", env!("CARGO_PKG_VERSION"));
serial_println!("  Intelligence as a kernel primitive.");
```

Two-line banner; tagline below the wordmark, matching the framebuffer hierarchy. Single-word "NEOPLUTARCH" on serial is fine (no rendering constraint).

### 4.3 Wordmark stylization notes

The audit recommends Jarvis defer to existing rendering code for typography decisions; the audit's content prescription is the **text** (`NEO  PLUTARCH` / `NEOPLUTARCH` + tagline), not the visual styling. If the rendering supports color, weight, scale variations, Jarvis applies what reads as the natural analog of "H1 over blockquote" in framebuffer context.

`docs/design.md` (after Prometheus B1 update) will describe the boot aesthetic in the vision narrative. The two should remain consistent; if Jarvis's rendering choices differ from the design.md description, surface to PM for reconciliation before commit.

---

## 5. Out-of-scope items (do not rename in this dispatch)

These items contain `altimit` references but are out of scope for the kernel-side rename:

| File | Reason |
|---|---|
| `CLAUDE.md` | RAG indexer collection name (`altimitneo-code`) and external on-disk paths (`E:\AI Automation\RAG\AltimitNeo`) refer to an external system the PM doesn't control. PL handles RAG rename separately. |
| `Plans/` (entire tree) | Internal PM-tier artifacts. Historical record per Bible 18.1. References to "AltimitNeo" in `Plans/` are date-stamped and remain truthful for their date. |
| `docs/` (already-text-replaced files) | Covered by 2026-05-10 rename three-commit dispatch (commit `cca6889`). |
| `docs/design.md` | Separate Prometheus B1 dispatch handles surgical identity update. |
| `boot-log-*.txt` (project root) | Historical boot logs; never edit historical logs. |
| `target/` | Build artifacts, regenerate on build. |

---

## 6. Implementation guidance for Jarvis

This audit informs the Jarvis dispatch but does not substitute for the dispatch itself. The forthcoming dispatch will specify:

- **Commit boundaries.** PM lean: one commit for the rename, separate commits for ADR-019 sub-phase A (`IrqSafeMutex` introduction) and ADR-020 (LockOwner field). Three-commit bundle in one Jarvis dispatch with gates between.
- **Gate verification.** Each commit needs `cargo k` PASS at delta 0, plus `cargo neo-test all` NEO_SMP=4 PASS at the SC-3B closure-gate level. The rename commit specifically requires demonstrating that build orchestration still works after package/target rename (xtask is the critical verifier here).
- **Branch / worktree discipline.** Jarvis uses a `claude/*`-equivalent session branch convention; PM fast-forwards `main` after gates pass. Per hybrid git posture, push to `origin/main` after fast-forward.
- **Stash discipline.** Existing stashes `{0}` through `{3}` preserved untouched. No stash interactions during the dispatch.

### 6.1 Cargo package rename cascades

The most failure-prone single change is `altimit-neo-kernel` → `neoplutarch-kernel`. Touches:

1. `kernel/Cargo.toml` name field (the source of truth)
2. `Cargo.lock` (autoregenerates; verify on first build)
3. `tools/xtask/src/main.rs` `KERNEL_PACKAGE` constant
4. Any other place in `tools/`, `userland/`, or `kernel/` that references the package by name

Recommend Jarvis: change `Cargo.toml` first, run `cargo k`, expect failure, follow the error messages to find any missed references, fix iteratively until `cargo k` passes.

### 6.2 Target JSON file rename

`kernel/x86_64-altimit.json` → `kernel/x86_64-neoplutarch.json` is a file rename (git mv), not a content edit. Same for `userland/x86_64-altimit-user.json` → `userland/x86_64-neoplutarch-user.json`. Then update `.cargo/config.toml` files (3 of them) to reference the new names.

### 6.3 Boot wordmark verification

After the rename commit, Jarvis runs `cargo neo-tty` (or equivalent boot-in-QEMU command) and visually verifies the boot sequence renders:

- The wordmark "NEO  PLUTARCH" (or "NEOPLUTARCH") centered on the framebuffer
- The tagline "Intelligence as a kernel primitive." below it
- The serial banner showing both lines

Verification log captures a serial-output snapshot. If the framebuffer rendering looks substantially different from design intent (e.g., text wraps, doesn't center, etc.), surface to PM before commit.

---

## 7. Cross-references

**Audit substrate:**
- `git grep` output run 2026-05-10 against HEAD `3a2cb70260cc69ef82ad1964c9d34dea300253b3`.
- This document is the canonical audit artifact for the kernel-side rename.

**Forward dispatches (pending):**
- Prometheus dispatch: ADR-018 + ADR-019 + ADR-020 ratification work (lands ADR files in `docs/`, bible v1.5.0 amendment with Article 8.5.4 expansion, `docs/design.md` B1 surgical update).
- Jarvis dispatch: kernel-side rename (this audit's targets) + ADR-019 sub-phase A (introduce `IrqSafeMutex`) + ADR-020 implementation (`LockOwner` field). One bundle, three sub-commits with gates between.

**Authority:**
- Bible 25.1 (project name) + 25.2 (neo- prefix convention) — define the rename rules.
- Bible 4.1 (monolithic kernel) — confirmed honored in kernel source.
- Bible 6.2 (`.neom` format) — confirmed honored.
- Bible 10.2 (NeoFS) — confirmed honored.
- PL ratification 2026-05-10: A3 boot-identity decision.

**Source documents consulted:**
- `Plans/HANDOFF-2026-05-10-end-of-day-sc-3b-closed-rename-complete.md` §4.4 (kernel-bible-alignment audit forward marker).
- `Plans/DISPATCH-2026-05-10-rename-three-commit-prometheus.md` (the original rename dispatch and its negative-list discipline).
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` (Jarvis = Codex, kernel-implementer routing).
- `docs/bible.md` v1.4.0 (Article 25 current state).
- `docs/design.md` (vision doc — boot aesthetic referent for the A3 decision).

---

**End of audit.**

*Findings: 30+ rename targets across kernel/, boot/, tools/, userland/, tests/, root. Legacy bible-misalignment patterns clean. Boot identity decision A3 specifies the split-identity render (NEO PLUTARCH wordmark + "Intelligence as a kernel primitive." tagline byline). This artifact is the consumption substrate for the forthcoming Jarvis kernel-rename bundle dispatch.*
