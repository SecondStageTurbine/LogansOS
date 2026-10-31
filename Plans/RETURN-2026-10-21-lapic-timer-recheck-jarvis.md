# RETURN: reworked LAPIC timer re-review (PR #84)

**Verdict: BOUNCE -- the original findings are closed, but four new P2 findings remain.**

Reviewed `b456df3112af6da681f6d392a82e84bf9f095ee4` on `codex/lapic-timer-recheck-jarvis`, against `origin/claude/x2apic-support` at `b5b30d13d4e8c73b5f1f733fcd6c67df6a3327ad`. This is the timer lane only. Read my original BOUNCE from `origin/codex/lapic-timer-crosscheck-jarvis`, the entire new timer/reference-clock/parser modules, integration changes, tests, and the author's [rework report](https://github.com/SecondStageTurbine/LogansOS/pull/84#issuecomment-6322845301).

## Requested closure decisions

| Item | Status | Evidence and scope |
| --- | --- | --- |
| P2-a: independent calibration, cross-check, leaf 0x16 estimate | **CLOSED for the original finding** | `calibrate_tsc`, `calibrate_lapic`, `measure_rates`, and `choose_rate` prefer leaf 0x15, then successful HPET / PM / PIT measurements. The primary is checked against another source when available, with an inclusive +/-5% limit. Leaf 0x16 is used only after these fail and is explicitly logged as an estimate. It can still program an estimated tick when no reference exists; that is not independently calibrated timing. The scenario no longer validates a rate against itself. New acquisition and dispute-handling defects are below. |
| P2-b: x2APIC scenario asserts mode | **CLOSED** | `test_harness.rs:18475` rejects anything except `ApicMode::X2Apic` under the x2APIC scenario feature, with `apic_mode_not_x2apic`. The CPU selection alone is no longer the proof. |
| Fallback limitation: verify PIT and display a no-tick halt | **CLOSED for the missing verification/diagnostic** | `scheduler.rs:383,436` checks `TICKS` after PIT programming/unmasking. Failure prints `NO TICK SOURCE ... halting`, then halts in production or emits a named test failure. With `fb-progress-console` enabled and the framebuffer initialized, `serial::_print` mirrors that line to the screen; `main.rs:636` starts the mirror before this point. Screen output is conditional on that feature, not implied by `lapic-timer` alone. Findings R1/R2 prevent an unconditional claim that fallback is reliable. |

## New findings

### R1 -- P2: a rejected TSC rate still rejects independently calibrated periodic mode

**Locations:** `kernel/src/arch/x86_64/lapic_timer.rs:402-411,454`; `kernel/src/sched/scheduler.rs:438-440`.

The `RateChoice::Mismatch` arm disables deadline mode but returns `Some(primary.hz)` and publishes it in `TSC_HZ`. The rejected primary then sizes periodic first-tick verification and PIT fallback verification. A sufficiently low erroneous primary makes both windows shorter than a valid tick period. Thus the claim that a disputed TSC rate only disqualifies deadline mode is false outside modest discrepancies.

**Concrete reproduction:** report leaf 0x15 `(EAX,EBX,ECX)=(1,1,24_000_000)` while the actual TSC runs at 2.4 GHz. HPET measures 2.4 GHz, so the TSC choice is `Mismatch`. Independently, the actual LAPIC runs at the correctly reported 24 MHz crystal and its HPET measurement agrees, so periodic calibration succeeds with count 240,000 at 100 Hz. Nevertheless `first_tick_budget(Some(24_000_000),100)` is only 1,200,000 actual TSC cycles: **0.5 ms**, versus the first periodic tick at 10 ms. It is disarmed as silent. The subsequent PIT verification budget is only **5 ms**, also shorter than a freshly programmed 100 Hz PIT period; it can falsely report no tick source and halt.

The actual helper functions reproduce the mismatch, accepted LAPIC rate, and those budgets in the host probe. This is a source/arithmetic reproduction, not a boot with injected CPUID. Verification needs an independent elapsed-time bound, or a separately justified conservative bound when the TSC rate is disputed; the rejected primary cannot remain the authority for those deadlines.

