<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Seats resume on the public repo (2026-10-31)

Development has moved to this public repository, `SecondStageTurbine/LogansOS` (J.c., 2026-10-31). The old repository,
renamed `LogansOS-private`, is the read-only archive. The cloud seats start fresh here and the relay runner on the
always-on box polls this repo's `Plans/RELAY-QUEUE.md`.

Read this file with `git fetch origin && git show origin/main:Plans/DISPATCH-2026-10-31-seats-resume-on-public.md`.

## Rules that are new with the public repo

1. **Everything you push is public the moment you push it.** Before every push, run the personal-details scan the PM
   gave you in your session prompt (it isn't written here, because the list itself would publish what it guards).
   Any hit in our own text (not vendored upstream test data) is removed before pushing. Don't add Windows user
   paths, LAN addresses, MAC addresses, hostnames, serial numbers or real-machine ACPI tables. These files stay out of
   this repo: `docs/hardware/inventory-*`, real-machine `fixtures/acpi/<machine>/` sets, `scripts/m910q/`.
2. **The gate runs on GitHub-hosted runners** (no KVM, 4 vCPU, TCG): dispatch it yourself on your branch's exact head,
   `CI` workflow, `workflow_dispatch`, ref = your branch. A 4-shard run takes about 25 minutes. Keep at most two of
   your own gates in flight. A scenario that only fails here on time is a timeout to raise (with the elapsed time in
   the commit), not a code fault; `desktop_home_cycle` was the first (main `76a1f03`).
3. **Don't attach the private archive** (it stops you on a prompt only J.c. can answer). Everything you need from it
   is already here: carried-over PRs #1-#8 and the `import/pr<N>` branches below.
4. **Every time you wake, read the newest `PM:` comments on each of your PRs before anything else.** A
   cross-family review that bounces your PR is posted there; "ready" on a bounced head wastes a gate.
5. Everything else in `CLAUDE.md` holds: the close-gate ritual, verify-before-merge, the AAG, cross-family reviews,
   no model names in commits or PR text, and a `PM:` comment on the PR with the gated head SHA when it's ready.

## Already carried over (by the PM, squashed, scanned)

| Private PR | Public PR | Branch | Next |
|---|---|---|---|
| #266 | #1 | `claude/ot61-vendor-verify` | gate |
| #261 | #2 (draft) | `claude/ot61-c3a-crt` | clang-in-CI blocker, then gate |
| #260 | #3 | `dexter/size-budgets` | gate |
| #249 | #4 (draft) | `claude/ot61-c2.4-stylo` | paperwork, then gate |
| #272 | #5 (draft) | `claude/l2.1-always-undo` | gate |
| #255 | #6 | `claude/ot90-p3b-stop-cpus` | gate |
| #256 | #7 (on #6) | `claude/ot90-p3b-quiesce` | gate after #6 merges |
| #257 | #8 (on #7) | `claude/ot90-p4-power-cap` | gate after #7 merges |

## Imported, with conflicts to resolve: each needs its seat

These conflicted with current `main` (they had fallen behind on the private repo too), or their parent moved after
they were cut. **The PM has imported each one here as `import/pr<N>`** (J.c. approved, 2026-10-31): the PR's net
change applied three-way onto its public base, conflict markers committed as they fell, scanned. Do NOT attach the
private archive. Start your branch from the import, resolve the markers (`git grep -n '^<<<<<<<'`), gate, and open
the PR from your own branch name. The import commit message lists the conflicted files. Stacked imports were applied
on the previous import, so resolve them in order.

| Private PR | Branch @ head | Seat |
|---|---|---|
| #271 | `claude/user-copy-fault` @ `c1ef8197` | A |
| #220 | `claude/a1a-apps` @ `fd293a63` | B |
| #254 | `claude/l71-boot-time` @ `39389ac7` | C |
| #258 | `claude/ot90-p5-power-menu` @ `55391e3c` (re-stack onto #8) | C |
| #251 | `claude/ot61-r5-fetch-broker` @ `adfb64c3` | D |
| #262 | `claude/ot61-r4-swgl` @ `13bcaf38` (re-stack onto #2) | D |
| #263 | `claude/ot61-c3b-pthreads` @ `2a2f7e9e` (re-stack onto #2) | D |
| #264, #268, #269, #270 | `claude/ot61-r4b-webrender`, `-r4c-atrium`, `-r4d-grant`, `-r6a-client` (stack on #262) | D |

Open the public PR with the same title, say "carried over from private #N" in the body, and keep the base order of a
stack. Re-stack only onto a base that is already a public PR.

## Seat A: the user-copy fault

Re-land private #271 here (a syscall's user copy that faults returns EFAULT instead of halting the kernel; CP-99).
It is SMP/concurrency-adjacent kernel code: the AAG needs a cross-family lens, which the PM routes. Then gate.

### Seat A, next: tiered testing that fits the free tier (OT-58, J.c. 2026-11-01)

J.c. will never pay for GitHub, and the free tier runs 20 jobs at once while one full gate is about 20 jobs, so
five seats' gates queue behind each other for an hour or more. Build the tiered gate OT-58 planned
(`docs/THINK-2026-07-12-ci-gate-scaling-SYNTHESIS.md`):

1. **Per PR, a quick tier:** fmt, clippy, host tests, SPDX and one short QEMU smoke shard. Target 3-5 jobs,
   about 15 minutes, so every seat can gate at once.
2. **The full tier on a batch:** the 4-shard QEMU set and the feature matrix run once on a merge of several
   quick-green PRs before they land (the PM's batch-gating practice, made a workflow input), and on any PR that
   touches the SMP/concurrency core (scheduler, thread lifecycle, futex, per-CPU, locks, kill paths), which still
   gates alone.
3. **Main stays green:** a red batch is bisected by PR; the escaped-defect contract (revert first) is unchanged.
   The scenario that caught an escape is promoted into the quick tier.
4. Write the THINK first (which scenarios go in the smoke shard and why, the batch input, the concurrency-core path
   filter), open it as a PR for the PM, then build it without breaking the current `CI` workflow the other seats
   are using.

## Seat B: the WM lane and the terminal window

1. Re-land private #220 (OT-88 A1a, the WM's client table). If it isn't yours, say so in your first `PM:` comment.
2. Gate #5 (`claude/l2.1-always-undo`).
3. Then **Lane 1, L1a**: the terminal window app on the WM. Design and prior art are in
   `docs/THINK-2026-10-31-lived-in-os-lanes.md`, section 3. Write the L1a THINK first (the client protocol, the
   escape-sequence subset and its byte oracle, raw/cooked mode, the kill-switch, and how no input reaches the terminal
   while the lock screen is up). Open it as a PR for the PM's read before building.

## Seat C: OT-90 and boot time, then FAT

1. Gate #6, then #7, then #8, in order (each after the one below it merges).
2. Re-stack private #258 onto #8 and open it here.
3. Re-land private #254 (L7.1 boot time) and gate it.
4. Then **Lane 3, L3a**: mount FAT read-only from a block device (THINK section 5). Trap-mining first.

## Seat D: OT-61 (the browser), always on

**J.c. (2026-10-31): at least one seat works on the browser at all times; it is the biggest project in the OS.**
Seat D is that seat. It never goes idle and is never pulled onto other lanes. When it's waiting on a gate or a
review, it starts the next browser step instead (the next OT-61 rung, a THINK for the rung after, or prior-art work for
it), and says in its `PM:` comment what it moved on to.

1. Gate #1.
2. #2: clear the clang-in-CI blocker (the hosted runner image can install clang in the job), then gate.
3. Re-stack #262 and #263 onto #2, then the rung-4 stack (#264, #268, #269, #270) in order; re-land #251.
4. #4 stays draft until its paperwork is done.

## Relay 226: Dexter, Lane 2a (miniz_oxide)

MODEL: DeepSeek, high, because it's a bounded host-only build, Dexter's lane.

Vendor `miniz_oxide` (MIT; checked 2026-10-31 at `7fc2dd4`, re-check the head and licence on the day) under
`third_party/miniz_oxide/` with its licence file, upstream URL and commit recorded. Add host KATs: inflate known gzip
and zlib streams fetched raw with `git` or `curl` (never a summarizing fetch), plus a round trip. No kernel change.
See `docs/THINK-2026-10-31-lived-in-os-lanes.md` section 4.

- WORKTREE: `~/workers/dexter` on the always-on box (yours alone).
- BRANCH: `dexter/l2a-miniz-oxide` (`git fetch origin && git checkout -B dexter/l2a-miniz-oxide origin/main`).
- Before pushing, check your diff for personal details (rule 1): Windows user paths, LAN addresses, MACs, hostnames,
  email addresses. Vendored upstream test data is fine.
- Close: `scripts/worker-close-gate.sh`; push; open a PR to `main` and paste the push-CI URL. RETURN file
  `Plans/RETURN-2026-10-31-l2a-miniz-oxide-dexter.md` on your branch, naming the head SHA.
- RAG: not available on the always-on box; use grep and direct reads.
- Disk reclaim: after the PM confirms the merge, `cargo clean` in your worktree.
