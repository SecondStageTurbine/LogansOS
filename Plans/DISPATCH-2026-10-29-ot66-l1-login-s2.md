<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-66 L1, credentials and the login screen → S2

**Why.** Login is an MVP (0.5) item. L0, the design, is done:
- `docs/THINK-2026-10-27-ot66-login-session.md` merged as `ee5df102`, after Chad's re-check (#117);
- J.c. decided §10 rows 2 and 4 on 2026-10-29: the recovery code, and a 10-minute idle lock on by default (the lock
  itself is L2).

L1 is the next rung. It's userland only, plus xtask (§8).

## Scope: the §8 row L1, as written

1. The `NEOCRED1` reader and writer (§4.3): two 160-byte files, `account.cred.a` and `account.cred.b`, and
   `account.meta`.
2. The KDF through vault-core's Argon2id entry point (§4.1), no new crate, with the floor at
   `KdfParams::INTERACTIVE_V1`. A below-floor record is refused. Add the `LOGIN kdf params=… elapsed_ms=…` line.
3. The first-boot account screen (§7), which shows the recovery code once.
4. The login screen (§5). Wrong password, the 2ⁿ⁻³ s backoff after the 3rd failure, capped at 60 s, with the counter
   in neo-init's memory (§4.4).
5. The recovery code: 20 symbols from a 32-symbol alphabet, drawn from `sys_getrandom`. Use it, then set a new
   password, and a new code is issued.
6. `.system/` seeded by `neofs-mkfs` for `xtask vm-image`. The OT-87 seed comes later, in S3's lane.

L2 (lock and unlock) is **not** in this dispatch.

## Re-check the NeoFS claim against today's main

§4.3's cut guarantee rests on one property: **a write to one file never rewrites a block belonging to another
file.** The THINK cites `inode.rs` and `cache.rs` line numbers from before #180 (the NeoFS barrier, `c805f2b2`) and
#187 (the allocator move, in batch 15). #180 added a flush barrier before the superblocks, and it defers reuse of
freed blocks until the freeing generation is durable. That should only strengthen the property. Verify it on the
code as it is when you build, cite the current `file:line` in the PR, and update §4.3's citations.

## The oracles (CLAUDE.md, "Format/ABI specs")

- **`NEOCRED1`:** one hand-laid 160-byte example of each file, with a known password, salt and parameters. Cover:
  - a valid pair;
  - a pair whose newer file is torn;
  - a below-floor slot.
  
  The verifier bytes come from the P-H-C reference `argon2` CLI at a pinned commit, the vault-core precedent
  (`userland/vault-core/src/lib.rs:208-209`). Fetch the source with `git`, build it, run it, and paste the output as
  literal bytes. Never take bytes from a summarizing fetch.
- **The KDF:** the RFC 9106 Argon2id vector through the shared entry point. Take the vector from the RFC's raw text.
- **The backoff schedule:** a KAT.

## The scenarios (4-shard gate), on the R1 VM layout, two boots

- Boot 1 creates the account. Boot 2 shows the login screen: a wrong password is refused, and the right one starts
  the session.
- The recovery code resets the password.
- A below-floor record is refused.
- **The cut:** a crash after each write and each flush of a password change. Each case leaves exactly the old or
  the new password accepted, and the untouched file's BLAKE3 is the same before and after. Extend the
  `phase3_neofs_crash_mid_overwrite_recovery` hook to every step.
- **The kill-switch:** with `login` off, boot goes straight to the desktop, bit-identical to today, with the same
  scenario output. Give the neo-init `.text` identity with `login` off.

## Timing on hardware: an acceptance item, not a merge gate

The §4.2 timing lines from the Zenbook (image A) and the OptiPlex decide the shipped parameters. They need
bare-metal boots, so they're an acceptance milestone after merge (CLAUDE.md, "Merge bar vs acceptance bar"). The
merged default is the floor, `INTERACTIVE_V1`.
- A Zenbook timing boot must not create an account or write any disk. It only runs the KDF once and prints the
  line. Put that boot behind a test feature that has no disk sink. The Zenbook's internal NVMe is never written.
- The PM schedules the hardware runs with J.c.

## Prior art

THINK §11 has the survey: licences, last commits, and principle only. If L1 uses anything new (a crate, a
password-field widget, an alphabet table), add a row to the PR's survey table with its licence and last commit.

## Rules

- `scripts/worker-close-gate.sh` and one PR, branch `claude/ot66-l1-login`. It's userland and xtask, but it's in the
  image, so the PM runs the 4-shard gate.
- The PR body carries:
  - the oracle table;
  - the cut-scenario results per step;
  - the kill-switch `.text` table;
  - the NeoFS re-check with its current `file:line`;
  - the prior-art rows, if there are any.
- Review after merge: **Muse** (§9: credential storage, the KDF floor, the recovery path and the backoff, all
  Claude-authored). The PM writes the relay.
- Commit and push at every checkpoint. Send the PM the head SHA from `git ls-remote`.

```
WORKTREE: cloud session S2 (your own clone)
BRANCH:   claude/ot66-l1-login
          git fetch origin && git checkout -B claude/ot66-l1-login origin/main
git show origin/main:Plans/DISPATCH-2026-10-29-ot66-l1-login-s2.md
git show origin/main:docs/THINK-2026-10-27-ot66-login-session.md
```

RAG isn't available in the cloud session. Before ending, `git status --porcelain` and `git log @{u}..` are both empty.
