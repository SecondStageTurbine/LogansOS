<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: CI / hygiene batch (three small real items) → Prometheus

**Date:** 2026-07-22
**From:** PM (Claude)
**To:** Prometheus (Qwen; host-only / bounded-build lane — a perfect fit)
**Status:** DISPATCHED — **host-only + light CI, mostly push-CI (NO hardware, NO heavy QEMU gate).** Three small, real, independent items that remove recurring friction. Do them in order; each is its own commit; a failed/uncertain one is noted, not forced.

## Item 1 — deterministic ISO builds (kills the reproducibility non-determinism)
The bare-metal ISO builds are non-reproducible: two identical-input rebuilds produce different ISO hashes while the AHCI/NVMe payloads are byte-identical — the variance is **xorriso volume-timestamp metadata**. Fix it at the source: pin the ISO volume timestamp so rebuilds are byte-identical (e.g. honor `SOURCE_DATE_EPOCH` and/or pass `xorriso -volume_date …` / a fixed `--modification-date`) in the ISO assembly (`tools/xtask`). Prove it: two clean rebuilds of the same commit → **identical ISO SHA-256**. This makes tested-bytes == shipped-bytes trivial forever.

## Item 2 — A4 manifest-gate scanner hardening — **REASSIGNED to Dexter**
*(`Plans/DISPATCH-2026-07-22-a4-scanner-numeric-syscall-hardening-dexter.md` — parallelized so Prometheus and Dexter run concurrently. **Prometheus does items 1 + 3 only; skip this one.** Kept here for context: the A4 scanner catches named `SYS_*` but a numeric-literal `rax` syscall evades it — Dexter closes that hole.)*

## Item 3 — host-checks → `ubuntu-latest` (frees self-hosted capacity)
Move the cheap **host-checks job** (fmt/clippy/test/SPDX/identity) from `self-hosted` to GitHub-hosted `ubuntu-latest` in `.github/workflows/ci.yml`. It's low-RAM + fast → fits the standard runner, uses the account's Actions minutes, and frees a ryzen runner for the QEMU shards. **Keep the heavy 4-shard QEMU job on `self-hosted`** (it needs ~24 GiB, doesn't fit a standard hosted runner). Verify: the host-checks job runs green on `ubuntu-latest`; the QEMU shards unchanged on self-hosted. **Note the hermeticity discipline** — the host-checks are deterministic (no `-cpu`/RNG dependence), so moving them adds no runner-variance risk.

## Standing rules
- Each item is behavior-preserving CI/build tooling — no kernel/userland runtime change. Items 1-2 touch `tools/xtask` / `build.rs` (let push-CI run); item 3 touches `.github/workflows/ci.yml`. A full 4-shard QEMU gate is NOT required unless a change touches a gated code path — but **do let CI run** (never `[skip ci]` a tools/CI change).
- SPDX on any new file; no new invariant.

## Close-gate + merge
- `scripts/worker-close-gate.sh` (rebase-fresh). Green push-CI URL for each; item 1 shows the two-rebuild-identical-SHA proof; item 2 shows the numeric-off-manifest-fails proof; item 3 shows host-checks green on `ubuntu-latest`. Branch `prometheus/cihygiene-batch`; PM merges after CI + a light review (no heavy AAG — host-only tooling). If item 3's runner move is riskier than expected (billing/perms), skip it + note — items 1-2 stand alone.
- **Push-CI first, honest CI URLs** — the standing rule: green without a CI URL is not done (a tools change lets CI run).

## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-22-cihygiene-batch-prometheus.md`. Files: `tools/xtask/src/main.rs` (ISO assembly — item 1), `userland/neo-std-pal/build.rs` + `tools/ot61-pal-manifest.toml` (item 2), `.github/workflows/ci.yml` (item 3). Work in `Workers\prometheus`; branch `prometheus/cihygiene-batch`.
- **One-line:** **Three small real CI/hygiene items, host-only/push-CI, no hardware.** (1) **Deterministic ISO builds** — pin the xorriso volume timestamp (`SOURCE_DATE_EPOCH`/`-volume_date`) so two rebuilds of a commit produce an identical ISO SHA (kills the reproducibility non-determinism). (2) **A4 scanner hardening** — the gate only catches named `SYS_*`; make it also catch numeric-literal `rax` syscalls (map number→name→allowed-list) or lint against numeric `rax`, so a numeric off-manifest syscall FAILS the build (falsification-proven). (3) **Host-checks → `ubuntu-latest`** — move the cheap fmt/clippy/test/SPDX/identity job to GitHub-hosted (frees a self-hosted runner), keep the 24-GiB 4-shard QEMU on self-hosted. Each its own commit + green push-CI URL + its proof; behavior-preserving tooling only. If item 3 hits a billing/perms wall, skip + note (1-2 stand). Never `[skip ci]` a tools/CI change.
- **RAG** (PowerShell): `xtask iso assemble xorriso limine`, `neo-std-pal build.rs scan_sys_tokens A4`, `ci.yml runs-on self-hosted host tests`. Health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first; if down, note it + grep/Read (PL owns RAG startup; never a blocker).
