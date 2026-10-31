<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Fix the flaky `tlb_shootdown_timeout` kernel panic in CI QEMU SMP → Jarvis

**Date:** 2026-06-02
**From:** PM/Cerebro
**To:** Jarvis (worker tree, feature branch — your choice, e.g. `prometheus/tlb-shootdown-timeout-fix`)
**Status:** ✅ CLOSED (2026-06-03) — fixed in `5a9c878` (re-poke + 50 ms bounded panic), CI green on `main`, and bare-metal smoke green on real hardware (OptiPlex, AHCI, 12 CPUs). See `Plans/CLOSE-REPORT-2026-06-03-tlb-shootdown-timeout-flake.md`.
**Priority:** High — this is the only thing keeping `main`'s QEMU job from being reliably green.
**Requires:** QEMU + Docker, SMP (`NEO_SMP=4`). This does **not** reproduce on host-checks; it is an in-guest SMP timing fault.

> **This supersedes the worker task in [`DISPATCH-2026-06-01-ci-sched-capacity-pressure-jarvis.md`](DISPATCH-2026-06-01-ci-sched-capacity-pressure-jarvis.md).** That dispatch's original framing (a `sched_ready_queue_capacity_pressure` *scheduler functional* failure) was wrong; its 2026-06-02 root-cause correction (committed to `main` as `8a28c88`) re-identified the failure as a **TLB-shootdown timeout panic** that is **not specific to any one scenario**. This dispatch is the focused follow-through on that corrected diagnosis. Close the sched-capacity dispatch as superseded.

---

## The failure — exact evidence

`main` CI run **#123** (head `2c09cb6`, the BUILD-4 merge), job "QEMU Integration Tests". Host-checks and SPDX were **green**; only the QEMU job failed. Serial logs captured this panic:

```
================================================
!!  KERNEL PANIC  !!
panicked at kernel/src/mm/tlb_shootdown.rs:236:13:
tlb_shootdown_timeout pending=1 targets=0x000000000000000e
================================================
NEO_TEST/1 event=FAIL name=<scenario> reason=kernel_panic
```

Decoded:

- **`targets=0x0e`** = bits 1,2,3 ⇒ the shootdown was broadcast to **CPUs 1, 2, 3** (BSP=0 correctly excluded).
- **`pending=1`** ⇒ exactly **one** of those three APs never decremented `payload.pending` before the sender's TSC deadline expired. Not all three — one straggler.
- **`reason=kernel_panic`** ⇒ the panic handler is what called `debug_exit(EXIT_FAILURE)`; this is *not* a scenario workload returning nonzero (the mistake the prior dispatch made decoding exit status 35).

**Crucially, this is cross-cutting, not scenario-specific.** In run #123 it tripped on `sys_inspect_running` (attempt 2) and `sys_wait_already_exited` (attempts 2 and 3) — **not** on `sched_ready_queue_capacity_pressure`, and **not** on any BUILD-4 transformer/transcendental scenario (those all passed). Whichever SMP scenario happens to issue a shootdown when the straggler AP is briefly unavailable is the one that dies. This is why it presents as a different "failing scenario" across runs and why it's ~50–60% flaky rather than deterministic.

**This is a pre-existing defect, independent of BUILD-4.** The BUILD-4 merge is clean (host-checks green, its own scenarios pass). Do not look for the cause in BUILD-4.

## Where it lives

`kernel/src/mm/tlb_shootdown.rs`. The relevant machinery:

- **`shootdown()` (line 158)** — taken under the global `SHOOTDOWN_LOCK` (an `IrqSafeMutex`, so the sender runs with **IRQs disabled** for the whole operation). Publishes the payload to each target's per-CPU slot, sends **one** IPI per target (`apic::send_tlb_shootdown_ipi`, lines 168–170), self-invalidates, then busy-waits.
- **`wait_for_acks_or_panic()` (line 227)** — busy-spins reading the TSC, returns when `pending == 0`, **panics at line 236** when `read_tsc() - start >= timeout_cycles` while `pending != 0`.
- **`handle_ipi()` (line 179)** — the target's IPI handler: swaps its slot to null, `invalidate_local`, then `payload.pending.fetch_sub(1, Release)`. This only runs when the target AP **actually takes the IPI** — i.e. interrupts enabled and not in an IRQ-masked window.
- **The budget: `SHOOTDOWN_TIMEOUT_MS = 1` (line 20).** `timeout_cycles()` = `tsc_cycles_per_ms() * 1`. **A one-millisecond deadline.**

## Leading hypotheses (instrument before you patch)

Rank by likelihood; confirm with measurement rather than guessing. The signature (one straggler, ~50–60% flaky, only under emulated SMP) points hard at a **marginal timeout under QEMU TCG**, not a logic error in the ACK protocol.

