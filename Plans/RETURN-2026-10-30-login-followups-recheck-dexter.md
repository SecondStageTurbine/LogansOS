<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — #243 login follow-ups: security re-check (Section 196)

**Seat:** Dexter (DeepSeek family) · **Re-check of:** my `Plans/RETURN-2026-10-30-login-recheck-dexter.md`, items 3b, G1 and qualifiers A and B
**Reviewed at:** `#243` head `7947a189b7c5a0d3304fcace20afe7aa05edc7db` (`pr243`)
**Branch:** `dexter/login-followups-recheck-dexter` · **Base:** `origin/main` = `1c35aaac`
**Diff:** `git diff origin/main...pr243` — one commit, 5 files, +133/−28 (merge-base `46fa59fd`)
**RAG:** healthy (`{"status":"healthy","ollama":true,"qdrant":true}`). Sources read from the `pr243` tree.
**Scope:** code review at the PR head; I ran no QEMU gate. Push CI on the head is green (run
<https://github.com/SecondStageTurbine/LogansOS/actions/runs/41123949038>: fmt, lint, host tests, product build, feature
matrix), but its **QEMU shards are SKIPPED** — there is no 4-shard evidence on this SHA. Line numbers are at `7947a189`.

## Verdict

| # | Item | Verdict |
|---|---|---|
| 1 | 3b — `show()` applies a saved count's delay at the lock's first try | **CLOSED** (not witnessed; note 2) |
| 2 | G1 — the reset retries and logs a reset that still fails | **CLOSED** |
| 3 | Qualifier A — the root mount's own `/.system` is guarded | **CLOSED** |
| 4 | Qualifier B — folded spellings, relative, root `.system`, `SYS_MODEL_LOAD`, byte-unchanged check | **CLOSED** |
| 5 | Q2 — the root match avoids over-matching | **PASS** |
| 6 | Q3 — can the reset retry loop forever | **No** |

---

## 1. 3b — CLOSED

`show()` still loads the persisted count (`userland/neo-init/src/desktop_session/lock.rs:299-300`), and now derives the
delay from it (`:303`) instead of zeroing the field: `wait_until = now_ns() + delay_secs(failures)` (`:304-308`), the
note (`:309`), a `LOCK backoff wait_s=… at=show` line (`:310-316`), and it withholds the immediate
`LOCK prompt=password` while a delay is pending (`:319-322`). The enforcement is `tick()`'s expiry branch (`:333-338`)
plus the key drop while `wait_until != 0` (`:346-349`); `attempt()` is reachable only after that flag clears
(`:354-357`). The same field is still armed on a failed attempt in the same lock (`:396-417`).

The delay table is `userland/login-core/src/lib.rs:356-366` (0 below 3, 1 s at 3, doubling, 60 s cap) — the same
function sign-in applies before its first check (`login.rs:691-693`), so lock and sign-in now agree. The old
`s.wait_until = 0` is gone.

Caveat: no scenario reaches `show()` with a nonzero count (each lock scenario signs in successfully first, which
resets), so the new `delay != 0` branch is not executed by the gate — note 2.

## 2. G1 — CLOSED

`Attempts::reset` (`userland/neo-init/src/desktop_session/login.rs:280-293`) writes `set(0)`, retries up to
`RESET_TRIES = 3` (`:303`), and after a third failure emits
`LOGIN attempts reset=failed at=<where> failures=<n>`. All three former `let _ = attempts.set(0)` sites now call it:
sign-in (`:699`), recovery (`:765`), unlock (`lock.rs:390`). `git grep 'attempts\.set(0)'` at this head finds nothing.

Fail-safe is preserved: `set` (`:259-274`) only updates `state` after `write_whole` returns true, so a failed reset
leaves the valid copy at the old count — it can charge a delay, never grant a try. The retry is finite (Q3), and the
failure is now visible, which is what G1 asked for.

