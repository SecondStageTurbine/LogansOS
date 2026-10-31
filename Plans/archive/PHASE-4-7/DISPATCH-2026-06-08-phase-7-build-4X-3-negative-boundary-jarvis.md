<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7 BUILD-4X-3 — the cross-tenant negative test (the silicon-enforced device boundary) → Jarvis

**Date:** 2026-06-08
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `389aee6`: `codex/phase-7-build-4X-3-negative-boundary-jarvis`)
**Status:** DISPATCHED. **Blind-hardware (OptiPlex). This build closes the device-boundary governance line (§9.8) — the fourth and last.**
**Builds on:** BUILD-4X-1/4X-2 (`7626f62`/`e78cc79`, execlist context + per-context PPGTT proven, positive boundary proven) + BUILD-3 (`c1fe512`, capability RO sharing at the op boundary — this build adds its *device-side* enforcement). ADR-032, ANSPEC §9.7/§9.8.

## 0. What this proves (the thesis, instantiated)
4X-2 proved one tenant's data resolves through *its own* per-context PPGTT. **4X-3 proves the boundary holds against an adversary:** a *second* tenant's device-side (EU A64) access to the *first* tenant's tensor must **page-fault**, and an RO-shared tensor must **fault on write**. On a PASS, the kernel's authority is enforced *by the silicon's page tables*, not by an op-boundary check a tenant could race — and the governance thesis crosses from "governs by API convention" to "governs by silicon-enforced boundary." This is the line the whole architecture analysis converged on.

## 1. The validation is a NEGATIVE test — and the worst failure LOOKS LIKE SUCCESS
Every prior build proved *authorized* access *succeeds*. This one proves *un*authorized access **fails** — a different, more dangerous shape. **The catastrophic failure is the silent isolation hole:** tenant B reads tenant A's data and the run *looks fine* (no crash, plausible bytes). So:
- **The gate is the FAULT, not the absence of a crash.** A passing 4X-3 is one where the cross-tenant access *faulted*, the RW-on-RO *faulted*, and we proved it's the *right* fault.
- **A positive control is mandatory:** B reading its *own* tensor must *succeed*. Without it, a fault could mean "isolation works" OR "B's own page tables are broken" — indistinguishable. The own-access-succeeds + cross-access-faults pair is the proof.

