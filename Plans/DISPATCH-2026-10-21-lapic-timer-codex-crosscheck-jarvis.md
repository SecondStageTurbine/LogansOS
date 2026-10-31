<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family check of the LAPIC timer tick (CP-69) → Jarvis (Codex)

**Type:** adversarial review only. **Deliverable:** a pushed RETURN file.

**Why you.** This change replaces the kernel's tick source on the BSP: the timer interrupt's EOI, the
first tick after the scheduler starts, and the PIT/PIC fallback. That is the SMP / timer core, so
CLAUDE.md requires a different-family lens, and the author is Claude. Any Codex seat (Jarvis, Janus,
Chad) is valid. The change is stacked on the x2APIC PR (#82, CP-68) and uses its register path. If you
also have the CP-68 check, do this one second.

## What is under review

Branch `claude/lapic-timer`, stacked on `claude/x2apic-support`. The PM names the head SHA in the relay.
Read:

```
git fetch origin
git show origin/claude/lapic-timer:Plans/DISPATCH-2026-10-21-lapic-timer-codex-crosscheck-jarvis.md
git diff origin/claude/x2apic-support origin/claude/lapic-timer -- kernel/src/arch/x86_64/lapic_timer.rs kernel/src/arch/x86_64/lapic_timer_logic.rs kernel/src/arch/x86_64/apic.rs kernel/src/arch/x86_64/timer.rs kernel/src/sched/scheduler.rs
git show origin/claude/lapic-timer:docs/concurrency-pitfalls.md | grep -n "CP-69"
git show origin/main:Plans/NOTE-2026-10-21-zenbook-target-s2.md      # §4a, the trap (after #83 merges; else origin/claude/zenbook-target)
```

The change in one paragraph:
- With `lapic-timer` on, `sched::init_bsp` calls `lapic_timer::start(100)` before falling back to
  `pit::init` + `pic::unmask_irq(0)`.
- `start`:
  - finds the TSC rate from CPUID 0x15, then 0x16, then a polled PIT channel 2 that times out when the PIT
    is gated;
  - plans TSC-deadline (CPUID.01H:ECX[24]), else periodic (crystal from 0x15, or measured against the TSC
    over 10 ms);
  - arms the plan, then waits up to five periods (with IF=1) for `TICKS` to move. If nothing arrives it
    disarms (LVT masked, count 0, deadline 0) and demotes: deadline, then periodic, then the PIT.
- The tick fires the PIT's old vector, 32, through the unchanged `timer_isr_entry`.
- `lapic_timer::eoi()` replaces `pic::end_of_interrupt(0)`. It reads the LAPIC's ISR bit for vector 32:
  - set: it re-arms the deadline (TSC-deadline mode), then EOIs the LAPIC;
  - clear: an ExtINT PIT tick, so it EOIs the PIC.
- Feature off is today's PIT/PIC code. The product and bundle images do not turn the feature on yet; that
  is the PM's call after this check.

## Pitfall-bank consultation (the author's; refute it)

| Pitfall | Touch | Claim |
|---|---|---|
| CP-2/CP-3 (IF-off spin on a resource this CPU must service) | introduces a spin | The first-tick wait spins with **IF=1** on the BSP only, and is bounded (five periods, or ~1 s at an unknown TSC rate). With IF=0 it does not wait and logs `unverified`. |
| CP-15/CP-21 (EOI-only reschedule vs a busy CPU) | N/A | APs still have no timer; only the BSP's tick source changes, and its preemption path (`sched::on_tick`) is unchanged. |
| CP-22/CP-23 (`sti; hlt`; MWAIT) | guards | The LAPIC timer keeps counting in C1 (`hlt`). We have no MWAIT idle. Deeper C-states without ARAT would stop it, which is a future concern for an MWAIT idle. |
| CP-27 (IPI to an AP in init) | N/A | BSP only; no new IPI. |
| CP-64 (long IF=0 vs tick delivery) | unchanged | Missed ticks coalesce as they did with the PIT. In TSC-deadline mode the re-arm is `now + period`, so a long IF=0 section delays the next tick rather than bursting. |
| CP-68 (x2APIC register path) | uses it | Every timer register goes through `apic::read_reg`/`write_reg`, which choose MMIO or MSR by the resolved mode. `IA32_TSC_DEADLINE` is a plain MSR in both modes. |
| CP-69 (new) | guards | First-tick proof plus demotion; the EOI follows the ISR bit. |

## Refute it

1. **EOI correctness.**
   - Is reading ISR bit 32 a sound way to tell a LAPIC tick from an ExtINT PIT tick, in both APIC modes?
     Check the SDM on ExtINT and the ISR.
   - Can vector 32 be in service for any other reason? Consider a nested interrupt, a spurious interrupt,
     or the test-only one-shot timer.
   - What happens if a LAPIC tick and a PIT tick are both pending during a demotion?
2. **Disarm and demotion races.**
   - A tick can land between arm and verify, or between the budget expiring and `disarm`. Does `TICKS`
     ever move without the mode being alive?
   - Can the deadline MSR fire after the LVT is masked, or after the switch from deadline to periodic
     (SDM: a mode change disarms the deadline)?
   - Is the `mfence` between the LVT write and the first `IA32_TSC_DEADLINE` write the right SDM fence?
3. **Calibration.**
   - Check `tsc_hz_from_leaf15`/`_leaf16` against the SDM, and the KATs that pin them.
   - The PIT channel-2 poll: is the port 0x61/0x43/0x42 sequence right? Does its timeout really bound it
     on a gated PIT?
   - Is "periodic counts at the 0x15 crystal" right for the LAPIC timer with divide-by-1?
4. **The first-tick wait.** It runs inside `init_bsp`, with IF=1 and the scheduler installed. Can a tick in
   that window preempt into something that is not ready yet? Is the five-period budget (`first_tick_budget`)
   enough on real hardware?
5. **Tests.**
   - `lapic_timer_demote` arms a masked first mode. Does that really exercise the demotion path that a
     gated or silent timer would take?
   - The tick-count check is 2..=200 per half second, because TCG's TSC-to-virtual-clock ratio moves with
     host load (we saw 4 to 50). Is that too weak? What would a hermetic rate check look like?
   - The `no-verify` control "fails" by timing out. Is that an acceptable control?
   - TSC-deadline mode cannot run under TCG. What, short of silicon, could test it? Is there anything in
     the code that only the deadline path executes and that no test reaches?
6. **Kill-switch.** With the feature off, is the timer path byte-for-byte today's (timer.rs, scheduler.rs,
   apic.rs)?
