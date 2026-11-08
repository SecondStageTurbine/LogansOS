<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 327: #74 re-check (the syscall personality, the manifest fixes, R3's gate)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-325-328.md` §327.
**Reviewed:** PR #74 head `8ccd675165195e29cbd1ff8afd8b3335b7f815ea` (`docs/THINK-2026-11-08-lessons-from-raven.md`),
against `19defcb8f2672603251e5af65e79ef26dcc94eaf` (the revision I read at relay 320) and the code at the PR's base
`13eed2c54caf76cbef78dad762467df68611546a`. Current `origin/main` is `7072cf58270044fa99d93ccc5e3f5d02eabb8976`.
The PR is docs-only: `git diff --stat 19defcb8 8ccd6751` is one file, +43/−15; `git diff --stat origin/main...8ccd6751`
is one file, +234.

**Overall: PASS.** All four re-check items pass. Two narrow notes: the new manifest parenthetical's "three … instead of
`license`" is off by one as literally worded, and the gate's "handle's fsync count" is not an existing observable (the
test needs a counter; there is a block-op counter it can reuse). Neither is a blocker. The prior item-3 fail is resolved:
§2 now sends a PE `syscall` to the PE personality's table, never the native table, and names the negative test.

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing refs |
|---|---|---|---|
| 1 | §2 states the personality, where it is set, and that it can't be changed by the PE process | **PASS** | THINK:75, 85-91 |
| 2 | The gate is named: a PE test with an inline native-numbered `syscall` that gets the error and leaves no side effect | **PASS**, one implementation note | THINK:92-99 |
| 3 | The two item-1 wording fixes are made | **PASS**, one wording note | THINK:41-42, 50-51 |
| 4 | R3 has the allocator KAT/power-cut gate | **PASS** | THINK:233 |

## Item 1 — PASS: the personality is stated, set at spawn, and immutable

- **The rule is stated.** THINK:85: "a process spawned as a PE program carries the PE personality for its whole life";
  THINK:86: "The syscall dispatcher sends a `syscall` from such a process to the **PE personality's table, never to the
  native table**." THINK:88: any number not in that table, "every NT system-service number among them, gets the
  unknown-syscall error with no side effect."
- **Where it is set.** THINK:75: "set at spawn"; THINK:91: "The personality is set by the spawner (the PE loader's
  launcher)." That is a named setter and a named point in the process lifecycle.
- **It can't be changed by the PE process.** THINK:91: "and can't be changed by the process." Together with THINK:85
  ("for its whole life"), that is the immutability requirement.
- **The supporting code facts are exact at this head.** `SYS_EXIT=0` … `SYS_FSYNC=15` are at
  `kernel/src/arch/x86_64/syscall.rs:90-105` (`SYS_FSYNC` at :105), the dispatch is `match num` at :1042, and the
  `_ => ENOSYS` fallback is at :1406. `sys_fsync_impl` is at :7100. `mov eax, 0x0F` zero-extends to `rax=15`, so it
  reaches `SYS_FSYNC`, exactly as THINK:80 says. A4 is a build-time check over the PAL/overlay
  (`userland/neo-std-pal/a4_gate.rs:10-14,34-35`), so THINK:83-84's "never sees third-party PE code" is right.

**Narrow note (not a fail):** THINK:77 says "only an unassigned number reaches `ENOSYS` (`:1406`)". The fallback is the
unassigned-number path, but a few *assigned* net numbers also return `ENOSYS` under feature gates
(`kernel/src/arch/x86_64/syscall.rs:1276,1289`). The collision at 15 is unaffected; "an unassigned number, or a gated
stub, reaches `ENOSYS`" would be exact. This is a problem-statement wording nit, not the personality rule.

## Item 2 — PASS: the gate is named, with the error and the no-side-effect checks

THINK:92-99 names the exact test:

- a **QEMU negative test**, written before the loader exists, in the same PR as the personality flag (THINK:92);
- a **PE-personality test process** holding a granted file handle executes `rax=15` (`SYS_FSYNC`), `rdi=<handle>`,
  `syscall`, then `rax=1` (`SYS_WRITE`) to its console (THINK:93-94);
- each **must return the unknown-syscall error** (THINK:94);
- **no side effect**: the handle's fsync count is unchanged and nothing reached the console (THINK:95-97);
- the **control**: a native process making the same calls succeeds (THINK:99).

That is a concrete discriminator: with the personality flag the native table is not reached; without it the test would
fsync and print.

**Implementation note:** "the handle's fsync count" has no reader at this head. `sys_fsync_impl`
(`kernel/src/arch/x86_64/syscall.rs:7100-7110`) calls `file.fsync()` and has no counter; `git grep` finds no
`fsync_count`. There is a write/flush counter in `login_cut::Counting`
(`kernel/src/drivers/block/mod.rs:995-1017,1092-1112`), but it is gated by `neo-test-login-cut` and it cuts the boot.
The same PR should add a small per-handle fsync counter, or check the block device's write/flush ops, so the asserted
check has something to read. The gate is *named*; this is the one observable it still needs.

