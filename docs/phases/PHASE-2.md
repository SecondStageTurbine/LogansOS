# Phase 2 - Capabilities, SMP, Allocators, TLB

| | |
|---|---|
| **Status** | BUILD-1/2/3 substantially complete (2026-05-11). Phase 2 formal close pending — see `Plans/PM-DISPOSITION-2026-05-24-phase-2-observe-close.md` for remaining work and scope-class items currently with PL. |
| **Started** | 2026-05-04 |
| **Closed** | - |
| **ADRs landed** | 010 (capability system foundation); 011 amended (then superseded by 013); 012 (buddy + slab allocator foundation); 013 (Phase 2 SMP retrofit, supersedes 011) |
| **Commits** | BUILD-1: see below; BUILD-2: see below; BUILD-3 sub-commits 0, 1a, 3, 4: see below |

---

## What Phase 2 Is Building

Phase 2 is the kernel hardening phase after the Phase 1 foundation. Bible
Article 13 scopes it to SMP, capability system, buddy + slab allocators, and
advanced TLB management.

BUILD-1 lands the capability foundation first because later allocator and SMP
work need a stable handle/capability model to audit against. The goal is not to
finish every future capability operation; the goal is to make the existing
Channel and Process handles real capabilities with rights, derivation tracking,
grant, and revoke.

---

## Original Criteria

The BUILD-1 handoff criteria:

- **P2-1.1** - Extend `Handle` variants with kernel-private `CapId` and
  `CapRights`.
- **P2-1.2** - Preserve userspace handle indices as dense, recyclable,
  per-process references.
- **P2-1.3** - Add per-process derivation logs keyed by global monotonic
  `CapId`.
- **P2-1.4** - Add `sys_derive`, `sys_revoke`, `sys_grant`, and
  `sys_recv_grant`.
- **P2-1.5** - Gate existing `sys_send`, `sys_recv`, and `sys_kill` through
  rights checks without changing existing userspace call sites.
- **P2-1.6** - Implement grant as derived-copy, not move; donor keeps source,
  recipient receives fresh child `CapId`.
- **P2-1.7** - Implement revoke as slot-freeing invalidation; subsequent use
  of the same handle index returns `-EBADF`.
- **P2-1.8** - Land ADR-010's 10 cap scenarios in the default QEMU suite.
- **P2-1.9** - Existing Phase 1.5b/1.5c scenarios pass unmodified.
- **Anti-criterion P2-1.A1** - Do not start SMP.
- **Anti-criterion P2-1.A2** - Do not replace the frame allocator or add slab.
- **Anti-criterion P2-1.A3** - Do not add tensor/model/inference caps early.

---

## BUILD-1 - Capability Foundation

Landed shape:

- `CapId(0)` is reserved as null/no-cap. Real caps allocate from a global
  monotonic `AtomicU64` starting at 1. This is acceptable in single-CPU
  BUILD-1; Phase 2 SMP may replace allocation with per-CPU banks if contention
  ever matters.
- Handle indices remain 0-based, per-process, dense, recyclable, and
  userspace-visible. `CapId` is kernel-private and never returned to userspace.
- `Handle::Channel` and `Handle::Process` now carry `(cap_id, rights)`.
- `CapRights` splits uniform meta rights (`USE|DERIVE|GRANT|REVOKE`) from
  per-variant semantic rights (`SEND|RECV|CLOSE` for Channel,
  `KILL|WAIT|INSPECT` for Process).
- `sys_chan_create` and `sys_spawn` mint full-right root caps so all existing
  Phase 1 userspace behavior remains valid.
- `sys_spawn` inherited handles are derived-copy grants at process creation:
  each child handle gets a fresh `CapId`, parent linkage, and requires the
  parent source cap to carry `GRANT`.
- `sys_send`, `sys_recv`, and `sys_kill` enforce `USE` plus the relevant
  semantic right. Wrong variant or revoked slot returns `-EBADF`; missing
  rights returns `-EPERM`.
- `sys_derive` installs reduced-right child caps in the same process after a
  uniform bit-subset check.
- `sys_revoke` walks per-process derivation logs across the process registry
  and frees every revoked handle slot.
- `sys_grant` / `sys_recv_grant` reuse the existing IPC rendezvous machinery
  and ADR-011 single-CPU handoff invariants. Recipient rights are computed as
  `donor_actual & offered & recipient_max`; the recipient is the consumer for
  handoff-ordering purposes.
- libneo exposes the new syscall wrappers and rights constants to userspace.
- `cargo neo-test` includes 10 default `cap_*` scenarios covering mint, derive,
  derive denial, grant, grant denial, local revoke, cross-process revoke,
  descendant revoke, and survive-exit behavior.

---

## Course Corrections

- **Spawn inheritance needed real child CapIds.** Treating inherited handles as
  plain clones would make parent and child table slots share a `CapId`, so
  revoke could not distinguish the source from descendants. BUILD-1 made
  spawn-time inheritance a derived-copy operation requiring `GRANT`.
- **Grant denial must not install rendezvous state.** `sys_grant` without
  `GRANT` fails preflight with `-EPERM`; the kernel should not park a rejected
  grant for a future receiver. The `cap_grant_missing_meta` scenario now parks
  the receiver first with an explicit control-channel ready handshake, so the
  recipient observes `-EPIPE` deterministically.
