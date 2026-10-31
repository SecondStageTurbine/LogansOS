<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# EVIDENCE FIX-FORWARD: MT7601U `f4a0940` Lane C

**Date:** 2026-07-25
**From:** PM
**To:** Prometheus
**Candidate:** `f4a0940872e865207128a240587f7cac036f0cf9`
**Parent:** `8ab6409fe711e531c744d0296f8855beab7b7b3a`
**Branch:** `codex/mt7601u-post-qselect-terminal-jarvis`
**Status:** **EVIDENCE-ONLY BOUNCE. Candidate code remains frozen.**

Do not change the candidate, append a commit, amend, rebase, force-push,
dispatch a workflow, build with credentials, boot hardware, reset a device,
open a PR, or merge.

Oracle Lane A and Cerebro Lane B remain `TOKEN-OK`. This bounce does not
invalidate their code reviews. It corrects only the incomplete Lane C proof.
The four-shard and silicon holds remain in force.

## Why the Lane C return is not accepted

The returned `TOKEN-OK` is unsupported in four material respects:

1. It says the dispatch's two pinned patch IDs were incorrect, but independent
   PM recomputation confirms they were correct.
2. Its "Corrected Patch IDs" table contains commit pairs but omits the actual
   stable patch-ID values.
3. Its OFF-equality table gives only one size per configuration. It omits both
   sides' SHA-256 values, both sizes, and byte-comparison results.
4. Its ISO result is hypothetical ("would be byte-identical") and states that
   the firmware blob was absent. That is not inspection or reproduction of
   Jarvis's actual blank-credential ISO, which must contain the MT7601U
   firmware module.

Source-level gating is also insufficient for the requested artifact-level
structural-absence proof.

## 1. Immutable identity

Re-fetch and prove:

- remote branch tip equals the candidate;
- candidate parent equals `8ab6409fe711e531c744d0296f8855beab7b7b3a`;
- child own-stat is exactly five files, `+602/-111`;
- the paths are exactly:
  - `kernel/src/drivers/usb/mt7601u/linux_parity_logic.rs`
  - `kernel/src/drivers/usb/mt7601u/mod.rs`
  - `kernel/src/test_harness.rs`
  - `tools/ai-ops-tests/src/lib.rs`
  - `tools/ai-ops-tests/tests/mt7601u_hostile_oracle.rs`
- the detached evidence worktree is clean before and after the work.

## 2. Correct stable patch-ID ledger

Recompute with `git patch-id --stable` and return the full pair table. The
expected values independently computed by the PM are:

| Frozen commit | Replay commit | Stable patch ID |
|---|---|---|
| `9ff50e15502b346615c84e7c0c54f2499cd21469` | `e71d164515cd5ab815adad0af0454e5128f59c13` | `2852b75b9b2bc5e8fdf9f85c1354aa7b331548cd` |
| `078a60f4c4d0d5688ec8021d262b1aa50c6ad202` | `a9f0e68ae3d5aaa1d2f4e1be79eafda382b4e11e` | `725617769222ce0664cd1a59745a11f809e769a2` |
| `162cd3d40e7cadc2953fd074d9947c25336b05d0` | `ec4d4b4e9ece861db943e3afd6e570e0db902454` | `75aa8080f32c855fc4ec3f804e434464fa7342ef` |
| `ff01516e3766b2c86dd56eeece1c5fbeb6e8cc4c` | `3543864499e3a7fb9423c58efbd88fed5923654f` | `23bdc6aed5059a82eec4a242b36189f777fa5279` |
| `1e58f14bed0abd97fddc9ace3c583abb30f6d0de` | `0bf4af7bf374c0bd48940b67e3d8a8cf8a8d581b` | `c9f7f6d02d037b42159649b26269862e06f96be3` |
| `6f1391047550c18584d4f05b2c79bab6799d58a9` | `958efb54bc12515df383856bcef3023e15af7620` | `580d79c4e3593efbdcb013952efa0f72bd917e4f` |
| `7ca6fbe039b3af17d87188999e1c4f5a71be560d` | `45fcab57f8b5fa87620e23ae621a6decd1520af1` | `6cc2b3776e16acb9d0575546c93a1cac94f402f1` |
| `625e531f353b417498a968dfa0b74f1d45c0e592` | `22776a71678b8d390472d2e5923304a6c20e881d` | `fa91766d1a64e83dca7344d2e9757b1a5fa02070` |

