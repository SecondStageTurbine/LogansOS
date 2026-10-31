# RETURN: x2APIC fix re-check (PR #82)

**Verdict: CLOSED for P2 and note 3. No new actionable findings in the fix commit.**

Reviewed head `b5b30d13d4e8c73b5f1f733fcd6c67df6a3327ad`, exclusively the fix diff `0f31c474..b5b30d13`, on `codex/x2apic-recheck-jarvis`. Read the original BOUNCE from `origin/codex/x2apic-crosscheck-jarvis:Plans/RETURN-2026-10-21-x2apic-codex-crosscheck-jarvis.md`, the changed implementation and tests, and the startup/readiness call sites.

## P2: CLOSED

`init_local_apic` reads this CPU's own `IA32_APIC_BASE` and calls `apic_logic::ap_mode_check` before branching to either initialization path (`kernel/src/arch/x86_64/apic.rs:107`). The check precedes the sole `ApicBase::write` at line 138. The x2APIC path still performs no APIC-base write.

- An xAPIC kernel rejects any CPU with `X2APIC_ENABLE` set, before the code can clear that bit or write the MSR.
- An x2APIC kernel proceeds only when both `LAPIC_ENABLE` and `X2APIC_ENABLE` are set; an xAPIC or disabled AP is rejected.
- A disabled AP with both bits clear can still proceed through the xAPIC initialization path.

Both mismatch directions log the CPU, LAPIC ID, flags, and driven mode, then call the diverging `halt_forever` (`apic.rs:117`; implementation at `arch/x86_64/mod.rs:85`, `cli` followed by an endless `hlt` loop).

The call chain is `ap_entry` -> `init_traps_ap` -> `apic::init_ap` -> `init_local_apic`. Only after this returns can the AP reach its ready log and `run_ap_bootstrap` (`smp.rs:268-278`). The latter signals `APS_READY` at `sched/scheduler.rs:422`, enables interrupts, then marks shootdown-ready at line 426. A mismatched AP reaches neither readiness publication. Moving the former x2APIC-only check to the shared entry therefore closes the asymmetric P2 without removing the existing direction's protection.

## Note 3: CLOSED

`apic_logic::xapic_icr` now returns `None` for destinations above `0xff`, and `Some((dest << 24, low))` for representable IDs (`apic_logic.rs:29`). `send_fixed_ipi`, used by both reschedule and TLB-shootdown sends, handles `None` by recording/logging the refusal and returning before either ICR MMIO write (`apic.rs:231-241`). It cannot mask a high ID into another CPU's destination.

`bring_up_aps` checks mode-dependent addressability at `smp.rs:152`. Rejection occurs before LAPIC-ID registration, `cpu.bootstrap`, and the `started` increment, so a skipped AP neither starts nor adds an expected ready-barrier count. x2APIC retains full-width destinations; representable xAPIC destinations retain their previous ICR values.

## Attempts to refute the fix

Checked for a bypass around the moved mode guard, readiness before the guard, a remaining tuple caller of the changed `xapic_icr` API, an ICR write before destination rejection, and a ready-barrier count including skipped APs. None was found.

Also checked the new skip's interaction with CPU numbering: IDs can have holes because `next_cpu_id` advances before the skip, and `online_cpu_count` still reports the bootloader count. The threaded-matmul consumer can publish to such an absent slot, but its existing bounded grace and CAS-based BSP steal-back execute unclaimed work (`ai/par_matmul.rs:435-458`). That does not establish a new correctness failure in this commit. No implementation change is requested by this re-check.

## Validation and limits

- `cargo test -p apic-logic-tests`: PASS, 8 unit tests, 0 failures; doc tests PASS (0 tests). The suite imports the actual kernel helper module and covers both mode mismatch directions, disabled APIC handling, destination rejection, valid xAPIC encodings, and full-width x2APIC encoding.
- `git diff --check 0f31c474 b5b30d13`: PASS.
- Startup ordering and refusal wiring were reviewed directly in source. No kernel build, QEMU boot, injected mixed-mode run, or hardware run was performed. Host tests validate helper behavior, not hardware execution of the halt paths.

Only this return note is included in the review commit; pre-existing worktree edits are excluded.