- **Worker-side semantic failures were initially invisible.** The root test
  harness only gates on `neo-init` exit code, so a worker exiting nonzero could
  still produce a false green if `neo-init` exited 0. Cap-test workers now hang
  on semantic failure, forcing the scenario to timeout instead of silently
  passing.
- **Scheduler smoke setup was not atomic under a live PIT.** Timer preemption
  could run `ping`/`pong` while `smoke_test` was still spawning the test cohort,
  causing intermittent pre-scenario stalls. `smoke_test` now resets its flags
  and spawns ping/pong/hog under `without_interrupts`; preemption is still
  proven after all three threads exist.
- **Local revoke test originally omitted `REVOKE`.** The test derived a
  SEND-only child, so `sys_revoke(child)` correctly failed with `-EPERM` and
  the follow-on send blocked. The scenario now derives a child with `REVOKE`
  and verifies slot-freeing `-EBADF` semantics.
- **Process/capability critical sections needed interrupt masking.** Full-suite
  cap tests exposed PIT preemption while the current CPU held process registry
  or handle-table locks. The timer path can re-enter scheduler/process lookup,
  so BUILD-1 masks interrupts around process registry, handle install/derive,
  grant, revoke, and cleanup critical sections. This is a single-CPU invariant;
  SMP must replace it with real lock ordering.
- **Descendant revoke needed recipient readiness, not timing.** The parent could
  revoke immediately after `sys_grant` while the recipient had received the cap
  but not yet derived the local child. `cap_revoke_descendants_walk` now has the
  worker send a ready message only after receive/derive, and the parent revokes
  only after that handshake.
- **ADR-011's "blocked peer first" rule was too broad.** Production boot showed
  that if the parent arrived first and blocked in `go-block`, the worker's
  receive completion woke the parent before the worker entered its follow-on
  blocking recv. The kernel now uses consumer-first handoff: sender/grantor
  completing a waiting receiver yields to the receiver; receiver/recipient
  completing a waiting producer keeps the current consumer running with a
  bounded defer until its next syscall or block. ADR-011 records this BUILD-1
  amendment.
- **Production boot must scan serial semantics, not just timeout.** The legacy
  60s timeout wrapper can exit successfully after QEMU is killed even if the
  kernel printed a `BUG:` line and panicked later. BUILD-1 verification scans
  for `neo-init exit code 0`, clean frame audit, and absence of `KERNEL PANIC`
  or `BUG:`.

---

## Verification Log

### BUILD-1

- `docker compose run --rm neo cargo k` - PASS.
- Targeted `rustfmt --edition 2024 --config skip_children=true` over BUILD-1
  touched Rust files - PASS.
- Repo-wide `cargo fmt -- --check` - FAILS on unrelated pre-existing formatting
  drift in files outside BUILD-1 scope; not accepted into this commit.
- Focused capability scenarios - PASS:
  `cap_mint_and_use`, `cap_derive_subset`, `cap_derive_rights_violation`,
  `cap_derive_missing_meta`, `cap_grant_basic`, `cap_grant_missing_meta`,
  `cap_revoke_local`, `cap_revoke_cross_process`,
  `cap_revoke_descendants_walk`, `cap_survive_exit`.
- Full `docker compose run --rm neo cargo neo-test` - PASS, including
  `expect_pass_but_fail` meta-test detection.
- Production serial-gated `cargo iso && cargo neo-tty` - PASS:
  `neo-init exit code 0`, `frame: leak audit clean`, no `KERNEL PANIC`, no
  `BUG:`.
- `git diff --check` - PASS.

---

## Anti-Criteria

- SMP was not started. ADR-011's single-CPU handoff invariants remain in force;
  Phase 2 SMP must replace them deliberately rather than editing around them.
- Buddy/slab allocators were not started. The bump/free-list frame allocator
  remains the Phase 1 allocator until a dedicated BUILD replaces it.
- Capability copy/move, `sys_close`, `sys_wait`, and `sys_inspect` remain
  deferred. Their rights bits are reserved ABI, not BUILD-1 behavior.
- Tensor, Model, InferenceTicket, and File capability variants were not added.
  Phase 5+ variants must extend the same `Handle` enum instead of creating
  parallel handle tables.

---

## BUILD-2 — Buddy + Slab Allocators

BUILD-2 replaces Phase 1's bump+free-list frame allocator with a classic power-of-2
buddy allocator (orders 0..14, 4 KiB to 64 MiB) and adds a per-CPU slab allocator
for fixed-size kernel object pools. ADR-012 governs the design; Bible Article 7.4
mandates both buddy and slab in Phase 2. The buddy sources physical frames; slab
sits atop buddy for Process, Thread, Channel, Handle, EndpointState, and
DerivationEntry caches (three of six are production-live after BUILD-2; see anti-criteria below).

The kernel heap (`linked_list_allocator`) coexists permanently with slab — slab
serves fixed-size pools, the heap serves variable-size allocations. Neither replaces
the other. Three allocation classes: fixed-size → slab, variable-size → kernel heap,
device-resident tensor → tensor allocator (Phase 5+, separate subsystem).

### Sub-commit 0 — Slab interface stub (commit `03e8759`)

ADR-012 §Q3. Declares the `SlabCache<T>` trait, `Slab<T>`, `Magazine<T>`,
`GlobalCache<T>`, and `SlabPerCpuState` shape with per-CPU + global cache pattern.
No implementation; no callers. Compiles; existing test suite green (anti-regression).

### Sub-commit 1 — Buddy frame allocator (commit `b617acf`)

