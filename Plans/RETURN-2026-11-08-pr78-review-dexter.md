<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 348: #78 (SWGL rasters into a kernel Atrium surface)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md` §348, which runs the **#78** items and the **All four** items of `Plans/DISPATCH-2026-11-08-relays-329-332.md` §329, for #78 only.
**Reviewed:** PR #78 head `7fd9eaf58cc839cd67deec606f5e600743771dbf` (`claude/ot61-r4c-atrium-v2`), its own diff against #77 `c8a065744a779d7d95e06dbcb8299d889718fd90`: `git diff refs/pr/77...refs/pr/78` (10 files, +577/−13). `origin/main` is `5eabdc3722a0cc05b5803c0b1d271700a542fde3`; #78 is stacked on #77, whose base is `13eed2c5` (base-drift note 1).
**Overall: PASS on all five checklist items.** The surface is the kernel's validated descriptor and the mapping is exactly the granted frames; the six new std calls are all in the PAL manifest with provenance and the A4 gate still scans the overlay (34 A4 host tests and 195 xtask tests pass at the head, `cargo fmt --check` clean); the witness is a full-page hand-laid BGRA comparison that a blank or wrong raster fails; the kernel feature is opt-in; and the THINK/vendored records carry the prior art. Five non-blocking notes at the end. The head's own full-tier run is green (4 QEMU shards, run [`42454687770`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42454687770)), and the OT-61 closure job is green ([`42454664137`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42454664137)).

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing lines |
|---|---|---|---|
| #78.1 | The surface the client writes is bounded: size from the kernel's grant; a write past it refused, not clamped | **PASS** (note 3) | `kernel/src/atrium/logic.rs:62-70,105-134`; `kernel/src/atrium/buffer.rs:146-150,194-196,378-381`; `kernel/src/mm/pmap.rs:1194-1203`; `userland/swgl-smoke/src/atrium.rs:88,101,119-123,167-174` |
| #78.2 | Every new std-overlay call is in `tools/ot61-pal-manifest.toml`, and the A4 gate still covers the overlay | **PASS** | `tools/ot61-pal-manifest.toml:56,79-84`; `userland/neoplutarch-std-overlay/std/neoplutarch_rt.rs:54-59,278-320`; `userland/neo-std-pal/a4_gate.rs:11-13,61-63`; `tools/xtask/src/ot61_c05_tests.rs:44-57,110-117,145-152,184-200` |
| #78.3 | The scenario's witness is one a blank or wrong raster would fail | **PASS** | `userland/swgl-smoke/src/atrium.rs:47-57,101,181-210`; `tools/xtask/src/main.rs:18833-18849` |
| All.1 | Each feature is off by default, and the off build is today's image (kill-switch) | **PASS** | `kernel/Cargo.toml:81,498,883,938`; `kernel/src/test_harness.rs:176-177,3059`; `Cargo.toml:99`; `tools/xtask/src/main.rs:8253-8254,8281,8294,8384` |
| All.2 | Prior art: the PR body or THINK carries the survey with licences and dates for what's borrowed | **PASS** | `docs/THINK-2026-10-31-ot61-r4-swgl-floor.md:10-13,38-43,60-71`; `third_party/ot61-swgl/VENDORED.md:64-172` |

## #78.1 PASS — the grant is kernel-side and immutable; commits past it are refused, not clipped

**The grant.** `SYS_BUFFER_CREATE` takes the 24-byte descriptor (`kernel/src/arch/x86_64/syscall.rs:8471-8509`) through `validate_desc` (`kernel/src/atrium/logic.rs:105-134`). Every failure is an error — zero dimension, short or misaligned stride, `bytes != stride × height`, `> 64 MiB`, wider/taller than the output — there is no clamp path. The buffer stores that descriptor unchanged (`kernel/src/atrium/buffer.rs:194-196`) and allocates `ceil(bytes / 4096)` frames (`:150`). `SYS_BUFFER_MAP` maps exactly `frames.len() × 4096` bytes (`kernel/src/mm/pmap.rs:1194-1203`), so a pixel write past the granted mapping faults; nothing rounds it down or silently extends it.

