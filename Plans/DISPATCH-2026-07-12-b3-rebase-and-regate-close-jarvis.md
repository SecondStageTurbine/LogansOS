<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: B3 close — rebase onto the OT-59-fixed `main` and re-gate → Jarvis

**Date:** 2026-07-12
**From:** PM (Claude)
**To:** Jarvis. **Branch:** `codex/ot16-b3-qwen35-deltanet-bringup-jarvis` (unchanged — the existing B3 branch, currently head `82f33270`).
**Status:** DISPATCHED but **GATED — do NOT start until the PM confirms BOTH OT-59 fixes are merged to `main`.** B3's deliverable is already proven (parity `c536c7a` + fail-closed generation oracle `175db85`); this is purely the close ritual, held until the tree it merges onto is green.

## The one precondition (why this is gated, not startable now)
B3 cannot pass its `workflow_dispatch` QEMU gate until **both OT-59 defects are off `main`**, because the gate re-runs the *whole* suite and would redden on `swapgs_irq_ring3` (Jarvis's other lane) and the spawn/wait-completion flake family (Cerebro) **regardless of B3**. Re-gating B3 before then just burns ~4 fleet-hours on a foregone red. **Wait for the PM's go** — the PM merges the two fixes, confirms `main`'s gate is green, then releases this close. (The PM will name the exact `main` tip to rebase onto in the go message.)

## The close (the ritual — one pass, in order)
1. **Rebase onto the fixed `main`.** `git fetch origin && git rebase origin/main`. **This is NOT docs-only anymore** — the two OT-59 fixes changed kernel files, so expect real rebase interaction in the scheduler / `qwen_inventory.rs` / process-lifecycle areas. Resolve conflicts carefully; the GDN math and `.neom` v4 path must survive untouched.
2. **Re-prove the deliverable on the rebased tree** (the rebase moved the base — the old evidence was on `89a9cc8`):
   - **Host parity oracle** — re-run; top-1 parity vs llama.cpp `GGML_OP_GATED_DELTA_NET` must still hold bit-for-bit (Q5_K preserved-raw, 128-lane reduction). The OT-59 fixes are scheduler/completion-path, so parity should be untouched — but *prove* it, don't assume.
   - **Qwen2.5 unchanged** — the kill-switch: Qwen2.5 paths bit-for-bit identical (the double kill-switch equality).
3. **`scripts/worker-close-gate.sh`** — rebase-freshness (now clean: base == `origin/main` tip), the full CI host set in CI order via Docker, SPDX, and the Step-4 self-`git diff --stat` vs `main` (inspect it — no phantom deletions).
4. **Fire the mandatory `workflow_dispatch` QEMU gate on the exact rebased head** (kernel-touching ⇒ this is required; push CI is not the gate). Confirm **all 4 shards + host + SPDX green**. The `arthur_real_brain_boot_qwen35`/generation oracle must PASS in-kernel (`qwen35_generation_oracle` observed, `cross_token_state_carried=true token_oracle=32`). If any of the previously-flaky spawn/wait scenarios flakes again, that means Cerebro's fix was incomplete — **stop and report**, don't retry-until-green (a retried-green here would mask a still-live race).

## RETURN (paste the evidence block)
- The rebased head SHA; the **green push CI URL** AND the **green `workflow_dispatch` QEMU URL** on that exact head (both — "green without a CI URL is not done").
- The re-proven parity evidence (rebased) + the in-kernel generation-oracle PASS line.
- The Step-4 self-`git diff --stat` vs `main`.
- The B3 AAG-input notes for the PM's 5-lens panel: **completeness** = the in-kernel generation oracle; **ABI** = the `.neom` v4 format + Q5_K-preservation byte-oracle; **memory-safety** = the owned-`Vec` GDN state (zero-new-`unsafe` claim, re-confirmed post-rebase); **I-14** = no governance lock / no DMA in the GDN path; **concurrency** = sequential decode.

## PM merge (what happens after your RETURN)
The PM runs `scripts/pm-merge-check.sh origin/codex/ot16-b3-qwen35-deltanet-bringup-jarvis` (expect ff-safe after the rebase) + an independent CI re-fetch on the exact head_sha + the own-`git show --stat` inspection + the **5-lens AAG** (this is a kernel-touching merge). On clean: `--ff-only` merge to `main`, push. **Note:** this merge also lands B3's CI staging patch (host 60 / QEMU 240 + staged QEMU) on `main` — the correct interim tourniquet that OT-58's tiered gate later retires.

**Disk reclaim (after PM confirms merge):** once B3 is on `origin/main` and confirmed, delete this build's tree to reclaim its `target/` (verify the work is on `origin/main` first — never delete an unmerged tree). `cargo clean` is the faster interim if you want to keep the checkout.

---
## Relay — what to tell Jarvis (send only after the PM's go)
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-12-b3-rebase-and-regate-close-jarvis.md`. The PM's go message names the exact `main` tip to rebase onto.
- **One-line:** **B3 close — do NOT start until the PM confirms both OT-59 fixes are merged and `main`'s gate is green.** Then: rebase `codex/ot16-b3-qwen35-deltanet-bringup-jarvis` onto the fixed `origin/main` (real kernel-conflict territory now, not docs-only — resolve carefully, keep the GDN math + `.neom` v4 intact), **re-prove parity + Qwen2.5-unchanged on the rebased tree**, run `scripts/worker-close-gate.sh`, fire the mandatory `workflow_dispatch` QEMU gate on the exact rebased head, and RETURN both green CI URLs (push + QEMU) + the re-proven parity + the in-kernel generation-oracle PASS + your self-`git diff --stat` + the AAG-input notes. **If a previously-flaky spawn/wait scenario flakes again, STOP and report — do not retry-until-green** (that would mask a still-live race and means Cerebro's fix is incomplete). PM then does pm-merge-check + independent CI re-fetch + 5-lens AAG + `--ff-only` merge. Disk-reclaim after.
- **RAG:** semantic code search is available — use it for "neom v4 load invariants", "qwen35 generation oracle scenario", "qwen2 kill switch equality", "gdn state set layer cache decode loop". Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`. If down/degraded the PL owns RAG infra — note it and fall back to grep/Read (never a blocker).
