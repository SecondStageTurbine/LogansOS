# PM-NOTE — CVE-pending "CIFSwitch" Relevance to NeoPlutarch Phase 4

**Date:** 2026-05-28
**From:** PM (Cerebro-seat)
**Status:** Analysis artifact — design substrate for BUILD-1c and BUILD-5 PLAN consumption. Not a design change to any ratified position.
**Trigger:** PL request 2026-05-28 to assess relevance of the CIFSwitch advisory (heyitsas.im/posts/cifswitch/, published 2026-05-27).
**Cross-references:**
- `Plans/PM-NOTE-2026-05-28-cve-2026-31431-copy-fail-relevance-to-phase-4.md` (same trend, different surface).
- `Plans/PHASE-4-THINK-v0.2-2026-05-22-network-stack-design-direction.md` (R11/R12 risk register; capability shape).

## 1. The Story In Brief

CIFSwitch is a distro-specific Linux LPE (CVE assignment pending as of 2026-05-27) in the kernel's CIFS/SMB subsystem and the userspace `cifs-utils` helper. Discovered by Asim Viladi Oglu Manizada using an LLM harness with graph-traversal tooling (an evolution of the same approach that found Copy Fail; the author cites Copy Fail as motivation).

The latency: the kernel-side bug has been present since 2007. Composing the exploit required reasoning across five subsystems (kernel CIFS, kernel keyrings, request-key, cifs.upcall, NSS) and identifying one missing validation hook.

**Mechanism, condensed:**

1. Kernel CIFS uses `request_key("cifs.spnego", description, ...)` to obtain Kerberos/SPNEGO material for an authenticated mount. The `description` is a semicolon-delimited string containing kernel-asserted fields: `uid`, `creduid`, `pid`, `upcall_target`, etc.
2. The `cifs_spnego_key_type` did **not** define a `.vet_description` hook. The kernel keyring subsystem therefore accepted *any* description for that key type, regardless of origin.
3. An unprivileged userspace process calls `request_key("cifs.spnego", forged_description, ...)` with attacker-controlled `pid` and `upcall_target=app`.
4. `/sbin/request-key` runs as root per the default rule (`/usr/sbin/cifs.upcall %k`), launching the upcall helper.
5. `cifs.upcall` parses the description as if kernel-generated. `switch_to_process_ns(arg->pid)` enters the attacker's mount namespace.
6. Inside that namespace: a forged `/etc/nsswitch.conf` pointing at `libnss_pwn.so.2`. Before privilege drop, the helper calls `getpwuid(uid)` → NSS lookup → loads the attacker-controlled `.so` as root → drops `sudoers.d` config → game over.

**The fix:** add a 5-line `.vet_description` hook that rejects descriptions when `current_cred() != spnego_cred` (i.e., only kernel CIFS using its private credential can produce legitimate descriptions). Trivial once you see it; latent for 19 years because composing it requires multihop reasoning across kernel + cifs-utils + NSS + namespaces.

## 2. Is NeoPlutarch Exposed Today?

**No, on three independent grounds:**

- **No CIFS, no SMB, no Kerberos** in NeoPlutarch's Phase 4 perimeter. No analogous subsystem.
- **No Linux-style keyring subsystem** with the "kernel passes a string description that userspace re-parses" shape.
- **No rootful userspace helpers.** Per Bible Article 12.2 (no root user; privilege is per-capability, not per-uid), NeoPlutarch has no `request-key`-equivalent privileged daemon. Capability-gated invocation replaces the rootful-helper pattern entirely.

The specific surface CIFSwitch attacks does not exist in NeoPlutarch's design. The exploit chain (`request_key()` → `cifs.upcall` as root → namespace switch → NSS hijack) has no analog.

## 3. The Pattern Lesson — Where It Actually Lands

CIFSwitch is, structurally, a **cross-boundary trust failure**. The kernel produced a structured input (the key description string), passed it through a shared subsystem (the keyring), and a downstream consumer (cifs.upcall) treated the input as kernel-authoritative without verification that the input *actually came from the kernel*. The keyring subsystem was the integrity gap — no `.vet_description` meant anyone could forge the input.

The lesson generalizes. Anywhere a NeoPlutarch syscall accepts a structured input from userspace and a downstream consumer treats fields of that input as authoritative, the same class of bug is reachable unless the kernel explicitly distinguishes "authoritative kernel-generated field" from "untrusted userspace-supplied field."

**Concrete implications, by sub-commit:**

### BUILD-1c (UDP + capability gate; `sys_net_socket_udp`, `sys_net_send_udp`, etc.)

This is the first NeoPlutarch syscall accepting structured userspace input into a network context. The lesson:

1. **The capability rights bitmap is the SOLE authority on what a holder may do.** There is no other "looks-trusted" channel. No string-parsed permission flags, no kernel-side "if the caller has CAP_X-like privilege" check that bypasses the capability.
2. **`bind_spec` validation.** When userspace calls `sys_net_socket_udp(NetworkAccess) -> UdpSocket` followed by an implicit or explicit bind, the bind parameters (address family, port range, scope) must be validated against the rights encoded in `NetworkAccess`. The kernel does NOT trust the userspace bind parameters; it intersects them with the cap's rights.
3. **No mixed-provenance structured inputs.** If a syscall accepts a struct from userspace, every field in that struct is *untrusted*. The kernel never has a "this struct came from a privileged context, trust its kernel-context fields" pathway. The CIFSwitch mechanism (`pid` field in the description trusted because the description allegedly came from kernel CIFS) has no analog because we don't build the analog.
4. **Capability-derivation primitive design (Forward Marker FM-1 from the speculative TCP sketch).** When BUILD-3 introduces `sys_net_accept_tcp(TcpListener) -> TcpSocket`, the derivation is kernel-side only. Userspace cannot fabricate a derived cap, period. The kernel's mint-derived-cap primitive must be the only path.

### BUILD-5 (kTLS-hybrid; `sys_net_tcp_enable_tls(TcpSocket, KeyMaterial)`)

This is the most cross-boundary-trust-sensitive syscall on the Phase 4 roadmap. Userspace (libneo + rustls) does the TLS handshake and hands the record-layer key material into the kernel. The kernel then operates on that material under userspace pressure (send/recv).

The CIFSwitch lesson at BUILD-5 PLAN time:

1. **Key material is opaque to the kernel except as cipher input.** The kernel does not parse handshake-derived metadata. No "this key was negotiated with peer X so treat it as having property Y" pathway.
2. **Per-record state is kernel-private after `enable_tls`.** Userspace cannot read or modify the record-layer state once enabled. (This intersects with the Copy Fail PM-NOTE's no-userspace-modifiable-memory-under-kernel-crypto principle.)
3. **The `TcpSocket` capability holder's rights at enable-time govern.** A holder with READ-only on the cap cannot enable TLS (the encrypted-write side requires WRITE). FM-2 from the speculative TCP sketch (reserve READ/WRITE bits independently in the inner-cap bitmap) becomes more load-bearing — TLS state interacts with rights independence.

## 4. The AI-Discovery Angle — Same Trend, Confirmed

Copy Fail (2026-04-29) was AI-found by Theori's Xint Code. CIFSwitch (2026-05-27) was AI-found by an independent researcher using an LLM harness with graph-traversal tooling (the GraphWalk paper's approach, applied to vulnerability hunting). Both used multihop composition across multiple subsystems. Both had latencies of 9 and 19 years respectively. Both are the first wave of a category we will see more of.

NeoPlutarch's defenses-by-construction (Rust memory safety eliminating bug class 1; capability gating eliminating bug class 2; no-rootful-helpers eliminating bug class 3) are the right shape, but they need to be **verified against multihop composition attacks**, not just reviewed in isolation. The CIFSwitch fix was a 5-line patch — but spotting it required composing knowledge across 5 subsystems. NeoPlutarch's reviewers (PM, Jarvis, Cerebro, Prometheus) must consciously model multihop attack chains, not just per-syscall correctness.

**Concrete implication for project process:**

The BUILD-1c PLAN-time review (when BUILD-1c opens) should include an explicit "cross-boundary composition" gate: enumerate the syscalls that take structured userspace input, name every field that crosses a trust boundary, and verify each field is treated as untrusted at every consumer. This is the same discipline that ANSPEC-001 §13.2 names but in operational form for the network stack.

## 5. What This PM-NOTE Does NOT Do

- Does NOT change any ratified Phase 4 position.
- Does NOT modify the BUILD-1 PLAN, the BUILD-1a worker dispatches, or any sub-commit ISC.
- Does NOT modify the Copy Fail PM-NOTE — that artifact's guidance for BUILD-5 stands; this artifact extends the same lesson to BUILD-1c.
- Does NOT propose any Bible amendment or ADR. The substance is operational substrate for BUILD-1c and BUILD-5 PLAN.

## 6. Disposition

Filed alongside the Copy Fail PM-NOTE as Phase 4 PLAN-time substrate. BUILD-1c PLAN (when opened after BUILD-1a closes) inherits the §3 BUILD-1c sub-section as a pre-condition checklist. BUILD-5 PLAN inherits §3 BUILD-5 alongside R12 and the Copy Fail PM-NOTE.

Two AI-found multihop LPEs in one month is the trend Bible Article 23 was written for. NeoPlutarch's posture (no rootful helpers, capability gating, structured-input-is-always-untrusted) is the correct response; the discipline is in execution, not in re-architecting.

---

**End of PM-NOTE.**

*CIFSwitch is structurally a cross-boundary trust failure. NeoPlutarch has no analogous surface today. Lesson applies to BUILD-1c (capability rights as sole authority; no mixed-provenance structured inputs) and BUILD-5 (kernel-private TLS state; cap-rights govern enable). No ratified position changes.*