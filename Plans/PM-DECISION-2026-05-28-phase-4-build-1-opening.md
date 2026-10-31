# PM Decision — Phase 4 BUILD-1 Opening

**Date:** 2026-05-28
**From:** PM (Cerebro)
**Status:** BUILD-1 PLAN OPEN — awaiting PL ratification
**Authority chain:**
- Phase 4 opened: `Plans/PM-DECISION-2026-05-28-phase-4-opening.md`.
- Design substrate (ratified): `Plans/PHASE-4-THINK-v0.2-2026-05-22-network-stack-design-direction.md` + `Plans/PL-RATIFICATION-2026-05-22-phase-4-think-v0.2-q1-q12.md` (Q1–Q12 ratified in full, plus pre-ratified Q-A and Q-B).
- Phase 3 formally closed: `Plans/CLOSE-REPORT-2026-05-28-phase-3.md`.
- Bible v1.7.2 with Phase 4 marked Active (Article 13 ledger; PM-DISPOSITION-2026-05-28-bible-v1.7.2-phase-ledger-amendment.md).

## 1. Substrate

`main` is published at:

```
697e911 ADR-024: Ratify Phase 8 Desktop Design System & Tech Stack
```

Phase 3 (NeoFS, block layer, real disk I/O) is formally closed and bare-metal-verified through BUILD-5b. Phase 2 capability infrastructure is verified by Phase 2 close (12-cpu hardware sweep, commit `ce2cba3`). THINK v0.2 is ratified in full. ADR-024 (Phase 8 desktop design system) is scope-orthogonal to Phase 4 and does not constrain BUILD-1.

Prerequisites per THINK v0.2 §7:
- **P1** Phase 3 formally closed — ✅ satisfied.
- **P2** Phase 2 capability infrastructure verified — ✅ satisfied (Phase 2 close).
- **P3** PL IPv6 network functional — PL environmental, assumed satisfied; PL flag if not.
- **P4** smoltcp wrap/fork/scratch decided at PLAN — **resolved in §4 Q1 below.**
- **P5** no Phase 3 unresolved blockers — ✅ satisfied.

## 2. THINK v0.2 Scope Anchor

Per THINK v0.2 §5, BUILD-1 ships IPv6 + UDP + virtio-net + the capability shape. Goal milestone (THINK §5, unmodified):

> A NeoPlutarch userspace process, holding a `NetworkAccess` capability, opens a `UdpSocket`, binds to a SLAAC-acquired IPv6 global address, sends a UDP datagram to a remote IPv6 host, receives the response. The reply path uses notification-on-activity to wake the process. Loopback short-circuit works for process-to-process UDP on `::1`. Privacy Extensions enabled; address rotation does not silently break long-lived sockets. The Phase 3 substrate is green.

THINK §8 already executed the perimeter `/simplify` pass (cut IPv4, TCP, DNS, HE, TLS, DHCPv4/v6, e1000e, USB Ethernet, namespacing, mDNS, IPsec, pluggable CC, completion-based I/O from BUILD-1). PLAN's `/simplify` work is the inner-loop pass: do not add machinery beyond what each ISC requires, do not re-litigate THINK's perimeter cuts (§6 below).

Sub-commits per THINK §5: BUILD-1a, 1b, 1c, 1d, **1.1** (PL-ratified Q-A), **1f** (PL-ratified Q-B), 1e. Seven sub-commits total. PLAN's job is to attach ISC criteria per sub-commit and resolve the three explicit PLAN-time decisions THINK handed forward.

## 3. Current Substrate Observations

Code-state observations relevant to BUILD-1, mirroring the BUILD-4-opening template's substrate-observation pattern:

