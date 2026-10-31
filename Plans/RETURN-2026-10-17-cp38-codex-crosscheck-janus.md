<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: CP-38 / CP-39 cross-family review — Janus

## Scope and verdict

**Verdict: BOUNCE for checklist item 6 (AC/SMAP entry state).** The primary
CP-38 restore-clobbers-newer-state hypothesis did **not** survive tracing.
The new FPU save/restore is structurally sound under the existing scheduler
and homogeneous-CPU FPU policy. Keep that fix; the requested entry-state
review also uncovered the confirmed security-boundary gap below.

The AC defect **predates this PR**. It is not a regression introduced by
`a9ffbad3`, and reverting CP-38 would not fix it. I am treating it as a
blocking result of the dispatch's explicit AC audit, not misattributing it
to the FPU change.

- Worktree: `E:\AI Automation\workers\janus` only.
- Return branch: `codex/cp38-crosscheck-janus`.
- Reviewed PR head: `c2f2eae60671b4d46c1b44469fa4e609ae650486`.
- CP-38 commit: `a9ffbad3f97e7895bc1632ea5f09d176bc1815b9`.
- Comparison base: `57261d66f9c777e02a167370349a8a5fc6383b0a` (`origin/main`
  when fetched). Read all eight changed files in the three-dot branch diff,
  including the later CP-39 change, feature wiring, scenario selection and docs.
- RAG: `http://127.0.0.1:8404/health` failed to connect. Used direct reads and
  exact-symbol searches, as the dispatch permits. Did not change RAG services.
- Review only: no production or test-code change is included in this return.

## Finding F1 — P1: interrupt entry inherits AC, bypassing SMAP

**Locations:** `kernel/src/arch/x86_64/idt.rs:233` (common entry through the
call at line 245); `kernel/src/arch/x86_64/timer.rs:124` and `:247` (both
timer variants). Supporting policy: `arch/x86_64/mod.rs:92`, `:196`;
contrast `arch/x86_64/syscall.rs:456`.

The new `cld` establishes DF=0, but neither entry sequence clears AC. The
FPU helpers do not clear it either. With CR4.SMAP enabled, an IRQ arriving
with AC=1 therefore runs Rust with explicit supervisor accesses to user
pages permitted outside `with_user_access`. Userspace can set AC with
`pushfq`/`popfq`; an aligned instruction loop can wait for a hardware IRQ.
A ring-0 interrupt inside an existing user-access window has the same issue.
The timer may additionally switch into another kernel continuation with
AC still set: the context shims do not switch RFLAGS.

This is a demonstrated missing protection, not a claim of a complete
privilege-escalation exploit or of a specific unchecked pointer in a driver.
The kernel explicitly enables SMAP; its entry path must establish the
closed access window independently of the interrupted context.

