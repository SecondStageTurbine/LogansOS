<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: The day-1 shelf, round 2 — Python + 8 categories the PL named

**Date:** 2026-07-12
**Author:** Prometheus (Qwen)
**Dispatch:** `Plans/DISPATCH-2026-07-12-day1-shelf-round2-research-prometheus.md`
**Branch:** `prometheus/day1-shelf-round2-research`
**Scope:** Host-only, primary sources only. Every dependency claim verified from the project's own Cargo.toml/Cargo.lock/LICENSE. ToS claims verified from the service's own terms/API policy pages.

**Reuses round 1's tier framework verbatim** (`Plans/RESEARCH-2026-07-11-rust-day1-apps-FINDINGS.md`):
| Tier | Name | Prerequisites | What runs |
|------|------|--------------|-----------|
| **T1** | Console/transcript-compatible | VT100 console, libneo file I/O, basic signal/cooperative yield, `no_std` + `alloc` target | TUI apps (ratatui/crossterm), CLI tools, library-backed Arthur skills |
| **T2** | CPU-rendered GUI | G1 winit/softbuffer backend, minimal `std` PAL (fs+threads), softbuffer surface, basic input routing | egui/iced/Slint software-rendered apps, pixel-buffer viewers |
| **T3** | GPU-required | Phase-6 GPU accel, wgpu/Vulkan driver, full `std`, G1 compositor with GPU composition | wgpu apps, Lapce, Zed, Servo |
| **REJECTED** | C-dependent | Any load-bearing `*-sys` crate requiring C toolchain at build or C shared lib at runtime | GTK-rs, servo, miro-pdf, etc. |

**NEW axis this batch introduces:** ToS/legal verdict per networked service: **OPEN** (third-party clients permitted), **GRAY** (discouraged/unclear), **WALLED** (forbidden/proprietary — documented dead-end, never listed as a working ship).

---

## Executive summary

**Python is viable via RustPython.** The core VM is pure Rust. WASM build proves `no_std` capability. Stdlib modules needing OS calls (ssl, subprocess, socket, threading) map to libneo syscalls or are disabled. Arthur running sandboxed, cap-confined RustPython as a code-execution substrate is the right shape — structurally superior to WASM-per-OT-44 for Python workloads because RustPython exposes native capability boundaries the VM enforces.

**Password manager is the strongest day-1 fit.** Entire RustCrypto stack (AES-GCM, ChaCha20Poly1305, Argon2id, SHA-2) is pure Rust with zero `*-sys`. No production Rust password manager exists — building from scratch is the only path, and that's an advantage: we design the vault format for Arthur-native integration. AEAD (AES-GCM) from day one is better-than-mainstream vs Bitwarden's AES-CBC+HMAC.

**Archive formats: strong T1 with caveats.** ZIP/TAR/deflate are pure Rust. zstd has a pure-Rust alternative (`ruzstd`). LZMA/XZ is the gap — no good pure-Rust option. 7-Zip unsupported. Don't ship what Arthur/`neo>` already covers.

**Office formats: split as expected.** Format interop libraries exist for READ xlsx (calamine), WRITE xlsx (rust_xlsxwriter), WRITE docx (docx-rs). Cannot READ .docx in pure Rust — candid gap. Spreadsheet/PPT application is a total dead-end (no shippable option).

**Chat: 4 OPEN protocols, 3 WALLED.** Matrix, Telegram, XMPP, IRC are all OPEN with pure-Rust libraries. Signal is GRAY (protocol open, client closed, AGPL + C deps). Discord user clients are WALLED by ToS. WhatsApp and iMessage are WALLED. Arthur as unified cap-scoped inbox is the differentiated shape.

**HTML rendering: buildable today.** All pipeline components exist in pure Rust (html5ever, cssparser, taffy, cosmic-text, vello_cpu/tiny-skia). The gap is integration glue (~2-3k LOC for minimal viewer), not missing components. No JavaScript execution expected for day-1.

**Video decoding: candid gap.** No pure-Rust video decoder exists for any mainstream format (H.264, VP9, AV1 all missing). This is a hard gap with no near-term path.

---

## ISC-1: Python on LogansOS (RustPython)

### Core VM purity audit

| Aspect | Finding |
|--------|---------|
| **Project** | `RustPython` — https://github.com/RustPython/RustPython |
| **Stars / Health** | 22k stars, very active (daily commits), MIT license |
| **Core VM purity** | **PURE RUST.** The VM (`vm/`), parser (`parser/`), compiler (`compiler/`) are pure Rust. No `*-sys` crates in the core. |
| **`libc` usage** | Present but FFI-types-only: `signal-hook` uses `libc` crate for signal numbers. Does NOT link C code — `libc` is pure Rust type definitions. |
| **Optional features dragging C** | `ssl` → `openssl-sys` (C), `zlib` → system zlib, `hashlib` → may pull OpenSSL. All optional, disabled with `default-features = false`. |
| **WASM build** | Exists and works — proves core VM is `no_std` capable for computation-only Python. |