- **No network module exists yet.** `kernel/src/net/` does not exist; the Bible's Article 17 source tree already names it. BUILD-1a creates it. No syscalls in the `0x180..0x1FF` network range are wired.
- **Drivers available for parallel reuse.** `kernel/src/drivers/block/{virtio_blk,ahci,nvme}.rs` provide the pattern for `kernel/src/drivers/net/virtio_net.rs`: PCI enumeration, MSI-X, DMA-buffer alloc, RX/TX ring management, completion-via-notification. The recent Phase-3-BUILD-5b AHCI release/acquire publication-fence discipline (commit `2fdb7cb`) applies to virtio-net descriptor submission as well — BUILD-1a inherits that lesson by reusing the same patterns.
- **NetDevice trait does not exist.** Parallel to `BlockDevice` (Phase 3 BUILD-1). BUILD-1a defines it.
- **Capability infrastructure ready.** Phase 2 capability table, cap-passing IPC (Article 12), and the notification primitive used for socket-activity and address-deprecation events are all live and bare-metal-verified.
- **Block-device log sink available.** BUILD-5b validated this end-to-end; BUILD-1 uses it for forensic capture during bare-metal verification (BUILD-1e) and Hop-by-Hop diagnostic logging (BUILD-1f).
- **No external network dependencies adopted.** smoltcp is *not* in `Cargo.toml` yet; adopting it is a §4 Q1 PLAN decision.

## 4. Load-Bearing BUILD-1 PLAN Decisions

THINK v0.2 explicitly handed three decisions to PLAN. Each carries a PM recommendation with rationale; PL ratifies the opening to accept or override.

### Q1 — smoltcp posture (P4 prerequisite; THINK §5 R1)

**Recommendation: Hubris-style wrap.** Adopt `smoltcp` as a Cargo dependency (v0.13.0 or current); build a kernel-owned scheduling shim around it; do not fork into the tree.

**Rationale:**
- smoltcp v0.13.0 (March 2025) shipped substantive content (IPv6 SLAAC, TCP improvements, Rust 2024). The "minimal maintenance" framing from asterinas#1821 is empirically out-of-date.
- Asterinas's smoltcp→Netstack3 migration debate closed as "not planned" (#1821). The production-Rust-bare-metal ecosystem has narrowed to smoltcp not by consensus but by walking away from migration — smoltcp's position as the Rust-native canonical choice (Bible Article 11.2) is strengthened.
- Hubris's `task/net` is the only documented "wrap" analog with similar concurrency posture (small netstack, smoltcp as Cargo dep, kernel-owned scheduling shim). Proven pattern.
- Fork costs ongoing maintenance debt with no commensurate benefit (we are not differentiating on IP-layer mechanics). From-scratch is a 12–18-month detour from the Phase 4 ship gate at BUILD-5; rejected on Article 1.4 (no parity-chasing — but conversely no NIH).
- Wrap satisfies Article 15.7 (lean patch rule): no machinery beyond what's needed.

**Residual risk (R1):** smoltcp's poll-loop assumptions may interact differently with NeoPlutarch's interrupt-driven scheduler than with Hubris's controlled service-processor scheduler. Mitigation: BUILD-1a includes an explicit smoltcp-integration smoke test (an Ethernet frame round-trip via the wrap) as ISC-1a-3 *before* BUILD-1b implements the IPv6 stack on top. Any wrap-shape obstacle surfaces at BUILD-1a, not BUILD-1b/1c.

**Rejected at PLAN:** fork into tree, from-scratch implementation.

### Q2 — RDNSS option processing placement (THINK §9 reserved)

**Recommendation: read at BUILD-1, defer use to BUILD-4.** BUILD-1b's SLAAC/RA-processing code lifts the RFC 8106 RDNSS option from inbound Router Advertisements and stores resolver addresses in a kernel-side `ResolverConfig` structure. The structure is *populated* at BUILD-1 but not *consumed* until BUILD-4's stub resolver wires it up.

