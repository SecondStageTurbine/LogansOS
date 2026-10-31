<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE (relays #170–#172) → Chad: TCP listen bounds, exit-code bounds, the lock screen's WM

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. A bundle, so one model throughout (CLAUDE.md bundle rule). On their own,
#170 would be Sol medium and #171 and #172 Luna high.

All three merged after batch 19 (run `40883250031`, green at `2a313a39`). Review each squash on `main`:
`git show <sha> --stat` and `git show <sha>`. One pushed RETURN per relay, each a plain checklist (pass or fail, the
enforcing `file:line`, a concrete input for any fail): `Plans/RETURN-2026-10-29-<slug>-review-chad.md`.

## #170: #211 `cb5d633a`, OT-94 TCP listen bounds. Slug `tcp-listen-bounds`

Muse covers the security side (#168). Check the mechanics:
1. Are the half-open count and the accept queue updated under the same lock as `CONNECTIONS`, so the bound holds
   with two CPUs taking SYNs at once?
2. Does every state transition out of SynReceived (ACK, RST, timeout, close) decrement the half-open count exactly
   once?
3. **The test harness (CP-37).** `tcp_listen_bounds` takes the net spin locks with interrupts on and asserts
   `interrupts_enabled()`. Syscalls take the same plain locks with IF=0. Is there any other taker in that scenario,
   so it could deadlock? Pass if not, and note it.
4. Is the scenario deterministic (the tick waits, the drain counts)?

## #171: #214 `1bcd2a1f`, OT-91, an exit code is kept only while someone can claim it. Slug `exit-codes-bounded`

1. Is an exit code dropped exactly when its last claimant goes: the last process handle is revoked or closed, and
   the spawner exits without waiting? Is it never dropped while a `sys_wait` caller could still claim it?
2. Can two CPUs (a wait and a revoke, or an exit and a revoke) race to drop it twice, or leak it?
3. Do a parent's unwaited children, with their handles revoked, leave nothing behind? Check the scenario's counts.
4. Does the change keep #205's obj-quota charge for the exit-code insert released exactly once?
5. Is the `exit-codes-test` userland feature confined to the scenario?

## #172: #208 `ed56c60d`, OT-66 L2, the lock screen's WM. Slug `l2-lock-wm`

Muse covers input isolation (#169). Check the WM's state machine in
`userland/neo-init/src/desktop_session/lock.rs` and `desktop_session.rs`:
1. Lock, the password entry, unlock, and a wrong password: is every transition handled, with no state the WM can't
   leave?
2. F1–F3 from the pre-merge AAG (route takeover, channel flood, control waits): is each fixed, and does a scenario
   leg fail without the fix?
3. Does the WM's main loop stay live while locked: no blocking wait on any client channel?
4. `kernel/src/atrium/focus.rs` and `sak_logic.rs`: do the KATs in `tools/atrium-logic-tests/src/sak_kats.rs` cover
   the new lock case, with Super+L handled kernel-side?
5. **Prior art:** is the PR's section present, and do its claims hold?

```
MODEL: GPT-6.1-Sol. EFFORT: high. (A bundle: one model throughout, per the bundle rule.)
WORKTREE: E:\AI Automation\workers\chad   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/review-bundle-2026-10-29e-chad
          git fetch origin && git checkout -B codex/review-bundle-2026-10-29e-chad origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-review-bundle-b19-chad.md
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push all three RETURNs. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