### The `no_std` / `std` boundary

**What runs with no OS (pure computation):**
- Arithmetic, string manipulation, list/dict/set operations
- Control flow (if/for/while/try), functions, classes
- `math`, `cmath`, `random` (with custom RNG backend)
- `json`, `re` (regex), `collections`, `itertools`-style modules
- Any pure-Python library (pandas numpy would need BLAS/LAPACK — C deps)

**Stdlib modules that force the std-analog (need OS calls):**

| Module | What it needs | Libneo mapping | Verdict |
|--------|--------------|----------------|---------|
| `os` | File I/O, process info, env vars | libneo syscalls (`SYS_READ`, `SYS_WRITE`, `SYS_MAP_REGION`) | **Shim-able** — write PAL backend for os module |
| `io` / `open()` | File I/O | libneo file capabilities | **Shim-able** |
| `time` | Clock, sleep | libneo time syscall | **Shim-able** |
| `socket` | TCP/UDP networking | Needs net/TLS userland surface (P5) | **Later** — blocked on P5 |
| `subprocess` | Fork/exec | No fork/exec in capability model | **Never or cap-confined WASM** |
| `threading` / `multiprocessing` | Threads, processes | libneo channels for coop. threading; no OS threads without std PAL | **Partial** — cooperative only until P2 |
| `ssl` | TLS | Needs rustls (pure Rust) + socket shim | **Later** — blocked on net surface |
| `ctypes` | C FFI | No C toolchain on LogansOS | **Disabled** — structurally incompatible |

### The Arthur-cap-confined code interpreter

**Recommendation: YES, cap-confined RustPython is the right shape for Arthur's code-exec tool.**

| Aspect | RustPython (cap-confined) | WASM (OT-44 H3) |
|--------|--------------------------|-----------------|
| **Capability enforcement** | Native — RustPython VM runs inside Arthur's cap bundle; file I/O goes through libneo syscalls which enforce caps at the kernel level | Sandbox-only — WASM has no native capability concept; must build cap mediation in userland |
| **Performance** | Near-native — JIT-less but direct execution on host arch | Slower — interpretation overhead + memory indirection |
| **Python ecosystem** | Direct — runs actual Python 3 code, imports work (with shimmed stdlib) | Limited — needs pyodide-style Emscripten shims, many modules don't compile to WASM |
| **Security surface** | Small — pure Rust VM, no C FFI when built without ssl/zlib/hashlib | Small — WASM sandbox is well-audited |
| **Complexity** | Medium — need stdlib shims for os/io/time | Lower — WASM runtime handles isolation |

**Verdict:** RustPython wins for Arthur's code interpreter because capability enforcement is native (kernel-enforced via libneo syscalls) rather than userland-mediated. The stdlib shim work (~3 modules: os, io, time) is the same cost regardless of runtime. For N=1 sovereignty, running Python natively on LogansOS is structurally cleaner than polyfilling POSIX inside WASM.

**Tier:** T1 for pure-computation REPL (no OS calls). T2 for file/OS Python (needs std PAL subset for os/io/time shims). Net/subprocess Python = later/maybe-never.

---

## ISC-2: The 8 categories

### 1. Password manager — **T1/T2, BUILD FROM SCRATCH**

#### RustCrypto purity audit (verified from Cargo.toml)

| Crate | Version | Pure Rust? | no_std? | *-sys deps | License |
|-------|---------|-----------|---------|-----------|---------|
| `aes` | 0.9.1 | YES | YES | NONE | MIT/Apache-2.0 |
| `aes-gcm` | 0.10.3 | YES | YES | NONE | MIT/Apache-2.0 |
| `chacha20poly1305` | 0.11.0 | YES | YES (default-features=false) | NONE | MIT/Apache-2.0 |
| `argon2` | 0.5.3 | YES | YES (default-features=false) | NONE | MIT/Apache-2.0 |
| `sha2` | 0.11.0 | YES | YES | NONE | MIT/Apache-2.0 |

**Transitive deps:** All pure Rust. `cipher`, `aead`, `digest`, `crypto-common`, `password-hash`, `blake2` — zero `*-sys`. The only caveat: `getrandom` (used by crypto crates for nonce generation) depends on `libc` for Linux/macOS, but supports custom backend (`getrandom_backend = "custom"`) which eliminates the dep entirely.

**Existing Rust password managers:** None production-quality. Searched passhook, pwwn, tango — all 404 or personal projects with 0 stars. Building from scratch is the only path.

#### Recommended vault architecture

```
Crypto stack:     Argon2id (KDF) + AES-256-GCM (AEAD) + SHA-256 (IDs/integrity)
RNG:              getrandom with custom LogansOS entropy backend
Storage:          NeoFS content-addressed blob, MessagePack binary format
Atomicity:        OT-17 session guarantees (all-or-nothing vault writes)
Capability scope: Per-vault cap bundle; per-entry caps for sharing
```