**Rationale:**
- RDNSS is one TLV in RAs that are already being parsed for prefix info and lifetimes. Lifting it at BUILD-1 is small additive work (one option-type branch).
- Forcing BUILD-4 to re-enter and re-touch RA-parsing code adds friction and re-litigates BUILD-1b's `/simplify` boundary.
- The Bible's Article 15.7 lean-patch rule applies symmetrically: lifting now buys "BUILD-4 doesn't need to revisit", which is a real capability gain.
- The data is benign at rest (a small static structure with no consumer until BUILD-4); no behavior change visible from userspace.

**Rejected:** defer RDNSS lift entirely to BUILD-4 (would force BUILD-4 to re-enter the SLAAC code path).

### Q3 — Speculative TCP sketch depth (R16 mitigation; THINK §4.1 v0.2)

**Recommendation: syscall signatures + connection-state-machine composition outline.** Filed as a companion artifact: `Plans/PM-NOTE-2026-05-28-phase-4-build-1-speculative-tcp-shape-sketch.md`. The sketch enumerates `sys_net_socket_tcp`, `sys_net_listen_tcp`, `sys_net_connect_tcp`, `sys_net_accept_tcp` with their capability-handle return shapes, and steps the layered-handle model through TCP's RFC 9293 states (CLOSED → LISTEN / SYN-SENT → SYN-RECEIVED → ESTABLISHED → FIN-WAIT-1/2 → ... → TIME-WAIT) noting where the UDP-derived shape composes cleanly versus where TCP's connection-state model imposes new requirements (partial-write tracking, urgent-data, half-close semantics, listening-socket-yields-accepted-socket cap derivation).

**Rationale:**
- Light enough to actually do at PLAN time without dragging into BUILD-1 critical path.
- Deep enough to surface obvious API-shape obstacles (e.g., if TCP requires per-connection rights bits the UDP shape didn't anticipate, or if listening sockets need a cap-derivation primitive the capability subsystem doesn't yet support).
- Lives as a PM-NOTE (hypothesis-validation artifact, not a commitment). No kernel code emitted at PLAN time.
- If the sketch surfaces an obstacle: PLAN re-opens or BUILD-1c API surface is annotated with forward markers. If clean: BUILD-3 proceeds with the layered shape on a stronger basis.

**Rejected:** signatures-only (would miss state-machine implications); full state-machine implementation sketch (premature, drags PLAN scope).

### Q4 — `NetworkAccess` rights bitmap representation (THINK §9 reserved)

Deferred to BUILD-1c dispatch. Not load-bearing at the opening level; implementation detail decided when capability is wired into syscall plumbing.

## 5. Proposed BUILD-1 Sub-Commit Split With ISC Criteria

Seven sub-commits per THINK §5. Each carries scope, ISC criteria (numbered `ISC-1{x}-N`), and explicit anti-scope. ISCs are the verifiable pass conditions; sub-commit closes when all its ISCs hold.

### BUILD-1a — Substrate (`NetDevice` + virtio-net + smoltcp wrap)

**Scope:** Create `kernel/src/net/` and `kernel/src/drivers/net/`. Define `NetDevice` trait paralleling `BlockDevice`. Implement virtio-net probe + bringup using the Phase 3 PCI/MSI-X/DMA patterns. Adopt smoltcp as a Cargo dependency (per Q1) and wire a minimal kernel-owned scheduling shim around it.

**ISCs:**
- **ISC-1a-1.** `kernel/src/drivers/net/virtio_net.rs` exists; PCI vendor `0x1AF4` / device `0x1000` probed and bound; link state observed in serial log.
- **ISC-1a-2.** `NetDevice` trait defined in `kernel/src/net/device.rs`, paralleling `BlockDevice` in shape (alloc/free, submit, completion-via-notification, descriptor publication discipline matching the Phase 3 BUILD-5b release/acquire pattern).
- **ISC-1a-3.** smoltcp wrap smoke test: a raw Ethernet frame submitted via the wrap is delivered to virtio-net's TX ring; a frame received on RX is delivered up through the wrap. Loopback through QEMU's net backend confirms round-trip. **This ISC is the early gate for R1.**
- **ISC-1a-4.** Phase 3 sweep stays green: `NEO_SMP=4 --no-meta` passes without regression.
- **ISC-1a-5.** No syscall surface exposed to userspace at this sub-commit (kernel-internal only).

