<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay #74: kernel build ID follows HEAD (#136)

Reviewed commit: `093e1c9c9b57d3bf9428f2ead7e2e23b2fb682d1`.
Branch: `codex/build-id-review-jarvis`.

**Overall checklist verdict: FAIL — item 3's default Docker mount does not get
the correct ID. This is a reproduced, pre-existing Docker trust/configuration
problem, not a regression introduced by #136. The #136 code review passes.**
No implementation changes or revert were made: reverting #136 would retain the
Docker failure and remove the working HEAD-refresh fix. Deliverable is this
review report only.

## Checklist

1. **PASS — xtask passes the ID.** `tools/xtask/src/main.rs:6145` applies the
   build-ID block to kernel builds through `build_member_at_opt_level`.
   `tools/xtask/src/main.rs:6168` reads `git -C <workspace> rev-parse HEAD`;
   `tools/xtask/src/main.rs:6185` checks tracked changes with
   `status --porcelain --untracked-files=no`.
   `tools/xtask/src/main.rs:6192` returns the full clean hash or its first 26
   characters plus `-dirty` (32 ASCII bytes). A nonblank caller override remains
   inherited because the block only supplies a missing/blank value
   (`tools/xtask/src/main.rs:6146`); the build script also preserves overrides
   (`kernel/build.rs:24`). Default/product and scenario paths reach this helper
   at `tools/xtask/src/main.rs:4954` and `tools/xtask/src/main.rs:5128`.
   The separate ELF-check Cargo invocation at `tools/xtask/src/main.rs:6354`
   relies on the plain-Cargo fallback; the checklist's named helper is covered.

2. **PASS — plain Cargo reruns on HEAD changes.** `kernel/build.rs:22` invokes
   the watcher. `kernel/build.rs:38` lists HEAD, logs/HEAD, and packed-refs;
   `kernel/build.rs:43` adds HEAD's symbolic ref. Each path is resolved via
   `git rev-parse --git-path` (`kernel/build.rs:49`), joined relative to the
   manifest directory, and emitted only when it exists (`kernel/build.rs:53`).
   This handles absolute paths returned for linked worktrees. Actual incremental
   Cargo probes using the unchanged build script passed for a new commit,
   detached checkout, linked-worktree commit, and a commit after packing refs.

3. **FAIL — no-repo and CI paths pass; the default Docker mount fails the
   correct-ID requirement.** With no repository, Git failures become `None`
   (`tools/xtask/src/main.rs:6175`, `kernel/build.rs:78`) and the build script
   emits the stated placeholder `unknown-build-id` (`kernel/build.rs:28`). An
   actual host Cargo probe in an existing directory outside Git succeeded with
   that placeholder. CI uses `actions/checkout@v4`
   (`.github/workflows/ci.yml:206`, `.github/workflows/ci.yml:627`); the checked-out
   HEAD is sufficient even for a shallow/detached checkout. The Docker CI job
   matches the host UID/GID (`.github/workflows/ci.yml:337`), avoiding the ownership
   mismatch reproduced below. Plain and xtask image builds are both present at
   `.github/workflows/ci.yml:652` and `.github/workflows/ci.yml:659`.

   The development container installs Git (`Dockerfile:26`) and runs as `neo`
   (`Dockerfile:82`), mounting the repository at `/workspace`
   (`docker-compose.yml:35`). `.dockerignore:9` excludes `.git` only from image
   construction, not the runtime bind mount. On this Windows Docker host the
   mounted repository is owned by `0:0`, while `neo` is `1000:1000`. Git rejects
   it as dubious ownership. Both xtask's lookup and the build script's fallback
   therefore fail; the build script succeeds but emits the **wrong ID
   `unknown-build-id`**, rather than
   `093e1c9c9b57d3bf9428f2ead7e2e23b2fb682d1`.

   Reproduction from this worktree (PowerShell; the mount is read-only):

   ```powershell
   docker run --rm --network none --mount 'type=bind,source=E:\AI Automation\workers\jarvis,target=/workspace,readonly' --workdir /workspace/kernel -e RUSTUP_TOOLCHAIN=nightly neoplutarch-dev:latest bash -lc 'git rev-parse HEAD; rustc --edition=2024 build.rs -o /tmp/relay74-build-script && CARGO_MANIFEST_DIR=/workspace/kernel /tmp/relay74-build-script'
   ```

   Git prints `fatal: detected dubious ownership in repository at '/workspace'`;
   the script prints `cargo:rustc-env=NEO_BUILD_ID=unknown-build-id`.
   As a diagnostic control, running the same binary with the process-local
   environment `GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=safe.directory
   GIT_CONFIG_VALUE_0=/workspace CARGO_MANIFEST_DIR=/workspace/kernel` emits the
   correct full reviewed hash and the existing Git watcher paths. No persistent
   Git trust configuration was changed.

   Baseline confirmation in the same container:

   ```sh
   git -c safe.directory=/workspace show 00fd2ab2:kernel/build.rs > /tmp/relay74-baseline.rs
   rustc --edition=2024 /tmp/relay74-baseline.rs -o /tmp/relay74-baseline
   CARGO_MANIFEST_DIR=/workspace/kernel /tmp/relay74-baseline
   ```

   The pre-change script also emits `unknown-build-id`. Route this existing
   default-container ownership/trust problem to build-environment maintenance;
   it does not justify reverting #136. The CI Docker UID override is the
   repository's existing working pattern. CI behavior here was inspected, not
   rerun as a full kernel/QEMU job.

