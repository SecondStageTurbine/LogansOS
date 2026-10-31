<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: cross-family AAG on C1.1 `threads_futex` (`922052a0`) — Oracle

**Date:** 2026-08-02 · **From:** Oracle (Claude-family) · **To:** PM (Claude)
**Dispatch:** `Plans/DISPATCH-2026-08-02-aag-c1.1-cross-family-oracle.md` (`a014c20b`)
**Branch:** `oracle/aag-c1.1-return` — doc-only. No build, no gate, no runner consumed.
**Candidate:** `codex/ot61-c1.1-aag-fix-forward-cerebro` @ `922052a0` (31 files, +6034/−187)
**RAG:** healthy (`{"status":"healthy","ollama":true,"qdrant":true}`) — used for orientation; all
findings below are from direct file reads at the candidate SHA.

**Verdict: 6 findings, none merge-blocking. One (F3) I recommend closing before merge on cost
grounds — it is a one-line change to a lifetime invariant this very branch introduced — not because
I proved a repro.** Detail and the honest limits are below.

---

## 1. The coverage limit, stated first

Authorship on this branch is mixed, so my clearance is not uniform. Commit authorship at the
candidate SHA (`git log --format='%h %an'`):

| Portion | Commits | Author | My lens |
|---|---|---|---|
| The delta | `4c29f63a`, `2b789e66`, `922052a0` | **Codex** (Janus) | **Genuine cross-family** |
| The base | the other 37 commits | **Claude** (Cerebro) | **Same-family — weaker** |

