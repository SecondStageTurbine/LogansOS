<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-40 — make in-kernel inference preemptible (the IF=0 liveness fix) → Jarvis

**Date:** 2026-06-24
**From:** PM (Claude)
**To:** Jarvis (forward-path / scheduler lane). New branch off `origin/main` HEAD: `codex/ot40-inference-preemptible-jarvis`.
**Status:** DISPATCHED — the architectural follow-on the **CB-1b/OT-37 adversarial-audit gate** surfaced (lens 2). **Non-conflicting with the live bare-metal session** by subsystem (this is the syscall/scheduler inference path, not mt7601u/xHCI/DHCP) — but **coordinate the MERGE** with Cerebro (see §5). **Kernel-touching ⇒ 4-shard QEMU gate + AAG.**

## 0. The decision this build makes
Today the inference syscalls (`sys_generate` 0x121, pre-existing `sys_infer_run` 0x120) run the **entire forward with interrupts disabled** (SYSCALL clears IF via SFMASK; `syscall_entry` never `sti`s before dispatch). A multi-token / large-model generation therefore masks the **calling CPU's timer tick + residency/liveness accounting** for the whole run — an **I-14(b)** liveness-degradation hole (OT-40). Make in-kernel inference **preemptible** so a long generation no longer freezes its CPU, *without* changing the emitted tokens.

## 1. References
```
git fetch origin
git show origin/main:kernel/src/arch/x86_64/syscall.rs   # SFMASK/IF on entry (~208); syscall_entry asm (no sti before dispatch, ~333); sys_generate_impl loop; sys_infer_run_impl
git show origin/main:kernel/src/sched/scheduler.rs       # on_tick / preemption — the timer path that's currently masked
git show origin/main:kernel/src/ai/qwen_inventory.rs     # next_logits — the per-token forward (KV cache is per-call local; no lock held across it)
git show origin/main:docs/OPEN-THREADS.md                # OT-40 (the finding + the two approaches)
git show origin/main:docs/complexity-governance.md       # I-14(b) liveness; the kill-switch rule
```

## 2. ISCs
- **ISC-1 — Make the forward preemptible.** OBSERVE the two approaches and pick the minimal-correct one:
  - **(A, recommended) Re-enable interrupts during the compute.** The autoregressive loop holds only **stack-local** state (the AAG concurrency lens confirmed: per-call KV cache, no lock held across the forward), so it is safe to run with `IF=1`. Enable interrupts for the inference compute window (at minimum at each token boundary; ideally across `next_logits`) so the timer can preempt; restore the syscall's IF state on return. Verify no path inside the forward assumes IF=0.
  - **(B) Run the forward on a preemptible kernel thread** rather than in the IF=0 syscall body — more invasive; only if (A) proves unsafe.
- **ISC-2 — Identical tokens (the equivalence anchor).** The fix changes *scheduling*, not *arithmetic*: assert the emitted token sequence is **bit-for-bit identical** with vs without the fix on a fixed prompt (reuse the OT-37 fixture path / a deterministic small-model decode). Preemption must not perturb the greedy output.
- **ISC-3 — Liveness proof.** Add a gate showing the calling CPU's **timer tick / preemption advances during a long generation** (esp. at **`NEO_SMP=1`**, the worst case where IF=0 froze *all* governance). The degradation OT-40 named must be gone.
- **ISC-4 — Kill-switch.** Ship behind a switch; **disabled ≡ the current IF=0 behavior, bit-for-bit** (an I-2-form collapse), with an equality regression. (Default-on is fine since ISC-2 proves token-identity — but the switch must exist.)
- **ISC-5 — standards.** SPDX; `cargo fmt --all --check`; clippy `-D warnings` (kernel blocking) via Docker. No new cross-cutting invariant (this *restores* I-14(b)); any new `unsafe` (IF manipulation) justified + minimal.

## 3. Failure-mode awareness (it's the interrupt path — be careful)
- Re-enabling IF mid-syscall exposes the inference path to **timer/IPI reentrancy** — confirm the per-CPU TSS.rsp0 / stack handling is correct for a preemption taken *inside* the syscall, and that the forward's stack-local state survives a context switch and resume (it must, since it's just stack — but prove it).
- The **SMAP/`with_user_access`** windows for the prompt-in / reply-out copies must NOT straddle a point where IF is enabled and a preemption could fire mid-copy with user access open — keep user-access windows IF=0 / short, do the long compute IF=1.
- Watch for any **spin-lock** taken with IF=1 that an interrupt handler also takes (deadlock) — the forward takes no lock across the loop (verified), but re-confirm after the change.