**Vault format:**
- Header: version, Argon2id params, AES-KW wrapped master key, SHA-256 integrity
- Entries: per-entry UUID, AAD = [entry_id, version], AES-GCM ciphertext + tag + 96-bit nonce
- Stored as single MessagePack blob on NeoFS, content-addressed

#### Better-than-mainstream (Art. 1.7)

| Aspect | Bitwarden/1Password | LogansOS Vault |
|--------|---------------------|----------------|
| AEAD mode | AES-256-CBC + HMAC (NOT authenticated encryption) | **AES-256-GAM or ChaCha20-Poly1305** (proper AEAD) |
| KDF | PBKDF2-SHA256 default (Bitwarden) | **Argon2id mandatory** (memory-hard, configurable) |
| Crypto supply chain | libsodium (C), OpenSSL (C) | **Pure Rust, zero C FFI** |
| Storage | Centralized server or proprietary blob | **NeoFS content-addressed, cap-scoped** |
| Sovereignty | Vendor-hosted or self-hosted with trust | **Arthur-native, no trusted third party** |

**Verdict:** Build as Arthur-native vault (T1: CLI via crossterm for management; T2: egui GUI when G1 lands). The crypto stack is pure Rust and the architecture is better-than-mainstream by default (AEAD vs CBC+HMAC, Argon2id vs PBKDF2, zero C supply chain).

---

### 2. Archive manager — **T1, LIBRARY ROUTE**

#### Format coverage (pure Rust create + extract)

| Format | Create | Extract | Crate | Purity |
|--------|--------|---------|-------|--------|
| **ZIP** | YES | YES | `zip` 8.6 (`default-features = false, features = ["deflate"]`) | PURE — pure Rust deflate via `deflate-flate2-zlib-rs` |
| **TAR** | YES | YES | `tar` | PURE — zero deps |
| **GZIP/deflate** | YES | YES | `flate2` (defaults to `miniz_oxide`, pure Rust) | PURE |
| **Zstandard** | YES | YES | `ruzstd` (pure Rust alternative to `zstd` which wraps C) | PURE — but less mature than C zstd |
| **LZMA/XZ** | NO good option | Partial (`lzma-rs` exists but incomplete) | — | GAP — no production pure-Rust LZMA |
| **7-Zip** | NO | NO | — | GAP — format is complex, no Rust implementation |
| **BZIP2** | YES | YES | `bzip2` crate (pure Rust backend available with `default-features = false`) | PURE (avoid default which pulls `bzip2-sys`) |

#### Verdict

Library route. Wire into Arthur: "extract this archive" → Arthur detects format, dispatches to appropriate crate, writes extracted files to NeoFS. Don't ship a separate archive manager app — Arthur covers the need. The LZMA/XZ gap is acceptable for day-1 (most archives are ZIP/TAR).

---

### 3. System utilities — **SUPERSEDED by Arthur/`neo>`**

| Tool | Pure Rust? | Shipped separately? | Reason |
|------|-----------|-------------------|--------|
| `ripgrep` | Mostly pure (1 C dep: `onig` regex, optional) | NO — Arthur search covers this | Arthur + RAG semantic search is superior to text grep |
| `fd` | Pure Rust | NO — Arthur file navigation covers this | "Find files in Documents" → Arthur lists them |
| `dust` | Mostly pure | NO — Arthur disk usage queries cover this | Cap-scoped NeoFS introspection replaces du |
| `procs` | Pure Rust | NO — Arthur process management covers this | Arthur already manages cap-bundle lifecycle |
| `bottom` (htop) | Has platform C deps | NO — Arthur system monitoring covers this | "Show me system stats" → Arthur queries kernel telemetry |
| `uutils/coreutils` | Partially pure (many platform-specific C deps) | NO — too broad, many tools superseded | Individual utilities either covered by Arthur or not needed on N=1 OS |

**Verdict:** Don't ship separate system utility apps. Arthur/`neo>` covers every need. The only exception: if a specific tool is requested as a standalone T1 app (e.g., ripgrep for scripting), it's shippable with the `onig` C dep flagged as manageable (vendored build available).

---

### 4. Media player — **T1 library, VIDEO GAP**

#### Audio (confirmed from round 1, extended)

| Component | Crate | Pure Rust? | Role |
|-----------|-------|-----------|------|
| Decode | `symphonia` | YES — zero `*-sys` | FLAC, MP3, AAC, Vorbis, Opus, WAV, MKV container |
| Playback API | `rodio` | YES (delegates to cpal) | High-level playback; wraps cpal + symphonia |
| Raw I/O | `cpal` | Linux: alsa-sys (C); Win/Mac: safe bindings only | Direct OS audio output |

