<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 246 — PR #29 re-check — Jarvis

Reviewed head: `e4824383e7f48f8a6bb4429769ff6d5499962dc1`.
RETURN branch: `codex/pr29-recheck-jarvis`, from `origin/main` at
`42b222cda` (abbreviated). Read the latest `PM:` comment on PR #29
(2026-11-01 10:56:31 UTC), the relay 233 RETURN, and Section 233's original requirements.
Overall: **FAIL** — four earlier findings pass; item 4 remains incomplete and its fix introduces a cleanup failure.
All source references below are at the reviewed head.

1. **PASS — content has no dial capability.** `userland/neo-init/src/main.rs:8498`
   gives the root to the broker; `:8502` spawns a separate `fetch-content` with only its channel.
   `userland/fetch-broker/src/content.rs:81` checks the no-cap dial returns EACCES and
   handles 0–3 cannot dial. The serial oracle requires the refusal at
   `tools/xtask/src/main.rs:18476`. Egress inventory: SYS_NET_DIAL requires a current-process
   NetDial handle with USE + DIAL and matching host/port (`kernel/src/arch/x86_64/syscall.rs:5393`,
   `:5400`, `:5415`); legacy socket create/bind/sendto/recvfrom, TCP connect, and resolver
   return ENOSYS (`:1246`). Socket send needs an existing writable TCP socket and refuses
   the datagram fallback (`:5630`, `:5651`). Listen requires the wildcard root (`:5170`);
   accept requires an existing readable listener (`:5223`). Spawn/grant require an authorized
   donor (`:1931`, `:7955`), and content receives neither a socket nor dial authority.
   Derivation requires an existing parent and cannot amplify rights
   (`kernel/src/sched/process.rs:3377`, `:3380`, `:3427`). Broker replies are encoded bytes
   (`userland/fetch-broker/src/main.rs:86`). This verifies the submitted scenario; it is not
   a claim of production browser-launch integration.

2. **PASS — the earlier mapped/NAT64 counterexamples are refused.**
   `kernel/src/sched/net_dial_cap_logic.rs:83` refuses all mapped IPv6, `:87` compatible IPv6,
   and `:91`–`:119` both specified NAT64 prefixes, 6to4 and Teredo. The existing IPv4,
   unique-local, link-local and multicast rules remain (`:49`, `:112`). Policy shares these
   exact functions (`userland/fetch-proto/src/lib.rs:27`, `src/policy.rs:199`). The earlier
   `https://[::ffff:8.8.8.8]/` and `https://[64:ff9b:1::7f00:1]/` inputs are now regression
   fixtures (`userland/fetch-proto/src/policy.rs:344`). Kernel resolution filters the entire
   candidate vector before handing only the surviving vector to Happy Eyeballs
   (`kernel/src/net/happy_eyeballs.rs:108`, `:112`, `:125`); subsequent attempts cannot
   recover a refused candidate. The documented fixture-only NON_PUBLIC exception remains
   (`userland/fetch-broker/src/main.rs:388`).

3. **PASS — byte oracles remain and oversized channel prefixes are rejected.**
   `userland/fetch-proto/src/wire.rs:38` caps messages at 65,536 bytes and `:40` defines a
   65,537-byte receive buffer. Encode and decode enforce the aggregate limit (`:205`, `:287`).
   Both endpoints use the larger buffer (`userland/fetch-broker/src/main.rs:51`, `:65`;
   `src/content.rs:57`), so even a longer channel send truncated to that buffer is rejected.
   The exact previous 65,536-byte FETCH plus `0xff`, and a still-longer send's received prefix,
   are checked at `userland/fetch-proto/src/wire.rs:508`. Hand-laid FETCH, HEAD and small-message
   fixtures and malformed-input tests pass. Response headers are limited and trimmed to fit
   (`userland/fetch-broker/src/main.rs:355`, `:374`).

4. **FAIL — request-derived caps are nontransferable, but the original process-bound dial-right requirement remains unmet.**
   The new request root omits GRANT (`userland/fetch-broker/src/main.rs:104`), scopes preserve
   that ceiling (`kernel/src/sched/process.rs:3443`), and spawn/channel grant reject those
   request caps (`kernel/src/arch/x86_64/syscall.rs:1931`, `:7955`). That fixes request-cap
   forwarding. However, the broker still holds its original GRANT-bearing root at handle 1
   (`userland/fetch-broker/src/main.rs:43`; `kernel/src/sched/process.rs:258`, `:389`).
   Nothing prohibits deriving a fresh scope directly from that root and transferring it.
   Concrete sequence: in the broker, derive `phase4.test:4433` from handle 1, spawn a worker
   with that scope (or channel-grant USE + DIAL to a waiting recipient), then exit normally.
   Scope derivation preserves GRANT (`kernel/src/sched/process.rs:3443`), generic spawn/grant
   accept it (`kernel/src/arch/x86_64/syscall.rs:1931`; `kernel/src/sched/process.rs:3874`,
   `:3900`), and teardown reparents recipient edges instead of revoking them
   (`kernel/src/sched/process.rs:3123`, `:3166`, `:3173`). The recipient retains dial authority
   after the donor exits. The latest PM comment explicitly preserves this delegation behavior;
   calling the root launcher authority narrows the contract but does not implement Section 233
   item 4 as written. This is an existing capability policy, not a new graph regression.

5. **PASS — the earlier feature-off TCP/TLS changes are removed or gated.**
   `kernel/src/net/tcp.rs` is byte-for-byte identical to this PR's parent (`c97bb8569`), so
   the unconditional loopback-drop diagnostic is gone. The kernel's feature-off root rights
   and dial path remain the previous ones (`kernel/src/sched/process.rs:381`;
   `kernel/src/arch/x86_64/syscall.rs:5427`). `userland/libneo/src/tls.rs:42` gates the longer
   poll budget, `:345` preserves the feature-off ReadTraffic error, and `:413` preserves
   reading first on every request pass. The new TLS feature is enabled by the broker fixture
   worker (`userland/neo-worker/Cargo.toml:104`; `tools/xtask/src/main.rs:7026`). With it off,
   the original 512-poll limit remains. This is source-level behavior verification, not a
   binary-equality measurement.

New from the fix — **FAIL:** the request root also omits REVOKE (`userland/fetch-broker/src/main.rs:105`), inherited by each scope (`kernel/src/sched/process.rs:3443`), so cleanup's ignored `sys_revoke(scope)` returns EPERM (`kernel/src/sched/process.rs:3491`); concrete input: complete the fixture FETCH, then repeat it on the same live broker channel — each `Conn::close` (`userland/fetch-broker/src/main.rs:200`) leaves its scope and any narrowed leaf installed until process teardown; error-path revokes at `:161` and `:172` fail too. Retain REVOKE while removing GRANT and check cleanup results.

Validation at the reviewed head: `cargo test -p fetch-proto` — 29 passed, including
address counterexamples, all hand-laid wire fixtures, malformed messages and the exact
oversized-prefix regression; `cargo test -p cap-graph-logic-tests` — 9 passed. No source
changes were made. Capability transfer/lifetime and failed cleanup are source traces.
QEMU and xorriso are unavailable here; boot scenarios were not rerun. The green CI gate
reported in the PM comment is external evidence, not a local boot-test claim.
