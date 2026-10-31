<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — #247 K1b and #253 L2.1 re-check (Section 218, re-queued)

**Seat:** Dexter. **Reviewed at:** #247 head `272bc85a` and #253 head `23a8d98a` (each the current
`origin/claude/k1b-scoped-root` / `origin/claude/l2.1-action-log` head and the matching `pull/247` / `pull/253`
head). **Base:** `origin/main` = `ba12be7d`. **Branch:** `RETURN-2026-10-31-k1b-l21-recheck-dexter`.
**Answers:** `Plans/DISPATCH-2026-10-31-rechecks-215-218.md` §218.
**Read:** `git fetch origin +pull/247/head:pr247 +pull/253/head:pr253`; static review of
`git diff origin/main...272bc85a` and `git diff origin/main...23a8d98a`.
**Re-queued (relay 223).** The first pass reviewed `cf96e17f` / `93cbc705`. Since then the branches were rebased and
each gained one commit: #247 `272bc85a` (the narrow probe leg asks for `EXECUTE`, not `MOUNT`; `THINK:186` follows)
and #253 `23a8d98a` (an as-built THINK paragraph). The PRs' own code and test legs are otherwise unchanged by the
rebase; every line cited below is re-pinned to the new trees.
**Scope:** code and test-leg review only. I ran no QEMU or host gate and claim no gate result; the 4-shard gate is the
PM's before merge. #253 touches `kernel/Cargo.toml` and `kernel/src/test_harness.rs` for the test feature, so it is
kernel-touching for gate purposes.

## Checklist

| # | Item | Verdict |
|---|---|---|
| 1 | #247 F1: is the Lens narrowed, or is a browse-only root justified, and is `THINK:90` corrected? | **PASS** — not narrowed; the browse-only root is justified, and line 90 is corrected. |
| 2 | #247 item 5: are the path-scoping prior-art rows present, with licenses and dates, and do they hold? | **PASS** — four rows; all four cited commits resolve, the licenses hold, and the technical claims hold. One date is US-Pacific rather than UTC (note 1). |
| 3 | #247: is the new `..` leg a real escape test? | **PASS** — it is a discriminating escape attempt. The companion `narrow` leg is now real too (the old `MOUNT` form was feature-dependent). |
| 4 | #253: is the 204 F1 intent read now polled and bounded, with a stall leg? Are the 213 gaps (record-full Always, identity mismatch, escape during the wait) covered? | **PASS** — both reads use the same bounded poll, the intent-stall leg exists, and all three gaps have scenario legs. |

**No FAIL.** The notes at the end are residuals, not blockers; no concrete FAIL input is needed.

---

## 1. #247 F1 — the Lens is not narrowed; a browse-only root is justified; `THINK:90` is corrected — PASS

**The Lens cap stays browse-only.** `lens::bloom` opens `PATH` (`/`) with `OPEN_ROOT_TEMPORARY` and exactly
`FileRights::LOOKUP | READDIR` (`userland/neo-init/src/desktop_session.rs:5371-5375`), and the runtime witness is
`DESKTOP lens bloomed path=/ rights=lookup,readdir focus=none` (`:5413`). The cap has no `READ`, so a file's bytes
cannot be read through it; `system_store_check` refuses `.system/` at the open before the cap is consulted
(`kernel/src/arch/x86_64/syscall.rs:6401-6404`).

**`THINK:90` is corrected.** The threat row now reads: "no client holds a root cap it can **read** through. The one
exception is the Lens: the WM's read-only browser holds `/` with `LOOKUP|READDIR` only. It can list names but can't
open a file for reading, and `.system/` stays refused by `system-store` (`docs/THINK-2026-10-27-ot66-login-session.md:90`).
That replaces the false "no client holds the root cap" text.

**The justification is in the design record.** "K1b as built" records the Lens as the WM's read-only browser, the
names-only exception, and the later L3 narrowing path (`THINK:189-195`). That is the "browse-only root justified"
branch of the dispatch: the Lens is deliberately trusted WM chrome, not a client whose purpose is narrowed. My
former F1 asked for exactly one of "narrow the Lens, or qualify the claim (and say if the Lens is trusted WM
chrome)"; the PR takes the qualification branch and names the Lens as the exception.

**Residual (not this F1):** `sys_spawn_client` still hands a full `/` (all but `MOUNT`) to the shell/installer path
when that path is used; my §211 note 5 named it as a forward note. Image A does not spawn a client that way, and it
is outside the Lens finding. It is unchanged, so it is not a regression of this fix.

## 2. #247 item 5 — the path-scoping prior art is present with licenses and dates; the claims hold — PASS

**The rows are present.** `THINK:204-215` has a four-row table for K1b's path-scoping mechanism:

- FreeBSD Capsicum: `cap_rights_limit` / `openat` beneath a directory fd; `NI_LCF_STRICTREL` /
  `NI_LCF_CAP_DOTDOT`; BSD-2-Clause; `freebsd/freebsd-src` `6d705f2b`, 2026-10-30.
- Linux `openat2` `RESOLVE_BENEATH` / `RESOLVE_NO_SYMLINKS` (`LOOKUP_BENEATH`); GPL-2.0-only;
  `torvalds/linux` `36aad21d`, 2026-10-30.
- Fuchsia directory handles; BSD-3-Clause (not re-verified, with the proxy reason stated); last commit "not
  verified".
- WASI preopens / `wasmtime` / `cap-std`; Apache-2.0 WITH LLVM-exception and Apache-2.0/MIT; `c57fe564`
  2026-10-30 and `b2112115` 2026-10-19.

**The named commits resolve.** I re-fetched the four GitHub commit endpoints:

- `freebsd/freebsd-src` `6d705f2b` = `6d705f2bf17f2c221ffd0d9b0824ba0fb429392e`, 2026-10-31T03:04:10Z
  (2026-10-30 20:04 PDT / 23:04 EDT).
- `torvalds/linux` `36aad21d` = `36aad21d50d1af3f6954bb0c988257a908202477`, 2026-10-31T05:01:54Z
  (2026-10-30 22:01 PDT; 2026-10-31 01:01 EDT).
- `bytecodealliance/wasmtime` `c57fe564` = `c57fe5644237b90edda3132a14e1ca25b4e0cf72`, 2026-10-30T19:15:38Z.
- `bytecodealliance/cap-std` `b2112115` = `b21121159cbbdca93c0d213dfc8e6f66577d6657`, 2026-10-19T22:13:10Z.

The table states no time zone. FreeBSD's 2026-10-30 is that commit's US-Pacific day (and its US-Eastern day);
wasmtime's and cap-std's dates match the commit dates directly; Linux's 2026-10-30 is that commit's US-Pacific day
(note 1).

**The licenses hold.** FreeBSD's project license is BSD-2-Clause (the specific `namei.h` is BSD-3-Clause, as are
some kernel files; the row is the project/Capsicum license); Linux is GPL-2.0-only; wasmtime is Apache-2.0 WITH
LLVM-exception; `cap-std` is Apache-2.0/MIT; Fuchsia is BSD-3-Clause. The Fuchsia row honestly says its license and
commit were not re-verified because the container's proxy refused `fuchsia.googlesource.com`; it is a named
limitation, not a hidden missing row.

**The technical claims hold.** Capsicum bounds what a descriptor can open and keeps `..` against the start
directory; `openat2` performs the beneath check during the walk; a Fuchsia process's namespace is the directory
channels it was handed; WASI preopens resolve every path beneath a preopened directory with `..` refused at the top.
The stated difference is also right: those bind a scope to a directory object (vnode, channel, descriptor), while
K1b binds it to a normalized path and resolves from the root (`THINK:213-215`). K1b's soundness argument — no
part-way symlink traversal, `..` folded before resolution — matches the code (`kernel/src/fs/mod.rs:1894-1926` for
the fold and the whole-component compare), and the mount caveat is stated at `THINK:200-202`.

**Only gap (explicit, not a FAIL):** the Fuchsia row has no verified license/commit date; it says so. If the PM
treats a verified date on every row as hard, that one row is the only place to bounce; the three verified rows and
the K1b-specific claims hold.

## 3. #247 — the new `..` leg is a real escape test; the `narrow` leg is now real too — PASS

**The `..` leg.** The probe adds `probe("/neofs/.arthur/..", dir_rights)` and expects `eperm`
(`userland/neo-arthur/src/main.rs:968-969`); the scenario's required serial line is
`ARTHUR scope outside=eperm inside=ok dotdot=eperm sibling=eperm narrow=eacces`
(`tools/xtask/src/main.rs:16301`, feature wiring at `kernel/Cargo.toml:941`, `userland/neo-arthur/Cargo.toml:73`).

**It exercises the actual escape path.** With `cap-rooted-open`, `sys_open` normalizes the path before the cap
lookup (`kernel/src/arch/x86_64/syscall.rs:6419-6427`); `normalize_absolute_path` pops `..`
(`kernel/src/fs/mod.rs:1903-1926`); `scope_contains` compares whole components (`:1894-1901`). So
`/neofs/.arthur/..` becomes `neofs`, which no held scope contains, and `root_dir_cap_for` returns `EPERM`
(`kernel/src/sched/process.rs:1303-1333`).