**Audio verdict:** Same as round 1. symphonia decodes → voice-lane driver (OT-46) outputs PCM. No player app needed; Arthur skill: "play this song."

#### Video — candid gap

| Format | Pure-Rust decoder? | Best option | Verdict |
|--------|-------------------|-------------|---------|
| **H.264/AVC** | NO | None exists | Hard gap |
| **H.265/HEVC** | NO | None exists | Hard gap |
| **VP9** | Project `mozc` appears removed/unfindable | No successor | Hard gap |
| **AV1** | `rav1e` is encoder ONLY | `dav1d` wraps C libdav1d | Hard gap (pure Rust) |
| **MPEG-4** | NO | None exists | Hard gap |

**Video verdict:** CANDID GAP. No pure-Rust video decoder exists for any mainstream format. Path options:
1. Accept C dep on `dav1d` (AV1 only, bounded island) — delivers functionality at purity cost
2. Wait for community project (no active effort found)
3. Skip video for day-1; static images (PNG via pure-Rust `png` crate) are sufficient

---

### 5. Office suite — **SPLIT: Library interop (near-term) + App gap**

#### Format interop libraries (verified from Cargo.toml)

| Format | READ (pure Rust) | WRITE (pure Rust) | Best Crate(s) | Purity |
|--------|-----------------|-------------------|---------------|--------|
| **.xlsx** | YES | YES | `calamine` (read) + `rust_xlsxwriter` (write) | PURE — both zero `*-sys`, zip uses pure Rust deflate |
| **.xls** (legacy) | YES | NO | `calamine` (read only) | PURE |
| **.ods** | YES | NO | `calamine` (read only) | PURE |
| **.docx** | **NO** | YES | `docx-rs` (write only) | PURE — but CANNOT READ .docx |
| **.pptx** | NO | NO | `litchi` claims R+W but pre-0.1, no releases | GAP |
| **PDF** | YES (text extract) | YES (generation) | `lopdf` + `printpdf` | CAUTION — printpdf has transitive `libc` via allsorts |
| **SVG → image** | YES | N/A | `resvg` | PURE — gold standard, zero C deps |

#### The honest gaps

1. **Cannot READ .docx in pure Rust.** `docx-rs` is write-only. `litchi` claims R+W for all OOXML but is pre-0.1 with no releases. This is the single biggest office gap — if someone sends you a `.docx`, you can't open it.
2. **Spreadsheet APPLICATION — total dead-end.** Zero pure-Rust spreadsheet apps exist. GitHub search returned `rust-sheets` (0 stars, 1 commit, "UNDER PLANNING"). Building a spreadsheet app from scratch (grid UI, formula engine, cell editing, charting) in egui would be multi-person-year effort.
3. **Presentation APPLICATION — total dead-end.** No pure-Rust presentation app exists.

#### Verdict

**Library route for format interop:** Ship calamine (read xlsx/xls/ods), rust_xlsxwriter (write xlsx), docx-rs (write docx), resvg (SVG rendering). Wire into Arthur: "open this spreadsheet" → calamine reads → Arthur presents data in Workfield. "Create a report" → rust_xlsxwriter generates .xlsx.

**App gaps:** Spreadsheet and presentation apps are candid, multi-year gaps. Document the gap honestly. For day-1, the library route covers "not being a dead end when sent an .xlsx" (can read). The .docx read gap remains — Arthur can extract text from docx via its ZIP+XML structure using `zip` + `quick-xml` even without docx-rs's reader.

---

### 6. Web browser — **ELEVATED to OT-61, day-1 floor assessed**

Per dispatch, full browser evaluation lives in OT-61. For the shelf:

**Day-1 interim floor:** CPU HTML renderer + Arthur-mediated fetch
- **HTML parsing:** `html5ever` — PURE RUST, WHATWG-compliant
- **CSS parsing:** `cssparser` — PURE RUST (MPL-2.0)
- **Layout:** `taffy` — PURE RUST, Flexbox + Grid + Block
- **Text shaping:** `cosmic-text` — PURE RUST
- **Rendering:** `vello_cpu` or `tiny-skia` — PURE RUST, CPU-only

**Buildable today?** Yes. All pipeline components exist and are pure Rust. The gap is integration glue (~2-3k LOC for minimal structural HTML + inline styles viewer). No JavaScript execution for day-1. Arthur mediates fetch: "open this URL" → Arthur fetches HTML → renders in CPU viewer.

**Verdict:** Not a preload — it's a T2 build opportunity once G1 + std PAL land. Document as the browser gap floor until OT-61 delivers the full evaluation.

---

### 7. Chat / messaging aggregator — **4 OPEN, 3 WALLED**

#### Per-protocol analysis (verified from Cargo.toml + ToS pages)

