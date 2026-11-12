<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Relay 406 — PR #108, A3p CP-61 re-check

Reviewer: Janus (Codex).
Dispatch: `Plans/DISPATCH-2026-11-12-relays-405-406.md`, Section 406.
Reviewed head: `a6b3bcf3160615111a11150eb9798f49604062af`.
RETURN branch: `codex/pr108-recheck-janus`, based on `origin/main`.
Coordinates refer to the reviewed head. No implementation changes.

**Verdict: FAIL (P2, CP-61 evidence gap remains).** Numeric-handle reuse
and generation exclusion are now checked. However, a duration around a
userspace syscall does not force death during a kernel park, and the new
generation's surface-ready bit does not identify its first request. These
are discriminator failures, not demonstrated production lost wakes.

## 1. Required park → death → numeric-handle reuse ordering: FAIL

The new code makes substantial progress:

- `tools/xtask/src/main.rs:8794` selects the witness for the reuse scenario;
  `:8964` selects the probe's idle-before-exit feature.
- `userland/neo-init/src/desktop_session.rs:2175` builds the live client
  items; `:2189` logs the client table; `:2190` captures a timestamp;
  `:2192` calls `Scene::park`; `:2194` logs returned readiness and elapsed
  time. `witness_wake` decodes the actual returned mask at `:3420`.
- `userland/neo-worker/src/probe_app.rs:284` logs an idle, `:285` waits
  two seconds, and `:287` exits 7. The futex deadline loop is at `:302`.
- `tools/xtask/src/main.rs:9446` requires gen 1 connected, then its set,
  then the idle and its end. `:9454` selects a gen-1 wake between idle and
  end; `:9465` requires at least 500 ms. `:9474` requires the next changed
  set to be empty. `:9488` reads gen 2's handles and `:9490` requires both
  numeric handles to equal gen 1's.

The author's [exact-head report](https://github.com/SecondStageTurbine/LogansOS/pull/108#issuecomment-6830611509)
provides these ordered lines (author-reported local serial, not an
independently recovered QEMU trace):

```text
DESKTOP apps client connected client=1 value=1
DESKTOP park set c1g1 f=22 s=19
PROBEAPP idling before exit ms=2000
DESKTOP park woke ms=2010 ready=c1g1f,c1g1s,
DESKTOP apps client ended client=1 value=7
DESKTOP park set clients=none
DESKTOP apps client connected client=1 value=2
DESKTOP park set c1g2 f=22 s=19
DESKTOP park woke ms=530 ready=c1g2f,
```

These prove the logged table generations and equal numeric handles
`(g1, f22, s19)` → `(g2, f22, s19)`. They do not prove the WM had installed
a park token when the probe died. `witness_set` logs the table before the
syscall, not a kernel registration; elapsed time includes execution,
preemption and runnable delay, as well as blocked time. A ready first scan
returns immediately at `kernel/src/arch/x86_64/syscall.rs:7145`, without
registering at `:7152` or parking at `:7193`.

**Concrete accepted schedule:** run `desktop_apps_reuse_boot` with the new
features. After the probe's idle line, let the WM record `parked_at`
(`desktop_session.rs:2190`), then preempt it before `:2192`. Let the probe's
two-second deadline expire and let it exit on another CPU. Resume the WM
after 2010 ms. Its first scan sees the closed channels and returns their
ready bits immediately; the witness prints exactly the qualifying
`ms=2010 ready=c1g1f,c1g1s,`. Complete ordinary cleanup and relaunch with
the same numeric handles. Every new ordering check can pass, although
death preceded this wait's registration and no park occurred in it.
There is no acknowledgement from the kernel park to the probe's exit.

Also, the script still waits for `ended` before relaunching
(`tools/xtask/src/main.rs:767`), and the oracle expressly expects an empty
set before the replacement set (`:9474`, `:9488`). It therefore does not
repair the original Seat-I requirement of reuse before the WM's next wait
(`Plans/DISPATCH-2026-11-12-seat-i-a3p-parked-loop.md:37`). Section 406's
three-step ordering alone already fails for the schedule above.

## 2. No dead-generation readiness after removal: PASS, scoped to the trace

`tools/xtask/src/main.rs:9482` rejects every subsequent set or wake naming
`c1g1`, starting with the empty set. The necessary death notification
itself is allowed before removal. Production removes the old table entry
at `userland/neo-init/src/desktop_session.rs:4257`; `wait_items` iterates
current clients at `:3331`. A later set therefore excludes that generation.

This is a check of logged WM generations. The witness derives its labels
from the current table and numeric handles (`desktop_session.rs:3423`),
not kernel object identities. Source enforcement of identity remains the
kernel's two-scan comparison (`syscall.rs:7181`); the trace alone cannot
distinguish an incorrectly attributed stale bit after numeric reuse.

## 3. New client's first request wakes the WM: FAIL as a discriminator

