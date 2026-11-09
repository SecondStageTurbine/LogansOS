<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: L1a.1 step 2b, stdio chosen at spawn (2026-11-08)

**Status:** design, revision 2; Dexter's re-check (relay 325, `Plans/RETURN-2026-11-08-pr72-recheck-dexter.md` on `dexter/pr72-recheck`) PASSED, and its two notes are folded in. The PM answered §5 on #72 (all three as recommended). Dexter's design read (relay 322,
`Plans/RETURN-2026-11-08-pr72-design-dexter.md` on `dexter/pr72-design`) passed items 2 and 3 and failed 1, 4, 5 and 6.
Revision 2 answers each failure; §6 maps them. **Author:** Seat B. **Builds on:** step 2a, the kernel pipe (#56,
review-complete at `71bc2dca`, waiting to land), whose THINK is `docs/THINK-2026-11-02-l1a1-pipe-wiring.md` (§2b is
the outline this expands). Also builds on `docs/THINK-2026-10-29-ot88-app-model.md` §3.2 ("How the child gets its
stdio") and `docs/THINK-2026-10-31-l1a-terminal-window.md` §3.2-3.4 (the term page and the child's calls).

Citations are to `origin/main` at `13eed2c5`. `[2a]` marks code that exists only on #56, at its landing head `1e98d65e` (Chad's reviewed `71bc2dca` plus the merge of `main` at `8c3ac35`, which moved `KIND_PIPE` from 19 to 20).

## 1. What the code says

1. **fd 1 never looks at the handle table.** `sys_write_impl` sends every fd-1 write to serial and the framebuffer
   console (`kernel/src/arch/x86_64/syscall.rs:6555-6582`). `sys_open` never returns 1. It can return 0 when neither
   `serial-stdin` nor `usb-hid` is on (`RESERVED_FILE_FDS`, `:1574-1579`).
2. **fd 0 is input only under `serial-stdin` or `usb-hid`.** There it is `sys_read_stdin`, a yield loop over the
   keyboard and serial (`:6724-6733`, `:6762-6790`). Without those features, `SYS_READ(0)` falls through to
   `file_lookup_with_rights(0)`, which gives `EBADF` for anything but a file in slot 0.
3. **Spawn installs the inherited handles at 0..n, then makes the thread runnable**, all inside `spawn_user_body!`
   under `CAP_GRAPH` (`kernel/src/sched/process.rs:4300-4380`): install, then `register(process)`, then
   `try_push_ready(thread)`. So anything the child must see at its first instruction has to be on the `Process`
   before `try_push_ready`. A field set after `spawn_user` returns (the `obj_spawn` charge, `:4508-4513`) is too
   late for stdio: the child can already have written to fd 1.
4. **Spawn inherits a handle only if it carries `GRANT`** (`syscall.rs:1904-2083`, the `has_meta(GRANT)` arm). Pipe
   handles carry every meta right `[2a]` (`process.rs` `full_pipe_rights`), and buffers do too, so both can be named.
5. **`pipe_lookup` already returns the handle's `CapId`** `[2a]` (`syscall.rs:2175`), and `handle_info` already
   reports `KIND_PIPE = 20` `[2a]` (`handle_info_logic.rs:48`; 19 since #8 is `PowerControl`), with the end in `cap_semantic` (`READ` or `WRITE`;
   `flags` is 0 for a pipe).
6. **`neo-shell` already reads fd 0 a byte at a time and writes fd 1** (`userland/neo-shell/src/main.rs:1119-1155`).
   A read that returns neither 1 nor `EAGAIN` exits with code 2, so EOF ends the shell. Its `spawn` command gives the
   child a channel, pings it, prints the reply and waits for the exit (`:1157-1215`).
7. **std's stdin is `Unsupported`, and stdout and stderr are both fd 1**
   (`userland/neoplutarch-std-overlay/std/stdio_neoplutarch.rs:24-50`). std turns `EBADF` on a standard stream into
   success (`is_ebadf`), so std must not learn about stdin by trying a read that might fail with `EBADF`.
8. **`Handle::Buffer` and `SYS_BUFFER_*` exist only under `atrium-compositor-g1`** (`process.rs:968`,
   `syscall.rs:289-299`, `:8517`), and `pipe` doesn't imply it (`pipe = ["wait-multiplex"]` `[2a]`). So the term
   page, which is a buffer, needs both features.
9. **Scenarios run ring-3 test code** through the scenario's `user_feature` (`tools/xtask/src/main.rs:2202-2206`), and
   xtask can require a serial line to be absent (`require_serial_absent`, `:21586`).

## 2. The design

### 2.1 What the process holds

`Process` gains `#[cfg(feature = "pipe")] stdio: SwitchMutex<Stdio>` (the `obj_spawn` cfg-field pattern):

```rust
pub struct StdioRef { slot: u32, cap_id: CapId }
pub struct Stdio { stdin: Option<StdioRef>, stdout: Option<StdioRef>, term: Option<StdioRef> }
```

**A reference into the handle table, not an `Arc`.** The table stays the only owner. Each use resolves `slot` and
requires the handle there to have the same `CapId`, checked **first**, under the table lock. Only then does it check the
kind, the end and the right. That buys three things:
- **Close and revoke reach stdio.** `SYS_CLOSE` of the slot, or a parent's `SYS_REVOKE`, removes the handle. The
  next fd-1 write is then `EBADF`, and the endpoint count was released by the normal `Detached` path. A held `Arc`
  would need its own endpoint count, and a revoke would not reach it.
- **A reused slot can't redirect stdio** (CP-61's shape): a new handle in the same slot has a new `CapId`. Because the
  `CapId` is compared first, any reuse is `EBADF`: a read end, a buffer or a write end of another pipe alike, never
  `EPERM` (Dexter 322 item 4).