## 3. Qualifier A — CLOSED

`is_system_store_path` (`kernel/src/fs/mod.rs:1812-1820`) now matches component 0 **or** component 1 being exactly
`.system`:

```rust
let mut parts = normalized.split('/');
parts.next() == Some(".system") || parts.next() == Some(".system")
```

The check runs before any resolution for both path-taking syscalls: `SYS_OPEN` (`kernel/src/arch/x86_64/syscall.rs:6387-6390`)
and `SYS_MODEL_LOAD` (`:2925-2928`), through `system_store_check` (`:8788-8795`), so a root `/.system/…` is `EPERM`
without the VFS being consulted. It is compiled into image A (`system-store` in
`PRODUCT_IMAGE_KERNEL_FEATURES`, `tools/xtask/src/main.rs:6043`), not just the test image.

I copied `is_system_store_path` + `normalize_absolute_path` verbatim into a scratch binary (`VfsError` stubbed) and
ran 17 inputs: all root spellings (`/.system`, `/.system/account.cred.a`, `//.system//…`, `/.system/../.system/…`)
matched; `.systemX`, `.systematic`, `.systemx` did not; the child forms matched; `/neofs/x/.system` and `/` did not;
a relative path was `Invalid`. 17/17 as intended.

## 4. Qualifier B — CLOSED

`userland/neo-worker/src/record_guard.rs` now probes `read`, `write`, `dotdot`, `dir`, `create`, `slashes` (`//`),
`dot` (`./`), `relative`, `root` (`/.system`) and `control` (`/neofs`) (`:43-94`), and `model` is a `SYS_MODEL_LOAD`
of the record (`:120-124`). A write open that **succeeds** is no longer passive: the probe replaces the record's bytes
(`SYS_FILE_REPLACE`, or `sys_write` where that syscall is not compiled in) (`:105-115`).

The WM witness is `login.rs:968` (before) and `:996-1002` (after): both copies must be byte-identical and copy A must
have been readable, or `LOGIN record guard unchanged=1` is not emitted. The scenario asserts every value
(`tools/xtask/src/login_scenarios.rs:374-386`): `read/write/dotdot/dir/create/slashes/dot/root = -1` (`EPERM`),
`relative = -22` (`EINVAL`), `control = ok`, `model = -1`, `unchanged = 1`. EPERM/EINVAL are -1/-22
(`userland/libneo/src/lib.rs:321`, `:338`); the relative path is refused by `normalize_absolute_path` before
resolution (`fs/mod.rs:1822-1825`), and the scenario image `neo-test-login` enables `system-store` while
`cap-rooted-open` is a separate feature not in its list (`kernel/Cargo.toml:922`; `:97`), so that open reaches the
normalizer rather than a root-cap refusal.

The scenario is `ignored: false` (`tools/xtask/src/main.rs:2828-2837`), i.e. in the default suite the 4-shard gate
runs. The probe is real in both directions: if any guard weakens, its `-1`/`ok` value changes, and if a write open
succeeds the bytes change and `unchanged` reads 0.

## 5. Q2 — the root match does not over-match — PASS

The predicate is exact component equality, not a substring. The first `next()` consumes component 0; the second
reads component 1, and `||` short-circuits, so a root match does not advance twice. The normalizer drops the leading
`/`, so component 0 is the root/mount name.

| Path | Match | Why |
|---|---|---|
| `/.system/account.cred.a` | yes | root mount's own `.system` (the new case) |
| `/.system` | yes | same, the directory itself |
| `/.systemX/a`, `/.systematic` | **no** | component equality is exact |
| `/x/.system/a` | yes | child-mount shape; matched by the old `nth(1)` too — not a widening |
| `/neofs/x/.system/a` | **no** | third component; the predicate only looks at two |
| `/` | no | no components |
| `neofs/.system/a` | no | unnormalizable (`EINVAL` before any check) |