## Item 3 — PASS: both item-1 wording fixes are made

1. **Packaging.** THINK:50-51 now reads: "Packaging refuses to build when a component with an unresolved review is
   enabled by the build (its `cmake_keys` are configured; `scripts/package-native.py:130-135`). A component the build
   doesn't enable can stay unresolved." That matches the Raven code at `74e3e3e5`: the blocker list is components whose
   `review != "reviewed"` **and** whose `cmake_keys` are configured (`scripts/package-native.py:131-135`). `External
   build inputs` has no `cmake_keys`, so it can never block; `SQLite` blocks only when
   `RAVEN_SQLITE_COMPONENT_DIR` is set. The old overstatement ("while any review is unresolved") is gone.
2. **The manifest fields.** THINK:41-42 now reads: "an SPDX `license` or a declared `source_terms` /
   `distribution_license` / `LicenseRef` reference". The manifest at `74e3e3e5` has 14 components: two have no
   `license` key (`Installer upstream libraries`, `source_terms: LicenseRef-Installer-Upstream`; `SQLite`,
   `source_terms`/`distribution_license: blessing`), and a third (`External build inputs`) has
   `license: LicenseRef-Configuration-Dependent`. The old "each with … an SPDX `license`" is gone.

The prior item-1 note about the app matrix is also fixed: THINK:36-37 now says the workflow is Notepad++'s alone and the
other cases max out at 75.

**Wording note (not a fail):** the new parenthetical at THINK:42, "three of its 14 components use those instead of
`license`", is literally true for only two: the third uses a `LicenseRef` *as* its `license` value. If the intended
statement is "three carry no SPDX id", it is correct; "three of its 14 components carry no SPDX id" would remove the
off-by-one reading. One-word tightening, not a reason to bounce.

## Item 4 — PASS: R3 has the allocator KAT, the power-cut run and the O(1) timing

THINK:233, R3, now carries the gate:

- a **host KAT** in `tools/neofs-commit-logic-tests` that pins generation G, frees a block after G, and proves the
  block is not reused until the pin is released;
- the **same model's power cut** at every write and flush, with a pin held, after which G still mounts;
- **`install_fresh_4k` timed with a snapshot taken**, to show the snapshot itself costs O(1).

The harness supports the shape: `tools/neofs-commit-logic-tests/src/model.rs:1-15` is a write-back-cache disk whose
power can be cut before any event (a write or a flush), with any subset of unflushed writes landing and flushes able
to fail; `src/tests.rs:1-6` runs each script, then cuts before every event, every landing subset, and every flush
failure. `install_fresh_4k` is a real scenario (`tools/xtask/src/main.rs:12529,12767`) with recorded timings
(`tools/xtask/data/hosted-timings.tsv:160`). The `pin` is new — the model has no pin concept today — so the KAT is
the spec for adding it, which is right for a design gate.

**Minor phrasing note:** "power cut at every write and flush" is slightly loose; the harness cuts before every event
(write *or* flush) and separately fails flushes. Same model, concrete gate.

## Verification performed

- Read the THINK at `8ccd6751` and the revision diff `19defcb8..8ccd6751` (one file, +43/−15), plus the prior RETURN
  `Plans/RETURN-2026-11-08-pr74-design-dexter.md`.
- Re-read Raven at `74e3e3e586d4c2df1b30fdb724ec1b15ef9243ef` from `/tmp/raven`: `scripts/package-native.py:120-135`,
  `LICENSES.json` (14 components, licence fields), `scripts/benchmark-app-matrix.py:181-194`.
- Checked our code at the PR head: syscall numbers and the `ENOSYS` fallback
  (`kernel/src/arch/x86_64/syscall.rs:90-105,1042,1406`), `sys_fsync_impl` (`:7100`), the write guard's counters
  (`kernel/src/drivers/block/mod.rs:995-1017,1092-1112`), A4 (`userland/neo-std-pal/a4_gate.rs:10-14,34-35`),
  the NeoFS commit-logic model/tests (`tools/neofs-commit-logic-tests/src/model.rs:1-15`, `src/tests.rs:1-6`), and
  `install_fresh_4k` (`tools/xtask/src/main.rs:12529,12767`; `tools/xtask/data/hosted-timings.tsv:160`).
- Did not build or run anything; the THINK is docs-only and says nothing was built or run. RAG health was not queried;
  used `grep`/`read` throughout.
- Working tree: this branch contains only this RETURN. The pre-existing untracked `pr59-review/` directory is not
  staged and is not part of this branch.

## Land note (not a checklist item)

The PR head's base is `13eed2c54`; current `origin/main` is `7072cf58`. The THINK's `:1406` is correct at the PR head,
but `_ => ENOSYS` is at `:1415` on current `main` after the +65-line syscall change, so a rebase should update that one
line ref. The `:90-105` constants block is unchanged (`SYS_FSYNC=15` is still :105), and the `mov eax, 0x0F` collision
is unaffected.

No code, THINK or source change.