**The client uses the grant it was given.** It creates from `BufferDesc::xrgb(7, 5, 36)` (`userland/swgl-smoke/src/atrium.rs:88`; 180 bytes → one frame/page) and binds that descriptor's width/height/padded stride to SWGL (`:143`). The compositor handle's `query` must return exactly that descriptor, so a kernel-side clamp would be caught (`:119-123`; `sys_buffer_query_impl` returns `encode_desc(&buffer.desc())`, `kernel/src/arch/x86_64/syscall.rs:8586-8605`).

**Past the surface is refused.** `commit_damage` validates the rectangle against the immutable descriptor with `checked_add`, not a clip (`kernel/src/atrium/logic.rs:62-70`), and returns `EINVAL` (`kernel/src/atrium/buffer.rs:378-381`). The scenario exercises the concrete overrun input `commit_damage(client, 5, 0, 3, 1)` — `x + width = 8 > 7` — and requires `EINVAL` (`userland/swgl-smoke/src/atrium.rs:167-174`); a zero-width or zero-height rectangle is refused the same way. So there is no silent clamp at either the descriptor or the damage boundary.

Note 3 records the one literal: `PAGE` is 4096, which is this descriptor's page-rounded grant, but it is not derived from `desc.bytes`.

## #78.2 PASS — all six calls are in the manifest with provenance, and the A4 gate still scans the overlay

The six new trampolines are `buffer_create`, `buffer_map`, `buffer_unmap`, `buffer_query`, `buffer_commit_damage` and `derive`, each a single `syscall4` call in the sanctioned overlay trampoline file (`userland/neoplutarch-std-overlay/std/neoplutarch_rt.rs:54-59,278-320`). Their kernel numbers are the kernel's (`SYS_DERIVE = 8`, `SYS_BUFFER_CREATE = 0x1A0`, `MAP = 0x1A1`, `QUERY = 0x1A2`, `COMMIT_DAMAGE = 0x1AD`, `UNMAP = 0x1B6`), which G2 checks.

All six are in `[gate].allowed_syscalls` (`tools/ot61-pal-manifest.toml:56`) with a provenance line each (`:79-84`). The provenance is specific: `SYS_DERIVE` says attenuation only and names the compositor grant; `SYS_BUFFER_MAP` says role 0 needs `MAP_WRITE`, role 1 needs `MAP_READ`, W^X; `SYS_BUFFER_COMMIT_DAMAGE` says COMMIT and "does not present". `SYS_FB_PRESENT` is deliberately absent, so a bundle ELF that presented would fail the gate.

**The gate still covers the overlay.** `a4_gate.rs` states the scanned surface as the PAL `src/` plus `userland/neoplutarch-std-overlay/` (`:11-13`) and sanctions the overlay's `neoplutarch_rt.rs` as the second trampoline (`:61-63`). The host test's `real_sources()` walks exactly that overlay tree (`tools/xtask/src/ot61_c05_tests.rs:44-57`). `S3_SYSCALLS` is in the positive expectation (`:110-117`) and in the per-syscall unlisted-refusal test (`:184-200`).

**I ran it at the head.** `cargo test -p xtask a4_` → 34 passed, 0 failed, 162 filtered; `cargo test -p xtask` → 195 passed, 0 failed, 1 ignored; `cargo fmt --all --check` → clean. The head's OT-61 closure job ([`42454664137`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42454664137)) also runs the same `ot61-swgl-build` command (`cargo run -p xtask -- ot61-swgl-build`), which at this head builds both `swgl-smoke` and `swgl-atrium-smoke` and runs `verify_std_real_syscall_sites` on each ELF — the gate's non-lexical backstop — and is green.