**Genuinely cross-family (Janus's delta):** the capability split-brain fix (F1), the 7b negative-control
rewrite (F4), the three-join assertion (F5), the exit-site tagging (F6). These are Codex-authored and I
am a Claude-family lens on them — this is the corroboration `CLAUDE.md` §Cross-model diversity asks for,
and no one else has checked them.

**Same-family (Cerebro's base):** the CP-37 `Pmap::inner` fix, the CP-36 lifetime anchor, the libneo
symbol gating, the futex wake-before-wait barrier. **My clearances here are weak evidence** — Cerebro
and I are both Claude, and the load-bearing precedent (CP-35/CP-36) is precisely that a full 5-lens
Claude panel cleared the exact seam a Codex lens then broke. F2 and F3 land in this portion; that they
survived my reading is *not* the same standard as Janus's cross-check of the earlier base. If the PM
wants a genuine cross-family clearance of the base's **new** material (the anchor, the IrqSafeMutex
conversion), that still requires a Codex or Qwen lens — Janus's cross-check predates both.

---

## 2. Merge evidence I can contribute (verified, not asserted)

**Tree identity `2b789e66` → `922052a0` holds — the gate transfers.** The gate
[`30737219520`](https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30737219520) has
`headSha = 2b789e66`, one commit behind the candidate. Per the §Docs-only staleness rule I asserted
subtree identity rather than asking for a re-gate:

```
IDENTICAL kernel      f582c75bd1318f4788de09a3a4805de0adbc33c5
IDENTICAL userland    97cc0ba621cf82484ac77b0dc8dc434eb9e88173
IDENTICAL tools       4c303d5caafa4ad6ba1f7ba7bd7cc0cb9ec20a41
IDENTICAL Cargo.toml  dba930caf1861639aaf2a04c57d62944486f11cf
IDENTICAL Cargo.lock  b875b6af3a2d9b4ae9bbd59d9b4c4493746d0bd5
```

`git diff --stat 2b789e66 922052a0` is one file, `Plans/RETURN-…-janus.md`, +12/−4. The gate on
`2b789e66` is valid evidence for `922052a0`.

**Per-job conclusions, independently re-verified** (the dispatch correctly refuses to accept a
run-level "success"): all four QEMU shards `success`, plus Host/Lint, SPDX, Monolithic Identity Guard,
and the four non-blocking feature-gated matrices. `event = workflow_dispatch`, `run_attempt = 1` — the
workflow was not re-run.

**`threads_futex` passed on scenario attempt 1, in 4.2 s against a 180 s budget** (shard 2/4):

```
[xtask] neo-test: launching QEMU for `threads_futex` (serial: .../threads_futex.attempt1.serial.log)
[xtask] neo-test: `threads_futex` qemu elapsed=4.2s status=exit status: 33 budget=180s
[xtask] neo-test: `threads_futex` PASS
```

This matters for lens 5 and is not visible from the run-level conclusion — see §5.

---

## 3. Findings

### F1 — The capability split-brain fix is *sufficient*, not *complete*; CP-34's recorded guard does not cover the failure that occurred
**Lens 1 · cross-family · CONFIRMED · non-blocking · structural**

`2b789e66` adds `neo-test-dw1-desktop-wm` and `neo-test-dw2-scene-policy` to three `cfg` predicates.
The assembly in `kernel/src/main.rs:320-397` has **four** hand-maintained sync points:

1. `#[cfg(not(any(…)))]` — the empty-slice arm (6 features)
2. `#[cfg(all(not(gpu-glass-slice6), any(…)))]` — the `inherited_vec` builder (5 features)
3. `#[cfg(any(arthur-compositor, dw1, dw2))]` — the inner GpuSubmit push
4. `#[cfg(any(…))]` — `let inherited = &inherited_vec[..]` (6 features)

I enumerated the miss-patterns. Two fail **loud** (a miss in list 2 or 4 leaves `inherited_vec`
undefined or unbound ⇒ compile error). Two fail **silent**:

- **Miss list 3 only** — the original OT-75 signature, reachable again verbatim.
- **Miss all four** — the natural "added a scenario, forgot the cap plumbing" case: compiles clean,
  boots, receives `&[][..]`, exits 40.

**No live second instance.** I enumerated the invariant precisely — *a scenario needs the inherited cap
iff it calls `set_gpu_submit_enforce(true)` and presents from neo-init* — and checked every arming site
(`git grep set_gpu_submit_enforce`): `test_harness.rs:2131` (arthur-compositor), `:2195` (dw1),
`:2217` (dw2). All three are now in the predicates. `neo-test-arthur-compositor-baremetal` is covered
transitively (it enables `neo-test-arthur-compositor`). `neo-test-gpu-glass-slice6` uses the exclusive
arm and an in-kernel present path with an explicit `authorized` bool (`test_harness.rs:5351`), not the
process cap table. **The two patched instances are the only ones — but the structure that produced them
is intact.**

**CP-34 is now known-incomplete.** Its "Our guard" reads *"A single accumulating builder with a
documented order … replacing the exclusive `cfg` arrays."* That builder was **already present and
correct** when DW-1/DW-2 broke — the failure was in the *gating* of the builder, which CP-34 does not
name. Its discriminator (`threads_cap_compose`) is a composed `browser-runtime + net-dial-cap` build; a
composed build cannot detect a scenario *missing* from the predicate lists. Per the bank-grows-never-
prunes rule, CP-34 should be amended to name the four-way predicate sync as the residual hazard.

**The remedy is already in this repo, two hundred lines away.** `kernel/Cargo.toml:353-367` introduces
`neo-test-atrium` / `neo-test-atrium-runtime` as *"two pure-marker umbrella features"* that
*"stand in for the hand-maintained `any(feature = …)` lists"*, so *"a new atrium scenario opts into the
umbrella(s) HERE, in one place."* That is exactly this problem, already solved, already proven. Applying
it: add a marker `root-cap-gpu-submit = []`; each compositor scenario enables it in its own feature list;
the four predicates reference the marker instead of enumerating scenarios. One opt-in site, all four
predicates fire by construction. Optional belt-and-braces: the xtask registry test at
`tools/xtask/src/main.rs:10431` is the natural home for a scenario↔predicate coverage assertion.

### F2 — `libneo::nthread::spawn_no_bootstrap` is a `/// Test-only.` symbol shipped ungated
**Lens 4 · same-family (Cerebro, `4f6de464`) · CONFIRMED · low severity · one-line fix**

```rust
/// … the falsifiable proof that the bootstrap is what installs it.
/// Test-only.
pub fn spawn_no_bootstrap(                     // userland/libneo/src/nthread.rs:191
```

No `#[cfg]`. It is the blocker-E negative control — it deliberately creates a thread with **no control
block**, so `control_block()` and `errno_cell()` return `None` for it. `git grep -i 'test-only'` over
`userland/libneo/src` returns exactly three hits: the two `SYS_FUTEX_QUEUE_LEN` symbols (both gated) and
this one (ungated). The commit whose stated purpose was *"gate libneo test symbols"* (`16b5ecf3`) gated
two of the three and missed the third in the same crate. Not a kernel privilege hole — `sys_thread_create`
is reachable directly regardless — but it is precisely the public-surface hygiene this lens exists for.

**The symbol that bit this lane before is genuinely fixed on both halves** — I checked both, which was
the specific ask:

| Half | Const | Dispatch arm | Impl |
|---|---|---|---|
| libneo | `#[cfg(feature = "neo-test")]` `lib.rs:135` | — | `#[cfg(feature = "neo-test")]` `lib.rs:1916` |
| kernel | `#[cfg(all(browser-runtime, neo-test))]` `syscall.rs:157` | same, `:740` | same, `:5087` |

**Minor asymmetry, non-blocking:** libneo gates on `neo-test` alone; the kernel on
`all(browser-runtime, neo-test)`. A libneo build with `neo-test` against a kernel without
`browser-runtime` would compile a call to an unimplemented syscall (runtime `ENOSYS`, not a compile
error). N/A today — `neo-test-threads-futex` enables `browser-runtime` — but the gates are not
equivalent and only one of them is load-bearing.

### F3 — CP-36's lifetime anchor is installed on only one of the two user-thread constructors
**Lens 2 · same-family (Cerebro) · invariant gap CONFIRMED · reachability UNPROVEN · recommend closing before merge**

`Thread::process: Option<Arc<Process>>` is CP-36's fix for the force-kill-vs-join UAF. Its own doc
(`kernel/src/sched/thread.rs:112-115`) states the guarantee as *"guaranteeing the `Process` outlives
**every thread that can still dereference it**."*

`git grep 'set_process_anchor'` returns **one** definition and **one** caller:
`process.rs:2429`, inside `thread_create_for_current`. The four `Thread` constructors initialise
`process: None`, and `Thread::new_user` — the constructor used for **every user process's initial
thread** — is never anchored afterwards.

The initial thread *can* dereference the `Process`. `thread_join_for_current`
(`process.rs:2493-2575`) deliberately drops its `Arc<Process>` and parks holding two raw pointers:

```rust
let reg_mutex: *const IrqSafeMutex<ThreadRegistry> = { … &proc_arc.threads as *const _ };
// proc_arc dropped here
…
let mut reg = reg_lock.lock();   // Phase 3, AFTER the park — deref into the Process allocation
```

and neo-init's **main thread** is the joiner — check 7 joins three handles. That is the exact CP-36
shape (park owning nothing per CP-30 → valid wake → Ready→Running window invisible to both kill scans →
teardown drops the last `Arc`), on the one thread class the anchor does not cover.

The function's own SAFETY comment argues from `PROCESSES` registration — *"The Process stays alive via
its PROCESSES registration for as long as THIS thread … is blocked here"* — which is the **pre-CP-36
argument that CP-36 was written to refute**. For anchored threads the code is safe despite the stale
comment; for the initial thread neither the comment nor the anchor applies.

**What I verified:** the anchor has exactly one installation site; `Thread::new_user` leaves it `None`;
the main thread parks in `thread_join_for_current` holding a raw pointer derived from `&proc_arc.threads`
after dropping the owned `Arc`. **What I did not verify:** whether whole-process teardown accounting can
actually drop the last `Arc` while the initial thread sits in the Ready→Running window — that is the
reachability leg and it needs a teardown trace I did not do. Per the dispatch's rule I therefore report
this as a finding naming the missing fact, **not** as a proven blocker. I recommend closing it before
merge anyway: the fix is a `set_process_anchor` call on the `spawn_user` path, and the asymmetry itself
is a maintenance hazard even if today's interleaving happens to be unreachable.

**Two related notes, both clean:**
- The stale SAFETY comment in `thread_join_for_current` should be rewritten to cite the anchor. The next
  reader reasons from the comment, and the comment states the argument CP-36 disproved.
- `done_word: Box<AtomicU32>` is correctly `Box`ed: the joiner captures the *heap* address, so
  `Vec` reallocation of `reg.records` cannot invalidate it. `thread_detach_for_current` refuses a
  `join_claimed` record. Both checked, both sound.

### F4 — The 7b negative control no longer has teeth
**Lens 3 · cross-family · CONFIRMED · non-blocking**

The dispatch asked for a mutation argument. Here it is: **there is no mutation of the barrier property
that makes check 7b fail.**

`race_control_check` returns only `36`/`37` (setup failures), `40` (join failure), `0`, or `38`. `0` and
`38` are its *subject* outcomes, and the new handler accepts **both**:

```rust
match race_rc {
    38 => say(b"… weak barrier race observed (zero wake picks) ok\n"),
    0  => say(b"… waiters already enqueued (inconclusive) ok\n"),
    _  => return race_rc,
}
```

Previously `0` was `return 39`. The control is now a **reporter**, not a control: it cannot fail on the
property it exists to assert.

**The loosening is a correct flake fix, and I want that on the record.** Janus's data is real — 4/4
attempts hit the old `return 39` — and the old assertion was schedule-dependent *by construction*: it
asserted that a race window would be observed, which no schedule guarantees. Keeping it would have been
keeping a known false-positive generator in the highest-flake scenario in the tree.

**But a third option exists, and it is why I am recording this.** The window is only unobservable
because the waiter may reach `sys_futex_wait` before the snapshot. Make it deterministic instead of
optional: have `race_parked_waiter` increment `WAITERS_READY` and then spin/yield a bounded number of
iterations *before* calling `sys_futex_wait`. That forces `qlen == 0` at snapshot time by construction,
restoring a real assertion without reintroducing the flake. As merged, the scenario proves the
queue-length barrier is **sufficient** (check 7 passes) but no longer proves it is **necessary** — the
kill-switch-shaped falsifier is gone. That is a coverage regression relative to the branch's own intent,
not a defect introduced to `main`, hence non-blocking.

### F5 — The "exact three-join" assertion is unreachable
**Lens 3 · cross-family · CONFIRMED · low severity**

The dispatch describes the replaced `filter_map` as *"could iterate zero times while printing ok."*
Checking that precisely: it could not, and neither can the replacement fire.

```rust
fn spawn_or(code: &mut i32, …) -> Option<JoinHandle> {
    match nthread::spawn(…) { Ok(h) => Some(h), Err(_) => { *code = 1; None } }
}
```

`spawn_or` returns `None` **iff** it sets `*code = 1`, and check 7 does `if code != 0 { return 30; }`
immediately after the spawn loop (`main.rs:2425-2431`). So at the join loop all three handles are
`Some`. Therefore:

- `let Some(h) = h else { … return 35 }` — **unreachable**.
- `joined != 3` — **unreachable**: the loop runs exactly 3 times and each iteration either increments
  `joined` or returns.
- The original `filter_map` could iterate zero times only if all three were `None`, which requires
  `code != 0`, which `return 30` already caught.

So the hazard was **latent, not live**, and the fix is correct-but-vacuous defense-in-depth. Answering
the dispatch's question exactly: these two assertions do **not** have teeth — because an upstream guard
already has them. Worth keeping (they are free and they document the intent), worth not counting as new
coverage.

### F6 — The tagging commit introduced a new untagged, ambiguous exit code
**Lens 3 · cross-family · CONFIRMED · low severity**

`4c29f63a`'s stated purpose was *"Tagged every pre-existing `return 35` site with an unambiguous serial
site name."* In the same commit, `race_control_check` gained:

```rust
for h in handles.into_iter().flatten() {
    if h.join() != Ok(0) { return 40; }        // no say() tag
}
```

which propagates through 7b's `_ => return race_rc`. Check 8, ~18 lines later, independently returns
`40` for `hw.join() != Ok(0)`. **Exit 40 is now ambiguous** between "negative-control waiter join
returned non-`Ok(0)`" and "rwlock writer join failed" — and unlike every `35` site the same commit
tagged, the new `40` carries no `say()` marker. Same defect class the commit set out to close. One
`say(b"… EXIT=40 site=race-control-join\n")` fixes it.

---

## 4. Lens 2 — pitfall-bank consultation (`docs/concurrency-pitfalls.md`, CP-1…CP-37)

Per §Concurrency / SMP-touching dispatches, disposition per class the change touches:

| Class | Disposition | Discriminator / evidence |
|---|---|---|
| **CP-37** (non-IRQ-safe lock across preemptible + IF=0) | **Guard — verified complete on the paths I audited** | See the nesting audit below. Bounded-spin detector armed, `neo-test`-gated, 7 labelled acquire sites. |
| **CP-36** (Ready→Running invisible to both kill scans) | **Guard — with the gap in F3** | `Thread::process` anchor; **not installed on `Thread::new_user`**. |
| **CP-35** (relocating wake must transfer the reservation) | Guard, unchanged by the delta | `wake_thread_front` capacity check + `smp_c11_migration_reservation` armed. |
| **CP-34** (feature-exclusive root-cap assembly) | **Guard incomplete — F1** | Builder is compositional; its *gating* is not. Recorded guard does not cover OT-75. |
| **CP-30** (blocking syscall must own nothing across a park) | Guard | `thread_join_for_current` holds only raw pointers; `thread_exit_current` drops `proc_arc` before `scheduler::exit()`. Verified by reading both. |
| **CP-33** (kill drains only some wait queues) | Guard, unchanged | Complete sibling drain (`5829ee3c`). |
| **CP-16** (wake with a foreign lock held) | **Guard — verified** | `thread_exit_current` closes the registry-lock scope, *then* calls `futex::wake`. Read directly; the ordering is explicit. |
| **CP-14** (lost wakeup) | Guard | `wait_on_key` re-checks `done_word == 0` under the futex queue guard; check 6's 2000 ping-pong rounds armed. |
| **CP-11** (wrong atomic ordering) | Guard | `done_word.store(1, Release)` paired with `load(Acquire)` in both the loop test and the under-guard predicate. Correct pairing. |
| **CP-12** (ABA / slot reuse) | Guard | Generation widened to `u64`; `done_word` is a heap-stable `Box`, so `Vec` realloc cannot invalidate the joiner's pointer. |
| **CP-2 / CP-3** (IF=0 section waiting cross-CPU) | **Guard — verified** | TLB shootdown moved out of every `Pmap::inner` section; `pte_write_fence_and_shootdown` is called only after the guard drops. |
| CP-1, CP-4…CP-10, CP-15, CP-17…CP-29, CP-31, CP-32 | **N-A for this delta** | Janus's delta is userland scenario observability + one `cfg` list; it adds no kernel lock, IPI, cross-CPU path, allocator path, or DMA path. The base's kernel changes are covered by the rows above. |

**CP-37 completeness — the specific "does another path reach it with interrupts enabled?" question.**
`Pmap::inner` is now an `IrqSafeMutex`, so *every* acquire masks interrupts and the asymmetric mix is
gone by construction rather than by audit. I then asked the inverse — **does the now-always-IF=0
section nest a lock that others take at IF=1?**, which would push CP-37 one level down. Three nested
locks, all checked, **all clean**:

| Nested lock | Type | Verdict |
|---|---|---|
| `borrowed_pins` (`pmap.rs:152`) | plain `spin::Mutex` | **Single acquire site** (`:907`), only ever inside the IF=0 section ⇒ no mix. |
| kernel heap (`Vec::push`) | `Mutex<Heap>` | Every `alloc`/`dealloc`/`init` wrapped in `without_interrupts` ⇒ always IF=0. |
| frame allocator (page-table alloc) | `Mutex<Option<BuddyAllocator>>` | `with_frame_alloc` wraps in `without_interrupts` ⇒ always IF=0. |

**This lead was my strongest hypothesis going in and it did not survive.** Recording it as refuted
rather than silently dropping it: the CP-37 fix is complete with respect to the bank's own grep-able
rule on the paths I read.

**Second join/reap UAF hunt — the dispatch's explicit "assume one exists" instruction.** I hunted and
found **F3**, which is not a second UAF but an unclosed edge of the *first* one. I also cleared three
candidate paths: `done_word` vs `Vec` reallocation (safe — heap-stable `Box`), `thread_detach_for_current`
removing a claimed record (refused: `if rec.detached || rec.join_claimed { return EINVAL }`), and a
force-zombified joiner dereferencing after the park (safe — a zombified thread never resumes, so there
is no deref). The window that remains open is F3's.

---

## 5. Lens 5 — completeness, and how to read this gate

**Which reading do I believe: the historical rate reflects the now-fixed defect, and it should
collapse.** Three reasons, in descending strength:

1. **The mechanism is identified and closed by construction, not probabilistically.** CP-37 documents
   the same rate I measured independently — *"`threads_futex` reddened the 4-shard gate ~2-in-3, and
   reproduced 4/4 locally at `NEO_SMP=4`"* — and attributes it to a specific same-CPU deadlock
   (`Pmap::inner` held preemptibly at IF=1 by the reaper, spun for at IF=0 by `thread_create`). Making
   the lock an `IrqSafeMutex` removes the preemptible-holder state *entirely*; there is no residual
   probability, only the question of whether another lock has the same asymmetry — which §4 checks and
   clears.
2. **The observed mode flipped.** The CP-37 failure signature is a **silent budget timeout** (the stuck
   CPU takes no timer tick, so every tick-driven diagnostic dies with it). This gate's attempt-1 result
   is **4.2 s against a 180 s budget** — the fast mode, not a near-budget squeak. This is the same
   bimodality discriminator I used in OT-74 to separate `arthur_reversibility` (a real race: 7–17 s
   completions vs failures pinned at 420 s, zero intermediate values) from
   `sys_generate_kv_persist_equivalence` (capacity: a genuine 84–117 s distribution).
3. **My own 2/3 sample overlaps the broken window and is n=3.** As I flagged in the OT-74 RETURN, the
   per-scenario error bars are wide. The 66.7 % is a *caution flag about the measurement period*, not a
   stable property of the scenario.

**But the green gate is the weakest leg of that argument, and it should not be cited as the strong
one.** Under the null hypothesis "the rate is still 2/3," a single attempt-1 green occurs with
probability **1/3** — so this run rules the null out at roughly 67 % confidence, which is close to no
evidence at all. Anyone reporting "the gate was green, therefore the flake is fixed" is leaning on the
one leg that barely holds. **The mechanism argument carries this, not the run.**

**What would make it measurable, at zero runner cost:** the null dies at ~5 consecutive attempt-1
greens ((1/3)^5 ≈ 0.4 %). The harness already emits the discriminator in its own log line —
`threads_futex.attemptN.serial.log` plus the `PASS`/elapsed line — so the OT-74 scrape can harvest this
from the next few *natural* gate runs. **No dedicated runs, no contention on the binding constraint.**
I'd recommend that over any deliberate re-fire.

**The generalisable point for the PM's merge report:** a run-level `success` hides scenario retries, and
for the one scenario in 284 with a measured 2/3 attempt-1 failure rate that is exactly the information
you need. The per-shard conclusions the dispatch already asks for are necessary but not sufficient —
the **per-scenario attempt number** is the discriminator here, and it is one `grep` away.

---

## 6. What each lens checked (no rubber-stamp)

- **Lens 1 (split-brain).** Read all four `cfg` predicates at the candidate; enumerated every
  miss-pattern and classified each loud/silent; derived the actual invariant
  (`set_gpu_submit_enforce(true)` + neo-init present) and checked all three arming sites plus the
  transitive and exclusive-arm cases; traced `compose_and_present` → `sys_fb_present` →
  `scanout_authorized` → `caller_holds_gpu_submit` to confirm the exit-40 mechanism; read CP-34 and
  compared its recorded guard against the failure that actually occurred. → **F1.**
- **Lens 2 (concurrency core).** Read CP-34/35/36/37 in full; audited `Pmap::inner`'s lock-site
  inventory and all three nested locks for a CP-37 recurrence (refuted); read
  `thread_join_for_current`, `thread_exit_current`, `thread_detach_for_current`, `Thread::free`,
  `set_process_anchor` and all four `Thread` constructors; verified CP-16 ordering and the
  Release/Acquire pairing on `done_word` by reading both sides. → **F3**, plus the refuted CP-37 lead.
- **Lens 3 (do the assertions have teeth).** Mutation-argued each of the three new assertions
  separately; traced `spawn_or`'s `None`/`code` coupling to prove unreachability; enumerated every
  `return` code in the battery to find the collision. → **F4, F5, F6.**
- **Lens 4 (libneo surface).** Enumerated every `pub` item in `nsync.rs` and `nthread.rs`; swept
  `-i 'test-only'` across `userland/libneo/src`; checked `SYS_FUTEX_QUEUE_LEN` on **both** halves
  (const + dispatch arm + impl on the kernel side) rather than one. → **F2**, plus the gate asymmetry
  note.
- **Lens 5 (completeness / flake).** Re-verified per-job conclusions and `run_attempt` via the API;
  scraped the shard-2 log for the per-scenario attempt marker and elapsed time; applied the OT-74
  bimodality discriminator; computed the actual evidential weight of one green run under the measured
  historical rate. → **§5.**

---

## 7. Recommendation

**Merge is justified on the merge bar** (§Merge bar vs acceptance bar): rebase-fresh, 4-shard gate
green on a SHA whose code tree is provably identical to the candidate, per-job conclusions
independently re-verified, and no finding that would make `main` worse than `main`-without-this-branch.
This branch closes a real deadlock (CP-37) and a real capability defect (OT-75); holding it only makes
it staler.

**Before merge, if the PM wants them (my ranking):**
1. **F3** — one line (`set_process_anchor` on the `spawn_user` path) + rewrite the stale SAFETY comment.
   Cheapest close of the highest-consequence finding, and it is the invariant this branch introduced.
2. **F2** — one `#[cfg(feature = "neo-test")]` attribute.
3. **F6** — one `say()` line.

**After merge (ledger/bank work, not code):**
4. **F1** — the `root-cap-gpu-submit` marker feature, plus a CP-34 amendment naming the four-way
   predicate sync as the residual hazard. This is the finding with the longest tail: it is the one that
   will recur.
5. **F4** — the deterministic 7b control (delay the race waiter before `sys_futex_wait`).
6. **F5** — no action; recorded so the assertions are not counted as coverage they don't provide.

**Standing caveat, repeated because it is the point of this exercise:** F2 and F3 are same-family
findings against Cerebro's base, and my *clearances* on that base are correspondingly weak. The base's
new SMP material — the CP-36 anchor and the `IrqSafeMutex` conversion — has **not** had a cross-family
adversarial lens; Janus's cross-check predates both. If the PM wants the §Cross-family lens REQUIRED
rule satisfied for the concurrency core as merged, that is still outstanding, and this RETURN does not
supply it.
