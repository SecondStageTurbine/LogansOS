<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 366, PR #72 stdio-at-spawn review — Janus

Reviewed head: `576cd862c5e48dee81a3a67a1a489ad46665cf4d` (also the fetched `refs/pr/72`).
Dispatch: `Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md`, Section 366, from `origin/main` at `145059d5b`.
RETURN branch: `codex/pr72-review-janus`, based on that main. All code citations below refer to the reviewed PR head, not the RETURN branch.

**Verdict: REQUEST CHANGES on the concurrency discriminators.** The production publication, capability identity, refusal and endpoint paths pass this source review. F1 and F2 are gaps in the promised evidence; neither alleges a demonstrated production use-after-free. The unexecuted late control is not an acceptable completed control.

## Findings

### F1 — P2: the late control still needs runtime expected-failure evidence

Concrete control input: enable `neo-test-pipe-stdio-late-control` and run leg 1's `SYS_SPAWN_STDIO("shell", [stdin_rd, stdout_wr], 0x00FF_0100)`. The control substitutes NONE before construction (`kernel/src/arch/x86_64/syscall.rs:2125`), arms the recorder before spawn (`:2127`) and delays the real store until after a witnessed child syscall or a bounded miss (`:2324`, `:2343`). The harness rejects misses (`kernel/src/test_harness.rs:3530`). This source wiring is present, but clippy does not establish its runtime hit, failure reason or serial witness.

**Source check: the witness should expose the bug for this shell scenario.** FIRST is recorded at syscall entry (`syscall.rs:1094`, `:2309`), before stdio resolution. However, spawn places the child on the caller's CPU (`kernel/src/sched/scheduler.rs:648`), and syscall entry clears IF (`syscall.rs:691`). The shell's first syscall writes READY (`userland/neo-shell/src/main.rs:70`, `:1254`); it does not park before reading the selection. The parent therefore cannot resume on that CPU merely between the first-entry record and that selection read. Its late store follows at least that console write, so the quiet-prefix assertion can detect the defect (`tools/xtask/src/main.rs:21918`). The recorder is sufficient here because of these scheduling and child-startup constraints, not because any arbitrary first syscall proves stdio use. A future cross-CPU or different-child version needs a witness at the actual stdio decision.

The exact named runtime outcome still needs checking. The first write is the banner, not the prompt; a timer can return to the parent between those two user calls. Thus do not infer `pipe_stdio_prompt_not_in_pipe` solely from FIRST. The quiet-prefix checker rejects a leaked banner even if the prompt later uses the pipe. Capture which path actually rejects the control; if it rejects only with a different generic reason, align the witness/diagnostic with the THINK's promised named outcomes.

Clippy checks neither scheduling nor the expected failure. Required runtime run, at the reviewed head (and rerun after changes), with serial and exit status retained:

```sh
NEO_TEST_EXTRA_FEATURES=neo-test-pipe-stdio-late-control cargo neo-test pipe_stdio
```

Also run the control with `NEO_SMP=2` and the normal `cargo neo-test pipe_stdio` without the control. The normal run must pass. The control must fail for observed old-selection use (`pipe_stdio_prompt_not_in_pipe`, with console leakage); `pipe_stdio_late_window_not_hit` is a correctly rejected miss, but supplies no hit evidence and needs another run. A generic timeout/build failure is not this control's evidence. Simply adding CPUs does not place this parent and child on separate CPUs. No runtime control run was performed in this review; the existing green normal CI does not replace it.

### F2 — P2: leg 3 labels a visible prompt as a parked reader

`userland/neo-init/src/pipe_stdio_test.rs:188` sets `parked` to the result of reading `neo> `, then kills immediately (`:189`). The shell writes the prompt before entering `read_line` (`userland/neo-shell/src/main.rs:81`–`:82`). There is no acknowledgement of the fd-0 token park.

Concrete input and schedule: run leg 3's shell spawn with empty stdin; let the shell finish writing its prompt, then let a timer preempt it in ring 3 before `SYS_READ(0)` and resume the terminal on the same CPU. The terminal reads the buffered prompt and kills a ready child. Reaping, stdout EOF and stdin EPIPE can all pass, and the test prints `parked=true`, without exercising kill of a parked fd-0 reader. For example, a regression making the selected fd-0 call nonblocking would let the shell poll EAGAIN and still satisfy those assertions. The existing explicit pipe park tests would not establish that this fd path blocked.

Require an observed token park for the target pid and selected stdin before issuing kill, with a bounded named failure if the park never occurs. The existing `pipe_park_kill` test has the stronger pattern: it waits for `parked_for_test(End::Read) == 1` before terminate (`kernel/src/test_harness_pipe.rs:891`). This finding concerns the new leg's CP-33 discriminator; the shared production token extraction remains guarded.

## Section 366 checklist

