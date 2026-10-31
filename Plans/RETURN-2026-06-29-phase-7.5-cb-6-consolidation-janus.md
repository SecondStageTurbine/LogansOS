# RETURN - Phase 7.5 CB-6 consolidation (Janus)

Branch: `codex/cb-6-consolidation-janus`

Base: `origin/main@ba35de40c1a48435d10eda48e3e4b5983b4fa305`

Remote proof SHA: `714d6c45a350dd0e6b9b236924cb965cbce88a5c`

Dispatch: `Plans/DISPATCH-2026-06-29-phase-7.5-cb-6-consolidation-janus.md`

Audit: `Plans/THINK-2026-06-24-phase-7.5-cb-6-consolidation-audit.md`

## Verdict

PASS. CB-6 landed as the ordered five-subcommit consolidation:

- ISC-1 `a75b45d` - mt7601u clippy PAY debt paid.
- ISC-2 `1c78079` - dead scaffolding removed.
- ISC-3 `57cc695` - invariant ledger prose refit.
- ISC-4 `d340fb0` - close-tree status drift repaired.
- ISC-5 `714d6c4` - closed OPEN-THREADS rows pruned.

This RETURN is filed after the proof run as a docs-only `Plans/**` evidence commit. No kernel, tool, or source files changed after the proven SHA above.

## Remote Evidence

Push CI:

- Run: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/28404169339
- Event: `push`
- Head SHA: `714d6c45a350dd0e6b9b236924cb965cbce88a5c`
- Result: green. SPDX and Host Tests, Format & Lint passed; branch-push QEMU was correctly skipped.

Manual QEMU gate:

- Run: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/28404189814
- Event: `workflow_dispatch`
- Head SHA: `714d6c45a350dd0e6b9b236924cb965cbce88a5c`
- Result: green. Host, SPDX, and all 4 QEMU shards passed.
- `NEO_SMP=4`: set by `.github/workflows/ci.yml`.
- `-cpu max`: preserved by `tools/xtask/src/main.rs::run_qemu_test_invocation` for the QEMU scenarios that require it.

QEMU shard job URLs:

- Shard 1/4: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/28404189814/job/84162702998
- Shard 2/4: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/28404189814/job/84162703019
- Shard 3/4: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/28404189814/job/84162703009
- Shard 4/4: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/28404189814/job/84162703003

## Local Evidence

Worker close gate:

- Command: `& "C:\Program Files\Git\bin\bash.exe" scripts/worker-close-gate.sh`
- Result: PASS on `714d6c4`, rebased on `origin/main@ba35de4`.
- Covered `cargo fmt --all --check`, host-crate tests, workspace clippy with `-D warnings`, kernel clippy with `-D warnings`, SPDX, and diff self-inspection.

CB-6 specific local checks:

- `docker compose run --rm neo cargo clippy -p neoplutarch-kernel --all-targets --features mt7601u`
  - Exit: 0.
  - Final diagnostic count: 9 banked warnings.
  - Remaining warnings: 6 rng dead-code diagnostics and 3 USB `result_large_err` diagnostics.
  - The mt7601u PAY warnings are gone. The audit's 7-warning estimate is stale against current clippy output because the rng C1/C2 bank splits into separate diagnostics.
- `docker compose run --rm neo cargo check -p neoplutarch-kernel --features interface-identity`
  - Exit: 0.
- `docker compose run --rm neo cargo check -p neoplutarch-kernel --features neo-test-interface-identity-select`
  - Exit: 0.
- `docker compose run --rm neo cargo check -p neoplutarch-kernel --features neo-test-interface-cap-derive-revoke`
  - Exit: 0.
- `docker compose run --rm neo cargo check -p neoplutarch-kernel --features neo-test-file-cap-derive-grant`
  - Exit: 0.

## Scope Closeout

ISC-1 clippy debt:

- Used `cargo clippy --fix` for the PAY class, then hand-checked the result.
- Removed the unused USB re-export and the dead `wedge_seen` scaffold.
- Fixed the mt7601u module-doc lazy-continuation warning.
- Added explicit `clippy-bank:` comments only to the retained USB oversized-error and rng/KAT banked surfaces.
- Did not remove the rng KAT oracle.

ISC-2 dead scaffolding:

- Removed `loader/elf.rs::PF_R`.
- Removed the top-level `arch::disable_interrupts()` and `arch::interrupts_enabled()` wrappers; the x86_64 implementations remain.
- Removed `cdc_ecm::debug_pipe_drops()`.
- Removed `virtio_net::debug_used_ring_acquire_barriers()` and `virtio_net::debug_primary_device()`.
- Preserved the consumed cdc_ecm debug helpers and gate-facing surfaces.

ISC-3 invariant ledger:

- `docs/complexity-governance.md` active section refit from 11,532 chars to 7,483 chars.
- I-14/I-15 guarantee and guard names remain visible.
- §1 page-fit flag is marked resolved by CB-6.

ISC-4 status drift:

- ADR-057 now states association is proven on silicon and the usable-link tail remains.
- Bible Article 9.10 now names the staged `mt7601u.bin` Limine module and removes the stale in-git/blob implication.
- ADR-017 now points readers to the current MT7601U amendment before historical iwlwifi text.
- `CLAUDE.md` Genesis/offboarding text and Git Worktree callout are current.
- CB-3 received the minimal registration-binding assert in the `interface-identity` ON path.

ISC-5 OPEN-THREADS:

- Removed resolved active rows while preserving close evidence in commit `714d6c4`:
  - OT-2: `d1c8bb6`; push CI https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/27910976427; QEMU https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/27911002255.
  - OT-29: row close `1c3ee65`, main-contained cherry-pick `4522133`; push CI https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/27957709384; QEMU https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/27957728489.
  - OT-30: `e517a52`; push CI https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/27973845115; QEMU https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/27975223225; F4 follow-up `546ef3d` QEMU https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/28002712628.
  - OT-37: proof joint `d801f70`; fix `2a4843c`; push CI https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/28099852911; QEMU https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/28099858869; proof-joint push CI https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/28108524816.
  - OT-40: `9b4a875`; push CI https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/28149690857; workflow_dispatch QEMU https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/28149701174.
- Removed the 2026-06-20 and 2026-06-17 historical prune tables.
- Kept OT-39 and OT-41 active.
- Reconciled OT-18 and OT-27 as still-active umbrellas without stale "only E4 remains" or "GTK/SMP remain open" contradictions.

## Explicit Non-Removals

- No gate discriminator was removed.
- No rng KAT oracle was removed.
- Kill switches remain default-off where applicable.
