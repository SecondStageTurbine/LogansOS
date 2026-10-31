<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #61): discriminators and HWP re-check (#124)

Reviewed SHA: `0ffc4e227c55ca2248ebf9848337bbb33c04e5aa`.
Branch: `codex/discriminators-hwp-recheck-chad`.
Overall verdict: **FAIL, item 5's early-control guarantee remains incomplete.** R6 is closed; the slow-observer false red and never-woken join hang are fixed. This is a review-only return; no production regression or runtime control failure is claimed.

| Item | Verdict | Evidence at reviewed SHA |
|---|---|---|
| 1. R6 decision time | PASS | `kernel/src/ai/par_matmul.rs:580` samples elapsed before the CAS at `:604`; exactly that value goes to `past_steal_grace` at `:585` and successful claims record it using `fetch_min` at `:609`, before compute at `:615`. Reset is at `:160`; harness resets at `kernel/src/test_harness.rs:23508`, reads the minimum at `:23528`, and rejects values below the independent threshold at `:23551`. Long compute and later steals cannot hide an early decision. |
| 2. Independent threshold | PASS | `kernel/src/test_harness.rs:23468` writes out `EXPECTED_GRACE_CYCLES = 500_000_000`; `:23551` uses it rather than the helper result. `:23560` separately rejects `grace_threshold_mismatch`. |
| 3. Zero-grace control | PASS | `kernel/Cargo.toml:1059` composes the control with the scenario; `kernel/src/ai/par_matmul.rs:190` returns `(hz, 0)`. An ordinary first-look steal below 500M cycles fails `stole_before_grace` at `kernel/src/test_harness.rs:23553`. Even if scheduling delays the first look past 500M, the zero helper result still fails the independent mismatch check. The gate registry includes only the positive feature (`tools/xtask/src/main.rs:3112`), not the control. No control execution was performed. |
| 4. Production path | PASS | `kernel/src/ai/par_matmul.rs:563` gates the old decision block on time-grace and absence of the scenario. Its `grace_inputs`/`past_steal_grace(rdtsc().wrapping_sub(published),...)` body is unchanged from the parent. Override storage, elapsed local, and recording are scenario-only (`:183`, `:579`, `:608`). |
| 5. Early check and control | **FAIL (control coverage)** | `kernel/src/sched/tick_sleep.rs:220` publishes the worker's actual deadline; `:222` records its resumed execution tick; `:235` compares those values, correctly removing the slow-observer false red. But the early control at `:129` advances expiry, not execution. `:156` queues a scheduler wake; `kernel/src/sched/scheduler.rs:2120` puts the thread at the back of the ready queue. A ready thread need not run within that same tick. Concrete false-pass schedule below. |
| 6. Never woken / detach | PASS | `kernel/src/sched/tick_sleep.rs:227` bounds the completion wait to 200 ticks (plus the separate initial installation wait of up to 10 ticks at `:226`), with a TSC fallback at `:193`. On false completion, `:230` detaches and `:231` returns woken=false; `kernel/src/test_harness.rs:24949` immediately fails `tick_sleep_not_parked`. No-wake control uses now=0 at `tick_sleep.rs:134`, leaving a positive deadline unexpired. Worker closure `:217` captures no stack references and uses only statics `:182`-`:184`; TABLE owns a value token, not a selftest reference (`:40`). `kernel/src/sched/kthread.rs:274` installs an Arc in the registry; detach marks detached and only unregisters an already-exited worker (`:377`-`:389`). The still-parked thread/control remain valid. |
| 7. HWP image A | PASS | `tools/xtask/src/main.rs:4947` includes hwp-performance by default; old flag is accepted without modifying features at `:4992`. Both ISO and image-kernel-build call the same builder (`:4984`, `:6280`); CI product-real-lm at `.github/workflows/ci.yml:820`-`:821` has the same feature set. `kernel/src/arch/x86_64/cpufreq.rs:124` obtains per-CPU CPUID caps and returns on !hwp at `:125`; the only writes, PM_ENABLE at `:134` and HWP_REQUEST at `:142`, follow that check (and request follows enable). HWP is CPUID leaf 6 EAX bit 7 (`cpufreq_logic.rs:35`). |

## Confirmed source counterexample for item 5

Use the early-control alone. The test worker starts at tick 100 and publishes deadline 120, parks, and the observer sees its token (parked=true). At actual tick 119, the control's effective now is 120, so expiry removes its token and makes it ready. Another runnable thread or an interrupt delays resumed execution until tick 120. The worker then publishes WOKE_AT=120 and exits. The observer reads woken=true, woken_early=(120<120)=false, slept=20, and past_skips=true. All conditions at `test_harness.rs:24949` pass; a normal parked run can also satisfy the later counter checks. Thus the early-control need not FAIL.

I checked the actual expiry, scheduler enqueue, record and harness predicates and evaluated these exact values locally: expiry=true, woken_early=false, slept=20, harness rejection=false. This is a source-level scheduling counterexample, not a QEMU reproduction. It does not show production sleep returning early. To discriminate early token expiry independently of scheduling, the test needs an expiry-time observation tied to its own token/deadline; resumed execution time alone cannot establish it.

## Validation and handoff

Read both prior returns, requested parent-to-reviewed diff, surrounding executor/selftest, kthread ownership, scheduler wake, feature/CI and CPUID/MSR paths. No kernel build or QEMU run was required or performed. RAG health was healthy (ollama=true, qdrant=true); supplied paths and exact symbols made direct reads sufficient.

Only this RETURN is committed. Preexisting untracked graft cache files were preserved in stash `chad preexisting graft cache before relays 61 and 66`. After push, checked empty `git status --porcelain` and empty `git log '@{u}..'`.

