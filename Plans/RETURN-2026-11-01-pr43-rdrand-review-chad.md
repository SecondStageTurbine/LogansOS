<!-- SPDX-License-Identifier: CC-BY-4.0 -->

Relay 258 — Chad — PR #43 RDRAND review, 2026-11-01.

Reviewed head: `06ef43aace76a3f24d43d8feab41a9c39c00ea15`
(`claude/rdrand-carry-fix`, fetched as `pr43`), against
`origin/main` at `9cbfc8182d8d310ab127e8696c1cd6b6920d11f2`.
Read the [PR body](https://github.com/SecondStageTurbine/LogansOS/pull/43);
the fetched head includes the subsequent production boot regression test.
All source line references below refer to that reviewed head.

- PASS — RDRAND semantics and bounded retries:
  `kernel/src/rng/entropy.rs:10`, `:53`, `:67`, `:71`, `:80`.
  The loop makes at most ten attempts per word, returns a value only for
  `setc` byte 1, retries byte 0, and returns `None` on exhaustion. A failed
  draw's zero cannot escape as `Some(0)`; a successful draw whose value
  happens to be zero remains valid. Four words are required at `:43`.
  This matches the [Intel SDM Vol. 2B, RDRAND and RDSEED entries,
  printed pages 4-544 and 4-546](https://cdrdv2-public.intel.com/782151/253667-sdm-vol-2b.pdf#page=546):
  CF indicates success, failure zeroes the destination, and the other
  arithmetic flags are cleared. RDSEED is not used here.

- PASS — Assembly and compiler contract:
  `kernel/src/rng/entropy.rs:58-64`. `setc` immediately follows `rdrand`
  in one asm block, so no intervening instruction can overwrite CF.
  `out(reg)` with a `u64` selects a 64-bit destination; the separate
  `out(reg_byte)` captures the carry without overlapping the live value
  output. There are no inputs or memory accesses. `nomem, nostack` are
  appropriate, and removing `preserves_flags` correctly declares flag
  changes. Optimized host assembly confirms adjacent `rdrandq %rcx;
  setb %dl`, followed by the carry-byte comparison.

- PASS — D6 remains fail-closed:
  `kernel/src/rng/entropy.rs:20`, `:27`, `:43`, `:71`;
  `kernel/src/rng/drbg.rs:31-37`, `:83-88`;
  `kernel/src/rng/mod.rs:43-46`. Exhausting any word's ten attempts
  propagates `None` before jitter mixing. Production initialization still
  calls `Csprng::init`, whose unchanged `expect` panics instead of seeding
  from jitter alone. Reseeding retains its unchanged panic on entropy
  failure. Inputs of ten `(value=0, CF=0)` draws, including after three
  successful words, returned no seed in the reviewer harness.

- PASS — Other kernel/userland call sites:
  `userland/libneo/src/lib.rs:97-118`. A case-insensitive search for
  `rdrand|rdseed` across `kernel/` and `userland/` at the reviewed head
  found just two instruction sites: the fixed kernel helper and this
  userland fallback. The fallback already uses adjacent `rdrand; setc`,
  accepts nonzero carry, retries at most 64 times per chunk, and returns
  an error on exhausted zero carry before copying the destination.
  Neither site has the inverted check now; no RDSEED instruction site
  was found.

- PASS — A regression test that reaches production initialization is
  present: `scripts/ci/rng-seed-boot.sh:21-27`, `:31-55`, and
  `.github/workflows/ci.yml:481-485`, `:553-565`. The manual close-gate
  runs this job. It builds `kernel-rng` without `neo-test`, boots QEMU
  TCG with `-cpu max`, rejects a panic or missing seed marker, and has a
  bounded wait. Kernel defaults do not enable `neo-test`
  (`kernel/Cargo.toml:77`, `:372`, `:408`); the marker is emitted after
  `rng::init` returns (`kernel/src/main.rs:766-769`). Thus it exercises
  `Csprng::init`, unlike the fixed-seed branch at
  `kernel/src/rng/mod.rs:37-40`. There is no committed host carry KAT;
  the production boot test satisfies the requested regression coverage.

- PASS — Independent carry validation: a reviewer-only Rust harness used
  the reviewed entropy source with only the RDRAND asm block replaced by
  queued `(value, carry)` results. Seven cases passed at both opt-level 0
  and 3: immediate success, successful zero, failure then success, success
  on attempt ten, ten failures, first-word exhaustion, and fourth-word
  exhaustion. The same seven cases all failed against the pre-fix entropy
  source from the named main base. Separately, the unchanged reviewed
  asm passed 100 real hardware draws plus a mixed 32-byte collection at
  each optimization level. These scratch tests are review evidence, not
  tests added by the PR. Shell syntax, entropy-file rustfmt, and PR diff
  whitespace checks also passed.

- PASS — Executed the PR's actual `scripts/ci/rng-seed-boot.sh` in an
  isolated checkout of the named head, using the dev container, pinned
  `nightly-2026-04-16`, and its QEMU TCG. The build and boot exited 0 with
  `RNG_SEED_BOOT verdict=PASS reason=DRBG seeded from RDRAND`. The serial
  log contained the production CSPRNG initialization marker and no kernel
  panic. This is local execution evidence; it does not claim that the
  full remote CI suite has completed.

- PASS — Overall: no blocking findings in the five requested checks.
