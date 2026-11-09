<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: PR #96 re-check (Chad, relay 378)

**Reviewed head:** `0eea46148334b655d177f45ba6178a16aca74136` (`claude/size-budget-stable`).
**Review branch:** `codex/pr96-recheck-chad`.
**Dispatch:** Section 378 of `Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md`.
**Result:** PASS on both requested items. The PM's acceptance of the product kernel's one-codegen-unit setting supersedes relay 376's item 5 objection.

## Findings

- **PASS — ceilings re-baselined from #96's own CI measurement, with the existing margins and reasons.** Commit `1a09cd3b8ba1d56b2f722ca2909734c44c494497`, present in the reviewed head, changes both ceilings. `tools/size-budget.toml:18-19` records kernel `.text` of 1,562,501 B from PR run 42590049541 (main `6975f72e` + #96), plus 32 KiB: **1,595,269 B**. `tools/size-budget.toml:29-30` records the model-less ISO less kernel debug at 122,860,422 B from that same run, plus 1 MiB: **123,908,998 B**. Its subtraction also checks: 152,576,000 B as built minus 29,715,578 B kernel debug = 122,860,422 B. Both reasons retain the historical baselines and explain the new measurement; the kernel reason explains the codegen-layout noise and the ISO reason explains debug-info noise. Both margins and the subtraction were independently checked using the TOML parsed from the pinned head.

- **PASS — the PR and TOML state that the product kernel now builds with `codegen-units=1`.** The [PR body](https://github.com/SecondStageTurbine/LogansOS/pull/96), under “The change”, item 1, explicitly says “The product kernel is one codegen unit”, names `PRODUCT_IMAGE_KERNEL_CODEGEN_UNITS = "1"` and `CARGO_PROFILE_DEV_CODEGEN_UNITS`, identifies it as the shipped kernel, and reports the measured code-size cost. `tools/size-budget.toml:14-19` likewise identifies the shipped code as one codegen unit and says in its reason that #96 builds the product kernel that way. The code agrees: `tools/xtask/src/main.rs:7359` sets the constant to `"1"`; `tools/xtask/src/main.rs:9788-9799` applies the environment override to the kernel build with an explicit product opt-level. This is the deliberate product change accepted by the PM in Section 378.

## Evidence and limits

Read the pinned head and re-baseline commit directly, fetched PR #96's head, and checked the live PR body through GitHub's API; its head was still `0eea46148334b655d177f45ba6178a16aca74136`. The body still describes the re-baseline as a next commit in item 5, but that commit is present at the reviewed head and both requested declarations are explicit.

The jobs API for [run 42590049541](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42590049541) reports Build Checks job `129069514268` and its step 8, “Size budgets — image A .text + ISO (blocking)”, successful. The detailed job-log endpoint returned HTTP 403, so the exact CI byte values above are verified against the committed reasons and re-baseline commit message, not independently recovered from the raw log. The run's QEMU shards were cancelled; this RETURN does not claim that run was a fully green gate. No builds were rerun for this narrow documentation and ceiling re-check.
