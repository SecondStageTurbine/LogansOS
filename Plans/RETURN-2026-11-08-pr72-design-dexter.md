<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 322: #72 design read (stdio chosen at spawn)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relay-322.md` §322.
**Reviewed:** PR #72 head `43c2593b13dfb2a7f6675d32b1272377801c10b4` (`docs/THINK-2026-11-08-l1a1-stdio-selection.md`). The
dispatch queued `b8bd99c4`; the head I fetched is `43c2593b1`, the commit that folds in the PM's §5 answers. Code at
`origin/main` `42ccc9238` (the design cites `13eed2c5`; `13eed2c5..42ccc9238` is `Plans/` only) and at #56
`71bc2dca` for the `[2a]` code. Branch `dexter/pr72-design`.

**Overall: FAIL.** Items 2 and 3 pass, and the core shape — a slot + `CapId` reference, set before
`register`/`try_push_ready`, with the handle table as the only endpoint owner — is right and should be kept. Items 1,
4, 5 and 6 have gaps that must be fixed before the code: the term page is a `Buffer`, which only exists under
`atrium-compositor-g1`, while 2b is stated as `pipe`-only; the two new syscalls have no SDK-ABI rows (the xtask drift
guard will red the host gate); `SYS_STDIO_INFO`'s error, all-none and term-identity cases are undefined; the
`HandleKind::Pipe` "end in the flags byte" claim is false; and the `pipe_stdio` scenario cannot be implemented as
written (no serial capture, no 5-argument test dispatch, a one-shot stand-in, a racy late-control, and no positive
term-page leg).

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing refs |
|---|---|---|---|
| 1 | `SYS_SPAWN_STDIO` / `SYS_STDIO_INFO`: arguments, rights on each handle, every error case, the ABI oracle | **FAIL** | THINK:63-78, 123-134; #56 `kernel/src/sched/process.rs:987`; #56 `kernel/src/arch/x86_64/syscall.rs:288-289`; #56 `docs/SDK-ABI.md:354-363,504`; `tools/xtask/src/main.rs:26029` |
| 2 | The selection is set on the child before it can run (before `try_push_ready`) | **PASS** | THINK:79-82; #56 `kernel/src/sched/process.rs:4397-4463` |
| 3 | Endpoint counting: the child's copy is retained, its exit releases it, no leak or early EOF | **PASS** | THINK:92-98; #56 `kernel/src/sched/process.rs:2091-2107,1840-1846,1673-1687,2278-2295,3248` |
| 4 | fd 0/1 with a pipe vs the console: routing, `EBADF`, blocking/non-blocking | **FAIL (narrow)** | THINK:102-119, 135-146; #56 `kernel/src/sched/process.rs:1200-1213`; #56 `kernel/src/arch/x86_64/syscall.rs:2189-2201,2341-2346`; `kernel/src/arch/x86_64/syscall.rs:1573-1579,6555-6582`; `userland/neoplutarch-std-overlay/std/stdio_neoplutarch.rs:85-87` |
| 5 | Kill-switch: `pipe` off gives today's spawn and console stdio, unchanged | **FAIL** | THINK:148-154; #56 `kernel/Cargo.toml:551`; #56 `docs/THINK-2026-11-02-l1a1-pipe-wiring.md:128` |
| 6 | The CP table (CP-14/33/61/67) and the `pipe_stdio` / late-control discriminators | **FAIL** | THINK:156-206; #56 `kernel/src/test_harness_pipe.rs:29-41,843-849`; #56 `kernel/src/arch/x86_64/syscall.rs:3733-3742,2240-2256`; #56 `kernel/src/drivers/console/serial.rs:223-238` |

## Item 1 — FAIL: the ABI is incomplete, and the term page is gated on `atrium`

