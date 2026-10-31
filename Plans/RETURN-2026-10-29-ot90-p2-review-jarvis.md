<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: OT-90 P2 power mechanism, relay #154 / #195

Reviewer: Jarvis. Worktree: `E:\AI Automation\workers\jarvis`.
Branch: `codex/review-bundle-2026-10-29c-jarvis`. **FAIL.**
Read squash `83d8e74f`, the F1–F5 table and THINK sections 3–4.
The power implementation/logic and qmp.rs are unchanged from that squash to
review main `87458b7e`; line references below use the latter.

1. **PASS for a progressing timer read at least once per wrap.**
   `kernel/src/acpi/power_logic.rs:241` sums consecutive masked deltas into a
   saturating u64 total; :253 uses that total instead of one start-to-now
   difference. Both 24/32-bit timers can therefore exceed one wrap and end.
   `tools/acpi-power-logic-tests/src/tests.rs:1034` checks the old maximum
   cannot reach the 10s target, then exact read counts on both widths and
   an early done predicate. The 32-test host suite passes. A stopped timer
   or polling delayed by an entire wrap is outside that progress assumption;
   the function cannot infer unobserved complete wraps. The huge-us conversion
   saturates its multiplication, so this is not a precision proof for all u64
   durations; the actual P2 waits are well within the exact arithmetic range.
2. **FAIL — F2 checks physical width but not HHDM representability.**
   PM1 writes are correctly ordered A(type), B(type), A(enable), B(enable)
   (`kernel/src/acpi/power.rs:220`), with each register's own preserved bits
   and typ (`power_logic.rs:309`). Port-width checks (:193, :440) prevent
   16-bit truncation; PCI config (:200, :497) constrains bus-0 encoding.
   Init premaps cached SystemMemory registers (`power.rs:109`, :454), and
   runtime `gas_write8` (:415) never maps.
   However `phys_ok` (`power_logic.rs:277`) accepts every nonzero address below
   2^52, while `mm::phys_to_virt` (`kernel/src/mm/mod.rs:514`) computes
   `VirtAddr::new(phys + hhdm_offset())`. Concrete supported-address-space
   input: HHDM offset 0xffff800000000000, FADT RESET_REG_SUP with an 8-bit
   SystemMemory reset GAS at **0x0001000000000000 (2^48)**. The planner accepts
   it, then init's premap adds to **0x10000800000000000**: overflow with checks,
   or wrapped noncanonical 0x0000800000000000 without checks. Init panics
   before any reset fallback can run. The same issue applies to reduced
   sleep registers. Validate the checked direct-map translation/canonical
   kernel VA as well as architectural physical width before caching/mapping.
   This is a source-derived input, not a firmware-injection run.
3. **PASS with the documented IF-on entry requirement.**
   `power.rs:380` reopens interrupts during waits when entry IF was set,
   then disables them for the next write run; :364 restores IF for halt.
   CF9's short 50us gap stays inside its write run. Long S5/ACPI_ENABLE/reset
   waits use `wait`. Entry IF=0 deliberately remains zero, so P3/P4 must
   enforce their stated IF-on calling contract; P2 does not override it.
4. **FAIL — F4 boot counting is sound; F5 duration is not checked by the judge.**
   `tools/xtask/src/qmp.rs:190` requires exactly one guest-shutdown event,
   the expected 1/2 BEGIN frames, and the required ordered power markers.
   Restart allows only 1–2 guest-reset events before shutdown. The caller
   also requires QEMU exit success (`tools/xtask/src/main.rs:9138`). Empty
   logs, extra boots, wrong reset reasons and absent shutdown cannot pass.
   But the fallback match (:261) is only `reset step=1 wait ended`, not
   `us=6000000`. Executed negative control: take the existing valid fallback
   serial fixture, replace `us=6000000` with **`us=0`**, retain its valid
   RESET/SHUTDOWN events, then assert judge refuses it. That assertion FAILS:
   judge accepts it. Thus shortening the neutral wait can remove the runtime
   wrap discriminator while the scenario stays green. Require the planned
   duration marker; retain the real timer wait and its host negative control.
5. **PASS — source exclusion and inspected author binary evidence.**
   The module and ACPI/main entry points are `power-control` gated, and the
   scenario-only neutral reset is behind neo-test. #195's table records equal
   `.text` sizes/hashes with the feature off for default/desktop/product;
   the off scenario is a smoke, not the binary proof. These pairs were not
   independently rebuilt here.

Executed at `87458b7e`: acpi-power-logic-tests 32 passed; xtask qmp tests
3 passed, 1 ignored (the opt-in real-QEMU test). The temporary zero-duration
judge assertion failed as described and original source bytes were restored.
Independently queried
[batch 16](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40843436986):
success at `fb039fbdd6b925e4aaf346025e6789f34e0cd908`. RAG healthy.
No new kernel boot, hardware register write or binary comparison claimed.
Route confirmed failures through the escaped-defect process; the feature is
default-off and absent from the product lane under review.
