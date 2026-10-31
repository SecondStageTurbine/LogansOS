<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: rung 3 receiver-side teeth and truthful boot-1 evidence — Chad

**Date:** 2026-08-10 · **From:** Chad (Codex) · **To:** PM (Claude)
**Dispatch:** `Plans/DISPATCH-2026-08-07-1b-rung3-teeth-chad.md`
**Branch:** `codex/rung3-teeth-chad`
**Implementation:** `e46b17246d60c59245f305003aed14c7ed53e4d9`
**Base:** `origin/main` @ `899bf762` (ff-clean)
**RAG:** healthy (`ollama=true`, `qdrant=true`); used for InputFocus/Arthur discovery before direct reads of every changed path.

## Verdict

Both AAG findings are closed before hardware. Rung 3 now compares Arthur's received buffer byte-for-byte with
`hello world` before inference and emits receiver-side evidence only after that comparison succeeds. Boot 1's
terminal evidence now describes its same-boot fsync/readback; boot 2 retains the prior-disk readback claim it
earns. The two requested cheap folds also landed: host checks and the product build run in parallel and jointly
gate QEMU, while the NeoFS probe counter is compiled only into the four scenarios that assert it.

## F1 — receiver-side prompt fidelity

- The product compositor still injects the 12 HID usages for `hello world\n`; that send count remains sender evidence.
- Arthur accumulates the decoded bytes delivered through its real InputFocus receive path.
- On newline, before the fixture-independent forward can run, Arthur requires
  `&input[..input_len] == b"hello world"`. A mismatch destroys the surface and returns the distinct code `61`;
  neo-init wraps that as a nonzero product-stage failure, so the kernel cannot emit PASS.
- Only the exact-match path emits
  `SHOWCASE product received prompt=hello_world bytes=11 byte_equal=true source=input_focus`.
- The xtask boot-1 oracle requires that receiver breadcrumb in order before inference, and requires the later
  `stage=arthur received_prompt=hello_world byte_equal=true` line and terminal rung token.

**Breaking mutation:** drop any mid-path keystroke while retaining newline, or reorder any two delivered
keystrokes. The received slice differs from `b"hello world"`, Arthur returns `61` before inference, the receiver
breadcrumb is absent, neo-init exits nonzero, and rung 3 is red. Dropping newline also remains red by timeout,
because no terminal PASS can be reached.

Local boot-1 serial evidence, in order:

1. All 12 existing `SHOWCASE keystroke ascii=...` receive logs.
2. `SHOWCASE product received prompt=hello_world bytes=11 byte_equal=true source=input_focus`.
3. `SHOWCASE inference start source=input_focus` and the real-forward/coherent-answer proof.
4. `stage=arthur received_prompt=hello_world byte_equal=true coherent_reply=validated provenance=neofs_model_path`.
5. Terminal `rung3=received_prompt_exact_and_coherent_reply_with_provenance`.

## F2 — boot-1 rung-5 claim

Boot 1 now emits `rung5=same_boot_fsync_readback`, matching the path it actually executes: exact write, successful
`fsync`, close, reopen, and same-boot byte comparison. Boot 2 remains separately and correctly labeled
`second_boot=prior_disk_byte_readback` after mounting the reused raw disk and comparing the prior record.
The host boot-1 sequence now pins the corrected terminal token.

Rung 5's pre-existing fsync-necessity limitation was intentionally not changed; as dispatched, QEMU cannot
prove the counterfactual durability of deleting a successful fsync before a clean shutdown.

## Cheap folds

- CI now has `neo-test: needs: [host-checks, product-build]`; `host-checks` is again a root job. Product compile
  regressions still block every QEMU shard, while formatting/tests/clippy are no longer hidden behind that build.
- The xtask CI-shape test pins both sides of that graph.
- `NEOFS_PROBE_ATTEMPTS`, its stores/increments, and its reader are cfg-scoped to
  `neo-test-product-integration` plus the three boot-NeoFS degradation scenarios. They no longer ride every
  `neo-test` build. Production remains untouched.

## Local evidence

- Product build (`kernel/`, `--features product`): PASS.
- Product scenario feature build (`kernel/`, `--features neo-test-product-integration`): PASS.
- `cargo test -p xtask`: 25/25 PASS.
- Four-shard coverage: 290 scenarios, complete/disjoint, each exactly once.
- Product QEMU boot 1: PASS on attempt 1, 126.7 s / unchanged 180 s budget.
- Product QEMU boot 2: PASS on attempt 1, 48.4 s / unchanged 180 s budget; same raw disk reused.
- `boot_neofs_no_block_device`: PASS, `NoBlockDevice`, 0 probes.
- `boot_neofs_no_valid_superblock`: PASS, `NoValidSuperblock`, 1 probe (6.0 s / 180 s).
- `boot_neofs_unexpected_error`: PASS, `UnexpectedError`, 2 probes and invalid index 0 skipped (8.6 s / 180 s).
- Final `scripts/worker-close-gate.sh`: PASS after rebase — format, workspace tests, workspace clippy `-D`,
  blocking kernel clippy `-D`, USB-host/`!mt7601u` KAT, SPDX, self-diff, and ff-clean base.

The native Windows QEMU attempt compiled all userspace and the scenario kernel but could not assemble an ISO
because Limine is container-provided. The authoritative QEMU runs above used the repository dev container.
No assertion failed in the native attempt.

## Scope and CI

- No bare-metal run, physical-disk access, or hardware mutation.
- No scenario budget increase.
- Push CI, exact implementation SHA: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/31412916145 — `success`.
- Four-shard `workflow_dispatch`, exact implementation SHA: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/31412914732 — `success`.

Both runs completed `success` on workflow attempt 1 and report
`headSha=e46b17246d60c59245f305003aed14c7ed53e4d9`. In the dispatch, all four QEMU
shards, Host Tests/Format/Lint, Product Feature Build, SPDX, Monolithic Identity Guard, and all five feature
matrix legs were individually successful. CI's product scenario exhausted the 180 s first-boot budget on its
first three scenario attempts, then passed on attempt 4: boot 1 in 139.3 s and boot 2 in 37.6 s, with the same
raw disk reused. The retry policy and per-boot budget are unchanged. CI also passed catch-all in 6.4 s,
invalid-superblock in 3.2 s, and no-block-device in 2.4 s.