## 4. I-14 / scope
This build **restores I-14(b)** (liveness) — state it. No new device driver, no forward-math change (tokens identical), no `.neom`/vocab change. **Scope: the inference-preemption fix ONLY** — do NOT touch mt7601u/xHCI/DHCP (the live bare-metal subsystem), the forward arithmetic, or the chat template.

## 5. Gate + close (PM) — MERGE COORDINATION
- **4-shard QEMU `workflow_dispatch` green** at `NEO_SMP=4` **and** an `NEO_SMP=1` liveness check (ISC-3) + the token-identity equality (ISC-2). **Kernel-touching ⇒ AAG** (lenses: concurrency/reentrancy on the IF-enable; interrupt/EOI ordering; completeness — does the liveness gate actually prove preemption fired?).
- **MERGE COORDINATION (the one cross-lane note):** this changes `sys_generate`'s runtime behavior, which the **live bare-metal Arthur run** observes. Hold the merge / coordinate with Cerebro so it lands **either before the bare-metal Arthur run (so silicon tests the fixed behavior) or after that run has captured its result** — not mid-iteration. The PM sequences this at merge; flag it in the RETURN.
- **Close-gate `scripts/worker-close-gate.sh`** (rebase LAST; full set; SPDX; self-stat). RETURN green push-CI + the 4-shard QEMU URL + the `NEO_SMP=1` liveness evidence + the token-identity proof. **Disk reclaim** after merge per CLAUDE.md.

---
## Relay — what to tell Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-24-ot40-inference-preemptible-jarvis.md` (+ OT-40 in `OPEN-THREADS.md`, `syscall.rs`, `scheduler.rs`, `qwen_inventory.rs`). **Branch:** `codex/ot40-inference-preemptible-jarvis` off `origin/main` HEAD.
- **One-line:** **Make in-kernel inference preemptible — the IF=0 liveness fix the AAG caught.** `sys_generate`/`sys_infer_run` run the whole forward with interrupts disabled (SFMASK clears IF; no `sti` before dispatch), so a long generation freezes the calling CPU's timer/preemption + residency accounting (I-14(b) hole). Fix (recommended): **re-enable interrupts during the compute** — the autoregressive loop is stack-local (per-call KV cache, no lock across the forward, AAG-confirmed), so it's safe to run `IF=1`; keep the SMAP user-copy windows IF=0/short, do the long compute IF=1. **Prove:** (a) emitted tokens **bit-for-bit identical** with/without the fix (scheduling change, not arithmetic — reuse the OT-37 fixture), (b) the calling CPU's **timer/preemption advances during a long generation at `NEO_SMP=1`** (the worst case), (c) **kill-switch** disabled ≡ current IF=0 behavior. **Watch:** timer/IPI reentrancy inside the syscall (TSS.rsp0/stack), no lock taken IF=1 that an ISR also takes. **Restores I-14(b).** **Scope: the preemption fix ONLY — no mt7601u/xHCI/DHCP (Cerebro's live bare-metal subsystem), no forward-math/template change.** **Gate:** 4-shard QEMU at `NEO_SMP=4` + an `NEO_SMP=1` liveness check + AAG. **MERGE is coordinated with the bare-metal Arthur run (PM sequences — flag it).** **Close-gate `scripts/worker-close-gate.sh`; RETURN green push-CI + QEMU URL + SMP=1 liveness + token-identity proof.**

---
## BOUNCE 2026-06-25 (PM, AAG) — silent SIMD-corruption blocker; the mechanism is right, the FPU-save and the test are not

The fix (`2a04469`) is **mechanically sound** — AAG lenses 2 & 3 cleared it: IF is toggled via safe wrappers with a `Drop`-guard restore on every exit path, the SMAP user-copy windows sit *outside* the IF=1 compute window (no preempt-mid-`stac` hole), the window holds **no** AI-governance lock, and I-14(b) liveness is genuinely restored. **But AAG lens 1 found a high-confidence silent-data-corruption BLOCKER, and lens 4 found why CI missed it.**

