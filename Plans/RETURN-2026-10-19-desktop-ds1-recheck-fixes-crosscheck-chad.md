<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: DS-1 fix-forward re-check (Chad)

**Reviewed:** merged fix-forward `d41286eb` (#56), against the two prior RETURNS at `cc1f4aa9` and `bf6434f5`.

## Verdicts

- **DS-1ab F1 (`cc1f4aa9`): CLOSED.** The launch remains on Home while Arthur is spawned and handshaken. The handoff then activates Arthur, sends the intent, waits for Arthur's discard acknowledgement, presents the unfocused window, and finally presents the focus light. A client commit received while Home is visible snapshots into the WM-owned copy and is deliberately not presented until `show_window`, so the latest committed picture cannot be lost at launch.
- **DS-1c F1 (`bf6434f5`): CLOSED.** The WM creates one `LensStore` at scene construction. Each bloom lends a derived `USE | MAP_WRITE | QUERY | COMMIT` handle, which lacks `GRANT`; a failed Lens is killed, waited, its channels closed, and its process capability revoked. Failure to reap poisons the store and prevents reuse. No per-bloom WM buffer, mapping, or copy plane remains.
- **DS-1c F2 (`bf6434f5`): CLOSED.** `wants_root_lens` tokenizes only on ASCII whitespace and positively matches exactly `ls /`, `list /`, `list directory /`, or `list the directory /`, case-insensitively. Extra words, alternate paths, doubled slashes, and reordered tokens are rejected.

No new findings survived re-verification.

## What I checked

1. **DS-1ab launch order and first frame.** Traced `arthur_session` through the first commit, focus request, `HANDOFF` swap/discard/present sequence, and `commit`. No view is shown before its owner has the route. Commits under Home update `client_copy`; `show_window` composes that copy. The `desktop_home_boot` injection after `view switch=window residue_pixels=0` would fail on the old ordering because `x` and Backspace would land in Home rather than produce Arthur's two keystroke witnesses before yield. Keys entered earlier, while Home remains visible and routed, stay in Home's queue and are discarded on its next handoff turn rather than reaching Arthur.
2. **DS-1c allocation, lending, and failure paths.** Walked every return in `lens::bloom` and `lens::serve`, including channel/open/spawn/create/grant/import/commit/present/reply failures. Pre-spawn failures close locally-created resources; post-spawn failures kill/wait/close/revoke. Only one Lens can be in `bloom` because the WM serves synchronously, and `LensAttempts::shown` prevents another after success. A killed process loses its derived writable mapping; a reap error poisons the shared store. The Lens cannot regrant the buffer because the derived meta-rights contain `USE` only. `draw` fills the entire plane before rendering, so a successful retry cannot expose stale pixels from a prior Lens.
3. **Leak discriminator.** The handle probe allocates 16 channel pairs from first-free slots, closes them, and requires the same 32-slot sequence after two late failures. A leaked per-attempt handle among the roughly eight transient handles changes that sequence. The mapping probe establishes the monotonic mmap stride with two maps; after two blooms, the third map must be exactly one stride after the baseline. Any intervening WM mapping advances the cursor and makes `maps_leaked` nonzero. This witnesses WM handle/mapping leaks; child-process mappings disappear with process teardown.
4. **Strict parse and kill-switches.** Re-derived all four accepted forms and the negative cases. With `desktop-lens` off the new store, slot, worker protocol, attempt budget, and fault injection are absent. With `desktop-home` off the original non-Home launch block remains. The Lens fault path is gated by `desktop-lens-fault-test`, so production Lens builds contain no forced failure.
5. **Gate limits.** This pass ran host KATs, not QEMU. The handle witness deliberately samples the first 32 free slots and the map witness depends on the current monotonic, non-reusing mmap cursor. It does not independently count kernel objects unreachable from the WM handle table. The existing QEMU scenarios remain the end-to-end discriminator for input routing, late failed blooms, and the final Lens capture.

## Verification

`cargo test -p atrium-logic-tests -p arthur-logic-tests -p neo-atrium` passed: 108 + 70 + 48 tests, with two pre-existing performance/preview tests ignored.
