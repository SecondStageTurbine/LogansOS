<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — #253 L2.1 (the hash-chained action log, `NAL1`): security review (Section 204)

**Seat:** Dexter · **Reviewed at:** #253 head `75eae4f8` (the current `pull/253` head).
**The dispatch's head is stale:** it names `124ea1e7`, which is **not an ancestor** of the current branch. The
branch has moved: the same subject now appears as `ae024cfe` (rebuilt on the current base, different tree), and two
commits were added after it (`d2b325dd` "the #253 bounce", `75eae4f8` "THINK L2: appending as built"). `git diff
origin/main...pr253` now serves `75eae4f8`; the line numbers below are at that head.
**Branch:** `dexter/action-log-review` · **Base:** `origin/main` = `924c189c`
**Answers:** `Plans/DISPATCH-2026-10-31-b35-b36-reviews.md` §204
**RAG:** healthy (`{"status":"healthy","ollama":true,"qdrant":true}`). I read the PR tree directly; the RAG query
returned no additional call sites.
**Scope:** static review of `git diff origin/main...75eae4f8` (27 files, +2493/−28); I ran the three host test
programs named at the end, not the 4-shard QEMU gate. The PR touches `kernel/Cargo.toml` and
`kernel/src/test_harness.rs` (test feature wiring), so the mandatory 4-shard gate remains the PM's before merge.

## Verdict

| # | Item | Verdict |
|---|---|---|
| 1 | Record format byte-oracled, by hand | **PASS** — hand-laid records and independent BLAKE3 links; I reproduced both links with `b3sum` |
| 2 | Chain verified on read; truncation/tampering fail closed | **PASS within the stated design** — verified before every append; a break is cut, kept aside and marked; the in-boot truncation anchor holds. **Named limits:** no cross-boot anchor, and the truncation path is untested |
| 3 | Log WM-only under `.system/`; a client cannot write, truncate or replace it | **PASS** — kernel `SystemStore` gate on every root-form open, no unlink/rename, symlinks refused, no client path |
| 4 | Every grant, prompt answer, action and undo logged with intent; growth bounded | **PASS for the flows this slice implements** — all answer/lend/refusal/done records carry the intent; cap 1 MiB with rotation. `Undone`/`GrantRevoked` are defined but unreachable until L1.3b/L2.3 (see note) |
| **F1** | **Blocking (inherited from §203, extended by this PR): the path/intent reads park the WM before the lock check** | **FAIL** — a client can hold the lock screen off / unservable with one `OP_PERMISSION`; #253 adds a second unpolled blocking read |

---

## 1. The record format is byte-oracled by hand — PASS

**The oracle is independent of the codec.** The test file `userland/action-log-core/src/tests.rs:1-6` (outside the
codec) states the two records were laid out field-by-field from the THINK table by a separate Python `struct` script
and linked with the Python `blake3` package, not this crate's `link`. The fixtures are literal bytes:
`R0` (`tests.rs:11-19`, 101 bytes) and `R1` (`tests.rs:26-32`, 80 bytes), with `L0` (`:21-23`) and `L1` (`:34-36`).
`encode_is_the_oracle` (`:70-74`), `decode_reads_the_oracle` (`:77-80`) and `link_is_the_oracle` (`:83-86`) compare
the codec to those literals, not to a round trip.

**Independent reproduction (my run).** I rebuilt `R0`/`R1` from the THINK §2 table (not from the arrays) with
Python `struct` (`<HBBQQIHH`, magic, 16-byte `prev`, 16 reserved zero bytes, path, intent) and hashed them with the
official `b3sum`:

- `R0` = 101 bytes, BLAKE3 prefix `8f70abda49cbeda3b341af5046d4ada6` = `L0`;
- `R1` = 80 bytes, BLAKE3 prefix `6db51fe5d29686e058dd787977e012ca` = `L1`.

Both match the fixtures. The field walk also matches the table: magic `NAL1`, version 1, kind/action at 6/7, seq at
8, time at 16 (1761830400 = `00 66 03 69 00 00 00 00`), detail at 24 (13 = `0d` in `R1`), path/intent lengths at
28/30 (16/21 for `R0`, 16/0 for `R1`), `prev` 32..48, 16 reserved zero bytes 48..64, then path and intent
(`userland/action-log-core/src/lib.rs:9-12` is the documented layout; `encode` at `:107-126`, `decode` at `:151-188`).