**Anti-scope:** IP layer, ICMPv6, SLAAC, UDP, capability gating, syscalls, loopback short-circuit — all in later sub-commits.

### BUILD-1b — IPv6 stack core (IP + ICMPv6 + ND + SLAAC + Privacy Extensions + RDNSS lift)

**Scope:** IPv6 packet receive/transmit path; ICMPv6 (Echo, ND, RA processing per RFC 4861); SLAAC stable + temporary global addresses per RFC 4862 + RFC 8981 with default lifetimes (TEMP_PREFERRED 1 day, TEMP_VALID 2 days, IDGEN_RETRIES 3, MAX_DESYNC 0.4×preferred); RFC 8106 RDNSS option lift into kernel `ResolverConfig` (per Q2, populated-but-unconsumed); Hop-by-Hop and other extension-header posture per BUILD-1f (parallel-developed; see §5 BUILD-1f below).

**ISCs:**
- **ISC-1b-1.** Kernel SLAACs a stable IPv6 global address from a real RA on the test network; address observable in `sys_net_stats`-equivalent kernel debug surface (precursor to BUILD-1d).
- **ISC-1b-2.** Kernel SLAACs a temporary IPv6 address per RFC 8981 with the THINK-specified default lifetimes; modified-EUI-64 *not* used for stable address (random per RFC 7217 or per implementation choice documented in dispatch).
- **ISC-1b-3.** ICMPv6 Echo Request/Reply works end-to-end (smoke; uses raw IP path, not UDP).
- **ISC-1b-4.** RA RDNSS option lifted into `ResolverConfig`; structure populated but no consumer yet; debug surface confirms storage.
- **ISC-1b-5.** All BUILD-1f ISCs hold (extension-header posture; see below — 1b and 1f land in the same dispatch since extension-header processing lives in the IPv6 receive path).
- **ISC-1b-6.** Phase 3 sweep green; BUILD-1a ISCs still hold.

**Anti-scope:** UDP datagrams (BUILD-1c), capability gate (BUILD-1c), loopback short-circuit (BUILD-1d), user-configurable Privacy Extension lifetimes.

### BUILD-1c — UDP + capability gate (`NetworkAccess` / `UdpSocket`)

**Scope:** UDP send/receive on IPv6. `NetworkAccess` outer capability + `UdpSocket` inner capability. Syscall surface: `sys_net_socket_udp`, `sys_net_send_udp`, `sys_net_recv_udp`, `sys_net_close`, `sys_net_wait` (notification-on-activity per Q7). `NetworkAccess` rights bitmap representation (the Q4 deferred decision lands here). Per-process socket limit 64. Decide and document syscall numbers within the `0x180..0x1FF` band.

**ISCs:**
- **ISC-1c-1.** Userspace process holding `NetworkAccess` opens a `UdpSocket`, binds to the SLAAC global address, sends a UDP datagram to a remote IPv6 host. Datagram observed leaving the interface.
- **ISC-1c-2.** Reply UDP datagram delivered via notification-on-activity; `sys_net_wait` wakes the process; payload readable via `sys_net_recv_udp`.
- **ISC-1c-3.** `UdpSocket` capability handle passes between processes via the standard cap-passing IPC; receiver can recv on it.
- **ISC-1c-4.** Process without `NetworkAccess` cannot create a `UdpSocket` (`sys_net_socket_udp` returns capability-denied).
- **ISC-1c-5.** Per-process socket limit 64 enforced; 65th open returns the configured error.
- **ISC-1c-6.** `NetworkAccess` rights bitmap decision documented in dispatch (rights for outbound, inbound, port-bind range, etc.).

