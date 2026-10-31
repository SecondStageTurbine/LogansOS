<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family re-check of the DI-1a fixes → Jarvis (Codex)

**Type:** review only. **Deliverable:** a pushed RETURN file. **Why you:** you bounced DI-1a (#46,
`Plans/RETURN-2026-10-18-di1a-bref-codex-crosscheck-jarvis.md`, 681988b1). This is SMP / concurrency-core,
so a different-family lens is required before merge.

## What is under review

Branch `claude/di1a-fixes`; the PM names the head SHA. It is stacked on the #44 fix-forward
(`claude/hid-fixforward`) until that merges. Diff: `git diff origin/claude/hid-fixforward...origin/claude/di1a-fixes`.

1. **F1: tag aliasing.** `kernel/src/atrium/pointer_route_logic.rs`:
   - Sequences run 1..=`MAX_SEQ` (65 535). A tag is the sequence and is compared in full.
   - Past `MAX_SEQ`, `mint` returns `None` and `expiry()` yields one `ROUTE_EXPIRED` (0x40) record with
     tag 0.
   - In `focus.rs`, `mint_records` sends that record once and mints nothing more; it no longer resets the
     ring under the same route.
   - The WM (`desktop_session.rs` `Pointer::poll`/`reclaim`) reclaims on `ROUTE_EXPIRED` and revokes the
     old handle.
2. **F2: RESYNC leaves a stale grab.** The new pure `neo_atrium::pointer_grab::decide` is the WM's
   per-record decision:
   - a RESYNC record with a button held cancels the grab (`Consumed` until all buttons are up);
   - a RESYNC record with all buttons up still reaches the grabbed client.
3. **F3: uncapped route scan (CP-54).**
   - `INPUT_ROUTE_CAP` = 32 live client routes; `activate` returns `false` past it and the syscall
     returns `EBUSY`.
   - The pointer route is outside the cap. `RouteEntry.pointer` is stored, so `DELIVERY` never upgrades
     a `Weak`: an upgrade there could drop the last reference, and `Drop` takes `DELIVERY`.

## Discriminators (claimed)

- **KATs** in `tools/atrium-logic-tests/src/pointer_route_kats.rs`:
  - `a_retained_tag_never_aliases_a_newer_record` mints the whole space and checks that `consume(1)` is
    stale while the 64 newest tags consume their own records;
  - `a_spent_route_sends_one_expiry_record`;
  - the exhaustion KAT, moved to the new boundary.
- **KATs** in `userland/neo-atrium/src/tests.rs`:
  - `resync_with_a_button_held_cancels_a_stale_grab` (press, lost up, then held RESYNC → cancel; up → end);
  - `resync_with_buttons_up_reconciles_the_grabbed_client`.
- **`di1_forward` route_cap leg:**
  - cap+4 activations: exactly the cap succeeds and 5 are refused;
  - the pointer route still reclaims under the flood;
  - after the flood drops, a new focus activates.
- **Regression:** `desktop_pointer_boot`.

## Refute it

1. Can any tag be consumed as a record other than the one it was minted as, on one route or across a
   reclaim? Is the expiry record itself consumable or forgeable?
2. After expiry, is there a window where reports are lost silently, or the WM keeps forwarding on the
   spent route? Does `sys_revoke` of the old route handle have side effects (descendants, a live
   `POINTER_ROUTE_ID`)?
3. Does the RESYNC rule ever forward a new press to the old target, or swallow the release the grabbed
   client needs?
4. Is every id lookup under `DELIVERY` now bounded? Check RECV, flush, mint and forward. Can the cap
   starve legitimate focuses: dead entries, the pointer route, a refused activate leaving partial state?
5. Did the `activate` return change or the `EBUSY` path break a caller or an ABI promise?
6. Anything else a Claude reviewer would structurally miss.

A finding counts only if it survives your own re-verification; uncertain items are notes. "No findings"
must say what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/di1a-fixes-crosscheck-jarvis   (git fetch origin && git checkout -B codex/di1a-fixes-crosscheck-jarvis origin/claude/di1a-fixes)
```

```
git fetch origin
git show origin/claude/di1a-fixes:Plans/DISPATCH-2026-10-18-di1a-fixes-codex-crosscheck.md
git diff origin/claude/hid-fixforward...origin/claude/di1a-fixes
```

Optional runs, at `NEO_SMP=4`: `cargo test -p atrium-logic-tests`, `cargo test -p neo-atrium`,
`cargo xtask neo-test di1_forward`.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?"
questions; grep still wins for exact symbols. Health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`, expecting `{"status":"healthy",...}`. If it is down or
degraded, the PL manages it: note that in your RETURN and continue with direct reads.
Optional self-serve start:
`powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='pointer record ring tag expiry reclaim'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-di1a-fixes-codex-crosscheck-jarvis.md` to
`codex/di1a-fixes-crosscheck-jarvis` and push it. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- each finding with severity, file:line and a repro;
- what you checked for each numbered item.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete the
worktree.