1. **PASS — stdio installed before publication.** The handle install block ends at `kernel/src/sched/process.rs:4564`; the store is at `:4574`, registration at `:4580`, and `try_push_ready` at `:4613`. `Stdio::for_spawn` takes each child slot and its child-side capability from the same inherited list (`:1679`), whose fresh IDs were allocated by `sys_spawn_impl` (`kernel/src/arch/x86_64/syscall.rs:2101`). No real-path child can run before the selection is stored. Only the explicitly gated late control substitutes NONE (`syscall.rs:2125`).

2. **PASS — refusal checks, with precedence.** Existing prechecks precede decode (`syscall.rs:1997`, `:2018`); name/handle resolution precedes role validation (`:2030`, `:2054`, `:2120`). Step 2 checks all upper bits, every index, every duplicate-role pairing and term support (`kernel/src/ipc/pipe_logic.rs:98`). Step 4 checks the actual clones for pipe end, USE and the semantic right, and for term kind, exactly 4096 bytes, USE, MAP_READ and MAP_WRITE (`syscall.rs:2354`). Every refusal happens before spawn construction.

   | Concrete input (valid name and user buffers unless stated) | Refusal |
   |---|---|
   | stdin index 0 naming a live GRANT-bearing pipe write end, word `0x00FF_FF00` | step 4 `EINVAL` (-22), wrong end |
   | stdout index 0 naming a live GRANT-bearing pipe read end, word `0x00FF_00FF` | step 4 `EINVAL`, wrong end |
   | either pipe role naming a live GRANT-bearing channel or buffer | step 4 `EINVAL`, wrong kind |
   | term index 0 naming a pipe, word `0x0000_FFFF` | step 4 `EINVAL`, wrong kind |
   | inherited array contains a closed slot or caller slot `0xFFFF_FFFF` | step 3 `EBADF` (-9), `syscall.rs:2108` |
   | a live inherited handle lacks GRANT | step 3 `EPERM` (-1), `syscall.rs:2107`, before role checks |
   | a GRANT-bearing selected pipe lacks USE or its READ/WRITE right | step 4 `EINVAL`, `syscall.rs:2359` |
   | `n=1`, stdin index 1, word `0x00FF_FF01` | step 2 `EINVAL`, selection index out of range |
   | stdin and stdout both index 0, word `0x00FF_0000` | step 2 `EINVAL`, duplicate role; all three pairings are checked |
   | bit 24 or 63 set | step 2 `EINVAL`, reserved bits |
   | term index 0 with `atrium-compositor-g1` absent | step 2 `EINVAL`, even before handle resolution |
   | term buffer 4092/8192 bytes, or GRANT-bearing term lacking USE/MAP_READ/MAP_WRITE | step 4 `EINVAL`, `syscall.rs:2375` |

   A file disallowed by ordinary spawn is refused earlier with `EPERM` (`syscall.rs:2098`); wrong-kind does not universally mean EINVAL when an earlier check applies. A selection-array index out of range is distinct from a caller-table slot out of range.

3. **PASS — capability identity before kind/rights/use.** `pipe_lookup` compares the expected ID at `syscall.rs:2496`, before the kind/rights match at `:2499`; buffer lookup does the same at `:9095` before `:9098`. Empty or replaced slots return EBADF, including reuse by the wrong pipe end or another buffer. Precision about lock lifetime: `HandleTable::get` returns an owned clone (`process.rs:2306`), so the temporary table guard ends at the lookup statement. The comparison operates on that coherent cloned snapshot, not a borrowed table entry; close cannot change its ID/object or free its Arc. The code is safe here despite the comments' literal claim that the subsequent comparison is still under the table lock. Keeping an explicit guard through comparison would align the implementation wording with the THINK.

4. **PASS — copy selection before resolution/park.** `Process::stdio` returns a Copy value and releases its guard (`process.rs:1711`); `selected_stdio` returns only a `StdioRef` (`syscall.rs:2266`). fd 1 calls `pipe_write_at` with the expected ID (`:7157`), fd 0 calls `pipe_read_at` before the keyboard arm (`:7329`). A close before lookup gives EBADF, including slot reuse; zero-length fd reads/writes retain their zero result (`:7314`, `:7154`). After a successful lookup the Arc pins the old object; an operation already in flight may complete, and closing the last same-side endpoint is handled by the shared pipe logic. There is no raw reference into a freed table entry. Both bodies transfer ownership to `with_pipe_anchor` before blocking (`:2599`, `:2664`), and the thread releases it on reap (`kernel/src/sched/thread.rs:273`). **CP correction:** close/reuse is CP-61 and park ownership is CP-67; CP-112 is specifically publication before the child's first instruction (`docs/concurrency-pitfalls.md:153`, `:4847`).

