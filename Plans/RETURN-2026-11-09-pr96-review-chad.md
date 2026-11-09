<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: PR #96 stable size measurement review (Chad, relay 376)

**Reviewed head:** `697ad91ca5f02e87ee31434ef7c729f4350b3424` (`claude/size-budget-stable`)
**Review branch:** `codex/pr96-review-chad`
**Dispatch:** Section 376 of `Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md`.

## Findings

- **PASS — product kernel build and measured image A match.** The build path sets `CARGO_PROFILE_DEV_CODEGEN_UNITS=1` for the kernel when the explicit product opt-level is set (`tools/xtask/src/main.rs:7352-7359, 9788-9799`). `measure_image_a` builds through `build_product_image_kernel_with`, then assembles the model-less image A ISO; the normal `size-budget` and `--stability` paths use that same helper (`tools/xtask/src/main.rs:7525-7545, 7569-7574`). This measures the image A kernel/ISO the PR now defines as the product build.

- **PASS — kernel debug sections are excluded from the budgeted ISO.** The ELF parser sums sections named `.debug_*`; `ImageASizes::iso_less_debug` subtracts that total from the assembled ISO, and the report labels the adjusted figure separately from the as-built ISO (`tools/xtask/src/main.rs:7510-7522, 7735-7759`). The `size-budget` comparison uses the adjusted value (`tools/xtask/src/main.rs:7464-7493`).

- **PASS — stability measurement is within its bounds.** The PR description reports `CARGO_INCREMENTAL=0` measurements of `.text` moving 64 B against 1,024 B, and the debug-adjusted ISO moving 713 B against 65,536 B. CI run [42590049541](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42590049541) completed the blocking size-budget step successfully (Build Checks step 8); the full-tier workflow invokes `size-budget --stability` after the normal budget (`.github/workflows/ci.yml:1280-1287, 1332-1335`). The rest of that workflow was still in progress when checked, so this is a pass for the size step, not a claim that the full workflow was green.

- **FAIL — ceilings have not been re-baselined from the new measurement.** `tools/size-budget.toml` still contains the pre-PR ceilings and their #82 reasons (`[kernel_text]` 1,616,127; `[iso]` 157,540,352; lines 13-27). The PR body explicitly says the second commit that will re-baseline from this PR run's CI numbers is still to come. The check in this section therefore is not yet satisfied.

- **FAIL — the product image itself changes.** The new codegen-units setting is applied to `build_product_image_kernel`, which is also the kernel used by `iso --with-model` and `image-kernel-build` (`tools/xtask/src/main.rs:7352-7359, 7384-7419, 9791-9799`). The PR body reports that the shipped kernel's `.text` changes from 1,516,511 B to 1,560,485 B, with the one-unit build causing more inlining. That is a product kernel/image build change, beyond changing only the measurement. A fixed one-unit measurement proxy would avoid changing the product image while stabilizing the gate.

## Runtime evidence

The blocking size-budget step passed in CI run 42590049541. The PR description provides the local before/after stability values; GitHub's public job-log endpoint denied access to the detailed step log, so the exact CI-emitted byte values could not be independently read here. Other CI shards were still running at review time.
