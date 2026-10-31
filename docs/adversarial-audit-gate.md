<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# The Adversarial-Audit Gate (AAG) — PM standing rule for kernel-touching merges

**Status:** ACTIVE (established 2026-06-24). PM-owned. Peer to the byte-oracle rule, the
blind-hardware discriminator rule, and the kill-switch rule.

## Why this exists
When a model can author a booting kernel in minutes (the Tolmo/Fable-5 NT-TCB demo, 2026), **authoring
stops being the constraint and verification becomes the entire game.** The same capability that *writes* a
trusted computing base can be pointed *at* one — to find the concurrency bug in the dispatcher hand-off, the
ordering mistake in the EOI path, the ABI drift that breaks a real driver. Those three are not hypothetical
for NeoPlutarch; they are our actual bug history (OT-27c SMP hang; the timer-preempt-disable masking attempt;
P4b SeqCst; the `.neom` `weight_off` absolute-vs-relative drift that birthed the byte-oracle rule). The QEMU
gate, the byte-oracle, and the kill-switch each catch a *structural* class of wrongness; none of them
systematically hunts **logic races, interrupt-ordering mistakes, ABI drift, or coverage gaps the gate's
fixtures don't exercise.** The AAG is the lane that does — an adversarial AI pass that tries to *refute* a
build's correctness before it lands. Better the model breaks the kernel on the branch than someone else breaks
it on `main`.

## When it triggers
- **MANDATORY** as a leg of PM verify-before-merge for **every kernel-touching branch** — the same trigger
  as the mandatory 4-shard QEMU `workflow_dispatch`. It runs **after** the branch is CI-green and ff-safe,
  **before** the `--ff-only`/cherry-pick to `main`.
- **EXEMPT:** doc-only and host-only branches (no kernel risk surface) — e.g. the CB-prep / status-drift
  class. (A host-only build still gets its normal byte-oracle + host-test gate; it just has no kernel race /
  IRQ / DMA surface for the AAG to audit.)
- Workers MAY self-run a lighter AAG before RETURN to arrive clean (the close-gate "catch upstream"
  philosophy), but the **authoritative** pass is the PM's at merge — the PM holds the Agent/Workflow tooling.

## The mechanism — a skeptical panel, prompted to refute
The PM spawns **N independent agents** (Agent/Workflow `parallel`/`pipeline`), each assigned a distinct
failure-lens and **instructed to REFUTE correctness, default-to-suspicious** — not to confirm it. Each agent
returns a structured verdict: `{lens, finding|none, severity, confidence, evidence/repro, what_I_checked}`.

**The four lenses (the named bug classes) + the completeness lens:**
1. **Concurrency / dispatcher hand-off** — data races, lock-ordering, the scheduler/IPC rendezvous, atomics
   ordering (SeqCst/Acquire-Release), per-CPU state, preemption windows. (Watch for "fix" that *masks* a race,
   e.g. disabling preemption.)
2. **Interrupt / EOI ordering** — IRQ/ISR paths, EOI sequencing, spurious/nested interrupts, and the **I-14
   fault-containment boundary** (assert no AI-governance lock is taken on any ISR path).
3. **ABI / format / on-wire drift** — syscall struct ABI, `.neom`/file formats, network frames, byte layouts;
   does an independent hand-laid byte oracle exist and is it *right*? (the byte-oracle rule, audited adversarially)
4. **Memory safety / `unsafe` / DMA** — every new `unsafe` justified; DMA buffers fenced (`dma_arena::fence`,
   I-14c) under VT-d; arena/bounds correctness.
5. **Completeness ("what did the gate NOT cover?")** — the green QEMU gate proves *what*, exactly? Name the
   paths the fixtures/`-cpu` choices/device-less QEMU **cannot** exercise (the CB-1 device-less broadcast-RX
   T-046/T-047 case; the Stage-B fixture-not-real-brain case). These become explicit bare-metal-only risks,
   not silently-assumed-covered.

## The verdict rule
- A finding **bounces the branch** only if it is **confirmed real** — it survives an adversarial
  re-verification (a second agent prompted to *refute the finding* cannot). This is the perspective-diverse /
  adversarial-verify pattern: kill on a survived finding, not on a first-pass suspicion.
- **Uncertain / low-confidence findings are recorded as RETURN notes, NOT merge-blockers.** The gate must not
  cry wolf — a gate that blocks on speculation gets ignored, which is a fired **KC-5** (the discipline failing
  silently).
- **"No findings" is not free:** each lens must state *what it checked*. A lens that returns `none` without
  saying what it inspected is a non-audit and is re-run. (No merge-theater — a rubber-stamp AAG is worse than
  none, because it launders unreviewed code as "adversarially cleared.")

## Scaling N to the risk surface
- **Small / localized kernel touch** → the 4 lenses, single-pass (+ completeness).
- **Load-bearing build** (new scheduler/IPC path, new driver, new syscall/ABI, new `unsafe`, a connectivity
  DMA path) → the 4 lenses **+ adversarial re-verify on every finding** + a second concurrency agent (the
  highest-yield lens). Match the depth to the blast radius; don't manufacture a panel for a one-line fix.

## How it composes (replaces nothing)
The AAG is the **third leg of PM verify-before-merge**, alongside:
1. `scripts/pm-merge-check.sh` — ff-safety / stale-base backstop.
2. PM's own `git show --stat` inspection + independent CI-run verification (push + 4-shard QEMU on the exact SHA).
3. **AAG** — adversarial correctness audit (this doc).

It **does not replace** the worker close-gate (`scripts/worker-close-gate.sh`), the QEMU gate, the byte-oracle,
or the kill-switch — it complements them by catching the logic/ordering/coverage wrongness those structurally
miss. A confirmed AAG finding bounces to the worker with the **specific repro** (a red-team finding, not a
vague "looks risky"), exactly like Cerebro's CB-1 inject-vs-real-lease catch.

## Record
The PM's merge report names the AAG outcome: lenses run, N, any confirmed findings (→ bounce) and uncertain
notes (→ carried to the build's ledger entry / bare-metal risk list). A clean AAG is stated as
"AAG: 4 lenses + completeness, no surviving findings" — never a silent pass.
