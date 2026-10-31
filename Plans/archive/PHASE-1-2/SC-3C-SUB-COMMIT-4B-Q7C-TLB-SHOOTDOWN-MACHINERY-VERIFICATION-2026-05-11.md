# SC-3C Sub-Commit 4b Verification: Q7.c TLB Shootdown Machinery

**Date:** 2026-05-11
**Owner:** Jarvis
**Status:** PASS

## Scope

Sub-commit 4b lands the Q7.c shootdown machinery only. It adds the explicit `CpuMask` target-set API, vector `0xfc` receiver path, per-CPU `pending_shootdown` slots, stack-local `ShootdownPayload` with `AtomicU32` ACK accounting, and the D9 sender-side self-invalidation ordering.

No production Pmap mutation call sites are wired in this sub-commit; those remain gated to sub-commit 4c. AP bootstrap now enables interrupts after IDT, LAPIC, slab, and scheduler setup so cross-CPU IPIs can be delivered on initialized APs.

## Gates

| Gate | Result | Notes |
|---|---:|---|
| `cargo k` | PASS | Existing warnings only. |
| `cargo k --no-default-features` | PASS | Existing warnings only. |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test q7c_shootdown_synthetic_round_trip` | PASS | `NEO_TEST/1 event=PASS name=q7c_shootdown_synthetic_round_trip target_mask=0x000000000000000e targets=3 variants=3 total_remote_ipis=9 self_invalidations=3` |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test q7c_shootdown_timeout_panic` | PASS | Expected panic observed: `tlb_shootdown_timeout pending=1 targets=0x0000000000000002`; harness emitted `NEO_TEST/1 event=PASS name=q7c_shootdown_timeout_panic expected_panic=tlb_shootdown_timeout`. |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all` | PASS | First full-suite attempt timed out in historical `cap_survive_exit`; isolated `cap_survive_exit` rerun passed, and the subsequent full-suite rerun passed including the deliberate-failure meta-test. |

Direct host `cargo neo-test` is not a valid gate in this environment because Limine/xorriso are unavailable on the host; Docker compose was used for QEMU scenario gates.

`cargo fmt` could not be run because `rustfmt` is unavailable for the pinned nightly toolchain in this environment.

## Notes

- D3: `INVLPG_THRESHOLD_PAGES` is `33`.
- D4: `shootdown(range, targets)` takes an explicit target set; SC-3C callers will pass broadcast-minus-self until Phase 2+ per-mm target tracking exists.
- D5/D10: senders wait synchronously with a 1ms bounded timeout and panic on missing ACKs.
- D8: vector `0xfc` is registered in the IDT.
- D9: self-invalidation runs after remote IPI dispatch and before the ACK wait loop.
- D11: each CPU owns a single `AtomicPtr<ShootdownPayload>` pending slot.
- No `200/200 NEO_SMP=4` stress run is required for 4b in isolation per dispatch; that gate belongs after 4c call-site wiring.
