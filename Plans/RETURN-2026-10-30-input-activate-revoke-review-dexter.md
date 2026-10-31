<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — #185 security review: input-route ACTIVATE vs revoke

**Seat:** Dexter (DeepSeek family) · **Relay:** `#185`, slug `input-activate-revoke`
**Reviewed at:** `origin/main` = `002160eba0ce3e7957b64db67dbc6fcb47ae2b5b`
**Subject:** #210, merged as `ff4bdd51ca5c5c7a9ae29632dc19784c37ea987d`
**Branch:** `dexter/security-review-2026-10-30`

## Verdict

| # | Item | Verdict |
|---|---|---|
| 1 | Non-owner cannot get usable ACTIVATE | **PASS** |
| 2 | RECEIVE-only refused ACTIVATE, defined errno, route unchanged | **PASS** |
| 3 | Revoking the owner's cap ends every minted/derived ACTIVATE | **FAIL** |
| 4 | Scenario's negative leg fails under the old rule | **PASS** |

One FAIL. It is the item Muse's #167 found FAILing, and it is still FAILing on current `origin/main`.

---

## 1. Can a non-owner get ACTIVATE by minting, deriving, transferring, or a spawn grant? — PASS

No path yields **usable** route authority. Four paths checked:

**Minting.** `sys_input_focus_mint_activator_impl` gates on `input_focus_handle_lookup(h, focus_rights::RECEIVE)`
(`kernel/src/arch/x86_64/syscall.rs:8462`). `RECEIVE` is carried only by `client_input_focus_rights()`
(`kernel/src/sched/process.rs:305-308`); the ACTIVATE sibling (`process.rs:321-324`) has no RECEIVE. So a process
can mint ACTIVATE only on a focus it already owns — that is the designed Arthur→WM flow.

**But ownership is not the gate, and it does not need to be.** `sys_input_focus_activate_impl` refuses unless the
caller holds the input-system authority (`syscall.rs:8495-8497`), which is exact-kind (`process.rs:1865-1871`) and
non-delegable (`process.rs:359-361` — `USE` only, no `DERIVE`/`GRANT`). Any process can create a focus and mint
itself an ACTIVATE sibling; it still gets `EPERM` on `SYS_INPUT_FOCUS_ACTIVATE`. That is the #210 F1 fix.

**Deriving.** `derive_handle` requires `DERIVE` on the parent and a subset child (`process.rs:2725-2730`). The
compositor sibling has `USE|GRANT` and no `DERIVE` (`process.rs:321-324`), so an ACTIVATE cannot be derived. The
root cannot derive one either: `ACTIVATE` is not a subset of `RECEIVE`.

**Channel transfer.** `sys_grant` needs `GRANT` (`syscall.rs:7804-7807`), which the activator has, so the WM can
re-grant it. The recipient still needs input-system to *use* it, and that cap cannot be transferred
(`process.rs:359-361`, refused at `syscall.rs:1896-1904` because it lacks `GRANT`).

**Spawn grant.** `sys_spawn_impl` records `parent: Some(h.cap_id())` for every inheritable handle
(`syscall.rs:1896-1903`) and the generic path also requires `GRANT`. The input-system cap carries no `GRANT`, so
`sys_spawn` refuses it; only the kernel's own `InheritedHandle` puts it into the session root
(`kernel/src/main.rs:455`, `:485`).

The single route-setter in the kernel is that one gated syscall — `focus::InputFocus::activate` has exactly one
caller (`grep '\.activate()'` ⇒ `syscall.rs:8501`).

## 2. Is a RECEIVE-only holder refused ACTIVATE, with a defined errno, and the route unchanged? — PASS

`EPERM` in both readings, and no state change.

- Without input-system: `caller_holds_input_system()` returns `EPERM` (`syscall.rs:8686-8692`) before the handle is
  even resolved.
- With input-system but a RECEIVE-only handle: `input_focus_handle_lookup(h, ACTIVATE)` returns `EPERM` because
  `rights.has_semantic(ACTIVATE)` is false (`syscall.rs:7911-7915`).

Route unchanged: both refusals return before `f.activate()` (`syscall.rs:8498-8502`), and `activate` is the only
mutator of `ACTIVE_INPUT_FOCUS`/the route set (`kernel/src/atrium/focus.rs:200-218`). The errno is `EPERM`
(not `EACCES`) on every path; `EBADF` for a non-focus/wrong-variant handle is correct and distinct.

## 3. Does revoking the owner's capability end every ACTIVATE minted from it? — FAIL

**No.** The mint writes no edge in the capability graph, so `revoke` cannot reach the minted sibling — and it
cannot reach anything granted downstream of it either.

### Enforcing lines

- `kernel/src/arch/x86_64/syscall.rs:8470-8478` — the mint installs the sibling with **`parent: None`** (line 8477).
- `kernel/src/sched/process.rs:2805-2884` — `revoke_handle` revokes the transitive closure of `derivations`
  edges only: `revoked.insert(root.cap_id())` (`:2842`), then the walk at `:2852-2860` follows `parent ∈ revoked`,
  then removal at `:2870-2874`. A `None` parent makes the minted cap a **root** of that forest.
- Compare the two paths that *do* record the edge: `complete_grant` passes `Some(source_cap_id)`
  (`process.rs:3187-3190`), and `sys_spawn` passes `Some(h.cap_id())` (`syscall.rs:1896-1903`). The mint is the
  one edge-less install of a *more*-privileged sibling.

### Exact sequence

1. Client `P` (holds the root `R`: `USE|DERIVE|GRANT|REVOKE|RECEIVE`, `process.rs:305-308`) calls
   `SYS_INPUT_FOCUS_MINT_ACTIVATOR(R)`.
