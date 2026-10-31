<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family re-check of the #44 / #38 fix-forward → Jarvis (Codex)

**Type:** review only. **Deliverable:** a pushed RETURN file. **Why you:** you found F1 and F2 on #44
(`Plans/RETURN-2026-10-18-hid-contention-codex-crosscheck-jarvis.md`, ee30f77e), and the change is SMP /
concurrency-core, so a different-family lens is required before merge. Janus's #38 finding (e3e4f9ec) is
fixed in the same PR; check it too.

## What is under review

Branch `claude/hid-fixforward`; the PM names the head SHA in the relay. Diff:
`git diff origin/main...origin/claude/hid-fixforward`.

1. **F2 (CP-37 on `PRIMARY`; pitfall CP-56).** In `kernel/src/drivers/usb/hid_keyboard.rs` and
   `hid_mouse.rs`, every `PRIMARY` acquisition (bind store, `primary()` lookups, `is_bound`) and every
   device-lock acquisition (`drain`, `try_read_pointer_event`, `debug_inject_report`, both
   `pump_routed_events`) now runs inside `without_interrupts`, whoever calls it.
   `InputFocus::deactivate` now takes `DELIVERY` (to bump the epoch) and does so IF=0 too.
2. **F1 (handoff disclosure; pitfall CP-55).** `kernel/src/atrium/focus.rs`:
   - `ACTIVATION_EPOCH` is bumped under `DELIVERY` by `activate`, by a successful `deactivate`, and by
     `Drop` of the active focus.
   - The new `Stage { queue, epoch }` replaces the drivers' bare `InputQueue`.
   - `stage_event` discards an older-epoch batch before appending. `flush` discards an older-epoch batch
     (`HANDOFF_DROPPED`) instead of delivering it. The pointer-route records path is not epoch-bound.
   - The keyboard-state reset policy is in the ADR-074 amendment: losing the keyboard means every key is
     released.
3. **Janus #38.** `kernel/src/atrium/input_queue_logic.rs`: the key-overflow fallback evicts the oldest
   *key*, not `pop_front`.

## Discriminators (claimed)

- **`hid_contention` handoff leg.** A holder on the other CPU keeps `DELIVERY`. A real usb-kbd 'z' is
  staged for A (`flush_contended` must rise). B is activated before any flush. Required:
  `b_events=0 a_events=0 handoff_dropped=2`. The PR shows the same leg FAILing with both epoch checks
  disabled.
- **`hid_contention` mixed_if leg.** A test-only witness counts `PRIMARY` and device-lock acquisitions
  made with IF=1. The fd-0 entry points are driven 64 times with interrupts ON. Required:
  `if_on_acquisitions=0`. `NEO_TEST_EXTRA_FEATURES=neo-test-hid-mixed-if-control` restores main's
  acquisition and must FAIL with `reason=hid_lock_taken_with_interrupts_enabled` (it shows 64).
- **KAT** `a_key_overflow_never_evicts_the_sole_queued_release_at_the_head`: Janus's capacity-3 case.
  It fails on main.

## Refute it

1. Is there any `PRIMARY`, device-lock or `DELIVERY` acquisition left at IF=1 (any caller, any
   feature)? Is the witness placed at every site it claims?
2. Can an event decoded under epoch E still reach a focus activated at E+1? Consider:
   - a stage read of the epoch racing the bump;
   - a batch partly decoded before and partly after a handoff;
   - A → B → A;
   - deactivate → reactivate of the same focus;
   - mouse kind-2 events with no pointer route.
3. Does discarding introduce a new stale-state hazard the reset policy does not cover (for example, a
   press delivered to A whose release is discarded while A keeps the keyboard)?
4. Is the handoff leg vacuous? Could 'z' be flushed to A before the handoff, or B be activated after a
   flush?
5. Does the mixed_if witness prove the invariant the deadlock depends on? Is the control a faithful
   reproduction of main's shape?
6. Is the queue fallback still bit-identical to the legacy rule on key-only streams?
7. Anything else a Claude reviewer would structurally miss.

A finding counts only if it survives your own re-verification; uncertain items are notes. "No findings"
must say what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/hid-fixforward-crosscheck-jarvis   (git fetch origin && git checkout -B codex/hid-fixforward-crosscheck-jarvis origin/claude/hid-fixforward)
```

```
git fetch origin
git show origin/claude/hid-fixforward:Plans/DISPATCH-2026-10-18-hid-fixforward-codex-crosscheck.md
git diff origin/main...origin/claude/hid-fixforward
```

Optional runs, at `NEO_SMP=4`:
- `cargo xtask neo-test hid_contention` should PASS.
- The same with `NEO_TEST_EXTRA_FEATURES=neo-test-hid-mixed-if-control` should FAIL with
  `reason=hid_lock_taken_with_interrupts_enabled`.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?"
questions; grep still wins for exact symbols. Health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`, expecting `{"status":"healthy",...}`. If it is down or
degraded, the PL manages it: note that in your RETURN and continue with direct reads.
Optional self-serve start:
`powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='HID stage activation epoch focus handoff'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-hid-fixforward-codex-crosscheck-jarvis.md` to
`codex/hid-fixforward-crosscheck-jarvis` and push it. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- each finding with severity, file:line and a repro;
- what you checked for each numbered item.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete the
worktree.