**What is right.** The numbers are free and adjacent to #56's `0x1BA`–`0x1BC`: `0x1BD`/`0x1BE` (THINK:63-64, 123). The
new number rather than a fifth-argument meaning is right: `SYS_SPAWN`'s dispatch passes only `a0`–`a3`
(`kernel/src/arch/x86_64/syscall.rs:1045-1054`), so a non-zero fifth argument is ignored today. The stdin/stdout
rights are consistent with #56: `full_pipe_rights` gives a read end `READ`, a write end `WRITE`, and full meta
(`process.rs:240-246`); `pipe_lookup` requires `USE` plus the end's right (`syscall.rs:2189-2201`). The
index-equals-child-slot claim is correct: the child's table starts empty and `install` takes the lowest free slot
(`process.rs:4397-4418`, with the `debug_assert_eq!` at `:4413-4417`). The three stated oracle cases are
self-consistent (THINK:127-134).

**FAIL — the term page is a `Buffer`, and `Buffer` is `atrium-compositor-g1`-gated.** The term row (THINK:77) is "a
`Handle::Buffer` of exactly 4096 bytes", and the std-side kind (THINK:137) is "a `Buffer`". But `Handle::Buffer` is
`#[cfg(feature = "atrium-compositor-g1")]` (#56 `kernel/src/sched/process.rs:987`), and so is `SYS_BUFFER_CREATE`
(#56 `kernel/src/arch/x86_64/syscall.rs:288-289`). `pipe = ["wait-multiplex"]` (`kernel/Cargo.toml:551`) does not
imply atrium. The design says "all of 2b is under `pipe`" (THINK:150) and proposes
`neo-test-pipe-stdio = ["neo-test", "pipe"]` (THINK:172), so the 2b gate's kernel has no `Buffer` at all: the
term-page arm, the `SYS_STDIO_INFO` term field and the "a term page of 8 KiB" refusal (THINK:191) cannot compile or
run. Concrete input: `cargo build -p neoplutarch-kernel --features neo-test,pipe` (the scenario's own feature set)
fails on `Handle::Buffer`. Fix: either add `atrium-compositor-g1` to the scenario feature and state that the term
page is available only with the Atrium buffer (and that a `pipe`-only kernel reports term `0xFFFF`), or give the term
page its own `pipe`-gated handle kind rather than reusing `Handle::Buffer`. The first is the smaller change; the
terminal product image already has Atrium, but the 2b gate must say so.

**FAIL — no SDK-ABI rows.** The design adds two non-test `pub const SYS_*` but never says to add the two inventory
rows. `docs/SDK-ABI.md`'s drift guard (`tools/xtask/src/main.rs:26029`,
`tests::sdk_abi_doc_covers_every_kernel_syscall`) parses every non-test `pub const SYS_*` and fails a missing row. On
#56 the pipe rows are `docs/SDK-ABI.md:354-363`, and the range note at `:504` says `0x1BD–0x1BF empty`; both need
updating (rows `445/0x1BD` and `446/0x1BE`, plus the argument/errno/feature columns). Concrete input:
`cargo test -p xtask sdk_abi_doc_covers_every_kernel_syscall` on the 2b branch — red until the rows exist. This is a
host-check gate, not a style point.

**FAIL — `SYS_STDIO_INFO`'s error cases are undefined.** The design says `(out, 8)` and "writes 8 bytes" (THINK:123)
but never states `len != 8` or a bad `out`. The `SYS_HANDLE_INFO` precedent is explicit: `len != RECORD_LEN` →
`EINVAL`, a bad buffer → `EFAULT` (`kernel/src/arch/x86_64/syscall.rs:7836-7845`). Concrete inputs:
`SYS_STDIO_INFO(out, 7)` and `SYS_STDIO_INFO(0x1, 8)` have no stated result; the host oracle cannot pin them. Add
"`len != 8` → `EINVAL`; invalid `out` → `EFAULT`; reserved flag bits 1–15 are 0".

**FAIL — the all-none selection is ambiguous.** `SYS_SPAWN_STDIO(..., stdio = 0x00FF_FFFF)` names no stdin, no stdout
and no term (THINK:69), which is legal. Is `SYS_STDIO_INFO`'s flags bit 0 then 1 ("selected at spawn") or 0? The
oracle's no-selection row `ff ff ff ff ff ff 00 00` is described for "a process with no selection" (THINK:134), but
the all-none-via-`SYS_SPAWN_STDIO` case is not in the oracle. Define it as `Stdio::NONE` with flags 0, so
`0x00FF_FFFF` is byte-identical to `SYS_SPAWN` for the child, and add that oracle row.

**FAIL — the term page has no `CapId` check on the userland path.** `SYS_STDIO_INFO` reports a *slot* and
`SYS_HANDLE_INFO`'s 16-byte record has no `CapId` (`kernel/src/sched/handle_info_logic.rs:54-68`), so a std-side
consumer that maps the term page by slot cannot tell whether the Buffer there is still the one selected at spawn. The
`StdioRef` `CapId` protects fd 0/1 because the fd path re-resolves it (`THINK:111-113`); the term page has no fd path.
Concrete input: spawn with a term Buffer at slot 2; close slot 2; `SYS_BUFFER_CREATE` a 4096-byte buffer (it lands at
2); the std overlay sees a `Buffer` at slot 2 and maps it as the term page, though it is a different `CapId`. The
design's CP-61 claim (THINK:52) is true for fd 0/1 and not for the term page. Fix: report the selected `CapId` (or
another stable identity) in `SYS_STDIO_INFO`, add a kernel resolve/map call for the term page that checks the
`StdioRef` `CapId`, or derive a stable term handle at spawn before the child can run and report that slot.

**FAIL — the planned refusals do not cover every check.** The checks are stdin read end, stdout write end, term
4096-byte Buffer with read+write, and no duplicate index (THINK:74-78). The refusal leg covers stdin-as-write-end,
stdout-as-channel, an 8 KiB term, the same index twice, an index ≥ n, and bit 24 (THINK:188-194). It does not cover:
stdin naming a Buffer or a Channel (wrong kind), stdout naming a Buffer, term naming a Pipe, a term with `MAP_READ`
but no `MAP_WRITE`, or a 4095-byte term. The host decoder KAT cannot cover the handle-kind checks; only the scenario
can. Add those inputs to leg 6 (or a separate leg).

**Note — error precedence.** The stdio checks are "before anything is built" (THINK:74), but the existing spawn
checks (name length/UTF-8, handle-array pointer, `GRANT`, `ParentGone`) run too. The ABI row should say the order, or
the single-bad-input outcome for each, so a caller with both a bad name and a bad stdio gets a defined errno.

**Note — the term "read and write rights" should name the bits.** A `Handle::Buffer`'s semantic rights are
`atrium::logic::buffer_rights::{MAP_READ, MAP_WRITE}` (`kernel/src/sched/process.rs:316-325`), not the generic
`READ`/`WRITE`. State `MAP_READ|MAP_WRITE`; `PRESENT`/`COMMIT`/`QUERY` are not required.

## Item 2 — PASS

The ordering is exactly as the design says: in `spawn_user_body!` the inherited handles are installed at `0..n`
(`process.rs:4397-4418`), then `register(process)` (`:4421`), then `try_push_ready(thread)` (`:4454`); the child is
not runnable before the push. The field can be set after the installs (it needs the child-side `CapId`s) and before
`register`; `register` consumes `Process`, so the store must precede it. The "`obj_spawn` is too late" comparison is
right: that field is set after `spawn_user` returns (the design's `:4508-4513`; at #56
`process.rs:4588-4593`), by which point the child can already run.

One note, not a fail: if the store takes the child's `stdio` lock while still holding the child's `handles` lock
(inside the install block), the order is `handles → stdio`; the fd path is `stdio → handles`. The child is
unregistered and unrunnable at that moment, so there is no live inversion, but capture the `CapId`s in a local and
set the field after the install block to keep the order acyclic. This also avoids a future lockdep entry.

## Item 3 — PASS

The counts are as claimed. `HandleTable::install` calls `retain_pipe` (`process.rs:2091-2107`); `Detached::release`
calls `release_pipe` (`:1840-1846`); the table's `Drop` drains (`:1673-1687`); the normal exit path drains through
`cap_graph.teardown` (`:3248`); `drain` fires the removed handles' keys (`:2278-2295`). The child's handle is a
separate endpoint, so the parent's copy keeps the pipe from EOF until the child exits. The failed-spawn path
(`try_push_ready` → `unregister_unstarted`, `Thread::free`, `proc_arc` drop) drops the unstarted `Process`, whose
`HandleTable::drop` releases the same way. `Stdio` holds no `Arc`, so it adds no count. The `pipe_stdio` leg 3 can
observe the count with `PipeObject::counts_for_test` and `Arc::strong_count` (`test_harness_pipe.rs:428-429,894-905`).

## Item 4 — FAIL (narrow): the routing shape is right; three ABI details are not

**PASS on the core.** fd1's selected path is before the `fd != STDOUT_FD` console branch
(`syscall.rs:6563-6565`), and fd0's selected path is before the `serial-stdin`/`usb-hid` arm (`:6723-6733`); the pipe
bodies are the same ones #56 tested (`syscall.rs:2271-2376`). A closed/reused slot's `pipe_lookup` returns `EBADF`
for an empty or non-pipe slot (`:2188,2201`), which is the "no console fallback" PM answer. The fd path uses the
blocking bodies; `SYS_READ`/`SYS_WRITE` carry no flags, so fd 0/1 are always blocking, which matches #56's default
(the only non-blocking path is `PIPE_NONBLOCK` on `SYS_PIPE_READ`/`SYS_PIPE_WRITE`, which the fd path cannot express).
That is worth one sentence in the THINK so it is not read as an oversight.

**FAIL — the `HandleKind::Pipe` end is not in the flags byte.** THINK:136 says "with the end in the record's flags
byte". #56's `Handle::handle_info` sets `flags = FLAG_ROOT_DIR` for a VFS root, else `0` (`process.rs:1200-1213`); the
pipe end is already in `cap_semantic` (`READ`/`WRITE`, from `full_pipe_rights`, `process.rs:240-246`), and
`file_rights` is `None` for a pipe. Concrete input: `SYS_HANDLE_INFO` on a pipe read end returns `kind=19,
cap_semantic=0x01, flags=0`; a std overlay that reads the end from `flags` sees `flags=0` for both ends and cannot
tell them apart. Fix the THINK to say "the end is the `cap_semantic` READ/WRITE bit" (no kernel change), or add a
`FLAG_PIPE_WRITE` and say so; do not leave the false claim.

**FAIL — `EBADF` on a stale slot is not guaranteed for a wrong-end reuse.** `pipe_lookup` checks `USE` and the needed
semantic right, then returns `EPERM` for any other `Handle::Pipe` (`syscall.rs:2189-2201`). If the child closes its
stdout slot and a new pipe's **read** end lands there, a design that checks `need` before `expect` returns `EPERM`
(-1), not `EBADF` (-9). The design's "`EBADF` for good" (THINK:55-56) and CP-61 row (THINK:162) require the
`expect: Option<CapId>` check to run **before** the kind/right check, under the same lock. State that order; leg 5
currently installs a **write** end, which passes the `need` check and hides the difference. Concrete input: close slot
1, `SYS_PIPE_CREATE` a second pipe (its read end lands at 1), `SYS_WRITE(1, "x", 1)` — expected -9, with `need`
checked first it gives -1.

**FAIL — zero-length `SYS_WRITE(1)` changes contract.** Today `sys_write_impl` returns 0 for `len == 0` (the loop
does not run, `syscall.rs:6567-6582`). #56's `SYS_PIPE_WRITE` returns `EPIPE` for `len == 0` with no reader
(`try_write(&[])`, `syscall.rs:2341-2346`). If fd1 delegates unconditionally, `SYS_WRITE(1, buf, 0)` on a selected
stdout whose terminal is gone changes from 0 to -32. POSIX `write(2)` with count 0 returns 0. Decide: preserve 0 for
`len == 0` before the pipe body (cleaner), or state the change and add it to the oracle. Concrete input: selected
stdout, reader closed, `SYS_WRITE(1, buf, 0)`.

**Note — `sys_open` never returns 0 or 1 is not true.** THINK:14-15 says "`sys_open` never returns 0 or 1".
`RESERVED_FILE_FDS` is `[0,1]` only under `serial-stdin`/`usb-hid`; otherwise it is `[1]`
(`syscall.rs:1573-1579`), so fd 0 is available to `SYS_OPEN` in a `pipe`-only build. The next sentence (THINK:16-18)
correctly says fd 0 can be a file in slot 0; the two sentences contradict. Fix the first: "`sys_open` never returns
1; it can return 0 when neither `serial-stdin` nor `usb-hid` is on." The routing design is unaffected.

**Note — std's `is_ebadf` can swallow the PM's EBADF.** `is_ebadf` returns true only for EBADF
(`stdio_neoplutarch.rs:85-87`), and std uses it to turn a standard-stream EBADF into success. A selected stdout that
is closed returns `EBADF` from the kernel, but a std program's `write` can still see success. The PM's "a closed
stdout stays EBADF" is a kernel contract; if it is meant to be observable through std, the overlay needs a rule (e.g.
`is_ebadf` only when `SYS_STDIO_INFO` says no selection). 2b does not ship the overlay, so this is a note for
L1a.2/L1a.3, not a 2b blocker — but it should be written down now.

