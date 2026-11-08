<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 320: #74 design read (lessons from Raven)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-315-321.md` §320.
**Reviewed:** PR #74 head `19defcb8f2672603251e5af65e79ef26dcc94eaf` (`docs/THINK-2026-11-08-lessons-from-raven.md`), against the code at `cf41929d76bf43be14609813da0484a0e17ebe08` (then `origin/main`). The branch was rebased onto `42ccc9238` after review; `git diff --stat cf41929d 42ccc9238` is `Plans/` only, so every code reference below is unchanged. Branch `dexter/pr74-design`.
**Source checked:** `mirkobrombin/raven` at `74e3e3e586d4c2df1b30fdb724ec1b15ef9243ef` (2026-11-07), fetched and read from a checkout. Paths below beginning `native/`, `scripts/` and `.github/` are Raven's at that commit; `kernel/`, `userland/` and `tools/` are LogansOS at the main SHA above. I also read PR #74's newest `PM:` comment (2026-11-08T05:53:43Z).

**Overall: FAIL.** Items 2 and 4 pass. Item 1 has two narrow factual overstatements. Item 3 is the substantive fail: the THINK says a PE program's stray NT `syscall` gets our unknown-syscall error, but our syscalls are number-keyed and NT numbers collide with real LogansOS numbers, so a PE binary can invoke a real syscall; the A4 gate is a build-time check over our PAL/overlay, not a runtime guard over third-party PE code.

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing refs |
|---|---|---|---|
| 1 | Raven's facts are stated accurately and cited to its source at `74e3e3e5` | **FAIL** (narrow; two sentences) | THINK:36-38, 45; Raven `scripts/package-native.py:130-135`; `LICENSES.json:599-608,612-623,657-664` |
| 2 | Nothing proposes copying the GPL-3.0 host; every borrowable piece names its licence | **PASS**, one note | THINK:9-13, 84-85, 183-197; Raven `LICENSE`, `LICENSES.md`, `LICENSES.json` |
| 3 | The NT-personality sketch fits the capability model and the A4 syscall backstop | **FAIL** | THINK:57-59, 63, 69-72; `kernel/src/arch/x86_64/syscall.rs:90-105,1043-1161,1406`; `userland/neo-std-pal/a4_gate.rs:10-14,34-35,53-55` |
| 4 | The app matrix, licence manifest and installer snapshot each have a concrete next step and gate | **PASS**, one note | THINK:99-115, 119-155, 167-181, 199-206; `kernel/src/installer.rs:900`; `tools/neofs-commit-logic-tests` |

## Item 1 — measured facts pass; two summary sentences do not

