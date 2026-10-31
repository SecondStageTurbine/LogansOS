<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Fix missing ring-3 `swapgs` on device-IRQ/IPI entry (page fault in `current_cpu_id`) → Jarvis

**Date:** 2026-06-04
**From:** PM/Cerebro
**To:** Jarvis (worker tree, feature branch — continue on `codex/phase-5-baremetal-inference-jarvis` or a fresh one)
**Status:** DISPATCHED — kernel fix authorized
**Unblocks:** Phase 5 Gate 2 (`Plans/PHASE-5-CLOSE-PLAN-2026-06-04.md`); blocker `Plans/BLOCKER-2026-06-04-phase-5-inference-baremetal-smp4-page-fault.md`.
**Requires:** QEMU + Docker, SMP (`NEO_SMP=4`), `--bare-metal-mode`. Local reproducer exists — fix it there, do **not** run the OptiPlex until it's green.

---

## The bug (root cause confirmed by Cerebro — not a hypothesis)

`infer_transformer_forward --bare-metal-mode` at `NEO_SMP=4` page-faults `cr2=0x18`, rip in `per_cpu::current_cpu_id` (`per_cpu.rs:117`). That function is `mov eax, gs:[24]` — **`cr2=0x18` = the `[24]` offset read with GS-base = 0** (the *user* GS value). An interrupt entered a kernel handler **without `swapgs`** while the CPU was in ring-3, then the handler touched per-CPU state.

**Why:** the device-IRQ/IPI vectors are registered with `set_handler_fn(...)` — plain `extern "x86-interrupt"` Rust handlers that **never execute `swapgs`** (`kernel/src/arch/x86_64/idt.rs:69-73`):
```
idt[TLB_SHOOTDOWN_IPI_VECTOR].set_handler_fn(tlb_shootdown_ipi);
idt[RESCHEDULE_IPI_VECTOR].set_handler_fn(reschedule_ipi);
idt[AHCI_MSI_VECTOR].set_handler_fn(ahci_msi_interrupt);
idt[NVME_MSI_VECTOR].set_handler_fn(nvme_msi_interrupt);   // <- confirmed trigger
idt[SPURIOUS_VECTOR].set_handler_fn(spurious_interrupt);
```
The **timer** is the *only* IRQ that does it right: it uses a custom asm stub `timer_isr_entry` (`idt.rs:67`, `set_handler_addr`) that branches on saved `CS.RPL` and `swapgs`-es on a ring-3 source (`timer.rs:70-76`, and swaps back on the ring-3 return path, `:156-160`). The MSI/IPI handlers have **no such prologue**.

**The trigger** (why it only shows now): the bare-metal log-sink writes to **NVMe**, generating completion MSIs *during the inference run while neo-init is in ring-3*. At `NEO_SMP=4` one of those MSIs lands on an AP in userspace → `nvme_msi_interrupt` runs with user GS=0 → `nvme::handle_interrupt()` → `current_cpu_id()` → fault. Normal QEMU has no NVMe log-sink (no such MSIs during userspace); `SMP=1` dodges the ring-3-delivery window. That's the whole signature.

## What rework is required

### 1. Fix the confirmed vectors (the Gate-2 blocker)
Give `nvme_msi_interrupt` and `ahci_msi_interrupt` a **swapgs-aware entry**, mirroring the **proven** `timer_isr_entry` discipline:
- on entry, branch on saved `CS.RPL` (`test byte ptr [rsp+8], 3`); if ring-3 source, `swapgs` so GS-base = `&PER_CPU`;
- run the existing `handle_interrupt()` body;
- on the return path, if the source was ring-3, `swapgs` back before `iretq`.

Reuse the timer's pattern exactly — do **not** invent a new scheme. (These handlers don't need the timer's *stack-switch* dance, only the GS half; but follow the same CS.RPL branch + symmetric swap.)

### 2. Audit the siblings — do NOT knowingly ship the same latent bug
The IPI vectors (`tlb_shootdown_ipi`, `reschedule_ipi`) and `spurious_interrupt` use the **same** `set_handler_fn` with no swapgs. The TLB-shootdown handler in particular calls `per_cpu::current_per_cpu()` (→ `current_cpu_id`), and an IPI **can** arrive while a target AP is in ring-3 — i.e. the identical fault is latent there, just not triggered by the current test matrix. For **each** of these vectors, determine: can it arrive in ring-3, and does its handler touch per-CPU state? Fix every one that answers yes to both, with the same swapgs discipline.
- **If this audit balloons into a large refactor**, stop and bring the scope back to PM — fix the confirmed NVMe/AHCI path first (unblocks Gate 2) and we'll sequence the IPI hardening separately. Don't let it become an open-ended rewrite.

## ⚠️ `swapgs` is a footgun — get it exactly right
`swapgs` bugs (double-swap, asymmetric entry/exit, NMI-during-swap reentrancy) are classic and nasty. Hard rules:
- **Symmetric:** every ring-3-entry `swapgs` has exactly one matching ring-3-exit `swapgs`. Ring-0 source = no swap at all.
- **Branch on the *saved* CS, not current state** (the `[rsp+8]` RPL test, as the timer does).
- Mind NMI/`#DF` (they have their own IST handling) — don't introduce a window where an NMI sees the wrong GS. Match how the timer/IPI vectors are already configured for IST.
- The fault must not move: after the fix, an MSI in ring-3 runs `handle_interrupt` with **kernel** GS, and `current_cpu_id` returns the right id.

## Instrument / regression pin
- Add a test that exercises a **device completion (or IPI) delivered during ring-3 under SMP** and asserts no fault — so this can't silently regress. The `infer_transformer_forward --bare-metal-mode` SMP=4 run is the integration guard; a tighter targeted scenario (force an MSI/IPI while a user thread runs) is better if cheap.

## Close gate
- `git fetch origin && git rebase origin/main`; re-run the gate after.
- **The reproducer is green:** `infer_transformer_forward --bare-metal-mode` at `NEO_SMP=4` reaches the inference **PASS** via `NEO_LOG` (no `cr2=0x18`, no page fault). Run it several times — the original was deterministic, so a few clean runs suffice, but confirm it's not newly flaky.
- **No regression:** full `cargo neo-test` suite green at `NEO_SMP=4` (normal mode), and `--bare-metal-mode` SMP=1 still green.
- Host-checks green (`cargo fmt --all --check`, `cargo test --workspace --exclude neoplutarch-kernel`, `cargo clippy --workspace --exclude neoplutarch-kernel -- -D warnings`).
- SAFETY comments on the new asm/swapgs; the audit (#2) dispositioned (fixed, or escalated to PM with reasoning).
- **Fold in** the existing xtask `--bare-metal-mode` wiring from `b4f3724` (it's correct — don't redo it).
- Push to your **feature branch, not `main`**; PM merges via `pm-merge-check` + `--ff-only`. Close report with the before/after fault evidence and the vector audit.
- **Then** Gate 2's OptiPlex boot proceeds (the inference-on-metal run), and Phase 5 close (Gate 3) follows.

## Scope guard
This is a **kernel trap-entry GS-discipline fix** + the (already-done) xtask wiring. Do not touch inference op logic, the oracle, or the `.neom` format. One concern: correct `swapgs` on device-IRQ/IPI entry.
