<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Desktop DS-1b LaunchCap cross-check — Janus (Codex)

**Verdict: BOUNCE**

Reviewed DS-1b as merged by `babdbf894e1a642901a13d0280805be7dd5c450d` and its integration on current `origin/main`. The advisory authority decision is implemented as specified, and I found no allowlist or possession-proof bypass. One liveness finding survives on current main after DI-1a made secure attention/pointer service non-blocking.

## Finding

### HIGH — withholding the LaunchCap proof re-traps the WM and prevents it from consuming secure attention

**Locations:** `userland/neo-init/src/desktop_session.rs:723-740`, `userland/neo-init/src/desktop_session.rs:798-803`, `userland/neo-init/src/desktop_session.rs:1130-1138`

The current desktop-pointer session deliberately polls the surface with `try_recv_request` and calls `scene.pointer_tick(...)` while Arthur is quiet. That is what lets the WM consume the kernel's SAK latch and escape a malicious or wedged client. Once Arthur sends a well-formed `OP_LAUNCH`, however, the WM synchronously enters `launch::serve`, whose first operation is blocking `sys_recv(broker_end, &mut proof)`.

Deterministic attack sequence:

1. Arthur sends a valid 64-byte `OP_LAUNCH` on its surface endpoint. The WM's polling receive accepts it.
2. Arthur keeps its LaunchCap endpoint open but sends no eight-byte proof.
3. The WM parks forever in `sys_recv` on the broker endpoint. The client does not close the peer, so there is no `EPIPE` wake.
4. Super+Escape can still be latched by the kernel, but the WM no longer reaches `pointer_tick` / `sys_input_sak_take`; pointer and Spine interaction are likewise starved.

At the original DS-1b merge this was equivalent to DS-1a's already-recorded blocking-surface focus trap. It is no longer equivalent on current main: DI-1a removed that surface-channel trap, while this second blocking receive silently retained it. The proof receive needs a polled/state-machine form (or another cancellation path that continues servicing SAK/pointer input) before the secure-attention close is complete.

## Adversarial checks

1. **Broker judgment.** `judge` first finds a cap bound to the actual surface-channel provenance supplied by the WM, then requires the request's provenance and presented token to match, then applies `allow(module_id)` and the closed `module_name` enumeration. A peer identity with another provenance, no/mismatched proof, post-revoke proof, module 0, ids at or above 32, and unknown ids all fail. Replaying a token does not widen authority: a holder can repeat an already-authorized launch, which is consistent with this non-consumable advisory cap. A second `mint` for the same surface creates a duplicate table entry, but the production path mints once and `revoke` removes all matches; this is an API-hardening nit, not an observed bypass.

2. **Rendezvous possession.** The broker receives only on its own cap endpoint, so sending proof bytes on the surface channel cannot satisfy it. Arthur receives both the surface endpoint and LaunchCap endpoint with full channel rights and can delegate both to a child; such a child can launch only the same surface-bound allowlist. That is transferable capability authority, not amplification. Delegating only one endpoint cannot complete a launch. Missing proof causes the liveness finding above; extra/concurrent proofs can cause a token mismatch/refusal but cannot authorize an off-list module. Closing the cap wakes the broker receive with failure and yields status 7.

3. **Focus trap / wait behavior.** The withheld-proof path is a current-main blocker as described above. The broker waits only for `MODULE_PROBE`; the allowlist maps that id to the fixed no-handle `worker`, which exits. The non-probe branch does not wait. I found no client-controlled module name or allowed long-lived module in this rung.

4. **Pinned advisory test.** Arthur itself—not the WM broker—calls `sys_spawn("worker", &[])`, waits on the returned process cap, and logs the exit. Xtask requires the exact `direct_spawn=allowed exit=1` line after the broker grant/refusal sequence, so the assertion cannot pass merely because a brokered launch succeeded or because spawn failed.

5. **Wire layout.** `OP_LAUNCH` is opcode 8. The request is 24 bytes of envelope plus a 40-byte payload: identity at 24..48, little-endian module id at 48..52, zero reserved word at 52..56, and little-endian nonzero token at 56..64. Decode rejects module 0, nonzero reserved, token 0, and wrong length. Replies encode opcode `0x8008`; status 7 is `CapRequired`, 10 is `NotPermitted`, and 11 is broker spawn failure. The hand-laid request oracle agrees with these offsets independently of the codec.

6. **Kill-switch.** Launch protocol constants, request variants, client launch helper, broker module, session mint/serve code, and Arthur probes are gated by `atrium-launch` / `desktop-home`. With `desktop-home` off, the S3 session does not construct a broker, mint a cap, receive one in Arthur, or enter a launch match arm. The pre-existing desktop session/glass gate expectations remain separate and unchanged.

7. **Cross-family notes.** `LaunchBroker::judge` would accept a directly constructed zero-token request with `presented=Some(0)`; the only production caller receives a decoder-validated request, so this is not currently reachable, but rejecting zero again in `judge` would make the public pure API fail closed. Repeated successful probe launches also retain waited process handles until the session ends; bounded-handle exhaustion is possible for an authorized client, although that client can already exercise the stronger no-proof liveness attack.

## Verification

- RAG health: `healthy` (`ollama=true`, `qdrant=true`); semantic discovery was followed by direct source and syscall/IPC inspection.
- `cargo test -p atrium-logic-tests`: 95 passed, including the launch byte oracle and broker judgment KAT.
- `NEO_SMP=4 NEO_TEST_ATTEMPTS=1 cargo xtask neo-test desktop_home_boot`: all userspace and scenario-kernel builds passed. Execution stopped before QEMU because `/home/neo/limine` is absent in this Windows checkout.
- Inspected channel grant/close semantics: the cap is granted with full channel rights; a live same-side endpoint keeps the broker receive waiting, while explicit close/process exit wakes it with `EPIPE`.