**The load-bearing facts are exact.** I counted them at the cited Raven commit: 2,040 unique `registry.add("…")` names (`native/nt/api.h:74`; the class citation is one line off, the class starts at 74, not 73), 62 unique `Nt*` names, 58 directories under `native/guest/`, 259,538 lines of guest C, and 79,212 lines of host C++ in `native/nt`, `native/cpu` and `native/desktop` once test files are excluded (the THINK's "about 79,200"). The seccomp block at `native/cpu/native_cpu.cpp:747-758`, the snapshot code at `native/nt/snapshot.cpp` plus `native/main.cpp:159-170,375-384`, the ten-install-snapshot prune, the restore that first snapshots the current prefix, and the warn-but-continue failed snapshot are all as stated. The app-matrix stages, 25-point scoring, `results.json`/`results.md` and screenshot evidence are at `scripts/benchmark-app-matrix.py:6-8,147,175-196`. The licence manifest's three CI failure classes plus the `native/upstream/` notice rule are at `.github/scripts/check_licenses.py:33-56`.

**FAIL (narrow) — two sentences overstate Raven's manifest.**

1. **THINK:45, "Packaging refuses to build while any review is unresolved".** `scripts/package-native.py:130-135` raises only for a component whose `review != "reviewed"` **and** whose `cmake_keys` are configured in the build. `External build inputs` (`LICENSES.json:657-664`) has no `cmake_keys`, so `requires-release-inventory` can never block; `SQLite` (`:612-623`) blocks only when `RAVEN_SQLITE_COMPONENT_DIR` is set. Concrete input: a build with SQLite disabled — the manifest still has a `pending-maintainer-review` component, and packaging does not refuse. Fix: "packaging refuses to build when an unresolved component is enabled by the build".
2. **THINK:36-38, "`components`, each with … an SPDX `license`".** Three components have no `license` key: `Installer upstream libraries` (`:599-608`, `source_terms`), `SQLite` (`:612-623`, `source_terms`/`distribution_license`) and `External build inputs` (`:657-664`, a `LicenseRef`). Concrete input: `python3 -c` over `LICENSES.json` shows 14 components and no `license` for those three. Fix: "a `license`, or a declared `source_terms`/`distribution_license`/`LicenseRef` reference".

**Note, not a fail:** the app matrix implements a workflow only for Notepad++ (`scripts/benchmark-app-matrix.py:181-192`), so the other six cases max out at 75/100. THINK:31-33's "an untested workflow earns no points" is accurate; "one automated workflow" should say "one for Notepad++, the rest untested".

## Item 2 — PASS

**No GPL copying is proposed.** Raven's root `LICENSE` is GPL-3.0-or-later with the Raven-owned guest adapters excluded; `LICENSES.json` lists the host as GPL-3.0-or-later and the guest-adapter component as MIT; `LICENSES.md` gives DXVK as zlib and vkd3d-proton as LGPL-2.1-or-later. The THINK reads the host for principle only (THINK:9-13), never copies its code, tables or text, and its §4 JSON is stated as our own format (THINK:122). §2, §3 and §5 are design comparisons, not code lifts. §6 names each piece the dispatch asks about: MIT guest adapters, Unicode-3.0 code-page/locale data, DXVK zlib, Skia BSD-3-Clause, with vkd3d-proton explicitly excluded and libpng/SQLite/fonts held for a PM decision (THINK:187-197). That matches Raven's own `LICENSES.md` and `LICENSES.json`.

**One note to narrow before a build borrows:** THINK:11 and :190 shorthand "`native/guest/`" as MIT. The manifest's MIT component is "Raven guest adapters" (the exact file list in `LICENSES.json`); `native/guest/raven-installer/patches` is a separate `LicenseRef-Installer-Upstream` component (`LICENSES.json:599-608`), and the provisioned installer libraries (zlib, libxml2, libxslt, IJG JPEG, LibTIFF) carry their own notices. §6 does not list those four upstream libraries. "Re-check each file" covers this, but if the MSI/setupapi/advpack work ever borrows from `raven-installer`, the licence row should name them. Not a fail under §320's list.

## Item 3 — FAIL: the raw-syscall posture is not what the THINK says it is

The sketch's containment sentence (THINK:63) is "a PE process is a normal process with only the caps it was granted; nothing extra to trap". That is consistent with our capability model. The next step (THINK:69-72) then says: "A Windows program's `syscall` instruction would only ever come from our own DLLs, so it is one of our syscalls. A stray NT syscall … gets our 'unknown syscall' error, which is the right failure." That is false as written.

- Our syscall dispatch is keyed by number: `SYS_EXIT=0`, `SYS_WRITE=1`, `SYS_SPAWN=2`, … `SYS_CLOSE=14`, `SYS_FSYNC=15` (`kernel/src/arch/x86_64/syscall.rs:90-105`). The match dispatches those handlers (`:1043-1161`); only an unassigned number reaches `_ => ENOSYS` (`:1406`).
- Windows x64 puts the NT SSN in `eax`; our entry reads the number in `rax` (the A4 gate's own G4 rule, `userland/neo-std-pal/a4_gate.rs:34-35`). Concrete input: a PE binary executes `mov eax, 0x0F; syscall` (the NT `NtClose` SSN on current x64 builds). Our number 15 is `SYS_FSYNC`, so the kernel runs `sys_fsync_impl` (`:1161`), not ENOSYS. A deliberate PE can do more: it can set `rax` and our argument registers to invoke any real LogansOS syscall directly. The kernel cannot tell the caller's code origin.
- Capabilities still bound the process's authority, so this is not a capability escape. It does bypass the Win32 API/personality layer's argument validation and any policy that layer would enforce, and it contradicts the THINK's stated failure mode.
- A4 does not catch it. A4 is a **build-time** check: it scans `.rs`, `.sh` and `.py` under `userland/neo-std-pal/src/` and `userland/neoplutarch-std-overlay/` (`userland/neo-std-pal/a4_gate.rs:10-14`) and disassembles built std-real ELFs (`:53-55`). Third-party PE code mapped by a future loader is outside both surfaces. The THINK never names A4, and the sketch's "no NT syscall table in the kernel" is the right A4-compatible stance, but the runtime story is missing.

**Fix — choose and state one posture:**

- **(a) Accept raw syscalls as capability-bounded.** Delete the "only ever from our DLLs" and "unknown syscall" claims; state that a PE process may issue any LogansOS syscall directly, that the kernel capability check is the only security boundary, and that the Win32 API layer is a compatibility/validation layer, not a security boundary.
- **(b) Make the API layer a gate.** Add a per-process runtime check (an allowed syscall-site/IP range, or a PE-process flag that refuses raw syscalls outside a sanctioned trampoline) and prove it with a QEMU negative test: a PE binary that inlines `syscall` must be refused. A4 stays the build-time gate for our own PAL/overlay; the runtime check is the complement. Do not add an NT syscall table to the kernel (that would break the THINK's own constraint).

Either way, add the failing test as the concrete discriminator: a PE test image that sets `rax=15` / `rdi=<granted file handle>` and executes `syscall`; today it reaches `SYS_FSYNC`, so the test fails until the chosen posture is implemented.

**Minor, same item:** THINK:57-59 cites the `execute_action` cap-check as the containment for "every process". `execute_action` is Arthur's userland action surface (`userland/libneo/src/action.rs:317-325`); it is not how a PE process is contained. The kernel capability checks on the syscall path are the general boundary, and the sentence should say so.

## Item 4 — PASS, one note

- **App matrix.** THINK:99-115 plus R2 (`:204`): an `app-matrix` xtask over our apps, 25 points per stage, oracles for installed (the W5 verify line, `kernel/src/installer.rs:900`), started, main window plus framebuffer hash, and a per-app workflow; output `target/app-matrix/results.json`; a nightly reporting tier, not the quick tier, with promotion only through the escaped-defect rule. The next step is the xtask and one needle per stage; the gate is the nightly/batch tier. Concrete.
- **Licence manifest.** THINK:119-155 plus R1 (`:203`): `third_party/LICENSES.json`, `cargo xtask licence-check` in `Build Checks`, five failure conditions, a one-time seed from the 12 current `third_party/` directories plus the `ot61-browser` wheels, and an image-build refusal for unreviewed components. The allowed list matches `CLAUDE.md:915-917`. The next step is the host-only build; the gate is `Build Checks` on every PR. Concrete.
- **Installer snapshot.** THINK:167-181 plus R3 (`:205`): per-action undo and whole-scope snapshots compose; a NeoFS snapshot can be a pinned generation (O(1)) once the allocator keeps a pinned generation's blocks out of the free map; a restore is itself undoable; a failed snapshot blocks the install; the count is bounded. The next step is the R3 THINK. **Note:** R3's "gate" is only a dependency ("L3's update design"), not a test. Add the acceptance test to R3: a host KAT in `tools/neofs-commit-logic-tests` that pins generation G, frees a block after G, and proves the block is not reused until the pin is released; plus the existing `install_fresh_4k` timing scenario to show the snapshot is O(1). That makes the design's gate concrete.

## Verification performed

- Read the THINK at `19defcb8` and the PR diff (1 file, +206, docs only). Read PR #74's newest PM comment.
- Fetched Raven at `74e3e3e586d4c2df1b30fdb724ec1b15ef9243ef` and checked the cited files directly: `native/nt/api.h`, `native/cpu/native_cpu.cpp`, `native/cpu/native_cpu.h`, `native/cpu/served_queue.h`, `native/nt/snapshot.cpp`, `native/main.cpp`, `scripts/benchmark-app-matrix.py`, `LICENSES.json`, `LICENSES.md`, `LICENSE`, `.github/scripts/check_licenses.py`, `scripts/package-native.py`; counted handlers, Nt* names, guest dirs and lines with `grep`/`wc`.
- Checked our syscall numbers, dispatch and ENOSYS fallback (`kernel/src/arch/x86_64/syscall.rs`), and A4's scanned surface (`userland/neo-std-pal/a4_gate.rs`).
- RAG health at `http://127.0.0.1:8404/health` did not respond; used `grep`/`read` throughout (not a blocker).
- Did not build or run anything; this is a design read, and the THINK itself says nothing was built or run.
- Working tree: the commit is only this RETURN; the pre-existing untracked `pr59-review/` directory is not staged and is not part of this branch. No code, THINK or source change.

## Recommendation to the PM

Do not route R4 until the raw-syscall posture is decided and written down: either the Win32 layer is a compatibility layer and capabilities are the only boundary, or a runtime syscall-site gate is added and proven with the inline-syscall PE test. The architecture in §2 is still the right long-range shape; this is a containment sentence and a test, not a redesign. Fold the two item-1 wording fixes into the same THINK edit, and give R3 the allocator KAT/power-cut gate. The app matrix and licence manifest can be routed as proposed.