ADR-012 §Q1 + §Q2. Replaces `BumpFrameAllocator` and its intrusive free-list overlay
with `BuddyAllocator`. Coalesce path is the sole free path; split/coalesce never touch
`allocated_pages` (Invariant 1 from ADR-012 amendment). HHDM-before-buddy-init ordering
required (Invariant 3).

New test scenarios: `\`buddy_split_coalesce\``, `\`buddy_max_order\``,
`\`buddy_alloc_free_alloc_returns_same_frame\``, plus `\`expect_pass_but_alloc_fail\``
deliberate-failure meta-test (symmetric with Phase 1.5c's `expect_pass_but_fail`).

### Sub-commit 2 — Slab implementation (commit `93bf178`)

ADR-012 §Q3. Lands slab implementation behind sub-commit 0's interface. Per-CPU magazines
with global cache fallback; lockless hot path on per-CPU, per-cache global lock on overflow.
`THREAD_CACHE`, `PROCESS_CACHE`, `CHANNEL_CACHE` declared as static slab instances.

New scenarios: `\`slab_local_to_global_promotion\`` — alloc/free pattern exceeding
`local_capacity`, verifying objects flow to global cache; `\`slab_global_to_buddy_overflow\`` —
exceeding `global_capacity`, verifying slab returns excess backing pages to buddy.

### Sub-commit 3 — Channel migration to slab (REVERTED) (commit `e754723`)

The initial Channel migration (commit `13d7a25`) was reverted because
`Arc::from(Box<T>)` routes `Drop` to the global allocator rather than the slab's free path.
The allocation traverses slab but the free bypasses it — slab accounting silently bleeds.

Test `\`slab_channel_migration\`` retained in `tools/xtask/src/main.rs` as `ignored: true`
regression pin with this canonical comment:

```rust
    // Sub-commit 3 fix-up: the Channel slab migration was reverted because
    // Arc::from(Box<T>) routes drop to the global allocator rather than the
    // slab's free path. The test scenario remains in tree as a regression
    // pin: when Arc-aware slab integration lands in a future BUILD, flip
    // this back to `ignored: false` and the test should PASS.
```

When Arc-aware slab integration lands in a future BUILD, flip to `ignored: false` and
the test should PASS. CHANNEL_CACHE remains declared and initialized; it is unused by
production code.

### Sub-commit 4 — Thread migration to slab (commit `f5a4984`)

ADR-012 §Q3. Thread allocation on `THREAD_CACHE.alloc()`, free via the cache.
`\`slab_thread_migration\`` scenario with two-invariant check: allocation traverses slab
(`buddy_pages_held` increases), and free returns storage to cache (`local_in_use` or
`global_in_use` increases). Thread is unaffected by the Arc-final-owner pattern because
Thread is not Arc-shared (only one scheduler queue holds it at any time, modulo the
rendezvous-block pattern).

Course correction: `Thread` cache alloc/free bracketed with `without_interrupts`. The slab
per-CPU magazine is single-CPU safe only while maskable interrupts are disabled; full-suite
verification exposed intermittent scheduler-smoke timeouts when Thread free ran with PIT live.

### Course Corrections

- **Handle and EndpointState are not slab-cached.** Pre-BUILD-2 checkpoint found that
  `Handle` is stored inline in `Vec<Option<Handle>>` slots and `EndpointState` is a
  subfield of `Channel`'s allocation. Slab caching them would be a category mistake.
  ADR-012 BUILD-2 amendment scope correction records this.

- **Arc-vs-slab boundary.** `Arc::from(Box<T>)` accepts slab-allocated storage but returns
  it to the global allocator on final drop. This blocks Channel and Process slab migration.
  Three known resolution paths: custom Drop returning to slab, `Arc::new_in()` with custom
  allocator (unstable `allocator_api`), or refactoring away from Arc final-owner. Deferred
  to future BUILD. ADR-012 post-close summary records this finding.

- **DerivationEntry deferred.** Per-process derivation log uses `BTreeMap<CapId, Option<CapId>>`;
  entries are opaque BTreeMap internal nodes, not directly slab-allocatable. Deferred to
  sub-commit 3 implementation analysis; no decision made in BUILD-2.

### Verification Log

- `cargo k` — PASS at commit `f5a4984`.
- All default `slab_*` tests PASS: `\`slab_local_to_global_promotion\``,
  `\`slab_global_to_buddy_overflow\``, `\`slab_thread_migration\``.
  `\`slab_channel_migration\`` excluded (ignored, see sub-commit 3 above).
- All buddy tests PASS: `\`buddy_split_coalesce\``, `\`buddy_max_order\``,
  `\`buddy_alloc_free_alloc_returns_same_frame\``, `\`expect_pass_but_alloc_fail\``.
- Phase 1.5b/1.5c anti-regression PASS: `ipc_heap_kill_smoke`, `smep_catch`, `smap_catch`,
  `smep_policy`, `smap_policy`, `kaslr_smoke`.
- Frame leak audit clean (baseline=final).

### Anti-Criteria

- Arc-aware slab integration deferred to future BUILD. `PROCESS_CACHE` and `CHANNEL_CACHE`
  declared but unused by production.
- Per-CPU slab sharding for actual multi-CPU contention deferred to BUILD-3 (when SMP lands).
  Per-CPU mechanism exercises in single-CPU per ADR-012 Principle 4; concurrent contention
  debugged separately in BUILD-3+.

---

## BUILD-3 sub-commit 0 — ADR-013 + ADR-011 Supersede + GLOBAL Fixup

Three commits: initial (`fe5bc3d`), remediation (`b4b3ac9`), lint cleanup (`57fd235`).

### Initial commit `fe5bc3d`

ADR-013 lands at `docs/ADR-013-phase-2-smp-retrofit.md` with all nine load-bearing decisions
(Q1–Q9). ADR-011 acquires `Superseded by: ADR-013` metadata. Post-boot fixup walks PML4 entries
256-511, sets the GLOBAL bit on kernel-half leaf entries (per ADR-013 §Q7.a). Limine did not set
CR4.PGE; the fixup explicitly enables it.

### Remediation commit `b4b3ac9`

First boot revealed that `paging::sanity_check` was running pre-fixup — its claim that
`global=true` was always false against reality, so BUG warnings fired on every boot during
normal operation. Remediation: drop the pre-fixup sanity check; capture actual serial output
post-fixup. Specifically observed:

```
[ kernel ] paging: sanity OK - kernel .text sample virt VirtAddr(0x...) -> phys PhysAddr(0x...) (global=true)
```

at a KASLR-relocated address. Post-fixup `paging::sanity_check` reports `global=true` on kernel
`.text` sample. Frame leak audit clean. All BUILD-2 anti-regression tests PASS.

### Lint cleanup commit `57fd235`

`cargo k` warnings: explicit `unsafe` blocks added per Rust 2024 `unsafe_op_in_unsafe_fn`,
`unused mut` removed in GLOBAL fixup. Final warning count 22 at this commit (PM-verified by
clean-tree `cargo k`).

### Verification Log

- Post-fixup `paging::sanity_check` reports `global=true` on kernel `.text` sample — PASS.
- CR4.PGE explicitly enabled (Limine did not set it) — PASS.
- Frame leak audit clean — PASS.
- All BUILD-2 anti-regression tests PASS — PASS.

---

## BUILD-3 sub-commit 1a — Per-CPU Infrastructure Refactor

Commit `305f4d9`. ADR-013 §Q1 + §Q2 + Amendment 1 + Amendment 2. 7 files modified:
`kernel/src/arch/x86_64/{gdt,idt,mod,per_cpu,syscall}.rs`, `kernel/src/mm/slab.rs`,
`docs/ADR-013-phase-2-smp-retrofit.md`. Net ~600 lines (395 insertions, 324 deletions).

### What landed

- `MAX_CPUS = 64` constant; `PER_CPU` becomes `[PerCpu; 64]` array. New fields: `cpu_id`,
  `active_pmap_cr3`, `scheduler_ptr`, `tss_ptr`.
- Per-AP GDT (Option A from ADR-013 Amendment 1): `static GDTS: [Once<PerCpuGdt>; MAX_CPUS]`.
- Per-AP TSS + IST stacks: **static storage**, not heap-backed. This is a deviation from the
  original dispatch (which assumed heap was up by trap-init time). The actual boot order has
  `init_traps()` at line 233 and `mm::init()` at line 288 of `kernel/src/main.rs`. Coder 1
  caught the deviation, used `static IST_STACKS: [PerCpuIstStacks; MAX_CPUS]` and
  `static TSS_STORAGE: [TssSlot; MAX_CPUS]` with `MaybeUninit` and lazy
  `initialize_per_cpu_tss(cpu_id)`, and recorded the correction in ADR-013 Amendment 1's
  "Implementation correction" subsection.
- `init_traps` splits into `init_traps_bsp()` + `init_traps_ap(cpu_id)`.
- `per_cpu::init` splits into `init_bsp()` + `init_ap(cpu_id)`.
- `syscall::init` splits into `init_bsp()` + `init_ap()`. All four MSRs (EFER, STAR, LSTAR,
  SFMASK) are per-CPU.
- `idt::load_for_ap()` — IDT shared (exception handlers stateless, IST indices resolve via
  current TSS); each AP loads but doesn't init.
- `gdt::set_kernel_rsp()` writes via `current_per_cpu().tss_ptr` with `debug_assert!` null-check.
- `mm::slab::init_per_cpu(cpu_id)` exposed. Called from `ap_entry` in sub-commit 1b.
- Compatibility shim `pub fn init_traps()` retained as alias for `init_traps_bsp`; `main.rs`
  unchanged in 1a.

**APs do not exist in 1a.** No Limine MP request. No `ap_entry`. Single-CPU boot only.

### Boot log evidence

From `boot-log-1a.txt` (untracked file at project root):

```
[ kernel ] gdt: cpu=0 loaded (tss_sel=0x30)
[ kernel ] per_cpu: PER_CPU[0] at 0xffffffffa6ccfd40 → IA32_GS_BASE (BSP)
[ kernel ] paging: GLOBAL fixup complete - 4K leaves: 4575, 2M leaves: 251, 1G leaves: 0
[ kernel ] paging: sanity OK - kernel .text sample virt VirtAddr(0xffffffffa6c22ab0) -> phys PhysAddr(0x1f1f3ab0) (global=true)
[ kernel ] frame: leak audit clean (baseline=1026 final=1026)
[ kernel ] phase 0 boot complete
[ kernel ] no scheduler yet — halting forever
```

4K leaf count 3626 → 4575: increase of 949 leaves explained by 64 statically-allocated IST
stack arrays of 20 KiB each. Frame leak audit clean confirms static memory isn't tracked by
frame allocator (correct — it's reserved at link time).

### Course Corrections

- **Boot-order deviation.** Dispatch claimed `mm::init` precedes `init_traps`, so heap-backed
  TSS/IST was feasible. File says reverse: `init_traps()` at line 233, `mm::init()` at line 288.
  Static storage used instead. Forward implication: future BUILDs needing heap-backed per-CPU
  state must change boot order first.

### Verification Log

- `cargo k` warning count 18 at `305f4d9` (down from 22 at `57fd235`). Four warnings retired by
  `#[allow(dead_code)]` on asm-load-bearing offset constants (`KERNEL_RSP_OFFSET`,
  `USER_RSP_SAVE_OFFSET`, `SLAB_STATE_OFFSET`, `star_user_base()`). Constants remain in source
  as documentation of asm layout invariant.
- Boot reaches `\`phase 0 boot complete\`` — PASS. No `BUG:`, no `KERNEL PANIC`.
- Frame leak audit clean (baseline=1026 final=1026) — PASS.
- 23/23 default scenarios PASS reported by coder. `\`expect_pass_but_fail\`` correctly detected
  by meta-test.

---

## BUILD-3 sub-commit 3 (SC-3B) — Q4/Q6 IPC Retrofit

**Closed 2026-05-10** via commit `e2b3f4f`. Sub-step 5c — IRQ-safe acquisition for `Channel.state`. Verification log: `Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md`. Closure report: `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md`. Gate scores: 200/200 NEO_SMP=4, 25/25 NEO_SMP=2. Mechanism named: **Hpw** — preempt-while-holding + IRQ-off same-CPU waiter.

### Follow-up ADR proposals

Three proposals filed in `Plans/` awaiting project-lead ratification:
- `ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md`
- `ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md`
- `ADR-PROPOSAL-2026-05-10-owner-metadata-production-diagnostic.md`

### What landed

- Sub-step 5a: IPI infrastructure (sub-commit 4 dependency)
- Sub-step 5b: `Channel.state` lock-order fix (single-CPU safe, SMP-compatible)
- Sub-step 5c: IRQ-safe acquisition discipline for `Channel.state` call sites (five call sites, `without_interrupts(...)` wrapper)

### Course corrections

- **Release-on-switch pattern.** The original dispatch assumed a single-lock release after `context_switch`. The lock-order resolution required releasing **two** locks (Channel mutex + per-CPU scheduler lock) on the new stack. The `context_switch_with_release` helper accepts a lock list (max 4 entries) for multi-lock release. See `Plans/DISPATCH-2026-05-10-sc-3b-sub-step-5c-irq-safe-lock-discipline-cerebro.md` for the correction.

### Verification Log

- `cargo k` — PASS at commit `e2b3f4f`.
- `cargo neo-test` 200/200 PASS on NEO_SMP=4 — PASS.
- `cargo neo-test` 25/25 PASS on NEO_SMP=2 — PASS.
- Frame leak audit clean — PASS.
- No `BUG:`, no `KERNEL PANIC` — PASS.

---

## BUILD-3 sub-commit 4 (SC-3C) — Q7.b CR3 cache + Q7.c IPI TLB shootdowns

**Closed 2026-05-11 via commits `88c7bc2`, `3272840`, `ead8b19`.** Three sub-commits per SC-3C D12 ratification: 4a (Q7.b CR3 cache), 4b (Q7.c TLB shootdown machinery), 4c (Q7.c call-site wiring). Verification logs: `Plans/SC-3C-SUB-COMMIT-4A-Q7B-CR3-CACHE-VERIFICATION-2026-05-11.md`, `Plans/SC-3C-SUB-COMMIT-4B-Q7C-TLB-SHOOTDOWN-MACHINERY-VERIFICATION-2026-05-11.md`, `Plans/SC-3C-SUB-COMMIT-4C-Q7C-CALL-SITES-VERIFICATION-2026-05-11.md`. Gate scores: `cargo k` PASS, `cargo neo-test all` PASS, 200/200 NEO_SMP=4 PASS, D14 missing-shootdown-detection scenario PASS. Mechanism named: N/A — no new wedge mechanism.

### D-decisions ratified

Fourteen BUILD-relevant D-decisions (D1–D14) plus one meta-decision (D15 — Cerebro deep-research dispatch). Four BUILD decisions revised post-prior-art research (D3 N=16 → N=33, D4 API takes target-set as input parameter with T2 forward optimization, D9 FreeBSD curcpu_cb overlap pattern, D14 missing-shootdown-detection scenario). All finalized by `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` and independently verified by Cerebro deep-research (`Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md`).

### What landed

- Sub-commit 4a: Q7.b CR3 compare-and-skip. Wire `per_cpu.active_pmap_cr3` field in scheduler context-switch path. B2.a design: compare target CR3 against cached value; skip write on match. CR3-write-then-field-store ordering ensures stale-low is the only failure mode (one redundant write, no correctness issue).
- Sub-commit 4b: Q7.c TLB shootdown machinery. New IPI vector `0xfc`, IDT handler, `ShootdownPayload` struct with per-shootdown `AtomicU32` ACK counter, INVLPG helper, per-CPU single-slot shootdown payload via `AtomicPtr`. G3 hybrid granularity (INVLPG for ≤33 pages; full flush above threshold). T1 broadcast target selection. S3 synchronous with a 1ms re-poke window, 50ms final bounded sender wait, and panic-on-timeout for targets that remain non-responsive after re-signal.
- Sub-commit 4c: Q7.c call-site wiring. Modify `pmap.rs` `unmap_page` and related mutation sites to invoke shootdown on cross-CPU-visible page-table changes.

### Course corrections

No course corrections. An initial `q7c_concurrent_munmap_smp` attempt faulted before the intended stale-TLB check because the harness read a user page from kernel mode with SMAP still enforced; corrected to use the existing `with_user_access` helper. This was test choreography, not a shootdown failure.

### Verification Log

- `cargo k` — PASS at commit `ead8b19`.
- `cargo neo-test all` — PASS, including deliberate-failure meta-test detection.
- 200/200 NEO_SMP=4 `smp_cross_cpu_rendezvous_stress` — PASS (results stored under `target/verify/sc3c-4c-stress-neo-smp4-20260511-181228`).
- D14 missing-shootdown-detection scenario — PASS: `q7c_missing_shootdown_detection detected=stale_tlb_read_after_unmap negative_feature=neo-test-missing-shootdown-negative value=0xa7`.
- Frame leak audit clean — PASS.
- No `BUG:`, no `KERNEL PANIC` — PASS.

---

## BUILD-3 sub-commits 5+6 (SC-4) — Q8.a/b capability table SMP + Q8.c blocked-thread-kill walk

**Closed 2026-05-11 via commits `c18646e`, `c02abf7`, `57f3cb1`.** Three sub-commits: 5a (Q8.a capability table SMP — SwitchMutex on process handles/derivations), 5b (Q8.b Phase A — retire `cr3_for_thread` for Thread-level `Arc<Pmap>`), 6 (Q8.c blocked-thread-kill walk + `close_peer` my-side fix + `zombify_thread`). Verification logs: `Plans/SC-4-SUB-COMMIT-5A-HANDLES-DERIVATIONS-SWITCHMUTEX-VERIFICATION-2026-05-11.md`, `Plans/SC-4-SUB-COMMIT-5B-PHASE-A-RETIRE-CR3-FOR-THREAD-VERIFICATION-2026-05-11.md`, `Plans/SC-4-SUB-COMMIT-6-BLOCKED-THREAD-KILL-WALK-VERIFICATION-2026-05-11.md`. Closure report: `Plans/CLOSE-REPORT-2026-05-11-sc-4.md`. Gate scores: `cargo k` PASS, `cargo k --no-default-features` PASS, `cargo neo-test all` PASS, GitHub CI run 25705327371 green. Mechanism named: N/A — correctness-implementation work for ratified D1-D18.

### D-decisions ratified

Eighteen D-decisions (D1–D18) per `Plans/SC-4-THINK-2026-05-11.md`. Cerebro D5 reframed PM's original D4 from "what lock type for PROCESSES?" to "should PROCESSES be on the hot path at all?" — substrate audit confirmed Phase A retirement of `cr3_for_thread` in favor of Thread-carrying `Arc<Pmap>`. Four PM THINK corrections folded; D1 (SwitchMutex) and D7 (exiting flag) cross-validated by Cerebro citations. All finalized by `Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md`.

### What landed

- Sub-commit 5a: Q8.a — retrofit `Process.handles` and `Process.derivations` from `spin::Mutex` to `SwitchMutex`. Enforce `handles` before `derivations` locking in derive/revoke paths. Document revoke walk ordering (PROCESSES snapshot only, per-process locks in ascending ProcessId order). Incident: initial test exposed real derive-vs-revoke race; fixed by validating and removing caller's root handle under lock before cross-process derivation scanning.
- Sub-commit 5b: Q8.b Phase A — add `Thread.pmap: Option<Arc<Pmap>>`, retire `process::cr3_for_thread(pid)` entirely, scheduler CR3 prep reads `next.pmap` directly. Add `Process.exiting: AtomicBool` checked during capability install; `Process.exit_state: AtomicU8` with CAS-claim discipline for natural exit and forced kill. Incident: `Thread::free` slab-drop leak fixed by clearing `thread.pmap = None`.
- Sub-commit 6: Q8.c — add `sched::zombify_thread(Box<Thread>)` for forced retirement of parked IPC threads. `Channel::extract_my_side_if_blocked` for terminate's drained-handle walk. `Channel::close_peer` extracts both peer-side and my-side endpoint states; peer waiters wake with `EPIPE`, my-side waiters are zombified. `process::terminate` handles Ready targets through `kill_thread_by_pid` and Blocked-in-IPC targets through drained channel handle walk.

### Course corrections

One operational adjustment: PL commit `5a88925` (`tools/xtask: retry transient neo-test scenario failures`) landed between SC-3C closure and SC-4 5a. Parallel to the e7782fc pattern from SC-3C. CI reliability discipline continues to evolve with the test surface.

### Verification Log

- `cargo k` — PASS at commit `57f3cb1`.
- `cargo k --no-default-features` — PASS.
- `cargo neo-test all` — PASS, including deliberate-failure meta-test detection.
- GitHub CI run 25705327371 QEMU Integration Tests — success.
- `rg -n "cr3_for_thread" kernel/src` — no matches; command exits 1 (Phase A retirement complete).
- `rg -n "q8c_deferred\|kill-on-Blocked.*EINVAL" kernel userland` — no matches (all deferred markers cleared).

---

## Transition from BUILD-1

Update: this transition is complete; see BUILD-2 and BUILD-3 sections below.

BUILD-1 gave BUILD-2 a real capability substrate: rights bits exist, handle
indices remain stable, revoke can invalidate descendants, and grant can move
authority between live processes without ambient names.

Both BUILD-2 (allocators) and BUILD-3 (SMP) are in flight; design substrate is
ADR-012 + ADR-013.

---

## ADR Ledger (post-BUILD-3)

| ADR | Title | Status | Date | Related Bible articles |
|-----|-------|--------|------|------------------------|
| ADR-014 | SC-3B outcome B — release-on-switch with prior-art convergence | ratified | 2026-05-10 | 8.5.3, 9.1 |
| ADR-015 | KASLR-tolerant heap placement | ratified | 2026-05-07 | 6.2, 7.4 |
| ADR-016 | SMP wedge fix — lock-spin resolution and structural conformance plan | ratified | 2026-05-08 | 8.5.3 |
| ADR-017 | Janus first-pass citation spot-check methodology | ratified | 2026-05-10 | N/A (process) |
| ADR-018 | Bible 8.5.4 preempt-while-holding clarification | ratified | 2026-05-10 | 8.5.4 |
| ADR-019 | SwitchMutex IRQ-off by construction | ratified | 2026-05-10 | 8.5.3, 8.5.4 |
| ADR-020 | Owner metadata as production-stable diagnostic | ratified | 2026-05-10 | 8.5.3 |

---

## Forward reference: async discipline for Phase 8+ libneo

When Phase 8+ introduces a userspace async runtime in `libneo` for streaming syscalls (`sys_inf_stream_next` per ANSPEC-001 §5.3, future cap-streaming variants), async-bloat avoidance becomes a real concern.

Reading reference for whoever picks up that work:

- Tweede Golf, "Debloat your async Rust" (2026-04-13): https://tweedegolf.nl/en/blog/235/debloat-your-async-rust

Key tactics from that article that apply to libneo's async surface:

- `impl Future` pass-through instead of `async fn` for thin wrappers around blocking syscalls. Avoids nested state machines.
- `futures::FutureExt::map` for postamble transforms instead of `async fn` with a single `.await`.
- Pass references to large data across `.await` points, not values. Stack-frame discipline (per ANSPEC-001 §6.3.4) translates directly: variables held across `.await` cost memory in proportion to suspension duration.
- Audit `async fn` declarations for ones that don't actually need to be async (no `.await` in the body or only `.await` on `ready` futures). Replace with `fn` returning `impl Future`.

The kernel itself remains synchronous per Bible Article 8.4. This note applies only to userspace libneo code that wraps streaming kernel syscalls in an async-friendly API for application use.

This is a Phase 8+ concern. Recorded here so the libneo async-runtime author finds it when they search for "async" in the docs tree. Not actionable in Phase 2.

---

## Forward reference: Phase 2+ TLB management optimizations

SC-3C (sub-commit 4) landed the correctness baseline for SMP TLB management: CR3 compare-and-skip cache (Q7.b), broadcast IPI shootdowns with INVLPG hybrid granularity (Q7.c). Several performance and safety optimizations are deferred to future sub-commits. Recording them here so the next BUILD author finds them when they search for "TLB", "PCID", or "shootdown" in the docs tree.

### PCID / ASID enablement

Deferred per SC-3C D2. Current kernel has no PCID; every CR3 write triggers a full local TLB flush. Interesting when TLB-miss cost dominates measurement or when many concurrent processes exceed TLB natural capacity. Linux global-ASID promotion mechanism uses a sampling-gate-plus-rising-floor pattern (1-in-32 statistical gate, not deterministic 3-CPU threshold) — see `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md` §3.5. Future ADR-driven sub-commit when measurement justifies.

Source: `Plans/SC-3C-THINK-2026-05-10.md` §3.1 B3/B4 alternatives; `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` §4 item 1.

### T2 targeted shootdown (per-Pmap CPU mask)

Deferred per SC-3C D4. Current broadcast sends IPI to all CPUs except self. T2 would track which CPUs have loaded each Pmap's CR3 and target only those. Critical design choice for the future dispatch: **strictly-accurate vs loosely-accurate per-mm cpumask**. Linux chose loosely-accurate (add-only on switch-in, lazy scrub on IPI receipt) because the context-switch path is hotter than the IPI path. NeoPlutarch's inference scheduler may hold threads on accelerators for long stretches with rare context switches — strictly-accurate could be cheaper net. This trade-off must be documented explicitly in whichever dispatch lands T2.

Source: `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md` §3.1; `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` §4 item 2.

### Lazy TLB (receiver-side short-circuit)

Deferred per SC-3C D6. Linux's `should_flush_tlb` pattern: skip TLB invalidation on CPUs that aren't actively running the affected mm. **Critical exception:** lazy-TLB optimization is bypassed unconditionally during page-table teardown (`freed_tables=true`). A CPU with stale upper-level paging structures cached can fault into freed memory after teardown completes — Linux encodes this as an explicit flag that forces delivery to every CPU regardless of lazy state. Any NeoPlutarch lazy-TLB implementation must encode the same exception.

Source: `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md` §4; `Plans/SC-3C-THINK-2026-05-10.md` §4.1 L2/L3.

### Shootdown batching (mmu_gather analogue)

Deferred per SC-3C D7. Linux's `mmu_gather` batches page-table modifications and sends one IPI for many pages. Interesting if profiling shows shootdown-per-unmap is a workload bottleneck. Adds significant complexity; defer until measurement justifies.

Source: `Plans/SC-3C-THINK-2026-05-10.md` §4.1 B2; `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` §4 item 4.

### AMD INVLPGB / Intel RAR hardware-broadcast

Phase 3+ forward marker. INVLPGB (AMD Zen 4+) and RAR (Intel 4th-gen Xeon+) can invalidate TLB entries across all CPUs via a single hardware broadcast, eliminating IPI-based shootdown entirely. Gate: NEO_SMP > 4 or measurement-driven. Linux's `broadcast_tlb_flush` integration point uses `mm_global_asid(mm)` check inside `flush_tlb_mm_range` — not inside the function itself.

Source: `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` §4 item 6; `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md` §3.4, §3.5.

### Spectre-v2 BTB flush on context switch (IBPB analogue)

Phase 2+ CPU-vulnerability mitigation cluster. Every CR3 switch is a potential BTI attack surface. Deserves its own ADR; not SC-3C territory. Related to Q7.b's CR3 cache: once CR3 writes are conditional, the IBPB call sites must be placed correctly relative to the skip path.

Source: `Plans/DISPATCH-2026-05-11-sc-3c-plan-stage-jarvis.md` §2 context (Linux `cond_ibpb` BTB-flush interaction); `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` §2 input context.

---

## Forward reference: Phase 2+ capability + process-exit optimizations

SC-4 (sub-commits 5+6) landed the correctness baseline for SMP capability-table operations and process-exit sequencing. Several deferred items are recorded here so the next BUILD author finds them when they search for "capability", "RwLock", "exit", or "zombify" in the docs tree.

### FM-D5.1: spin::RwLock pathological at 4-64 CPU range

`spin::RwLock` shows a 5× regression vs `Mutex` in the 4–64 CPU contention range per Cerebro D5 §5.4. Any future capability-table read-heavy path that considers `RwLock` must benchmark first. At NeoPlutarch's scale (≤64 CPUs, slab-backed handles), `SwitchMutex` remains the default choice.

Source: `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md` §5.4; `Plans/CLOSE-REPORT-2026-05-11-sc-4.md` §6.2.

### FM-D5.2: arc-swap Phase B fallback

Deferred unless Phase A audit reveals a blocker. `arc-swap` would allow lock-free readers on the process-table path, but Phase A (Thread-carrying `Arc<Pmap>`) removes PROCESSES from the hot path entirely, making this optimization unnecessary for current workloads.

Source: `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md` §6; `Plans/CLOSE-REPORT-2026-05-11-sc-4.md` §6.2.

### FM-D5.3: illumos doors architectural reference

Illumos `door` mechanism provides a design language for cross-process IPC that may inform Phase 3 capability-streaming variants. Referenced in Cerebro D5 as prior art for single-syscall RPC patterns.

Source: `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md` §10; `Plans/CLOSE-REPORT-2026-05-11-sc-4.md` §6.2.

### FM-D5.4: PROCESSES single-Mutex correctness boundary

PROCESSES registry validated as correct with a single `Mutex` at NeoPlutarch's scale, cross-referenced against FreeBSD `allproc` and illumos `pidlock`. Future multi-registry sharding requires explicit ADR.

Source: `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md`; `Plans/CLOSE-REPORT-2026-05-11-sc-4.md` §6.2.

### FM-D13: Single-thread-per-process invariant

Phase 2+ multi-thread per process requires iterating all threads of `target_pid` via a scheduler-side per-pid enumeration that doesn't exist yet. `zombify_thread` is reentrant; the walk pattern generalizes, but the enumeration primitive must be built first.

Source: `Plans/SC-4-THINK-2026-05-11.md` §7 D13; `Plans/CLOSE-REPORT-2026-05-11-sc-4.md` §6.2.

### FM-D14: COMPLETED_EXIT_CODES growth bound

Phase 2+ `sys_wait` consumer deferred. Without a wait/consume path, `COMPLETED_EXIT_CODES` grows unbounded per exited process. Growth is bounded by process lifetime in practice but requires a formal consumer to prevent memory leaks in long-running workloads.

Source: `Plans/SC-4-THINK-2026-05-11.md` §7 D14; `Plans/CLOSE-REPORT-2026-05-11-sc-4.md` §6.2.

### FM-Q9: Per-CPU reaper distribution with multi-thread support

ADR-013 §Q9 specified per-CPU reapers + `Process.live_thread_count: AtomicU32` gate. Sub-commit 2 attempted implementation; 100x `ipc_heap_kill_smoke` failed at run 81/100 with a kernel hang in `reap()`; 8-experiment bisection localized to Q9-specific machinery; deferred. The interim shape (BSP scans all per-CPU schedulers' zombie queues; APs do not reap) is provably correct under SC-4 D13's single-thread-per-process invariant — the thread reap IS the process exit, and SC-4 D8's `Process.exit_state: AtomicU8` CAS already provides Process-exit serialization. `live_thread_count` is obsolete under D13. Q9 lifts only when Phase 2+ multi-threaded user processes enter scope, at which point design alternatives in `Plans/Q9-EXPLORATION-2026-05-11.md` §3 (a/c/d) become live options. See ADR-013 BUILD-3 amendment "sub-commit 2 Q9 deferred with bisection evidence" for the original deferral; ADR-013 BUILD-3 amendment "Q9 deferred permanently under D13 invariant" (newly added by this commit) for the final disposition.

Source: `Plans/Q9-EXPLORATION-2026-05-11.md` §6.1; `docs/ADR-013-phase-2-smp-retrofit.md` BUILD-3 amendment cluster.
