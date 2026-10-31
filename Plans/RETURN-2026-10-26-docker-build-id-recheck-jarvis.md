<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay #76: Docker build-ID re-check after #138

Reviewed SHA: `c3aae11c5953f5058fa476ecd9dfd6c93c0a0bde`.
Branch: `codex/docker-build-id-recheck-jarvis`.

**Overall verdict: PASS.** The real Windows Docker host now resolves the
reviewed commit despite the bind mount being owned by `0:0` and the container
running as `neo` (`1000:1000`). Review report only; no source changes or revert.

1. **PASS — real host.** `docker-compose.yml:45` supplies process-local Git
   trust. From `E:\AI Automation\workers\jarvis`,
   `docker compose run --rm neo git rev-parse HEAD` printed
   `c3aae11c5953f5058fa476ecd9dfd6c93c0a0bde`, without a dubious-ownership
   error. `id` and `stat -c %u:%g /workspace` confirmed `1000:1000` versus
   `0:0`. Running the unchanged `kernel/build.rs` inside Compose also emitted
   `cargo:rustc-env=NEO_BUILD_ID=c3aae11c5953f5058fa476ecd9dfd6c93c0a0bde`.
   The dispatch-permitted plain kernel build succeeded with the repository-pinned
   Linux nightly (`nightly-2026-04-16`):
   `docker compose run --rm neo bash -lc 'cd kernel && cargo build -Zjson-target-spec --bin kernel'`.
   Cargo exited 0 (`Finished dev profile ... in 1m 21s`).
   `strings target/x86_64-neoplutarch/debug/kernel | grep -F c3aae11c5953f5058fa476ecd9dfd6c93c0a0bde`
   inside Compose printed the banner string
   `D  build c3aae11c5953f5058fa476ecd9dfd6c93c0a0bde | debug-assertions=`.
   The linked ELF is 30,728,000 bytes; `kernel/src/main.rs:594` embeds that ID.

2. **PASS — all services and image declaration.** The three services set
   `GIT_CONFIG_COUNT=1`, `GIT_CONFIG_KEY_0=safe.directory`, and
   `GIT_CONFIG_VALUE_0=/workspace` at `docker-compose.yml:45`,
   `docker-compose.yml:67`, and `docker-compose.yml:91` respectively.
   `docker compose config --format json` confirmed all three effective values.
   `Dockerfile:107` sets exactly the same environment for direct `docker run`
   after building the updated image. Only `/workspace` is trusted, never `*`.
   The existing local image predates that Dockerfile change; its environment
   was not mistaken for a rebuilt image, and no full image rebuild was needed
   to exercise the Compose fix.

3. **PASS — no persistent trust configuration.** The reviewed delta only
   adds environment declarations (`Dockerfile:107`, `docker-compose.yml:45`);
   it contains no `git config` write. `git config --show-origin --get-all
   safe.directory` inside Compose reported `command line: /workspace`.
   `/home/neo/.gitconfig` remained absent. The repository config SHA-256 was
   identical before and after a container `git rev-parse HEAD`:
   `a58a8b250bb821a6e57d4bfaea9be72f922308c667b9f004d3a7b716dc31c9a1`.
   The host global config hash remained
   `7EE6A7974549075B071AE6B7780B28E1234C305261A85D3F68FE7A3EA3FF7ADE`.
   Host Git still reads its pre-existing safe-directory entries
   (`E:/AI Automation/AltimitNeo` and `*`); neither was added by this change
   or this review.

4. **PASS — retry and incremental behavior on the host.**
   `kernel/build.rs:35` emits the missing `OUT_DIR` child
   `build-id-retry-until-git-answers` only in the fallback closure; the Git
   success path at `kernel/build.rs:27` bypasses it. A scratch Windows Cargo
   package copied this exact build script and printed `env!("NEO_BUILD_ID")`.
   Four actual `cargo +nightly-2026-04-16 run -vv` invocations proved:
   no repository produced `unknown-build-id` and the retry directive; an
   unchanged second build was `Dirty` because that path was missing; creating
   a repository and commit recovered on the next build to
   `9baee411a34bc8fe2f7d7adf9b6279f222365bc7`; an unchanged fourth build was
   `Fresh` and retained that ID. The successful build-script output did not
   emit the retry directive. Logs remain in the temporary directory
   `C:\Users\<user>\AppData\Local\Temp\relay76-cargo-6c4a661fe285448fb5c612343b3db5fd`.

5. **PASS — CI paths remain valid (source review, not a new CI run).**
   `.github/workflows/ci.yml:337` still matches the host UID/GID for Docker.
   Checkouts remain at `.github/workflows/ci.yml:206` and
   `.github/workflows/ci.yml:635`; the ordinary product kernel build is at
   `.github/workflows/ci.yml:658` and xtask image build at
   `.github/workflows/ci.yml:666`. The workflow has no changes between
   `dde0d988` and the reviewed SHA. Successful Git lookups retain the same
   HEAD-derived ID through `kernel/build.rs:27` and
   `tools/xtask/src/main.rs:6168`; the new retry directive only affects failed
   lookups. The real container and host success probes above exercise those
   ID sources; the supplied dispatch identifies the merged CI gate as green.

## Environment and preservation

- RAG health was healthy, with Ollama and Qdrant available. Discovery used
  `graft.cmd ask ... --source` before exact searches; the PowerShell shim was
  blocked by execution policy, so the `.cmd` entry point was used.
- Applied Rust best-practices guidance to `kernel/build.rs` and systematic
  debugging guidance to distinguish environment/toolchain issues from the
  reviewed change. The image-default July nightly hit `x86_64` crate E0046
  errors for the changed unstable `Step` trait; final verification uses the
  repository-pinned April nightly instead. An offline attempt lacked a
  sysroot dependency; an initial xtask attempt was stopped while preparing
  userspace dependencies, before the direct kernel build.
- Pre-existing `.gitignore` and `.ignore` changes are preserved in stash
  `591a6651ec1bd84f6e753b9be1cec1c12f43093c`, named
  `relay76-preserve-preexisting-graft-ignore`. They remain saved through relay
  #78 for the parent to restore afterward; they are not part of this report.
- `git diff --check` passed. Clean-tree and no-unpushed-commit checks are
  performed after committing and pushing this report.


