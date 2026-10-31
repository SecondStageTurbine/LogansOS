<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: C1 fixes re-check, relay #131 / #184

Reviewer: Jarvis. Worktree: `E:\AI Automation\workers\jarvis`.
Branch: `codex/ot61-c1-fixes-recheck-jarvis`. Read squash `7636372b`
and current main `87458b7e`. References below use `87458b7e`.
**Original C1.2 cache/setter findings and both C1.3b findings: closed.**
**Checklist verdict: FAIL only for item 1's stronger every-return wording.**

1. **FAIL literally; PASS for the implemented switch-in/set contract.**
   `kernel/src/sched/scheduler.rs:1000`, :1009 always call
   `load_user_fs_base`, whose :1036 WRMSR has no cache check.
   `set_current_fs_base` (:1048) writes the Thread field and hardware under
   the local scheduler lock with IF=0. Searches find no `active_fs_base` or
   `FS_BASE_UNKNOWN` left in kernel source. Thus the original skipped-write
   counterexamples are repaired, including a cross-process switch-in.
   Concrete counterexample to *every return*: set TLS to nonzero B, execute
   `mov fs,0x23`, then call a nonblocking ordinary syscall without a context
   switch. `syscall_dispatch`'s return (:1361) restores no FS base, and neither
   assembly tail does so; hardware remains zero while Thread.fs_base=B.
   The new test itself relies on this behavior: `userland/neo-init/src/main.rs:2829`
   loads the selector then :2830 probes zero through a syscall. This is not
   a surviving cross-thread leak: the next switch-in/set repairs it. Either
   narrow the checklist to that documented contract (`scheduler.rs:1028`) or
   add a write on every return if that stronger contract is intended.
2. **PASS — ESRCH reaches the boundary with no owned resource retained.**
   The only external `wait_on_key` callers are `futex::wait`
   (`kernel/src/sched/futex.rs:283`, tail return) and join
   (`kernel/src/sched/process.rs:4165`, explicit return at :4171).
   Join drops its registry guard before waiting (:4146), retains only raw
   pointers/ids, and deliberately leaves the claimed record to process teardown.
   `wait_on_key` consumes its temporary Process Arc in `is_some_and` and
   drops FUTEX_QUEUE on its ESRCH return (:163). No join retry remains on ESRCH.
3. **PASS — IF stays clear through the final user transition.**
   `kernel/src/arch/x86_64/syscall.rs:1363` disables IRQs before the final
   retire check. Its returning path owns no guard that could restore IF=1.
   Both assembly variants restore FPU/GP state, execute CLI/SWAPGS/IRETQ
   (:842, :945); neither contains STI or POPF. The xsave restore helper does
   not enable interrupts. The scenario-only tail hold preserves IF. A late
   retire IPI remains pending to CPL3 instead of being consumed in the tail.
4. **PASS — discriminators match the removed mechanisms, by source trace
   and inspected recorded falsifications.** `neo-init/src/main.rs:2812`
   verifies selector loads actually zero the base, retries only for an
   intervening restoration, checks the same-value setter, then sleeps and
   requires restoration on the same CPU. Restoring the full cache yields
   author result 8024; restoring only switch-in caching yields 8034.
   `neo-worker/src/main.rs:685`, :772 run a late join on CPU1; discarding ESRCH
   again leaves role 5 stuck before the boundary. :697, :788 hold CPU2 after
   the last check; the IF=1 arming syscall and three-tick tail hold in
   `process.rs` let the sole retire IPI land there. Removing the final CLI
   leaves role 6's no-syscall ring-3 spinner alive. These falsifiers were not
   rerun here; their schedules were checked against the implementation.
5. **PASS — cfg exclusion plus author section comparison.**
   Scheduler helper/call sites and Thread field are `browser-runtime` gated;
   final CLI/check is likewise gated at `syscall.rs:1361`. New scenario
   hooks use `neo-test-threads-*`. #184 records equal `.text`/`.rodata` for
   default/desktop/usb-host and 25 changed `.data` bytes attributed to panic
   locations. This review did not rebuild those binary pairs or independently
   classify each differing data byte; it does not claim whole-ELF equality.

Independently queried the dispatched gate:
[batch 13](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40799093977)
is completed/success at `0395982f3363707dbe648d9e7725c79614beca75`.
RAG health was healthy; review used pinned diffs, exact symbol searches and
direct source reads. No new QEMU/injected-IPI execution is claimed. The stale
setter comment at `scheduler.rs:1044` still mentions a per-CPU cache; remove
that wording along with clarifying item 1. No need to reopen the two repaired
exit-group mechanisms on account of that contract clarification.