## #78.3 PASS — the witness is a full-page hand-laid BGRA comparison, not "no panic"

`PATTERN` is hand-written (`userland/swgl-smoke/src/atrium.rs:47`) and `bgra()` spells out the expected memory order (`:50-57`: red `00 00 FF FF`, blue `FF 00 00 FF`, green `00 FF 00 00`). The client pre-fills the whole one-page mapping with `0xCD` (`:101`), so a no-op or blank raster leaves `0xCD` and fails. `want` is built at `:181-187` with the pattern at `y * STRIDE + x * 4` and `0xCD` everywhere else; `seen` is the full page read through the compositor's RO mapping (`:189`); `first_bad` finds the first differing byte (`:190`) and `check(13)` exits 13 on any mismatch (`:206-210`). The comparison therefore catches a blank page, a wrong colour, a flip/transpose, a tight stride, a wrong offset, or a write into the padding/tail. The harness independently requires the exact `ok(13)` and `PASS` serial lines (`tools/xtask/src/main.rs:18833-18849`), so a scenario that dropped the check could not pass the harness.

The scenario is `ignored: false` in `TEST_SCENARIOS` (`tools/xtask/src/main.rs:2314-2322`), so it is in the default selection the head's green 4-shard run exercises.

## All.1 PASS — the feature is opt-in; off is today's image

`kernel/Cargo.toml:498` defines `neo-test-swgl-atrium-smoke = ["neo-test", "browser-runtime", "mem-quota", "atrium-compositor-g1", "neo-test-atrium-runtime"]`; it is **not** in `default` (`:81`). Every component it pulls is itself opt-in: `atrium-compositor-g1` (`:883`) and `neo-test-atrium-runtime` (`:938`). The harness `SCENARIO` and its block are `#[cfg(feature = "neo-test-swgl-atrium-smoke")]` (`kernel/src/test_harness.rs:176-177,3059`). The standalone crate is excluded from the root workspace (`Cargo.toml:99`), so `cargo build --workspace` does not build `swgl-atrium-smoke`; the bin is staged only when the scenario is selected (`tools/xtask/src/main.rs:8253-8254,8281,8294,8384`). The default kernel/image does not include the feature or the binary.

## All.2 PASS — the survey is present, with licences and dates

The THINK records the borrowed closure: SWGL/`glsl-to-cxx`/`webrender_build` 0.70.0 at the spike's pin `2e341fa1`, gleam 0.15.1, glsl 6, `gl_generator`, with the licence list MPL-2.0/MIT/Apache-2.0/BSD-3-Clause/Unlicense (`docs/THINK-2026-10-31-ot61-r4-swgl-floor.md:10-13`), and WebRender's 0.70 licence mix including `r-efi` taken under MIT and `unicode-ident`'s Unicode-3.0 (`:38-43`). The font-backend survey has per-candidate licences and last-commit dates (`:60-71`). The vendored table is headed "checked 2026-10-31" and gives every crate's licence, repository and upstream commit (`third_party/ot61-swgl/VENDORED.md:64-172`).

#78 adds no new third-party code: the Atrium buffer, the overlay module and the scenario are first-party, and the only upstream code it links is the already-surveyed SWGL 0.70.0 from S1. The PR body points at the rung-4 THINK. That satisfies "licences and dates for what's borrowed".

## Verification performed

