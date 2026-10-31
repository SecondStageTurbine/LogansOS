# Dispatch - Phase 4 Network Stack OBSERVE Pass (Pre-THINK Substrate)

Date: 2026-05-20
Agent: Cerebro
Status: AUTHORIZED RESEARCH (OBSERVE-CLASS; INFORMATIONAL ONLY PER BIBLE ARTICLE 18.1)

Authority:

- PL signal 2026-05-20: "yes" to PM proposal that Cerebro perform a pre-THINK
  OBSERVE pass while Jarvis works BUILD-5a-NVMe
- `Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload.md` (PL's
  Phase 4 design input plus PM's engineering refinements; this is the substrate
  Cerebro's research either confirms, refines, or refutes)
- `Plans/PM-NOTE-2026-05-20-post-phase-3-sequencing-rung-by-rung.md` (rung-by-
  rung sequencing — Phase 4 THINK formally opens after Phase 3 closes; this
  OBSERVE is preparation, not THINK opening)
- `docs/bible.md` v1.6.0 Article 11 (Networking, current posture), Article 9.10
  (network driver phase placement), Article 12 (capability-based security),
  Article 14 (OBSERVE → THINK → PLAN → BUILD/VERIFY algorithm), Article 18.1
  (informational artifacts), Article 3 (no POSIX compatibility)
- ADR precedent: ANSPEC-001 was drafted during Phase 1.4 work, ANSPEC-002 v0.1.0
  was drafted before Phase 3 BUILD-3 closed; the project has precedent for
  spec/research work happening ahead of the phase it pertains to

Substrate:

```text
(active Jarvis lane: BUILD-5a-NVMe; substrate at commit 3da6563 plus 396585a)
```

## 1. Mission

Produce a Phase 4 network-stack OBSERVE artifact: research-and-survey, not
design-and-decide. The output becomes part of Phase 4 THINK's authority chain
when THINK formally opens (post Phase 3 closure per the rung-by-rung
sequencing decision).

The mission is to put real research depth behind the PM-NOTE's proposed
Phase 4 positions, so that when Phase 4 THINK formally opens, Cerebro (or
whichever agent dispatches THINK) is not starting from zero.

The artifact is OBSERVE-class per Bible Article 14: gather context, identify
constraints, surface design space. It is NOT THINK-class: it does not
enumerate design alternatives and choose between them. It maps the terrain;
the route-selection is THINK's job.

## 2. Scope

### 2.1 In Scope

**A. Survey existing kernel network stacks with attention to posture
similarities and differences from NeoPlutarch.** Concrete subjects:

- **smoltcp** — Bible Article 11.2 names it as a derivation candidate.
  Architecture, sockets-equivalent API, no_std posture, capability-shape
  fit, what would need to change to fit NeoPlutarch's capability discipline.
- **Fuchsia Netstack3** — Rust, capability-shaped, modern. Currently the
  closest production OS analog to what NeoPlutarch would be building.
  Architecture, how it composes with Zircon's capability model, what
  syscall surface it exposes through fdio.
- **Hubris** — Oxide's Rust microkernel-ish system. Networking is
  task-based with capability-oriented IPC. Architecture, how it differs
  from smoltcp in design.
- **seL4-derived systems** — LionsOS, CAmkES network components. How
  capability discipline composes with networking in formally-verified
  contexts.
- **Theseus OS** — relevant since ADR-022 named Theseus's cell-swap
  pattern as a forward marker. Does Theseus have a network stack?
  What shape?
- **Linux** as comparison only — what's being rejected, what's being
  borrowed (if anything). NOT a copy target per Bible Article 1.4.

For each: brief architectural summary, what NeoPlutarch could learn,
what would not transfer.

**B. Survey relevant RFCs and current IETF status.** Concrete documents:

- **RFC 6724** — Default Address Selection for IPv6. The deterministic
  algorithm that makes "IPv6-preferred" work consistently. Load-bearing
  for Phase 4 design.
- **RFC 8305** — Happy Eyeballs v2. The connection-racing algorithm for
  dual-stack endpoints. Load-bearing per PM-NOTE.
- **RFC 6434** — IPv6 Node Requirements (Dec 2011). Confirms IPsec is
  SHOULD-not-MUST. Look for any post-2026 updates that would change
  this; if RFC 6434 has been superseded, surface.
- **RFC 9293** — TCP, current consolidated spec (Aug 2022, supersedes
  RFC 793 and many extensions).
- **RFC 8200** — IPv6 specification, current.
- **RFC 8504** — IPv6 Node Requirements update.
- **RFC 8085** — UDP usage guidelines.
- **RFC 5961, RFC 7323** — TCP improvements (anti-blind-injection,
  high-performance extensions).
- **RFC 4861** — IPv6 Neighbor Discovery.
- **RFC 4862** — IPv6 Stateless Address Autoconfiguration (SLAAC).
- **RFC 2131, RFC 8415** — DHCPv4, DHCPv6.

For each: current status, what it normatively requires of an
endpoint OS, where NeoPlutarch's existing Bible posture composes.

**C. Survey capability-shaped socket API design space.** Concrete
patterns:

- **Fuchsia fdio** — how Fuchsia exposes sockets as file-descriptor-
  like objects backed by Zircon channels.
- **Hubris task IPC** — networking as task-to-task IPC with
  capabilities.
- **Capsicum** (FreeBSD) — capability-mode sockets, what's been
  validated.
- **seL4 IPC-as-syscall** — pure-IPC networking patterns.
- **Plan 9's network filesystem approach** — sockets as files in a
  hierarchical namespace, historical reference for non-BSD-sockets
  designs.

For each: what the capability protection actually protects, how
processes get initial network capabilities, what the syscall surface
looks like.

**D. Identify the design questions Phase 4 THINK will need to resolve.**
Non-exhaustive enumeration:

- Sockets-equivalent API shape (capability handles to what — channels,
  endpoints, connections?).
- In-kernel TCP/UDP/ICMP scope (TCP plus UDP plus ICMP, or minimum
  subset like UDP first?).
- TLS scope (kernel-resident, userspace library, hybrid kernel-side
  primitive plus userspace policy?).
- DNS resolver placement (kernel-resident, userspace, hybrid?).
- Hardware driver layer (Bible Article 9.10 names virtio-net for QEMU,
  e1000e for hardware; what's the trait surface, how does it compose
  with the BlockDevice substrate from Phase 3?).
- Network namespaces / multi-tenancy / per-process network capability
  scoping.
- Loopback semantics.
- Routing table representation and update model.
- Address auto-configuration (DHCPv4, SLAAC, DHCPv6 — which are
  in-kernel, which are userspace?).
- IPv6 Privacy Extensions (RFC 8981 — temporary addresses) — Phase 4
  or later?
- mDNS / link-local discovery — in scope for Phase 4 or deferred?
- TCP congestion control algorithm (CUBIC, BBR, Reno — which is the
  default?).

For each question: surface the design space and what existing posture
constrains. Do NOT choose.

**E. Identify where existing Bible/ADR posture pre-resolves design
space.** Surface explicitly:

- Bible Article 11.2: in-kernel stack; IPv4 + IPv6 with IPv6 default;
  not BSD sockets.
- Bible Article 11.3: TLS as kernel concern at least for authenticated
  model downloads; minimal audited crypto code, not roll-our-own
  primitives.
- Bible Article 9.10: virtio-net and e1000e as Phase 4 first-class
  drivers; Realtek RTL8169-class deferred.
- Bible Article 12: capability-based security; sockets-equivalent
  objects are capability-protected like every other kernel object.
- Bible Article 3: no POSIX compatibility; no BSD-sockets API.
- ADR-022 forward markers: any Phase 4+ failure mode where graceful
  recovery would significantly beat panic is a Theseus-revisit trigger.

For each: what's already decided, what's left for THINK to specify.

**F. Confirm or refute the PM-NOTE proposed positions.** The
PM-NOTE captured these proposed Phase 4 positions:

a. Dual-stack mandatory.
b. IPv6-preferred with Happy Eyeballs (RFC 8305) for connection
   establishment; RFC 6724 for address selection.
c. TLS as the security primitive; IPsec deferred to Phase 8+ opt-in.
d. NAT64/DNS64 transition support deferred to Phase 8+.
e. No BSD sockets.
f. Capability-protected sockets-equivalent.

For each: what current RFCs and current OS implementations say. Where
the PM-NOTE position is well-supported, confirm. Where current
practice has moved, surface the gap so the PM-NOTE can be updated
before Phase 4 THINK formally opens.

### 2.2 Anti-Scope

NOT in this OBSERVE:

- Design decisions. The artifact maps terrain; it does not choose
  routes.
- Implementation proposals (Rust crate selection, syscall ABI
  specification, trait surfaces).
- Bible amendment proposals. If research surfaces a gap, note it in
  the artifact for PM consideration; do not draft amendment text.
- ADR drafting.
- ANSPEC drafting. Phase 4 may eventually have an ANSPEC-NET or
  similar; that's a THINK + spec-drafting follow-on, not an OBSERVE
  output.
- Implementation-language choices, dependency choices, crate-version
  pins.
- Phase 5+ AI subsystem considerations (those are Phase 5 THINK's
  job; do not bleed into Phase 4).
- Performance benchmarking, latency targets, throughput targets.
  These are PLAN/BUILD concerns once design is set.
- Specific syscall numbers, error code allocations, or ABI commitments.

## 3. Stop Conditions

Stop and surface to PM before continuing if:

- Research reveals a hard conflict between PM-NOTE's proposed
  positions and current RFC/IETF state in a way that the PM-NOTE
  should be corrected immediately (i.e. a position the PM-NOTE
  proposes is materially wrong, not just refinable).
- A research thread starts requiring actual design decisions to
  proceed (not just enumeration of options).
- The artifact starts feeling like THINK-class work (enumerating
  decisions and choosing) rather than OBSERVE-class (mapping
  terrain).
- A research subject named in §2 turns out to be inaccessible or
  not real (e.g. Hubris doesn't have a real network stack at the
  shape relevant; surface and PM will refine the subject list).
- The artifact exceeds ~3000 lines. OBSERVE artifacts should be
  comprehensive but not exhaustive; if Cerebro finds himself
  writing a 5000-line survey, that's THINK substrate work
  masquerading as OBSERVE.
- Research surfaces something that suggests Bible Article 11 itself
  needs revision before Phase 4 THINK opens. Surface; do not edit
  the Bible.

## 4. Required Close Artifact

Create:

```text
Plans/OBSERVE-2026-05-20-phase-4-network-stack-survey-cerebro.md
```

(Or later date if research spans multiple days.)

Required content:

- substrate state at start of research;
- §2.1.A existing-kernel survey;
- §2.1.B RFC survey with current status per document;
- §2.1.C capability-shaped socket design space survey;
- §2.1.D design questions enumeration (no decisions);
- §2.1.E pre-resolved design-space enumeration;
- §2.1.F PM-NOTE position confirmation/refutation;
- explicit list of suggested-corrections to the PM-NOTE if research
  found any;
- explicit list of design questions surfaced for Phase 4 THINK to
  resolve;
- references list (RFCs cited, OS/codebases reviewed, papers/blogs
  if any);
- explicit confirmation: "this artifact is OBSERVE-class per Bible
  Article 14; it maps terrain and does not choose routes. It is
  informational per Bible Article 18.1; it does not bind the
  project. Phase 4 THINK is the natural authority for resolving the
  design questions enumerated herein."

## 5. PM Posture

PM monitoring. Lanes after this dispatch:

- **Jarvis:** BUILD-5a-NVMe active (Phase 3 implementation lane).
- **Cerebro:** Phase 4 network stack OBSERVE pass (this dispatch;
  parallel lane).
- **Prometheus:** idle.

The two active lanes have zero shared substrate. Jarvis works the
storage driver; Cerebro researches networking. No coordination
required.

When Cerebro closes the OBSERVE artifact, PM consumes. The artifact
becomes part of Phase 4 THINK's authority chain when Phase 4 THINK
formally opens (post Phase 3 closure). Between now and then, the
artifact is informational reference.

Expected turnaround: Cerebro's prior research artifact (ALTIMIT
real-world implementation) was substantial; this network OBSERVE is
similar in shape but tighter in scope. PM does not impose a hard
timeline; quality over speed per Bible Article 1.2.

