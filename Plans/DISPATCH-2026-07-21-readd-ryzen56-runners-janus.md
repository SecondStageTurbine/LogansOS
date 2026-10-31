<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Re-add ryzen-5/6 runners on the M75q → Janus

**Date:** 2026-07-21
**From:** PM (Claude)
**To:** Janus (main-tier; M75q runner-host owner)
**Status:** DISPATCHED — **infra/ops, NO gate** (re-register + prove). ryzen-5/6 were **de-registered** (removed via `gh api DELETE .../runners/{47,48}`, not just offline) as a workaround during Cerebro's P1 build — his branch was cut before the host-aware-preflight merge, so it ran the old flat-32-GiB guard and the 29.3 GiB M75q runners failed it non-deterministically as shards scattered. **That root cause is already fixed on `main` (`6f5f550`, the per-runner 28-GiB floor for ryzen-5/6).** So this is a straight re-registration; the poisoning won't recur for workflows cut from current `main`.

## Why now (it's on the critical path again)
Beyond restoring QEMU capacity, the M75q is the **Linux/amd64 ocloc host** the GPU glass lane depends on: **P1 Slice 3 compiles the blur kernel via `ocloc` in a Docker-Linux container** (ADR-077 v2 amendment 7 names <runner-host>). Cerebro's Slice 2 is dispatched now; Slice 3 needs this host live. Re-adding the runners restores both the gate throughput and the ocloc host in one step.

## Task
1. **SSH the M75q** (`<user>@<host>`, Windows 11, WSL2/Docker — the setup you built). The startup task `AltimitNeo-WSL-CI` should already keep WSL+Docker resident.
2. **Re-register the two runners** — `./config.sh` (fresh registration token from repo Settings → Actions → Runners; **PL provides it, no token in git**) for `ryzen-5` and `ryzen-6`, same as your original build: `self-hosted` label, 4 CPU / 14 GiB caps, restart-safe. Reuse your existing `~/actions-runner-*` config if present, else re-run config.
3. **Confirm the host-aware preflight is what runs** — current `main`'s `ci.yml` gives ryzen-5/6 the 28-GiB floor; a workflow cut from current main will pass the preflight on these boxes. (No `ci.yml` change needed — it's already merged.)

## Verification / close
- **Both runners ONLINE + idle** in the repo's Actions runner pool, labeled `self-hosted`.
- **Real end-to-end proof** (same bar as your original close): fire a `workflow_dispatch` of `ci.yml` (or reuse an in-flight gate) and confirm **a QEMU shard lands GREEN on ryzen-5 or ryzen-6** under the 28-GiB floor — not just "runner online." Paste the run URL + which shard landed where. No regression to ryzen-1..4.
- **Monitor note (not blocking):** the open question is whether the heaviest shards (RDRAND/crypto/`-cpu max`) thrash at the 14-GiB per-runner cap. If a heavy shard reds on ryzen-5/6 specifically (infra-signature, not code), note it in the RETURN — that's the datum that would tell us to drop the M75q to one 28-GiB runner instead of two 14-GiB. Don't pre-emptively change it; report the evidence.
- **Do NOT re-adopt the de-register-or-lower-the-guard workaround** — the host-aware preflight is the fix; the big-box 32-GiB floor stays untouched.
- Doc: if any step differs from your original runbook procedure, update `docs/runbook-ci-runner-capacity.md` (push to `codex/readd-ryzen56-janus`; PM cherry-picks). Otherwise a RETURN with the green-shard proof suffices.

## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-21-readd-ryzen56-runners-janus.md`. Your runbook: `git show origin/main:docs/runbook-ci-runner-capacity.md`. Work on the M75q (`<user>@<host>`).
- **One-line:** **Re-add ryzen-5/6** on the M75q — they were de-registered (removed) during Cerebro's P1 build (his stale branch ran the old flat-32 preflight; the host-aware 28-GiB fix is now on `main`, `6f5f550`). Straight `./config.sh` re-registration (PL supplies the token, no token in git), `self-hosted` label, 4 CPU/14 GiB, restart-safe. **Close proof = a real `workflow_dispatch` lands a GREEN QEMU shard on ryzen-5/6 under the 28-GiB floor**, no regression to ryzen-1..4. This also restores the **Linux ocloc host** that P1 Slice 3 needs. If a heavy shard thrashes at 14 GiB specifically, report it (the datum for one-28-GiB-runner vs two-14-GiB) — don't change it pre-emptively; don't re-adopt the de-register/lower-guard workaround.
- **RAG:** n/a for on-machine ops; repo refs are `ci.yml` + the runner-capacity runbook (cited above).