**It is discriminating, not cosmetic.** If the check were made on the raw path before folding, the raw string
`neofs/.arthur/..` would pass the scope's prefix and the path would resolve to `/neofs` — the probe would log `ok`,
not `eperm`. If a resolver treated `..` as a literal record and failed, the probe would log `other`, also not
`eperm`. The leg therefore fails both naive alternatives. The companion `sibling` leg (`/neofs/.arthurx`) catches a
raw-prefix/whole-component mistake, and the `narrow` leg catches a rights check that only tests containment
(`main.rs:970-976`).

**The new `narrow` leg is now a real test (commit `272bc85a`).** The old leg asked for `MOUNT`
(`LOOKUP|READDIR|MOUNT`). With `mount-authority` on — the product's feature set — `open_path_and_install` masks
`MOUNT` off a plain open before the scope check (`kernel/src/arch/x86_64/syscall.rs:6405-6414`), so the old leg
would ask for `LOOKUP|READDIR`, find its scope, and log `ok`; it would not test the rights-subset rule. This
scenario's kernel has `mount-authority` off (`kernel/Cargo.toml:941`), which is why the old leg passed the scenario
without proving the product path. The new leg asks for `EXECUTE` (`main.rs:972-976`, `THINK:186`), which
`mount-authority` does not mask and which Arthur's scope does not carry
(`userland/libneo/src/lib.rs:1159-1172`: `/neofs/.arthur` is READ|WRITE|LOOKUP|READDIR). `root_dir_cap_for` sees a
held scope containing the path but not covering the requested right, so it returns `EACCES`
(`kernel/src/sched/process.rs:1322-1331`). The later directory check that rejects READ/WRITE/EXECUTE on a directory
(`kernel/src/fs/mod.rs:1524-1529`) is never reached; if the scope-rights check were skipped, the open would reach
that check and the probe would log `other`, not `eacces` — so the leg still discriminates. The commit is test-only
plus one THINK line; no production behavior changed.

## 4. #253 — the intent read is polled and bounded with a stall leg; the 213 gaps are covered — PASS

**The intent read is polled and bounded.** `recv_part` uses `sys_try_recv`, serves the lock/pointer/SAK on `EAGAIN`,
and returns false at `PART_NS` (`userland/neo-init/src/desktop_session/permission.rs:242-246,259-285`). It is used
for the path (`:311`) and for the intent (`:319-326`). `pointer_tick` calls `lock::service` first
(`desktop_session.rs:414-424`), and `arthur-actions` includes `login`, so the lock is still served while the WM
waits. `PART_NS` is 2 s in the product and 45 s in the boot test; the intent adds at most one more bound, not an
unbounded wait. The session passes its own `escaped` through `Client` when `desktop-pointer` is on
(`desktop_session.rs:1479,1736-1746`), so a pointer/SAK action applied during the wait updates the same flag
`ask` later reads (`permission.rs:342,389`).

**The intent-stall leg exists and would fail a blocking intent read.** `permission_stall(path_sent = true)` sends a
header with `path_len = 1`, `intent_len = 1`, sends the path byte, and withholds the intent
(`userland/neo-arthur/src/showcase_surface.rs:1195-1212`). The boot script then presses Super+L (boot 1) or lets the
idle lock engage (boot 2) and expects `LOCK locked ...`, `LOCK unlocked ...`,
`PERMISSION refused reason=incomplete`, and status 33 (`tools/xtask/src/login_scenarios.rs:246-257,289-308`;
expected `:1195-1200,1363-1368`). A blocking intent read would park before `lock::service`, so the `LOCK` lines
would be missing and the leg would time out.

**213 gap 1 — record-full Always.** `seed_record` fills the 4 KiB record to one entry short, leaving room for
`todo.txt` (Always in boot 1) but not the next `full.txt` (`permission.rs:523-572`). The scenario expects the
`full.txt` Always to answer as once, log `PERMISSION record=failed`, return status 0, and prompt again on the next
read (`login_scenarios.rs:274-279,1220-1227`). A record success would emit `answer=always`/status 30 and fail the
expected lines. The code path is the `record()` failure branch (`permission.rs:420-428`).

**213 gap 2 — identity mismatch.** `bad_identity` sends a request naming another surface and then a done naming
another surface (`userland/neo-arthur/src/desktop_actions.rs:241-282`). The WM reads the path and intent, then
refuses the request with `PERMISSION refused reason=identity` and `CapRequired` status 7
(`permission.rs:340-350`; `userland/libneo/src/atrium_protocol.rs:159-180`); the wrong done is refused with
`PERMISSION done refused reason=identity` and leaves the lent cap in place
(`desktop_session.rs:1764-1783`); the right done revokes it. The scenario checks
`bad_done=7 read=1 good_done=0 read=-` (`login_scenarios.rs:269-276,1210-1218`). The one-byte expectation is the
`93cbc705`/`1f82cc18` fix; the cap being still readable after the wrong done (read=1) and unreadable after the right
done (read=-) is the property.