## 2. ISCs
- **ISC-1 — Two tenants, two per-context PMLs.** Set up two execlist contexts (tenant A, tenant B), each with its own 48-bit PML4 (reuse 4X-1/4X-2 machinery). A's tensors mapped only in A's PML4; B's only in B's. Nothing cross-mapped. The two contexts run temporally (one at a time — no concurrency needed; the boundary is *spatial*, enforced by the separate page trees, independent of execution order).
- **ISC-2 — The cross-tenant negative test (the gate).** Tenant B's batch issues an A64 access to **tenant A's tensor VA** (an address valid in A's PML4, absent from B's). It **must fault**. Plus the **positive control**: B accessing its **own** tensor must **succeed** (proves B's context/PML4 is sound, so the fault is *isolation*, not breakage).
- **ISC-3 — The RW-on-RO device-side test (closes the BUILD-3 §9.7 seam).** Map a tensor **read-only** (PML4/PT entry RW-bit clear) in a context; a **write** to it must **fault**; a **read** must **succeed**. This is the device-side (page-table) enforcement of the RO that BUILD-3 enforced at the op boundary — so a shared-RO tensor (BUILD-3) is now hardware-unwritable by the grantee, not just policy-unwritable.
- **ISC-4 — The gate captures (the negative-test discriminators).** On each expected fault, capture: `select` bit (**must be PPGTT**), fault **address** (must be A's tensor VA / the RO page), and the **active PML4 root** (must be B's / the writing context's — proving the fault fired *in the right context*). Distinguish:
  - cross-access **faults**, select=PPGTT, addr=A's VA, B's PML4 active → ✅ isolation enforced
  - cross-access **succeeds** (no fault) → ❌ **silent hole** (PMLs not separate / a shared mapping) — the failure the gate exists to catch
  - B's **own** access faults → ❌ B's PML4 broken (not isolation) — the positive control catches this
  - RW-on-RO **faults**, RO-read succeeds → ✅ RO enforced at the device
- **ISC-5 — Byte oracle.** Hand-laid RO PTE (RW-bit clear) + the two-context PML4 layouts (A's VA present in A's PML4 / absent in B's), asserted independent of the construction code. Reuse the RW/RO PTE oracle.
- **ISC-6 — Standards.** Default-off; SPDX; SAFETY/PRM+RESEARCH citations; `cargo fmt`; host suite (the RO + two-context PML4 oracles + 4X-1/4X-2/4-1 + accounting/admission/sharing regressions) green; `shard-coverage`; `clippy -D warnings` (CI/Linux); builds on+off. ADR-032 gets a BUILD-4X-3 section. **On PASS: §9.8 spatial-isolation flips to ENFORCEMENT-PROVEN; the fourth governance line is real; BUILD-3's §9.7 device-resident-shared-tensor seam is closed.**

## 3. Failure-mode + discriminator section (REQUIRED — blind-hardware; the negative-test shape)
Captures: select + fault addr + active PML4 root + CSB + RING_HEAD.
| # | Mode | Likely cause | Discriminator |
|---|---|---|---|
| 1 | **Cross-access succeeds (no fault)** — the silent hole | the two PMLs share a mapping, or B's PML4 maps A's frame, or both contexts use one PML4 | the cross-access MUST fault; if it doesn't, dump both PML4s and confirm A's VA is absent from B's |
| 2 | **B's OWN access faults** (positive control fails) | B's PML4 doesn't map B's own tensor (construction) | fault addr = B's own VA → fix B's PML4 before trusting the negative result |
| 3 | **Cross-access faults but in the WRONG context** | the PML4 root wasn't swapped between A's and B's submission | the active-PML4-root capture: the fault must fire with B's root loaded |
| 4 | **RW-on-RO doesn't fault** | the RO tensor's PTE was written RW | the PTE RW-bit + the write-fault; cross-check the RO PTE oracle |
| 5 | **cross-access faults (A's VA, B's context, select=PPGTT) + B's-own succeeds + RW-on-RO faults + RO-read succeeds** | — | ✅ the device boundary is silicon-enforced — 4X-3 done, §9.8 line closed |

## 4. Gate (two-stage)
- **Stage A (you):** builds (on+off); the RO + two-context PML4 oracles + regressions green; clippy; fmt; shard-coverage.
- **Stage B (PL, OptiPlex):** **B's access to A's tensor faults** (select=PPGTT, A's VA, B's PML4 active); **B's own access succeeds**; **RW-on-RO faults, RO-read succeeds**. Report the fault addr + select + active root for each, + the positive-control results. *(SMP-flake known hazard: cold power-cycle + retry if it wedges pre-dispatch.)*

## 5. Scope guard
The cross-tenant negative + RW/RO device-side enforcement only. **Do NOT** add concurrent execution (the boundary is spatial — two temporal contexts suffice), new ops, preemption, or multi-device placement. **Do NOT** change 4X-1/4X-2 submission or PPGTT machinery (reuse — two contexts of the proven one). Prove the denial, and prove it's the *right* denial.

## 6. Merge + the close (PM, on Stage-B PASS)
Close-gate: `git fetch origin && git rebase origin/main`, re-run Stage-A, push. PM `pm-merge-check` → `--ff-only` → `main`. Then the **device-boundary close**: §9.8 → enforcement-proven, ADR-032 + a `Plans/CLOSE-REPORT` for the 4X arc, and the bible Article 13 records the fourth governance line real — **the kernel governs AI between adversarial tenants, enforced by the accelerator's page tables.** All four governance lines (measure / arbitrate / own / isolate) then real on silicon; Phase 7's remaining work (policy-driven placement, the broader scheduler) builds on a complete enforcement substrate.
