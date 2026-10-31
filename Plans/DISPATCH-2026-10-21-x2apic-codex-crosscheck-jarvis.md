<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family check of x2APIC support (CP-68) → Jarvis (Codex)

**Type:** adversarial review only. **Deliverable:** a pushed RETURN file.

**Why you.** CP-68 changes how every IPI, EOI and MSI is delivered: the local-APIC register path, the
TLB-shootdown and reschedule IPIs, and AP bring-up. That is the SMP / interrupt core. CLAUDE.md requires a
different-family lens on it, and the author is Claude. Any Codex seat (Jarvis, Janus, Chad) is valid.

## What is under review

Branch `claude/x2apic-support`, based on `origin/main` `e42d2483`; the PM names the head SHA in the relay.
Read:

```
git fetch origin
git show origin/claude/x2apic-support:Plans/DISPATCH-2026-10-21-x2apic-codex-crosscheck-jarvis.md
git show origin/claude/x2apic-support:Plans/NOTE-2026-10-21-x2apic-s2.md      # failure bank + audit
git diff origin/main origin/claude/x2apic-support -- kernel/src/arch/x86_64/apic.rs kernel/src/arch/x86_64/apic_logic.rs kernel/src/arch/x86_64/smp.rs kernel/src/drivers/pci.rs
git show origin/claude/x2apic-support:docs/concurrency-pitfalls.md | grep -n "CP-68"
```

The change in one paragraph:
- The Limine MP request now asks for x2APIC. The mode is resolved from the BSP's `IA32_APIC_BASE` and
  cross-checked against Limine's response flag.
- In x2APIC mode:
  - registers are MSR `0x800 + (offset >> 4)`;
  - the ICR is one 64-bit WRMSR to `0x830` with the destination in bits 63:32, preceded by
    `mfence; lfence`;
  - EOI is MSR `0x80B`, written 0;
  - `IA32_APIC_BASE` is never written;
  - an AP whose MSR disagrees with the BSP's mode halts.
- The xAPIC path is today's code.
- MSI/MSI-X addresses target the BSP's registered APIC ID, and are refused (the device is left without
  MSI) when that ID exceeds 255.
- The `xapic-only` feature restores the old request.

## Refute it

1. **SDM mode rules.** Is there any path that writes `IA32_APIC_BASE` in x2APIC mode, or that issues an
   xAPIC MMIO access after the mode is x2APIC? Check every caller of `read_reg`/`write_reg`/`reg_ptr`,
   including test-only ones (the LAPIC one-shot timer). Can `mode()` be read before `init_bsp` stores it,
   for example by an early MSI EOI on the BSP, or by a fault path? If so, which way does it go wrong?
2. **ICR encoding and ordering.**
   - Check the 64-bit ICR value against the SDM: vector, delivery mode fixed, physical destination,
     level-assert bit, destination in 63:32.
   - Is `mfence; lfence` before the WRMSR sufficient and necessary? Consider the shootdown payload
     publication (`publish_payload_to_targets`, then `send_tlb_shootdown_ipi`) and the reschedule path.
   - Is there any other non-serializing x2APIC MSR write whose ordering matters (EOI)?
3. **The 32-bit ID audit.**
   - Find any place an APIC ID is narrowed: `register_lapic_id`, `lapic_id_for_cpu`, `bring_up_aps`, the
     ICR, MSI, anything in per-CPU state or tracing.
   - Is the xAPIC 8-bit mask safe? Can an ID above 255 reach the xAPIC path, say from a MADT x2APIC entry
     with `xapic-only`?
4. **The MSI path.**
   - `msi_address_for_bsp` uses the BSP's registered ID, or 0 before registration. Can a device be
     programmed before `init_bsp`, and is 0 then right?
   - Is "refuse above 255" the right failure for each driver (NVMe returns an error; xHCI and AHCI fall
     back)?
   - Should MSI target the BSP at all, or the CPU that owns the device's IRQ work? Today's behaviour
     targets APIC 0.
5. **AP bring-up.** Limine enables x2APIC on every CPU it starts. Does `init_traps_ap` touch the APIC
   before `init_ap`'s mode check? Is halting a mismatched AP correct with respect to the AP ready-barrier
   and the shootdown-ready mask (CP-8, CP-27)?
6. **Mode resolution.** The MSR wins over Limine's report. Is there a case where that is wrong, such as
   firmware with x2APIC on the BSP only?
7. **The tests.**
   - `x2apic_smp_ipi` runs on `-cpu max,+x2apic` (it needs QEMU ≥ 9; CI builds 10.2.3). Does it really
     exercise the x2APIC ICR, EOI and SVR paths?
   - Is anything vacuous? For example, could a reschedule IPI's receipt be counted without the x2APIC
     path running?
   - The control (`xapic-only`) must FAIL with `apic_mode_not_x2apic`.
   - The host KATs pin encodings to SDM values: check those values.
   - Under CI's QEMU 10, plain `-cpu max` also comes up in x2APIC, so every `max` scenario now runs
     x2APIC. Is any of them sensitive to that?
8. **Kill-switch.** Is the xAPIC branch of every function byte-for-byte the pre-CP-68 code? With
   `xapic-only`, is the request exactly the old one?
9. Anything else, including the laptop next-layer list in the NOTE §4.

A finding counts only if it survives your own re-verification; uncertain items are notes. "No findings"
must say what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/x2apic-crosscheck-jarvis   (git fetch origin && git checkout -B codex/x2apic-crosscheck-jarvis origin/claude/x2apic-support)
```

Optional runs, at `NEO_SMP=4`. **They need QEMU ≥ 9**; QEMU 8.x TCG has no x2APIC, so the scenario fails
there with `apic_mode_not_x2apic`.
- `cargo xtask neo-test x2apic_smp_ipi` should PASS with `reschedule_ipis=48 shootdown_ipis=48
  timeouts=0`.
- With `NEO_TEST_EXTRA_FEATURES=xapic-only` it should FAIL with `reason=apic_mode_not_x2apic
  mode=xAPIC`.
- `cargo test -p apic-logic-tests` runs the encoding KATs.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?"
questions; grep still wins for exact symbols. Health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`, expecting `{"status":"healthy",...}`. If it is down or
degraded, the PL manages it: note that in your RETURN and continue with direct reads.
Optional self-serve start: `powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='local APIC ICR write send IPI to target cpu'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-21-x2apic-codex-crosscheck-jarvis.md` to `codex/x2apic-crosscheck-jarvis`
and push it. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- each finding with severity, file:line and a repro;
- what you checked for each numbered item.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete
the worktree.
