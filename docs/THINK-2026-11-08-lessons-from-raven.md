<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: lessons from Raven (Windows programs on Linux)

**Date:** 2026-11-08. **Seat:** E. **Status:** docs only; proposals for the PM, no build.
**Asked by:** J.c. (2026-11-07), `Plans/HANDOFF-2026-11-03-pause.md`, "learn from Raven".
**Source read:** [mirkobrombin/raven](https://github.com/mirkobrombin/raven) at `74e3e3e586d4c2df1b30fdb724ec1b15ef9243ef`
(committed 2026-11-07, one main author). The source was read from a `git` checkout; nothing was built or run.

**Licence boundary.** Raven's host runtime (`native/` outside `native/guest/` and `native/upstream/`, plus
`scripts/`) is **GPL-3.0-or-later**. It was read for principle only, and **nothing in this THINK, or in any build it
proposes, copies its code, data, tables or text**. Its guest adapters (`native/guest/`) are MIT, and its imported data
carries its own licences (listed in §6). Nothing here proposes taking any of it either. Where a later build might, §6
names the licence and the file to re-check.

## 1. What Raven is, measured

- **The program runs natively on the CPU**, as a ptrace'd child of the Linux host. Raven fakes the Windows kernel from
  outside the process:
  - A **seccomp filter** lets through only the system calls made from Raven's own "gate" instruction. Any other
    syscall stops the guest for the tracer (`SECCOMP_RET_TRACE`, `native/cpu/native_cpu.cpp:747-758`).
  - Windows API calls reach the host as **breakpoint traps at export stubs**.
  - A **shared-memory "served" queue** lets the guest hand a call over without a ptrace stop (`served_queue.h`,
    `native_cpu.h:53-56`).
- **It answers at the Win32 API layer, not at NT system-call numbers.** The host registers **2,040 handlers keyed by
  export name** (`ApiRegistry`, `native/nt/api.h:73`), and only **62 of them are `Nt*` functions**. On top of those
  sit **58 guest adapter DLLs**: PE code that runs inside the program and implements comctl32, msi, the CRT,
  gdiplus, and so on.
  - Guest DLLs: about 259,500 lines of C (MIT).
  - Host `nt/`, `cpu/` and `desktop/`: about 79,200 lines of C++ (GPL).
- **Its acceptance test is an app matrix** (`scripts/benchmark-app-matrix.py`). Each real installer (7-Zip, 7-Zip
  MSI, Notepad++, VLC, Audacity, …) runs silently in a fresh prefix, under Raven and under Wine. Each case scores
  **25 points per stage observed**: installed (the program file is where Windows would put it), process started,
  main window shown (title matched), and one automated workflow. **An untested workflow earns no points.** The
  results go to `results.json` and `results.md`, with a screenshot of the window as evidence.
- **Licences are a machine-readable manifest checked in CI.**
  - `LICENSES.json` has a `schema_version`, a `review_date`, and `components`, each with `name`, `paths` or
    `cmake_keys`, an SPDX `license`, a `review` status (`reviewed` / `pending-maintainer-review` /
    `requires-release-inventory`), a `provenance` file and a `boundary` note.
  - `.github/scripts/check_licenses.py` fails CI in three cases:
    - a guest file that no component lists;
    - a listed path that isn't tracked;
    - a licence that is neither a known SPDX id nor a declared reference.

    It also requires every directory under `native/upstream/` to carry its own notice.
  - Packaging refuses to build while any review is unresolved (`scripts/package-native.py:130-135`).
- **A snapshot before every installer, and a one-command restore** (`native/nt/snapshot.cpp`, `native/main.cpp:159-170`,
  `375-384`).
  - A program "that looks like an installer and that no other program started" triggers a copy of the whole prefix
    first. Files unchanged since the previous snapshot are hard links to it, so they share one copy.
  - The ten newest install snapshots are kept.
  - `--restore-prefix` puts one back **and keeps what the prefix held before as a new snapshot**, so a restore can
    itself be undone.
  - A failed snapshot is reported but doesn't block the install.

## 2. An NT personality for LogansOS: our own dispatcher, not ptrace

Raven needs ptrace and seccomp because it doesn't own the kernel: containment and interception both have to come from
outside the process. We own the kernel and already contain every process with capabilities (Article 29.3; the
`execute_action` cap-check), so the two jobs separate cleanly.