1. **The 1 ms TSC budget is far too tight under QEMU TCG (leading suspect).** TCG does not model TSC frequency faithfully, and — more importantly — the sender busy-spins (`core::hint::spin_loop()` = `PAUSE`, which does **not** yield a vCPU under TCG's round-robin). While the sender burns a full `tsc_cycles_per_ms` of TSC delta in a tight loop, the host may not schedule the *target* AP's vCPU thread long enough for it to take the IPI and ACK within that window. Classic TCG SMP busy-wait starvation: the very CPU we're waiting on can't run because we're spinning. One AP slipping past a 1 ms emulated budget ~half the time fits perfectly.
2. **IRQ-disabled windows on the target AP.** `handle_ipi` only runs when the target has interrupts enabled. If the straggler AP is momentarily in *any* IRQ-off critical section (another `IrqSafeMutex` hold, a scheduler critical region) when the IPI arrives, the ACK is deferred until IRQs re-enable — which, stacked on (1)'s tight budget, blows the deadline.
3. **Single IPI, no re-poke.** The sender sends the shootdown IPI **exactly once** and never re-sends (lines 168–170). If that IPI is ever coalesced or lost, the target never ACKs and the panic is guaranteed. Linux re-sends on a sub-timeout before declaring a CPU stuck; we have no retry at all.
4. **TSC-as-clock unreliability under TCG.** Even `tsc_cycles_per_ms()` (CPUID 0x15/0x16, lines 302–320) may return a frequency inconsistent with how TCG advances the guest TSC, making the nominal "1 ms" budget effectively microseconds of emulated progress.

## What I did NOT determine (do this first)

- **Why the one straggler AP is late.** Instrument it. There is already a `neo-test-instrumentation` feature with per-CPU shootdown counters (`shootdown_ipis_received`, etc., lines 357–395) — extend it to record, per target, the TSC at IPI **send** vs. at `handle_ipi` **receipt**, so you can measure the actual ACK-latency distribution under `NEO_SMP=4`. The shape of that distribution is the whole answer:
  - If the straggler eventually ACKs just past the 1 ms line (a fat tail, no true hang) → it's hypotheses 1/2/4: a budget/scheduling problem.
  - If the straggler *never* ACKs even given seconds → it's hypothesis 3 (lost IPI) or a genuine deadlock; bumping the timeout would only mask it.
- **Whether a re-poke fixes it.** Cheap experiment that discriminates 3 from 1/2: after a sub-timeout, re-send the IPI to any still-pending target before panicking. If that eliminates the flake, the original IPI was being lost/missed.

## The fix — principled, not a number-bump

⚠️ **Do not simply raise `SHOOTDOWN_TIMEOUT_MS` and call it done.** A blind constant bump with no understanding of *why* the AP is late is exactly the "relax the gate to make CI green" move the standing close-gate warns against (cf. the BUILD-4 bounce). It would also mask hypothesis 3 (a genuinely lost IPI) or any real deadlock behind a longer spin.

The robust design (Linux's model) and the likely correct deliverable:

- A **generous** deadline with measured margin over the observed worst-case ACK latency under `NEO_SMP=4` TCG (your instrumentation gives you the number — pick a bound with comfortable headroom, not one that's marginal).
- **Plus an intermediate IPI re-send**: if a target hasn't ACKed within a sub-window, re-poke it before the final deadline; only panic after a target has been re-poked and is *still* silent well past any plausible IRQ-disabled window. That panic then means a genuinely wedged CPU, which is what a panic should mean.
- Consider whether the sender's tight `PAUSE` spin should do anything to avoid starving the target vCPU under TCG (the re-send naturally breaks the pure spin; that may be sufficient).

Land the change with **evidence**: the ACK-latency distribution before/after, and the chosen budget's margin over it.

## Repro (Docker works in-container; daemon must be running)

```bash
# full suite as CI runs it:
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test

# because it's ~50–60% flaky, measure the RATE — loop it and count panics:
for i in $(seq 1 20); do
  docker compose run --rm -e NEO_SMP=4 neo cargo neo-test \
    && echo "run $i: PASS" || echo "run $i: FAIL"
done
```

Inspect `target/neo-test/*.serial.log` for the `tlb_shootdown_timeout` frame on failing runs. Do **not** fixate on whichever scenario name appears in `event=FAIL` — the panic is in the shootdown path, not the scenario.

## Close gate

- `git fetch origin && git rebase origin/main` before close (current `main` tip is `2c09cb6`); re-run the gate after the rebase.
- **Statistical clearance:** run the full `cargo neo-test` suite under `NEO_SMP=4` **≥ 20 consecutive times with zero `tlb_shootdown_timeout` panics.** Given the ~50–60% base failure rate, 20 clean runs is strong evidence the flake is gone (a surviving 50% bug would almost certainly have shown by then). Paste the pass/fail tally.
- Host-checks green (`cargo fmt --all --check`, `cargo test --workspace --exclude neoplutarch-kernel`, `cargo clippy --workspace --exclude neoplutarch-kernel -- -D warnings`).
- **Keep an assertion/instrumented check** that pins the ACK-latency margin (or the re-send behavior) so this can't silently regress.
- **ADR check:** the shootdown timeout/retry policy is a contract of the SC-3C TLB-shootdown design (see the `D3/D4/D9` decision references in `tlb_shootdown.rs`'s doc comments). If you change the timeout constant or introduce a re-send protocol, update the governing ADR to record the new policy of record and the measured justification — don't let the ADR and the code disagree.
- If your investigation concludes the real cause is a genuine deadlock (a target AP that *cannot* service the IPI by construction, not merely slowly), **stop and bring it to PM with evidence** before shipping — that's a correctness bug in the locking/IRQ discipline, not a timeout tuning problem, and the fix is different.
- Push to your **feature branch, not `main`.**

---

## Bare-metal smoke — post-merge close-out (Option A, observational)

**Status (2026-06-03):** the fix merged to `main` (`5a9c878`) and **CI is green** — the flake is dead in the QEMU/TCG SMP environment that first exposed it (worst-case ACK 130.9 µs vs the 50 ms budget; ~382× headroom). This is the **final** step: confirm the 1 ms re-poke + 50 ms bounded-panic behaves on **real APIC delivery + real TSC** — the one thing QEMU/TCG cannot vouch for, and the reason this bug existed. **One physical boot. Not blocking other work; it closes the flake "for good."**

### Why observational (and not a dedicated `q7c` bare-metal run)

`xtask --bare-metal-mode` result-capture (the on-disk `NEO_LOG_v1` region read back after the run) is currently wired **only** for the AHCI/NVMe NeoFS read-write exemplars — `bare_metal_log_sink_target_feature` accepts exactly `phase3_ahci_neofs_mount_and_readwrite` and `phase3_nvme_neofs_mount_and_readwrite`. The `q7c_shootdown_*` scenarios have **no log sink and no disk pairing**, so `cargo neo-test q7c_shootdown_synthetic_round_trip --bare-metal-mode` is rejected. Rather than build that plumbing (deferred — see below), **reuse the AHCI exemplar as the vehicle**: real disk I/O under SMP drives heavy real cross-CPU shootdowns (DMA buffer map/unmap across CPUs), exercising the fixed path under load on real silicon, *with* automated `NEO_LOG` readback. Use **AHCI** (proven-green on metal per HANDOFF-2026-05-28); NVMe was previously blocked.

### Procedure

This layers the TLB acceptance criteria on the **existing AHCI bare-metal exemplar runbook** (imaging the bundle + paired disks to the physical box is per that established procedure — not re-specified here):

1. Build/run the AHCI exemplar bare-metal bundle on the **fixed** kernel:
   ```
   cargo neo-test phase3_ahci_neofs_mount_and_readwrite --bare-metal-mode
   ```
   (produces the bootable Limine bundle + paired storage/log-sink disks; deploy to the physical box per the AHCI bare-metal runbook).
2. **Attach a serial cable (COM1)** to capture `serial_println` live — this is how a `tlb_shootdown` panic frame surfaces on a headless box.
3. Boot the physical machine from the bundle.

### Pass criteria

- [ ] **SMP brings up all APs** — serial shows `smp: AP cpu_id=N ready` for each AP (confirms real cross-CPU shootdowns ran during bring-up).
- [ ] **No `tlb_shootdown.rs` panic** anywhere in the serial log, through NeoFS mount + read-write.
- [ ] **The AHCI exemplar reaches PASS** via the `NEO_LOG` readback (same validation path as the QEMU bare-metal sink) — i.e. the shootdown path did not wedge under real I/O load.

### If it surprises us

Capture the **full serial log** plus the panic message's `unacked_slots=… repoked=… elapsed_cycles=…` fields (the fix added these to the panic at `tlb_shootdown.rs`) and bring it back. That line discriminates the three failure modes: a target **never re-poked** (re-poke logic bug), **re-poked but still silent** (genuinely lost IPI / wedged CPU), or **ACK tail near budget** (real-hardware latency longer than expected — would argue for revisiting the 50 ms margin). Real hardware should be *far* tighter than QEMU's 130 µs; anything close to the budget is a real signal.

**On green:** the flake is closed on QEMU *and* metal — the bare-metal box goes back in the closet.

### Deferred (Option B — only if we ever want the hardware ACK-latency *number*)

Wire `q7c_shootdown_synthetic_round_trip` (+ the `q7c_shootdown_timeout_panic` negative) into `bare_metal_log_sink_target_feature` and `prepare_bare_metal_storage_disks` so `--bare-metal-mode` captures `max_ack_cycles` on metal directly. ~half-day of xtask plumbing. **Not needed to close the flake** — recorded here so the option isn't lost.
