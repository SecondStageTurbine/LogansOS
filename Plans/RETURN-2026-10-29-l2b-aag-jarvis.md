<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: L2b kernel lock route, relay #160 / #216

Reviewer: Jarvis. Worktree: `E:\AI Automation\workers\jarvis`.
Branch: `codex/l2b-aag-jarvis`. Overall: **FAIL**.
Read the requested three-dot diff, then the L2-to-L2b tip diff to exclude the
already-merged L2 stack. Reviewed `948979d9` (rebased from dispatched `bf92cd59`);
The old head was fetched explicitly. A whole-tree comparison is not empty: the rebase also incorporates intervening main changes; no whole-tree identity is claimed.
Line references below are at `948979d9`, not the RETURN branch.

1. **FAIL — the literal post-latch completion guarantee is stronger than the gate.**
   `kernel/src/atrium/focus.rs:257` takes DELIVERY, checks `receivable` at :258,
   then pops at :266. CPU A passes the check for client C and pauses before
   popping a queued key. CPU B completes `latch_sak(KEY_L)` (:634), including
   the lock-word update (:788), without taking DELIVERY. A resumes and returns
   `Some(key)` while locked. `wait_ready` has the analogous check/result gap
   (:284). A call whose route read starts after the latch sees the lock route
   (:752); this finding concerns an already-running receive, not a claim that
   newly decoded same-keyboard keys reach C. Specify whether overlapping reads
   may linearize before the latch; the dispatched absolute guarantee does not
   permit that exception. Item 7 supplies a separate newly-started read case.
2. **PASS — same-keyboard post-chord routing, allowing the documented drops.**
   `focus.rs:634` engages synchronously before the filter returns; `stage_event`
   (:347) records the bumped epoch, `flush` checks it before reading the route
   (:401, :405), and `route_event` reads that same route (:322). Same-device
   decode/flush serialization means a key decoded after the chord cannot use a
   pre-chord route snapshot. Contention, epoch mismatch and overflow can drop
   input; this is destination isolation, not guaranteed delivery.
3. **PASS — no lock acquisition on engage.** `focus.rs:787` uses atomic
   `fetch_update`, epoch RMW and a read. The optional watchdog notification
   (:794; `kernel/src/watchdog.rs:188`) is telemetry, not DELIVERY acquisition.
   CAS retry is lock-free, not a bounded number of retries.
4. **FAIL — one-word transitions are atomic, but the composed drop is not.**
   `kernel/src/atrium/sak_logic.rs:100` defines the word transformations;
   register/release/drop use `fetch_update` (`focus.rs:810`, :831, :851), so
   concurrent engage cannot tear the id/bit pair. However drop publishes word
   zero before clearing ACTIVE (:857). The item-7 schedule exposes the client
   route between those writes. Pure word KATs cannot test this second word.
5. **PASS — authority checked before any op.**
   `kernel/src/arch/x86_64/syscall.rs:8667` checks routing and
   `caller_holds_input_system` before `sak_op`; its register/release arms
   (:8793) additionally look up ACTIVATE rights. #210's
   `sys_input_focus_activate_impl` also checks InputSystem. Self-minting a
   focus activator is therefore insufficient.
6. **FAIL — forwarding checks before acquiring DELIVERY.**
   `focus.rs:570` filters the target before :571 takes DELIVERY; enqueue is
   at :605. Concrete schedule: A calls `forward_record` with a live pointer
   route, valid unconsumed tag and client C, observes unlocked and pauses at
   :571. B completes Super+L. A acquires DELIVERY, consumes the tag and enqueues
   the record to C while locked. No lock-state revalidation follows acquisition.
   The record can remain queued until unlock. This violates the stated
   no-client-delivery rule even though a fresh client receive is gated meanwhile.
   Merely moving the check inside DELIVERY would still leave an overlapping
   engage/check-to-enqueue window, since engage deliberately ignores that lock.
7. **FAIL — dying-lock-focus publication briefly reopens lock-free reads.**
   Start with ACTIVE=C and LOCK=(L,locked), the normal state immediately after
   the chord while the WM has not run. CPU A drops L under DELIVERY
   (`focus.rs:725`, :737), publishes LOCK=0 at :851 and pauses before :857.
   CPU B now calls C.is_active/receivable (:240, :246): `input_route` sees
   unlocked word zero and ACTIVE=C, and returns true. No release occurred.
   Dequeue waits for DELIVERY and is closed by the eventual ACTIVE=0 store;
   this is a real visibility-gate gap, not a claim that dequeue runs through
   the held lock. Clear the fallback route before publishing an unlocked word,
   or retain an engaged/no-focus tombstone until an explicit release.
8. **PASS — source exclusion and inspected author kill-switch evidence.**
   `kernel/Cargo.toml:757` opts in; `focus.rs:742` restores the original plain
   active-id load when off. New state/drop/filter/engage paths are cfg-gated,
   and the off `sak_op` is the old take (`syscall.rs:8770`). #216 records equal
   text and stripped ELF hashes for six off configurations and equal WM builds.
   Those binary comparisons were not independently rebuilt during this review.
9. **PASS — fallback preserves latch bits.**
   `userland/neo-init/src/desktop_session/lock.rs:141` compares registration
   with `SAK_LOCK_OK=0x100`, calls `note_sak` on a nonnegative take value and
   records KERNEL_ROUTE=false. Lock/unlock then retain L2 activation paths
   (:275, :406). No extra take silently consumes the fallback registration bits.
10. **FAIL — sections exist, but CP-97 overstates the guard.**
    `docs/concurrency-pitfalls.md:4227` describes instant client-receive denial,
    no-client forwarding and fail-closed drop; items 1, 6 and 7 qualify or
    falsify those claims. #216's prior-art row accurately describes sway's
    unfocus, abandoned-lock and replacement behavior, checked against
    [sway/lock.c](https://github.com/swaywm/sway/blob/master/sway/lock.c)
    and its [MIT license](https://github.com/swaywm/sway/blob/master/LICENSE).
    This is principle reuse; no vendored code was found. GitHub resolves the
    survey pin to `1652c54b73f67df17b7b4ab0b0f7048204aa8104`.

Executed: `cargo test -p atrium-logic-tests --features sak-lock-route lock_word`
(1 passed), full `cargo test -p atrium-logic-tests --locked` (144 passed), and
`git diff --check` (clean). RAG health was healthy; discovery used graft and
then direct pinned source reads. No injected kernel interleaving, new QEMU run
or binary-identity rebuild is claimed. The passing sequential KAT does not
cover the schedules above. Keep #216 blocked pending resolution and permanent
cross-CPU discriminators for the forwarding and drop boundaries.
