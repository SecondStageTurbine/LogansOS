<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: #49 DI-1a fixes cross-check — Jarvis

**Verdict: MERGE.** All three findings from my earlier #46 RETURN are **CLOSED**. I found no new
DI-1a blocker after tracing record identity through exhaustion/reclaim, the production RESYNC state
machine, and every `DELIVERY.routes` lookup. This verdict is for the DI-1a delta over its stated
`claude/hid-fixforward` base; that base has its own independent RETURN.

Reviewed `origin/claude/di1a-fixes` at `acd9be73`. RAG health was healthy; semantic discovery was
followed by direct reads of the ring, focus router, syscalls, revoke walk, WM pointer loop, and KATs.
`cargo test -p atrium-logic-tests` passed 100/100; `cargo test -p neo-atrium` passed 25 with two
explicitly ignored visual/performance tests.

## Earlier findings

1. **#46 F1, tag aliasing — CLOSED.** A route mints only sequences 1..=65,535 and compares the full
   stored `u32` sequence to the `u16` tag. It never resets the ring under that route. After exhaustion,
   tag 0 carries one unconsumable expiry notification; userland claims a new route, switches handles,
   then revokes the old handle. The old route fails the live-route-id check, so a retained old tag
   cannot consume a record on the new route.
2. **#46 F2, stale RESYNC grab — CLOSED.** The exact production decision function cancels any existing
   grab when a RESYNC record reports buttons held, enters `Grab::Consumed`, and drops records until all
   buttons are up. A buttons-up RESYNC remains on `Grab::Client` long enough to forward the missing
   release, then clears the grab.
3. **#46 F3, uncapped route scan — CLOSED.** There are at most 32 client entries plus one pointer-route
   entry. Activation prunes dead weak owners, returns `false` without changing the active id or epoch
   when the cap is full, and the syscall maps refusal to `EBUSY`. RECV, flush, mint, and forward all
   scan only this bounded vector. The pointer-route flag is stored directly, avoiding a `Weak::upgrade`
   whose temporary could run `Drop` while `DELIVERY` is held.

## Findings

No findings survived re-verification.

## Dispatch checks

1. **Identity / expiry:** within a route, each nonzero tag is minted once. Slot overwrite makes an old
   tag stale, consumption makes it already-used, and exhaustion prevents wrap. Tag 0 cannot match any
   slot sequence. Across reclaim, `POINTER_ROUTE_ID` changes under `DELIVERY` before the old handle is
   revoked, so forwarding through the old handle returns `NotRoute`; the new ring is reachable only
   through the new handle. The expiry notification itself is never passed to forward.
2. **Spent-route behavior:** reports after sequence exhaustion are deliberately dropped until the WM
   drains the one expiry record. The notification is appended even when the bounded route queue is
   full (the queue policy evicts an older entry), so `expiry_sent` cannot silently consume the only
   signal. The WM swaps to the newly claimed route before revoking the old handle. Pointer-route caps
   have no descendants, so revoke has no unrelated descendants or live-route side effects.
3. **RESYNC:** a held RESYNC is never classified as a fresh press and cannot reach the old target; it
   cancels to `Consumed` until release. A buttons-up RESYNC is forwarded to an existing client grab as
   the missing release, then ends it. With no grab, either form is consumed without selecting a target.
4. **Bounds / admission:** every id lookup under `DELIVERY` is capped at 33 entries. Dead client entries
   are pruned on activation; the pointer route is outside the client count; a refused activation leaves
   no route, active-id, or epoch partial update. A process retaining 32 live routes can keep later
   activations at `EBUSY`, which is the explicit bounded-resource contract; releasing those owners lets
   the next activation prune and succeed, as the route-cap leg checks.
5. **ABI / callers:** successful `SYS_INPUT_FOCUS_ACTIVATE` remains zero; only capacity refusal adds
   `EBUSY`. Production callers already reject any nonzero return, and no caller assumes a refused
   activation changed focus. Internal ignored return values remain success-path test setup.
6. **Other:** `sys_revoke` removes the old route cap under IF=0; its final `InputFocus::drop` sees the
   new live route id and cannot clear it. No new tag forgery, stale-target forwarding, partial reclaim,
   or unbounded scan survived review.