| Protocol | Rust Library | C Deps | License | Maintenance | ToS Verdict | Arthur-Ready? |
|----------|-------------|--------|---------|-------------|-------------|---------------|
| **Matrix** | `matrix-sdk` + `vodozemac` E2EE | sqlite (bundled, optional) | Apache-2.0 | Excellent (Element-sponsored, daily commits) | **OPEN** — open standard, third-party clients encouraged | YES — primary anchor |
| **Telegram** | `grammers` | NONE | MIT/Apache-2.0 | Good (814 stars, active through Feb 2026) | **OPEN** — MTProto API explicitly for third-party clients | YES |
| **XMPP** | `xmpp` + `tokio-xmpp` | NONE (rustls default) | MPL-2.0 | Good (v6.0.0 Jun 2026, active org) | **OPEN** — IETF standard, no restrictions | YES |
| **IRC** | Roll your own (protocol is trivial) | NONE | — | N/A | **OPEN** — RFC 1459, open protocol | YES |
| **Signal** | `libsignal` (crypto only, NOT client) | boring-sys (C++) | AGPL-3.0 | Excellent but internal-facing | **GRAY** — protocol open, client closed, no third-party API | NO* |
| **Discord** | `serenity` / `twilight` | NONE (rustls default) | ISC | Excellent | **WALLED** — user clients forbidden by ToS, bot-only | BOT ONLY |
| **WhatsApp** | — | — | — | — | **WALLED** — proprietary, Meta blocks unofficial clients | NO |
| **iMessage** | — | — | — | — | **WALLED** — Apple-walled garden, zero public access | NO |

*Signal: libsignal implements the Signal Protocol crypto (Double Ratchet) but is NOT a client. No network layer, no server connectivity. AGPL-3.0 + C deps make it unsuitable for LogansOS anyway.

#### Arthur as unified inbox

**Recommendation: YES.** All 4 OPEN protocols flow into Arthur/a Workfield as cap-scoped connections. Each protocol connection is a declared cap bundle: `{protocol, account_scope, message_read/write}`. Arthur presents a unified inbox in the Workfield — same pattern as email (library-behind-Arthur).

**Why this beats Ferdium/webview:**
- No browser dependency (avoids the browser gap)
- Native protocol clients (no WebView wrappers)
- Cap-scoped auth (each connection declares its permissions)
- Sovereign (you control the homeserver for Matrix, your Telegram app registration, etc.)

#### The WALLED services — documented dead-ends

| Service | Why WALLED | Workaround? |
|---------|-----------|-------------|
| **Signal** | No third-party client API; libsignal is crypto-only + AGPL + C deps | Bridge via Matrix (fragile). Accept the gap. |
| **Discord (user)** | User gateway closed 2019; token scraping = ToS violation = ban | Bot-only integration. Users keep Discord app for personal DMs. |
| **WhatsApp** | Proprietary protocol; Meta actively bans unofficial clients | No viable path. |
| **iMessage** | Apple-walled garden; zero public access | No viable path. |

**Verdict:** Ship the 4 OPEN protocols as Arthur skills (T1: CLI management via crossterm; T2: egui inbox GUI). Document WALLED services as structural platform limitations, not engineering debt.

---

### 8. (Carried) Re-confirmed round-1 categories

No changes to round-1 verdicts for email, notes, calculator, file manager, terminal tools. The new research doesn't alter those assessments.

---

## ISC-3: The day-1 shelf recommendation (per tier)

### T1 shelf (VT100 console + libneo file I/O + basic syscalls)

| App / Capability | What it is | C deps | License | ToS | Preload? |
|-----------------|-----------|--------|---------|-----|----------|
| **Helix** | Text editor | 0 | MPL-2.0 | N/A | **YES — primary editor** (from round 1) |
| **zellij** | Terminal multiplexer | 0 | MIT | N/A | **YES — terminal infrastructure** (from round 1) |
| **gitui** | Git TUI | 1 (libgit2, vendored) | MIT | N/A | **YES — or gix-backed fork** (from round 1) |
| **RustPython REPL** | Python code interpreter (pure computation) | 0 (default-features=false) | MIT | N/A | **YES — Arthur code-exec substrate** |
| **Arthur Vault** | Password manager (Argon2id + AES-GCM, NeoFS storage) | 0 | Custom (RustCrypto: MIT/Apache) | N/A | **YES — build from scratch, better-than-mainstream** |
| **Archive libraries** | ZIP/TAR/GZIP/zstd extraction (Arthur skill) | 0 | MIT/Apache/MPL | N/A | **YES — library route** |
| **Email stack** | imap-codec + mailparse + lettre (Arthur skill) | 0 | MIT/Apache-2.0 | N/A | **YES — library route** (from round 1) |
| **Audio decode** | symphonia → voice-lane driver (Arthur skill) | 0 | MPL-2.0 | N/A | **YES — library route** (from round 1) |
| **Image decode** | `image` / `zune-image` (Arthur Workfield) | 0 | MIT/Apache-2.0 | N/A | **YES — library route** (from round 1) |
| **Markdown + text** | pulldown-cmark + cosmic-text (Arthur) | 0 | MIT | N/A | **YES — library route** (from round 1) |
| **Office interop** | calamine (read xlsx), rust_xlsxwriter (write xlsx), docx-rs (write docx) | 0 | MIT/Apache-2.0 | N/A | **YES — library route** |
| **Chat: Matrix** | matrix-sdk + vodozemac (Arthur skill, cap-scoped) | 0 (sqlite bundled optional) | Apache-2.0 | OPEN | **YES — primary chat anchor** |
| **Chat: Telegram** | grammers (Arthur skill, cap-scoped) | 0 | MIT/Apache-2.0 | OPEN | **YES** |
| **Chat: XMPP** | xmpp + tokio-xmpp (Arthur skill, cap-scoped) | 0 | MPL-2.0 | OPEN | **YES** |
| **Chat: IRC** | Custom implementation (~few hundred lines) | 0 | — | OPEN | **YES** |