**BLOCKER (lens 1 — reentrancy).** `inference-preemptible` is in `default` and does **not** require `xsave-fpu`. The non-`xsave-fpu` `context_switch` (`kernel/src/arch/x86_64/context.rs:32-52`) saves only the 6 GP callee-saved registers — **no FPU/SSE/AVX**. The timer ISR's `fxsave` (`timer.rs:122-148`) protects only the *handler's* transient state and only XMM, never the interrupted forward's, and never across the task switch `on_tick → yield_now → context_switch` performs. So a real timer preemption fired `IF=1` **mid-forward** switches tasks and resumes the forward on **corrupted XMM/YMM → silently wrong logits/tokens.** This directly re-violates the **OT-23 YMM-XSAVE invariant** and the explicit `kernel/src/ai/ops/qk_matmul.rs:112-114` guard ("the scalar default exists precisely so the in-kernel forward cannot start using YMM by accident"). **Blast radius: the bare-metal Arthur run** (a multi-task boot — neo-init, daemons) **will** take real mid-forward preemptions and emit garbled tokens we'd misdiagnose as a coherence regression. Better caught here.

**Why CI passed green (lens 4 — completeness).** `timer_tick_delta=1` proves only that *an interrupt fired*, **not that a preemption occurred**: `yield_now` (`scheduler.rs:832-841`) no-ops when inference is the sole ready thread, so the test **never triggers the corrupting context-switch**. The green gate passed *because* it didn't exercise the dangerous path. The kernel even has an unused `CONTEXT_SWITCH_COUNT` (`scheduler.rs:67,682`) that would have proven a real switch.

### Fix (the bounce)
- **ISC-F1 (the blocker):** `inference-preemptible` MUST require `xsave-fpu`, AND the preemption context-switch must save/restore the forward's **full SIMD state — YMM via XSAVE, not just XMM via `fxsave`** — so a mid-forward switch is register-safe. Re-honor the OT-23 invariant + the `qk_matmul.rs:112` guard. (Verify the `xsave-fpu` context-switch path actually covers YMM for the resumed forward.)
- **ISC-F2 (make the gate actually prove it — this is what would have caught F1):** the liveness scenario must prove a **real preemption/context-switch** occurred mid-forward — assert `CONTEXT_SWITCH_COUNT` advanced (it exists, unused) **or** run a second ready thread that forces `yield_now` to switch. `timer_tick_delta` alone is insufficient.
- **ISC-F3 (token-identity under *real* preemption, on the *real* path):** prove emitted tokens are bit-identical across an **actual mid-forward context switch** (not IF=0-vs-IF=1 back-to-back), and cover the **`sys_generate` (0x121) multi-token path** — today only `sys_infer_run` + a library-`generate` 3-token oracle are checked; the headline syscall has no identity assertion.
- **ISC-F4:** add the **`NEO_SMP=1`** liveness case (the worst case the IF=0 hazard targeted; the original dispatch required it — it's currently SMP=4 only).
- **Keep:** the IF-wrapper mechanism + Drop-guard restore + SMAP non-overlap + lock discipline (lenses 2/3 clean) — the fix is the FPU-save + the test rigor, not a rewrite.

### Relay — what to tell Jarvis (BOUNCE)
- **One-line:** **The mechanism is right; it silently corrupts SIMD and the test never caught it.** AAG found: `inference-preemptible` ships in `default` WITHOUT `xsave-fpu`, so the non-xsave `context_switch` saves no FPU/YMM — a real timer preemption mid-forward resumes on **corrupted XMM/YMM → wrong tokens** (re-violates OT-23 + the `qk_matmul.rs:112` YMM-by-accident guard). CI passed only because `yield_now` no-ops as the sole ready thread, so `timer_tick_delta=1` proved an *interrupt fired*, **not a preemption**. **Fix:** (F1) make `inference-preemptible` require `xsave-fpu` + save the full **YMM** (XSAVE, not `fxsave`) across the preemption switch; (F2) make the liveness test assert a **real context-switch** (`CONTEXT_SWITCH_COUNT++` or a second ready thread), not just a tick; (F3) prove token-identity across an **actual preempted** run AND on the **`sys_generate` 0x121** path (currently only `sys_infer_run`); (F4) add **`NEO_SMP=1`**. Keep the IF-wrapper + Drop-restore (lenses 2/3 cleared them). Re-gate: 4-shard QEMU + AAG. RETURN green push-CI + QEMU URLs + the real-preemption proof + `sys_generate` token-identity + SMP=1.
