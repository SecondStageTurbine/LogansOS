<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: CI runner-capacity prequel — runbook + pre-flight RAM guard (the QEMU-effectiveness fix, formalized) → Oracle

**Date:** 2026-07-05
**From:** PM (Claude)
**To:** Oracle (main-tier coder). **Branch:** `codex/ci-qemu-effectiveness-prequel-oracle` off `origin/main` HEAD. **The consolidation pauses until this lands** (its own gate benefits).
**Status:** DISPATCHED — Oracle's find, scoped per his own reality-check: **the actual fix is host infra** (the `.wslconfig` RAM cap on the runner box — Docker currently sees 16 GB; the raise is applied by the PL via `wsl --shutdown` while the box is idle). The repo-side prequel is **Option 1**: (a) the capacity **runbook** in `docs/`, and (b) a **CI pre-flight RAM guard** so an under-provisioned runner fails FAST with an explicit infra verdict + remediation pointer — never again a mystery 70-minute cancel. Doctrine: the OT-35 precedent (infra is never a test verdict) plus the hermeticity mirror (a red gate must mean the code is wrong, never that the host was starved).

## ISCs
- **ISC-1 — The pre-flight guard.** A fast first step in the QEMU shard job (and the host job if it's also RAM-sensitive): read the container-visible memory total (`docker info` / `/proc/meminfo` — whichever the job actually experiences), assert `≥ THRESHOLD`, and on failure **exit 1 within seconds** with a message that (a) says INFRA, not test, (b) prints observed vs required, (c) points at the runbook (`docs/` path + the one-line remediation: raise `.wslconfig` cap, `wsl --shutdown` when idle, verify). **You set THRESHOLD from measurement** — what the 4-shard gate actually needs with headroom, stated in the runbook with its rationale. NOT `continue-on-error` — failing fast with correct attribution is the whole point.
- **ISC-2 — The runbook.** `docs/runbook-ci-runner-capacity.md` (CC-BY SPDX): the `.wslconfig` settings and why; the apply procedure (drain/cancel queued gates → `wsl --shutdown` → verify `docker info` MemTotal → resume); the symptom table (exit-137 runner loss, long-run cancels, shard-4 tail contention — note the OT-43 amplification link); the guard's threshold + how to change it if capacity changes.
- **ISC-3 — The demo run.** After the PL applies the infra fix: fire the full 4-shard `workflow_dispatch` on this branch. That single run proves BOTH the raised cap (shards run in the improved envelope) and the guard (it passed pre-flight on a correctly-provisioned box). If you can cheaply show the guard's failure path (e.g. a temporary threshold set absurdly high in one test push, then reverted), cite it; not mandatory if the logic is trivially readable.
- **ISC-4 — Hermeticity statement.** One paragraph in the RETURN: this changes NO scenario, NO shard composition, NO QEMU flags — gate coverage is untouched; the only behavior change is fail-fast attribution on under-provisioned hosts. (If your find ALSO suggests QEMU-level speedups beyond RAM, name them as follow-on candidates — don't fold them in here.)

## Sequencing (PL + PM actions interleaved)
1. **PL:** when the queued consolidation gate is drained or cancelled, apply the `.wslconfig` raise + `wsl --shutdown`, verify `docker info` shows the new total.
2. **Oracle:** land this prequel; ISC-3 demo run green.
3. **PM:** verify + merge; **the gate hold lifts** — Janus rebases and fires M4's gate; Oracle resumes the consolidation on top.

## Gate + close
- ci.yml is gate-affecting ⇒ the ISC-3 4-shard `workflow_dispatch` on this branch is mandatory (it doubles as the demo). Push-CI green too.
- **Close-gate `scripts/worker-close-gate.sh`**; RETURN: both green URLs + observed pre/post MemTotal + the threshold and its rationale + the ISC-4 hermeticity paragraph.
- **Disk reclaim** after merge.

---
## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-05-ci-runner-capacity-prequel-oracle.md`. **Branch:** `codex/ci-qemu-effectiveness-prequel-oracle` off `origin/main`.
- **One-line:** **Option 1 — runbook + pre-flight RAM guard.** Guard: first step of the QEMU job asserts container-visible MemTotal ≥ your measured threshold, exits 1 in seconds with an INFRA-not-test message printing observed-vs-required + the runbook pointer; no `continue-on-error`. Runbook: `docs/runbook-ci-runner-capacity.md` — the `.wslconfig` settings, apply procedure (drain → `wsl --shutdown` → verify), symptom table (exit-137 / long-cancel / shard-4 tail, note the OT-43 amplification link), threshold rationale. After the PL applies the cap raise, fire the 4-shard dispatch on your branch — one run proves the cap AND the guard. RETURN: both green URLs, pre/post MemTotal, threshold + rationale, and the hermeticity paragraph (no scenario/shard/flag changes — coverage untouched, only fail-fast attribution). Consolidation resumes after this merges.