**Coverage note (not a FAIL).** The hand-laid oracle covers two records (asked/read and done/read) — the header,
the text fields and the zero-link/next-link cases. It does **not** hand-lay a rotation marker, a refusal/detail
record, or every `Kind`; those are built by `encode` in the marker test (`tests.rs:155-187`). The byte-flip test
(`:114-125`) flips every bit of record 0 only. Two hand-laid records are enough to fix the wire layout; adding a
third fixture for a `Refused` with a detail word would close the remaining field combination (kind 6 + nonzero
detail), but it is not required by this item.

**Host evidence.** `cargo test -p action-log-core` → 8 passed, 0 failed; `cargo test -p atrium-logic-tests
permission_byte_oracle` → 1 passed (the `OP_PERMISSION`/`OP_PERMISSION_DONE` 64-byte request oracle at
`tools/atrium-logic-tests/src/lib.rs:1163-1249`); `cargo test -p arthur-logic-tests desktop_action_tests` → 2 passed.

## 2. Verified on read; truncation and tampering fail closed — PASS in the stated design

**Every append verifies the whole file first.** `append_inner` opens the log, reads it to the end, and calls
`alc::verify(&log)` before deciding the next sequence number and link
(`userland/neo-init/src/desktop_session/action_log.rs:211-224`). `verify` (`userland/action-log-core/src/lib.rs:208-241`)
checks magic, version, reserved zero, kind, lengths, sequence contiguity and each record's `prev` against the previous
record's BLAKE3; it returns the last good length and link, or `broken_at`.

**A break is not trusted, and it is preserved.** If verification breaks, the WM writes the bytes after the last good
record to `/neofs/.system/actions.broken` (up to 64 KiB), rewrites the log to the good prefix plus a `Refused` record
with detail `REFUSED_CHAIN_BROKEN`, then appends (`action_log.rs:227-250`, `:256-268`; constants at
`userland/action-log-core/src/lib.rs:62-84`). The tampered tail is not treated as records; the break is itself a record
the next `verify` accepts and the inspector can show.

**Truncation while the WM runs is anchored in memory.** After each successful append the WM stores the next sequence
and the last link (`action_log.rs:281-284`, `:46-73`); the next append compares the verified head to that anchor and
records `REFUSED_TRUNCATED` on a mismatch (`:244-249`). `set_last` runs only on success (`:281-284`), so a failed or
partial write is repaired on the next append rather than silently adopted.

**A lend is not made on an unappended record.** The `AllowedOnce`/`AllowedAlways`/`LentFromRecord` record is appended
before the cap is handed over; if the append returns false the WM revokes the derived cap and refuses
(`userland/neo-init/src/desktop_session/permission.rs:353-357`). That is the security-relevant fail-closed property:
no action runs on a decision the log could not record.

