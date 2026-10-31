# ADR-022: Fault-recovery direction — forward marker for Theseus-style cell-swap

| | |
|---|---|
| **Status** | Accepted (forward marker; no architectural commitment) |
| **Date** | 2026-05-20 |
| **Authority** | PM |
| **Supersedes** | None |
| **Superseded by** | - |
| **Related** | Bible Articles 1.2 (correctness priority), 9.2 (loadable kernel module prohibition), 15.2 (unsafe discipline), 18.1 (informational artifacts); ADR-019 (SwitchMutex IRQ-off-by-construction — substrate-level discipline without architectural commitment); `docs/research/ALTIMIT-real-world-implementation.md` §F (Theseus consideration, informational) |

---

## Context

NeoPlutarch's primary fault-elimination strategy is "make the bug impossible." Bible Article 1.2 elevates correctness above coherence above performance. Article 15.2 enforces strict `unsafe` discipline. This approach has served the project well through Phase 3: bugs are prevented by construction, not recovered from at runtime.

However, Phase 4+ (network stack) and Phase 6+ (accelerator drivers) introduce fault categories where "graceful component restart" may be meaningfully better than panic:

- A network driver fault that panics the kernel loses all connection state, in-flight requests, and every tenant sharing that CPU. A graceful driver restart that preserves the rest of the kernel's state is strictly better for multi-tenant resilience.
- An accelerator driver fault that kills the kernel also kills inference contexts owned by other processes. A mechanism to restart the driver without killing unrelated inference work would be a material improvement for Phase 7+ multi-tenancy.

The ALTIMIT research report §F (informational input only) recommends Theseus-style cell-swap as the pattern for this class of fault recovery. Theseus cell-swap replaces a faulty kernel component at runtime by swapping in a pre-compiled replacement "cell" without rebooting the kernel — intralingual (same language, same address space) hot-swapping of kernel modules.

Bible Article 9.2 currently forbids loadable kernel modules. Its phrasing is broad enough to read as forbidding Theseus-style cell-swap as well. Resolving this tension would require a Bible amendment per Article 27.

This ADR does not resolve that tension. It records the consideration so the decision to defer is explicit and discoverable, rather than implicit and rediscovered.

---

## Decision

**AT THIS TIME (2026-05-20):** NeoPlutarch does NOT commit to Theseus-style cell-swap fault recovery. No Bible amendment to Article 9.2 is proposed. No engineering work toward cell-swap is on the phase roadmap.

This ADR records the consideration as a forward marker so the decision to defer is explicit and discoverable, rather than implicit and rediscovered.

---

## Conditions for revisit

The project would revisit the cell-swap question if any of the following concrete conditions materialize:

1. **Network-stack failure mode.** A Phase 4+ network-stack failure mode is identified where a driver panic costs significantly more (lost connection state, in-flight request loss across tenants) than a graceful driver restart would.
2. **Accelerator driver fault mode.** A Phase 6+ accelerator driver fault mode is identified where the alternative to graceful restart is killing in-flight inference contexts owned by other processes.
3. **Equivalent pattern emerges.** An equivalent fault-isolation design (e.g., Asterinas OSTD framekernel discipline, or another intralingual approach) becomes substantially easier to adopt than Theseus cell-swap and produces similar guarantees.

---

## Anti-decision

The following are explicitly NOT decided by this ADR:

- Whether the project will EVER adopt cell-swap. It might never.
- Whether Article 9.2 will eventually be amended. That is a separate Bible-amendment process per Article 27.
- The specific architectural shape of any future fault-recovery work.
- Whether Asterinas OSTD framekernel discipline (separate from cell-swap; concerns `unsafe`-boundary segregation) is also adopted. The research report names both; this ADR only addresses the cell-swap question.

---

## Rationale for deferral

Three reasons to defer rather than commit now:

1. **No current fault mode justifies it.** Phase 3's substrate (NeoFS, block layer, mount table) has not exhibited a failure mode that cell-swap would address better than panic-and-reboot. The "make the bug impossible" strategy remains sufficient for current work.
2. **Article 9.2 tension is unresolved.** Cell-swap sits in tension with the Bible's loadable-module prohibition. Resolving that tension requires a Bible amendment, which is a higher-bar process (Article 27) than a forward marker. Prematurely committing to cell-swap would force a Bible amendment before the project has a concrete failure mode to justify it.
3. **Phase roadmap does not require it.** Neither Phase 4 (network) nor Phase 6 (accelerators) is close enough that deferring this decision creates implementation risk. The conditions for revisit (§ above) will surface naturally as those phases mature.

---

## Related ADRs

- **ADR-019** (SwitchMutex IRQ-off-by-construction) — example of how the project handles substrate-level discipline without architectural commitment. ADR-019 records a specific implementation decision (which synchronization primitive to use and how) without committing to a broader threading model. ADR-022 similarly records that cell-swap has been considered, without committing to adopting it.

---

**Document end.**

*This ADR is informational per Bible Article 18.1. Ratification status is per Article 27 PL signoff. No normative content is introduced; no architectural commitment is made.*
