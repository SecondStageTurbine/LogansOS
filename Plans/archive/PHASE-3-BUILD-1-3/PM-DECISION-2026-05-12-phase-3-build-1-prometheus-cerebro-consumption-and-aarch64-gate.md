# PM Decision: BUILD-1 Closure Prep Consumption and aarch64 Gate

Date: 2026-05-12
Authority: PM
Inputs:

- `Plans/PREP-2026-05-12-phase-3-build-1-closure-prometheus.md`
- `Plans/RESEARCH-2026-05-12-phase-3-build-1-aarch64-lane-feasibility-cerebro.md`
- `Plans/DISPATCH-2026-05-12-phase-3-build-1-1d-integration-closure-jarvis.md`
- `docs/ADR-021-phase-3-design-substrate.md`

## Decision

Prometheus's closure dossier shell is accepted as a useful non-normative prep artifact. It correctly inventories 1A, 1B0, 1B, 1C, leaves 1D pending, and does not claim BUILD-1 closure.

Cerebro's aarch64 technical findings are accepted. Cerebro's policy recommendation to treat the aarch64 runtime lane as deferrable to BUILD-2 is not accepted at this time.

The PM gate remains:

- BUILD-1 may close only if the aarch64 barrier-exposure lane honestly passes.
- If aarch64 cannot be implemented in 1D, Jarvis may land x86 integration plus a formal blocker artifact.
- If the blocker path is taken, the verdict is `x86 integration PASS / BUILD-1 closure BLOCKED on aarch64 lane`.
- Compile-only aarch64 evidence is useful structural evidence, but it is not barrier-correctness evidence and cannot close the ADR-021 gate.

No normative docs are changed by this decision.

## Accepted Prometheus Output

Prometheus may remain on standby until Jarvis returns 1D evidence.

The dossier shell should be used after 1D to:

- check that all integration evidence slots are filled;
- assemble the closure report if and only if closure is honest;
- assemble a blocked-disposition note if aarch64 remains unresolved;
- avoid editing ADR-021, Bible, README, phase docs, or indexes until PM gives a post-1D instruction.

## Accepted Cerebro Technical Findings

The following aarch64 findings are ratified as the technical basis for the 1D aarch64 decision:

- Current repo lacks `kernel/src/arch/aarch64/`, an aarch64 target JSON, an aarch64 linker script, aarch64 toolchain target config, and aarch64 `.cargo` target config.
- Current x86-only dependencies and devices (`x86_64`, `raw-cpuid`, `uart_16550`) must be gated or abstracted before a real aarch64 lane can compile.
- Limine should not be the first aarch64 route; a direct QEMU `-kernel` ELF lane is the smallest honest path.
- Virtio-mmio is acceptable for the aarch64 lane because split virtqueue barrier discipline is transport-independent; PCI is not required to expose the barrier class.
- Compile-only aarch64 evidence is too weak for the acceptance gate because `fence()` vs `compiler_fence()` both compile and only runtime/codegen behavior exposes the missing hardware barrier.
- The smallest useful runtime lane should negotiate `VIRTIO_F_EVENT_IDX` and exercise the notification-suppression branch, not an always-notify shortcut.
- Step-3 descriptor publish requires a real `fence(Ordering::Release)`.
- Step-5 notification-suppression ordering requires a real `fence(Ordering::SeqCst)` or equivalently strong full barrier.
- Named barrier wrappers such as `publish_descriptors()` and `before_notification_check()` are the preferred audit shape.

## Policy Correction

Cerebro's artifact says the lane should be a parallel workstream and, if incomplete, may be recorded as a BUILD-1 deviation deferred to BUILD-2.

That is research advice, not PM policy.

ADR-021 currently states that the aarch64 QEMU CI lane passes before BUILD-1 closes. The 1D dispatch intentionally preserves that gate. PM is not loosening it mid-closeout on the basis of feasibility research alone.

The acceptable 1D outcomes are therefore:

1. `BUILD-1 CLOSED`: all x86 integration evidence passes and a real aarch64 runtime barrier-exposure lane passes.
2. `x86 integration PASS / BUILD-1 closure BLOCKED`: x86 integration evidence passes, aarch64 remains unimplemented or compile-only, and Jarvis files the formal blocker.
3. `1D FAIL`: any required integration or regression gate fails.

There is no fourth outcome of "BUILD-1 closed with aarch64 deferred to BUILD-2" unless PM issues a later explicit ADR/PM decision changing the gate.

## Instruction To Jarvis

Continue the current 1D dispatch unchanged.

If Jarvis consumes Cerebro's aarch64 feasibility artifact, use it for the blocker content and for any attempted lane design, but do not use its BUILD-2 deferral language to claim closure.

If aarch64 is not completed in 1D:

- file `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md`;
- include the missing substrate and smallest viable lane from Cerebro's research;
- explicitly state that compile-only is insufficient;
- do not create a close report;
- report `x86 integration PASS / BUILD-1 closure BLOCKED on aarch64 lane`.

## Instruction To Prometheus

Stand by until Jarvis returns 1D results.

After Jarvis returns:

- if 1D closes all gates, use the dossier shell to prepare the close report for PM review;
- if aarch64 blocks closure, prepare a blocked-disposition note instead of a close report;
- do not edit normative docs until PM consumes the 1D result.

## Instruction To Cerebro

No further immediate research is required.

If PM later commissions the aarch64 implementation lane, Cerebro's artifact becomes the starting brief for the implementation dispatch.