- **Teardown has nothing new to do.** `Stdio` holds no object, so the process exit path is unchanged (CP-85: N-A).

A stale reference never comes back to life, because `CapId`s come from a global monotonic counter
(`process.rs:99-108`). Once it is stale, that stdio is `EBADF` for good; it never falls back to the console (PM, §5.3:
a child's output must never escape its terminal).

**Lock order.** The fd path copies the `StdioRef` out and drops the `stdio` guard before it takes the handle table or
parks. So `stdio` is never held with another lock, and a sibling thread is never kept spinning on it by a parked reader
(Dexter 322 item 4 note). The spawn stores the field after the install block has dropped the child's `handles` lock
(§2.2).

### 2.2 The spawn call

A new number, because `SYS_SPAWN`'s dispatch passes only four arguments (`syscall.rs:1045-1054`): a fifth one is
ignored today, so giving it a meaning would change old binaries silently (the `SYS_SPAWN_BUDGETED2` precedent).

- **`SYS_SPAWN_STDIO = 0x1BD` `(name, len, handles, n, stdio)`**, with the same return as `SYS_SPAWN` (pid, and the
  parent's process-handle index, packed).
- **`stdio` is one word:**
  - byte 0 is the index **into `handles`** of the child's stdin;
  - byte 1 is the index of its stdout;
  - byte 2 is the index of its term page;
  - `0xFF` means none;
  - bits 24-63 must be 0.

  An index into `handles` is also the child's slot, because the child's table starts empty and spawn installs
  `handles[i]` at slot `i` (`process.rs:4318-4335`, asserted by its `debug_assert_eq!`).
- **All three none** (`0x00FF_FFFF`) is `Stdio::NONE`, so the child is the same as one spawned by `SYS_SPAWN`:
  `SYS_STDIO_INFO` reports no selection, with flags 0 (§2.4).
- **What each index must name.** The handle kinds are checked on the caller's handles, the same clones the spawn
  installs:

  | Role | Must be | Rights |
  |---|---|---|
  | stdin | a `Handle::Pipe` read end | `USE` and `READ` |
  | stdout | a `Handle::Pipe` write end | `USE` and `WRITE` |
  | term | a `Handle::Buffer` of exactly 4096 bytes | `USE` and `buffer_rights::MAP_READ \| MAP_WRITE` (`process.rs:316-325`); `PRESENT`, `COMMIT`, `QUERY` not needed |

  No two roles may name the same index. A term index in a kernel without `atrium-compositor-g1` is `EINVAL`.
- **The errors, in order** (the first that applies is returned):
  1. `SYS_SPAWN`'s own pre-checks, unchanged: name length (`EINVAL`), name buffer (`EFAULT`), `n` above
     `MAX_SPAWN_HANDLES` (`EINVAL`), handle-array buffer (`EFAULT`);
  2. the stdio word alone (`EINVAL`):
     - a bit set in 24-63;
     - an index that is neither `0xFF` nor below `n`;
     - two roles naming the same index;
     - a term index without `atrium-compositor-g1`;
  3. the name copy and UTF-8 check (`EINVAL`), and each handle's resolution as today (`EBADF` for an empty slot,
     `EPERM` for no `GRANT`);
  4. each role's kind, end, size and rights (`EINVAL`);
  5. the spawn's own outcomes, unchanged (`EAGAIN`, `EBADF` on `ParentGone`, `ENOENT`, under quotas `ENOMEM`).

  Step 2 is a pure function, `pipe_logic::decode_stdio_word(word, n) -> Result<StdioSel, ()>`, with its host KATs. All
  checks run before anything is built, so a refused spawn leaves no half-built child.
- **Where it is set:** `spawn_user_body!` gains a `$stdio` argument, a `StdioSel` of child slots. The install loop
  records the child-side `CapId` of each installed handle (each is `with_cap(alloc_cap_id())` at the clone) into a
  local `Stdio`. After the install block, so the `handles` lock is dropped, and **before `register(process)`**, the
  local is stored in `process.stdio`. So the child's first instruction already sees it. The plain spawns pass
  `StdioSel::NONE`.
- **No implicit inheritance (PM, §5.1).** A child spawned with `SYS_SPAWN` has no stdio selection, even if its parent
  has one. Every stdio is a named capability the parent passes on purpose. This is the one real difference from Unix.
  So 2b also changes `neo-shell`'s `spawn` command:
  - when the shell itself has a selected stdio (`SYS_STDIO_INFO`), it spawns the child with `SYS_SPAWN_STDIO`;
  - `handles = [chan, my stdin slot, my stdout slot]`, with the stdio word `0x_FF_02_01`: the channel in 0, stdin in 1,
    stdout in 2;
  - with no selection, it spawns as today.
- **Non-goal for 2b: budgeted spawns (PM, §5.2).** The terminal spawns `neo-shell` unbudgeted. `SYS_SPAWN_BUDGETED`
  and `SYS_SPAWN_BUDGETED2` gain a stdio word later, when a caller needs one.
- **Endpoint counts across the spawn** (Dexter 322 item 3: PASS).
  - Each inherited handle goes through `HandleTable::install`, which calls `retain_pipe` `[2a]`
    (`process.rs:2091-2095`). The child's copy is one more endpoint, and the parent's copy stays counted, so there is
    no early EOF while the parent still holds its end.
  - The child's exit drains its table through `Detached`, which releases its copies.
  - A spawn that fails after the installs (`SchedulerFull`) drops the unstarted `Process`, whose `HandleTable::drop`
    drains the table the same way (`process.rs:1673-1677` `[2a]`). So a failed spawn leaks no endpoint either.
  - The terminal closes its own copies of the child's ends after the spawn, so the child's copies are the only ones
    (terminal THINK §3.3).

### 2.3 fd 0 and fd 1

**fd 1 (`sys_write_impl`):** if `stdio.stdout` is `Some`, copy it out and resolve it (§2.1):
- `len == 0` returns 0 before any lookup, today's contract and POSIX's, never `EPIPE` (Dexter 322 item 4);
- otherwise the write is `SYS_PIPE_WRITE`'s blocking body on that handle: the same code, the CP-67 anchor, atomic up to
  512 bytes, and `EPIPE` when the terminal is gone;
- a stale reference is `EBADF`.

If `stdio.stdout` is `None`, it takes today's path, byte for byte.

**fd 0 (`sys_read_impl`):** the same with `stdio.stdin` and `SYS_PIPE_READ`'s blocking body. `len == 0` already
returns 0 at the top of `sys_read_impl` (`:6714-6716`). The check comes **before** the `serial-stdin`/`usb-hid` arm,
so a selected pipe wins over the keyboard. With no selection, today's paths are unchanged.

**Always blocking.** `SYS_READ` and `SYS_WRITE` carry no flags, so fd 0 and fd 1 always use the blocking bodies. A
child that wants a try or a deadline uses `SYS_PIPE_READ`/`SYS_PIPE_WRITE` with `PIPE_NONBLOCK` on its stdio slot, or
`SYS_WAIT_ANY` on it, which is how the terminal THINK's `read_timeout` works (§3.4 there).

**The lookup.** The fd path calls the pipe syscall's body with the slot and the expected `CapId`: `pipe_lookup` gains
an `expect: Option<CapId>` argument. When it is `Some`, it is compared before the kind and the rights, under the same
table lock as the lookup, so the check and the use can't straddle a close. A mismatch, an empty slot or a non-pipe is
`EBADF`.

**fd 2** is not a kernel fd. std and neo-std-pal send stderr to fd 1 (§1 item 7), so stderr is the stdout pipe, as on
a Unix tty (terminal THINK §3.4).

**The atomicity a child sees:** `SYS_WRITE(1)` of at most 512 bytes is one atomic pipe write. That matches POSIX for a
pipe on fd 1. Today's console path splits at `SYS_WRITE_CHUNK` and has no atomicity to keep.

### 2.4 How the child finds its stdio and term page

- **`SYS_STDIO_INFO = 0x1BE` `(out, len)`** writes 8 bytes:
  - the slots of stdin, stdout and term page as `u16` each (LE, `0xFFFF` for none);
  - then `u16` flags: bit 0 means a stdio was selected at spawn; bits 1-15 are 0.

  `len != 8` is `EINVAL`, and an `out` that fails `validate_user_buf` or faults in the copy is `EFAULT` (the
  `SYS_HANDLE_INFO` precedent, `syscall.rs:7836-7860`). `len` is checked first, so a call with both wrong is
  `EINVAL` (Dexter relay 325). It reads only the caller's own `Process`, needs no capability
  and grants nothing.

  **Identity:** a slot is reported only while it still holds the selected `CapId` (the check is under the table
  lock), else as `0xFFFF`. The flags still say a stdio was selected, so a closed stdout reads as "selected, now
  none", never as "no selection". The report is advisory: the fd calls and the term map below check the `CapId`
  themselves, so a slot that goes stale after the report can't be used through them.
- **`SYS_STDIO_TERM_MAP = 0x1BF` `(role)`** maps the selected term page and returns its address. It is
  `SYS_BUFFER_MAP`'s body on the `stdio.term` reference, with the `CapId` checked first under the table lock:
  - `role` is `SYS_BUFFER_MAP`'s role word;
  - its rights checks and errors are `SYS_BUFFER_MAP`'s;
  - a stale or absent term is `EBADF`;
  - it needs `atrium-compositor-g1`, else `ENOSYS`.

  So the child never maps a buffer that only happens to sit in the term page's old slot (Dexter 322 item 1, term
  identity). This takes the last free number in the pipe range (`0x1BA`-`0x1BF`).