**Independent re-verification:** inspected the entire common IRQ entry,
both timer entries, both FPU helpers, and the actual built IRQ disassembly.
None changes AC. The syscall boundary already masks AC in SFMASK. Intel
documents the AC access override and the interrupt-cleared flags in SDM
Vol. 3A §§2.3, 4.6 and 6.12.1.3; AC is not among the interrupt-cleared flags.
[Intel SDM Vol. 3A](https://cdrdv2-public.intel.com/812386/253668-sdm-vol-3a.pdf).

**Diagnostic reproduction:** temporarily extend the existing `irq_xmm_preserve`
scenario's single asm block around its `std; int 0xe0; cld` sequence:

```asm
pushfq
or qword ptr [rsp], 0x40000
popfq
std
int 0xe0
cld
pushfq
and qword ptr [rsp], -262145
popfq
```

In `irq_xmm_test_body`, count `rflags & (1 << 18)` instead of bit 10, and
label the failure `ac_set_in_irq_body`. The stub and FPU helpers remain
unchanged. This probes ring-0 entry; the ring-3 hardware-delivery consequence
is a source/architecture argument, not a claimed ring-3 runtime test.

Runtime result: **FAIL**, `reason=ac_set_in_irq_body body_ac_set=64 rounds=64`,
with QEMU debug-exit status 35. The unmodified baseline passed. Repeated
boots produced the same 64/64 result; this is not an inferred failure from
the absence of a `clac` mnemonic alone.

**Requested fix:** establish AC=0 before invoking helpers/Rust in the common
IRQ entry and both timer variants, preserving the original interrupt frame
for `iretq`. Do not unconditionally emit `clac` on non-SMAP CPUs (`qemu64`
must still work); use a supported-CPU guard or a compatible flags-mask
sequence on the live flags. Add AC-entry and restored-caller-flags assertions.
The returning exception boundary should receive the same follow-up audit.

## Numbered checklist

### 1. Restore versus context switch — no FPU finding

`idt.rs:512`'s reschedule body is EOI-only (plus diagnostic counters).
It does **not** call `yield_now`. AHCI/NVMe acknowledge/count; xHCI drains
events; virtio-accel collects completions; virtio-blk's completion path can
wake a token, but `scheduler.rs:1891` queues that thread and optionally sends
an IPI, rather than switching the interrupted thread. TLB shootdown handles
the request and returns. Thus no current direct context-switching path was
found among the common-stub bodies.

I nevertheless traced the hypothetical switching case against the real
timer path (`timer.rs:311`, `scheduler.rs:1177`, `:1200`) and all three
XSAVE context shims (`context.rs:101`):

1. A is interrupted with image **A0**. The stub saves A0 on **A's kernel
   stack**. The handler computes with temporary image **AH**.
2. A switches to B. The context shim saves AH in A's per-thread FPU area,
   restores B's per-thread area, loads **B's RSP**, and returns to **B's
   continuation**. It cannot fall through A's suspended stub epilogue on B.
3. When A is selected again, the shim restores AH and **A's saved RSP**.
   A resumes inside its handler. On that handler's eventual return, its
   own stub restores A0 and returns to the interrupted instruction.
4. A0 is intentionally older than AH: AH belongs to the temporary handler,
   not to later execution of A's interrupted program. B's newer state lives
   in B's continuation/per-thread area and is not overwritten by A's stack
   image. The next switch out of A refreshes its per-thread FPU image.

For ring 0, the common stub uses the interrupted thread's stack directly.
For ring 3, the no-IST IRQ gate uses TSS.rsp0; `scheduler.rs:868`, `:874`,
`:910` update the thread kernel stack, and the common stub preserves its
`from_ring3` marker on that stack. Resumption unwinds A and swaps GS back
only at A's user return. The timer's separate IST entry first copies its
frame onto the corresponding thread stack, so it has the same ownership
argument. A terminating handler never returns to its stub, so it cannot
restore a dead thread's image into the next runnable thread.

Forced-switch diagnostic: temporarily made `irq_xmm_test_body` call
`sched::yield_now` and spawned a peer that repeatedly set XMM0, entered the
same vector and verified XMM0 after return. The two threads repeatedly
suspended inside separate instances of this IRQ stub. Result: **PASS**,
`peer_rounds=64 body_runs=129` with the original thread's 64 distinct
XMM0-15 rounds also preserved. This directly exercises the stack ownership
argument above on the XSAVE path. The diagnostic changes were then removed.

### 2. Alignment, size and initialization — no new finding

- The marker plus 15 saved GPRs plus anchor place the marker at `[rbp+128]`.
  The legacy path rounds RSP down to 16 and subtracts 512. The XSAVE path
  rounds down to 64 and subtracts a 64-byte multiple. Each helper/body call
  therefore starts with caller RSP divisible by 16; callee entry is 8 mod 16.
- The call's return address and callee frames are **below** the save area;
  neither overwrites it. The target disables the red zone. On return, RSP
  still identifies the same save area; final unwinding uses RBP, not an
  independently reloaded size.
- `fpu.rs:30` starts at 512; `:154` programs XCR0=x87|SSE|AVX, then rounds
  CPUID.0xD.0:EBX up to 64 bytes and publishes the mask/policy. The assembly
  helper zeroes the XSAVE buffer, including reserved header bytes, before
  saving. Helpers use no compiler-generated SIMD before the snapshot.
- BSP `main.rs:499` initializes FPU support before installing these traps
  (`:506`) or enabling IRQs (`:544`). AP `smp.rs:212` calls `enable_sse`
  before loading the shared IDT and before `run_ap_bootstrap` enables IF
  (`scheduler.rs:359`). There is no ordinary maskable-IRQ-before-XCR0 path.
- **Existing assumption, not independently hardened:** the policy globals
  are republished by each AP, not immutable after BSP initialization.
  Homogeneous CPUs publish the same size/mask. A heterogeneous AP selecting
  FXSAVE could change the global policy while a BSP XSAVE frame is live;
  the helper also reloads the size after the stub allocates it. Such a policy
  change is unsafe for the pre-existing timer/syscall/thread images too.
  No heterogeneous machine repro was performed. Freeze/validate a machine-wide
  policy before admitting CPUs if heterogeneous support is required.

### 3. Register clobbers — no finding

`fpu_save_area_asm` (`fpu.rs:40`) clobbers RAX, RCX, RDX, RSI, RDI and flags
on the XSAVE path (FXSAVE fallback only compares flags and saves). Restore
clobbers RAX/RDX and flags on the XSAVE path. Neither touches RBP or RSP
apart from call/ret. All interrupted GPRs were already pushed. Crucially,
`idt.rs:244` reloads `rdi = [rbp+128]` **after** the save helper, immediately
before the Rust body. Rust preserves RBP under SysV; the context shims also
save/pop it across stack switches. The duplicate RBP push is an intentional
anchor, not an unmatched push. Confirmed in the built test-entry disassembly.

### 4. AVX and exceptions — bounded safety; documentation overstates it

The kernel target's `features: ""` keeps baseline SSE2 and does not globally
enable AVX. Inspected the IRQ completion paths: no AVX matmul call is
reachable there. The explicit AVX2 functions are in `ai/ops/qk_matmul.rs`;
their runtime gate at `:997` checks OSXSAVE, XCR0.XMM/YMM and AVX2. The resolved
Blake3 1.8.5 and RustCrypto cpufeatures 0.2.17/0.3.0 paths suppress dynamic
feature detection on `target_os="none"` unless the feature is enabled at
compile time. Thus linking AVX implementations is not itself evidence that
these baseline IRQ bodies execute them.

FXSAVE does not protect YMM upper halves if a future body reaches AVX,
including a `vzeroupper`. Do not infer FXSAVE safety merely from the feature
name: the non-XSAVE boot path does not explicitly clear inherited OSXSAVE/XCR0,
and the matmul feature does not require `xsave-fpu` by itself. This is a
configuration/boot-policy caveat, not a reproduced IRQ-body regression.

**P2 nonblocking documentation note:** `idt.rs:129` and the CP-38 text in
`docs/concurrency-pitfalls.md:1414` should narrow “exceptions ... need no
change / never exposed” to the state actually protected. A small Rust probe
using this pinned compiler and an opaque C callee, plus actual kernel
breakpoint disassembly, show XMM0–15 spills and `cld`, **no** YMM-upper,
x87 or MXCSR save. If a returning exception reaches AVX code, that prologue
does not protect YMM upper halves; MXCSR status can also change even when
its ABI-preserved control bits do not. No such current returning AVX path
was established. Contrary to the dispatch example, `page_fault` at
`idt.rs:424` does not recover/return: its test hook returns only to fatal
handling, while user fault containment diverges. `breakpoint` is the actual
returning exception here. This is not a second reproduced blocker.

### 5. Coverage — what the tests do and do not establish

The committed discriminator tests all 16 XMM registers with distinct
patterns, verifies body count, enters with DF=1, and asserts the runtime
XSAVE/FXSAVE choice. It does not verify x87, MXCSR, YMM upper halves, caller
DF restoration (the caller immediately clears DF), or AC. Nor does it test
ring-3 entry, actual MSI/EOI delivery, AP execution, or scheduling inside
the body. The `0xe0` gate remains DPL0, so issuing that `int` from userspace
would fault rather than test ring-3 entry; use a hardware IRQ/IPI or an
explicit test-only DPL3 gate for that test.

Ring-3 and AP tests matter for stack/GS and CPU-local XCR0 setup. Real MSI
delivery matters for routing and EOI integration, although it shares this
same common assembly. YMM and switching tests matter for the extended-state
claim. A second maskable interrupt cannot interrupt the save: the gate
clears IF and the save helper does not enable it. NMI/exception delivery is
different; current NMI handling halts (`idt.rs:641`) and an invalid save
buffer faults fatally. This test provides no claim of recoverable NMI-safe
SIMD nesting. A future returning SIMD-using NMI needs its own preservation.

The `irq_xmm_preserve_fxsave` feature still uses default `xsave-fpu` and
exercises its **runtime fallback**, not the compile-time non-XSAVE macro.
The docs correctly distinguish that; CI coverage of the compile-time
variant remains a useful addition, not a claimed test performed here.

### 6. DF and other flags

DF clearing is correctly placed before every helper/Rust call in the common
stub and both timer variants. Before `cld`, only branches, pushes, scalar
loads/stores and stack/GS work execute; no string operation or Rust call
requires DF=0 there. The saved hardware RFLAGS frame remains unchanged for
`iretq`. IF/TF and RF/NT receive architectural entry treatment; AC does not.
**F1 is the surviving failure for this checklist item.**

### 7. Additional checks

Checked feature dependencies, the `concat!`/assembler macro expansion,
test-vector installation, scenario-name selection, `-cpu max` selection,
the fallback scenario, no-red-zone target configuration, thread versus IST
stack ownership, full GPR pop symmetry and existing stack capacities.
The common stub uses additional stack (512 bytes or the runtime XSAVE area,
plus alignment), as expected; I found no concrete stack-overflow repro.
No unrelated source patch, generated artifact, dependency update, or
existing untracked `graft/` content is included in this return.

## Validation and close

- Unmodified head: `cargo xtask neo-test irq_xmm_preserve` **PASS** under
  the local `neoplutarch-dev:latest` Docker image, QEMU TCG, one vCPU:
  `rounds=64 body_runs=64 xmm_regs=16 preserved=ok df_cleared=ok xsave=true`.
- Pinned compiler: rustc `1.97.0-nightly (e8e4541ff 2026-04-15)`.
- AC probe: deterministic **FAIL** on four boots, 64/64 each boot, as
  expected for the confirmed finding. Forced-switch probe: **PASS**,
  `peer_rounds=64 body_runs=129`, with both threads' XMM checks intact.
- These are local targeted checks, **not** a full CI/QEMU merge gate or a
  bare-metal result. No CI run was launched for this review-only return.
- Temporary diagnostic source changes were removed before committing.
- Disk reclaim: `cargo clean` removed 2,610 files / 1.4 GiB after testing.

## Re-check of `bc67f0e0`

**Verdict: MERGE.** The addendum fix resolves F1, so this re-check supersedes
the original BOUNCE verdict for the PR as amended by `bc67f0e0`. I found no
new blocking or nonblocking finding in the requested scope.

- **AC/DF mask:** `-263169` is exactly the 64-bit mask
  `~((1 << 18) | (1 << 10))` (`0xfffffffffffbfbff`). The mask is present after
  all GP-register pushes and before the first FPU helper or Rust call in the
  common entry (`kernel/src/arch/x86_64/idt.rs:237`) and both timer variants
  (`kernel/src/arch/x86_64/timer.rs:126`, `:253`). Before it, the stubs perform
  only entry classification, `swapgs`, scalar frame copying and GP saves. The
  `pushfq`/`and [rsp]`/`popfq` sequence is stack-neutral, preserves every live
  flag except AC and DF, and never addresses the hardware-saved RFLAGS frame.
  In particular it does not change the common stub's `[rbp+128]` marker offset
  or call alignment. Interrupt-gate entry has already cleared live IF, so the
  `popfq` cannot reopen interrupts; `iretq` later restores the caller's AC/DF
  from the untouched frame. Avoiding unconditional `clac` preserves operation
  on the no-SMAP `qemu64` leg.
- **Permanent discriminator:** the single inline-assembly block sets AC and DF
  immediately before `int 0xe0`, captures RFLAGS immediately after `iretq`,
  then clears DF and AC before leaving the block
  (`kernel/src/test_harness.rs:28780`). The body independently counts either
  flag arriving set, while `caller_flags_lost` proves both bits survived the
  return (`:28819`, `:28854`, `:28863`). Thus removing either mask bit fails
  the corresponding body assertion, while corrupting the saved frame fails
  the restoration assertion. The same code runs in the XSAVE-active and
  runtime-FXSAVE fallback scenarios. This remains a ring-0 software-interrupt
  discriminator and does not directly exercise a ring-3 source or either
  timer stub; the documentation states the timer gap explicitly. I inspected
  the claimed control design but did not rerun the optional QEMU scenarios.
- **CP-38 / CP-40 documentation:** CP-38 now limits the exception-handler
  claim to XMM0-15 plus `cld`, names the missing YMM-upper/x87/MXCSR state, and
  correctly identifies `breakpoint` as the returning exception
  (`docs/concurrency-pitfalls.md:1415`). CP-40 accurately records the AC/SMAP
  mechanism, the compatible mask, the discriminator's bounds, and the still
  open `x86-interrupt` exception-entry AC audit (`:1475`). That exception-side
  work is clearly scoped as follow-up and is not represented as fixed here.
- **Review mechanics:** reviewed commit
  `bc67f0e062e96994e2a649fa2545840db97a90d0` directly from the fetched PR
  branch. `git diff-tree --check` reported no patch-integrity errors. The RAG
  health endpoint again refused the connection, so I used direct commit reads
  as the dispatch permits. Graft's prebuilt index had not incorporated this
  commit and was not used as evidence.
