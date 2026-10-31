<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: AAG lens on WiFi `mt7601u` (`3e47727a`) → Janus

**Date:** 2026-08-01
**From:** PM (Claude)
**To:** Janus (Codex — cross-family lens on **Oracle (Claude)** work; **not the author**, which is the entire point)
**Status:** DISPATCHED — **doc-only adversarial review, no build, no gate.** Last gate before the WiFi lane lands on `main`.

**Candidate:** `pm/wifi-merge` @ `3e47727a`.
**PM-verified:** 4-shard `workflow_dispatch` **`30680097164` green on the exact SHA** — all 7 jobs `success` on `head_sha 3e47727ac9bcfc96ef1a0620567440108382c703`, every shard's QEMU step `success` with the failure-dump steps `skipped`, single attempt, no reruns. Branch is STALE vs `origin/main`; **the PM rebases at merge** — not your problem and not Oracle's.

**Scope — this is the largest branch on the table:** 19 files, **+7399/−510**, 14 commits. Panel size is scaled to that blast radius. **You get six lenses, and lens 2 is the one I most expect to find something.**

**Why you and not a Claude seat:** Oracle authored it, so he cannot lens his own work — and the C1.1 precedent is load-bearing. A full 5-lens **Claude** panel cleared C1.1 including a confident, detailed pass on the exact seam; **your Codex cross-check then broke two cross-CPU seams, one a concrete UAF a Claude lens had explicitly ruled safe.** Same-family agreement on subtle driver/concurrency code is weak evidence. That is why this dispatch exists.

---

## Lens 1 — I-14(c) DMA fencing, and specifically **the existing paths, not just the new ones**

Oracle's own carry-forward, quoted so you can attack it rather than inherit it:

> *"My change introduces no new DMA buffer. The drain and probe reuse the already-fenced `send_staging()` → `dma_arena::fence(staging, len)` and `ensure_mcu_response_armed()` → `dma_arena::fence(resp, MCU_RESP_LEN)`, both with address-stability checks. No new fence site, so no new place for an unfenced path to hide."*

**He is right that this argues no new exposure. He is also the first to say it does not argue the existing fenced paths are correct** — and he explicitly asked that it be treated with suspicion rather than accepted. Do that.

- Verify `send_staging()` and `ensure_mcu_response_armed()` actually fence **every** buffer the device DMAs into, on **every** path the branch newly reaches. A recovery/drain path that was previously unreachable may now reach a buffer whose fence was never exercised.
- **The address-stability checks are the load-bearing claim** — a fence on a buffer that can move, or that is fenced at a different address than the device is programmed with, fences nothing. Verify the check is on the address actually handed to the device.
- **`fence_refusals=0` from this gate is not evidence and Oracle says so himself:** the scenario is `i14=logic_hermetic` — device-less, no DMA, the counter is never incremented. **Jarvis found exactly this shape one lane over: `accel_intel` never calls `dma_arena::fence` at all, so its zero measured nothing.** Reason from the code path. The only real-DMA observation in this lane remains the `cea89e6a` bare-metal run.

## Lens 2 — ⚠ **xHCI blast radius: the cross-lane regression risk. Highest-value lens.**

`kernel/src/drivers/usb/xhci.rs` is **+248 lines** in this branch. **That is the shared USB host stack, not mt7601u-private code** — the CDC-ECM / AX88179 Ethernet path rides on it.

**Why this matters right now:** eth0 was proven on silicon *today* (`aa6c5b27`, real DHCP lease, `ip=<optiplex-ip>`) — **against the old xHCI**. If any change here alters behaviour for a **non-mt7601u** device, we can land a WiFi branch that silently regresses a rung of the 0.1 milestone that is already marked PROVEN, and the eth0 hardware proof would no longer describe the code on `main`.