Also return the unique child patch IDs:

- `8ab6409fe711e531c744d0296f8855beab7b7b3a`:
  `f170149eb42f9695e2da01aa8cd56fb1850c4e71`
- `f4a0940872e865207128a240587f7cac036f0cf9`:
  `7a8757636616f07926c152a2e84135c50519cc3a`

All ten commits from the frozen base through `f4a0940` must have unique stable
patch IDs.

## 3. Credential and channel proof

Return exact file/line evidence that:

- SSID is `b""`;
- passphrase is `b""`;
- `CHANNEL=0`;
- neither local credential string occurs in the committed candidate.

Do not create a credential-filled build.

## 4. Artifact-level structural absence

For all three candidate OFF artifacts below, inspect the actual normalized ELF
with symbol and string tools. Prove zero occurrences of the new MT7601U
post-`Q_SELECT` ledger, terminal, fence/gate, and Linux-parity structures when
`mt7601u` is disabled. Report the commands and match counts; do not substitute
source-level `cfg` reasoning.

Configurations:

1. workspace default;
2. `--no-default-features --features usb-host`;
3. `--no-default-features --features usb-hid`.

## 5. Reproducible normalized OFF equality

Build baseline `0be953de11695ced27ef86dfe9b4e06f2abfd18a` and candidate
`f4a0940872e865207128a240587f7cac036f0cf9` in separate clean, isolated target
directories with:

- `CARGO_INCREMENTAL=0`;
- one fixed neutral `NEO_BUILD_ID` for every row;
- the repository-pinned Rust toolchain;
- `RUSTFLAGS` including `-Z location-detail=none`;
- the exact three configurations listed above.

Normalize each kernel ELF with the same identified `llvm-objcopy --strip-all`.
Demonstrate that every output remains a valid ELF and that every baseline and
candidate `SHF_ALLOC` section is retained by the normalizer.

Return a six-artifact matrix containing, for every configuration:

- baseline normalized size and SHA-256;
- candidate normalized size and SHA-256;
- `cmp`/byte-equality verdict.

Include exact build commands, toolchain identity, `llvm-objcopy` identity,
normalization commands, and the ELF/section-retention checks. Digest envelopes,
text manifests, or stripped non-ELF surrogates are invalid.

## 6. Actual blank-credential ISO proof

Do not report what an ISO "would" contain. Either independently reproduce the
exact blank-credential candidate ISO in the authorized build environment or
inspect Jarvis's immutable artifact and establish its provenance.

The claimed artifact is:

- size: `126,660,608` bytes;
- SHA-256:
  `feadfdafdf24d110ccc967d14a53c0b2649b3b5ecc1d61e686ab03b402fb1149`.

Return:

- actual size and SHA-256;
- proof the kernel payload is from exact candidate `f4a0940`;
- ISO PVD volume, publisher, application, system, and preparer identities;
- proof the MT7601U firmware module is present with its expected size;
- proof SSID/passphrase placeholders are blank and `CHANNEL=0`;
- confirmation no local credential bytes occur in the ISO.

"Blank ISO" means blank WiFi credentials. It does not mean firmware absent.
If the exact artifact is unavailable and cannot be reproduced, return
`BOUNCE` rather than a hypothetical equality claim.

## Return contract

Return one evidence report with:

1. immutable identity and clean-worktree proof;
2. the complete corrected patch-ID ledger;
3. credential/channel proof;
4. artifact-level structural-absence commands and counts;
5. the complete six-artifact normalized equality matrix;
6. actual ISO identity, provenance, firmware, credential, size, and hash proof;
7. `TOKEN-OK` or `BOUNCE`.

This work is evidence-only. No code commit, candidate push, close-gate rerun,
workflow dispatch, hardware action, PR, or merge is authorized.