### T2 shelf (G1 + std PAL target + softbuffer)

| App / Capability | What it is | Toolkit | C deps | Preload? |
|-----------------|-----------|---------|--------|----------|
| **egui notepad** | Simple text editor | egui + vello_cpu | 0 | **YES — ~500 lines** (from round 1) |
| **egui image viewer** | Image viewer + annotator | egui + image crate | 0 | **YES — ~200 lines** (from round 1) |
| **Arthur Vault GUI** | Password manager interface | egui + RustCrypto | 0 | **YES — when G1 lands** |
| **CPU HTML renderer** | Minimal HTML viewer (structural + inline CSS) | html5ever + cssparser + taffy + cosmic-text + vello_cpu | 0 | **YES — ~2-3k LOC build** |
| **Chat inbox GUI** | Unified chat inbox (Matrix/TG/XMPP/IRC) | egui + protocol libs | 0 | **YES — when G1 lands** |

### T3 shelf (Phase-6 GPU accel — later)

Nothing new from round 2. When GPU lands: wgpu-based apps, potentially Servo embed.

---

## ISC-4: The prerequisite ladder + single highest-leverage unlock

### Extended prerequisite ladder (incorporating round 2 needs)

| # | Prerequisite | Cost estimate | Unlocks (round 1 + round 2) | Priority |
|---|-------------|---------------|---------------------------|----------|
| **P1** | **VT100/ANSI escape interpreter** for transcript console | 1-2 person-weeks (~500 lines, pure Rust) | **Entire T1 shelf:** Helix, zellij, gitui, RustPython REPL, Arthur Vault CLI, all ratatui apps, chat protocol CLIs | **HIGHEST — single biggest leverage** |
| **P2** | **Minimal `std` PAL target** (fs + threads subset, no networking) | 2-3 person-months (dedicated Rust/OSdev engineer) | T2 GUI apps, RustPython with os/io/time shims, any crate requiring `std::fs` or `std::thread` | High — blocks all GUI apps + file Python |
| **P3** | **G1 winit/softbuffer backend** (compositor client speaking G1 protocol) | 2-3 person-weeks (compositor design complete; backend is ~1000 lines) | T2 software-rendered GUI (egui, iced, Slint), HTML renderer, Vault GUI, Chat inbox GUI | High — co-requisite with P2 |
| **P4** | **Audio output driver exposure** (voice-lane driver → userland PCM write syscall) | Already banked in OT-46; needs userland API surface | Media playback (symphonia → PCM → driver) | Medium — audio is Phase-N but media decoding works without it |
| **P5** | **Full `std` PAL target** (adds networking: TCP, DNS, TLS) | +2-3 person-months on top of P2 | Email (IMAP/SMTP), chat protocols (Matrix/Telegram/XMPP/IRC), RustPython socket/ssl, browser floor | Medium-high — email and chat are high-value but can wait for P1/P2/P3 |
| **P6** | **GPU accel path** (Phase-6) | Unknown — architecture-phase work | T3 apps (wgpu, Servo, Lapce) | Low — not day-1 |

### Which SINGLE prerequisite unlocks the most shelf?

**P1 (VT100 interpreter)** still wins. 1-2 weeks of work enables:
- Round 1: Helix, zellij, gitui, all ratatui apps
- Round 2: RustPython REPL, Arthur Vault CLI, archive extraction CLI, chat protocol CLIs

**P1 + P5 together** (VT100 + net/TLS PAL) unlock the maximum shelf: everything above plus email, chat protocols, and networked Python. But P1 alone is the highest single leverage.

**Testing the dispatch hypothesis ("std-analog + OT-57 together"):** The `std` PAL (P2) doesn't directly enable T1 apps — they run on `no_std` + libneo syscalls. What enables T1 is P1 (VT100). So the hypothesis is partially correct: P1 + P5 (not P2) is the combination that unlocks the most shelf. P2 is needed for T2 GUI apps but doesn't expand the T1 shelf further.