**Note — the 2b scope boundary.** THINK:135-146 describes the std overlay's `HandleKind::Pipe`/`TermPage` and std's
stdin, then THINK:145-146 says 2b ships only the kernel calls and the libneo wrappers. The last is the right scope;
label the overlay bullets as L1a.2/L1a.3, or the build will try to land the overlay in 2b.

**Note — copy the `StdioRef` out before the blocking body.** `Stdio` is a `SwitchMutex` (THINK:39); if the fd path
holds the guard across the pipe body's park, the switch-mutex releases the token at the park
(`scheduler.rs:221-227`), so it is not a deadlock, but a sibling thread on the same process spins on the stdio lock
until the parker wakes (and, on a uniprocessor with IF=0, the reader cannot run until the spinner parks). Copy the
`StdioRef` out and drop the guard before calling the pipe body; it is also the simpler lock order.

## Item 5 — FAIL: the term page breaks the `pipe`-only claim, and the gate sentence conflates off with on

**FAIL — the term page is not under `pipe` alone.** THINK:150 says "all of 2b is under `pipe`"; the term row
(THINK:77) and the std-side `Buffer` (THINK:137) are `atrium-compositor-g1` code (#56 `process.rs:987`,
`syscall.rs:288-289`), and `pipe = ["wait-multiplex"]` (`kernel/Cargo.toml:551`) does not enable it. The
`neo-test-pipe-stdio = ["neo-test", "pipe"]` gate (THINK:172) therefore does not compile the term-page arm or its
8 KiB refusal. Fix as in item 1: add `atrium-compositor-g1` to the scenario feature and say the term page is
available only with it, or make the term page a `pipe`-gated kind. "With `pipe` off, nothing changes" is otherwise
right: the syscall numbers fall through to `_ => ENOSYS` (`syscall.rs:1406`), and the fd arms are cfg'd.