**Anti-scope:** TCP, IPv4, DNS, raw sockets, multicast send/join (deferred), broadcast (N/A on IPv6).

### BUILD-1d — Loopback short-circuit + per-process accountability

**Scope:** Short-circuit `::1` at the IP layer per THINK §4.7 / Q6. Per-process socket counters (packets-in/out, bytes-in/out). `sys_net_stats(socket_handle)` syscall.

**ISCs:**
- **ISC-1d-1.** Process-to-process UDP on `::1` round-trips without touching virtio-net's TX/RX rings (verified via interface counter delta = 0 during the loopback exchange).
- **ISC-1d-2.** `sys_net_stats(socket_handle)` returns packets-in/out and bytes-in/out per `UdpSocket`; values monotonic and consistent with sent/received traffic.
- **ISC-1d-3.** Loopback MTU semantics documented in dispatch as an *intentional* difference from real interfaces (forward marker for Phase 8+ filter/tap decisions).
- **ISC-1d-4.** No regression on BUILD-1c UDP via virtio-net (loopback path is in addition to, not replacement of).

**Anti-scope:** Filter rules on loopback (Phase 8+), tcpdump-tap point (deferred), routing-table modification API.

### BUILD-1.1 — Capability + address rotation (PL ratified Q-A)

**Scope:** Address-deprecation-event notification capability; reference-counted address retention while capability-bound sockets exist. Integrates with the BUILD-1c notification primitive and the BUILD-1b SLAAC/Privacy-Extensions code.

**ISCs:**
- **ISC-1.1-1.** A process holding a `UdpSocket` bound to a temporary address receives a notification when that address moves preferred → deprecated. Notification arrives on the same primitive used for socket-activity (`sys_net_wait` distinguishes via event type).
- **ISC-1.1-2.** A deprecated address is *not* removed from the routing / source-selection state while any capability-bound socket references it. Per-address reference count includes capability-bound sockets in the count; address removal gated on count = 0.
- **ISC-1.1-3.** Reference count is integrated with the Phase 2 capability table (not a parallel POSIX-fd-style accounting). Cap drop decrements; cap pass increments.
- **ISC-1.1-4.** Discharges THINK risk R13.

**Anti-scope:** User-configurable Privacy Extension lifetimes (deferred per `/simplify`); per-process address-rotation policy override (Phase 8+ namespacing).

### BUILD-1f — IPv6 extension-header processing posture (PL ratified Q-B)

**Scope:** Hop-by-Hop SHOULD-process per RFC 8200 + RFC 9673 (deferred-with-fast-path-skip semantics: hosts process Hop-by-Hop if they implement an option, fast-path skip otherwise); explicit refusal of Routing-Type-0 per RFC 5095; defensive parser posture; explicit refusal of all other extension-header types at BUILD-1 (SRv6, RPL, etc.). Diagnostic logging via the block-device log sink (BUILD-5b infrastructure).

**ISCs:**
- **ISC-1f-1.** Hop-by-Hop options parsed per RFC 8200; unknown option-action bits handled correctly (skip / discard / discard-with-ICMP-Parameter-Problem); RFC 9673 fast-path-skip applies when no option is implemented by the host.
- **ISC-1f-2.** Routing-Type-0 packets dropped with a diagnostic log entry via the block-device log sink.
- **ISC-1f-3.** SRv6, RPL, Mobility-Header, and other unsupported extension headers explicitly refused with a diagnostic; receive path does not crash, leak memory, or descend into recursion regardless of malformed nesting.
- **ISC-1f-4.** Defensive parser audit documented in dispatch: length-field validation before parser advance; bounded iteration; malformed option chains rejected with diagnostic; no infinite loop or OOB read possible (audit-by-construction with test vectors derived from the CVE-2026-23442 / CVE-2026-43501 class).
- **ISC-1f-5.** Discharges THINK risk R11.

