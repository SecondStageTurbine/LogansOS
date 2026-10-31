<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: `NetDialCap` — kernel-enforced outbound-network authority (close the ambient `SYS_NET_DIAL` hole) → Janus

**Date:** 2026-07-12
**From:** PM (Claude)
**To:** Janus. **Branch:** `codex/netdialcap-kernel-authority-janus`.
**Status:** DISPATCHED — **kernel-touching build.** Closes the ambient-authority gap **Janus himself found** in the OT-61 browser study: today `SYS_NET_DIAL` validates a name and directly installs a full-rights TCP socket — **any process can dial out**. Arthur's `Grant::NetDial { host, port }` is a userland policy record, not enforcement; a hostile renderer (or any compromised process) calls the syscall directly. **Develops NOW in parallel with OT-59; kernel pieces MERGE after OT-59's green gate** (stagger the gate-fire behind Cerebro/Oracle — three kernel lanes in flight, different subsystems: this one is the net/syscall-authority path).

## What to build
1. **A `NetDialCap` kernel object** — the capability that authorizes outbound dial, carrying its scope (host-pattern/port or equivalent — keep the scope model consistent with the existing `Grant::NetDial` shape so the grant store maps 1:1 onto the cap).
2. **`SYS_NET_DIAL` consumes the cap.** No cap (or out-of-scope dial) ⇒ the existing deny path (EACCES-class, the explain-never-vanish message discipline). The kernel checks; userland policy (Arthur/grants.json) becomes the *minting* layer, not the enforcement layer — Article 29.3's owner-readable-policy-mirror pattern, generalized.
3. **Migration so today's users stay green:** the existing HTTPS-path consumers (Arthur, the net scenarios) receive their `NetDialCap` at spawn/grant time through the normal cap-inheritance path — enumerate every current `SYS_NET_DIAL` caller (grep + RAG) and route each through a cap. **No scenario goes red because it silently relied on ambient dial** — if one did, that reliance is the bug being fixed; give it the cap explicitly.
4. **Attenuation + revocation:** the cap is attenuable (narrower host/port) and revocable through the existing revoke walk; the SDK self-attenuation norm (OT-44) applies — drop it when a phase no longer needs it.

## Standing-rule sections (all mandatory)
- **Kill-switch:** a build flag/mode where cap-enforcement is OFF ≡ today's ambient behavior **bit-for-bit** (the equality regression) — complexity provably switch-off-able while the new layer soaks.
- **Concurrency pitfall-bank consultation (OT-62, normative):** the cap-check on the dial path + any cap-table mutation — map the touched classes (CP-11/CP-13 on any new atomics; CP-1 lock-ordering if the cap check takes a lock on the syscall path; likely mostly N-A — state it, don't skip it).
- **Invariant candidate:** this is the *enforcement* face of the OT-61 lesson ("kernel-enforced authority, never a UI grant record"). If it earns a ledger invariant, name it and ship the guard in this build (the I-16 pattern); if it's covered by the existing cap-model invariants, say which.
- **Byte-oracle:** if the cap/scope crosses the syscall ABI as a serialized struct, ship the hand-laid-out byte fixture (ADR-028 rule).
- **Falsifiable scenarios (QEMU):** (a) a process **without** `NetDialCap` calling `SYS_NET_DIAL` is denied (the negative control — this is the scenario that would have caught the hole); (b) with an in-scope cap, the dial succeeds (the existing HTTPS fetch scenario, now cap-routed, stays green); (c) out-of-scope (wrong host/port) is denied; (d) post-revocation dial is denied.

## Scope guard
Kernel cap object + `SYS_NET_DIAL` gating + migration of existing callers + scenarios. Do NOT build the browser, the broker service, or per-origin browser policy (that's the browser program's later work — this build gives it the kernel primitive). Don't touch Cerebro's block/virtio path or Oracle's compositor surfaces. Preserve hermeticity — the new scenarios use the existing in-gate network fixtures, no live-internet dependency.

## Gate + close
- **Kernel-touching ⇒ the mandatory 4-shard `workflow_dispatch` QEMU gate to merge — AFTER OT-59 closes** (a red gate blocks regardless); develop + local-verify now; **stagger the gate-fire** (Cerebro's fix, Oracle's compositor, and this cannot stack on ~3 runners).
- `scripts/worker-close-gate.sh` (rebase first) + the two hard rules (green CI URLs; own `git show --stat`). PM AAG at merge: ABI lens (the cap struct), completeness lens (the negative-control scenario + "did every existing caller get routed?"), concurrency, I-14 (trivially — no ISR/DMA, state it).
- **RETURN:** branch + the cap object + the gated syscall + the caller-migration table (every `SYS_NET_DIAL` site → its cap source) + the 4 scenarios + the pitfall-bank table + the kill-switch equality + (once green) both CI URLs.
- **Disk reclaim** after PM confirms merge.

---
## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-12-netdialcap-kernel-authority-janus.md` (+ `git show origin/main:Plans/RESEARCH-2026-07-12-web-browser-FINDINGS.md` [your own finding — the ambient-`SYS_NET_DIAL` section]; `git show origin/main:docs/OPEN-THREADS.md` **OT-61** [the ratified browser program this serves] + **OT-44** [grant/attenuation norms]; `git show origin/main:docs/concurrency-pitfalls.md` [consult — normative]; grep/RAG the `SYS_NET_DIAL` implementation + every caller + `Grant::NetDial` + the grant store). **Branch:** `codex/netdialcap-kernel-authority-janus`.
- **One-line:** **Close the hole you found: make outbound dial a kernel-enforced capability.** Build the `NetDialCap` object (scope mirrors `Grant::NetDial`), make `SYS_NET_DIAL` consume it (no cap / out-of-scope ⇒ deny with the explain-never-vanish message), migrate every existing caller onto an explicitly-granted cap (enumerate them all — nothing stays green by ambient accident), attenuable + revocable per the existing model. Kill-switch: enforcement-off ≡ today's ambient behavior bit-for-bit + equality regression. Consult the pitfall bank (map or N-A, don't skip). Scenarios: no-cap denied (the negative control), in-scope succeeds (existing HTTPS scenario cap-routed), out-of-scope denied, post-revoke denied — hermetic, in-gate fixtures only. **Develop NOW; the QEMU gate + merge come AFTER OT-59 closes — stagger your gate-fire behind Cerebro's and Oracle's.** Close-gate ritual + both green CI URLs; PM AAG at merge. NOT in scope: the browser itself, broker services, per-origin policy — this is the kernel primitive they'll ride.
- **RAG:** use it for our surfaces (`SYS_NET_DIAL syscall tcp socket`, `Grant NetDial grant store`, `capability object revoke walk`, `https fetch scenario`). Health-check `Invoke-RestMethod http://127.0.0.1:8404/health`; if down, note it + fall back to grep/Read (never a blocker).
