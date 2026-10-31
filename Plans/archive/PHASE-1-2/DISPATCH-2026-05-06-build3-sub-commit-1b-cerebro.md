# Dispatch: BUILD-3 sub-commit 1b — AP bring-up via Limine MP request

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-06
**Parent commit:** `305f4d9` (sub-commit 1a, currently HEAD on `main`)
**ADR substrate:** ADR-013 §Q1, §Q2, Amendment 1, Amendment 2
**Spec substrate:** Bible Article 8.3 (per-CPU state real, no global locks in hot paths)
**Estimated surface:** ~150–250 lines net across 2–4 files. Smaller diff than 1a.

---

## 0. Standing instructions (read in full before starting)

These are the discipline rules you and PM agreed during sub-commit 1a. They are not optional.

1. **Commit on `main`, not detached HEAD.** Run `git status` and `git branch --show-current` before your first commit. If anything looks off, stop and surface it.

2. **Verify boot-order claims via direct file read** before relying on them. The 1a dispatch was wrong about the `mm::init` / `init_traps` ordering; you caught it correctly. Do that again here if anything in this dispatch contradicts what you see in `kernel/src/main.rs`.

3. **Flag deviations explicitly** in a docs/ADR amendment subsection if you have to deviate from this dispatch. The 1a Amendment 1 "Implementation correction" subsection is the pattern: name the dispatch's incorrect assumption, name what was actually true, give forward guidance for future BUILDs.

4. **Run verification before reporting it.** Do not infer test results from code inspection. Boot the kernel; capture serial output; quote literal lines.

5. **Diff warnings against baseline.** The HEAD baseline is **18 warnings at `305f4d9`**, PM-verified by clean-tree `cargo k`. Any new warning needs a one-line explanation; any retired warning needs a one-line confirmation it was actually removed (not suppressed by mistake).

6. **Pre-implementation OBSERVE pass.** Read every file you'll touch before writing the first line of code. The relevant surface for 1b is small: `kernel/src/main.rs` (boot-time SMP request iteration), `kernel/src/arch/x86_64/per_cpu.rs` (the `init_ap` entry point you will call), `kernel/src/arch/x86_64/mod.rs` (the `init_traps_ap` function you will call), and `tools/xtask/src/main.rs` (where `-smp N` will be added to the QEMU args).

---

## 1. What 1b adds

**Goal milestone:** Boot QEMU with `-smp 4`. Each AP reaches an `ap_entry` spin-wait. Each AP's serial output emits one line that proves its GS-base is wired to its own `PER_CPU[N]` slot. Single-CPU boot (`-smp 1`) still works identically to 1a. Full default test suite PASSes under both `-smp 1` and `-smp 4`.

**APs do not enter the scheduler.** They do not run threads. They do not touch IPC. They do not handle TLB shootdowns. They wire their per-CPU state, sentinel-verify it, then `cli; hlt; jmp` forever. The whole point of 1b is to verify all of:

- Limine's MP bring-up actually works in our boot path.
- AP firmware lands in our `ap_entry` extern "C" function correctly.
- GS-base wiring on APs is correct (the single most catastrophic thing to get wrong on x86_64 SMP).
- Per-AP GDT/TSS/IST stacks (statically allocated in 1a) are loaded correctly when each AP runs `gdt::init_ap(cpu_id)`.
- Per-AP MSRs (EFER, STAR, LSTAR, SFMASK) are written correctly via `syscall::init_ap()`.
- The shared IDT loads correctly on each AP via `idt::load_for_ap()`.
- Per-AP slab state initializes correctly via `mm::slab::init_per_cpu(cpu_id)`.

If any of those is wrong, 1b's verification gate fails. Sub-commit 2 (per-CPU schedulers, the actually-substantial SMP work) builds on a known-good 1b foundation.

---

## 2. Limine MP API — what you have to work with

Verified 2026-05-06 by direct read of `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/limine-0.6.3/src/{request.rs,mp/x86_64.rs,mp/mod.rs}`. Project depends on `limine = "0.6"` (Cargo.toml line); resolved version is `limine-0.6.3`.

**Request type:**
```rust
pub type MpRequest = Request<MpRespData, u64>;

impl MpRequest {
    pub const fn new(flags: u64) -> Self { /* ... */ }
}
```