**Anti-scope:** SRv6 implementation (Phase 5+ if ever), RPL implementation (Phase 5+), Mobility-Header (out of scope indefinitely), Fragment header handling beyond RFC 8200 minimums.

### BUILD-1e — Bare-metal verification

**Scope:** Phase 4 BUILD-1 bare-metal validation. Target: virtio-net under VMware on the OptiPlex 7080 Micro (mirrors Phase 3 BUILD-5b's role for storage). Extended-duration session per BUILD-1.1's R13 mitigation. Block-device log sink captures forensic state.

**ISCs:**
- **ISC-1e-1.** BUILD-1 goal milestone passes on bare-metal: UDP IPv6 echo, SLAAC global + temp address, capability gating, loopback short-circuit, notification-on-activity, all end-to-end.
- **ISC-1e-2.** ≥24-hour continuous session: address rotation observed (preferred → deprecated → removed cycle on temp addresses); a long-lived `UdpSocket` survives rotation without break per BUILD-1.1.
- **ISC-1e-3.** Block-device log sink captures rotation events and is extractable post-run (parallel to BUILD-5b's `neo-extract-log` discipline).
- **ISC-1e-4.** All BUILD-1a–1f ISCs hold under bare-metal load.
- **ISC-1e-5.** No regressions in the Phase 3 storage sweep run on the same hardware.

**Anti-scope:** Intel I219-V real-hardware verification (deferred per Q9 ratified to BUILD-2b or BUILD-6 with e1000e). USB Ethernet validation (later BUILD). Cross-vendor virtio-net (acceptable but not required).

## 6. `/simplify` Inner-Loop Review

THINK §8 already ran the perimeter `/simplify` (cut IPv4, TCP, DNS, HE, TLS, DHCP, e1000e, USB Ethernet, namespacing, mDNS, IPsec, pluggable CC, completion-based I/O from BUILD-1). PLAN does *not* re-litigate those cuts. PLAN's `/simplify` is the inner loop:

- **No machinery beyond what each ISC requires.** Dispatches must justify every line on lean-patch grounds (Article 15.7).
- **No "while we're here" expansions.** No proactive TCP scaffolding, no proactive IPv4 scaffolding, no proactive DNS code. The speculative TCP sketch lives as a PM-NOTE artifact only — zero kernel code emitted at PLAN.
- **Privacy Extensions ship RFC 8981 defaults at BUILD-1.** No user-configurable knob. Per-process / per-namespace policy override is Phase 8+.
- **Sub-commits are sequential, not parallel.** Each closes before its successor dispatches — except BUILD-1f, which lands in the same dispatch as BUILD-1b because extension-header processing physically lives in the IPv6 receive path. This is the only intentional sub-commit fusion.
- **Speculative TCP sketch is bounded.** Syscall signatures + state-machine composition outline, ~3–5 pages, no implementation. If the sketch surfaces a real obstacle, it becomes its own dispatched investigation; PLAN does not pre-pessimize against unknown obstacles.
- **smoltcp wrap is thin.** The shim is exactly the scheduling-adapter glue, not a reimagining of smoltcp's API. If the wrap shim grows past ~200 lines, that is a signal to revisit Q1 (a fork-light or scratch path may be cheaper than a fat wrap).

## 7. Risk-Register Cross-Reference (THINK v0.2 R1–R16)

| Risk | Ownership in BUILD-1 |
|------|----------------------|
| R1 — smoltcp poll-loop fit | Posture chosen in §4 Q1 (wrap). Early-gate via ISC-1a-3 smoke test. Residual carried through 1b/1c. |
| R2 — Phase 2 capability infra | Verified by Phase 2 close. Not active in BUILD-1. |
| R3 / R15 — real-hardware quirks (Intel I219-V e1000e) | Not BUILD-1 scope (virtio-net only). Owned by BUILD-2b or BUILD-6. |
| R4–R10 | Per THINK v0.2 §6; unchanged here. |
| R11 — IPv6 extension-header attack surface | Discharged at BUILD-1f (ISC-1f-1..5). |
| R12 — kTLS async-crypto CVE cluster | BUILD-5 owns (Phase 4 ship gate). Not active in BUILD-1. |
| R13 — Privacy Extensions × capability lifetime | Discharged at BUILD-1.1 (ISC-1.1-1..4). |
| R14 — DoT/DoH trajectory | BUILD-4d owns. BUILD-1's RDNSS lift (Q2) is consistent with the BUILD-4d direction. |
| R16 — Hubris UDP→TCP precedent unevidenced | Partial-discharge at BUILD-1 PLAN via the speculative TCP sketch PM-NOTE (Q3). Residual to BUILD-3. |

## 8. Required Next Artifacts

In order:

1. **`Plans/PM-NOTE-2026-05-28-phase-4-build-1-speculative-tcp-shape-sketch.md`** — companion to this PM-DECISION, satisfies the THINK §4.1 v0.2 R16-mitigation requirement that BUILD-1 PLAN sketch the TCP API surface speculatively. Filed in the same commit as this PM-DECISION.
2. **PL ratification** of this PM-DECISION (in chat or as a ratification artifact). On ratification, BUILD-1a dispatch is authorized.
3. **`Plans/DISPATCH-2026-05-28-phase-4-build-1a-netdevice-virtio-net-smoltcp-wrap-jarvis.md`** — first implementation dispatch to Jarvis, carrying BUILD-1a's scope and ISCs. Not authorized before ratification.

Subsequent dispatches (BUILD-1b/1f, 1c, 1d, 1.1, 1e) follow as each preceding sub-commit closes. Per Phase 3 precedent, each sub-commit gets its own dispatch artifact.

## 9. Authorization Boundary

**Authorized now (on filing this PM-DECISION):**
- PM filing the speculative TCP sketch PM-NOTE (a hypothesis-validation artifact, no kernel code).
- INDEX update referencing this PM-DECISION and the TCP sketch.
- Source reading, design review, and artifact creation in support of PLAN closure.

**Authorized on PL ratification of this opening:**
- BUILD-1a implementation dispatch to Jarvis.
- Cargo dependency on smoltcp (v0.13.0 or current) per Q1.
- Creation of `kernel/src/net/` and `kernel/src/drivers/net/` module trees.

**Not authorized:**
- BUILD-1b/1c/1d/1.1/1f/1e implementation work before BUILD-1a closes (cadence-sequential per §6).
- BUILD-2 (IPv4) and beyond.
- TCP, DNS, TLS implementation code (their perimeter rules apply).
- I219-V real-hardware verification (deferred per Q9).
- Any kernel code prior to PL ratification of this PM-DECISION (other than what's already on `main`).

## 10. Open Points for Ratification

PL ratification on this PM-DECISION confirms or overrides:

1. **Q1 — smoltcp wrap** (PM recommends wrap; alternatives are fork or scratch).
2. **Q2 — RDNSS lift at BUILD-1, consumed at BUILD-4** (PM recommends; alternative is full defer to BUILD-4).
3. **Q3 — Speculative TCP sketch = signatures + state-machine outline** (PM recommends; alternatives are signatures-only or full sketch).
4. **The sub-commit ISC sets** in §5 (PL may add or refine ISCs per sub-commit).
5. **The fused BUILD-1b/1f dispatch** in §6 (the only deviation from one-dispatch-per-sub-commit; PL may require separation).

Silent ratification (PL says "ratified" or "go") accepts all four PM recommendations and the §5 ISC sets as written.

---

**End of PM-DECISION.**

*BUILD-1 PLAN open. Substrate confirmed. Three PLAN-time decisions resolved with PM recommendations and rationale. Seven sub-commits with ISC criteria. Speculative TCP sketch filed as a companion artifact. BUILD-1a dispatch is the next step pending PL ratification.*
