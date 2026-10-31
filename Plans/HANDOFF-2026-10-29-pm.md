<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PM HANDOFF — 2026-10-29, afternoon (account switch)

**For:** the next PM session (Claude, the cloud PM), on J.c.'s new account. **From:** the PM session that ran 10/23 →
10/29. Read this, then `CLAUDE.md`, then the last ~150 lines of `Plans/LEDGER-2026-07-29-logansos-release-ledger.md`.
Everything below is on `origin`; nothing lives only in the old session.

## 0. The one-paragraph state

`main` is green and equals the batch 19 gate tree in every code path. **Batch 20** (#212 + #215, run `40898712909`,
head `942f20e0`) is **red on shard 4**, most likely #212's `neofs_grow_write`. §2 says how to confirm it and what to
do. Every open PR has an owner and a written fix list (§3). The coder seats S1–S5 were asked to push
to a clean checkpoint and stop; their reported states are in §4. J.c. pastes Muse's RETURNs by hand: four FAIL texts
(#157, #161, #167, #168) are **still owed**, and Muse's retries (#152 first, then #150, #151, #133, #134 items 4–5)
were cleared to run serially.

## 1. How the PM works (what the previous PM did each loop)

- **The gate.** Kernel-touching PRs gate in batches on the integration branch `claude/pm-handoff-document-ypgky3`:
  1. Build the batch tree on current `origin/main`, merging each PR head in turn.
  2. Conflicts are almost always additive scenario-registry entries. These files collide: `kernel/Cargo.toml`,
     `kernel/src/test_harness.rs`, `tools/xtask/src/main.rs`, and sometimes `neo-init`/`neo-worker` `main.rs`.
     Union them with `u3.py` (§6).
  3. `cfg(any(...))` feature lists need a **hand** resolve, because the union tool duplicates or drops commas there.
  4. Before firing, run the local checks:
     - `cargo fmt --all --check`;
     - `cargo test -q -p xtask`, which includes the shard-coverage test;
     - `cd kernel && cargo check -Zjson-target-spec --features <each touched scenario feature>`, looking for errors only;
     - kernel clippy: default, plus any feature the batch touches, with `-- -D warnings`.

     Per-scenario clippy with `-D warnings` always shows dead-code noise; that is not a merge error.
  5. Put the tree on the integration branch as a fast-forward:
     `git commit-tree <tree> -p origin/claude/pm-handoff-document-ypgky3 -p <tree-commit>`, then push.
  6. Fire `ci.yml` by `workflow_dispatch` on that branch (the GitHub MCP `actions_run_trigger`).
- **Merging on green.** Use `prep.sh <branch> <expected-head>` for each PR, in order (§6). It:
  - checks that the head hasn't moved;
  - merges `origin/main` into the head, taking any conflicted file from the gate tree;
  - pushes that merge to the PR branch;
  - prints the new head.

  Then squash-merge with `sha` = that head. Afterwards, verify `origin/main`'s `kernel userland tools scripts .github
  Cargo.toml Cargo.lock third_party` trees equal the gate tree's. **Edit prep.sh's gate SHA and batch name first.**
- **Before any kernel-touching merge, the AAG:**
  - a background subagent with lenses, prompted to refute and default to suspicious;
  - re-verify every FAIL yourself in the code before bouncing;
  - SMP-core changes also need a cross-family lens **before** merge (Codex, via a relay).
- **Relays.** `Plans/DISPATCH-<date>-<slug>.md` on main, plus a paste block with these lines:
  - MODEL/EFFORT;
  - WORKTREE `E:\AI Automation\workers\<coder>`;
  - BRANCH;
  - `git fetch origin && git show origin/main:<dispatch>`.

  A bundle runs on GPT-6.1-Sol high. Muse gets a plain read instruction (no tree, no push).
- **RETURNs.**
  - Codex coders push them to `codex/*` branches. The PM copies each `Plans/RETURN-*` file onto main; a one-liner
    over `git for-each-ref refs/remotes/origin/codex/` does it.
  - Muse's arrive as pasted text, which the PM banks verbatim.
- **Merge on green, then review.** A post-merge FAIL is normally fixed forward when a revert would reopen the
  original bug. Revert-first applies when main reddens.
- **Docs pushes** go from a local `pm-docs` branch tracking main: `git push origin HEAD:main`. Check
  `git branch --show-current` first. Pushing main from the gate branch is refused as a CI bypass.
- **Scheduled checks.** Use `send_later` to re-check runs. A `create_trigger` reminder failed to fire twice today, so
  always also check the run directly.
- **Lessons J.c. stated today:**
  - **Don't ask him to reorder relays he has already sent.** New relays go into a "next round" file, with no
    reshuffling.
  - He's away from his PC at times. Keep relays queued in one file, with blockers marked.
  - Plain language and EDT times in reports to him.

## 2. Batch 20: RED. Find the culprit first

- **Run `40898712909` at `942f20e0`** = main `ed56c60d` + #212 `c6e9b073` + #215 `0e21ab7f`.
  - **Shard 4 FAILED** (job `123540467242`, the "Run QEMU integration tests" step, after 72 minutes). Shards 1, 2
    and 3 passed. Shard 2 contains #215's `net_tick_retransmit` and `phase4_tcp_connect_stream_close`, which points
    further at #212.
- **The failing scenario isn't confirmed.** The log host (`productionresultssa15.blob.core.windows.net`) is blocked
  from the cloud session, and `get_job_logs` returns only the last ~5000 lines, which are the alphabetical tail of
  the serial dump.
  - That tail shows every scenario from `std_entropy` to `wpa3_sae` PASS, `tcp_listen_bounds` and `usb_hotplug`
    included.
  - Shard 4's batch-relevant scenarios are `tcp_listen_bounds`, `usb_hotplug`, `phase4_ndp_ipv6_neighbor` and
    `neofs_grow_write`. The partition was computed locally with `shard_partition` at `942f20e0`; the full lists for
    all 4 shards were 428 lines.
  - **So the prime suspect is `neofs_grow_write` (#212's new scenario)**, or its xtask host post-check
    `host_check_neofs_grow_write`. `phase4_ndp_ipv6_neighbor` is the other candidate before `std_*`.
  - **Confirm it** by asking J.c. to open the job page and search for `event=FAIL`, or ask S1 or S3 (who have QEMU)
    to run `cargo neo-test neofs_grow_write` and `phase4_ndp_ipv6_neighbor` at `942f20e0`.
- **Then:**
  - If it's `neofs_grow_write`: bounce #212 to S3, and re-gate #215 alone. Shard 2, with its scenarios, passed.
  - If it's #215-related: bounce it to S4.
  - Re-run only for infra.
- **Merge recipe once a batch is green:** use `prep.sh` per PR (§6) with the new gate SHA, squash with the
  expected head, and verify the code trees.
  - #215's head is `fb7bb615`: docs plus a merge of main; verify its code trees equal the gate's.
  - After #215 merges, fix its three "OT-98" net-lock mentions to **OT-97** (OT-98 is #219's kill-vs-exit).
- **#217 MERGED** as `6fc4b407` (host-only; push-CI green).

## 3. Every open PR: state and owner

| PR | What | Head | State | Owner, next |
|---|---|---|---|---|
| #212 | NeoFS grow-write fix (records take the grown size) | `c6e9b073` | **batch 20 red, shard 4; prime suspect** (§2) | confirm, then S3 |
| #215 | OT-95 net tick (IF-off) + net registries IrqSafeMutex | `fb7bb615` (gated `0e21ab7f`) | batch 20 red (shard 4, probably not #215); AAG passed | re-gate alone; then fix OT-98→OT-97 |
| #209 | OT-67 W1 SYS_FILE_REPLACE | `215024c4` | AAG bounce fixed (validate_user_buf, 64 KiB chunks, EIO) and PM-checked | **batch 21** |
| #213 | NeoFS refused-commit release + read re-check (stacked on #209) | `a8645341` | AAG bounce fixed (snapshot re-check) and PM-checked | **batch 21**, after #209 |
| #204 | OT-93 kill of Ready-in-syscall | `1138d919` (F2 fixed; reviewed `42be6f81`) | **Janus #149 FAIL:** F2 (wait completes before the survivor leaves the kernel) must be fixed; F1 (unbounded device wait) becomes OT-99, as on main | **S5** (taken over from S4; handoff clean) |
| #219 | OT-98 kill vs natural exit | `2de132e7` (rebased on #204; reviewed `82a7cffe`) | **Claude AAG FAIL-A/B + Janus #174 FAIL** (F1: a losing natural exit overwrites the exit code; F2 = FAIL-B, waiter in flight; F3: a false CP-91 claim on SYS_KILL); kill_at_exit **intermittent** at SMP=4 (S1 panic after ~50 rounds; Janus passed) | **S5**; gates with #204 in one batch |
| #216 | OT-66 L2b kernel lock route | `948979d9` | **Jarvis #160 FAIL:** items 1, 6 (input in flight across the chord), 7 (drop order), 10 (CP-97 wording) | **S2**; then Jarvis narrow re-check |
| #218 | OT-96 R1+R2 wire receive | `2c4a0f67` (hardened; reviewed `b2c8fe40`) | **Claude AAG FAIL** (forged DHCPACK, verified; RA unvalidated; uncapped tables; echo to group MAC) **+ Chad #173 FAIL** (IF-on takers `main.rs:542` and mt7601u DORA; unbounded `flush_wire_egress`; listener retirement; RA growth) | **S4**; then Chad re-checks the new head |

| #223 | C2.3a servo smoke Condvar check (Jarvis #156 fix, test-only) | `613f3551` | new | push-CI + light review; merge |
| #222 | OT-88 S1 settings WM side (atrium-settings-store) | `1de213d2` | new; close-gate passes | after #216; gate + AAG + cross-family |
| #221 | OT-61 pin charge (Muse #163 fix) | `38f8de07` | new; close-gate passes | gate + AAG + cross-family |
| #220 | OT-88 A1a app model (client table, probe app) | `9e46d426` | new; see S1's note in §4 | PM decides on #144's scope, then gate + AAG + cross-family |

Not yet a PR:
- **product-login** (S2, `claude/ot66-l1-product-login` @ `3a6e86d0`): **Muse #152 PASSED** (all 7 items;
  `Plans/RETURN-2026-10-29-ot66-l1-review-muse.md`). Have S2 open the PR. It adds `kernel-rng`, `sak-lock` and the
  WM's `login` to the product image, with a pairing test, which meets Muse's kernel-rng condition. It's
  kernel-touching (image features), so it goes in a batch gate; `sak-lock-route` joins after #216.
- **A1a** (S1, `claude/a1a-wip`): needs #144's design answers first.

## 4. The coder seats (cloud sessions on J.c.'s account)

| Seat | Session | Queue, in order |
|---|---|---|
| S1 | `session_01CycV6vuj65ph9XpLxgjqWj` | A1a (the app model). First amend the #191 THINK per Chad #144: blocking reply sends, death-local coverage, the 9-item wait set vs the cap of 8, futex wake exists, the stdio kill-switch, the open-with protocol. Then editor fixes (#145, #155). It also runs QEMU for S5 (S5 has none). |
| S2 | `session_01V22r3sW4DEBnWtNjWEDEkm` | #216 fixes. Then #127 (grant recipient's last Arc under CAP_GRAPH, plus the deferred drain). Then #153 (immutable tensor copied before EPERM). Then the settings WM side (`atrium-settings-store`). |
| S3 | `session_01PsTvTCdJ4yhqYKLPz2iw2E` | **#148 installer Back starts an install (P1).** Then quotas: #163 pins and #143 leaked leaf frame. Then #202 F1 (TCP listener retire vs admission), likely with Muse #168. |
| S4 | `session_01GH8i4JXihVETNAciDnL8xt` | #218 hardening. Then R3 (`claude/ot96-r3-dhcp-renew`, stacked; DHCP renewal, then net-tick + wire-rx into image A). Then #156 (vacuous Condvar check). |
| S5 | `session_015mvuCUwu62271uiYfe849R` | #204 F2 + #202 F2 (a wait returns only after the target can't act and its sockets are released). #219 FAIL-A/B + the SMP=4 panic. File OT-99 (device-wait cancellation). Then the power PR: #140 (GAS geometry) and #154 (HHDM overflow; the judge's duration). P3a waits on #204. |

**Seats' reported state at pause:**

- **S5** stopped with both trees clean and nothing unpushed.
  - **#204 @ `1138d919`.**
    - F2 is fixed: `DYING` holds a killed process until teardown is done and the last survivor is reaped, and only then
      completes its waiters and publishes EXITED. That also covers #202 F2.
    - Discriminator: `kill_ready_in_kernel` requires `wait_after_survivor_reaped=1`; the control is
      `neo-test-kill-wait-early-control`.
    - OT-99 is filed, and CP-95's wording is fixed.
    - Still to do: the PR body's pitfall table (CP-33, CP-85/86), the close-gate, and a QEMU run.
  - **#219 @ `2de132e7`, rebased on #204.**
    - The assertion is now `found_blocked || survivors > 0 || late > 0`.
    - FAIL-A: the sys_wait scan also pages `DYING`. FAIL-B: the wake happens under the waiters lock.
    - A late-survivor recount is added. New legs A and B, plus `-control-a` and `-control-b`.
    - **Still open:** Janus F1 (store the exit code only on a winning CAS) and F3 (SYS_KILL's outer IF-off wrap; it
      prefers handing the last Arc to the deferred drop). Also the CP-98 text, the PR body, the close-gate, and an S1
      QEMU rerun of kill_at_exit plus the three controls at SMP=4.
    - Its SMP=4 panic hypothesis is unconfirmed: a late-wake race, or a child on another CPU. It asked S1 for the
      ~120 lines before the panic.
  - **The SMP=4 panic, diagnosed (S5, from S1's logs at `claude/s1-kill-at-exit-logs` `ee0921ab`).**
    - It was the race leg, in a no-yield round. `terminate`'s claim won while the child was **Running on another
      CPU**, still on its way into `SYS_EXIT`.
    - The kernel-side killer (the harness) isn't pinned to the BSP, so the code comment "every user thread runs on the
      BSP, where this kill runs" is wrong.
    - With #204's survivor count (`current.pid`), this becomes a deferred kill, not a panic. That explains why
      Janus's run passed and S1's failed.
    - **Next: "leg C"**:
      - pin the harness to CPU 0 and the child to CPU 1;
      - hold the child Running inside `SYS_EXIT`, before `record_exit_code`;
      - `terminate` must count it as a survivor and defer;
      - on release, the waiter completes at the reap;
      - the control without the survivor count must panic.

      Then fix the comment, and do Janus F1 and F3.
  - **P3a @ `7ce04835`**, waiting on #204.
  - Then the power PR: #140 and #154.
- **S2 stopped with all trees clean.** Its messages arrived late and batched, so it finished the settings WM side
  before seeing the pause.
  - **#216** @ `948979d9`, unchanged. Jarvis's items 1, 6, 7 and 10 are **not started**; they're S2's first item next.
  - **Product-login** @ `3a6e86d0`: no PR yet. Muse #152 has passed, so **tell S2 to open it.** It gates in a batch.
  - **#222, the settings WM side** (`claude/ot88-s1-settings-wm` @ `1de213d2`, on main `4f0a8128`; close-gate passes).
    - `login_settings_store` PASS: two boots, live apply, store digests vs settings-core, refusals, the idle lock at
      the chosen time.
    - `login_settings_cut` PASS: 130 ops, 34 sampled cuts.
    - An OP_SETTING byte oracle. Kill-switch: off is bit-identical.
    - It needs the 4-shard gate, an AAG and a cross-family read. S2 parks it behind #216.
  - **L4** (`claude/ot66-l4-settings-hooks` @ `9aaacbc8`): parked. Its idle setter is superseded by #222's store.
  - **S2's queue:** #216 fixes, then #127, then #153, then #222 to the gate.
- **S4: #218 hardened to `2c4a0f67`.** All four of the PM AAG's blocking findings are fixed behind `wire-rx`, each
  with a QEMU falsifier and host KATs (an independent oracle in `gen_hardening.py`):
  - RFC 2131 `dhcp_reply_ok` (xid, chaddr, state, option 54);
  - `classify_ipv6` per RFC 4861 (hop limit 255, checksum, link-local RA source, destination filter); the raw
    `ndp::parse` fallback is removed;
  - caps: 16 v6 addresses, 16 routes deduped, 4 prefixes per RA, 128 ARP, 128 NDP, 8 v4 addresses;
  - group or zero source MACs dropped; ARP senders filtered;
  - three more scenarios added to the `compile_error!` list.

  With wire-rx off, the kernels are byte-identical to `b2c8fe40`.
  - **Possibly still open: Chad #173 F1 and F2.**
    - F1, IF-on takers: `main.rs:542` `virtio_net::service_all` after neo-init's reap; mt7601u's late-bind DORA
      loop.
    - F2: `flush_wire_egress` unbounded under a concurrent producer; virtio's uncapped used-index loop.

    S4's report didn't cover them, and I asked. Then Chad's narrow re-check runs on the new head.
  - Still open by design: SLAAC trusts any valid link-local RA. `wire-rx` will require `net-tick` after #215 merges.
  - **R3** `claude/ot96-r3-dhcp-renew` (now @ `8a6ad8dc`), stacked on #218 and #215, is not a PR yet. Its scenario passed
    5/5 and its regression 13/13.
  - **#223 opened (S4, `613f3551`, test-only, `servo_host.rs`):** Jarvis #156's fix.
    - The waiter marks itself parked under the mutex before the waker notifies.
    - The check also requires the wake to arrive within 5 s of a 10 s timeout, because `wait_timeout_while`
      re-checks the predicate and reported not-timed-out.
    - Falsified with notify removed. It needs only push-CI plus a light review.
    - S4 notes the same race in `threads.rs` check 11, for its own change.
  - **S4 confirmed that `2c4a0f67` does NOT address Chad #173 F1, F2 or F3, or the CP-56 row.** F4 (route dedup and
    cap) is done. #218's next round:
    - F1: `main.rs:542` IF-on `service_all`, and mt7601u's late-bind DORA loop;
    - F2: unbounded `flush_wire_egress`, and virtio's uncapped used-index loop;
    - F3: listener retirement, coordinated with S3's #202 F1 fix;
    - the CP-56 virtio TX-header row.

    Then Chad re-checks.
  - **R3** is now @ `8a6ad8dc` (a comment-only commit). S4's trees are all clean, and S4 has stopped.
- **S3 opened #221, the pin charge** (`claude/ot61-pin-charge` @ `38f8de07`, on main `18eba9c2`), the fix for Muse
  #163.
  - Each `SYS_BUFFER_MAP` charges `obj_weights::PIN` (128 B, checked by a KAT) inside `map_borrowed_region`, under
    `inner`, so a force-killed syscall can't leak the charge (CP-30/65). It's released once, at Pmap drop, and
    refused with ENOMEM before any change.
  - `MAX_BORROWED_PINS = 1024` per Pmap, in every build.
  - Scenario: Muse's input (a 64 KiB budget, 5000 maps) gets ENOMEM; falsified with weight 0.
  - Kill-switch: identical `.text` except the cap delta in desktop and image A.
  - **Deviation:** pins are not released on unmap (unmap never removed pins; no userland unmaps). PM: acceptable,
    stated in the PR.
  - It needs the 4-shard gate, an AAG, and a cross-family read.
  - **S3 also found:**
    - #203's kernel `.text` rows had been measured from a shared target dir. Re-measured: they hold.
    - `34c6c132` (#202's merge) is unbuildable on its own: neo-init names `neo-atrium?/install-chip` before
      neo-atrium has it. It only matters for bisects.
  - **S3 stopped with its tree clean.** Next session, in order:
    1. **#148 F1/F2** (not started, P1), as its own PR.
    2. #143, the leaf-frame guard, with a physical-frame discriminator.
    3. #202 F1, the TCP listener retirement flag, with Muse #168 if it is the same bug.
  - **Tip for comparing `.text` across two trees:** give each tree its own target dir. xtask bakes its workspace root
    in at compile time.
- **S1** opened **#220, A1a** (`claude/a1a-apps` @ `9e46d426`, on main `fd73b477`; close-gate passes).
  - It adds the client table (MAX_CLIENTS 6, Arthur in slot 0), per-client namespace and provenance, the probe app
    behind `atrium-apps`, per-client turns with ≤8 requests each, Super+Tab, and per-client faults. The WM closes and
    waits rather than killing (OT-98).
  - QEMU SMP=4: `desktop_apps_boot` and `desktop_apps_fault_boot` PASS, both falsified. With the switch off, all 23
    desktop scenarios PASS and 18 captures are identical to main. `.text` grows +3072 off and +13936 on.
  - **The PR lists as not done:** a client hung in its handshake still blocks the WM (#144 item 1), and the loop
    polls when other clients are live.
  - **PM to-do before gating:** decide whether #144's items 1–3 must be solved in A1a or may ride to A1b/A2. The
    handshake block is the same CP-57 class as #216. Was the THINK amended (a docs PR)? Check.
  - It's kernel-touching only through test-harness features, so it needs the 4-shard gate, a cross-family read and
    an AAG.
  - **S1's own verdict (pause report): #220 is A1a's skeleton and must not merge as "A1a done"** until #144's five
    points land. S1 had not read #144 before the pause.
  - **S1's next, in order:**
    1. A docs PR amending the #191 THINK per #144, with F0's AAG made explicit.
    2. Rework #220: an async or bounded handshake and reply; client-local comms, protocol and launch arms with
       non-blocking cleanup; a wait-set budget; disconnect-during-reply and Lens-failure legs.
    3. The E1 fix PR (Jarvis #155), then the THINK corrections from Janus #145.
    4. Re-stack #213 once #209 merges: `rebase --onto origin/main 215024c4`, then close-gate and push.
  - **S1's other branches:**
    - `claude/a1a-wip` @ `25092f2a`: superseded, deletable.
    - `claude/ot98-repro` @ `4ae388fc` (same as `ot97-repro`): the repro, not for merge.
    - `claude/s1-kill-at-exit-logs` @ `ee0921ab`: the #219 SMP=4 panic serial logs under `Plans/evidence/`, for S5.
      Not for merge.
  - All S1 trees are clean. #220's push-CI on `9e46d426` finished green.

## 5. Relays

- **Sent by J.c. today and returned:** everything in `relays-now.md`, except Muse's rate-limited ones. All RETURNs
  are banked on main (`0dfc91d7`). The FAIL routing is in the ledger entry "Codex RETURNs read and routed".
- **#173 and #174 were sent and returned (both FAIL), as were Chad's bundles b18 and b19:**
  - **PASS:** #164, #166.
  - **#165 FAIL:** obj-quota coverage is partial, and the emptied handle table's capacity is uncharged (S3's quota
    lane).
  - **#171 FAIL:** an in-flight wait has no claim pin against revoke, and a failed inherited table leaks claimant
    counts, so the exit code is kept forever. Not assigned yet; S5's exit/wait area is the natural owner.
  - **#170 FAIL:** listener retirement, the same as #202 F1 (S3).
  - **#172 PASS** on lock transitions. Its known residual (the WM blocks on client sends) is #144's item, which S1
    designs for in A1a.
- **Next: narrow re-checks** of #173 and #174 on the fixed heads, plus #149 (Janus) and #160 (Jarvis).
- **Post-merge, still to write:**
  - batch 20: #212, #215;
  - narrow re-checks after each bounce is fixed: Janus on #204, Jarvis on #216, Chad on #218, and the AAG-found fixes
    in #209 and #213.
- **Muse:**
  - Owed by J.c.: the FAIL texts for **#161** (L2b, item 2), **#157** (socket teardown, item 3; likely matches Chad's
    #158 F1), **#167** (#210, item 3: a minted ACTIVATE survives revoke, **product security**) and **#168** (#211,
    item 3: a forged RST frees a half-open connection).
  - **Muse's usage limit is also spent (J.c., 10/29 afternoon). Expect nothing from Muse until it resets.** Her
    retries were cleared to run serially once it does: **#152 first** (it gates login in the product build), then
    #150, #151, #133, and #134 items 4–5.
  - There is no fallback for Claude-authored security reviews: the Codex models refuse them (CLAUDE.md). So
    product-login and every post-merge Muse review in the relay files wait. Don't route those reviews to Codex.
  - **Banked:**
    - #162 PASS;
    - #163 FAIL (pins; S3 fixed it in #221);
    - #152 PASS (L1 login);
    - **#151 PASS** (K1a follow-ups; J.c. verified the key claims himself;
      `Plans/RETURN-2026-10-29-k1a-followups-review-muse.md`).
  - **#133 was running at pause.** Still to run: #134 items 4–5 and #150.
  - **Still owed as full text:** the FAILs #161, #157, **#167 (product security)** and #168, and the PASSes #132,
    #137, #138, #147, #169 and the partial #134.

## 6. PM tools (the old scratchpad dies with the session)

`prep.sh`. Set the two gate SHAs and the batch name before each batch:

```bash
#!/bin/bash
# usage: prep.sh <branch> <expected-short-sha>
set -e
cd /home/user/LogansOS
b=$1; exp=$2
git fetch -q origin main +refs/heads/$b:refs/remotes/origin/$b
head=$(git rev-parse origin/$b)
case $head in $exp*) ;; *) echo "HEAD MOVED: $b is $head, expected $exp"; exit 1;; esac
git checkout -q -B m-$b origin/$b
if ! git -c merge.conflictstyle=diff3 merge -q --no-edit origin/main >/dev/null 2>&1; then
  for f in $(git diff --name-only --diff-filter=U); do git checkout GATE_SHA -- "$f" 2>/dev/null || git rm -q "$f"; done
  git -c core.editor=true commit -q --no-edit
  echo "resolved conflicts from GATE_SHA"
fi
if [ "$(git rev-parse HEAD)" != "$head" ]; then
  git commit --amend -qm "Merge origin/main into $b (batch N gate GATE_SHA)"   # add the session's attribution lines
  git push -q origin HEAD:$b
fi
git rev-parse HEAD
git checkout -q pm-docs
```

`u3.py`, the union resolver for additive diff3 hunks. Run it after `git -c merge.conflictstyle=diff3 merge`. It is
wrong for `cfg(any(...))` lists, which you resolve by hand.

```python
# Resolve diff3 conflict hunks as: (ours lines not in base) + theirs lines.
import sys
for p in sys.argv[1:]:
    out=[];st=None;o=[];b=[];t=[]
    for line in open(p).read().split('\n'):
        if line.startswith('<<<<<<< '): st='o';o=[];b=[];t=[];continue
        if st and line.startswith('||||||| '): st='b';continue
        if st and line=='=======' : st='t';continue
        if st and line.startswith('>>>>>>> '):
            out+= [l for l in o if l not in b] + t; st=None; continue
        if st=='o': o.append(line)
        elif st=='b': b.append(line)
        elif st=='t': t.append(line)
        else: out.append(line)
    assert st is None, p
    open(p,'w').write('\n'.join(out))
    print('resolved',p)
```

Fetch PR heads with a forced refspec (`git fetch origin +pull/N/head:prN`). A plain fetch after a rebase leaves a
stale local ref. Take full SHAs from `git rev-parse`, never typed by hand (#192 merged with a mistyped tail; the content
was verified equal).

## 7. Open threads and pitfalls added today

- **OT rows:**
  - OT-91: exit codes (#214, merged).
  - OT-92: the NeoFS cache never evicts (held).
  - OT-93: #204.
  - OT-94: TCP listen bounds (#211, merged).
  - OT-95: the net tick (#215).
  - OT-96: wire RX (#218).
  - OT-97: the net locks become IrqSafeMutex by construction (#215's row).
  - OT-98: kill vs exit (#219).
  - OT-99: device-wait cancellation (S5 files it).
- **CP rows:**
  - CP-94: heap growth adjacency;
  - CP-95: kill of Ready-in-syscall;
  - CP-96: an authority checked only by a self-mintable right (#210);
  - CP-97: the lock route (#216; its wording is being corrected);
  - CP-98: a kill that assumes its target is Ready or parked (#219; add FAIL-A/B);
  - CP-37: corrected in both directions. Mixing IF-on and IF-off takers of one plain spin lock deadlocks.
- **Product facts learned today:**
  - The product kernel keeps **debug assertions and overflow checks on**: dev profile at opt-level 3
    (`tools/xtask/src/main.rs:5751`). Any `debug_assert!` reachable from userland is a product panic.
  - Nothing ticks the net clocks in a `wire-rx` build without `net-tick`. S4 is making `wire-rx` require it.
- **J.c.'s decisions today:**
  - side-by-side windows, floating with snap;
  - #191 D2–D5 and #192 E-D1–E-D5 as recommended;
  - C2.3b vendors the html5ever/selectors closure and builds first on CI runners;
  - login in the product after Muse #152;
  - **IPv6 after the MVP** (S4's §12 scope: R6/R7 behind `wire-rx-v6`), unless he says otherwise.

## 8. Standing constraints (unchanged)

- Never write the Zenbook's internal (Windows) NVMe. Images go only to the KINGSTON stick.
- No model identifiers in commits.
- Never close or reopen PRs, or push empty commits, to kick CI. Never skip or disable tests.
- Merge holds come only from J.c. Never merge `acpi-drop/`.
- Keep LAN addresses and admin accounts out of the repo.
- Exact bytes come from raw sources, never a summarizing fetch. GPL code is read for principle only.
