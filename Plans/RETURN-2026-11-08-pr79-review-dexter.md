<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 350: #79 (the client grants a separate capability)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md` §350, which runs the **#79** items and the **All four** items of `Plans/DISPATCH-2026-11-08-relays-329-332.md` §329, for #79 only.
**Reviewed:** PR #79 head `67525f8d0fed931b0b8776f483ce7e2a7cd6425d` (`claude/ot61-r4d-grant-v2`), its own diff against #78 `7fd9eaf58cc839cd67deec606f5e600743771dbf`: `git diff refs/pr/78...refs/pr/79` (13 files, +660/−54). `origin/main` is `d0dd0136160f3bb88683c92aed74d193f614ea17`; #79 is stacked on #78 `7fd9eaf5` and #77 `c8a06574`, whose base is `13eed2c5` (base-drift note 1).
**Overall: PASS on all four checklist items.** The surface crosses process boundaries only as a granted, kernel-attenuated handle: the child handle is `USE; MAP_READ|QUERY|PRESENT` even though the recipient asks for every right, and the write/commit/derive/re-grant legs are `EPERM`. The inherited list is explicit (one channel end plus the std-fixture runtime cap), and the kernel installs exactly that list. The no-`GRANT` re-grant refusal is required by an exact xtask serial needle. The kernel feature is opt-in and the standalone crate is workspace-excluded, so the default image is unchanged. The prior art is recorded in the THINK and `VENDORED.md`; #79 adds no new third-party code. The head's own full-tier PR run is green (4/4 QEMU shards, run [`42454689994`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42454689994)). Four non-blocking notes at the end.

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing lines |
|---|---|---|---|
| #79.1 | The client gets only the capability it needs (the surface), with the right rights; nothing is inherited implicitly | **PASS** (notes 2–3) | `kernel/src/test_harness.rs:3131-3148`; `kernel/src/sched/process.rs:4316-4338`; `userland/swgl-smoke/src/grant_client.rs:74-84,119-128`; `kernel/src/arch/x86_64/syscall.rs:8498-8509,8312-8315`; `kernel/src/sched/process.rs:319-324,3873-3881`; `kernel/src/atrium/logic.rs:313-315`; `userland/swgl-smoke/src/grant_compositor.rs:92-112,118-137` |
| #79.2 | A client without the grant is refused, and the scenario proves that refusal | **PASS** (note 3) | `userland/swgl-smoke/src/grant_compositor.rs:133-137`; `kernel/src/arch/x86_64/syscall.rs:8294-8315`; `kernel/src/sched/process.rs:3873-3881`; `tools/xtask/src/main.rs:18861-18878` |
| All.1 | Each feature is off by default, and off builds today's image (kill-switch) | **PASS** | `kernel/Cargo.toml:81,499-501`; `kernel/src/test_harness.rs:177-178,1321-1322,3116`; `Cargo.toml:99`; `userland/swgl-smoke/Cargo.toml:31-43`; `tools/xtask/src/main.rs:2312-2320,7198,8266-8272` |
| All.2 | Prior art: the PR body or THINK carries the survey with licences and dates for what's borrowed | **PASS** | `docs/THINK-2026-10-31-ot61-r4-swgl-floor.md:10-13,38-43,60-71,134-146`; `third_party/ot61-swgl/VENDORED.md:64-70` and the per-crate table |

## #79.1 PASS — the surface crosses only as a granted, attenuated handle; the inherited list is explicit

**Nothing is inherited implicitly.** `spawn_user_body!` starts from an empty `HandleTable` and installs exactly the `$inherited` slice at indices 0..n; there is no other install path in the spawn (`kernel/src/sched/process.rs:4316-4338`). The scenario hands each process exactly two handles (`kernel/src/test_harness.rs:3131-3148`):
- the test fixture's `root_browser_runtime_cap()` (the std thread/TLS authority; both binaries are std-linked), and
- one end of the one `Channel`, with `full_channel_rights()`.

The buffer is **not** in either inherited list. The client creates it itself (`userland/swgl-smoke/src/grant_client.rs:74-79`), and `SYS_BUFFER_CREATE` installs the creator's root with `full_buffer_rights()` — `USE|DERIVE|GRANT|REVOKE` meta plus `MAP_WRITE|MAP_READ|QUERY|PRESENT|COMMIT` (`kernel/src/sched/process.rs:319-324`; `kernel/src/arch/x86_64/syscall.rs:8498-8509`). That is the authority the owner needs to map RW, commit, and delegate a compositor view; it is created, not inherited.

**The offer is the right subset.** The client grants `META_USE` and exactly `MAP_READ | QUERY | PRESENT` (`userland/swgl-smoke/src/grant_client.rs:119-128`) — no `MAP_WRITE`, no `COMMIT`, no `DERIVE`, no `GRANT`. The compositor asks for **more** than the offer: `META_USE | META_DERIVE | META_GRANT` and every buffer semantic bit (`userland/swgl-smoke/src/grant_compositor.rs:92-98`). The kernel computes `source ∩ offered ∩ max_rights` (`kernel/src/sched/process.rs:3873-3881`; the pure predicate is `kernel/src/atrium/logic.rs:313-315`), so the extra request cannot widen the result.

