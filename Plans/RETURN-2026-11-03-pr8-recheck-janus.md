<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 311 — Janus — PR #8 narrow re-check

Reviewed head: `8e81cb4d96358e1800cba553214b5463673cddc4`.
PR base: `main`; merge base: `3de4d4445e29eb56c3fd607365a69969a1ad6037`.
Reviewed the PR's own change with `git diff $(git merge-base origin/main pr8) pr8`.
RETURN branch: `codex/pr8-recheck-janus`, from `origin/main` at `474026e03`.
Seat instructions: newest `PM:` comment, [#8 comment 6611859338](https://github.com/SecondStageTurbine/LogansOS/pull/8#issuecomment-6611859338).

- PASS — **1a (earlier FAIL): power-cap alone installs the root cap, and a check covers it.**
  `kernel/src/main.rs:437`, `:460`, and `:488` now include `power-cap` in
  all three assembly gates. The production call at `:463` uses
  `root_inherited_caps()`, whose gate includes it at `:1097` and whose push
  at `:1173` installs the power cap. The resulting slice reaches
  `spawn_user` at `:492`. `kernel/src/test_harness.rs:41405` now reads this
  same root builder, rejects zero or duplicate power caps at `:41410` and
  `:41413`, and checks USE without DERIVE/GRANT at `:41421`.
  `tools/xtask/src/main.rs:11718` requires `root_caps=1 power_caps=1`.
  A host probe compiled the verbatim root-builder definition and the
  production inherited-handle assembly, using the Cargo feature closure
  and small handle/constructor stubs: power-cap alone produced
  `root_caps=1 power_caps=1`; adding net-dial-cap produced
  `root_caps=2 power_caps=1`. This checks the previously failing gates and
  assembly, without claiming a kernel boot or actual process installation.

- PASS — **3 (earlier FAIL): SDK-ABI's no-cap error matches the code.**
  `docs/SDK-ABI.md:186` now documents EPERM for no PowerControl cap or a
  missing action right, plus ENOSYS without power-cap.
  `kernel/src/sched/process.rs:2338` scans live handles and returns zero
  rights without a usable cap; `kernel/src/acpi/power_abi.rs:54` refuses
  zero rights, and `kernel/src/arch/x86_64/syscall.rs:8288` maps that refusal
  to EPERM. Thus syscall 25 with `(action=1, flags=0)` from a non-holder
  returns the documented -1. The feature-gated dispatch at `syscall.rs:1178`
  preserves the feature-off ENOSYS fallback. The no-cap host KAT and SDK
  syscall inventory test pass.

- PASS — **5 (earlier FAIL): the PR body carries prior art and the CP table, including the concurrent-start guard and discriminator.**
  The [current PR body](https://github.com/SecondStageTurbine/LogansOS/pull/8),
  fetched through the public API, has prior art at body line 15 and the CP
  table at body lines 19–24. Its CP-34 row names the corrected root-table
  check. Its concurrent-start row names the single compare_exchange and
  the `ot90_cap` discriminator `second=-16`.
  `kernel/src/acpi/shutdown.rs:121` enforces the one-start gate even when
  two callers pass the preliminary running check;
  `kernel/src/arch/x86_64/syscall.rs:8295` returns EBUSY for the losing start.
  `kernel/src/test_harness.rs:41447` and `:41455` check first success and
  second EBUSY; `tools/xtask/src/main.rs:11721` requires the result marker.
  This scenario is a sequential second-start discriminator; concurrent
  exclusion follows from the CAS. The body's handle-lock and K4–K7 rows
  match the code: the lock scope ends at `syscall.rs:8275` before start,
  and this PR adds no STOP/IPI, DMA, or post-park operation.

- PASS — **New findings:** none within this narrow re-check of items 1a, 3 and 5; earlier passing items were not re-reviewed in full.

Validation at the reviewed head: `cargo test -p acpi-power-logic-tests
-p handle-info-logic-tests --offline` passed 42 + 4 tests;
`cargo test -p xtask --offline sdk_abi_doc_covers_every_kernel_syscall`
passed 1 test. The temporary source-extraction probe described above passed
both feature combinations. QEMU is unavailable here; no QEMU run, kernel
build, bare-metal test, or binary equality check is claimed. Delivery
changes only this RETURN file.