7. Anything else.

A finding counts only if it survives your own re-verification; uncertain items are notes. "No findings"
must say what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/lapic-timer-crosscheck-jarvis   (git fetch origin && git checkout -B codex/lapic-timer-crosscheck-jarvis origin/claude/lapic-timer)
```

Optional runs, at `NEO_SMP=4`:
- `cargo xtask neo-test lapic_timer_tick` should PASS with `mode=periodic demotions=0 apic=xAPIC`.
- `cargo xtask neo-test lapic_timer_demote` should PASS with `mode=pit demotions=1`.
- `lapic_timer_tick_x2apic` needs QEMU ≥ 9 and should PASS with `apic=x2APIC`.
- Controls:
  - `NEO_TEST_EXTRA_FEATURES=neo-test-lapic-timer-control` on `lapic_timer_tick` should FAIL
    `timer_not_lapic`;
  - `neo-test-lapic-timer-no-verify` on `lapic_timer_demote` should time out.
- `cargo test -p lapic-timer-logic-tests` runs the KATs.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?"
questions; grep still wins for exact symbols. Health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`, expecting `{"status":"healthy",...}`. If it is down or
degraded, the PL manages it: note that in your RETURN and continue with direct reads.
Optional self-serve start: `powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='timer interrupt end of interrupt PIC versus local APIC'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-21-lapic-timer-codex-crosscheck-jarvis.md` to
`codex/lapic-timer-crosscheck-jarvis` and push it. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- each finding with severity, file:line and a repro;
- what you checked for each numbered item.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete
the worktree.