- `git fetch origin +pull/77/head:refs/pr/77 +pull/78/head:refs/pr/78`; heads `c8a06574` and `7fd9eaf5` match the dispatch.
- `git diff --stat refs/pr/77...refs/pr/78`: 10 files, +577/−13, all reviewed (`docs/THINK…`, `kernel/Cargo.toml`, `kernel/src/test_harness.rs`, `tools/ot61-pal-manifest.toml`, `tools/xtask/src/main.rs`, `tools/xtask/src/ot61_c05_tests.rs`, the two overlay files, `userland/swgl-smoke/Cargo.toml`, `userland/swgl-smoke/src/atrium.rs`).
- Read the kernel create/map/query/commit/unmap implementations, `validate_desc`/`validate_damage`, `SharedPixelBuffer::create`/`commit_damage`, `map_borrowed_region`, and `derive_handle`'s attenuation check.
- Ran at the head `cargo test -p xtask a4_` (34 passed), `cargo test -p xtask` (195 passed, 1 ignored), `cargo fmt --all --check` (clean).
- GitHub check-runs at `7fd9eaf5`: full-tier `CI` run [`42454687770`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42454687770), `pull_request`, success — 4/4 QEMU shards, Host Tests/Format/Lint, Build Checks, RNG seed boot, fuzz, Nightly skip; `Batch report` skipped (not a batch). OT-61 closure run [`42454664137`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42454664137), `push`, success.
- RAG: not needed for this review (record/diff comparison and exact syscall lookups); used git show/grep/read.
- Working tree: this RETURN only on `dexter/pr78-review`; the pre-existing untracked `pr59-review/` directory is untouched.

## Non-blocking notes

1. **Stacked base / drift.** #78 is on #77 `c8a06574`, whose base is `13eed2c5`; `origin/main` is now `5eabdc372`. Against `13eed2c5`, the shared code files moved +1,048/−17 by `origin/main`. This review is of #78's own diff against #77, as §348 asks; the PM's batch assembly of #77+#78 and the gate on the assembled tree remain the merge evidence. The head's green run is #78's own `pull_request` full-tier run on the old base, not a batch run.
2. **The PR body's "not kernel-touching" is wrong at this head.** The diff touches `kernel/Cargo.toml` and `kernel/src/test_harness.rs` (and `tools/xtask/src/main.rs`), and `scripts/worker-close-gate.sh` classifies any `kernel/` or `tools/xtask/` change as kernel-touching. The gate-plan correctly ran the full 4 shards anyway, so this is a wording correction, not a gate gap — but the sentence should not be relied on (the same stale wording was noted on #77).
3. **`PAGE` is a literal, not derived from the grant.** `userland/swgl-smoke/src/atrium.rs:43` sets `PAGE = 4096`; for the 7×5 stride-36 descriptor (180 bytes) the kernel grants exactly one frame, so the fill at `:101` and the read at `:189` are exactly the grant. If this scenario's geometry ever grows past one page, derive `PAGE` and the read length from `desc.bytes`/`query` (or the witness would check only the first page while SWGL writes the rest). Not a defect for 180 bytes.
4. **The THINK says "both handles" are checked after exit; the harness checks the frame accounting.** `docs/THINK-2026-10-31-ot61-r4-swgl-floor.md:120-121` says the kernel side checks every frame comes back (buffer, both mappings, both handles). The harness checks `root.used() == baseline` and `underflows == 0` (`kernel/src/test_harness.rs:3101-3102`), which covers the buffer frames and charges; the handles are reaped with the process and are not separately counted. Wording, not a coverage gap for the frames.
5. **Stale "Gate: pending" in the PR body.** The body's last line says the gate is pending; the head has a green full-tier run on the exact SHA (note in "Verification"). The PM should treat the run, not the sentence, as the close-gate evidence.

## Recommendation to the PM

**PASS — #78 clears this review's five items.** The surface is bounded by the kernel's immutable descriptor and its page-rounded frames, an over-wide damage rectangle is refused `EINVAL` (with the concrete input at `atrium.rs:170`), all six new std calls are in the PAL manifest with provenance and the A4 gate still scans the overlay (verified by the host tests at the head), the witness compares every byte of the page through the compositor's RO view, the feature is opt-in, and the prior art is recorded. Merge on the PM's assembled batch evidence; correct the PR body's "not kernel-touching" and "Gate: pending" sentences, and keep note 1 in view when assembling #77+#78.