**FAIL — "the existing gate is the kill-switch leg for 'on, nothing selected'" is false.** The 2a THINK is explicit:
"Every scenario in the gate other than the two pipe scenarios builds with `pipe` off, so the off path is the whole
existing gate" (#56 `docs/THINK-2026-11-02-l1a1-pipe-wiring.md:128`). The existing gate is the **off** leg. The
**on/no-selection** leg is `pipe_stdio` leg 4 (THINK:185), and only that leg. Rewrite THINK:152-154: "the existing
gate is the off leg; leg 4 is the on/no-selection leg." The design has both legs; the sentence is what is wrong.

## Item 6 — FAIL: the pitfall rows are present; the discriminators are not implementable as written

**PASS on the CP table.** CP-14, CP-33, CP-61, CP-67 and CP-85 are the right rows, and the "child runs before stdio"
row is the right new one (THINK:158-166). The guards match #56: the fd path uses the 2a token park
(`with_pipe_anchor`), `terminate` finds the park by pid, and `StdioRef`'s `CapId` is the CP-61 check. The CP-33 "Arc
count returns" is observable (`test_harness_pipe.rs:894-905`). The CP-87 row's "checks on the cloned handles" is the
right reading. CP-61's row is right for fd 1 and incomplete for the term page (item 1).