**The result is checked, not assumed.** The compositor reads its own handle record and requires exactly `HandleKind::Buffer`, `cap_meta == META_USE`, `cap_semantic == MAP_READ | QUERY | PRESENT` (`userland/swgl-smoke/src/grant_compositor.rs:101-112`). The grant is a derived copy (fresh `CapId`, donor keeps the source: `kernel/src/ipc/mod.rs:489-494`), so the client's own handle is unaffected.

**The excess is refused at run time.** The compositor's handle cannot map the client (RW) view (`EPERM`, `grant_compositor.rs:118-122`), commit damage (`EPERM`, `:123-127`), derive a narrower handle (`EPERM`, no `DERIVE`, `:128-132`), or grant onwards (`EPERM`, no `GRANT`, `:133-137`). The kernel enforces each at the syscall: `sys_grant_impl` refuses a source without `meta_rights::GRANT` (`kernel/src/arch/x86_64/syscall.rs:8312-8315`), and `complete_grant` re-checks the same bit (`kernel/src/sched/process.rs:3873-3876`). There is no widening path.

Note 2 records the one broad fixture: both std binaries also get the runtime cap, which this scenario never exercises (it spawns no thread). It is explicit in the handle list, not implicit, and it does not carry any surface authority.

## #79.2 PASS — a holder without the `GRANT` right is refused, and the harness requires the refusal line

The compositor's granted handle carries `META_USE` only, so it has no `GRANT` meta right. Its attempt to hand the surface on — `channel::grant(chan, buf, META_USE, MAP_READ)` — is refused `EPERM` (`userland/swgl-smoke/src/grant_compositor.rs:133-137`). The kernel refuses in two places: `sys_grant_impl` resolves the channel with `SEND` (`kernel/src/arch/x86_64/syscall.rs:8294-8298`), resolves the source (`:8300-8311`), then returns `EPERM` when the source lacks `meta_rights::GRANT` (`:8312-8315`); `complete_grant` repeats the check before installing (`kernel/src/sched/process.rs:3873-3876`).

**The scenario proves the refusal and the harness requires the proof.** `require_observed_pass` for `swgl_grant_smoke` requires the exact serial line `[ swgl-grant-compositor ] ok(8) it cannot be granted onwards (EPERM, no GRANT)`, plus `ok(3)` (exactly the offer), `ok(5)` (cannot map the client view), `ok(10)` (the bytes), and both `PASS` lines (`tools/xtask/src/main.rs:18861-18878`). A scenario that dropped the re-grant attempt, or got a different errno, cannot pass the harness. The same block also forces the `ok(5)` refusal; the unchecked-by-needle `check(6)`/`check(7)` would still fail the run before `PASS`, because the compositor's `fail()` exits non-zero and the client treats a non-zero verdict as failure.

Note 3 records the scope of "without the grant": the refusal exercised is the **no-`GRANT`-meta** case (plus the attenuated-rights refusals), not a third process that holds no handle at all. The kernel's `channel_lookup`/`buffer_handle_lookup` return `EBADF` for a handle the process does not hold, so the absent-handle case is the normal syscall path; the dispatch's "xtask/harness line" is the `ok(8)` needle, which matches the no-`GRANT`-right reading.

## All.1 PASS — the feature is opt-in; off is today's image

`kernel/Cargo.toml:499-501` defines `neo-test-swgl-grant-smoke = ["neo-test", "browser-runtime", "mem-quota", "atrium-compositor-g1", "neo-test-atrium-runtime"]`; it is **not** in `default` (`kernel/Cargo.toml:81`). The harness `SCENARIO` constant and the two-process block are both `#[cfg(feature = "neo-test-swgl-grant-smoke")]` (`kernel/src/test_harness.rs:177-178,1321-1322,3116`), so a default kernel has no scenario constant and no block.

The standalone crate is excluded from the root workspace (`Cargo.toml:99`), so `cargo build --workspace` never builds the new binaries. The two `[[bin]]` entries are declared in `userland/swgl-smoke/Cargo.toml:31-43`, and xtask builds them only when the scenario stages them (`tools/xtask/src/main.rs:7198,8266-8272`). The scenario is registered `ignored: false` (`:2312-2320`), so it runs when named; it is not part of any default image. The default kernel and image are unchanged.

## All.2 PASS — the prior art is recorded, and #79 borrows no new third-party code