**213 gap 3 — escape during the wait.** `escapewait` holds the path for 4 s (`desktop_actions.rs:215-229`;
`showcase_surface.rs:1104,1156-1164`). `recv_part` passes the session's mutable `escaped` flag to
`scene.pointer_tick` (`permission.rs:269-280`), so a Super+Escape pressed while the WM waits is applied rather than
consumed and dropped; `pointer::apply` sets `*escaped = !to_window` and swaps the route to the WM
(`desktop_session.rs:5096-5163`, flag at `:5144`). The scenario expects
`DESKTOP pointer escape focus=spine via=sak`, the prompt, `PERMISSION route restored to=wm`, and the Enter refocus
(`login_scenarios.rs:258-267,1201-1209`). The old `pointer_idle` path consumed the escape and left `escaped =
false`; this leg would fail there.

**No regression found in the fix.** The `Client<'a>` change passes the session loop's `escaped` when
`desktop-pointer` is on and a local `false` when it is off (`desktop_session.rs:1736-1746`); the pre-request escape
still feeds `ask`, and the during-wait escape now updates the same flag. The identity refusal is serial-logged and
does not change authority (`desktop_session.rs:1771`). The boot test's `UNLOCKS` is 3 under `arthur-actions-test`,
matching the two stall unlocks plus the final lock leg (`lock.rs:620`). The action-log expected sequence carries
the new records (`login_scenarios.rs:1289-1306`).

**The new THINK paragraph (`23a8d98a`) matches the code.** `docs/THINK-2026-10-30-l2-arthur-shows-its-work.md:57-61`
says a request or done naming another surface is refused (`CapRequired`) before its path is checked and is named in
the serial log `reason=identity` — the identity check is at `permission.rs:340-350`, before `valid_path` at `:355`;
"while the WM waits for a path or an intent it serves the pointer and the SAK chords" — `permission.rs:269-280`;
"the prompt that follows keeps it on the WM" — `permission.rs:641-652`. The parenthetical "(Dexter's #248 re-check,
item 2)" is the 213 re-check's item 2, which named exactly this escape-during-the-wait edge.

## Notes (no additional blocker)

1. **The Linux prior-art row's date is US-Pacific, not UTC.** `36aad21d` is 2026-10-31T05:01:54Z; the row's
   2026-10-30 is that instant in PDT. FreeBSD's `6d705f2b` is 2026-10-31T03:04:10Z, which is 2026-10-30 in both
   PDT and EDT. The table does not state a time zone, so this is internally consistent; if the project wants UTC
   dates in that column, the Linux row is the one to correct. The SHA resolves and the claim holds either way.
2. **`recv_part`'s clock residual.** The deadline is `now_ns().saturating_add(PART_NS)` and `now_ns` uses
   `unwrap_or(0)` (`permission.rs:248-250`). If `CLOCK_MONOTONIC` ever failed, the wait would have no deadline;
   this is the same residual my 213 re-check named for the path read, and the clock is core kernel state. The
   normal path is bounded.
3. **The wait now serves full pointer actions, not only the lock/SAK.** `recv_part` calls `scene.pointer_tick`
   (`permission.rs:271`), so a click or chord during the wait can change focus/layout as in the session loop; the
   new leg covers the SAK escape, not a click. This is a deliberate part of the escape-during-wait fix and I found
   no authority change; it is worth knowing when reading the test coverage.
4. **The identity probe's `read=-` is a substring assertion.** `require_serial_sequence` uses `find`, so
   `read=-` matches `read=-1` but would also match another negative value. The revoke itself is separately checked
   by `PERMISSION done outcome=0 revoked=1`, so this is a weak witness, not a coverage hole.
5. **No QEMU result claimed.** The `login_arthur_permission` legs are scenario wiring and expected lines; the
   4-shard run on the rebased head remains the PM's before merge. #253 touches `kernel/Cargo.toml` and
   `kernel/src/test_harness.rs` for the test feature, so it is still kernel-touching for gate purposes.

## Disposition

**All four checklist items PASS by inspection.** The #247 fix takes the allowed "browse-only root justified"
branch, corrects `THINK:90`, adds the path-scoping prior art with licenses and dates (one row explicitly
unverified), and adds a discriminating `..` leg; the re-queued commit `272bc85a` also fixes the `narrow` leg so it
tests the scope-rights rule rather than a feature-dependent `MOUNT` mask. The #253 intent read uses the same
bounded poll as the path read, has its own stall leg, and the record-full Always, identity-mismatch, and
escape-during-wait legs cover the three 213 gaps; the re-queued `23a8d98a` only adds a THINK paragraph that matches
the code. I found no new defect in the fixes; the residual notes above are not blockers.

This was a code/test review. I ran no QEMU or host gate and claim no CI result.