**FAIL — no serial capture for the serial assertions.** Leg 1 says "check that serial shows neither the prompt nor
the reply" (THINK:180), leg 4 says "the prompt goes to serial" (THINK:185), and the control expects
`pipe_stdio_prompt_on_serial` (THINK:199-200). #56's `serial::write_bytes` writes the UART and the log sink; there is
no in-guest capture ring (`serial.rs:223-238`), and the QEMU serial log is outside the guest. The harness can check
the **pipe** side, but not "not on serial". Concrete input: the late-control build; the harness has no way to
distinguish "prompt on serial" from "prompt nowhere", so the named reason is an assumption. Fix: add a `neo-test`-gated
serial tee (a small ring in `write_bytes`, with a read/reset call for the scenario) and name the captured-byte
boundary; or use the framebuffer console, which the harness can already read, and say "not on the frame buffer
console".

**FAIL — no 5-argument test dispatch.** The 2a stand-ins reach the dispatcher through `syscall_dispatch4_for_test`
(`syscall.rs:3733-3742`), which passes `a0`–`a3`; `SYS_SPAWN_STDIO` has five arguments
(`name, len, handles, n, stdio`). The scenario needs a `syscall_dispatch5_for_test` (the real `syscall_dispatch`
already takes six, `:1028`) and a `PT_CALLS` entry with five args. Concrete input: the leg-1 spawn through the
2a harness cannot pass `stdio` at all. This is mechanical but must be in the design or the scenario is not
implementable "through the dispatcher".

