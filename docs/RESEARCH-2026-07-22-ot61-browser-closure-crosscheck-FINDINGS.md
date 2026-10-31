<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# OT-61 browser closure — cross-FAMILY check (Claude ← Codex) — FINDINGS

**Date:** 2026-07-22
**Reviewer:** Cerebro (Claude family; main-tier) — the cross-family lens on a Codex-authored direction
**Reviewed:** `docs/RESEARCH-2026-07-22-ot61-servo-host-closure-spike-FINDINGS.md` + `docs/RESEARCH-2026-07-22-ot61-swgl-browser-floor-spike-FINDINGS.md` (Janus, Codex), which converge with `docs/RESEARCH-2026-07-20-ot61-browser-crosscheck-FINDINGS.md` (Chad, Codex)
**Status:** cross-model design review — **NO-GATE** (verdict + divergences)
**Method note (why this is real cross-family evidence, not an echo):** the browser closure direction was, until this pass, validated **only by the Codex family** (Janus authored it; Chad's earlier REFUTE converges). Per CLAUDE.md *Cross-model diversity*, two same-family agents converging is **weak** evidence. My value-add is NOT re-verifying Janus's upstream line numbers (same work) — it is **(a)** independently re-deriving the load-bearing claims against **our own kernel/libneo source** (where all the containment actually lives), and **(b)** bringing the Claude-lens judgment on the architectural tensions. I re-read the real code for every LogansOS-side claim below; the upstream-shape claims I assess as architecturally sound and did **not** re-clone (Janus's git-worktree verification stands, and the load-bearing risk sits on *our* side of the seam anyway).

---

## 0. Verdict — **RATIFY-AFTER-AMENDMENT** [High confidence]

The direction is **sound and now independently corroborated against real LogansOS code** — this is genuine cross-family evidence, not Codex-echo. Servo/SWGL as a *bounded native OS port* (real `libstd` PAL + a private C++/libc++ subset + SpiderMonkey, forking only the capability-sensitive provider seams) with **SWGL rasterizing directly into a confined RW+NX `SharedPixelBuffer` and zero scanout/DMA/physical authority** is the correct shape. The `no_std`-strip framing is correctly REFUTEd; the "bounded native port, no POSIX personality to the ABI" containment is holdable; the C0-C3 ladder is honest *as a correctness floor*.

**But three amendments must be folded before C0 opens — and the first two are exactly the shared-blind-spot the cross-family lens exists to catch** (both Codex passes cover them incompletely, in the same direction). Lead-with-divergences:

- **A2 (the strongest catch): per-content-process MEMORY QUOTA, not just crash containment.** Both docs prove a content process *dies cleanly* (crash containment). Neither requires that a content process *cannot degrade the whole system before it dies* (resource containment). Web content is adversarial; without a per-process frame ceiling that trips **before** global `sys_map_region` exhaustion, a memory-bomb / decompression-bomb page starves every other process during the exhaustion window. "OOM kills only the content process" (Janus C3 gate) is **only true if the quota is the trip, not global exhaustion** — and no per-process quota exists today or in either ladder. This is the load-bearing containment gap.
- **A1: the W^X ⊗ JIT collision is a first-class GOVERNANCE-invariant decision, not a deferred perf/authorization footnote.** The ladder builds W^X (C1) as an invariant, runs no-JIT SpiderMonkey (C3) — honest for *correctness* — but treats JIT as "later, separately authorized." On LogansOS there is **no executable-writable-memory primitive at all** (user code is fixed at load; every data region is NX — I confirmed this in code). So adding JIT is not "relax W^X"; it is **introducing the first-ever runtime-generated-executable-code mechanism** — a security-load-bearing invariant change that gates the *usable* end-state. The no-JIT floor is honest, but the ladder hides that "usable modern browser" forces this decision. It belongs in the ladder **now** as a named, kill-switchable invariant-budget item (per complexity-governance Rule 2), not a stop-condition.
- **A5: a cheaper first de-risking step DOES exist** (both docs say "no prototype, blocked on C0-C2"). Two host-buildable-now spikes convert the biggest unknowns to checked facts without any kernel work — see §6. Janus's "a host demo wouldn't falsify the *target* risk" is true but too austere: de-risking is **separable**, not all-or-nothing.

Everything else RATIFIES, with the corroborations I independently checked listed in §7 (no rubber-stamp). None of the six load-bearing claims is REFUTEd; the amendments are pre-C0 scope, not wording.

---

## 1. Claim-by-claim

### Claim 1 — THE HEADLINE: SWGL rasterizes into a confined `SharedPixelBuffer` with NO scanout/DMA/physical path → **RATIFY** [High]

**Independently confirmed against our code (the containment side — where it matters):**
- `SharedPixelBuffer` client mapping is **RW+NX**, compositor mapping **RO+NX** — "W^X holds for both" (`kernel/src/atrium/buffer.rs:12-15`, read directly). SWGL writing virtual pixels into the client RW+NX mapping is exactly the confined case.
- `present()` refuses an unprivileged caller with **`EPERM` before any state change** — `if !scanout_authorized(caller_holds_gpu_submit) { return Err(EPERM) }`, ahead of the epoch/blit/flip (`buffer.rs:~205-214`). A browser client's direct-present attempt is structurally denied. Confirmed.
- HardwareFlip arms **only** a `FencedDmaAddr` via `fence_scanout(...)`, and buffer frames come from `DmaBuffer::alloc_in_arena` **only when scanout fencing is enabled** (`buffer.rs:88-111`, `~315-318`). SWGL never touches the DMA arena, `GpuSubmitCap`, MMIO, fence currency, or scanout — it only writes CPU pixels. Confirmed the *browser* never reaches that path.

**The SWGL side** (`swgl::Context: gleam::gl::Gl`, caller-owned `init_default_framebuffer`, `get_color_buffer(flush=true)`, `unimplemented!` on the non-WebRender GL subset) is architecturally consistent with SWGL's documented design as Mozilla's software WebRender backend, and with WebRender consuming `Rc<dyn gleam::gl::Gl>`. I did not re-clone at the pinned hash; the load-bearing risk is on our side and checks out.

**The `AtriumSwglRenderingContext` seam** genuinely lets a hardware provider swap in later behind the same `Rc<dyn gleam::Gl>` contract — the contract is stable. **Caveat (carried, not a divergence):** "hardware GL later replaces only the provider" is *contract*-true but must not read as *small* — the hardware provider **is** the OT-63/Vello-scale GPU program (a full WebRender GL profile: FBOs, shaders, texture arrays, sync). Chad's REFUTE stressed exactly this ("the largest schedule divergence"); the SWGL doc is a touch more optimistic than Chad's own earlier pass. Keep the SWGL floor honestly labeled "software page-paint correctness," and keep the hardware provider costed as a **separate off-critical-path GPU program** (dependency-depth: its own graphics-context/PPGTT/VT-d + typed-buffer/queue/fence stack), never folded into a paint rung.

**The `unimplemented!`-GL risk** is real but bounded (WebRender's shader/GL-call set is fixed and generated at build time — enumerable). Janus's mitigation (`panic_on_gl_error` + exercise every pinned shader) is sufficient **with one refinement (A6):** the coverage must be driven by the **actual Servo browser display-list corpus**, not just `wrench`'s fixtures — the browser can drive WebRender paths wrench never does, and that's exactly where a new call reaches an unimplemented entry on a refresh.

### Claim 2 — "bounded native port, no POSIX-personality-leak behind the bundle boundary" → **RATIFY** with amendment A4 [High]

The containment is holdable, for a reason Janus states correctly: the PAL implements Rust `std::sys::pal` **against libneo capabilities, not POSIX symbols** — so there is no ambient namespace to leak in the first place, and "unsupported ambient op fails closed" is structural. The OT-44 "hosting `std` grows a Linux-compat island" trap is real but is defended by the PAL terminating every authority-bearing op in an installed capability/preopen/provider.

**Divergence / strengthening (A4):** Janus's guard ("a new syscall-shaped compat entry needs its cap-provenance + a negative gate in the same change") must be a **build-breaking CI-enforced manifest diff**, not a review-time checklist — policy guards erode; a checked allowlist that fails the build on a new PAL symbol does not. Janus gestures at the machine-readable form (§5.6), so fold that as *the* mechanism, not an option. **And the manifest must cover the C++/libc++ CRT surface, not only the Rust PAL** — the C++ side (SpiderMonkey's libc subset) is the *higher-risk* leak vector: a single `#include` can pull a broader libc surface than intended, and it's where a POSIX-shaped island most easily creeps back in. Tighten the manifest hardest there.

### Claim 3 — the C0-C3 ladder is honest / is no-JIT SpiderMonkey a false floor? → **RATIFY as a correctness floor** with amendment A1 [High]

**Honest as a correctness floor.** A pure interpreter needs no executable-writable memory (bytecode is data; the SpiderMonkey C++ interpreter runs fine under total-NX). So C3 no-JIT **genuinely fits** LogansOS's W^X — it is not a false floor for *correctness* (does the browser compute the right DOM/pixels). No monster hidden *there*.

**The false-floor risk is the usable end-state, and it is the A1 governance collision.** A *usable* modern browser needs JIT; JIT needs runtime-generated executable code; LogansOS has **no such primitive** — I confirmed user memory is R+W+NX with no W→X transition (`sys_map_region` R+W+NX, `buffer.rs` NX mappings, `panic:abort` target). So JIT is not "flip a W^X knob," it is **the first-ever runtime-executable-code mechanism** — a new security invariant. Both Codex passes bury this in a stop-condition ("Stop if SpiderMonkey cannot run without executable writable memory"). **Amend the ladder to name it as a first-class rung/decision now:** the W^X⊗JIT resolution (interpreter-forever vs. a MAP_JIT-style W^X-preserving dual-map vs. an audited W→X region), its **kill-switch** (JIT-off ≡ interpreter, provably equal output), and its guard, are an invariant-budget item to scope before C0 — because the program can otherwise reach "paints correctly" (C5/S4) and only *then* discover the usable goal needs an invariant nobody scoped.

**Ordering refinement (A3):** the ladder's clean rungs understate the C1→C3 coupling. C1 freezes the thread/TLS/wait/VM/clock primitives for Rust `std`'s needs; C3's C++/SpiderMonkey then consumes the *same* primitives (Janus rightly says "one implementation defines the semantics"). The risk is a C3 surprise — a pthread/SpiderMonkey semantic the C1 primitive didn't anticipate (recursive-mutex, condvar `CLOCK_MONOTONIC` deadline distinct from Rust's, TLS destructor ordering, GC reserve-without-commit). **Co-design and validate C1's wait/TLS/VM/clock ABI against BOTH the Rust `std` contract AND the pthread/SpiderMonkey contract up front**, before freezing C1 — don't discover the C++ needs at C3.

### Claim 4 — the LogansOS gap table is correctly sized → **RATIFY (independently re-derived)** [High]

I checked each load-bearing row against the cited real code:
- **Heap/VM** — `sys_map_region` is a **monotonic per-process cursor** from `0x0000_4000_0000_0000`, **R+W+NX, zeroed, 16 MiB per-call ceiling** (`MAX_MAP_REGION_SIZE`), **no unmap/protect/reserve**, "regions persist for the process's lifetime" (`libneo/src/lib.rs:1768-1900`, read directly). The heap already tolerates the 16 MiB *call* ceiling by splitting into contiguous maps (Argon2id's 64 MiB matrix), so that ceiling alone is minor. The **deep** gap — correctly graded "high, before SpiderMonkey" — is the absence of a **real region tracker** (unmap/protect/decommit/reserve + guard pages); the heap's own comment corroborates Janus verbatim: *"Phase 5+ … would break this [contiguous] assumption; the heap will need a separate cursor or a real region tracker."* SpiderMonkey's GC (reserve-large / commit-lazy / decommit / mprotect) cannot be faithfully hosted without it. This is a **deep MM subsystem build (many-cycle)**, not a medium adapter — Janus's "high" grade is right; if anything the *dependency depth* (new VMA/region layer where maps are currently immediate-backed) deserves emphasis as the ladder's heaviest kernel rung.
- **Single-threaded userspace / no user-thread ABI** — confirmed: a `Process` is "a Pmap + Thread" (`kernel/src/sched/process.rs:4,293-307`); the heap notes "Phase 1.5b is single-threaded user." The user-thread lifecycle ABI (create/join/detach + stack/TLS bootstrap) is genuinely absent — "Required/high" is accurate.
- **Panic/unwind** — target is `os:none`, `panic-strategy:abort`, `static` (`userland/x86_64-neoplutarch-user.json`, read directly). "Abort across the C++ boundary, don't unwind Rust through C++" is the right initial policy and matches the target. **One honest sub-note:** with `panic=abort`, Servo's few `catch_unwind` recovery sites (SVG decode, script bindings) become **process-fatal** — a malformed SVG aborts the tab instead of recovering locally. That's *contained* (only the tab dies) but is a robustness regression vs. upstream, and patching those sites to explicit-error is real (small) fork surface that recurs each Servo refresh. Janus flags the patch; just log it as a standing per-refresh cost, not a one-time fix.

Nothing Janus grades "Required/high" is actually already present; nothing under-rated is secretly a deep kernel change beyond what he flags — **except the resource-quota gap in Claim 5**, which the table omits entirely.

### Claim 5 — per-content-process crash containment + no-dial-cap confinement → **RATIFY the crash half; AMEND with A2 for the resource half** [High]

**Crash containment (process dies cleanly) is LARGELY PRESENT — a new *assembly*, not a new deep kernel invariant.** Confirmed in `kernel/src/sched/process.rs`: each process **owns its address space** (separate `Arc<Pmap>` / CR3, `:293-307`), so a content-process memory fault cannot corrupt a sibling; a one-shot `exit_owner` CAS unifies natural-exit and forced-kill; a reaper zombifies → transfers exit codes → deregisters → **drops the Pmap (frees frames)**; `sys_kill`/`sys_wait`/`sys_inspect` give the parent kill+notify+introspect; a Blocked-in-IPC target is **extracted from its channel and zombified** so peers don't hang (`on_thread_reap`). The browser-specific work is **constellation-side reap/notify wiring + a negative gate** (crash a content process; assert compositor + siblings survive; assert the `SharedPixelBuffer` `Arc`-pinned lifetime holds — a dead client's mapping-`Pmap` Drop removes only its borrowed PTE, the compositor keeps its own `Arc` clone, frames free only when all mappings are gone, per `buffer.rs:5-16`). So Janus's "process / Good fit / medium" is **accurate for crash containment.** No new kernel invariant needed there.

**Divergence (A2) — the RESOURCE half is missing from both Codex passes.** Crash containment ≠ resource containment. On LogansOS a content process allocates from the **global** buddy allocator via `sys_map_region` with **no per-process frame ceiling** (verified: the syscall enforces only the 16 MiB *per-call* cap, not a per-process total). So a decompression-bomb image, a giant DOM, or an unbounded native allocation drives **global** frame exhaustion: `sys_map_region` returns `-ENOMEM` → Rust `handle_alloc_error` → abort (contained *eventually*), **but every other process's concurrent allocation fails during the exhaustion window** — a system-wide DoS caused by one adversarial page. Janus's C3 gate "OOM/crash kills only the content process" is therefore **not deliverable as written** without a **per-content-process memory quota** (kernel per-process frame accounting + a ceiling that trips *before* global exhaustion, turning a memory bomb into a local `-ENOMEM`/abort). SpiderMonkey's own GC `maxBytes` caps the *JS heap*, but the Rust/native side (images, DOM, decode buffers) is uncapped. **Fold a per-process memory quota into C2/C3 as the resource-containment guard** — it is the half that makes "kills only that process" true for adversarial content, and it is a real (bounded) kernel change neither doc scoped.

**No-dial-cap confinement — RATIFY (converges with Chad, independently plausible against code).** `NetDialCap` carries immutable `(host,port)` scope and derivation only attenuates (`libneo/src/lib.rs:1337-1405`, `kernel/src/sched/net_dial_cap_logic.rs`); the structural rule "renderer holds **no** dial cap, the network **broker** holds attenuated caps and re-authorizes every request/redirect" is correct and matches the earlier record. **Carried corroboration (both families, keep it stated):** compositor RO protects *integrity, not confidentiality* — a compromised compositor can read the pixels it maps; the browser threat model must not claim structural confidentiality from the compositor (Chad §5.2). The SWGL doc does not regress this; keep it explicit in the browser security ADR.

### Claim 6 — is there a cheaper first de-risking step than the full C0-C2 precursor? → **YES (divergence)** [Medium-High]

Both docs say "no prototype, blocked on C0-C2." That is right about the *end-to-end target smoke*, but **de-risking is separable** and two spikes are **host-buildable now**, needing zero kernel work, each converting the biggest unknown into a checked fact — see §6.

---

## 2. The amendments to fold before C0 opens

| # | Amendment | Why | Where it lands |
|---|---|---|---|
| **A1** | Name the **W^X ⊗ JIT** resolution as a first-class, kill-switchable **invariant-budget** decision in the ladder (not a stop-condition). | LogansOS has no runtime-executable-code primitive; JIT introduces the first one — a security invariant gating the *usable* end-state. | ladder pre-C0 decision; C3+ rung |
| **A2** | **Per-content-process memory quota** (frame ceiling tripping before global exhaustion). | "OOM kills only that process" is false for adversarial content without it — a memory bomb DoSes the system first. | gap table (new row) + C2/C3 gate |
| **A3** | Co-design & validate the **C1 primitives against BOTH the Rust `std` and pthread/SpiderMonkey contracts** before freezing C1. | avoid a C3 surprise where the C++ runtime needs a wait/TLS/VM/clock semantic C1 didn't anticipate. | C1 design |
| **A4** | Make the PAL surface allowlist a **build-breaking CI manifest diff**, and extend it to the **C++/libc++ CRT surface** (the higher-risk POSIX-leak vector). | policy guards erode; the C++ side is where a compat island creeps back. | C0/C3 gate |
| **A5** | Run the two **cheaper de-risking spikes now** (§6). | convert C0's unknown PAL surface + the SWGL↔Atrium byte seam into checked facts with no kernel work. | before C0 |
| **A6** | Drive the `unimplemented!`-GL / `panic_on_gl_error` coverage from the **actual Servo browser display-list corpus**, not just `wrench`. | the browser exercises WebRender paths wrench doesn't — that's where a refresh hits an unimplemented entry. | S2 gate |

---

## 3. The cheaper first step (A5, expanded)

**Spike-α — `std`-PAL symbol-surface enumeration [highest leverage; host-buildable now, no kernel work].** Add the first-class `x86_64-unknown-neoplutarch` target JSON (distinct `target_os`), stub `library/std/src/sys/pal/neoplutarch` with every fn as `unimplemented!()`, `-Z build-std=std,panic_abort` a trivial `std`-using binary, and read the **link errors / required-symbol list**. This turns C0's "PAL skeleton (unknown surface)" into a **finite, checked symbol manifest** — the single biggest unknown-reducer on the whole program, and it needs *only* host cargo + a toolchain patch. Janus reuses Hermit/WASI semantics but does not propose *enumerate-the-surface-first* as the cheap opening move; it should be the first thing built, because it makes the entire C1 scope concrete before any kernel primitive is written.

**Spike-β — the SWGL↔Atrium byte-contract oracle [host-buildable now].** On a host build, rasterize an **asymmetric R≠B** pattern through SWGL into a **padded-stride** buffer shaped exactly like a `SharedPixelBuffer` (XRGB8888_LE = bytes `B,G,R,X`, `kernel/src/atrium/logic.rs:14-17`), then compare against a **hand-laid byte oracle**. This falsifies the §4.1 channel-order + stride-alignment claim **today**, independent of the target PAL, and produces the reusable S1 fixture. Janus dismissed a host SWGL demo as "wouldn't falsify the *target* risk" — true for the PAL, but this de-risks the *format seam*, a separable real unknown, and the byte oracle is a required S1 deliverable regardless (our byte-oracle standing rule: the exact bytes come from the hand-laid fixture, never a summarizing fetch).

Neither spike needs the kernel; both are pure wins that shrink the C0-C2 uncertainty before a seat commits to the deep MM/thread/CRT work.

---

## 4. Scope, in the units that are real (no calendar)

Per the no-human-labor-time rule, I restate the shape as dependency-depth / iteration-cycles / gates, and **do not pass through** the "18-24 month / 6-9 month" figures the earlier Codex pass carried:

- **Critical path (dependency depth):** `std`-PAL symbol surface (Spike-α) → C0 target+PAL skeleton → **C1 threads/TLS/waits/VM-region-tracker/clocks** (the heaviest rung — a new kernel MM/threading layer, many-cycle) → C2 sealed handles + **no-dial + memory-quota (A2)** → C3 SpiderMonkey no-JIT + crash/resource containment → C4/S4 Servo layout closure → S3 Atrium pixel handoff. Each rung has a go/no-go discriminator; **C1 is the depth bottleneck** (region tracker + user-thread ABI + wait/TLS ABI, co-designed for both `std` and pthread per A3).
- **Off the critical path (separate program, not a paint rung):** the **hardware-GL provider** behind the stable `gleam` seam — an OT-63/Vello-scale GPU stack (graphics context/PPGTT/VT-d + typed buffers/queues/fences + the full WebRender GL profile). The SWGL software floor deliberately does **not** need it; keep it costed separately (Chad's largest divergence — I concur).
- **The governance rung (A1):** the W^X⊗JIT decision sits after correctness closure but must be *scoped* now — it is the difference between "software-correct browser" and "usable browser," and it is an invariant change, not a perf pass.
- **Uncertainty tier:** C1 (deep, genuinely-ours seams) and C3 (SpiderMonkey port) are high-uncertainty/many-cycle; S1-S2 (SWGL raster + WebRender fixtures) are lower-uncertainty with crisp byte oracles. Spike-α + Spike-β retire the most uncertainty per unit of work.

---

## 5. Where the two Codex passes and I DIVERGE vs. CORROBORATE

**Divergences (the shared-blind-spot catches — the point of the cross-family lens):**
1. **Resource containment ≠ crash containment (A2).** Both cover the process dying cleanly; neither requires it can't DoS the system first. For adversarial web content this is load-bearing. *[Highest-value catch.]*
2. **W^X⊗JIT is a governance-invariant collision, not a deferred footnote (A1).** Both treat JIT as later/perf; on our total-NX target it is the first runtime-executable-code primitive and gates the usable end-state.
3. **A cheaper first step exists (A5).** Both say "blocked on C0-C2"; the `std`-PAL surface enumeration + the SWGL↔Atrium byte oracle are host-buildable now. Janus's "a host demo wouldn't falsify the target risk" is too austere — de-risking is separable.
4. **Minor:** the SWGL doc is slightly more optimistic than Chad's own earlier pass about "hardware GL later drops in behind the provider" — keep the hardware provider costed as the separate OT-63/Vello-scale program Chad named.

**Corroborations I independently re-derived against real code (no rubber-stamp):**
- SWGL-into-confined-`SharedPixelBuffer` containment: RW+NX/RO+NX, `scanout_authorized`→`EPERM` before any state change, `FencedDmaAddr`-only flip arming, arena-backed only when fencing on — all TRUE (`buffer.rs`). *[Claim 1]*
- The gap table's heap/VM row (16 MiB call ceiling, no unmap/protect/reserve, contiguous monotonic cursor, needs a real region tracker for SpiderMonkey) and single-threaded userspace — TRUE, the heap's own comment matches (`libneo/src/lib.rs:1768-1900`). *[Claim 4]*
- Process model: separate `Arc<Pmap>`/CR3, reap→drop-Pmap, `sys_kill`/`wait`/`inspect`, IPC-endpoint cleanup on death; `panic:abort` target — TRUE (`process.rs`, target JSON). Crash containment is an assembly of existing primitives, not a new invariant. *[Claim 5]*
- No-dial-cap / broker structure and "compositor RO = integrity not confidentiality" — consistent with `NetDialCap` code and carried from Chad. *[Claim 5]*
- The `no_std`-strip REFUTE (Servo layout directly depends on script/fonts/net/paint/Stylo/Rayon/WebRender) — corroborated; the retained engine is intrinsically hosted+concurrent. *[Claim 2/3]*

---

## 6. Bottom line

The browser closure direction is **RATIFY-AFTER-AMENDMENT**, and — having re-derived the containment, gap-table, and process-model claims against our own source rather than trusting the Codex convergence — the corroboration is now **real cross-family evidence**. The SWGL-into-a-confined-`SharedPixelBuffer` floor is unusually well matched to Atrium and structurally can't reach scanout; the bounded-native-port host closure is the right shape and the `no_std` strip is correctly dead. Fold the three lead amendments before C0: **the per-process memory quota (A2)** so adversarial content can't DoS the system, **the W^X⊗JIT governance rung (A1)** so the usable end-state's invariant cost is scoped up front, and **the two cheaper de-risking spikes (A5)** so C0-C2's uncertainty is retired before a seat commits to the deep kernel work — plus the build-breaking PAL manifest (A4), the C1-co-design (A3), and the corpus-driven GL gate (A6). With those, C0 is safe to open.

---

## 7. Method + confidence

- **RAG** healthy (`ollama=true, qdrant=true`, 2026-07-22) — used for discovery only; every load-bearing LogansOS claim was then read directly from source.
- **Independently read this pass:** `kernel/src/atrium/buffer.rs` (present seam, W^X mappings, `scanout_authorized`, arena/`FencedDmaAddr` flip), `userland/libneo/src/lib.rs` (heap/`sys_map_region`, `sys_spawn`/`kill`/`wait`/`inspect`, `NetDialCap`), `kernel/src/sched/process.rs` (per-process `Pmap`, reap/exit, thread model), `userland/x86_64-neoplutarch-user.json` (target/panic). Upstream Servo/SWGL/gleam/SpiderMonkey at the pinned hashes: **not re-cloned** — Janus's git-worktree verification stands and the load-bearing risk is on our side of the seam.
- **Confidence:** verdict + the three lead amendments **[High]** (A2/A1 grounded directly in our code's absence of a per-process quota and any executable-memory primitive; A5 is constructive). The upstream-shape claims **[Medium-High]** (assessed architecturally, not re-cloned). Per-slice depth **[not quoted as calendar]** — expressed as dependency-depth/cycles/gates in §4.
