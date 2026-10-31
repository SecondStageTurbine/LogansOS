# RETURN: CP-69 LAPIC timer cross-family review

**Verdict: BOUNCE**

Reviewed `ca51fec656d8bb51587a3ca339d304aab21ba6a7` against its x2APIC parent `e77d8d273d1e2f4c7f2596744d492077d1176859`. The RAG search was healthy and returned timer/scheduler entry points; `graft ask` supplied the first-tick starting point. I read the changed timer, APIC, scheduler, IDT, test harness and test configuration code. Intel SDM Vol. 3A's APIC timer/ExtINT rules and the Intel CPUID instruction extensions reference were checked directly.

## Findings

### P2 — CPUID leaf 16 is not a guaranteed TSC rate, and the tests cannot detect a wrong one

**Location:** `kernel/src/arch/x86_64/lapic_timer.rs:270-275`; conversion at `kernel/src/arch/x86_64/lapic_timer_logic.rs:29-38`; rate check at `kernel/src/test_harness.rs:18455-18470`.

When CPUID leaf 0x15 does not produce a TSC frequency, the code uses leaf 0x16 EAX as an exact TSC frequency before trying the PIT measurement. Leaf 0x16 reports the processor's *base frequency*. Intel's CPUID reference says these are specification values and do not accurately correlate with values from other processor interfaces; the SDM's nominal TSC calculation uses leaf 0x15's TSC/core-crystal ratio. The implementation's “approximation” is not independently validated.

A wrong value can pass both first-tick verification and the scenario. For example, if leaf 0x16 says 2.4 GHz while the actual TSC rate is 2.0 GHz, the TSC-deadline interval is 24 million cycles, which produces about 83 ticks/s. The first tick arrives, so mode verification passes. The scenario then waits for 1.2 billion cycles (0.6 actual seconds), sees about 50 ticks, and passes its broad 2–200 bound. For periodic calibration, using the same incorrect TSC rate scales the measured LAPIC rate and the test interval together, so the error can similarly cancel.

**Repro:** run a CPU/hypervisor reporting leaf 0x16 base MHz that differs from the actual invariant TSC rate, with leaf 0x15's ratio unavailable. The kernel programs from the leaf 0x16 value, and `lapic_timer_tick` reports success despite a wrong real-time tick rate. Use an independent time source to validate the timer rate, or treat leaf 0x16 as an estimate and prevent it from claiming calibrated timing without a cross-check.