| Concern | Raven (Linux host) | LogansOS (proposal) |
|---|---|---|
| Containment | ptrace + seccomp: every non-gate syscall stops the guest | **already ours:** a PE process is a normal process with only the caps it was granted; nothing extra to trap |
| Where Windows calls are answered | breakpoint traps on export stubs, served by a host process; a shared-memory fast path to avoid stops | **in our own code, in the process:** a PE loader maps the program and resolves its imports to *our* guest DLLs, and those call our syscalls and IPC directly; no trap per API call |
| What the "kernel" answers | named Win32 exports (2,040), very few `Nt*` (62) | the same split: **answer at the documented Win32 export layer** and never emulate NT system-call numbers, which are undocumented and change between Windows builds |
| Per-call cost | a ptrace stop or a queue hand-off per host-served call | an ordinary call into our DLL, plus our syscall when it needs the kernel |

**What this means for a future design (not a build now):**
1. **The kernel learns nothing about NT.** No NT syscall table in `kernel/src/arch/x86_64/syscall.rs`. A Windows
   program's `syscall` instruction would only ever come from our own DLLs, so it is one of our syscalls. A stray NT
   syscall from a program that inlines one (rare: anti-cheat, some packers) gets our "unknown syscall" error, which is
   the right failure.
2. **The personality lives in userland:**
   - a PE/COFF loader, including relocations, TLS callbacks, delay-load and side-by-side manifests (Raven has all four);
   - guest DLLs for the Win32 surface (kernel32, user32, gdi32, advapi32 registry);
   - one personality server for what must be shared between processes (the registry, named objects, window
     ownership), reached by our IPC channels and holding only the caps it needs.
3. **What Raven's split teaches:**
   - most of the surface is library code that runs in the guest (about 3.3 lines of guest code for every line of
     host code);
   - the host answers what needs authority or sharing: files, processes, the registry, windows, input;
   - that is the same line our capability model draws, so the personality server is small and everything else is
     ordinary unprivileged code.
4. **Graphics:** Raven maps Direct3D to Vulkan through DXVK (zlib) and vkd3d-proton (LGPL-2.1, never copied). Ours
   would go to our own GPU path; this is far off the critical path and depends on a Vulkan-class driver we don't have.