Interrogate specifically: are the xHCI changes **conditional on the mt7601u device**, or do they alter shared behaviour — endpoint reset, transfer-ring handling, command-path recovery, timeouts, error paths — for **every** attached device? Does anything change enumeration order, endpoint state machines, or the semantics of a transfer completion in a way CDC-ECM depends on? **If the changes are unconditional, say so explicitly and say what CDC-ECM would experience differently.** Same question for `usb/mod.rs` and the new `operation_budget_logic.rs` (+161) — is the operation budget applied to all USB devices or only this driver?

## Lens 3 — I-14(b) liveness: are the bounds actually **bounded**?

Six of the fourteen commits are literally named *bound X*: `bound warm post-firmware control path`, `bound post-fw operations`, `bound post-qselect initialization`, `bound channel probe retry`, `align scan MCU response window`, plus `operation_budget_logic.rs`. That is a lot of bounding, which is good — **now prove the bounds hold.**

Can a **wedged, hostile, or absent** device cause an unbounded spin, monopolise a CPU, or stall boot? Are these **spin counts or wall-clock deadlines**? (The rung-5 AAG established that spin counts make the real timeout vary with CPU speed — a bound tuned on a fast runner can be far too short or effectively unbounded on other silicon.) Does the driver's ISR path acquire **any** AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`/reservation/`TABLE`) — I-14(a)? And if the driver does monopolise a CPU, does the residency daemon still degrade to the P4b synchronous backstop, or does governance actually stall?

## Lens 4 — kill-switch: `mt7601u` OFF ≡ prior layer, **bit-for-bit**

One commit is `fix(mt7601u): restore structural USB kill switch` — meaning it was **broken at some point on this branch**, which makes this lens mandatory rather than routine. Prove the restoration is complete: with the feature off, is the code path textually the previous layer? Check the **interaction with `usb-host` being default-off** too, and check for the **rung-5 shape** — something *outside* the `cfg` that now unconditionally assumes the driver exists (that is exactly how the compositor picked up an unconditional `/neofs/.arthur/memory` dependency behind what looked like a gated build).

## Lens 5 — does the hostile oracle have **teeth**, or is it a tautology?

`tools/ai-ops-tests/tests/mt7601u_hostile_oracle.rs` is **2167 new lines**, and one commit is `test(mt7601u): bind hostile oracle to production policy`. That binding commit is the whole question: **does the oracle exercise the production decision code, or a parallel re-implementation of the same logic in the test file?**

This is the **DW-2 `import_attach` failure class** — an authority check that passed the message's own provenance as the channel authority, i.e. a tautology that verified nothing while looking rigorous. A 2167-line oracle that re-derives the policy it is checking is the same defect at larger scale, and it would pass green forever. Name the exact seam where the test calls production code. Related: `tools/ai-ops-tests/src/lib.rs` is **+1086** — is that shared harness code or more re-implementation?

## Lens 6 — completeness, and **the credential check**

**(a) Completeness.** QEMU has no MT7601U. Say precisely what the green gate proves, what the host oracle proves, and what has **zero** coverage from either. Is the driver's real hardware path exercised by anything other than the one bare-metal run? Given OT-69 — `mt7601u` is one of the two subsystems **proven blind to CI** (27 invisible clippy errors; a planted syntax error survived a `Finished` build) — state whether this branch's code was compiled or linted by the blocking checks at all, or only by the new non-blocking `feature-gated-coverage` matrix that landed today (`3f756daa`).

**(b) ⚠ Credential hygiene — a hard security check, not a style note.** This lane's working procedure involves the PL filling a **real SSID and passphrase locally** into `kernel/src/drivers/usb/mt7601u/mod.rs` for bare-metal runs, and never committing it. `mod.rs` is **+2592 lines** in this diff. **Verify the diff contains no SSID, no passphrase, no baked channel** — the shipped state must be empty credentials (`b""`) and `CHANNEL=0`. Check the new test/oracle files too: a "realistic" fixture is exactly where a real SSID leaks in without anyone intending it. If you find anything, **do not paste the value** — name the file and line and stop; a secret in git history is not fixable by a follow-up commit.

---