`tools/xtask/src/main.rs:9495` accepts any later `c1g2f` wake. It does not
bound that wake by the new client's idle/end, identify a request, or exclude
channel closure. A surface is ready on peer death too: the author's own
gen-1 death wake includes both `f` and `s`. The existing scenario does
require the new probe to receive `b` and eventually exit 7
(`xtask main.rs:21653`), which demonstrates eventual service, but does not
attribute the accepted wake to its first request.

**Concrete oracle input:** in `CP61_LOG` (`xtask main.rs:29407`), replace
the final `DESKTOP park woke ms=900 ready=c1g2f,` with:

```text
PROBEAPP key code=5 count=1
PROBEAPP idling before exit ms=2000
DESKTOP park woke ms=2010 ready=c1g2f,c1g2s,
DESKTOP apps client ended client=1 value=7
```

The unchanged oracle returns `Ok(())`. This is a death-only gen-2 wake;
the request may have been serviced on an unrelated wake, with its own
wake missed. The oracle even returns `Ok(())` if the final gen-2 wake is
moved after a gen-2 `ended` line. I compiled and executed the exact extracted
oracle against both inputs. The five rejection cases at `:29431` don't
cover these inputs; their missing-request case substitutes session-only
readiness, whereas closure can set the surface bit too.

## 4. PR CP-61 row matches the demonstrated guarantee: FAIL

The [PR body](https://github.com/SecondStageTurbine/LogansOS/pull/108)
now correctly names handles `f=22 s=19`, generations 1 and 2, the empty
set, and the 2010 ms elapsed duration. But it calls that duration proof
that death woke a parked WM and calls `ready=c1g2f` proof of the first
request. Items 1 and 3 give concrete counterexamples to those inferences.
The enforcing checks are `xtask main.rs:9465` and `:9495`, respectively.

Required follow-up: tie the death trigger to an acknowledged kernel park
over the recorded old identities; record death and replacement identities
at that seam. For the replacement, identify and acknowledge the specific
first request and require its ready return before idle/death, while other
wake sources are quiet. Add negative controls for death-before-registration
and gen-2-closure-only readiness. Update the CP-61 row to those witnesses.

## 5. Retained requirements: PASS

The range-diff `3ed6c040..259feac2` versus `ddaa892cf..a6b3bcf3` shows all
five previous commits `=` and just the new witness commit. These findings
from the prior RETURN remain true:

| Item | Enforcing coordinates at this head |
|---|---|
| Bound | `kernel/src/sched/wait_any.rs:53`, `userland/libneo/src/wait_abi.rs:8`: 16; `kernel/src/arch/x86_64/syscall.rs:7078`: reject 17 before copy-in; fixed arrays at `:7085`, `:7095`. |
| CP-14 | `syscall.rs:7152` register, `:7163` recheck; `kernel/src/sched/wait_any.rs:208` checks fired before token installation at `:223`. No changes to these paths in the new commit. |
| Fairness | `desktop_session.rs:3214` allows one request; `:3990` rotates start; `:3991` visits each client; `:4012` applies the bound. |
| Idle lock | `desktop_session.rs:458` minimizes the deadline with `desktop_session/lock.rs:250`; requested/locked/idle cases are `:255`, `:258`, `:265`. |
| SAK latch | `lock.rs:177` records the request, `:255` forces deadline zero; `desktop_session.rs:462` does the same for pending client switching. |
| Kill-switch | `desktop_session.rs:2144` gates the parked path, `:2202` retains polling; `tools/xtask/src/main.rs:9350` honors `desktop-poll`. Witness code is separately gated at `desktop_session.rs:2187`, `:2193`; returning rc at `:487` adds no wait or action. |

The old executable comparison is documented in the prior RETURN; I did
not repeat a byte comparison against the new base and do not claim new
whole-binary equality.

## Validation and limits

- Read the exact-head five-file new commit, relevant implementation seams,
  prior RETURN, dispatch, current PR body and PM comments.
- `cargo test -p xtask cp61_oracle -- --nocapture`: **2 passed**.
- Exact extracted oracle compiled with `rustc --edition 2024`: baseline,
  gen-2-death-only and wake-after-gen-2-end inputs all returned `Ok(())`.
  The death-before-syscall schedule produces the same accepted baseline
  transcript; this is a source-level scheduling counterexample, not a
  locally executed scheduler fault injection.
- `cargo test -p atrium-logic-tests`: **162 passed**.
- `cargo test -p xtask feature_resolver_follows_neo_init_features`:
  **1 passed**.
- No local QEMU executable is available, so no independent boot run is
  claimed. GitHub API reported [exact-head run 43267339832](https://github.com/SecondStageTurbine/LogansOS/actions/runs/43267339832)
  `in_progress`, with no conclusion, during this review. A green run of the
  current oracle would not distinguish the accepted counterexamples.
