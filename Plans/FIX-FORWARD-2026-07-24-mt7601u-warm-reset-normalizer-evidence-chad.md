<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# FIX-FORWARD: MT7601U warm-reset OFF normalizer evidence -> Chad

**Date:** 2026-07-24
**From:** PM
**Candidate:** `fb2830d432ae5bf63214d69fb6541af4f599761f`
**Branch:** `codex/mt7601u-warm-reset-fresh-base-chad`
**Base:** `856874182b88679462f1a9d206726cfa3ff5eb1f`
**Push CI:** `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30139397576`
**Controlling disposition:** `Plans/DISPOSITION-2026-07-24-mt7601u-625e-gate-green-reset-prerequisite.md`
**Verdict:** **Code/gate evidence accepted for independent review. OFF artifact evidence BOUNCED. Preserve the candidate; return an evidence-only correction.**

## Accepted evidence

The PM independently verified:

- the remote branch resolves exactly to the candidate;
- merge-base is the named base, with the candidate ahead by two and behind by
  zero at return time;
- the cumulative original actuator patch and fresh-base replay have stable
  patch ID `8503eef83ba62c37e8ea809d2d1c497e2fbe5e1b`;
- the sibling-discovery child and its replay have stable patch ID
  `56c1d5b755d11697215ecbca3cfd7c488589bc61`;
- all seven production blobs are identical between the reviewed and replayed
  states;
- the branch touches exactly the seven returned paths;
- the focused FADT family reports 8 passed, 0 failed;
- the exact qualification feature combination compiles;
- the uninterrupted close gate passed on the then-current base;
- push CI run `30139397576` is attempt 1, event `push`, exact head SHA, with
  host/format/lint, SPDX, and monolithic identity successful.

No new production recut is required by this finding. Keep the candidate SHA
immutable.

## Artifact finding

The returned file named `kernel.off.normalized` is not a normalized kernel
artifact. It is a 294-byte text envelope containing four SHA-256 digests:

- a disassembly manifest;
- a normalized symbol manifest;
- printable data strings;
- section sizes.

Normalizer identity:

```text
E:\AI Automation\AltimitNeo-warm-reset-off-matrix-5ea8cc8e\normalize-off-elf.sh
sha256 ef82fac0dec2cd21cb5f65e33856b6ef9b20be43ccc4b6c584001b6831222471
```

That envelope omits non-printable payload bytes in allocated data sections.
Its equality therefore cannot establish artifact byte equality.

The normalizer leaves an intermediate `kernel.normalized.strip-all`. The PM
hashed those actual stripped ELFs and found every returned row unequal:

| Row | Bytes per side | Baseline SHA-256 | Candidate SHA-256 |
|---|---:|---|---|
| default | 2,984,376 | `52d38e8c3a5cabd403a39d0699db39f5ddb14f41512aa45b1b295a53a4ab1ee3` | `93c16af3a3161ae7d80106923a6dec1c51f1255e1dfd1346c73a3bc1a1b9bb92` |
| minimal/no-MT | 2,871,936 | `2166b5bffbc69fdc30d3f6aa2c81c2440a1349cea2a4f574d9d74ab6ce1c05ed` | `af5d145588539ca335faed6d0255c534bf8f7b1bfb04f2f727a0844c56283217` |
| USB host | 3,023,000 | `551f46897ce73a70635f0fac895c06cb5eb8636450f4ec7a69458b56150b385a` | `20c01c109cfd08ba2bd040e690bc17d37861225537dd22aabbd4d3cb7bbddd81` |
| USB HID | 3,043,992 | `616aa2dd012563dee886c5319ed62f07b7fa29dc9b160fdaed0608ca6c509176` | `c479f1135364080acb3c57c7497cbeac32ddcf3f9398d6143346421da37ba2e1` |
| QEMU IPC | 2,881,256 | `3b4bfa4e285faa6dcc2cc664dc8b72e97f2e366ffb41b0a17758b1f45758f9c7` | `589caf4a49344d53d6c6b1fbc7482600ec16308fb4f00503655df6a7f4fd908f` |
| USB bare metal | 3,053,632 | `575dcba77abea7f7fcbef4486dc81e0b3a4e0f9e7b8ed5a1b5980d1b1dfed7af` | `1ba4c9aaad47373d1415535c42447686bea745bf056a552f9ab09461c4b40281` |
| MT7601U association, reset OFF | 3,670,264 | `1e5d933fef6cfcb776f7cba3e62fbd53fabd7b796cacc842c257514a7c4c577a` | `f2ec51b8aecb99ffa4fc1897a40b54fa52bbf89a1e2c8a3336427526537e1e74` |

For the default row, `.text`, `.rodata`, requests, relocations, GOT, and
dynamic sections are byte-identical, but allocated `.data` differs at 30
bytes. The summary normalizer did not hash the complete `.data` payload.
Those differences may be source-location metadata caused by line movement,
but that is a hypothesis, not permission to discard an allocated section.

## Required evidence-only correction

Do not change, amend, rebase, replay, or push the candidate. Rebuild the same
seven rows against the same base and candidate SHAs using clean isolated
targets and identical inputs.

Use:

- the pinned toolchain;
- `CARGO_INCREMENTAL=0`;
- one fixed `NEO_BUILD_ID`;
- identical source and target remaps;
- `-Z location-detail=none` on both sides to neutralize compiler-generated
  panic source locations;
- a standard ELF normalizer that strips symbols/debug/non-allocated metadata
  but preserves every `SHF_ALLOC` section and its payload.

The normalized output must remain an ELF kernel, not a digest envelope. A
minimal accepted form is `llvm-objcopy --strip-all` or GNU
`objcopy --strip-all`, optionally removing specifically named non-allocated
metadata sections. Return the exact command, tool version/hash, `readelf -SW`
proof for removed sections, normalized size, paired SHA-256, and direct
`cmp -s` result for all seven rows.

If any normalized pair remains unequal:

1. identify every differing section;
2. classify whether it has `SHF_ALLOC`;
3. map differing bytes to their generating symbols/data;
4. stop and return the evidence.

Do not add a transform that deletes, hashes, sorts, or canonicalizes an
allocated section merely to obtain equality. No symbol/string-only argument
supersedes this requirement.

No new close-gate or push CI is required if the candidate remains untouched.
Return the evidence paths and a clean/remote-parity check.

## Parallel Janus review

Janus may review exact SHA
`fb2830d432ae5bf63214d69fb6541af4f599761f` now. Return the code/actuator
verdict separately from the artifact verdict. The PM will accept the reset
lane only when both are `TOKEN-OK`.

No workflow dispatch, Jarvis integration, credential ISO, hardware reset,
boot, merge, or PR is authorized.
