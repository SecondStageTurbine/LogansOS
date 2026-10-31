<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Janus fix-forward recheck (#47 / #48)

**Verdict: both RETURNs CLOSED.**

Reviewed current `origin/main` at `2acfa308`, including #47 (`663985b2`) and
#48 (`f9080197`), only for the defects raised by Janus in `1293dee4` and
`e3e4f9ec`.

## 1293dee4 — CLOSED

The withheld-LaunchCap-proof WM trap is closed.

- In the pointer build, `launch::serve` reaches the proof only through the
  non-blocking `recv_proof` loop
  (`userland/neo-init/src/desktop_session.rs:1690-1722`). Each empty
  `SYS_TRY_RECV` poll checks the three-second monotonic deadline, runs the
  caller's `scene.pointer_tick(...)`, and yields. A client that leaves its cap
  open and never sends a proof can no longer park the WM in `SYS_RECV`.
- The idle callback is the same SAK/pointer service used by the main surface
  loop (`desktop_session.rs:1210-1225`). It cannot re-enter the broker: it
  consumes pointer/SAK state and may swap focus/present, but does not receive
  or dispatch a surface request. An escape during the wait is therefore safe;
  the later refusal reply is surface-channel IPC and does not depend on Arthur
  still owning the keyboard route.
- Timeout closes the broker endpoint, sets the owning `Option<Handle>` to
  `None`, and revokes the provenance (`desktop_session.rs:1615-1627`). The
  client's late proof send then receives `EPIPE`; `atrium_client::launch`
  deliberately continues to the surface reply. Session teardown closes the
  endpoint only when the option is still `Some`, so the handle is closed once.
- `SYS_CLOCK_GETTIME` and `SYS_SCHED_YIELD` are enabled in the kernel only for
  `browser-runtime` or `atrium-pointer-forward`. A clock failure maps to
  `u64::MAX` and therefore an immediate timeout. The 10 ms monotonic clock
  resolution does not compromise the liveness bound.
- The pointer scenario requires the serial order `awaiting-proof` -> SAK
  `escape` -> `proof-timeout` -> revoke/refuse. Because the main request loop
  cannot resume until `launch::serve` returns after the timeout, the escape
  observed before timeout demonstrates that the SAK was handled inside the
  proof-poll loop.

Non-blocking note: the loop probes the rendezvous before checking the clock, so
a proof that becomes ready immediately after the nominal deadline can be
accepted on that iteration. This slightly softens proof admission at the
boundary, but it cannot recreate the indefinite withheld-proof trap and does
not keep the original RETURN open.

## e3e4f9ec — CLOSED

The key-overflow loss of the sole queued button release is closed.

- The final overflow fallback now searches for and removes the oldest
  non-pointer event instead of blindly calling `pop_front`
  (`kernel/src/atrium/input_queue_logic.rs:148-161`). In Janus's capacity-3
  sequence, this removes `key(4)` at index 1 while preserving the release at
  index 0; draining yields `release, key(5), key(6)`.
- The permanent KAT
  `a_key_overflow_never_evicts_the_sole_queued_release_at_the_head`
  (`tools/atrium-logic-tests/src/input_queue_kats.rs:298-329`) is the exact
  deterministic reproduction from the RETURN and passes. The broader input
  queue tests also pass, including final-button-state and key-only legacy
  equivalence.
- On the production capacity-64 path, reaching this fallback with a full queue
  and no superseded pointer transition means there is at most one pointer
  event, so at least one key exists for the scan to select. The `unwrap_or(0)`
  case is limited to a capacity-1 queue containing only a pointer and does not
  apply to the reported production failure.

## Verification

- RAG health: `healthy` (`ollama=true`, `qdrant=true`). The requested semantic
  query returned unrelated results, so the verdicts use direct commit diffs,
  source reads, and exact-symbol searches.
- `cargo test -p atrium-logic-tests input_queue`: **10 passed**.
- `cargo test -p atrium-logic-tests launch`: **2 passed**.
- Inspected the #47 pointer scenario ordering and the channel-close/`EPIPE`
  implementation. QEMU was not rerun for this review.