Intel references: [Architecture Instruction Set Extensions Programming Reference, CPUID leaves 0x15/0x16](https://cdrdv2-public.intel.com/843860/architecture-instruction-set-extensions-programming-reference-dec-24.pdf); [Intel SDM combined volumes](https://cdrdv2-public.intel.com/868137/325462-089-sdm-vol-1-2abcd-3abcd-4.pdf).

### P2 — The x2APIC timer scenario can pass while running in xAPIC mode

**Location:** `kernel/src/test_harness.rs:18409-18475`; scenario selection at `:749-751`; QEMU CPU selection at `tools/xtask/src/main.rs:7618-7621`.

`lapic_timer_tick_x2apic` calls the same `run_lapic_timer_tick` routine as the xAPIC scenario. That routine prints the APIC mode but never asserts it. Its pass conditions only require an active LAPIC timer, PIC IRQ0 masked, nonzero TSC rate, and a tick count in the broad range. The xtask's `max,+x2apic` CPU flag requests capability; it does not assert that Limine left the BSP in x2APIC mode or that the timer's register accesses used MSRs.

**Repro:** boot that scenario with `xapic-only` enabled (or otherwise arrange for Limine to leave the BSP in xAPIC mode). If the LAPIC timer works through MMIO, the scenario can still emit `lapic_timer_tick=ok apic=xAPIC`. Add a mode assertion under `neo-test-lapic-timer-x2apic` and fail unless `apic::mode() == X2Apic`.

## Review checklist

1. **EOI source detection:** `apic::in_service(32)` reads ISR register 1, bit 0 (`0x110`, x2APIC MSR `0x811`). The SDM says ExtINT bypasses the LAPIC IRR/ISR; a fixed-delivery LAPIC timer interrupt sets ISR. The test-only one-shot timer uses vector `0xe1`, and the configured APIC spurious vector is `0xff`, so I found no other current source that can set vector 32's LAPIC ISR bit. With IF clear in the handler, no same-vector nested interrupt races this check. PIC ExtINT gets PIC EOI; a LAPIC timer gets LAPIC EOI. At demotion, `disarm` masks and stops the LAPIC source; a late LAPIC ISR is still recognized and EOI'd even after the timer state changes. `start` only programs/unmasks PIT IRQ0 after it returns false, so the normal path does not intentionally run both tick sources concurrently.

2. **Disarm, demotion and ordering:** `disarm` stores inactive, masks the LVT (also changing away from TSC-deadline mode), clears the initial count and writes zero to `IA32_TSC_DEADLINE` when armed. The SDM specifies that zero disarms the deadline and changing timer modes disarms it. A tick can race the timeout check and increment `TICKS` just before disarm; its ISR is handled and EOI'd, then disarm stops any re-arm. This can demote a mode that fired at the timeout boundary, but does not leave its ISR set. The xAPIC LVT write precedes the first deadline WRMSR with `mfence`, as required; the same extra fence in x2APIC mode is harmless. The SDM says x2APIC LVT and deadline WRMSRs are ordered in program order.

   **Fallback limitation:** if LAPIC calibration/first-tick checks fail on a machine whose PIT/8259 route is also gated, `sched::init_bsp` still programs PIT channel 0 and unmasks IRQ0, but does not verify a PIT tick. That fallback cannot recover the Zenbook's stated PIT failure; `TICKS` can remain stopped and the scheduler can hang. The fallback is useful only on machines where the legacy path works. The QEMU fallback/control scenarios do not exercise a gated PIT.

3. **Calibration:** Leaf 0x15 conversion (`ECX * EBX / EAX`) and zero-field rejection are correct. Leaf 0x16's role is the first finding. The PIT channel-2 sequence selects channel 2, LSB/MSB, mode 0 (`0xb0`), loads `PIT_HZ/100`, raises gate 2 via port `0x61` bit 0, and polls OUT2 bit 5; channel 0 is left alone. The timeout is 4 billion TSC cycles, so it terminates if RDTSC advances; it is not a fixed wall-clock timeout. For Intel CPUs enumerating the TSC/core-crystal ratio, the SDM states that the LAPIC timer uses the core crystal (or processor bus clock) divided by the divide setting; using the nonzero 0x15 core-crystal value with divide-by-1 matches that rule on this path. The measured fallback also divides by the programmed divide-by-1 value.

4. **First tick and scheduler:** `main` enables interrupts before `sched::init_bsp`; `init_bsp` installs the BSP scheduler with an empty ready queue before calling `lapic_timer::start`. Thus the normal call reaches `ticked_within` with IF enabled, and an early tick runs against initialized scheduler state. `on_tick` is unchanged and remains BSP-only, as was the PIT IRQ0 tick; APs still have no timer/preemption point. The 5-period budget is 50 ms at 100 Hz when the TSC rate is right, and a fallback one-second budget when it is unavailable. If called with IF clear, `ticked_within` returns `None` and `start` treats the mode as usable but unverified; that is not the current `init_bsp` call path.

5. **Scenarios and controls:** `lapic_timer_tick` checks the forced-fallback control by requiring the LAPIC timer active, so the PIT control fails `timer_not_lapic`; the preceding smoke test also exercises the PIT on QEMU. `lapic_timer_demote` masks its first selected mode and proves the timeout/disarm/demotion mechanism on QEMU's periodic path, but does not prove real hardware silence or the deadline→periodic transition. The `no-verify` control removes the first-tick loop, leaves the timer masked, and then relies on an external QEMU timeout during the preemption smoke test; that is a plausible negative control but less diagnostic than a named failure marker. The rate bound is deliberately broad and not an independent calibration check (finding 1). QEMU TCG does not exercise TSC-deadline on the cited setup; deadline arm/rearm/disarm remains untested there. A KVM guest exposing host TSC-deadline or real silicon can cover it; host KATs only cover the pure arithmetic/planning logic.

6. **Kill-switch:** With `lapic-timer` off, `timer.rs` retains `pic::end_of_interrupt(0)`, and `sched::init_bsp` takes `lapic=false`, calls `pit::init`, then unmasks IRQ0 in the old order. The feature-gated LAPIC helpers and timer module are not compiled. The old PIT/PIC behavior is preserved; source code has a small cfg/constant refactor, so “byte-identical” is accurate for the path's effects, not literal source text.

7. **Other:** The ISR-lock boundary is unchanged: the new EOI helper does port/MSR/register operations and takes no lock; the existing timer handler's scheduler/preemption work follows EOI. APs remain outside this change. The shipping/product image does not enable `lapic-timer` yet, so adoption still needs the product decision and a Zenbook run.

## Validation

Static review only. No build or tests were run.
