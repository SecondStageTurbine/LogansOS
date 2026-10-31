<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Batch 38/39 bounces (seat C) and the user-copy fault lane (seat A)

Cross-family reviews: Chad 209 (`Plans/RETURN-2026-10-31-b38-rechecks-chad.md`), Janus 210
(`...-ot92-cache-review-janus.md`), Jarvis 212 (`...-l71-boot-time-review-jarvis.md`). The PM re-checked each FAIL
below in code. Rebase every branch onto `origin/main` first, keep `site/` out of the diff, and run
`scripts/worker-close-gate.sh`.

## Seat C

### #267 (shutdown deadline): bounced, P1 (Chad 3b)
The coordinator stops watching as soon as `STEP` is `Done` (`kernel/src/acpi/shutdown.rs:129`), but K3 prints after
its flush, and `power_off` / `restart` then print and call `log::flush()` (`power.rs:140-141`, `:268-269`). That
flush takes `SINK` and writes to the device with no deadline. Input: an armed block log sink whose device accepts the
first post-Done write and never completes it. Neither S5 nor restart is reached. Fix: abandon (or close) the sink
before handing over to the power path, so nothing after `Done` can touch the disk, or bring that flush under the
deadline. Add a post-Done stuck-sink leg for both power-off and restart. Also correct the THINK's line that power-off
"proceeds without retrying into a hang" until it is true.

### #250 (NeoFS cache eviction): bounced (Janus 4 and 5)
- **Kill-switch:** with `neofs-cache-evict` off, `forget_range` still bumps `evictions` (`cache.rs:358-366`), so a
  concurrent miss is no longer inserted. Off must be the old behaviour bit for bit: gate the bump (or the epoch
  check) on the feature, and add a unit test for the off mode.
- **CP-64 bound:** one transaction can dirty more than capacity (truncate an empty file to 300 blocks: 300 dirty
  blocks, and the next insert scans 301 entries with IRQs off). Either cap the dirty count per transaction (throttle
  inside the loop) or correct the CP-64 row and the "dirty stays below capacity" claim with the real bound.
- **Janus 3 (a mutation waits forever on a device that never completes) is a note, not a bounce:** the driver has no
  I/O timeout, and fsync already had the same wait. Say so in the PR, and note that #267's deadline is what bounds
  shutdown. A driver I/O timeout is a separate follow-up.
- **AAG note:** the `write_bytes_at` retry has no bound (a busy reader at capacity can starve a partial writer).
  Bound it, or use a per-block epoch.

### #254 (boot time): bounced (Jarvis 1 and 4)
- The numbers are labelled "From power-on", but the TSC origin is never related to firmware time. The PR's own
  fixture prints model and desktop times earlier than firmware exit. Either establish the origin (and refuse the
  summary when the times are out of order), or label it honestly ("since TSC reset").
- `ebs_exit_ns = 0` means "not recorded", but it prints `firmware 0.00 s`. Check each field and omit what is missing.
- Mark a CPUID-0x16 estimated rate as an estimate in the record and on Home.
- Jarvis 2 (a bad user pointer halts the kernel) is real, but it is the kernel-wide user-copy fault below, not
  #254's alone. No #254 change is needed for it, beyond using the shared helper once seat A lands it.

### #241 (egress bound): bounced for its claims (Chad 1b, 1c, 4)
- 1c: the 512 cap is an admission threshold. Put-back after the send can reach 528, and the syscall pump extends past
  it. Either count retained and in-flight segments against the cap, or call it an admission threshold in the code,
  the PR and CP-54.
- 1b: `snapshot_sized` retries for as long as interfaces keep being registered. Registration is finite (hardware), so
  this is not a hang in practice, but "the whole call is bounded" is not true as written. Add a retry bound (fall back
  to the stale snapshot) or state the dependence in CP-54.

### #265: Chad 2 PASS. It gates in the next batch.

## Seat A: a user-copy fault halts the kernel (new lane, after #247)