**FAIL — the 2a stand-in is one-shot, not a terminal.** `pt_caller` makes one syscall and exits
(`test_harness_pipe.rs:29-41`). `pipe_stdio` needs spawn → read prompt → write command → read reply → close → kill →
wait, and leg 5 needs close → create/derive → write. The design says "a harness stand-in plays the terminal" but names
no scripted multi-call thread or userland terminal module. Concrete input: the leg-1 sequence has no caller that can
make the second syscall. Fix: name a scripted stand-in (a thread over a call table, like a tiny terminal loop) or a
userland `neo-terminal` test module, and say how it synchronizes with the shell.

**FAIL — the late-control does not prove its window.** The 2a cp14 control counts `window_hits` and fails if the
window was not hit (`test_harness_pipe.rs:843-849`). The 2b control says "a yield in the harness" (THINK:200-201). A
yield does not prove the child ran before the post-`try_push_ready` store; on a slow/1-CPU run the store can land
first, the prompt reaches the pipe, and the control passes (or flakes red for the wrong reason). Fix: add a witness
that the child ran first (e.g. wait for its first console byte or a channel ping), count `window_hits`, and fail
`pipe_stdio_late_window_not_hit` if it is 0.

**FAIL — leg 5's "write end at slot 1" is not what `SYS_PIPE_CREATE` does.** `SYS_PIPE_CREATE` installs the **read**
end first, then the write end, at the lowest free slots (#56 `syscall.rs:2240-2256`; `HandleTable::install` lowest
free, `process.rs:2091-2107`). After closing slot 1, a new `SYS_PIPE_CREATE` puts the read end at 1; a
`SYS_WRITE(1)` then hits the wrong-end `EPERM` path unless `expect` is checked first (item 4). To put a **write** end
at 1, create the second pipe before closing 1 (its ends land at 2/3), close 1, then `SYS_DERIVE` the write end into 1.
Concrete input: the current leg as written installs a read end at 1 and expects -9; with the `need`-first order it
gets -1. State the derive/ordering.

**FAIL — there is no positive term-page leg.** Leg 1's word is `0x_FF_01_00` (no term, THINK:176); leg 5 has no term;
the only term mention is the 8 KiB **refusal** (THINK:191). The oracle's selected-with-term row
`00 00 01 00 02 00 01 00` (THINK:131) is a **host** KAT of `encode_stdio_info` (THINK:127), not the kernel's
`SYS_SPAWN_STDIO` term check or `SYS_STDIO_INFO`'s term slot. So a valid 4096-byte term page is never accepted by
the kernel scenario, and the kernel's term-slot reporting is never read. Concrete input: spawn a stand-in with
`handles = [stdin_rd, stdout_wr, term_buf]`, `stdio = 0x02_01_00`, have it call `SYS_STDIO_INFO`, expect the
selected-with-term oracle. Add this leg (and it also needs the Atrium buffer from item 1).