5. **Difficulty, in our units:**
   - the PE loader and a console-program subset (kernel32 file and console I/O) are bounded: a few build-gate cycles,
     with byte oracles for the PE structures;
   - a GUI subset (user32/gdi32 onto Atrium) is many cycles of genuinely uncertain work;
   - installers (MSI's custom actions, Inno, NSIS) are the deepest layer, many cycles: Raven needed MSI, setupapi and
     advpack adapters of its own (`native/guest/raven-msi`, `raven-setupapi`, `raven-advpack`).

   This is a long-range program, off the critical path.

## 3. The app-matrix acceptance test, for our own apps

Our scenarios are pass/fail needles. A staged score shows how far an app gets, and what regressed, with no guessing.

**Proposal:** an `app-matrix` xtask over our own apps (neo-shell, the Lens, the editor, the file manager, the terminal,
later the browser), each run fresh in QEMU from the installer image. Stages, 25 points each:

| Stage | Our oracle |
|---|---|
| Installed | the installer's W5 verify line for the app's files (`install: step=W5 verify … content_hash_matches=true`) |
| Started | the process-spawn line for the app's image, from a cold boot of the installed disk |
| Main window shown | the WM's map line for the app's window, plus a framebuffer hash taken under the serial script, kept as evidence like Raven's screenshot |
| Workflow | one scripted task per app (open a file, type, save, read it back), checked by its own needle |

- Raven's rule kept: **a stage without a test scores 0**, so an app's score can't look better than the evidence.
- The output is `target/app-matrix/results.json` plus a Markdown table, the same shape as `neo-test` timings.
- This is a **reporting tier, not a merge gate** at first: the scores ride in the nightly full run, and a drop is a
  regression to triage. A stage can be promoted into the merge-blocking tier only through the escaped-defect rule
  (PM-reviewed), as for any scenario.
- **Cost:** about one QEMU boot per app per run, so it belongs in the batch or nightly tier and not the quick tier
  (the OT-58 budget).

## 4. A machine-readable licence manifest for `third_party/`

Today each vendored directory has a prose `VENDORED.md`, and `cargo xtask vendor-verify` checks the crates under
`third_party/*/vendor/` against their `.cargo-checksum.json`. Nothing checks the licence claims mechanically.

**Proposal:** `third_party/LICENSES.json`, in our own format (none of Raven's text or schema is copied):

```json
{
  "schema_version": 1,
  "components": [
    {
      "name": "fatfs",
      "paths": ["third_party/fatfs"],
      "license": "MIT",
      "upstream": "https://github.com/…",
      "commit": "<sha>",
      "reviewed": "2026-11-08",
      "review": "reviewed",
      "maintainer": "<seat or owner, for an abandoned upstream>"
    }
  ]
}
```

**`cargo xtask licence-check`** (host-only and fast, so it can run in `Build Checks` on every PR) fails when:
1. a tracked file under `third_party/` is in no component;
2. a listed path isn't tracked;
3. a `license` is not on **our allowed list**. That list comes from the survey rule:
   - MIT, Apache-2.0, BSD-2/3-Clause, Zlib, ISC, MPL-2.0, Unicode-3.0, Unlicense, CC0-1.0, and their `OR`
     combinations;
   - GPL, LGPL and AGPL fail by name, with the rule quoted.
4. a component has no licence file of its own in its directory, or no `VENDORED.md`;
5. a component's `review` is not `reviewed` and a product image is being built (the image build refuses, like Raven's
   packaging step).

`VENDORED.md` stays the human page. The JSON is the checked claim, and the check is the byte-oracle idea applied to
provenance. A one-time build seeds it from the existing `VENDORED.md` files, which has to include the twelve current
`third_party/` directories, the `ot61-browser` vendor tree with its Python wheels among them.

## 5. Snapshot before an installer vs. our L2.1 always-undo

| | Raven | LogansOS today |
|---|---|---|
| Unit | the whole prefix (a Windows install's `C:` and registry) | one action: Arthur's `write_file` is snapshot-backed, and `trash_file`/`move_file` can be turned back into a trash entry (ADR-069's R and T tiers) |
| When | before any program that looks like an installer and was started on its own | every reversible action, logged in the hash-chained L2.1 action log (`.system/`, written only by the WM) |
| How | copy the tree; hard-link files unchanged since the last snapshot; keep the ten newest | per action: the old bytes or a trash entry, plus a log record |
| Undo of the undo | yes: a restore saves the current state as a snapshot first | per action only |
| Cost | proportional to the files changed since the last snapshot | proportional to the action |

**What to take:**
1. **The two compose; they don't compete.** Per-action undo suits Arthur's actions, which are small and known in
   advance. A whole-scope snapshot suits an installer or an update, which changes many files we can't list in advance.
   L3 (updates and rollback) needs the second kind.
2. **On NeoFS a snapshot can be a pinned generation, not a copy.** NeoFS is copy-on-write with numbered generations
   (`commit_logic`), so "snapshot before install" can mean recording the current root and keeping its blocks from
   reuse until the snapshot is released. That makes the snapshot itself O(1). The real design item is the allocator:
   blocks a pinned generation still references must stay out of the free map, which today's durable-generation
   deferred-free check (`commit_logic` rule 2) only covers for the newest durable generation. That needs its own THINK
   before any build.
3. **Make a restore undoable,** as Raven does: restore by switching to the snapshot's root and pinning the current one
   as a new snapshot, and record both in the L2.1 log.
4. **A failed snapshot should block the installer,** not just warn (Raven only warns), since it is our undo guarantee.
   On a full disk the user gets "not enough space to keep an undo point" and decides.
5. **Keep a bounded number** (Raven keeps ten), and show them in the Lens next to "What Arthur did".

## 6. Licences of anything this THINK might later borrow

Nothing is borrowed now. If a later build takes anything from Raven, re-check that file's own notice first.

| Item | Licence | Usable? |
|---|---|---|
| Raven host runtime (`native/` outside `guest/` and `upstream/`, `scripts/`, `.github/scripts/`) | GPL-3.0-or-later | **no**: principle only |
| Raven guest adapters (`native/guest/`, listed in its `LICENSES.json`) | MIT | yes, with notice; re-check each file |
| Windows code-page and locale data (`native/upstream/windows-codepages/`, …) | Unicode-3.0 | yes (on our allowed list since 2026-10-31) |
| DXVK | Zlib | yes, with notice |
| vkd3d-proton | LGPL-2.1-or-later | **no** |
| Skia | BSD-3-Clause | yes |
| libpng | libpng-2.0 | needs a PM decision (not on our list) |
| SQLite | blessing (public domain) | needs a PM decision (Raven itself lists it as pending review) |
| Fonts (Liberation, DejaVu) | OFL-1.1, Bitstream Vera | data, not code; needs a PM decision |

## 7. Proposed follow-ups (for the PM to route)

| # | Item | Kind | Depends on |
|---|---|---|---|
| R1 | `third_party/LICENSES.json` + `cargo xtask licence-check` in `Build Checks` | host-only build, a no-gate lane | none |
| R2 | `app-matrix` reporting tier over our own apps | xtask + scenarios, batch/nightly tier | the installer image; one needle per stage |
| R3 | THINK: NeoFS pinned-generation snapshots for installs and updates (allocator rule) | design | L3's update design |
| R4 | THINK: a userland Win32 personality (PE loader, guest DLLs, a personality server over IPC) | long-range design | none now; far off the critical path |