**Named limits (not defects against the THINK's stated design):**

1. **No cross-boot anchor.** `LAST_*` is in-memory, so a tail truncation or rewrite that happens while the WM is
   off is not detectable by the file alone; the chain can still catch a mid-file edit because the next record's
   `prev` no longer matches. The file says so (`action_log.rs:15-18`) and THINK §2 says the same ("Across a reboot
   only an anchor outside the file could tell"). This is the known L2.3-inspector/head-publish work, not a claim
   this slice makes.
2. **The WM keeps serving after a repaired break.** It cuts, preserves and marks rather than halting all actions.
   That is "fail closed on trust" (the tampered bytes are never used as records and no lend depends on them), not
   "fail closed on availability". The log is not the authority for a grant — `arthur-grants.json` is
   (`permission.rs:187-200`) — so a tampered log cannot mint access.
3. **The truncation path is untested.** `REFUSED_TRUNCATED` is defined and used only in `append_inner`
   (`action_log.rs:244-249`); the boot scenario exercises a *torn tail* (a broken record, repaired through
   `verify`/`REFUSED_CHAIN_BROKEN`) and rotation, not an in-boot truncation at a record boundary. The unit test
   `a_removed_or_reordered_record_is_caught` (`tests.rs:128-140`) shows `verify` alone cannot detect a last-record
   edit; that case is caught by the `LAST_LINK` check, and no test exercises it. A concrete test: write two good
   records, truncate the file to the first, append a third; expect `ACTIONLOG truncated`, a kind-6 detail
   `0xFFFF_FFFD` record, and a verify line whose head starts from the first record's link.

## 3. The log is WM-only and a client cannot write, truncate or replace it — PASS

**The kernel enforces the store, not the WM's convention.** Every root-form open at or under a mount's `.system/`
is `EPERM` unless the caller holds `SystemStore` (`kernel/src/arch/x86_64/syscall.rs:8782-8795`), reached from
`sys_open` at `:6387-6390` and `sys_model_load` at `:2925-2928`. The path is normalized before the check, so
`/neofs//.system`, `/neofs/./.system` and `/neofs/x/../.system` all hit it (`kernel/src/fs/mod.rs:1834-1842`,
`normalize_absolute_path` at `:1844-1867`). The cap is minted by the kernel root path into neo-init only; the
`login_record_guard` scenario proves a no-caps process gets `-1` for read, write, a `..` path, the directory and a
new file under `.system/` (`userland/neo-worker/src/record_guard.rs:19-38`,
`tools/xtask/src/login_scenarios.rs:440-462`).

**The log path is fixed and the WM is its only writer.** `userland/action-log-core/src/lib.rs:27` fixes
`/neofs/.system/actions.log`; the WM's `open()` creates it only through the normal `sys_open` path
(`userland/neo-init/src/desktop_session/action_log.rs:99-106`), so it needs the same `SystemStore` authority. The
repair file is `/neofs/.system/actions.broken` (`lib.rs:84`). The `Always` record is likewise
`/neofs/.system/arthur-grants.json` (`permission.rs:41`), written by `record()` (`:202-224`) and by nothing else.

**A client cannot get a handle to it.** There is no `SYS_UNLINK`/`SYS_RENAME`/`SYS_TRUNCATE` path syscall in the tree
(`git grep SYS_UNLINK|SYS_RENAME` returns nothing); `SYS_FTRUNCATE` takes a handle (`syscall.rs:115`, `:1141`), and
handles are per-process. A symlink is refused on open (`kernel/src/fs/mod.rs:1483-1485`), so a client cannot reach
the store through one. The WM's own path validator refuses `.system/`, `..`, `.` and empty components for every
permission request, so a client cannot even name the store (`permission.rs:155-168`). The product kernel carries
`system-store` (`kernel/Cargo.toml:104`, `:930`; `tools/xtask/src/main.rs:6121`, asserted at `:20940`).

**One latent note (not reachable today).** `is_system_store_path` tests `normalized.split('/').nth(1) == Some(".system")`
after the leading slash is stripped (`kernel/src/fs/mod.rs:1839-1842`). That is exactly right for today's single
`/neofs` mount (`/neofs/.system` → `neofs/.system` → component 1 = `.system`). A future second mount at `/mnt/foo`
would put its store at `/mnt/foo/.system` → `mnt/foo/.system` → component 1 = `foo`, which this check would not
catch. The store is only on `/neofs` today; make the check mount-aware before adding another mount with a `.system`.

## 4. Coverage of grants, answers, actions and undo; growth — PASS for this slice (undo is a later slice)

**The flow is logged at every decision point.** `permission::serve` records:

- a locked request: `DENIED` with `REASON_LOCKED` (`permission.rs:303-307`);
- a prompt: `Asked` before `ask()` (`:314`), then `ALLOWED_ONCE`, `ALLOWED_ALWAYS`, `DENIED` (user or lock), with the
  intent (`:317-351`);
- a lend from a standing grant: `LENT_FROM_RECORD` (`:339-341`);
- an open failure: `REFUSED`/`REASON_OPEN` (`:334-338`);
- a write refused before the prompt: `REFUSED`/`REASON_READ_ONLY` (`:286-290`);
- the cap's completion: `Done` with the outcome, or `Done`/`DONE_UNREPORTED` when a lent cap is revoked without the
  client's report (`:378-396`, `:101-111`);
- a failed path: `REFUSED`/`REASON_PATH` (`:281-284`); a remembered denial: `REFUSED`/`REASON_DENIED_THIS_SESSION`
  (`:291-299`).

All of these pass the intent through `note(...)`/`append(...)` (`permission.rs:58-68`). The client's intent is the
line it acted on (`desktop_actions.rs:62`); it is cut to 255 bytes on the wire and sent as a second message
(`userland/neo-arthur/src/showcase_surface.rs:1107-1123`), and the WM stores it with `printable()` (`?` for
non-printable, so a non-UTF-8 intent is kept, not dropped) (`action_log.rs:86-97`, `permission.rs:259-276`). The
`Done` record deliberately keeps the request's intent and path (`permission.rs:358-359`, `:384-386`).

**Growth is bounded.** The cap is 1 MiB (`action_log.rs:34-40`); past it the oldest half is dropped behind a rotation
marker outside the chain, with the first kept record's sequence and link preserved so the kept bytes and every later
link are unchanged (`:171-199`, `:256-268`; `verify` starts from the marker at `lib.rs:215-225`). A repair keeps at
most 64 KiB of the cut tail (`action_log.rs:41-42`, `:228-233`), and the read is capped at `CAP + IO`
(`:109-125`). A rotated log is at most ~CAP/2 plus the marker and one record.

**Notes (no additional blocker):**

1. **`GrantRevoked` (8) and `Undone` (9) are defined but never emitted in this PR**
   (`userland/action-log-core/src/lib.rs:40-41`, `:54-55`). THINK §4 assigns them to L2.3 (the inspector's Revoke) and
   L1.3b (WM-mediated undo) (`docs/THINK-2026-10-30-l2-arthur-shows-its-work.md:70`), which are later slices. The
   item's "undo" half is therefore not demonstrable at this head; the seven implemented kinds (1-7) are what this
   slice proves. Arthur's existing agentic undo is a direct `write_file` that goes through the prompt as an ordinary
   write, not an `Undone` record.
2. **The `Done` detail is client-claimed.** `desktop_actions::respond` reports its own outcome
   (`userland/neo-arthur/src/desktop_actions.rs:122-134`); the WM records that number. A malicious client can report
   any value, including `0xFFFF_FFFF`, which is also the WM's `DONE_UNREPORTED` sentinel
   (`action-log-core/src/lib.rs:82`, `permission.rs:85`). The WM-attested facts are the request, the answer and the
   lend; the completion is a claim. If the inspector is to distinguish "client said it failed" from "client never
   reported", use a separate kind/flag rather than a detail word the client controls.
3. **The Always grant is persisted before its log record.** `record()` writes `.system/arthur-grants.json` at
   `permission.rs:346-350`; `note(ALLOWED_ALWAYS)` runs after at `:353`. If the append fails, the action is correctly
   refused (`:353-357`) but the grant is already in the record, so the log can show `Asked` with no answer and a later
   `LentFromRecord`. Fail-closed on the action, but an audit inconsistency: write the log record first, or remove the
   entry if the append fails.
4. **A `Done` append failure is ignored.** `note(DONE, ...)` at `permission.rs:384-386` and `:108-110` discards the
   bool, and `lent_for` is already taken, so it is not retried; the action has run (the allow was recorded) but the
   log ends without its done. The serial says `ACTIONLOG append=failed`. A concrete input: make the log
   unappendable at `OP_PERMISSION_DONE` (e.g. a full disk); the binary log keeps the allow and loses the done. The
   pre-action record is the security-relevant one, so this is a note, not a fail.
5. **The shipping image does not enable the feature yet.** The product WM's neo-init feature list
   (`tools/xtask/src/main.rs:6635-6644`, `desktop-with-model`) has neither `arthur-actions` nor `arthur-action-log`;
   the log is compiled only for the `arthur-actions-test`/control scenarios (`:6655-6665`). So image A logs nothing
   today, and the two-boot scenario is the only end-to-end proof. If L2.1 is meant to ship in this batch, add the
   features (with the L1.3b write decision still off); if it is staged behind that decision, say so in the ledger.

## Blocking finding F1 — inherited from §203, extended by this PR (the branch, not the log)

**The permission path still performs blocking reads before it consults the lock.** `permission::serve` reads the
path with `sys_recv` (`userland/neo-init/src/desktop_session/permission.rs:253-257`) and **this PR adds a second
blocking read for the intent** (`:259-263`), both before the `lock::locked()` check at `:304` and before `ask`'s own
check at `:452-455`. `sys_recv` blocks until a matching send (`userland/libneo/src/lib.rs:2067-2078`); the WM's loop
services the lock only at its top (`userland/neo-init/src/desktop_session.rs:1490`), so while `serve` is parked the
lock cannot engage, and while locked the lock screen's field cannot be served.

**Concrete input.** Send a 64-byte `OP_PERMISSION` with `action = PERMISSION_READ`, `path_len = 1`,
`intent_len = 1`, then send the one path byte and **withhold the intent byte**. The WM parks at
`permission.rs:261`. Super+L (or the idle timer) then cannot get the lock screen served; with the lock already up,
the password field is not serviced. The path-only variant (withhold the path byte, `path_len = 1`, `intent_len = 0`)
is the §203 F1 exactly and still parks at `permission.rs:255`. My §203 RETURN (F1, on
`Plans/RETURN-2026-10-31-arthur-prompt-review-dexter.md` @ `dexter/arthur-prompt-review` `3d29b781`) found the path
variant on #248; this PR neither fixes it nor guards the new intent read.

**Fix.** Poll both reads through the existing guarded shape (`desktop_session.rs:2595-2618`:
`try_recv` + `lock::service`/`lock::guard_input`), or refuse a permission request while locked before either read,
and add the stall leg to `login_arthur_permission`. The new intent read must be covered by the same fix — it is a
second instance of CP-57 in the same function.

**Scope.** This is not a defect in `action-log-core` or in the log's append/verify path; the four log items above
pass. It is in `permission.rs`, which the #253 diff includes, and the #253 bounce (`d2b325dd`) touched that file
without fixing it. As dispatched (`git diff origin/main...pr253`), the branch as a whole is blocked by F1 until it
is fixed (on #248, then re-stack #253) or the PM splits the log delta against a #248 head that has the fix.

## Other notes (no additional blocker)

1. **Head mismatch:** the dispatch's `124ea1e7` is not in the branch; the current head is `75eae4f8`. If the PM's
   gate cites `124ea1e7`, it is stale. The reviewed tree is `75eae4f8` on base `924c189c`.
2. **The rotation marker is outside the chain by design** (`lib.rs:215-225`, `:88-90`). It is a marker, not a record
   whose content is vouched for; an attacker with write access could forge a marker at the file's start, but write
   access is exactly the WM-only property of item 3. No new issue.
3. **No QEMU run here.** The host tests I ran are listed above; the 4-shard gate is the PM's before merge (the diff
   touches `kernel/Cargo.toml`/`kernel/src/test_harness.rs` for the test feature).

## Host evidence (this review)

```
cargo test -p action-log-core                            # 8 passed, 0 failed
cargo test -p atrium-logic-tests permission_byte_oracle  # 1 passed, 0 failed
cargo test -p arthur-logic-tests desktop_action_tests    # 2 passed, 0 failed
```

Independent oracle check: reconstructed `R0`/`R1` from THINK §2 with Python `struct`, hashed with
`C:\Users\<user>\.cargo\bin\b3sum.exe`; `L0 = 8f70abda49cbeda3b341af5046d4ada6`,
`L1 = 6db51fe5d29686e058dd787977e012ca`, both equal to `userland/action-log-core/src/tests.rs:21-23` / `:34-36`.

## Disposition

**The action-log slice itself passes all four checklist items** (item 2's cross-boot-tail limit and item 4's
not-yet-shipped/`Undone` scope are named above, not hidden). **The branch as dispatched is not mergeable as a
whole because of F1**: the same `permission.rs` it carries still parks the WM on a client-controlled read before the
lock check, and #253 adds a second such read (the intent). Fix F1 (on #248), re-stack #253, then run the 4-shard
gate on the rebased head; the log code should not need changes for that.

This was a code review. I ran the three host programs above and no gate; I claim no CI or QEMU result.