If Cerebro completes before Jarvis closes BUILD-5a-NVMe, PM consumes
the OBSERVE artifact at the same disposition cycle as BUILD-5a-NVMe.
If Cerebro takes longer, PM consumes asynchronously when the
artifact lands. No dependency between the two lanes.

PL retains the option to redirect Cerebro to other research lanes if
priorities shift. This dispatch does not commit Cerebro for any
specific duration; he closes when the OBSERVE is comprehensive
enough.

## 6. Boundaries (Reinforcement)

Cerebro should treat this dispatch as research authorization, not
design authorization. The temptation in network-stack work is to
start choosing — "the right TCP congestion control is CUBIC,"
"the sockets API should look like X." Resist.

The artifact's value at this stage is breadth and clarity, not
decisiveness. THINK's value is decisiveness, and THINK is later.

If the artifact starts containing prescriptive language ("Phase 4
SHOULD adopt..."), Cerebro should rephrase to descriptive
("Phase 4 THINK will need to choose between A and B; A is
favored by [evidence]; B is favored by [evidence]; the trade-off
is..."). The grammatical mood matters: indicative for what exists,
interrogative for what THINK must answer, never imperative for
what NeoPlutarch should do.

Bible Article 18.1 governs the artifact's status. Phase 4 THINK
governs the design.