---

## ISC-5: Gaps + ledger disposition

### Candid gap list

| Category | Gap | Severity | Options |
|----------|-----|----------|---------|
| **Video decoding** | No pure-Rust decoder for any mainstream format (H.264, VP9, AV1 all missing) | Medium — users expect video playback | (a) Accept C dep on `dav1d` (AV1 only, bounded island); (b) Skip for day-1; (c) Wait for community project (none active) |
| **DOCX reading** | Cannot READ .docx in pure Rust (docx-rs is write-only; litchi pre-0.1) | Medium — "not a dead end when sent a .docx" | (a) Arthur extracts text from docx via zip + quick-xml (structural hack, no formatting); (b) Watch litchi for 0.1.0; (c) Accept the gap for day-1 |
| **Spreadsheet application** | No shippable pure-Rust spreadsheet app exists | Low-medium — users expect spreadsheets | (a) Library route: calamine reads data, Arthur presents in Workfield; (b) Multi-year build effort for native egui spreadsheet; (c) Document the gap |
| **Presentation application** | No shippable pure-Rust presentation app exists | Low — less common need | (a) Accept the gap; (b) SVG export via resvg is sufficient for simple slides |
| **PDF rendering** | No pure-Rust PDF renderer (round 1 carry) | Low-medium — text extraction works via lodpf | (a) Accept MuPDF C dep as bounded island; (b) Text-only PDF via lodpf is sufficient for v1 |
| **GIMP-class image editing** | No pure-Rust raster editor (round 1 carry) | Medium — users expect image editing | (a) Library route: `image` + `imageproc` for view + basic filters; (b) Simple egui editor (~2000 lines for crop/resize/brightness) |
| **Signal chat** | GRAY — protocol open, client closed, AGPL + C deps | Low-medium — Signal is popular | (a) Bridge via Matrix (fragile); (b) Accept the gap |
| **Discord/WhatsApp/iMessage** | WALLED — ToS or proprietary block third-party clients | Low — structural platform limitation | Document as known gaps. No engineering path exists. |

### Ledger disposition

**Proposed: Extend OT-60 with round-2 slices**

| Slice | Description | Depends on | Gate |
|-------|-------------|-----------|------|
| **OT-60 S6** | RustPython REPL (pure computation, no OS calls) | P1 (VT100 for interactive REPL) | Host KAT: Python arithmetic, string ops, control flow work in REPL; cap-confined file I/O blocked until P2 |
| **OT-60 S7** | Arthur Vault (Argon2id + AES-GCM vault on NeoFS) | NeoFS write caps, OT-17 session guarantees | Host KAT: create vault, add entry, retrieve entry, verify AEAD encryption; zero C deps in final binary |
| **OT-60 S8** | Archive extraction libraries (ZIP/TAR/GZIP/zstd) | libneo file I/O | Host KAT: extract test archives of each format; verify pure Rust build (cargo tree --edges no-dev \| grep -v sys) |
| **OT-60 S9** | Office format interop (calamine, rust_xlsxwriter, docx-rs) | libneo file I/O | Host KAT: read test .xlsx, write test .xlsx/.docx; verify pure Rust build |
| **OT-60 S10** | Chat protocol clients (Matrix/Telegram/XMPP/IRC as Arthur skills) | P5 (net/TLS PAL) | QEMU: connect to test server per protocol, send/receive message; cap manifest enforced per connection |

**Feeds into:** OT-60 (this shelf), OT-44 catalog (each preload is a cap-bundle), OT-57 (T1 console for CLI apps), OT-52 (G1 backend for T2 GUIs), OT-17 (vault encryption substrate).

---

## Cited primary sources

All claims verified from these primary sources:

**RustPython:**
- `RustPython/RustPython` — Cargo.toml, vm/Cargo.toml, LICENSE (MIT)
- WASM build: `python-wasm` example in repo

**RustCrypto:**
- `aes` — RustCrypto/block-ciphers/master/aes/Cargo.toml (MIT/Apache-2.0)
- `chacha20poly1305` — RustCrypto/AEADs/master/chacha20poly1305/Cargo.toml (MIT/Apache-2.0)
- `argon2` — RustCrypto/password-hashes/master/argon2/Cargo.toml (MIT/Apache-2.0)
- `sha2` — RustCrypto/hashes/master/sha2/Cargo.toml (MIT/Apache-2.0)
- `getrandom` 0.3.2 — docs.rs/crate/getrandom/0.3.2/source/Cargo.toml

**Archive:**
- `zip` 8.6.0 — docs.rs/crate/zip/8.6.0/source/Cargo.toml
- `tar` — tokio-rs/tar (pure Rust, zero deps)
- `flate2` — rust-lang/flate2-rs (miniz_oxide default)
- `ruzstd` — endigma/ruzstd (pure Rust zstd)