### R2 -- P2: starting HPET preserves legacy replacement and enabled comparator interrupts

**Location:** `kernel/src/arch/x86_64/refclock.rs:78-86`.

Discovery reads the general configuration and merely ORs in `ENABLE_CNF`. It never clears legacy replacement or disables comparator interrupts, including when the block was already enabled. The comment that no comparator interrupt is enabled is not established by this code: comparator state can already be configured by firmware.

**Reproduction:** supply a valid HPET with general configuration `0x2` (counter off, legacy replacement selected) and timer-0 interrupt-enable bit set (`timer0_config & 0x4 != 0`). The probe imports the unchanged production `refclock.rs`, substitutes an aligned memory register block and port-I/O stubs, and calls `discover`. The result is `global_config=0x3 timer0_config=0x4`: counting and comparator delivery are now globally enabled while legacy routing remains selected. General configuration `0x3` on entry is also left unchanged.

The [Intel HPET specification, sections 2.3.5 and 2.4.2](https://www.intel.com/content/dam/www/public/us/en/documents/technical-specifications/software-developers-hpet-spec-1-0a.pdf) says global enable allows previously enabled comparators to interrupt, and legacy replacement suppresses the 8254 interrupt route. **Inference from that specified behavior and this code:** a reference-clock probe can activate an inherited interrupt source or make its own PIT fallback unavailable on a platform with a working 8254. Discovery runs before BSP scheduler installation. Take ownership of interrupt/routing state before enabling HPET for polling, and ensure the legacy fallback route remains usable. The register-state reproduction is confirmed; no firmware-configured HPET boot was run.

### R3 -- P2: hardware-reduced FADT fields are interpreted as a real PM timer

**Location:** `kernel/src/acpi/timers_logic.rs:40-66`; consumed at `refclock.rs:105-108,148-156`.

The FADT parser reads `TMR_VAL_EXT` but never checks `HW_REDUCED_ACPI` (flags bit 20). It returns an extended or legacy PM port from fields which software must ignore on a hardware-reduced system, and `read_free_running` subsequently performs a 32-bit port read there. A table checksum does not make those ignored addresses meaningful.

**Reproduction:** pass a 276-byte, signature-correct, checksum-correct revision-6 FADT with flags `1 << 20`, legacy port `0x608`, `PM_TMR_LEN=4`, and extended system-I/O address `0x1808`. The actual parser returns `Some(PmTimer { port: 6152, wide: false })`; expected is no PM timer. Test both legacy-only and extended cases when correcting it. Such reads can target unrelated I/O, or introduce a false/dead calibration source, on exactly the class of modern reduced-hardware systems this lane addresses.

[ACPI 6.5 section 5.2.9](https://uefi.org/specs/ACPI/6.5/05_ACPI_Software_Programming_Model.html#fixed-acpi-description-table-fadt) explicitly requires ignoring the fixed-hardware fields at offsets 46-108 and 148-232 when this flag is set. Both PM address forms lie in those ranges. Refuse the PM timer before interpreting either address when hardware-reduced is set.

### R4 -- P2: a stuck-high PIT response is accepted as a completed calibration window

**Location:** `kernel/src/arch/x86_64/refclock.rs:204-227`, called by `measure` at lines 172-180.

The timeout only runs while OUT2 is low. There is no proof that programming mode 0 made OUT2 low before accepting its high state as terminal count. With an absent/nonresponding legacy device returning `0xff` from port `0x61`, or any stuck-high OUT2 response, the loop is skipped and the driver reports a completed 9,999,312 ns interval after just programming/read overhead. The outer sampler then divides its short counter delta by this invented duration. Without another reference this becomes a supposedly calibrated rate; with a good primary it can poison the cross-check and unnecessarily reject it.

**Reproduction:** in the same unchanged-source host probe, stub port reads to `0xff` and advance the sampled counter by 100 between its two calls. `measure(Pit,10_000_000,...)` returns `Some((100,9999312))` immediately, despite no counter/OUT transition. In contrast, constant-zero reads return `None` after the 3-billion-TSC-cycle timeout (about 682 ms on this host). So the stuck-low gated case is bounded, but unavailable/high responses are not identified as failed clocks. Require observable one-shot progress/completion rather than a high level alone. This is an injected I/O-response reproduction; I have not established which OUT2 level the Zenbook exposes when its PIT is gated.

## Remaining adversarial checks

- **Tables and mapping:** the new `read_table` validates target length (36 bytes through 1 MiB) and the entire leaf-table checksum before passing HPET/FADT bytes to the parsers. Field reads use checked slices. HPET is accepted only in memory space with a nonzero address; supported PM timers are port based, with extended-I/O preference, legacy length 4, and 24/32-bit masking. HPET period must be nonzero and at most 100 ns. Its page is mapped uncached before volatile register access, and 32-bit main-counter wrap is masked. The disabled-counter enable operation itself is present, but R2 makes its side effects unsafe. R3 is a separate FADT semantic validation failure.
- **Validation limits:** checksum validity alone does not validate every physical/GAS address. Existing shared root discovery still lacks the leaf reader's length cap, and the RSDP extended checksum is clipped to 36 bytes rather than validating an arbitrary declared extended length. Those are inherited parser limitations, not presented as new timer-specific findings here. Host parser KATs do not exercise physical table walking, checksums, or page-table mutation.
- **Calibration selection:** it gathers up to two successful independent measurements in HPET / PM / PIT order, continuing past failed measurements. With leaf 0x15 present, that is the primary and the best independent measurement checks it; otherwise the first successful measurement is checked by the next. A mismatch is not bypassed with leaf 0x16. A single source is explicitly logged as unchecked. The +/-5% helper handles both directions and the boundary correctly. R1 concerns the later use of a rejected rate, not this selection arithmetic.
- **Scenario:** `test_harness.rs:18485-18505` samples `TICKS` over 500 ms of the first discovered reference, HPET on the reported QEMU q35 setup, then checks 100 Hz +/-10%. The expected duration no longer comes from `TSC_HZ`, so the original correlated-rate false pass is removed. A dead selected reference produces `timer_reference_clock_dead`; the test does not silently revert to a TSC interval. PM or PIT can be used if no HPET was discovered, so this is not an unconditional HPET-presence assertion.
- **QEMU drift:** the author's reported ~3.5% difference between TSC measurements is inside the 5% acceptance threshold; it does not itself trigger `Mismatch`. The 0.3% LAPIC agreement and 50/50 HPET scenario results are the author's runs, not mine. Beyond 5%, disabling deadline while retaining an independently calibrated periodic timer is the correct separation, but R1 shows that the timeout path does not fully preserve it. I did not reproduce the reported QEMU drift.
- **Interrupt integration:** timer-vector/ISR-based EOI, masked one-shot LAPIC calibration, disarm before demotion, and BSP-only scheduling retain the structure reviewed in the original BOUNCE. First-tick verification follows scheduler installation with IF enabled in the normal boot path. It still returns unverified if called with IF clear. The feature-off path keeps the PIT/PIC behavior. TSC-deadline arm/rearm and real firmware/device states remain outside the host tests.
- **Controls:** the mode assertion, forced fallback, masked-first-mode demotion, named no-verify failure, and masked-IRQ0 no-tick failure are wired in source. The `pit-gated` control gates IRQ0 delivery; it does not simulate gated channel-2 reads and therefore cannot catch R4.

## Validation and scope

- `cargo test -p lapic-timer-logic-tests -p acpi-timer-logic-tests`: PASS, 10 + 5 unit tests; doc tests PASS (0 tests).
- `git diff --check origin/claude/x2apic-support origin/claude/lapic-timer`: PASS.
- Additional temporary host probe: imported the actual helper/parser/reference-clock files; replaced privileged port access, ACPI discovery, and MMIO mapping with controlled stubs. Confirmed the four input/output cases described above. No implementation files were changed. Reproduction values and observed outputs are recorded here; the probe is not a hardware emulator or a kernel boot test.
- No kernel build, QEMU scenario rerun, or silicon run was performed. The author's reported scenario/control passes do not cover these injected edge cases.
- Graft was used for initial discovery, followed by direct source reads. RAG health was healthy. Existing `.gitignore` and `.ignore` worktree edits are excluded from this commit.
