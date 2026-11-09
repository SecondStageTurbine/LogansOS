<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 309 — Janus — PR #8 PowerControl / SYS_POWER review

Reviewed head: `9121fba20a3e64cff5694ffd958efe304cc10bd1`.
Stack base #7: `fd51d950fbdb58f52498e5e24c1e78768beb594d` (also the merge base).
Fetched #14: `73453e318f6677a1e8c018e3ad2b3d46e202d248`; not reviewed here.
Scope: `git diff pr7...pr8`, 16 files, +410/-5; supporting reads at #8's
head, and the current public PR body. RETURN branch:
`codex/pr8-review-janus`, cut from `origin/main` at
`095e62d8c`.

- FAIL — **1a. The enabled feature does not always install the root cap.**
  `kernel/src/main.rs:425`, `:445`, and `:556` omit `power-cap` from all
  three outer capability-assembly gates. The new push at `:537` is therefore
  unreachable with that feature alone. `kernel/Cargo.toml:422` enables only
  the power-quiesce/stop/clean/control chain, none of those outer gates.
  Concrete input: build with `--no-default-features --features power-cap`,
  then have neo-init call syscall 25 with `(action=1, flags=0)`. It inherits
  no PowerControl cap and returns EPERM instead of starting shutdown.
  A host probe compiled the verbatim root-cap assembly with the Cargo
  feature closure: power-cap alone yielded `caps=0 power_caps=0`;
  power-cap + net-dial-cap yielded `caps=2 power_caps=1`. Include power-cap
  in all three gates and test the actual root table. The existing scenario
  cannot catch this: `kernel/src/test_harness.rs:40808` mints a separate cap
  and `kernel/src/arch/x86_64/syscall.rs:8129` accepts supplied rights.

- PASS — **1b. A minted cap cannot escape or survive as slot authority.**
  `kernel/src/sched/process.rs:389` mints USE plus POWER_OFF/RESTART only;
  no DERIVE, GRANT, or reserved SLEEP. The production constructor call is
  in neo-init's assembly at `kernel/src/main.rs:539` (the other caller is
  the test harness). `process.rs:3430` rejects derivation without DERIVE;
  `kernel/src/arch/x86_64/syscall.rs:8198` rejects grants without GRANT,
  and `:8223` additionally rejects the PowerControl kind. `syscall.rs:1958`
  explicitly refuses spawn inheritance. Grant completion independently
  rechecks GRANT at `process.rs:3927`. Close takes the entry out at
  `process.rs:2149`, reuse replaces it with the new handle at `:2115`, and
  `:2337` scans live entries for the PowerControl kind and USE on each
  call; neither a numeric slot nor a saved authorization bit suffices.

- PASS — **2. Refusals and exactly one concurrent start.**
  `kernel/src/acpi/power_abi.rs:45` rejects unknown actions/nonzero flags
  before checking authority, requires the requested semantic bit, and then
  checks running state. `kernel/src/arch/x86_64/syscall.rs:8115` maps the
  refusals to EPERM (-1), EINVAL (-22), and EBUSY (-16). A non-holder still
  gets EPERM while shutdown runs. The handles guard ends at `:8102`, before
  shutdown starts. `kernel/src/acpi/shutdown.rs:121` uses compare_exchange
  with AcqRel/Acquire: even if two callers pass the preliminary running
  check, only one can start; `syscall.rs:8122` maps the loser to EBUSY.
  A host probe using the actual decision/start/running bodies and a counted
  kthread stub checked the errors and ran 100 eight-thread races mixing
  actions 1 and 2: each produced one success, seven EBUSY, one thread spawn.

- FAIL — **3. The SDK-ABI entry disagrees on the no-cap error.**
  The numbers/actions are correctly pinned: `kernel/src/acpi/power_abi.rs:12`
  defines 25/1/2; `userland/libneo/src/lib.rs:78` shares that file and `:1473`
  uses its number. The dispatch literal at
  `kernel/src/arch/x86_64/syscall.rs:118` is locked to it by the compile-time
  assertion at `:120`. The independent numeric oracle at
  `tools/acpi-power-logic-tests/src/power_abi_tests.rs:12` compares against
  literal 25, 1, 2 and 0b001/010/100, not values calculated by the code.
  However, `docs/SDK-ABI.md:186` promises EBADF when no cap is held, while
  `syscall.rs:8103` supplies zero rights and `:8115` returns EPERM.
  Concrete input: an ordinary child with no PowerControl calls
  `raw3(25, 1, 0, 0)`; actual -1, documented -9. Correct the row to EPERM
  and mention ENOSYS for the feature-off kernel. The SDK inventory test
  passes because it checks syscall names/numbers, not error semantics.

- PASS — **4. Feature-off behavior, by source inspection.**
  `kernel/src/arch/x86_64/syscall.rs:1157` removes the new arm without
  power-cap, so syscall 25 falls through to ENOSYS at `:1387`.
  `kernel/src/main.rs:537` and `kernel/src/sched/process.rs:1439` remove the
  mint; the new handle variant, lookup and transfer guards are likewise
  gated. The libneo wrapper remains callable and reaches that ENOSYS.
  `kernel/src/acpi/shutdown.rs:119` retains the prior start/CAS/spawn
  sequence, adding only its success boolean; the prior harness callers
  discard it. K1-K7's implementation is unchanged by this PR. This is a
  functional source review, not a claim of measured binary equality.

- FAIL — **5. Required prior-art/CP section is absent from the PR body.**
  Concrete input: GET `https://api.github.com/repos/SecondStageTurbine/LogansOS/pulls/8`
  and inspect `body`. The [public PR body](https://github.com/SecondStageTurbine/LogansOS/pull/8)
  contains carry-over/restack notes, three feature bullets and claimed local
  gates; it contains no prior-art section, CP table, or explicit K4/K6
  deferral to #6/#7. There is no body file:line to cite for an absent section.
  Add the required analysis, including the CP-34 assembly failure above.
  The code review itself finds no new STOP/IPI, DMA or post-park operation:
  `kernel/src/acpi/shutdown.rs:111` adds an Acquire observation of existing
  KIND; `:121` retains the existing atomic one-start gate; the new syscall
  releases its handle lock before entering this existing shutdown path at
  `kernel/src/arch/x86_64/syscall.rs:8104`. K4/K6 remain the #6/#7 work;
  their previous reviews are not replaced by this P4 review.

Validation: at the reviewed head, `cargo test -p acpi-power-logic-tests
-p handle-info-logic-tests --offline` passed 42 + 4 tests;
`cargo test -p xtask --offline sdk_abi_doc_covers_every_kernel_syscall`
passed 1 test. Temporary host probes used verbatim source extraction for
root-cap assembly and decision/start/running, with small type/constructor
stubs and a counted kthread stub; they do not exercise the actual scheduler,
interrupts or ring-3 entry. QEMU is unavailable here. No QEMU run, kernel
build, bare-metal test or binary kill-switch comparison is claimed. This
delivery changes only the RETURN; no source fix or PR comment was made.