`flags` is `0` for default behavior, or `MP_FLAG_X2APIC` (= `1 << 0`) to request x2APIC mode if supported. **Use `flags = 0` for 1b.** ADR-013's anti-scope explicitly defers x2APIC.

**Response data (x86_64-specific, from `mp/x86_64.rs`):**
```rust
#[repr(C)]
pub struct MpInfo {
    pub processor_id: u32,
    pub lapic_id: u32,
    _resvd0: u64,
    goto_addr: AtomicPtr<()>,
    extra_argument: AtomicU64,
}

#[repr(C)]
pub struct MpRespData {
    pub flags: u32,
    pub bsp_lapic_id: u32,
    cpu_count: u64,
    cpus: *const (),
}
```

**Methods on `MpInfo` (from `mp/mod.rs`):**
```rust
pub type MpGotoFunction = unsafe extern "C" fn(&MpInfo) -> !;

impl MpInfo {
    pub fn bootstrap(&self, address: MpGotoFunction, extra_arg: u64);
    pub fn extra_argument(&self) -> u64;
}

impl MpRespData {
    pub const fn cpus(&self) -> &[&MpInfo];
}
```

**Important consequence:** the AP entry function's signature is `unsafe extern "C" fn(&MpInfo) -> !`. The AP is started by writing `goto_addr` via `info.bootstrap(ap_entry, extra_arg)` where `extra_arg` is a `u64` of your choosing. The AP reads it back via `info.extra_argument()`.

**This means you do NOT need a `cpu_id_from_apic_id()` function** that the 1a handoff §5 mentioned. The BSP, while iterating `cpus()`, knows the index `i` and can pass it as `extra_arg`:

```rust
for (i, cpu) in cpus.iter().enumerate() {
    if cpu.lapic_id == bsp_lapic_id { continue; }    // skip BSP
    cpu.bootstrap(ap_entry, i as u64);
}
```

The AP reads its `cpu_id` directly:

```rust
unsafe extern "C" fn ap_entry(info: &limine::mp::MpInfo) -> ! {
    let cpu_id = info.extra_argument() as u32;
    // ... wire per-CPU state for this cpu_id ...
}
```

This is simpler, more robust, and avoids the LAPIC-ID-to-cpu-id mapping table entirely. The handoff specified the wrong approach; this is the correction.

The Limine spec's relevant detail: "Note: The presence of this request will prompt the bootloader to bootstrap the secondary processors. This will not be done if this request is not present."

---

## 3. Implementation plan (suggested, not normative)

The shape that follows is what PM expects. If you find a better factoring during OBSERVE pass, deviate and document.

### 3.1 New file: `kernel/src/arch/x86_64/smp.rs`

A new module is preferred over piling onto `per_cpu.rs`. The module contains the AP entry path and any AP-startup-only helpers. `mod.rs` adds `pub mod smp;`.

Module skeleton:

```rust
//! SMP application-processor bring-up via the Limine MP protocol.
//!
//! Sub-commit 1b lands this module. APs are brought from firmware-parked state
//! to a spin-wait at the end of `ap_entry`. APs do NOT enter the scheduler;
//! that is sub-commit 2's territory.
//!
//! Per ADR-013 §Q2 + Amendment 2.

use limine::mp::{MpInfo, MpRespData};
use limine::request::MpRequest;

use crate::arch::halt_forever;
use crate::arch::x86_64::{self, per_cpu};
use crate::mm;
use crate::serial_println;

/// Limine MP request. `flags = 0` — no x2APIC (anti-scope per ADR-013).
#[used]
#[unsafe(link_section = ".requests")]
pub static MP_REQUEST: MpRequest = MpRequest::new(0);

/// Bring up secondary processors. Called by the BSP from `_kernel_start`
/// AFTER `mm::init()` has completed (so AP heap allocations are legal in
/// principle — see anti-scope below for what 1b does NOT use this for).
pub fn bring_up_aps() {
    let response = match MP_REQUEST.response() {
        Some(r) => r,
        None => {
            serial_println!("[ kernel ] smp: no MP response from Limine — single-CPU boot");
            return;
        }
    };

    let bsp_lapic_id = response.bsp_lapic_id;
    let cpus = response.cpus();
    serial_println!(
        "[ kernel ] smp: Limine reports {} cpu(s); bsp_lapic_id={}",
        cpus.len(),
        bsp_lapic_id
    );

    let max = per_cpu::MAX_CPUS;
    let mut started = 0u32;
    for (i, cpu) in cpus.iter().enumerate() {
        if cpu.lapic_id == bsp_lapic_id {
            continue; // BSP is already running this code path
        }
        if i >= max {
            serial_println!(
                "[ kernel ] smp: skipping cpu_index={} lapic_id={} — exceeds MAX_CPUS={}",
                i, cpu.lapic_id, max
            );
            continue;
        }
        serial_println!(
            "[ kernel ] smp: starting cpu_index={} lapic_id={}",
            i, cpu.lapic_id
        );
        cpu.bootstrap(ap_entry, i as u64);
        started += 1;
    }
    serial_println!("[ kernel ] smp: started {} AP(s)", started);

    // The BSP does not wait for APs to finish init in 1b. APs spin-wait;
    // sub-commit 2 will introduce a barrier here when scheduler entry is wired.
}

/// AP entry point. Called by Limine on each application processor after the
/// BSP writes `goto_addr`. The AP arrives with paging on, GDT loaded (Limine's
/// minimal one), interrupts disabled, on a 64 KiB stack provided by Limine.
///
/// This function never returns. APs spin-wait at the end pending sub-commit 2.
unsafe extern "C" fn ap_entry(info: &MpInfo) -> ! {
    // Limine has reserved a per-AP stack. We're ring-0, paging on, IF=0.
    // Critical: this function MUST NOT return — the goto_addr contract.
    // No use of `?`-returning helpers; no early-return paths.

    let cpu_id = info.extra_argument() as u32;

    // Step 1: Wire GS-base for this AP. Writes IA32_GS_BASE = &PER_CPU[cpu_id]
    // and IA32_KERNEL_GS_BASE = 0. After this, gs:[N] is meaningful.
    per_cpu::init_ap(cpu_id);

    // Step 2: Sentinel-verify. PER_CPU[cpu_id].cpu_id was set to `cpu_id` by
    // init_ap; current_cpu_id() reads gs:[24]. They MUST match. If they
    // don't, the GS-base wire is wrong and we panic loud and early — the
    // alternative is silent corruption of every per-CPU access for the
    // rest of the boot.
    let observed = per_cpu::current_cpu_id();
    if observed != cpu_id {
        // Cannot use panic! safely yet (no per-CPU IDT loaded, no per-CPU
        // TSS for IST). Print and halt directly.
        serial_println!(
            "[ kernel ] smp: AP cpu_id={} sentinel MISMATCH (gs:[24] reads {}) — halting",
            cpu_id, observed
        );
        halt_forever();
    }

    // Step 3: Per-AP arch init. Loads this AP's GDT (with its TSS descriptor),
    // loads the shared IDT, writes per-AP SYSCALL/SYSRET MSRs (EFER, STAR,
    // LSTAR, SFMASK).
    x86_64::init_traps_ap(cpu_id);

    // Step 4: Per-AP slab state. Allocates a SlabPerCpuState on the heap and
    // wires `PER_CPU[cpu_id].slab_state` to it. Heap is operational because
    // mm::init() ran on the BSP before bring_up_aps() was called.
    mm::slab::init_per_cpu(cpu_id);

    serial_println!(
        "[ kernel ] smp: AP cpu_id={} ready (lapic_id={}, processor_id={})",
        cpu_id, info.lapic_id, info.processor_id
    );

    // Step 5: spin-wait forever. Sub-commit 2 replaces this with
    // sched::run_ap_bootstrap(cpu_id) per ADR-013 §Q3.
    halt_forever();
}
```

### 3.2 Wire from `kernel/src/main.rs`

After `mm::init(...)` and before `sched::init()`, call `arch::x86_64::smp::bring_up_aps()`. The exact call site is between line 288 (current `mm::init`) and line 293 (current `sched::init`), but verify line numbers against current `main.rs` before quoting them in your close report.

The new call sequence in `_kernel_start`:

```rust
mm::init(hhdm_offset, memmap);
arch::x86_64::smp::bring_up_aps();   // ← NEW
sched::init();
sched::smoke_test();
```

**Why before `sched::init`:** keeps APs from running scheduler code (which the BSP hasn't initialized yet for the per-CPU shape). They'll spin-wait at the end of `ap_entry` regardless, but bringing them up before scheduler init means there's no window where APs see a half-initialized scheduler.

**Why after `mm::init`:** AP slab init needs `mm::with_frame_alloc` to succeed (per `slab::init_per_cpu`'s `debug_assert!`). And the AP needs the heap working before `mm::slab::init_per_cpu` does its `Box::new(SlabPerCpuState::new())`.

### 3.3 `tools/xtask/src/main.rs` — add `--smp N` support

Currently the QEMU invocations in `cmd_neo_test` (at `run_test_scenario` ~line 656) and in `run_qemu` (at ~line 718) have no `-smp` flag, so QEMU defaults to `-smp 1`. 1b's verification gate requires `-smp 4`.

**Option taken:** read an env var `NEO_SMP` (default `"1"`), pass `-smp N` to QEMU in both code paths. Default-1 preserves existing single-CPU CI. The verification harness sets `NEO_SMP=4` for 1b's gate.

Skeleton:

```rust
fn smp_args() -> Vec<String> {
    let n = env::var("NEO_SMP").unwrap_or_else(|_| "1".to_string());
    vec!["-smp".to_string(), n]
}

// In run_test_scenario, after the `qemu.args([...])` block:
qemu.args(smp_args());

// In run_qemu, similarly:
qemu.args(smp_args());
```

This is small and localized. Don't pile additional xtask changes onto 1b — anything beyond this is its own follow-on.

### 3.4 What you do NOT touch in 1b

- `kernel/src/sched/` — anything. Scheduler is sub-commit 2's surface.
- `kernel/src/ipc/` — anything. IPC SMP work is sub-commit 3.
- `kernel/src/mm/paging.rs` — already done in sub-commit 0.
- The compatibility shim `pub fn init_traps()` in `mod.rs` — leave it; main.rs still calls `init_traps_bsp` via the shim or directly. If you remove the shim, that's its own line item; defer to keep this commit clean.
- The `#[allow(dead_code)]` annotations on `init_traps_ap`, `idt::load_for_ap`, `syscall::init_ap`, and `per_cpu::init_ap`. Those become live in 1b — either remove the `allow(dead_code)` (preferred; the function is now called), or leave it (acceptable if removing produces a noisy diff). Document either choice in your close report so PM can verify.

---

## 4. Anti-scope (explicit)

These are NOT in 1b:

- AP scheduler entry. APs spin-wait at the end of `ap_entry`. Period.
- Cross-CPU IPC. APs don't take IPC syscalls. (They don't take any syscalls; they don't run user code.)
- TLB shootdown infrastructure. Not until sub-commit 4.
- IPI infrastructure. Not until sub-commit 4.
- x2APIC. ADR-013 anti-scope; `MpRequest::new(0)`.
- LAPIC timer setup on APs. APs don't run threads; they don't need a timer.
- Cross-CPU thread migration. Sub-commit 3b territory.
- `cpu_id_from_apic_id()` function. Replaced by `extra_argument` passing per §2 above.
- AP-level panic handler hardening. APs panic via `serial_println! + halt_forever()` for now; per-AP panic propagation is its own concern.
- Removing the `init_traps()` compatibility shim. Cleanup pass, not 1b.

---

## 5. Verification gates (what the close report must show)

All gates are PM-verifiable from your output. Run them; paste literal output; do not paraphrase.

### 5.1 Build

```
docker compose run --rm neo cargo k
```

Expected: green build. Warning count **18 (matches 1a baseline) or close to 18**. Any delta needs a one-line explanation per warning. Likely deltas:

- Removing `#[allow(dead_code)]` from `init_traps_ap`, `idt::load_for_ap`, `syscall::init_ap`, `per_cpu::init_ap` should retire 0–4 dead-code warnings (those functions are now actually called from `ap_entry`). Net warning count may drop slightly. That's good.
- New code may introduce new warnings; surface and explain each.

### 5.2 Single-CPU boot (anti-regression for 1a)

```
NEO_SMP=1 docker compose run --rm neo cargo neo-tty
```

Expected: identical-to-1a output. Specifically:

- `[ kernel ] gdt: cpu=0 loaded (tss_sel=0x30)` (unchanged from 1a).
- `[ kernel ] per_cpu: PER_CPU[0] at 0x... → IA32_GS_BASE (BSP)` (unchanged).
- `[ kernel ] paging: GLOBAL fixup complete - 4K leaves: 4575, 2M leaves: 251, 1G leaves: 0` (unchanged; 4K count of 4575 is preserved because IST/TSS storage is the same static allocation).
- `[ kernel ] paging: sanity OK - kernel .text sample virt VirtAddr(0x...) -> phys PhysAddr(0x...) (global=true)` (the address differs per boot due to KASLR — that's evidence of a real boot).
- `[ kernel ] smp: Limine reports 1 cpu(s); bsp_lapic_id=0` (NEW: 1b's MP request still gets a response with 1 CPU on `-smp 1`).
- `[ kernel ] smp: started 0 AP(s)` (NEW: nothing to start).
- `[ kernel ] frame: leak audit clean (baseline=N final=N)` for some N (typically 1026; minor variation acceptable if explained).
- `[ kernel ] phase 0 boot complete`.
- No `BUG:`. No `KERNEL PANIC`. No `gp:` exception.

### 5.3 Multi-CPU boot (the new gate)

```
NEO_SMP=4 docker compose run --rm neo cargo neo-tty
```

Expected: BSP output as 5.2 above, PLUS:

- `[ kernel ] smp: Limine reports 4 cpu(s); bsp_lapic_id=0` (the 0 may differ; whatever Limine reports).
- `[ kernel ] smp: starting cpu_index=1 lapic_id=...` for cpu_index 1, 2, 3 (interleaved with AP output below; interleaving is OK as long as every line is present).
- `[ kernel ] smp: started 3 AP(s)`.
- For each of cpu_id 1, 2, 3: `[ kernel ] per_cpu: PER_CPU[N] at 0x... → IA32_GS_BASE (AP)`.
- For each of cpu_id 1, 2, 3: `[ kernel ] gdt: cpu=N loaded (tss_sel=0x30)`.
- For each of cpu_id 1, 2, 3: `[ kernel ] syscall: SYSCALL/IRETQ enabled (LSTAR=..., SFMASK=IF|TF|AC|DF)`.
- For each of cpu_id 1, 2, 3: `[ kernel ] slab: initialized per-CPU state cpu=N at 0x...`.
- For each of cpu_id 1, 2, 3: `[ kernel ] smp: AP cpu_id=N ready (lapic_id=..., processor_id=...)`.
- BSP reaches `phase 0 boot complete`.
- No `BUG:`. No `KERNEL PANIC` from any CPU. No sentinel mismatch.

**Interleaving is expected** — APs run concurrently with the BSP after `bring_up_aps` returns. As long as every required line appears at least once, the order doesn't matter for verification purposes.

### 5.4 Test suite

```
NEO_SMP=1 docker compose run --rm neo cargo neo-test
```

Expected: 23/23 default scenarios PASS, plus `expect_pass_but_fail` meta-test detection. **Identical to 1a's test result.**

```
NEO_SMP=4 docker compose run --rm neo cargo neo-test
```

Expected: 23/23 PASS plus meta-test. The kernel under test still has a single-CPU scheduler (sub-commit 2 hasn't happened yet); APs spin-wait throughout each test. Tests that exercise IPC, capability operations, scheduling, etc. all run on the BSP. APs being present should not affect any test outcome — they're idle by construction.

If any scenario fails on `-smp 4` that passes on `-smp 1`, that's a 1b bug. Likely root causes:

- Wrong AP affecting BSP via shared mutable state that should have been per-CPU.
- AP panicking silently and that panic somehow propagating.
- Limine MP request placement breaking some other Limine response (e.g., interleaving with `MEMMAP_REQUEST` / `MODULE_REQUEST`).

Investigate before reporting close.

### 5.5 Frame leak audit clean across all boots

The frame leak audit must close on every `cargo neo-tty` boot above. `final == baseline` is the gate. APs never spawn user processes, so they don't disturb the frame allocator after init.

---

## 6. Commit shape

Single commit on `main`. Suggested commit message:

```
build-3 sub-commit 1b: AP bring-up via Limine MP request

Per ADR-013 §Q2 + Amendment 2. APs reach `ap_entry` spin-wait; per-AP GDT,
TSS, IDT load, MSRs, and slab state are wired. APs do NOT enter the scheduler
(that's sub-commit 2).

- New `kernel/src/arch/x86_64/smp.rs` with `MP_REQUEST` static and
  `bring_up_aps()` BSP entry plus `ap_entry()` AP entry.
- BSP iterates `MP_REQUEST.response().cpus()`, skips itself by lapic_id,
  passes sequential `cpu_id` via `extra_argument` so APs read their id
  directly without an APIC-ID-to-cpu-id mapping table.
- `_kernel_start` calls `arch::x86_64::smp::bring_up_aps()` after `mm::init`
  and before `sched::init`.
- `tools/xtask/src/main.rs` reads `NEO_SMP` env var (default "1") and passes
  `-smp N` to QEMU in `cmd_neo_test` and `run_qemu`.
- `#[allow(dead_code)]` removed from `init_traps_ap`, `idt::load_for_ap`,
  `syscall::init_ap`, and `per_cpu::init_ap` (now live).

Verified: NEO_SMP=1 boots identically to sub-commit 1a; NEO_SMP=4 reaches
`phase 0 boot complete` with all three APs sentinel-verified and slab-wired;
full default test suite (23/23) PASSes under both NEO_SMP=1 and NEO_SMP=4.
```

Adjust wording as you see fit; the substance must reflect what you actually did.

---

## 7. Risks and mitigations (read before implementing)

### 7.1 GS-base wire (highest risk)

If `per_cpu::init_ap(cpu_id)` writes the wrong address into `IA32_GS_BASE`, every subsequent `gs:[N]` access on that AP corrupts random kernel memory. The sentinel check in step 2 of `ap_entry` is the mitigation: `current_cpu_id()` reads `gs:[24]`; if it doesn't match the `cpu_id` argument, panic-halt before any other code runs.

**Action:** verify `per_cpu::init_ap` (already in tree from 1a) sets `IA32_GS_BASE = &PER_CPU[cpu_id] as *const PerCpu as u64` and `IA32_KERNEL_GS_BASE = 0`. Read the function. Don't trust this dispatch's prose; trust the file.

### 7.2 The AP arrives with Limine's GDT, not ours

When Limine starts an AP, it loads its own minimal GDT (long-mode kernel CS/DS) on the AP. The AP must call `gdt::init_ap(cpu_id)` (via `init_traps_ap`) to switch to its own per-AP GDT. Until it does, the AP's CS/DS reference Limine's table. This is fine for the few instructions between `ap_entry` start and `init_traps_ap` (which loads the new GDT), but you must not take an interrupt or fault in that window — Limine's IDT (if any) is not ours.

**Action:** `IF=0` (interrupts disabled) is the AP's state on entry per Limine spec; do not enable them in `ap_entry` before `init_traps_ap` runs. ADR-013 doesn't have an `enable_interrupts()` call on APs — leave them disabled. Sub-commit 2 will figure out when (if ever) APs enable interrupts.

### 7.3 The AP arrives with Limine's stack

Limine provides each AP with a 64 KiB stack. Don't blow it. `ap_entry` does very little stack-allocation work; calling `init_traps_ap` and `init_per_cpu` involves heap allocations on the BSP path but the AP's stack is fine for its locals.

**Action:** no recursion, no large `[u8; N]` locals. The current shape doesn't have any; preserve that.

### 7.4 Ordering: GS-base BEFORE init_traps_ap

`gdt::init_ap(cpu_id)` writes `PER_CPU[cpu_id].tss_ptr` (visible from sub-commit 1a's `set_kernel_rsp` lookups). For that write to be safe to read later via `current_per_cpu().tss_ptr`, GS-base must already be wired. The order is therefore:

1. `per_cpu::init_ap(cpu_id)` ← writes GS-base
2. sentinel-verify
3. `init_traps_ap(cpu_id)` ← uses GS-base via `current_per_cpu()` for tss_ptr
4. `slab::init_per_cpu(cpu_id)` ← uses heap, also touches per-CPU state

The skeleton above gets this right. Do NOT reorder.

Note: `gdt::init_ap` uses `set_slab_state_for_cpu` style explicit-cpu-id helpers in 1a's pattern, so it doesn't strictly require GS-base wired — but the sentinel verify requires GS-base, and it's cleaner to wire GS-base first regardless.

### 7.5 `MP_REQUEST` placement

The Limine request must be in the `.requests` linker section between the start and end markers (see existing `FRAMEBUFFER_REQUEST` etc. in `main.rs`). Putting the new `MP_REQUEST` static in `smp.rs` works because `#[used] #[unsafe(link_section = ".requests")]` is what the linker honors, regardless of which Rust module it lives in. The start/end markers in `main.rs` bracket the entire `.requests` section across all object files in the kernel binary.

**Action:** verify the resulting binary still has the Limine markers in the right place (`readelf -S target/.../kernel | grep .requests`). The build will fail loudly if anything is off; this is a sanity check, not a debugging step.

### 7.6 Things QEMU can do that real hardware can't

QEMU's `-smp 4` brings up 4 CPUs synchronously and presents them via Limine's MP response cleanly. Real hardware has more variability: Limine reports CPUs the BIOS told it about; some firmware quirks may surface. **1b is verified on QEMU only.** Bare-metal SMP verification is on the BUILD-3 close milestone (per ADR-013 migration checklist), not the sub-commit 1b close. If something here is QEMU-clean but bare-metal-broken, that's a future-BUILD finding.

---

## 8. Pre-implementation OBSERVE pass (mandatory)

Before writing any code, read the following files in full and confirm the dispatch matches reality:

1. `kernel/src/main.rs` — confirm boot order. The `_kernel_start` function around lines 220–300 is what matters. Note the actual line numbers so your close report quotes them correctly.
2. `kernel/src/arch/x86_64/per_cpu.rs` — confirm `init_ap`, `current_cpu_id`, and `MAX_CPUS`. Note that `init_ap` already exists with `#[allow(dead_code)]` (sub-commit 1a stub).
3. `kernel/src/arch/x86_64/mod.rs` — confirm `init_traps_ap` exists (it does, with `#[allow(dead_code)]`).
4. `kernel/src/arch/x86_64/gdt.rs` — confirm `init_ap` (writes per-CPU TSS via static storage), `set_kernel_rsp` (uses `current_per_cpu().tss_ptr`).
5. `kernel/src/arch/x86_64/idt.rs` — confirm `load_for_ap` exists.
6. `kernel/src/arch/x86_64/syscall.rs` — confirm `init_ap` exists.
7. `kernel/src/mm/slab.rs` — confirm `init_per_cpu(cpu_id)` exists and uses `set_slab_state_for_cpu(cpu_id, ...)`.
8. `tools/xtask/src/main.rs` — locate the two QEMU invocation sites (around lines 656 and 718).
9. `kernel/Cargo.toml` — confirm `limine = "0.6"`. Verify the resolved version in `Cargo.lock` if you want to be thorough — should be `0.6.3`.

If any of the above contradicts this dispatch, **flag the contradiction in your close report and either deviate (with ADR-013 amendment subsection) or surface the problem to PM before continuing**. Do not silently rewrite.

---

## 9. Out-of-scope items if you spot them during OBSERVE

Don't fix these; surface them in your close report under "Observations" so PM can triage:

- Stale comments referring to "Phase 1.4" or older that should now reference Phase 2.
- `#[allow(dead_code)]` annotations elsewhere that look stale post-1b.
- Any unrelated TODOs you notice in the touched files.

---

## 10. If something blocks you

If you hit something this dispatch didn't anticipate — Limine API surprise, AP entry fails in a way that doesn't match any of the §7 risks, sentinel mismatches consistently, anything at all — **stop, don't push through, report up**. The 1a discipline pattern (deviation flagged in ADR-013 Amendment 1's "Implementation correction" subsection) is the right shape: name the assumption, name what was actually true, propose forward.

Standing by for your close report.

---

**End of dispatch.**
