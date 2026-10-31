<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PM-NOTE: Quality-Gate Audit (pre-BUILD-4)

**Date:** 2026-06-01
**From:** PM/Cerebro
**Status:** findings recorded — fix dispatched (`DISPATCH-2026-06-01-ci-host-test-lint-gates-prometheus.md`)

PM-initiated audit ("have we shipped anything broken / glossed over to reach
BUILD-3?"). Verdict: **no broken code shipped** — the kernel builds (`cargo check`
exit 0), no panics/unwraps/stubs in shipped kernel AI code, `--locked` healthy.
But the **quality gates are weaker than the Bible claims**. Findings:

1. **Host unit tests guard nothing (priority).** `tools/model-logic-tests` and
   `tools/tensor-logic-tests` are not workspace members and not in `exclude` —
   orphaned. `cargo test -p <name>` errors; `--workspace` skips them. CI runs only
   `cargo neo-test`. Every "host tests pass" in BUILD-1/2 close reports was a
   hand-run `--manifest-path`; nothing enforces it. → fixed by the dispatch
   (membership + CI `host-checks` job).
2. **Clippy/fmt run in no gate.** `ci.yml` has no clippy or fmt step, contradicting
   Bible Article 15.5 ("clippy with warnings-as-errors … run in CI"). My earlier
   "clippy deferred to CI" (BUILD-2 close) was therefore inaccurate — there was no
   CI clippy to defer to. → dispatch adds fmt + clippy (host-tools blocking, kernel
   non-blocking pending backlog burndown).
3. **`importer_stub.rs` "not implemented"** — by design (ADR-027 §D6). Not debt.

**Standing follow-on:** kernel carries ~282 clippy warnings (since BUILD-1). The
gate-fix dispatch makes kernel clippy *run and report* (non-blocking); a separate
future cleanup build burns the backlog to zero and flips it to blocking. Tracked
here so it is not lost.

**Process lesson:** close reports asserted "PASS" against checks no automated
system enforced. The fix wires the checks into CI so future PASSes are mechanical,
not manual.
