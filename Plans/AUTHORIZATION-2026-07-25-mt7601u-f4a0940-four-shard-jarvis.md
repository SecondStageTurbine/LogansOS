<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# AUTHORIZATION: MT7601U `f4a0940` exact-SHA four-shard gate

**Date:** 2026-07-25
**From:** PM
**To:** Jarvis
**Candidate:** `f4a0940872e865207128a240587f7cac036f0cf9`
**Branch:** `codex/mt7601u-post-qselect-terminal-jarvis`
**Parent:** `8ab6409fe711e531c744d0296f8855beab7b7b3a`
**Push CI:** `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30159747107`
**Verdict:** **SINGLE-USE RUNNER TOKEN GRANTED FOR THIS EXACT SHA ONLY.**

This authorization covers one four-shard `workflow_dispatch` of
`.github/workflows/ci.yml`. It does not authorize a code change, amendment,
rebase, force-push, credential-filled build, hardware boot, reset action, PR,
merge, or rerun.

## Accepted review state

The PM accepts:

- Oracle Lane A: `TOKEN-OK`;
- Cerebro Lane B: `TOKEN-OK`;
- Prometheus Lane C v2: evidence substance accepted after independent PM
  verification.

The PM independently reconfirmed:

- remote branch tip equals the candidate;
- candidate is exactly one child of the named parent;
- child scope is exactly five files, `+602/-111`;
- committed SSID/passphrase placeholders are `b""` and `CHANNEL=0`;
- push CI `30159747107` is exact-SHA, attempt 1, green;
- the runner queue and in-progress set were empty before this authorization;
- all three isolated normalized OFF pairs are valid ELF files and
  byte-identical:

| Configuration | Baseline and candidate size | Baseline and candidate SHA-256 |
|---|---:|---|
| default | `3,139,424` | `6e1a3c1adffaf4fd79db542a5d5158ef9a02884385663fdd30282fb4c6156f86` |
| `usb-host` | `3,164,712` | `e9a87f553e144de6426cb4bcb258d2a60828b7f10d8d9abc7c6c797c6007e77c` |
| `usb-hid` | `3,185,208` | `eb7bcf2f73eb320c3c7068ce6518a1341157a2899e446cb92597e21b586812a9` |

- all three candidate OFF artifacts contain zero `mt7601u`,
  `operation_budget`, `linux_parity`, `post-qselect`, or `post_qselect`
  string matches;
- Jarvis's actual blank-credential ISO is `126,660,608` bytes with SHA-256
  `feadfdafdf24d110ccc967d14a53c0b2649b3b5ecc1d61e686ab03b402fb1149`;
- that ISO contains `/modules/mt7601u-fw`, exactly `45,412` bytes with
  SHA-256
  `4511b1d840e22aea2bf5fdca419c91c0d94cbfb291b9ac4f8be6d9100d1a7046`;
- ISO PVD identity is volume `ISOIMAGE`, blank publisher/application/system,
  and preparer `XORRISO-1.5.6 2023.06.07.180001, LIBISOBURN-1.5.6,
  LIBISOFS-1.5.6, LIBBURN-1.5.6`;
- neither local SSID nor passphrase occurs in the ISO.

The PM also recomputed the stable patch-ID ledger with Git
`2.55.0.windows.3`; it reproduces the values pinned in the Lane C
fix-forward exactly. Prometheus's unshown alternate values and proposed
"Git-version discrepancy" are not adopted as governance evidence. Pairwise
content equivalence and the ten-commit uniqueness property are independently
confirmed, so this reporting disagreement is non-blocking for the gate.

## Dispatch procedure

Before firing, run:

```powershell
git fetch origin
git ls-remote origin refs/heads/codex/mt7601u-post-qselect-terminal-jarvis
git status --porcelain=v1
```

Stop unless the remote ref resolves exactly to
`f4a0940872e865207128a240587f7cac036f0cf9` and the candidate worktree is
clean.

Fire exactly one manual run:

```powershell
gh workflow run ci.yml `
  --repo SecondStageTurbine/NeoPlutarch `
  --ref codex/mt7601u-post-qselect-terminal-jarvis
```

Immediately identify the new run and prove:

- event is `workflow_dispatch`;
- head branch is the authorized branch;
- head SHA is the exact candidate;
- attempt is 1.

Do not amend, rebase, commit, or push after firing. Do not rerun any job or
workflow if a shard fails or times out.

## Return contract

Wait for the run to complete, then return:

1. the exact GitHub Actions run URL and database ID;
2. event, attempt, head branch, and full head SHA;
3. result for shards 1, 2, 3, and 4;
4. result for host tests/format/lint, SPDX, and monolithic identity;
5. overall workflow result;
6. unchanged local and remote candidate tips and clean worktree proof;
7. confirmation that no rerun, code change, push, credential build, hardware
   action, PR, or merge occurred.

Green requires the exact SHA and every required job to succeed in this single
attempt. A green result is software-gate evidence only. The PM must
independently verify the run before deciding whether to authorize another
credential-filled ISO recut or hardware boot.
