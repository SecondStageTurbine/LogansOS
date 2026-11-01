<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 234: Dexter, Lane 2c (fuzz the decoders on the host)

**MODEL** DeepSeek, high: a bounded host-only build, the guardrail step of Lane 2
(`docs/THINK-2026-10-31-lived-in-os-lanes.md` section 4, item 3).

Same rules as your earlier relays (public repo: no personal details; RAG not available).

1. Add a host fuzz crate (e.g. `tools/image-decode-fuzz`, `cargo-fuzz`/libFuzzer or a deterministic in-tree
   fuzzer if cargo-fuzz can't run in CI without nightly extras) with targets for: inflate (zlib and raw), the
   `libneo` PNG decode, and the `libneo` JPEG decode. Seed corpora: the KAT vectors already in the repo.
2. A CI job that runs each target for a fixed budget (about 60 seconds each) on GitHub-hosted runners and fails on
   any crash, panic, timeout or allocation above the decode size cap.
3. Any crash you find: minimise it, add it as a regression KAT, and fix it (in our API wrapper; for an upstream bug,
   record it in `VENDORED.md` with the upstream issue link if one exists, and guard it in the wrapper).

- BRANCH: `dexter/l2c-decoder-fuzz`, from `origin/dexter/l2-codecs` (PR #36; it needs the codecs).
- Close: `scripts/worker-close-gate.sh`; push; RETURN `Plans/RETURN-2026-11-01-l2c-decoder-fuzz-dexter.md` on your
  branch, naming the head SHA, the fuzz time per target, and anything found.