5. **PASS — INFO, TERM_MAP and SDK numbers.** INFO checks length first (`syscall.rs:2415`), validates/copies the output with EFAULT handling (`:2418`, `:2434`), reports only matching IDs (`:2395`) and preserves selected=1 after closure (`process.rs:1672`). All-none becomes NONE with flags zero; the encoder's four oracle rows are host-tested. TERM_MAP returns ENOSYS without compositor support (`syscall.rs:2445`), EBADF for an absent term (`:2459`) and delegates to the buffer mapper with the expected ID (`:2462`), retaining normal role/runtime/rights errors (`:9228`). A replacement 4096-byte buffer in the old slot is EBADF (`userland/neo-worker/src/stdio_probe.rs:159`). Kernel constants (`syscall.rs:406`, `:410`, `:414`), libneo (`userland/libneo/src/pipe_abi.rs:23`) and SDK rows (`docs/SDK-ABI.md:366`) agree on 445/0x1BD, 446/0x1BE, 447/0x1BF. The SDK drift test passed.

6. **PASS — endpoint accounting.** Child installs call `retain_pipe` (`process.rs:4555`, `:2224`, `:2181`). StdioRef adds no owner or endpoint. Natural exit drains the table (`:4973`) and drops Detached (`:4996`); kill likewise tears down (`:6190`). Detached releases every pipe endpoint once (`:1974`, `:1994`); abandoned tables drain on Drop (`:1802`). Queue refusal unregisters/frees the unstarted child (`:4613`), so its installed handles also release. Closing the terminal's copies after spawn (`pipe_stdio_test.rs:135`) leaves the child's copies counted until close/exit; no new leak or early EOF path was found. EOF and kill endpoint observations are in legs 2/3; F2 limits only the claim that leg 3 proved an actual park.

7. **PASS — pipe-off behavior.** The Process field/store and fd dispatch arms are cfg-gated; pipe-off spawn uses the original `spawn_user_by_name` (`syscall.rs:2133`), and the three new syscall dispatch arms are absent (`:1413`). Plain and budgeted spawns pass NONE (`process.rs:4667`, `:4744`, `:4804`). Existing console/file paths remain after the gated branches (`syscall.rs:7160`, `:7331`). The shell's added INFO query returns ENOSYS and falls back to ordinary spawn (`userland/neo-shell/src/main.rs:1193`), preserving visible console behavior. With pipe on but no selection, leg 4 uses ordinary spawn and requires exit 2 (`pipe_stdio_test.rs:472`). The cited green full PR run covers the existing off scenarios and normal on scenario; I did not independently rerun the QEMU matrix.

8. **FAIL for runtime validation; PASS for current-scenario source wiring.** F1 supplies the control input, source argument and required run. A clippy-only control is insufficient. Run the normal and expected-failure commands above; retain the serial witness and named result, resolving any mismatch with the promised outcome. A window-not-hit result rejects that trial and does not prove detection of the bug.

9. **FAIL as a complete discriminator table; production guards present.** The requested class-by-class accounting is below. CP-33's new fd-path park witness and CP-112's control proof remain incomplete (F2/F1).

   | Class | Introduction / guard | Discriminator and review result |
   |---|---|---|
   | CP-14 lost wake | Existing mechanism reused by new fd callers; the pipe state check and token park share a guard (`kernel/src/ipc/pipe.rs:226`, `:319`). | **PASS:** `pipe_core` has an armed two-CPU window, eight rounds and a hit check (`kernel/src/test_harness_pipe.rs:806`). Its cp14 control drops the guard. Leg 1 is additional integration coverage, not independently proof of a park. |
   | CP-33 incomplete kill | New fd-0/1 callers; shared pid-based token extraction (`kernel/src/sched/scheduler.rs:1390`, `process.rs:6121`) guards them. | **FAIL for the new claimed discriminator:** leg 3 never observes park (F2). Existing `pipe_park_kill` waits for a real park before killing readers and writers (`test_harness_pipe.rs:891`, `:944`), so the shared machinery does have a discriminator. |
   | CP-61 slot reuse | Introduced by stored role slots; fresh child CapId plus expected-ID lookup guards stdout/stdin/term. | **PASS:** leg 5 closes stdout, reuses it with another write end and then a read end, requires -9, and checks the other pipe remained empty (`stdio_probe.rs:98`). Leg 9 replaces the term buffer and requires -9 (`:159`). The shared resolver covers stdin too; no separate stdin-reuse leg is claimed. |
   | CP-67 Arc across park | Existing ownership risk reused by the fd callers; thread-owned pipe anchor guards it. | **PASS:** shared-body `pipe_park_kill` checks post-kill Arc counts for reader/writer, with `neo-test-pipe-park-kill-control` as the leaking variant (`test_harness_pipe.rs:883`, `:935`; `scheduler.rs:1732`). Copying stdio holds no process Arc or stdio guard across this park. |
   | CP-112 publication race | Introduced by the new Process field; production store precedes register/ready (`process.rs:4574`). | **PASS source guard/wiring, FAIL completed control evidence:** normal leg 1 and quiet-prefix assertion are registered, and the same-CPU IF-off shell startup supports the witness, but no runtime expected-failure evidence is supplied (F1). The bank correctly names the class (`docs/concurrency-pitfalls.md:4847`). |

   For completeness, THINK §3's extra rows are consistent: CP-85 is N/A to the selection itself because it owns no object, and CP-87 uses the same inherited clones and CAP_GRAPH parent-liveness check before installation/publication (`process.rs:4533`).