2. Kernel installs `A` = `{ cap_id: fresh, rights: USE|GRANT|ACTIVATE }` with `parent = None` (`syscall.rs:8477`).
   Graph: `R` and `A` are **two unrelated roots**.
3. `P` grants `A` to the WM `W` over a channel. `W`'s copy `A'` gets `parent = Some(a)`
   (`process.rs:3187-3190`). Graph: `A → A'`, still disconnected from `R`.
4. `P` calls `SYS_REVOKE(R)` (`syscall.rs:7779-7784`).
5. `revoke_handle` revokes `{r}` and every descendant of `r`. There are none. `A` and `A'` survive, still
   holding the same `Arc<InputFocus>`.

### What the holder can still do afterwards

`W` (the input-system holder) can still `SYS_INPUT_FOCUS_ACTIVATE(A')` → `0`, and
`SYS_INPUT_POINTER_FORWARD(route, tag, A', x, y)` → `0`. The route then points at a focus whose owner was revoked.

The residual authority is **ACTIVATE-only**, and the impact is bounded — I am stating the bound rather than
inflating the finding:

- **No keystroke disclosure.** `RECEIVE` cannot survive the revoke: every RECEIVE-bearing cap descends from `R`
  by a *recorded* edge (`derive_handle` `process.rs:2742-2743`, `complete_grant` `process.rs:3189`,
  `sys_spawn` `syscall.rs:1896-1903`), so the closure removes them. `try_recv` needs a RECEIVE cap
  (`syscall.rs:8594-8603`). So the orphan's queue has no reader.
- **No memory-safety bug.** The surviving handle keeps the `Arc<InputFocus>` alive; there is no UAF.
- **What is left:** (a) the knob to make an undrainable focus the active route, so keys and pointer records are
  enqueued and discarded — a silent input sink while that route is held; (b) the `Arc<InputFocus>` pinned for the
  life of `A'` (object-lifetime leak); (c) the invariant itself, which is the real problem: the reachability
  closure is defeated at this one edge, so a future consumer that trusts revoke as full containment will be wrong.

`InputFocus::drop` clearing the route (`focus.rs:701-707`) does not help, because the object never drops.

### What the fix should key on

**The minted handle's parent in the capability graph.** Not a route epoch.

`sys_input_focus_mint_activator_impl` already resolves the root inside `input_focus_handle_lookup`, which
discards the `cap_id`; the fix is to read the root's `cap_id` under the same handle-table lock and pass
`Some(root_cap_id)` where `None` is today (`syscall.rs:8477`). That is one line of intent plus plumbing, and it
makes the existing revoke walk do the right thing with no new mechanism:

```rust
// sys_input_focus_mint_activator_impl
let (focus, root_cap_id) = input_focus_handle_lookup_with_cap(h, focus_rights::RECEIVE)?;
process::install_cap(&proc, Handle::InputFocus { focus, cap_id: .., rights: .. }, Some(root_cap_id))
```

A route epoch would be the wrong key: authority here is per-*object* derivation, which the graph already models
correctly for derive/grant/spawn. Epochs would need their own revocation rules and would not compose with the
existing `CAP_GRAPH` proof obligations (CP-87).

One caveat to record with the fix: **process teardown re-parents, it does not revoke** (`process.rs:2506-2531`), so
`Some(root_cap_id)` fixes *explicit* `SYS_REVOKE` only. On `P`'s death the edge would be re-parented and `A'` would
still survive. That is the documented CP-88 semantics for every capability (a grantee must not lose authority
because the grantor exited), so it is consistent — but the invariant should be stated as "revoke ends it", not
"owner exit ends it".

## 4. Does the scenario's negative leg fail with the old rule? — PASS

Yes, it is genuinely discriminating.

`probe_input_system_authority` (`userland/neo-arthur/src/showcase_surface.rs:947-975`, added by `ff4bdd51`) creates
its own focus, mints an ACTIVATE sibling, calls `sys_input_focus_activate(activator)`, and requires `EPERM`;
otherwise it logs `DESKTOP arthur input_system NOT_REFUSED`.

Under the old rule (no `caller_holds_input_system()` in `sys_input_focus_activate_impl`) that call reaches
`f.activate()`, which returns `true` (the route set has room) → `0`; had the set been full it would have been
`EBUSY` (`syscall.rs:8501`, `focus.rs:207-209`). Either way the returned value is never `EPERM`, so `ok` is false,
`NOT_REFUSED` is logged, and the gate fails: the needle is enforced in order by
`tools/xtask/src/main.rs:15769-15776` for `desktop_pointer_boot` / `desktop_resize_boot` via
`require_serial_sequence`. The scenario also depends on the side effect (with the old rule the fresh focus steals
the route and the scenario's later keys never reach Arthur), so it fails twice over.

---

## Notes for the PM

- **The dispatch's file list is wrong for the sibling relay.** `kernel/src/input/focus.rs` does not exist at
  `origin/main` or at `45798eb3`. The SAK/lock code is `kernel/src/atrium/sak_logic.rs` +
  `kernel/src/atrium/focus.rs` + `userland/neo-init/src/desktop_session/lock.rs`. Reported here only so the
  dispatch can be corrected; it did not block this review.
- RAG health-checked before use: `{"status":"healthy","ollama":true,"qdrant":true}`. Source reads were direct.
- This item is a **FAIL** and should bounce before any merge that claims revoke containment for input routes.
  It is not a regression from `ff4bdd51`; `ff4bdd51` fixed item 1 and left item 3 as it was.
