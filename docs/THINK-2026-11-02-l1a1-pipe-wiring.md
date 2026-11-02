<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: L1a.1 step 2, wiring the pipe into the kernel (2026-11-02)

**Status:** design for review before the code. **Author:** Seat B. **Builds on:** `kernel/src/ipc/pipe_logic.rs`
(step 1, #46, merged as `2a27d7c`), `docs/THINK-2026-10-29-ot88-app-model.md` §3.2 (T-a), and
`docs/THINK-2026-10-31-l1a-terminal-window.md` §3.2-3.4. **Answers:** Dexter's #46 review notes (relay 263, items 1-5).

Citations are to `origin/main` at `792f0bb`.

## 1. Three facts about the kernel that shape this

1. **Channels have no wait queues.** A channel is a synchronous rendezvous: the parked `Box<Thread>` lives inside
   the endpoint state (`kernel/src/ipc/mod.rs:78-119`) and the completer hands off to it. A pipe has many-to-many
   waiters and no rendezvous, so it **parks by wait token** instead, as the futex and `SYS_WAIT_ANY` do:
   `scheduler::try_block_current_with_token_and_guard` (`kernel/src/sched/scheduler.rs:2167`) to park, and
   `wake_thread_token` (`:2391`) to wake. The fallible form returns `Err(guard)` when the token table is full, which
   becomes `EAGAIN`, so a user can't trip the infallible form's `expect`.
2. **fd 0 and fd 1 never consult the handle table.** `sys_write_impl` sends fd 1 to serial and the framebuffer
   console (`kernel/src/arch/x86_64/syscall.rs:6414-6443`); `sys_read_impl` sends fd 0 to `sys_read_stdin`
   (`:6574`, `:6629-6671`). Inherited handles land at indices 0..n in spawn order (`kernel/src/sched/process.rs:4298-4320`),
   so slots 0 and 1 already hold other things (Arthur's channels). Hence OT-88's rule stands: **stdio is selected
   explicitly at spawn**, never by slot.
3. **Closing a channel handle closes the channel** (`sys_close_impl` calls `close_peer`, `syscall.rs:6881-6908`),
   even while a granted copy is open. A pipe must not copy that: its endpoints are counted **per handle**. The
   template is the socket's `retain_net` / `release_net` (`process.rs:1945`, `:1958`): every handle entering a table
   retains, every release that was the last one closes.

## 2. The pieces

**2a. The object, its syscalls and its wait item** (feature `pipe`, which needs `wait-multiplex` for the token park
and the wait item):

- `kernel/src/ipc/pipe.rs`: `struct PipeObject { state: IrqSafeMutex<Inner> }`, `Inner { logic: pipe_logic::Pipe,
  storage: Box<[u8]>, readers: TokenList, writers: TokenList }`. One struct under one lock (Dexter 5.1): the storage
  can't be paired with the wrong `Pipe`. The `TokenList` holds parked wait tokens, a small bounded array; a full list
  means `EAGAIN`, never an unbounded allocation. Stale tokens are harmless (`wake_thread_token` returns `Gone`).
- `Handle::Pipe { pipe: Arc<PipeObject>, end: End, cap_id, rights }`, `#[cfg(feature = "pipe")]` on the variant and
  on every arm that matches it (Dexter 5.5): `cap_id`, `rights`, `handle_info`, `with_cap` (process.rs 1041-1169),
  `ipc_lookup` (syscall.rs 5951-5995), `obj_weights::object` (weight `ARC + size_of::<PipeObject>() + capacity`),
  and `notify_removed` (process.rs 1987-1992, so `SYS_WAIT_ANY` sees the handle go: CP-61).
- **Endpoint counts:** `retain_pipe` on install calls `add_endpoint(end)`; `release_pipe` (in `Detached::release`,
  process.rs 1772-1797) calls `close_endpoint(end)` and fires the returned wakes. An end at count 0 can't be revived:
  `add_endpoint` refuses it (Dexter 1b; a step-1 follow-up in `pipe_logic.rs`, with a KAT).
- **Syscalls** in the free range `0x1B7`-`0x1BF` (all `cfg(feature = "pipe")`, each with an SDK-ABI row):
  - `SYS_PIPE_CREATE(capacity) -> rd | wr << 32`, two installs with the `sys_chan_create` rollback
    (syscall.rs 2112-2155); capacity 1..=64 KiB, else `EINVAL`.
  - `SYS_PIPE_READ(h, buf, len, deadline_ns) -> n`, and `SYS_PIPE_WRITE(h, buf, len, flags) -> n`. `deadline_ns`
    `0` is a try (`EAGAIN`), `u64::MAX` blocks, anything else is a deadline (`ETIMEDOUT`): the terminal THINK's
    `read_timeout` in one call. Buffers move through `user_copy` in 256-byte stack chunks (the `SYS_WRITE_CHUNK`
    pattern, syscall.rs 1531); a read checks the destination with `check_user_writable` before consuming bytes it
    can't put back (CP-99, syscall.rs 1574).
- **Blocking write (Dexter 3a/5.4):** a blocking write loops until all `len` bytes are written. A write of at most
  `PIPE_ATOMIC = 512` bytes is atomic: it waits for room for all of it, so two writers' small writes never
  interleave. A kill or `EPIPE` mid-loop returns the count written so far, if any. KATs in `pipe-logic-tests` for
  the atomic-write rule (a `write_all_or_none` helper in `pipe_logic.rs`).
- **Zero-length I/O (Dexter 1a), pinned as the contract:** a zero-length write is `EPIPE` with no reader, else 0; a
  zero-length read is 0 with bytes or a writer, `EOF` (0) with neither. Same as Linux. KATs for the full-pipe and
  no-reader cases.
- **`SYS_WAIT_ANY` item:** `WAIT_KIND_PIPE = 4` (`kernel/src/sched/wait_any_logic.rs`, `userland/libneo/src/wait_abi.rs`).
  Readiness is end-aware (Dexter 5.7): a read end is ready when `readable()`, a write end when `writable()`. The wait
  key is `wait_any::pipe_key(ptr, end)`, the channel key's shape; the pipe fires it on every wake edge, under its own
  lock (the order `PipeObject.state` → `WAIT_TABLE` → scheduler, as for channels). An EOF'd read end stays ready
  (Dexter 3b); the terminal drops it from its wait set when it shows "[process exited]" (L1a.3).

**2b. Stdio selection and the term page** (after 2a merges):

- `Process` gains `#[cfg(feature = "pipe")] stdio: SwitchMutex<Option<Stdio>>`, `Stdio { stdin: Option<Arc<PipeObject>>,
  stdout: Option<Arc<PipeObject>>, term: Option<Handle> }` (the `obj_spawn` cfg-field pattern, process.rs 489-490).
- `SYS_SPAWN_STDIO` (or a flags word on spawn): names which inherited handles are stdin, stdout and the term page;
  each is checked to be the right kind and end, else `EINVAL`.
- `sys_write_impl` fd 1 and `sys_read_impl` fd 0: if the process has a selected pipe, use it (blocking); else
  today's path, byte for byte.
- `HandleKind::Pipe` and `HandleKind::TermPage` in the std overlay; `std::os::neoplutarch::term` and `IsTerminal`
  (terminal THINK §3.4).

## 3. Pitfall bank (`docs/concurrency-pitfalls.md`)

| Class | Does 2a introduce it? | Guard | Discriminator (scenario `pipe_core` / `pipe_park_kill`) |
|---|---|---|---|
| **CP-14** lost wake | Yes, at empty→non-empty and full→non-full | The check, the token store and the release are one critical section: `try_block_current_with_token_and_guard` takes the `PipeObject` guard and drops it after the switch | A two-CPU leg: a writer forced (armed point) between the reader's empty check and its park; the reader wakes. Control build without the guard-held park: it hangs, and the scenario's deadline fails it |
| **CP-33** kill completeness | Yes, a new park site | Token parks only, so `terminate`'s `zombify_parked_thread_by_pid` (process.rs 5819-5832) finds them; the pipe's token lists hold no `Box<Thread>` | Kill a child parked in a read and one parked in a full write; both reap, the zombie count returns to 0 |
| **CP-61** wait on a reused slot | Yes, the new wait item | The pipe item pushes its key and `CapId` into `WaitIdentity`; `notify_removed` fires `fire_handle_gone` for `Handle::Pipe` | Close and reuse a pipe slot under a `WAIT_ANY`; the waiter retries on the new identity. Control: `wait-multiplex-no-identity-check` fails it |
| **CP-67** `Arc` across a park | Possible: the syscall frame holds `Arc<PipeObject>` | The frame drops its `Arc` before parking; the parked thread holds only a token, and the object stays alive through the handle table. If a wake needs the object, it looks the handle up again | `pipe_park_kill` with the per-kill leak counter at 0 |
| **CP-85** exit retiring one thread | Touches it: a threaded child exits with a reader parked | Endpoint release in `Detached` on the last handle's drop wakes the parked reader with EOF / writer with EPIPE | A threaded child exits while its sibling is parked in a pipe read; the read returns 0 |
| Wake on the last endpoint (OT-88) | Yes | `release_pipe` fires `close_endpoint`'s wakes | Kill the only writer while the reader is parked: the reader gets EOF |
| Counts across grants (OT-88) | Yes | Per-handle retain/release | Grant the write end away, close the original: the reader still waits; close the granted one: EOF. The object's charge returns to 0 |

I-14 (driver fault containment) and the AI-governance locks: N-A. No ISR path, and no governance lock is taken.

## 4. Kill-switch

`pipe` off: no `Handle::Pipe` variant, no syscall arms (the numbers return `ENOSYS`), no wait kind, no `stdio` field.
With `pipe` on and nothing creating a pipe, every existing scenario's captures are unchanged (a leg in the gate,
OT-88 §3.2's kill-switch leg).

## 5. Gate and review

2a: the 4-shard gate with `pipe_core` (round trip, partial writes, atomic small writes, EOF/EPIPE ordering, counts
across a grant, the `WAIT_ANY` item, deadline reads) and `pipe_park_kill` (CP-33, CP-67, CP-85 legs), plus the CP-14
two-CPU leg and the CP-61 reuse leg with their controls. It touches thread park and teardown, so the AAG needs a
cross-family lens (CLAUDE.md, the SMP-core rule): **GPT-6.1-Sol, high**.

2b: the 4-shard gate with `pipe_stdio` (a child whose fd 0/1 are pipes; a child with channels in slots 0/1 and no
selection is unchanged), and the same review lens for the spawn change.

## 6. Questions

1. The `TokenList` bound: 8 parked readers and 8 parked writers per pipe (a terminal has 1 of each), `EAGAIN` beyond.
   Recommend 8.
2. `PIPE_ATOMIC = 512` (POSIX's minimum `PIPE_BUF`). Recommend 512.