- **Byte oracle** (hand-laid; a KAT in `tools/pipe-logic-tests` against `pipe_logic::encode_stdio_info`, and the same
  fixture against libneo's decoder in `userland/libneo/src/pipe_abi.rs`):

  | Case | Bytes |
  |---|---|
  | stdin in 0, stdout in 1, term page in 2, selected | `00 00 01 00 02 00 01 00` |
  | stdin in 0, stdout in 1, no term page | `00 00 01 00 ff ff 01 00` |
  | selected, stdout since closed (stdin 0, term 2) | `00 00 ff ff 02 00 01 00` |
  | no selection, or all three none | `ff ff ff ff ff ff 00 00` |

- **SDK-ABI:** three rows (445 / `0x1BD`, 446 / `0x1BE`, 447 / `0x1BF`) in the Pipes section, with their arguments,
  returns, errors (in §2.2's order for the spawn) and features. The range note's "0x1BD-0x1BF empty" becomes
  "populated". The xtask drift guard `sdk_abi_doc_covers_every_kernel_syscall` fails without them.
- **What 2b ships for userland:** the libneo wrappers `sys_spawn_stdio`, `sys_stdio_info` (returning a decoded
  `StdioInfo`) and `sys_stdio_term_map`, the `neo-shell` change (§2.2), and the scenario's test code (§4).
- **Later, in L1a.2/L1a.3, not 2b** (recorded here so the design is whole):
  - the std overlay's `HandleKind::Pipe`, whose end is read from `cap_semantic`;
  - `HandleKind::TermPage`, a std-side kind for the slot `SYS_STDIO_INFO` names;
  - std's stdin reading fd 0 when a stdin is selected, and staying `Unsupported` otherwise;
  - `IsTerminal`, `libneo::term` and the std extension.

  One rule for that work: std's `is_ebadf` must not turn a **selected** stdout's `EBADF` into success. Otherwise the
  PM's "a closed stdout stays `EBADF`" would be invisible to std programs. So `is_ebadf` is true only when
  `SYS_STDIO_INFO` reports no selection (Dexter 322 item 4 note).

### 2.5 Features and kill-switch

- **`pipe` gates all of 2b:**
  - the `stdio` field;
  - `SYS_SPAWN_STDIO` and `SYS_STDIO_INFO`;
  - the fd-0/1 arms;
  - the `neo-shell` change. The shell asks `SYS_STDIO_INFO`; `ENOSYS` means no selection.
- **The term page also needs `atrium-compositor-g1`:** the term role, `SYS_STDIO_TERM_MAP` and the term field of
  `SYS_STDIO_INFO`, which is always `0xFFFF` without it.

With `pipe` off, the numbers are `ENOSYS` (the dispatch's `_` arm), and nothing else changes. With `pipe` on, a process
spawned without `SYS_SPAWN_STDIO` holds `Stdio::NONE`, and its fd 0/1 take the existing paths after one `None` check.

**The two legs** (Dexter 322 item 5):
- **Off:** every scenario except 2a's two and `pipe_stdio` builds with `pipe` off, so the off leg is the whole
  existing gate.
- **On, nothing selected:** `pipe_stdio` leg 4.

## 3. Pitfall bank (`docs/concurrency-pitfalls.md`)

| Class | Does 2b introduce it? | Guard | Discriminator |
|---|---|---|---|
| **CP-14** lost wake | No new park: fd 0/1 use 2a's park | 2a's single critical section | 2a's cp14 leg; `pipe_stdio` leg 1 parks the shell on fd 0 before the first byte |
| **CP-33** kill completeness | Yes, a new caller of the token park | The same token park, which `terminate` finds by pid | `pipe_stdio` leg 3: kill the shell while it is parked in fd-0 `read_line`; it reaps, and both pipes' far ends see the endpoints go (stdout EOF, stdin `EPIPE`) |
| **CP-61** reused slot | Yes: stdio names a slot (fd 0/1 and the term page) | `StdioRef`'s `CapId`, compared first under the table lock (`pipe_lookup`'s `expect`; the term map's check) | `pipe_stdio` leg 5: closed slot, a read end reused into it, a write end reused into it: `SYS_WRITE(1)` is −9 each time and nothing reaches the other pipe. Leg 9: a reused term slot is `EBADF` to `SYS_STDIO_TERM_MAP` |
| **CP-67** `Arc` across a park | Same as 2a | `with_pipe_anchor`: the fd path calls the same body | 2a's `pipe_park_kill` `Arc` count, on the same body |
| **CP-85** exit retiring one thread | N-A: `Stdio` holds no object | Handles leave through `Detached` | Leg 2's EOF after the shell's exit |
| **CP-87** revoke vs spawn | Yes: the selection is checked against handles being cloned | The checks run on the same clones that the `CAP_GRAPH` liveness check covers; a revoked parent fails the spawn (`ParentGone`) | Covered by the existing K1a-0 discriminator; no new window |
| **CP-112** child runs before its stdio is set (new, added to the bank) | Yes: the new field | Set before `register` and `try_push_ready` (§1 item 3) | Leg 1 (the prompt arrives in the pipe and never on serial), and the late-control below |

I-14 does not apply: there is no ISR path and no governance lock.

## 4. Gate and review

**Scenario `pipe_stdio`:**
- `neo-test-pipe-stdio = ["neo-test", "pipe", "atrium-compositor-g1", "neo-test-atrium-runtime", "close-object-handles"]`
  (the buffer syscalls need the Atrium runtime switch, and closing a buffer needs `close-object-handles`), with `user_feature: Some("pipe-stdio-test")`.
- **The terminal side is real ring-3 code:** `neo-init`'s `pipe-stdio-test` arm makes the real syscalls and blocks
  like a terminal would. It needs no kernel stand-in and no 5-argument test dispatch (Dexter 322 item 6).
- Each wait is a `PIPE_NONBLOCK` poll bounded by the TSC (2^35 cycles, seconds on QEMU's TCG clock): this kernel has
  no clock syscall to make a `SYS_WAIT_ANY` deadline from. So a missing byte is a named failure, not a hang.
- It reports each leg as a `PIPE_STDIO <leg> ...` serial line from its own console fd 1. xtask checks them in order
  with `require_serial_sequence`.
- **Two children:**
  - the real `neo-shell`;
  - the probe: `neo-worker`'s `pipe-stdio-test` role (`userland/neo-worker/src/stdio_probe.rs`), spawned as
    `worker`. It is a role, not a new module, so the image needs no new staging. With its stdin in slot 0 it reads one
    command byte from fd 0. With its stdin in slot 1 (`neo-shell`'s `spawn` put the channel in 0) it acts as the
    shell's child (leg 8). With no selection it prints its `SYS_STDIO_INFO` record on the console (leg 7).
- **The order:** the legs below run 1, 2, 3, 5, 6, 7, 8, 9, then 4. The unselected spawn runs last, so its marker
  splits the log: before it, no pipe child's output may reach serial.

The legs:
1. **Round trip:**
   - create a 4 KiB stdin pipe and a 64 KiB stdout pipe;
   - `SYS_SPAWN_STDIO("shell", [stdin rd, stdout wr], 0x_FF_01_00)`, then close its own copies of those two ends;
   - read the prompt `neo> ` from the stdout pipe;
   - write `echo pipe-ok\n`;
   - read until `pipe-ok\n` arrives after the echoed command line.

   xtask checks that none of `neo> `, `[ neo-shell ] ready`, `pipe-ok` and `probe-line` reach serial before the
   leg-4 marker (`require_pipe_stdio_prefix_quiet`).
2. **EOF:** close the stdin write end. The shell's `read_line` gets EOF, the shell exits with code 2 (`SYS_WAIT`), and
   the stdout pipe reads EOF once the bytes still buffered are delivered.
3. **Kill while parked:**
   - spawn the shell again the same way, read its prompt, and wait until `SYS_TEST_PIPE_PARKED` reports it token-parked
     on its stdin pipe (the prompt alone is not a park);
   - `SYS_KILL` it, and `SYS_WAIT` finds it gone (`Ok`, or `ESRCH` once the kill has finished: a killed process keeps
     no exit code);
   - the stdout read end then reads EOF, and a write to stdin is `EPIPE`, so both of its endpoints were released.
4. **No selection:**
   - print the leg-4 marker;
   - `SYS_SPAWN("shell", [chan])`, as today, and wait for it;
   - xtask requires `neo> ` on serial after the marker. The shell's fd-0 read is `EBADF` with no selection and no
     `serial-stdin`, so it exits with code 2 after its prompt.
5. **Stale stdout slot.** Spawn the probe with `[stdin rd, stdout wr]` and the word `0x_FF_01_00`, and send it `s`.
   Each case's result goes in the probe's exit code (its stdout is gone). In the order run:
   - **(c) a write end reused into it:**
     - create pipe P2 first (ends in 2 and 3);
     - close 1, then `SYS_DERIVE` slot 3, which lands in slot 1, the lowest free;
     - `SYS_WRITE(1)` is −9, and a `PIPE_NONBLOCK` read of slot 2 is `EAGAIN`.
   - **(a) closed:** close slot 1; `SYS_WRITE(1, "x")` is −9.
   - **(b) a read end reused into it:** create a pipe; its read end lands in slot 1. `SYS_WRITE(1)` is −9, not −1,
     because the `CapId` is checked first.
6. **Refusals:** each of these 16 cases is −22 from `SYS_SPAWN_STDIO`, and no child is created (no process handle is
   installed, so the caller's lowest free slot is the same after the cases as before):
   - stdin naming a write end, a channel or a buffer;
   - stdout naming a read end, a channel or a buffer;
   - term naming a pipe;
   - term pages of 4092 and 8192 bytes (a buffer's size is `stride × height` in 4-byte pixels, so 4095 can't exist);
   - a term without `MAP_WRITE` (derived down to `MAP_READ`);
   - the same index for two roles (stdin and stdout; stdout and the term page);
   - an index ≥ `n` (two cases);
   - bit 24 set, and bit 63 set.
7. **`SYS_STDIO_INFO`:**
   - the probe, sent `i`, writes its 8 bytes to fd 1, and the terminal compares them with the selected-with-term row;
   - the terminal's own call (no selection) gives the no-selection row;
   - `len` 7 is −22, and `out = 1` is −14;
   - an all-none spawn (`0x00FF_FFFF`) of the probe gives the no-selection row: the probe prints
     `PIPE_STDIO probe_unselected info=ffffffffffff0000` on its console, and xtask pins that line.
8. **The shell names its pipes for its child (§2.2):**
   - start a leg-1 shell and type `spawn worker\n`;
   - the probe sees a channel in slot 0, takes the ping, writes `probe-line` to its fd 1, replies and exits;
   - `probe-line` arrives in the terminal's stdout pipe, followed by the shell's relay and wait lines, and never on
     serial.
9. **The term page:**
   - the terminal writes the `NTP1` magic and the 80 × 25 size into the buffer before the spawn (terminal THINK §3.2);
   - the probe, sent `t`, maps it with `SYS_STDIO_TERM_MAP` and writes back the magic and the size it read;
   - sent `T`, it closes slot 2, creates a 4096-byte buffer (which lands in slot 2), and `SYS_STDIO_TERM_MAP` is −9.

**Control: `neo-test-pipe-stdio-late-control`.** `SYS_SPAWN_STDIO` spawns with no selection and stores it only
after the spawn returns, the `obj_spawn` shape that §1 item 3 calls too late. The witness:
- before the spawn, the control arms a recorder of the first syscall made by any process other than the spawner;
- after it, the spawn yields (IF on, 100 ticks, 1 s, at most) until that first syscall is the child's;
- a spawn whose window never opened counts a miss.

The two outcomes:
- the window was hit: the shell's first prompt goes to serial, leg 1's read of the prompt times out, and the terminal
  reports `pipe_stdio_prompt_not_in_pipe`;
- the window was not hit: the harness fails the scenario with `pipe_stdio_late_window_not_hit`. Without this, a
  missed window would let the late store land before the child's first write, and the control would pass.

Either way the control fails, and the named reason says which one (Dexter 322 item 6).

**Host KATs** (`tools/pipe-logic-tests`):
- `decode_stdio_word` with every step-2 refusal, plus all-none;
- the `SYS_STDIO_INFO` oracle's four rows against both the kernel's encoder and libneo's decoder.

**xtask:**
- the `TestScenario` entry, beside 2a's, with `user_feature`;
- `require_serial_sequence` for the `PIPE_STDIO` lines, ending in
  `PIPE_STDIO pass legs=round,eof,kill,stale,refusals,info,shell_child,term,unselected`;
- `require_pipe_stdio_prefix_quiet`: before the leg-4 marker, no `neo> `, no `[ neo-shell ] ready`, no `pipe-ok` and
  no `probe-line` on serial;
- the control's `NEO_TEST_EXTRA_FEATURES` row, which must fail with one of its two reasons;
- gate-plan: no new kernel file (the scenario hooks are in `test_harness.rs`, already core).

**Gate tier:** `full-alone`, since it touches spawn (`kernel/src/sched/process.rs`). **Review:** spawn and thread park
are SMP-core, so the review needs a cross-family lens: GPT-6.1-Sol, high.

## 5. Questions for the PM (answered on #72, 2026-11-08)

1. **Inheritance.** **Answered: none implicit.** Every stdio is a named capability; `neo-shell`'s `spawn` names its
   pipes (§2.2).
2. **Budgeted spawns.** **Answered: later**, when a caller needs one; a non-goal for 2b (§2.2).
3. **A closed stdout.** **Answered: `EBADF` for good**, no console fallback (§2.1).

## 6. Dexter's relay-322 findings and where revision 2 answers them

| Item | Finding | Answer |
|---|---|---|
| 1 | The term page is a `Buffer`, gated on `atrium-compositor-g1` | §1 item 8, §2.2 (a term index without it is `EINVAL`), §2.5; the scenario enables it (§4) |
| 1 | No SDK-ABI rows | §2.4, SDK-ABI: three rows, plus the range note |
| 1 | `SYS_STDIO_INFO`'s errors | §2.4: `len != 8` is `EINVAL`, a bad `out` is `EFAULT`, flag bits 1-15 are 0; leg 7 |
| 1 | All-none is ambiguous | §2.2 and §2.4: it is `Stdio::NONE`, flags 0; oracle row 4; leg 7 |
| 1 | The term page has no `CapId` check | §2.4: `SYS_STDIO_TERM_MAP` checks it, and `SYS_STDIO_INFO` reports only live slots; leg 9 |
| 1 | The refusals don't cover every check | §4 leg 6 covers each role's wrong kinds, both sizes and the missing `MAP_WRITE` |
| 1 (note) | Error precedence | §2.2, "The errors, in order" |
| 1 (note) | Which buffer rights | §2.2's table: `MAP_READ \| MAP_WRITE` |
| 4 | The pipe's end is not in the flags byte | §1 item 5 and §2.4: the end is `cap_semantic` |
| 4 | A wrong-end reuse gives `EPERM`, not `EBADF` | §2.1 and §2.3: the `CapId` is compared first; leg 5(b) |
| 4 | A zero-length `SYS_WRITE(1)` changes contract | §2.3: `len == 0` is 0 before any lookup |
| 4 (notes) | `sys_open` and fd 0; std's `is_ebadf`; the 2b scope; the guard across the park | §1 item 1; §2.4's rule for the later std work; §2.4's "later" list; §2.1 "Lock order" |
| 4 (note) | fd 0/1 are always blocking | §2.3, "Always blocking" |
| 2 (note) | `handles` → `stdio` lock order at the spawn | §2.2: the field is stored after the install block, before `register` |
| 5 | Only the term page breaks the `pipe`-only claim | §2.5 |
| 5 | The existing gate is the off leg, not the on leg | §2.5, "The two legs" |
| 6 | No serial capture | Ring-3 terminal code reports on serial; xtask's `require_serial_absent` checks the prefix before the leg-4 marker (§4) |
| 6 | No 5-argument test dispatch; the stand-in makes only one call | The terminal side is ring-3 (`neo-init`'s `pipe-stdio-test`), with real syscalls (§4) |
| 6 | The late-control doesn't prove its window | A first-syscall witness, and `pipe_stdio_late_window_not_hit` (§4) |
| 6 | Leg 5 puts a read end, not a write end, in slot 1 | Leg 5 has three cases; (c) uses `SYS_DERIVE` into the freed slot |
| 6 | No positive term-page leg | Leg 9 |
| 6 (notes) | The xtask registration; naming the modules and their handle layouts | §4: the xtask list, the probe (`neo-worker`'s `pipe-stdio-test` role), and each leg's handles |

## 7. As built (revision 3)

The code follows revisions 1-2 except where this table says, each a detail the build found:

| Where | Revision 2 said | Built | Why |
|---|---|---|---|
| §4 probe | a new module `neo-stdio-probe` | `neo-worker`'s `pipe-stdio-test` role, spawned as `worker` | the scenario-role pattern; a new module needs new image staging |
| §4 waits | `SYS_WAIT_ANY` with a 5 s deadline | TSC-bounded `PIPE_NONBLOCK` polls | the scenario kernel has no clock syscall to form an absolute deadline |
| §4 feature | `neo-test`, `pipe`, `atrium-compositor-g1` | plus `neo-test-atrium-runtime` | the buffer syscalls are `ENOSYS` without the runtime switch |
| §4 feature | (none) | plus `close-object-handles` | leg 9's `T` closes the term page's buffer with `SYS_CLOSE`, which refuses a buffer without it |
| §4 leg 3 | `SYS_WAIT` reaps the killed shell | `SYS_WAIT` is `Ok` or `ESRCH` | a killed process keeps no exit code, so a wait after the kill finished is `ESRCH` (first boot, run 42475546797) |
| §4 leg 3 | the prompt shows the shell is parked on fd 0 | the leg waits for `SYS_TEST_PIPE_PARKED` (0xF0B, `pipe_stdio` kernels only) to report the shell token-parked on its stdin pipe before the kill (`leg_kill:park_not_seen` otherwise) | the shell writes its prompt before it reads fd 0, so the prompt alone is not a park (Janus #72 F2) |
| §4 leg 6 | a 4095-byte term page | 4092 bytes | a buffer's size is `stride × height` in 4-byte pixels |
| §4 leg 6 | an 8192-byte term page | 1024 × 2 pixels | a buffer may be no wider than the framebuffer (`ExceedsOutput`); a 2048 × 1 one was refused at creation (run 42485247696) |
| §4 leg 7 | all-none gives the no-selection row | the probe prints the row on the console; xtask pins it | with no selection its fd 1 is the console |
| §4 order | legs 1-9 in number order | leg 4 last | its marker splits the log for the quiet-prefix check |
| §4 control | a per-pid counter in the spawn path | a first-other-syscall recorder, a yield after the spawn, and a named failure on a miss | a missed window would otherwise let the control pass |
