<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Arthur (Article 29) prep — the userland/serving-surface arc + Arthur's architecture → Prometheus

**Date:** 2026-06-16
**From:** PM (Claude)
**To:** Prometheus (deep-research/design lane). **Fresh** branch off `origin/main` HEAD: `prometheus/arthur-userland-arc-design`.
**Status:** DISPATCHED. **Design + research only — NO production code, NO `ci.yml`.** Output is one THINK doc under `Plans/`. Non-colliding (touches only `Plans/`). This maps the path from *inference-complete* to *a usable Arthur*, per the roadmap audit (2026-06-16) which confirmed **nothing skipped** but flagged the **serving-surface/userland arc** (esp. a userspace shell + the capability-action surface) as the real not-yet-built prerequisite.

## 0. The decision this sets up
Arthur (Article 29) is "natural-language command + capability-bounded autonomous action," **in userland** (Articles 6.6, 23.1, 23.3 — kernel stays model-agnostic, no resident persona, no telemetry). Building Arthur needs **Phase 8 complete = two arcs**: (1) inference maturity (8.1→8.5, in progress → first token generation) **and** (2) the **serving-surface + userland arc** — a shell/command surface to *invoke* Arthur and a capability-bounded action surface for Arthur to *act*. The audit found arc (2) **un-started** (bible:222; Article 28.2 "reach a working userspace shell"). This dispatch designs arc (2) + Arthur's architecture on top, so the path is sequenced before we build it.

## 1. What to work out (ground every claim in the actual code/articles — file:line)
- **The userland substrate that EXISTS vs is MISSING.** On `main` today: ring-3 processes + ELF loader (Phase 1.4), multi-process + IPC (Phase 1.5), the capability system (Phase 2), the inference syscalls (`sys_infer_run` + model load, Articles 20–22, Phase 5), FS (Phase 3), net (Phase 4/7.5). **Missing:** a **shell / command interface** (the thing a user types into) and the **capability-bounded *action* surface** (how Arthur effects changes — spawn, file ops, device/Arthur-grantable capabilities — all capability-gated). Confirm each by reading the tree; cite what's there.
- **The shell/command surface (arc 2a).** What is the minimal command surface Arthur needs to be *invoked* and to converse? A kernel-adjacent REPL vs a userland shell process vs a terminal/line-discipline. What's the smallest sufficient thing (Rule-3) that satisfies Article 28.2 ("boot, log in, type") without over-building a full POSIX shell?
- **The capability-bounded action surface (arc 2b).** Article 29.3: Arthur "can reach only what it has been granted." Map NL-command → intent → **capability-checked action**: what actions are in the v1 surface (file read/write, process spawn, query state, network), how each is gated by a granted capability, and how Arthur's process is granted/denied authority. This is the *security spine* of Arthur — design it against the existing capability system, not a new one.
- **Arthur's architecture (the app).** A userland process that: loads a model (the importer/`.neom`/GGUF path), runs inference (`sys_infer_run` once 8.5 lands), parses NL → intent, and executes capability-bounded actions. The NL→intent→action loop; how the *model* (which just emits tokens) drives *actions* safely (tool-calling/intent-extraction bounded by capabilities — the model proposes, the capability system disposes).
- **The constitutional constraints:** userland-only (6.6/23.1), model-agnostic kernel (23.3), no telemetry (23.2), friendly/practical character (29.1). Note how the design honors each.

## 2. Deliverable
`Plans/THINK-2026-06-16-arthur-userland-arc-and-architecture.md`:
- the EXISTS-vs-MISSING substrate table (file:line);
- the shell/command surface design (minimal-sufficient, with a recommendation);
- the capability-bounded **action** surface (the action list + the per-action capability gating + how Arthur is granted authority) — the security spine;
- Arthur's app architecture (the NL→intent→capability-checked-action loop; how token output drives bounded actions);
- a proposed **Phase-8-arc-2 + Arthur build slicing** (e.g., a minimal shell → the action/capability surface → Arthur-as-process wiring NL→inference→action), each kill-switchable, each with its gate;
- **open decisions for PM/PL ratification** before any of it is dispatched.

## 3. Scope guard
Design + `Plans/` THINK **only**. **Do NOT** write production code, touch `ci.yml`/kernel/`ai-ops-tests`, or pre-empt 8.4/8.5 (inference must complete first). Ground every "exists/missing" claim in the actual tree + the cited articles — **no memory**. Raw-source nothing external is needed (Arthur is our own design); cite the bible articles + code by file:line. **⚠ PUSH the branch before reporting done.**

---
## Relay — what to tell Prometheus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-16-arthur-userland-arc-prep-prometheus.md` (`git show origin/main:<path>`).
- **Branch:** `prometheus/arthur-userland-arc-design` off `origin/main` (fresh).
- **One-line:** Design **the path from inference-complete to a usable Arthur** (Article 29) — the audit confirmed nothing's skipped but flagged the **serving-surface/userland arc** as the gating not-yet-built prereq. Read the tree (file:line, **no memory**) for what EXISTS (ring-3/ELF, IPC, capability system, inference syscalls, FS, net) vs MISSING (a **shell/command surface** to invoke Arthur, the **capability-bounded *action* surface** for Arthur to act). Design: the minimal-sufficient shell (Article 28.2), the **capability-gated action surface** (the security spine — Arthur reaches only what it's granted, Article 29.3), and Arthur's userland app architecture (NL→intent→capability-checked-action; the model *proposes*, the capability system *disposes*; userland-only/model-agnostic/no-telemetry per 6.6/23). Deliver `Plans/THINK-2026-06-16-arthur-userland-arc-and-architecture.md` with a build slicing + open decisions to ratify. **Design doc only — no code.** Push before reporting done.