The match is deny-only (`system_store_check` can only return `EPERM`), so a false positive costs the availability of
a `.system` directory under a top-level name, never access. `/x/.system` is the same shape as `/neofs/.system`; the
code cannot tell a mount name from a plain directory without resolving, and the conservative read is the safe one.
So: `.systemX` is avoided; `/x/.system` matches by the pre-existing child-mount rule, and the root addition widens
nothing else.

## 6. Q3 — the reset retry cannot loop forever — No

`reset`'s loop is `for _ in 0..RESET_TRIES` (`login.rs:281`, `RESET_TRIES = 3` at `:303`); it returns on the first
success and otherwise exits after three iterations. It is called once per successful sign-in / recovery / unlock
(`login.rs:699`, `:765`, `lock.rs:390`), never from an outer retry loop. `set` (`:259-274`) is one `next` (a
`checked_add`, `login-core/src/lib.rs:560-565`) and one `write_whole`, and `read_small` is bounded by its 512-byte
buffer. Worst case: three failed writes, one log line, and the unlock/sign-in proceeds; the next lock charges the
persisted count through 3b — a delay, not a hang.

---

## Notes (non-blocking)

1. **The base is stale — snapshot at `1c35aaac`.** Merge-base `46fa59fd`; `origin/main` is 30 commits ahead and the
   reviewed files moved on main: `lock.rs` (+24, #222), `login.rs` (+17, #222), `fs/mod.rs` (+27, #246), and
   `login_scenarios.rs` (+476, #222/#246). The four fixes' hunks sit in code main did not change (`show`'s body, the
   `Attempts::reset` insertion, the predicate body; main's `fs/mod.rs` addition is the `power-clean` block at `:602`,
   and main's predicate is still the old `nth(1)` form), so the rebase should apply. But merge still needs the rebase
   plus a fresh 4-shard gate, and these conclusions are stated for `7947a189`; re-verify at the rebased SHA. This
   number is a snapshot, not a standing fact.
2. **3b's new branch is not witnessed.** No QEMU scenario reaches `show()` with `failures >= 3` — every lock
   scenario signs in first (reset), and the only nonzero case is a failed reset, which no test injects. So the
   `LOCK backoff wait_s=… at=show` line, the withheld prompt and the expiry branch (`lock.rs:333-338`) are unexecuted.
   A cheap gate would seed the attempts record (or add a test hook) and assert the line and the delayed prompt. Not a
   security hole; a falsifiability gap of the same class qualifier B just closed for the record guard.
3. **The expiry message is wrong for a carried delay.** `tick()` sets `note = WRONG` ("That password isn't right.")
   when the delay ends (`lock.rs:333-337`). For a delay armed by a failed attempt in this lock that is accurate; for
   a delay carried in by `show()` no attempt has been made, so the screen blames a password the user never typed.
   Cosmetic — the enforcement is correct.
4. **`.system` is a legal mount name.** `validate_mount_name` rejects only empty, `.`, `..` and `/`
   (`fs/mod.rs:1150-1155`). A root-level mount named `.system` would now be `EPERM` for every non-holder, whole mount,
   not just a record store. Deny-only and it needs the mount authority (#240), so it is not a hole; rejecting the
   name would keep the match strictly about the record store.

## Disposition

**All four CLOSED; Q2 PASS; Q3 no.** 3b applies the saved delay at the lock's first try with the same function
sign-in uses; G1's reset is bounded, retried and logged; the root `.system` is matched on exact components and
refused before resolution; and qualifier B's probes are real refusals with a byte-level witness and an `ignored:
false` gate line. No bypass found at `7947a189`.

Merge remains gated on the rebase to current `main` and the 4-shard QEMU gate (whose `login_record_guard` run is the
falsifiable part of B); 3b's ungated branch (note 2) is a follow-up, not a blocker.

This was a code review. I ran no QEMU gate and claim no gate result; push CI's QEMU shards were skipped on this head.