4. **PASS — scratch-repository tests cover the requested cases.**
   `tools/xtask/src/build_id_tests.rs:46` checks two commits and checkout back to
   the first; `tools/xtask/src/build_id_tests.rs:62` checks untracked exclusion,
   tracked dirty state, exact prefix/suffix, and the 32-byte limit;
   `tools/xtask/src/build_id_tests.rs:76` checks an unavailable repo path;
   `tools/xtask/src/build_id_tests.rs:84` checks formatting independently.
   `cargo test -p xtask build_id`: **4 passed, 0 failed**, 101 filtered out.
   The existing-directory no-repo case was additionally verified by the Cargo
   probe described above, because the unit test uses a nonexistent directory.

5. **PASS — #134 flags survived.** `tools/xtask/src/main.rs:6127` retains the
   target-rustflags block, including appending to an existing target variable.
   The adjacent build-ID block at `tools/xtask/src/main.rs:6145` sets a distinct
   environment variable on the same command. The product image selects
   `-Zub-checks=no` at `tools/xtask/src/main.rs:5040` and
   `tools/xtask/src/main.rs:5065`, passes it through
   `tools/xtask/src/main.rs:4963`, and applies it at
   `tools/xtask/src/main.rs:6139`. The existing `NEO_IMAGE_UB_CHECKS=on` switch
   remains effective. This conclusion is from command-construction review;
   no full image build was run.

## Additional validation and environment

- RAG health returned healthy with Ollama and Qdrant available. Discovery also
  used `graft.cmd ask ... --source` before exact searches and direct reads.
- An ephemeral host Cargo package copied the unchanged `kernel/build.rs` and
  printed `env!("NEO_BUILD_ID")`. Assertions passed for initial HEAD, a second
  commit, an unchanged rebuild, detached checkout back, caller override, linked
  worktree, linked-worktree commit, packed refs, commit after packing, and an
  existing directory with no repository. This tested build-script behavior
  without compiling kernel runtime code.
- Docker probes compiled only the unchanged build scripts with the image's
  installed `nightly`. The repository's pinned Linux toolchain was unavailable
  in that image offline; no toolchain download or image rebuild was performed.
- `git diff --check` passed before report commit.
- Pre-existing `.gitignore` and `.ignore` edits were preserved in local stash
  `cfac65f3e15959118ad7d9f088e26ecea33036d1`, named
  `relay74-preserve-preexisting-graft-ignore`. The generated `/graft/` cache is
  excluded locally through `.git/info/exclude`, not committed. These edits are
  not part of the return commit. The requested clean-tree/upstream checks are
  performed after pushing this report.