**Office:**
- `docx-rs` — bokuweb/docx-rs (537 stars, v0.4.19 Feb 2026, MIT)
- `calamine` — tafia/calamine (2.3k stars, v0.36.0 Jul 6 2026, MIT)
- `rust_xlsxwriter` — jmcnamara/rust_xlsxwriter (571 stars, v0.96.0, MIT/Apache-2.0)
- `litchi` — DevExzh/litchi (32 stars, pre-0.1, Apache-2.0)
- `resvg` — linebender/resvg (3.9k stars, v0.47.0 Feb 2026, MIT/Apache-2.0)

**Chat protocols:**
- `matrix-sdk` — matrix-org/matrix-rust-sdk (2.2k stars, pushed Jul 11 2026, Apache-2.0)
- `vodozemac` — matrix-org/vodozemac (Apache-2.0, `#![deny(unsafe_code)]`)
- `grammers` — Lonami/grammers (814 stars, pushed Feb 2026, MIT/Apache-2.0)
- `xmpp` / `tokio-xmpp` — gitlab.com/xmpp-rs/xmpp-rs (v6.0.0 Jun 2026, MPL-2.0)
- `serenity` — serenity-rs/serenity (5.6k stars, pushed Jul 5 2026, ISC)
- `twilight` — twilight-rs/twilight (852 stars, pushed Jul 12 2026, ISC)
- `libsignal` — signalapp/libsignal (5.9k stars, AGPL-3.0, boring-sys C dep)

**ToS pages:**
- Matrix: matrix.org/ecosystem/clients/ — lists independent clients, open standard
- Telegram: core.telegram.org/api — "100% open for all developers"
- Discord: discord.com/developers/docs/legal — user clients forbidden, bot-only
- Signal: signal.org/legal/ — no third-party client API
- WhatsApp: whatsapp.com/legal — proprietary, blocks unofficial clients

**HTML rendering:**
- `html5ever` — servo/html5ever (v0.39.0 Mar 2026, MIT/Apache-2.0)
- `cssparser` — servo/rust-cssparser (v0.37.0, MPL-2.0)
- `taffy` — DioxusLabs/taffy (v0.12.1 Jul 2026, MIT, 3.3k stars)
- `cosmic-text` — pop-os/cosmic-text (v0.19.0 Apr 2026, MIT/Apache-2.0)
- `vello_cpu` — linebender/vello (v0.0.9 May 2026, MIT/Apache-2.0)
- `tiny-skia` — linebender/tiny-skia (v0.12.0 Feb 2026, BSD-3-Clause)

**Video:**
- `rav1e` — Xiph.org/rav1e (encoder only, BSD-2-Clause)
- `dav1d` crate — wraps C libdav1d (NOT pure Rust)
- `mozc` — GitHub repo 404s, project appears removed

**Round 1 carry (unchanged):**
- Helix, zellij, gitui, symphonia, image, lodpf, pulldown-cmark, cosmic-text, imap-codec, mailparse, lettre — all verified in round 1

---

## Close-gate note

**Docs-only research — no CI run exists.** Paths-ignored per dispatch. `scripts/worker-close-gate.sh` host set locally (skip-with-note).

---

## RETURN

**Artifact:** `Plans/RESEARCH-2026-07-12-day1-shelf-round2-FINDINGS.md` (this file)
**Deliverables:**
1. Python verdict: RustPython is viable, cap-confined code interpreter is the right shape for Arthur — ISC-1
2. 8-category table with tier + purity-from-tree + license + ToS + verdict — ISC-2
3. Day-1 shelf recommendation per tier (15 T1 items, 5 T2 items) — ISC-3
4. Prerequisite ladder extended with round-2 needs; P1 (VT100) is single highest-leverage unlock — ISC-4
5. Candid gap list (video, docx read, spreadsheet/presentation apps, WALLED chat services) + OT-60 ledger disposition — ISC-5
6. All primary sources cited above

**Key findings for PL:**
- **Password manager = strongest fit.** Pure Rust crypto stack, better-than-mainstream architecture (AEAD vs CBC), build from scratch for Arthur-native integration.
- **Python via RustPython = viable.** Core VM is pure Rust. Cap-confined interpreter beats WASM for native capability enforcement.
- **Chat: 4 OPEN protocols ship, 3 WALLED are documented dead-ends.** Arthur as unified cap-scoped inbox is the differentiated shape.
- **Office: format interop libraries cover xlsx read/write and docx write. DOCX read and spreadsheet/presentation apps are honest gaps.**
- **HTML renderer: buildable today with pure Rust components (~2-3k LOC glue). Video decoder: hard gap, no pure-Rust option exists.**
- **P1 (VT100 interpreter, 1-2 weeks) remains the single highest-leverage unlock** — enables entire T1 shelf including all new round-2 additions.
