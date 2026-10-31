<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH — Dexter (DeepSeek implementation seat): the SDK-ABI refresh, then CI size budgets

**Why you:** J.c. offered DeepSeek as an extra coding seat (2026-10-30). Both tasks are bounded and not
time-critical (your weekly usage limit, CLAUDE.md). Both come from `docs/THINK-2026-10-30-playbook-gap-map.md`:
lane L4.4 and lane L7.2. You run headless through the relay runner, so finish each task in one run if you can, and
always push what you have.

## Task 1: the SDK-ABI refresh (L4.4). Branch `dexter/sdk-abi-refresh`, off `origin/main`
`docs/SDK-ABI.md` (last touched 2026-07-18) documents 49 syscalls, while `kernel/src/arch/x86_64/syscall.rs` defines
111 `SYS_` numbers.
1. Inventory every `pub const SYS_*`: number, name, args, return values and errno, the feature gate it's behind (its
   `cfg`), and whether it's in image A (`PRODUCT_IMAGE_KERNEL_FEATURES`, `tools/xtask/src/main.rs`). Mark test-only
   syscalls (`SYS_TEST_*`, `neo-test*`) as such, and don't document them as SDK.
2. Bring `docs/SDK-ABI.md` up to date for every non-test syscall, from the code and the matching `libneo` wrapper,
   not from memory. Note capability requirements (for example, SYS_OPEN's root form and `system-store`; SYS_NET_DIAL
   and `net-dial-cap`).
3. **A drift guard:** a host test (an xtask test or a small crate under `tools/`) that parses `syscall.rs` for
   `pub const SYS_` and fails if any non-test number is missing from `docs/SDK-ABI.md`. Falsify it by deleting one
   row.

## Task 2: CI size budgets (L7.2). Branch `dexter/size-budgets`, off `origin/main`
1. A committed budget file (for example `tools/size-budget.toml`): image A's kernel `.text` size and the ISO size,
   each with a ceiling and a short reason.
2. A CI step (`.github/workflows/ci.yml`, in the product build job that already builds image A) that measures both
   and fails when a ceiling is exceeded. The failure message says to raise the budget in the same PR, with a reason.
3. Set today's ceilings at the measured value plus a small margin (say so in the PR).
4. Falsify it: lower a ceiling below the current size on a scratch commit, and show the step fails.

## Rules (CLAUDE.md)
- SPDX headers on new files. `scripts/worker-close-gate.sh` before declaring a task done (Docker if your folder has
  it; otherwise run the same commands directly and say so).
- Open one PR per task, and post a comment starting `RETURN (dexter):` with the head SHA and the evidence. Don't merge.
- Commit and push at every checkpoint. No model identifiers in commits or PR text.