The THINK records the borrowed closure — SWGL/`glsl-to-cxx`/`webrender_build` 0.70.0 at pin `2e341fa1`, gleam 0.15.1, glsl 6, `gl_generator`, with licences MPL-2.0/MIT/Apache-2.0/BSD-3-Clause/Unlicense (`docs/THINK-2026-10-31-ot61-r4-swgl-floor.md:10-13`); WebRender's 0.70 licence mix including `r-efi` under MIT and `unicode-ident`'s Unicode-3.0 (`:38-43`); and the font-backend prior-art survey with per-candidate licences and last-commit dates (`:60-71`). The S3b section itself says what it reuses and what it leaves to rung 6 (`:134-146`). `VENDORED.md` is headed "checked 2026-10-31" and gives every crate's licence, repository and upstream commit (`third_party/ot61-swgl/VENDORED.md:64-70` and the table).

#79 adds no new third-party code: the grant/channel plumbing, `pattern.rs`, the two binaries and the harness/xtask rows are first-party; the client's only upstream code is the already-surveyed SWGL 0.70.0/gleam, and the compositor uses neither. The PR body points at the rung-4 THINK. That satisfies "licences and dates for what's borrowed".

## Verification performed

- `git fetch origin +pull/79/head:refs/pr/79`; head `67525f8d` matches the dispatch; the own diff is one commit over #78 (`git log refs/pr/78..refs/pr/79`).
- `git diff --stat refs/pr/78...refs/pr/79`: 13 files, +660/−54; every file read.
- Read the kernel grant path end to end: `sys_grant_impl` (`kernel/src/arch/x86_64/syscall.rs:8294-8352`), `sys_recv_grant_impl` (`:8354-8371`), `Channel::grant`/`recv_grant` (`kernel/src/ipc/mod.rs:489-592`), `complete_grant` (`kernel/src/sched/process.rs:3842-3926`), `full_buffer_rights`/`full_channel_rights` (`:258-266,319-324`), `granted_semantic` (`kernel/src/atrium/logic.rs:313-315`), and the spawn install loop (`kernel/src/sched/process.rs:4316-4338`).
- Ran at the review head `cargo test -p xtask ot61_c05_tests`: 41 passed, 0 failed (includes `a4_gate_passes_the_real_pal_and_overlay` and both S3B checks); `cargo fmt --all --check`: clean.
- Head check-runs at `67525f8d`: full-tier `CI` run [`42454689994`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42454689994), `pull_request`, success — 4/4 QEMU shards, Build Checks, Host Tests/Format/Lint, fuzz, RNG seed boot; Batch report skipped (not a batch).
- RAG: not needed for this review (exact syscall/rights lookups and a diff read); used `git show`/grep/read.
- Working tree: this RETURN only on `dexter/pr79-review`; the pre-existing untracked `pr59-review/` directory is untouched.

## Non-blocking notes

1. **Stacked base / drift.** #79 is on #78 `7fd9eaf5`, on #77 `c8a06574`, whose base is `13eed2c5`; `origin/main` is now `d0dd01361`. This review is of #79's own diff against #78, as §350 asks. #79 cannot be assembled alone: it needs #77+#78 below it, so the PM's batch evidence is the merge evidence for the assembled tree. The head's green run is #79's own `pull_request` run on the #78 base, not a batch run.
2. **Both std processes are handed the `BrowserRuntime` fixture cap.** It is explicit, and the std overlay uses it only for `SYS_THREAD_SET_TLS`/thread creation (`userland/neoplutarch-std-overlay/std/neoplutarch_rt/tls.rs:148-160`), which this scenario does not exercise. It carries no surface authority; not a defect for this test. A stricter fixture could drop it for both binaries.
3. **"No grant at all" is not separately exercised.** The refused case is the granted-but-no-`GRANT`-right handle, plus `MAP_WRITE`/`COMMIT`/`DERIVE` attenuation. A third process holding no handle at all would take the `EBADF` path (`kernel/src/arch/x86_64/syscall.rs:8300-8311`), which no scenario line covers. If the PM meant that case, add it; it does not weaken the property proved here.
4. **Cosmetic.** The client reuses check number 8 for both the verdict receive (`grant_client.rs:132-136`) and the verdict-length check (`:137`); a failure in either exits 8. The compositor's check 8 is the re-grant refusal. The PR body's "Gate: pending" is stale — the head has a green full-tier run (above).

## Recommendation to the PM

**PASS — #79 clears these four items.** The surface is created by the client and crosses only as a kernel-attenuated grant (`USE; MAP_READ|QUERY|PRESENT`), the recipient's extra request is intersected away, and the write/commit/derive/re-grant legs are refused `EPERM`; the inherited list is explicit and contains no surface handle. The scenario and the xtask needle require the no-`GRANT` refusal. The feature is opt-in and the crate is workspace-excluded, so off is today's image. The prior art is recorded and no new third-party code is added. Merge on the PM's assembled batch evidence for #77+#78+#79 (and #80 on top if it is in the batch), and include the A4 manifest diff in the AAG as the PR body requests.
