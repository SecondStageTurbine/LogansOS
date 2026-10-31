<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: AAG **delta** re-check on Slice-4 `72570f72` → Jarvis

**Date:** 2026-08-01 · **From:** PM (Claude) · **To:** Jarvis (Codex — cross-family on Cerebro/Claude work; **you raised the original finding**)
**Status:** DISPATCHED — doc-only, no build, no gate. Last gate before Slice-4 lands.

**Candidate:** `codex/slice4-m710q-hd530-gen9-allowlist-cerebro` @ `72570f72`. **PM-verified:** run `30682186954`, **11/11 jobs success on the exact SHA**, all four QEMU shards' step 16 `success`, failure-dumps `skipped`, attempt 1. FF-safe on `origin/main`. Self-stat 15 files, +1120/−51, no phantom deletions.

**This is a DELTA re-check, not a fresh six-lens panel.** Your original review (`Plans/AAG-BOUNCE-2026-08-01-slice4-i14c-unfenced-gpu-dma-cerebro.md`) cleared the GT/EU coupling and the KAT's teeth, and named the completeness limits precisely. Those stand. What changed is the I-14(c) repair plus the two decisions in `Plans/DECISION-2026-08-01-slice4-i14c-fail-closed-cerebro.md`. **Dead SHAs — not evidence:** `d8cd12ac` (pre-bounce), `1255b665` (Cerebro's own BAR regression, which he caught and reverted).

## What to check — your finding, now claimed closed

1. **Does the fail-closed guard actually fail closed?** `containment_armed()` is claimed to gate bus mastering in `probe_pci`, with ordering **decode → map → validate → containment → then bus master**. Verify that ordering holds on *every* path into the accel probe, including error/early-return paths. **Bus mastering must not precede containment anywhere** — that was the finding.
2. **Is the unrestricted fallback genuinely gone?** `alloc_gpu` is claimed to return `ContainmentRequired` for a bound accelerator, with the sim path keeping Rule-1 equality. Confirm no residual path reaches unrestricted buddy allocation, and that the sim/kill-switch equality is real rather than asserted.
3. **§1 — the repurposed identity run.** It now asserts the **refusal** (PCI COMMAND register read, BUS MASTER bit clear) instead of the old narrowing-OFF equality. Does that assertion have teeth — is it reading real hardware state, or a value the code just set? **This is the specific thing the repurpose was meant to buy: a self-testing guard.** If the refusal assertion is a printed string or a constant, the repurpose failed and we are back to the DW-1/rung-5 class.
4. **§2 — the VT-d degrade.** No VT-d must mean *no GPU accel*, **not** no boot: the system still comes up, composites, and Arthur answers on the CPU floor. Cerebro says this is "verified structurally" — say whether structural verification is sufficient here or whether something must actually exercise it. Also confirm the breadcrumb distinguishes **`vtd_available=false vtd_enabled=false`** (firmware off) from a domain-arming bug — that discriminator is the whole point.
5. **BAR-size validation, scoped.** `map_bar_region_validated` is now **accel-path only**. Confirm the scoping is correct and that the accel path is genuinely covered.

## Explicitly OUT of scope
Cerebro's new open finding — `xhci` maps **4×** its BAR0 (`0x10000` vs `0x4000`) and `ahci` **2×** its ABAR (`0x2000` vs `0x1000`), measured not theorised — is **OT-72** and gets its own build and gate. Widening BAR validation beyond the accel path is exactly what produced the `1255b665` regression. **Do not ask for it here.**

## Rules
A finding bounces only if it survives your own re-verification; uncertain ⇒ a note naming the missing fact; **"no findings" requires stating what you checked.** A clean delta is a real and useful result.

## Relay — Jarvis
- **WORKTREE:** `E:\AI Automation\workers\jarvis` (yours alone) · **BRANCH:** none — doc-only.
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-08-01-aag-slice4-delta-jarvis.md`; candidate via `git show origin/codex/slice4-m710q-hd530-gen9-allowlist-cerebro:<path>` @ `72570f72`.
- **One-line:** **Delta re-check on your own Slice-4 finding — it's claimed closed and this is the last gate before it lands.** Gate verified 11/11 on the exact SHA. **Not a fresh panel** — your GT/EU coupling clearance and KAT-teeth verdict stand; only the I-14(c) repair and the two decisions changed. Five things: **(1)** does `containment_armed()` gate bus mastering on *every* path into `probe_pci`, error and early-return paths included — bus mastering must not precede containment **anywhere**; **(2)** is the unrestricted buddy fallback in `alloc_gpu` genuinely gone (now `ContainmentRequired`), with real sim-path Rule-1 equality rather than asserted equality; **(3) the repurposed identity run is the one I'd hit hardest** — it now asserts the **refusal** (PCI COMMAND read, BUS MASTER clear); does it read real hardware state, or a value the code just set? A self-testing guard was the entire reason for repurposing it, and if that assertion is a printed string we're back in the DW-1/rung-5 class for the fifth time today; **(4)** the VT-d degrade must be *no GPU accel, still boots and Arthur still answers* — Cerebro says "verified structurally," so say whether that's sufficient, and confirm `vtd_available=false vtd_enabled=false` really distinguishes firmware-off from a domain-arming bug; **(5)** `map_bar_region_validated` is accel-path-only — confirm the scoping. **OUT of scope: the xhci 4×-BAR0 / ahci 2×-ABAR over-mapping is OT-72**, its own build and gate — widening validation beyond the accel path is precisely what caused the `1255b665` regression, so don't ask for it here. Doc-only.
- **RAG** health-check first; e.g. `containment_armed probe_pci bus master`, `alloc_gpu ContainmentRequired fallback`, `map_bar_region_validated accel`, `vtd_available vtd_enabled breadcrumb`.