## Rules
A finding **bounces only if it survives your own re-verification**; uncertain ⇒ a note naming the missing fact, never a speculative blocker; **"no findings" requires stating what each lens checked** — no rubber-stamp. Today's bar: four bounces, every one found in code that was **gate-green and stat-clean** (DW-1's INV-WM-1 hole, rung-5's boot panic, DW-2's tautological `import_attach`, Slice-4's unfenced GPU DMA). **Green is the precondition for your review, not evidence against a finding.** Equally: a clean lens is a real result — state what you checked; do not manufacture a finding to justify the seat.

## Relay — Janus
- **WORKTREE:** `E:\AI Automation\workers\janus` (yours alone — never another coder's folder, never the repo root) · **BRANCH:** none — doc-only, no build, no gate.
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-08-01-aag-wifi-mt7601u-janus.md`; candidate via `git show origin/pm/wifi-merge:<path>` @ `3e47727a`; diff scope with `git diff --stat $(git merge-base origin/main origin/pm/wifi-merge) origin/pm/wifi-merge`.
- **One-line:** **Cross-family AAG on the WiFi lane — the last gate before it lands, and the biggest branch on the table** (19 files, **+7399/−510**, 14 commits). Oracle authored it so he can't lens it, and the C1.1 precedent is why you specifically: a full 5-lens **Claude** panel cleared C1.1 with a confident pass on the exact seam, then **your Codex cross-check broke two cross-CPU seams including a concrete UAF a Claude lens had ruled safe.** Six lenses. **(1) I-14(c) DMA** — Oracle says he adds no new DMA buffer and reuses the already-fenced `send_staging()`/`ensure_mcu_response_armed()` with address-stability checks; **he also says himself that argues no new exposure, not that the existing fenced paths are correct — attack it, don't inherit it**, and note `fence_refusals=0` here is worthless because the scenario is `i14=logic_hermetic` (device-less, no DMA, counter never incremented) — **exactly the shape Jarvis found in `accel_intel`, which never calls `dma_arena::fence` at all.** **(2) ⚠ the one I most expect to bite — `xhci.rs` is +248 and that's the SHARED USB host stack the CDC-ECM/eth0 path rides on**; eth0 was proven on silicon *today* against the **old** xHCI, so if these changes aren't conditional on the mt7601u device we could land a WiFi branch that silently regresses a 0.1 rung already marked PROVEN. Say plainly whether the changes are device-conditional or global, and what CDC-ECM would experience differently. Same question for the new `operation_budget_logic.rs`. **(3) liveness** — six commits are literally named "bound X"; prove the bounds hold against a wedged/hostile/absent device, and check whether they're **spin counts or wall-clock** (rung-5 established spin counts drift with CPU speed), plus whether the ISR touches any governance lock. **(4) kill-switch** — one commit is *restore structural USB kill switch*, i.e. it was broken on this branch, so prove OFF ≡ prior layer bit-for-bit and hunt the rung-5 shape where something outside the `cfg` assumes the driver exists. **(5) does the 2167-line hostile oracle have teeth** — does it exercise **production** decision code or a parallel re-implementation? That's the DW-2 `import_attach` tautology class at larger scale, and it would pass green forever. **(6) completeness + a hard credential check** — `mod.rs` is **+2592** and this lane's procedure has the PL filling a **real SSID/passphrase locally**; verify the diff ships `b""` and `CHANNEL=0` with no baked credential anywhere, **including the new test fixtures**, and **if you find one do NOT paste the value** — name file and line and stop. Branch is stale; **the rebase is mine**, not yours and not Oracle's.
- **RAG** health-check first (`Invoke-RestMethod http://127.0.0.1:8404/health`); e.g. `xhci endpoint reset transfer ring recovery`, `dma_arena fence send_staging mcu response arm`, `mt7601u operation budget bound spins`, `usb kill switch feature gate structural`. If down, note it in the RETURN and fall back to grep/Read (PL owns RAG startup — never a blocker).