**Note — the xtask registration.** The design gives the feature name but not the `TestScenario` entry (the 2a
entries are at `tools/xtask/src/main.rs:5318-5340`), the control's `NEO_TEST_EXTRA_FEATURES` run, or the expected-PASS
string. The scenario is not in the gate until those are named.

**Note — the new modules are unnamed.** Legs 5 and 7 need a stand-in that can close/derive/write and a stand-in that
calls `SYS_STDIO_INFO`; leg 8 says "the smallest existing one that does, else a new few-line test module, chosen with
the code" (THINK:196-197). That is acceptable as a build-time choice, but legs 5/7 should say "a new test module"
explicitly, and leg 8 must state the module's handles (channel in 0, stdin 1, stdout 2) so it does not assume the old
`[child]`-only layout.

## What holds, and should not be redesigned

- The **slot + `CapId` reference** instead of an `Arc` (THINK:41-53) is the right call: it makes close/revoke reach
  stdio, prevents slot reuse from redirecting the fd path, and leaves teardown alone. `CapId`s are a global monotonic
  `AtomicU64` (`process.rs:99-108`), so "not reused" holds for any practical run.
- **Set before `register`/`try_push_ready`** (THINK:79-82) is correct and is the one ordering that closes the "child
  runs before its stdio" window.
- **Per-handle endpoint counting** (THINK:92-98) is correct; the child's exit and a failed spawn both release, and the
  parent's copy keeps the pipe alive.
- **No implicit inheritance** and **budgeted spawns later** match the PM's §5 answers (THINK:83-91, 210-213).
- The **byte oracle's three cases** and the `u16` layout (THINK:123-134) are internally consistent; only the all-none
  and error cases are missing.

## Verification performed

- Fetched PR #72 and reviewed the head I fetched, `43c2593b1` (queued `b8bd99c4`). Read the THINK at that head;
  `git diff --stat origin/main...pr72` is one file, +213.
- Fetched PR #56 (`71bc2dca`) for the `[2a]` code the design builds on. Read
  `kernel/src/arch/x86_64/syscall.rs`, `kernel/src/sched/process.rs`, `kernel/src/sched/handle_info_logic.rs`,
  `kernel/src/ipc/pipe.rs`, `kernel/src/ipc/pipe_logic.rs`, `kernel/src/test_harness_pipe.rs`,
  `kernel/src/drivers/console/serial.rs`, `kernel/Cargo.toml`, `tools/xtask/src/main.rs`, `docs/SDK-ABI.md`,
  `docs/THINK-2026-11-02-l1a1-pipe-wiring.md`.
- Read the current `origin/main` (`42ccc9238`; the design cites `13eed2c5`, and `13eed2c5..42ccc9238` is `Plans/`
  only) for `sys_write_impl`, `sys_read_impl`, `sys_close_impl`, `RESERVED_FILE_FDS`, `SYS_HANDLE_INFO`,
  `Handle::Buffer`, and the std overlay (`userland/neoplutarch-std-overlay`, `userland/neo-shell`,
  `userland/libneo`).
- Checked the SDK-ABI drift guard (`tools/xtask/src/main.rs`, `sdk_abi_doc_covers_every_kernel_syscall`) and the pipe
  rows/range note on #56.
- Did not build or run anything; this is a design read, and the THINK says the code is not written. No source, THINK
  or other file changed; this commit is the RETURN only. The pre-existing untracked `pr59-review/` directory is not
  staged and is not part of this branch.
- RAG is not available in this worker; used `grep` and direct reads throughout.

## Recommendation to the PM

Keep the stdio-ref shape, the set-before-run ordering and the endpoint design. Before the 2b build: (1) decide the
term page's feature (add `atrium-compositor-g1` to `neo-test-pipe-stdio` and say the term page needs it, or make it a
`pipe` kind) and add a positive term leg; (2) add the two SDK-ABI rows and update the range note; (3) define
`SYS_STDIO_INFO`'s error, all-none and term-identity cases; (4) fix the `HandleKind::Pipe` end source (use
`cap_semantic`), the `expect`-before-`need` order, and the zero-length write; (5) write the scenario's stand-in,
5-arg dispatch, serial capture/observation, and late-control witness. The CP table and the core spawn/pipe reasoning
do not need a redesign; this is an ABI-and-test-plan pass, not a new architecture.