**Problem (PM-verified on main).** `validate_user_buf` (`kernel/src/arch/x86_64/syscall.rs`) checks only the address
range. Syscalls then copy to or from user memory with a raw `copy_nonoverlapping` inside `with_user_access`. If the
page is unmapped or read-only, the CPU faults in ring 0, and the page-fault handler (`kernel/src/arch/x86_64/idt.rs`,
the kernel-mode branch after the `USER_MODE` check) calls `halt_forever()` outside test builds. Input: from any
process, `SYS_BOOT_TIMES(0x0000400000000000, 80)` (or any syscall with an output buffer) on an unmapped user page.
The whole machine stops. This applies to every syscall that touches user memory, not just #254.

**Build:**
1. **Prior-art survey and CP consult first** (the rules in CLAUDE.md). Read for principle: Linux's exception table
   (`__ex_table` / `fixup_exception`, GPL: principle only), the BSD `copyin`/`copyout` with `onfault`, and Redox and
   Theseus if they have one. List the license and last commit date of each.
2. A fault-tolerant user copy: a registered copy routine whose fault the handler recognises by RIP (an exception
   table, or a per-CPU "in user copy" recovery point), so a kernel-mode fault on a user address inside it returns
   `EFAULT` instead of halting. Translating the pages first is not enough by itself, because another thread can
   unmap between the check and the copy.
3. Route every syscall user copy (in and out) through it. Start with the output paths; list any you leave for later.
4. Any kernel-mode fault outside those routines still halts (fail-loud is unchanged).
5. A scenario: a ring-3 worker calls each output syscall with an unmapped buffer, and with a buffer that crosses
   from a mapped page into an unmapped one; each returns `EFAULT` and the kernel keeps running. Its control (the
   fixup disabled) must fail.
6. Kill-switch per the governance rules, and an SMP note: one thread copying while another thread of the same
   process unmaps the page.

The cross-family review will be Codex (it is SMP-adjacent kernel core).

## Section 213: Dexter, re-check of #248's fixes (your 203 findings, head `6b27864a`)
1. F1: are the path and intent reads now polled and bounded, with `lock::service` / `lock::guard_input` still running
   while the WM waits? Does the new stall leg (a header without its path, then Super+L and the idle lock) actually
   exercise it, and would it fail on the old code?
2. F2: after Super+Escape, does an answered prompt restore the route to its real owner?
3. Item 5: are the CP-57 / CP-97 rows present and accurate?
4. Notes: is `identity` now checked on both messages, and is an Always that could not be saved answered as once?

RETURN: `Plans/RETURN-2026-10-31-arthur-prompt-recheck-dexter.md`.

## Seat A: #247 bounce (Dexter 211), before the user-copy lane

Dexter found no escape from a scoped cap: `..`, symlinks, mounts, rename/link and the two path syscalls are all
covered, and no widening was found. Two items to fix:
- **F1:** the Lens is a client and is handed `/` with `LOOKUP|READDIR` (`lens::bloom`, `desktop_session.rs:5181`,
  `:5371-5382`). It can't read files, and `.system` is still refused, but it contradicts THINK line 90 ("no client
  holds the root cap") and the lane's "every client". Narrow the Lens to the subtrees it shows, or justify a
  browse-only root in the THINK and the PR, and correct line 90 either way.
- **Item 5:** the survey covers login/session prior art, not path scoping. Add the path-scoping rows (Capsicum's
  `openat` beneath a directory fd and `O_BENEATH` / `RESOLVE_BENEATH`, Fuchsia's directory handles, WASI preopens),
  each with its license and last commit date.
- Notes: per-client revocation is lost because `sys_spawn_scoped` closes the WM's copies (F2; say so, or keep a
  revocable copy). There are no tests for `..`, symlink or mount escapes; add a `..` leg at least.

Rebase, keep `site/` out of the diff, run `scripts/worker-close-gate.sh`, push, and post the head SHA. Then start the
user-copy fault lane above.
