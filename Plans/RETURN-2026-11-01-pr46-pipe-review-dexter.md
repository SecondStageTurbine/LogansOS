<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 263 — PR #46 (L1a.1 step 1: the kernel pipe's state machine and host KATs) — Dexter

Reviewed head: `d0978ee6a4c99bf931490c4b951d8c76faf108a3` (PR #46, `claude/l1a1-pipe`).
Base: `origin/main` at `f2210a32a0f2e9131dff098fe3ed4a25ccf91594`; merge-base
`d2389eee2906e43791d07292c7b9e385a4d77b90`. The branch is behind `main` only by the
docs-only relay commit `f2210a32a`, so `origin/main...pr46` reviews the current code.
RETURN branch: `dexter/pr46-review`, based on the `origin/main` above.

Design: `docs/THINK-2026-10-31-l1a-terminal-window.md` §3.3 and
`docs/THINK-2026-10-29-ot88-app-model.md` §3.2 (T-a). Line numbers below are the reviewed head.

Verification on the reviewed head: `cargo test -p pipe-logic-tests` → 12/12 pass;
`rustfmt --check` on both new files clean; `cargo clippy -p pipe-logic-tests --all-targets
-- -D warnings` clean. I also ran a scratch differential fuzz (not committed: 20,000 random
op sequences × 200 ops, capacities 1..17, including grants and closes) of the module against
a `VecDeque` model of the same semantics: no divergence in bytes, counters, `buffered`,
`readable`, `writable`, or either wake flag.

Overall: **PASS**. No blocking defect. The notes in items 1–4 are edge pins, and item 5 is
the step-2 friction list the dispatch asked for — none of it is a defect in this step.

## Checklist

- **PASS — 1. Every state and transition the THINK names exists.**
  - Empty → data: `write` stores, and wakes readers on the empty→non-empty edge
    (`kernel/src/ipc/pipe_logic.rs:188-208`). Data → empty: `read` drains and resets `head`
    (`:233-242`). Full: `space == 0` → `WouldBlock` (`:191-194`).
  - Reader closed while a writer is blocked: the last read close drops the buffer and returns
    `Wake { writers: true }` (`:159-166`); the writer's next `write` returns `BrokenPipe`
    before looking at space (`:185-187`). KAT `epipe_is_immediate` (`tools/pipe-logic-tests/src/lib.rs:124-136`).
  - Writer closed with data buffered: the last write close leaves the buffer and wakes readers
    (`:167-171`); the reader drains, then gets `Eof` (`:220-228`). KAT `:110-122`.
  - Both closed: `is_dead()` is both counts zero (`:122-124`); closing either end last is
    handled (`:149-174`). KAT `:161-165`.
  - Zero-length write: `Written(0)` with no wake (`:188-190`). Zero-length read: `Read(0)`
    without consuming (`:223-232`). KAT `:194-202`.
  - Endpoint grants: per-handle `add_endpoint` with a checked counter (`:138-145`); a non-last
    close returns the default wake (`:172`). KATs `:138-166`.
  - Matches the THINK table (`docs/THINK-2026-10-31-l1a-terminal-window.md:173-177`) and
    OT-88 §3.2's rows (`docs/THINK-2026-10-29-ot88-app-model.md:379,:398-401`).
  - Notes, not fails: (a) a zero-length write with no reader returns `BrokenPipe` because the
    reader check precedes the empty check (`:185-190`); a zero-length read with no writer
    returns `Eof`, not `Read(0)` (`:220-228`). The THINK does not pin either, and POSIX leaves
    zero-length I/O on a non-regular file unspecified; pin one answer in step 2 if callers
    branch on it. (b) `add_endpoint` will revive an end whose count is already 0 (`:138-145`)
    while a double close is refused (`:154`): there is no "closed forever" bit, only the count.
    That is unreachable through the handle table (a grant needs a live handle to name), but a
    debug assertion would make the state machine total.

- **PASS — 2. Bounds are fixed and cannot overflow.**
  - Capacity is fixed at creation and bounded by `MAX_PIPE_CAPACITY` (64 KiB)
    (`:29,:82-88,:92-103`); every call re-checks `storage.len() == capacity`
    (`:182-184,:217-219`), and the stdin/stdout sizes match the THINK (`:25-27` vs
    `docs/THINK-2026-10-31-l1a-terminal-window.md:173`).
  - A write larger than the free space stores `min(space, src.len())` and reports it as a
    partial success (`:191-201`); it never indexes past either end of the ring (`:197-200`),
    and `read` mirrors that (`:235-238`).
  - Counters cannot wrap: `readers`/`writers` are `u32` with `checked_add` (`:143`) and
    `checked_sub` (`:154`); a failed add does not assign. `head`/`len` are bounded by
    `capacity`, so `head + len` ≤ 2 × 64 KiB — no `usize` overflow on the ring arithmetic
    (`:197,:238`).
  - KATs: capacities and wrong storage `tools/pipe-logic-tests/src/lib.rs:58-71`, partial
    write `:96-108`, wrap `:86-94`, 10,000 bytes through a 7-byte pipe `:204-240`; the fuzz
    above adds 4M ops.
  - Note, not a fail: the `TooManyEndpoints` path has no KAT because the counts are private.
    Inspection is the evidence. If the guard is to be tested, add a `#[cfg(test)]` constructor
    or a small `add_n` test seam in step 2.

- **PASS — 3. EOF and EPIPE match the THINK (and the POSIX points it names).**
  - EOF is returned only when the buffer is empty and the last writer has closed
    (`:220-228`); a writer close with data buffered delivers the data first (`:167-171`).
  - EPIPE is immediate when the last reader is gone, even at a full pipe, and the unreadable
    bytes are dropped (`:159-166,:185-187`). This is the OT-88 EPIPE-ordering row
    (`docs/THINK-2026-10-29-ot88-app-model.md:398`).
  - The two wake edges are exactly CP-14's: readers are woken only on empty→non-empty
    (`:196,:202-208`) and writers only on full→non-full (`:234,:243-249`); last-endpoint
    closes wake the other side (`:159-171`).
  - Notes, not fails: (a) `write` returns a short success whenever there is *some* space
    (`:195`). That matches OT-88's "blocks only when it is full" (`docs/THINK-2026-10-29-ot88-app-model.md:379`)
    and POSIX permits short writes, but this state machine does not provide POSIX's
    ≤ `PIPE_BUF` atomicity. Concrete input: capacity 4096 with 3000 bytes buffered, a
    2000-byte `write` returns `Written(1096)`, not `WouldBlock` (`:191-201`). Step 2's
    blocking `sys_write` must either loop to completion for small writes or define/report
    short writes. (b) `readable()`/`writable()` treat EOF/EPIPE as ready (`:128-135`), which
    is right for `SYS_WAIT_ANY`; the consequence is that an EOF'd read end stays ready
    forever, so the terminal must drop it from the wait set when it shows "[process exited]"
    (`docs/THINK-2026-10-31-l1a-terminal-window.md:176`) or it will spin on a level-triggered
    wait.

- **PASS — 4. The host KATs cover each transition, with hand-derived expectations.**
  - Reader-close race: `:124-136`. Writer close with data: `:110-122`. Both closed and counts
    across grants: `:150-166`. Full buffer: `:96-108`. Zero-length: `:194-202`. Wrap:
    `:86-94`. Long stream: `:204-240`. Readiness: `:176-192`. Double close: `:168-174`.
    Capacity/wrong storage: `:58-71`.
  - I re-derived every expected value against the THINK/OT-88 table independently of the
    module; they are hand values, not round-trips through the implementation.
  - Coverage gaps, all edges rather than named transitions, so not a fail: (a) zero-length
    write *on a full pipe* and *with no reader* are not pinned (`:194-202` only covers a live,
    non-full pipe); (b) `BadCapacity` on `read` is untested — only the write path
    (`:70` vs `pipe_logic.rs:217-219`); (c) `add_endpoint(End::Read)` and the `TooManyEndpoints`
    path have no KAT (the writer grant is covered, `:141-166`); (d) the EPIPE-order
    "close reader first, then writer" pair is not asserted together. Also, the fallback block
    at `:231-237` is unreachable (a read from a full pipe always clears `writer_parked` at
    `:228-230`); it is harmless dead test code.

- **5. What step 2 (wiring it into the kernel) will need — answer, not a verdict.** Nothing
  here fails step 1; these are the points the wiring must settle, hardest first.
  1. **The buffer's owner and charge.** `Pipe` owns only counters and `head`/`len`; every call
     takes caller-owned storage, checked only for length (`pipe_logic.rs:182-184,:217-219`).
     Step 2 must own the 4 KiB/64 KiB buffer, pair it with the `Pipe` under one lock, and
     charge/free it. `Pipe::new` cannot fail for memory and `PipeError` has no out-of-memory
     variant (`:68-77,:92-103`), so the fallible allocation path is entirely step 2's. The
     doc's "no allocation" (`:8`) is true of the state calls, not of a pipe object. A
     same-length but different buffer is silently accepted, so make the pairing structural
     (one struct inside the lock), not a convention.
  2. **Identity, `Arc`, and the wait key.** `SYS_WAIT_ANY` needs a per-object wait key and a
     per-side readiness check (`kernel/src/ipc/mod.rs:459,:469`) plus the reused-slot identity
     re-check (`kernel/src/arch/x86_64/syscall.rs:6195-6270`). `Pipe` has no identity and
     derives `Copy` (`pipe_logic.rs:81`); step 2 must wrap it in `Arc<...>` (stable address
     for the key, plus `End`) and add an `End` index/key mapping. A naive
     `Handle::Pipe { pipe: Pipe }` would copy the counters on `Handle::clone`
     (`kernel/src/sched/process.rs:881-882`) and split the accounting.
  3. **Wake fan-out and CP-14.** The module returns `Wake { readers, writers }` but holds no
     wait-queue handle. Step 2 must add per-pipe, per-direction queues, enqueue the parked
     thread and fire the wake under the pipe lock — the check and its effect in one critical
     section, as the module's own doc says (`:9-12`). The flags are necessary, not sufficient;
     CP-33/CP-67/CP-85 work lives here, not in this file.
  4. **Blocking write semantics.** With `Written(n)` partial whenever there is some space
     (`:195-201`), the blocking `sys_write` must either loop until the whole request is
     written (and decide what kill/interrupt does mid-loop) or return a short count. The
     module has no "write what fits, then park for the rest" operation; that decision is
     step 2's.
  5. **Kill-switch gating.** The file carries no `#[cfg(feature = "pipe")]` of its own; step 2
     must gate the `mod` declaration *and* the `Handle::Pipe` variant, so `pipe` off is
     today's syscall table with no handle type (OT-88 §3.2's kill-switch leg). The KAT crate
     mounts the source by path (`tools/pipe-logic-tests/src/lib.rs:12`), so keep that path
     stable when adding the `mod` line.
  6. **Teardown and release.** `is_dead()` is only a predicate (`:122-124`); step 2 must hang
     the buffer/charge release off the both-closed point, close each end exactly once (a
     double close is an error, `:154`), and perform all wakes from the last-endpoint close
     (`:159-171`) — including for a process killed while its peer is parked.
  7. **End-agnostic readiness.** `readable()`/`writable()` take no end (`:128-135`); the wait
     scan must dispatch on the handle's `End`, and `End` has no `idx()`/kind mapping yet
     (`:33-36`). Trivial, but step 2 adds both.

No kernel code is wired by this step: the diff is 5 files with no `mod` line and no feature
(`git show --stat d0978ee6`), and `tools/pipe-logic-tests` is a host-only workspace member.
This was a read-only review; the branch carries only this RETURN.