## THINK §7 departures, each reviewed

| §7 row at the reviewed head | Result | Code and reason |
|---|---|---|
| Worker role instead of another module (`THINK:398`) | **PASS** | Worker selects `stdio_probe::run` under the scenario feature (`userland/neo-worker/src/main.rs:451`); xtask requests that feature (`tools/xtask/src/main.rs:5402`). It exercises real user syscalls without new module staging. |
| TSC-bounded nonblocking polls (`THINK:399`) | **PASS** | `pipe_stdio_test.rs:24`, `:41`, `:63`, `:357`, `:449`: bounded empty-pipe waits. Blocking spawn/wait are not asserted to be bounded by this poll mechanism; the scenario harness supplies its outer timeout. |
| Add runtime feature (`THINK:400`) | **PASS** | `kernel/Cargo.toml:1243`; buffer mapping checks runtime enable at `syscall.rs:9228`. |
| Add close-object-handles (`THINK:401`) | **PASS** | Same scenario feature row; stale-term probe uses SYS_CLOSE at `stdio_probe.rs:161`, and Buffer closure requires this feature (`syscall.rs:7704`). |
| Killed child's wait may return ESRCH (`THINK:402`) | **PASS for return semantics** | `pipe_stdio_test.rs:192` accepts Ok or ESRCH while requiring EOF/EPIPE and successful kill. This is separate from the missing parked witness in F2. |
| 4092-byte invalid term buffer (`THINK:403`) | **PASS** | Created and refused at `pipe_stdio_test.rs:278`, `:303`; descriptor stride is a multiple of four and bytes must equal stride times height (`kernel/src/atrium/logic.rs:118`). 4095 bytes cannot describe this buffer format. |
| 8192-byte buffer as 1024 x 2 (`THINK:404`) | **PASS** | `pipe_stdio_test.rs:247` builds width=min(pixels,1024), height=pixels/width; case at `:304` reaches the stdio size refusal instead of framebuffer width refusal (`atrium/logic.rs:130`). |
| Print/pin all-none record on console (`THINK:405`) | **PASS** | `pipe_stdio_test.rs:380`, `stdio_probe.rs:52`, xtask's expected record at `tools/xtask/src/main.rs:19248`. The exact bytes are `ff ff ff ff ff ff 00 00`. |
| Run leg 4 last (`THINK:406`) | **PASS** | `pipe_stdio_test.rs:495` puts the unselected leg last; `tools/xtask/src/main.rs:21924` splits at its marker and rejects the four pipe-output strings in the prefix. The earlier all-none diagnostic is deliberately unselected. |
| First-other-syscall recorder / late yield (`THINK:407`) | **PASS source wiring; FAIL runtime validation** | `syscall.rs:2309`, `:2326`, `:2343`; the child's same-CPU IF-off first banner write supports detection, and the harness rejects a recorded miss. F1 names the concrete control input and the run still needed; clippy alone supplies no runtime discriminator result. |

## Validation and scope

- Fetched the PR and inspected its exact head in a detached worktree. No PR implementation changes were made; this branch adds only this RETURN.
- `cargo test -p pipe-logic-tests`: **27 passed**, including all four new stdio tests, existing endpoint/EOF tests and the encoder/libneo decoder oracle.
- `cargo test -p xtask sdk_abi_doc_covers_every_kernel_syscall -- --exact tests::sdk_abi_doc_covers_every_kernel_syscall`: **1 passed**, 196 filtered out.
- Independently read GitHub's run/job metadata: [CI 42493229380](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42493229380) is completed/success at the exact reviewed SHA; all four QEMU shards, host checks and build checks succeeded. This is existing PR evidence, not a reviewer-executed control run.
- Local RAG health request was connection-refused; used direct source reads and exact symbol searches. This did not block inspection of the named paths.
- No normal or control QEMU run was executed locally. The runtime control remains required as specified in F1. A normal green gate alone cannot close F1 or F2.

Disposition: return to Seat B for the leg-3 parked witness and runtime late-control evidence (including any diagnostic correction that run exposes), then re-review at the resulting head. Seat C's host quantization lane is outside Section 366 and was not changed.
