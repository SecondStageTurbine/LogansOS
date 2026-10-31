<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: post-merge review of A9-D, the turning emblem while Arthur thinks (#155)

**MODEL:** GPT-6-Luna. **EFFORT:** high. **Seat:** Janus.
**Reviewed SHA:** `d3603830fb9afcb8a0ef9716dd6bd0ee1a28bf76` (pinned squash on main).
**PR:** [#155](https://github.com/SecondStageTurbine/LogansOS/pull/155), merged 2026-10-27.

## Verdict: FAIL — request PM revert-first

The protocol validates the rejected rectangles, but the refusal path returns from
`arthur_session` and exits `neo-init` instead of rejecting the request while keeping
the WM alive. For example, with a 640x400 Arthur surface, a valid `OP_BUSY` followed
by `OP_BUSY { x: 639, y: 0, width: 2, height: 1 }` passes decoding, fails the surface
bounds check, returns 120, and unwinds `run()` to `_start`/`sys_exit`. This is a
concrete WM/session termination caused by the required refusal case. The kernel
subsequently reinitializes the framebuffer and draws its boot sequence; this is not
an indefinitely stuck emblem, but it is not a live compositor refusal.

The failure is post-merge. PM should revert-first before another implementation
attempt. No source changes or revert were made in this review.

## Checklist

1. **FAIL — OP_BUSY parsing and refusal behavior.** Decode rejects zero serial and
   zero width/height (`userland/libneo/src/atrium_protocol.rs:544`); registry bounds
   use widened `u32` sums and channel provenance (`userland/libneo/src/atrium_protocol.rs:779`).
   The hand-laid byte oracle and registry tests cover zero fields, oversized/edge
   rectangles, wrong provenance, and no commit-serial consumption
   (`tools/atrium-logic-tests/src/lib.rs:1042`, `:1097`). But for the concrete
   639+2 rectangle above, `Request::Busy` treats the validation error as fatal and
   returns 120 (`userland/neo-init/src/desktop_session.rs:1454`). The result
   propagates out of the session (`userland/neo-init/src/desktop_session.rs:1063`)
   and `_start` exits neo-init (`userland/neo-init/src/main.rs:155`). Thus the WM
   does not survive a refused rect. A valid request on a different surface channel
   is similarly rejected by provenance validation, then follows the same fatal
   branch. The decoder's `u32` addition cannot wrap for two `u16` operands.

2. **PASS, with teardown caveat — the emblem ends.** An ordinary model/generation
   error is rendered and committed by Arthur (`userland/neo-arthur/src/main.rs:1056`);
   the WM commit copies the client pixels, takes `end_busy()`, unions its damage,
   then presents (`userland/neo-init/src/desktop_session.rs:1744`). Replacing a
   valid Busy request first ends the previous state and damages both old and new
   placements (`userland/neo-init/src/desktop_session.rs:5180`). Arthur's supported
   generation path is synchronous, so it cannot send a Home-yield request while
   that generation is in progress; Home therefore cannot normally toggle the
   active emblem. If the channel closes during Busy, `try_recv_request` returns
   24 (`userland/neo-init/src/desktop_session.rs:1366`) and unwinds the entire
   desktop session; neo-init exits and the kernel reinitializes the framebuffer
   and renders its boot sequence (`kernel/src/main.rs:457`, `:964`). That ends the
   visible overlay by destroying the desktop, rather than by clearing/repainting
   the busy state. The closed-channel behavior is severe but does not leave an
   emblem drawing indefinitely.

3. **PASS — clipping, Home visibility, repaint, and independent oracle.** Busy
   placement is intersected with the client rectangle (`userland/neo-init/src/desktop_session.rs:5161`);
   `busy_dest()` returns `None` while Home is shown (`:5161`), and composition
   includes the layer only in the client view (`:902`). End-of-busy repaint is
   covered by the commit damage union above. The compositor KAT uses an independent
   floating-point source-over and checks every pixel within/outside the emblem
   (`tools/atrium-logic-tests/src/busy_emblem_kats.rs:79`); a second all-pixel test
   clips an emblem crossing the client's bottom-right edge (`:105`). The KATs test
   composition/clipping, not the runtime end/repaint transition.

4. **PASS for xtask-built product/test images; no universal feature guard.** A WM
   without `desktop-busy` does not recognize the feature-gated `OP_BUSY` verb
   (`userland/libneo/src/atrium_protocol.rs:43`, `:544`); its receive error returns
   24 and terminates the session (`userland/neo-init/src/desktop_session.rs:1406`).
   The xtask product mapping includes `desktop-busy` and the matching Arthur
   mapping includes `arthur-busy-emblem` (`tools/xtask/src/main.rs:5550`, `:5764`);
   `product_image_pairs_the_busy_emblem` asserts those two mappings
   (`tools/xtask/src/main.rs:17498`). The dedicated `desktop-busy-boot-test` mapping
   also selects both sides (`tools/xtask/src/main.rs:5551`, `:5775`). Other
   xtask-mapped desktop images leave both off. The Cargo features remain separately
   selectable (`userland/neo-init/Cargo.toml:144`, `userland/neo-arthur/Cargo.toml:88`),
   so a manually assembled image can still enable only one; the test guards the
   product mapping, not arbitrary feature combinations.

5. **PASS — test-only KAT and prompt swap.** The busy boot kernel feature is
   scenario-specific (`kernel/Cargo.toml:776`); xtask maps that scenario to
   `desktop-busy-boot-test` (`tools/xtask/src/main.rs:2360`, `:5551`). Arthur's
   model KAT path is selected only by `arthur-boot-model-kat`; the normal boot-module
   path explicitly excludes it (`userland/neo-arthur/src/main.rs:285`, `:295`).
   The product mapping selects `arthur-boot-module`, not the KAT feature
   (`tools/xtask/src/main.rs:5764`). The kernel prompt/max-token replacement and
   its serial witnesses are all `cfg(feature = "neo-test-desktop-busy-boot")`
   (`kernel/src/arch/x86_64/syscall.rs:3734`); scenario labels/pass line have the
   same cfg (`kernel/src/test_harness.rs:448`, `:494`). The product and image-A
   mappings do not select that test-only kernel feature.

   `MODEL_05B_PATH` has mutually exclusive cfg variants: normal NeoFS, production
   boot-module 0.5B, and the KAT inventory model (`userland/neo-arthur/src/main.rs:282`,
   `:285`, `:295`). The KAT constant is gated by `arthur-boot-model-kat`.

6. **PASS — pace check rejects a frozen/slow WM.** The expected frame interval is
   integer `1000 / 12` ms (`tools/xtask/src/main.rs:8075`). The checker requires
   exactly two ended emblems, at least 1000 ms and two presents each, and
   `presents * 2 >= ms / BUSY_FRAME_MS` (`:8093`). Its tests reject one present
   over 30 seconds, 179 presents over 30 seconds, a sub-second run, and a missing
   second emblem (`:17484`). The checker is applied to `desktop_busy_boot` serial
   (`:13144`).

7. **PASS — kill-switch.** With the busy features off, Arthur takes the legacy
   branch, commits “Thinking...”, and erases it after generation
   (`userland/neo-arthur/src/main.rs:1045`, `:1049`, `:1065`). The WM `busy` state,
   tick, and busy layer are each compiled only under `desktop-busy`
   (`userland/neo-init/src/desktop_session.rs:282`, `:902`, and the feature-gated
   busy module beginning at `:5140`).

8. **PASS — prior art.** The merged PR body contains the requested comparison
   table for existing emblem/spinner approaches, with approach/fit/use, license,
   last-commit date and SHA for each candidate. It states that no GPL source was
   copied and that only general design principles informed the implementation,
   satisfying `CLAUDE.md`'s unfamiliar-feature rule.

## Evidence and scope

- Reviewed the complete pinned commit diff and traced protocol decode, registry
  validation, WM receive/commit/render paths, Arthur generation, feature mappings,
  and the KAT/pace checks. RAG health was healthy; semantic search was used for
  discovery, with the pinned checkout as the source of truth.
- PR #155 is merged. GitHub gate run `40446328993` reports success, including all
  four QEMU integration shards. Its `headSha` is `eedfba68b34b0ba297ec0f67c3a6c5e6868910c5`,
  not the reviewed squash SHA above. The PR body reports the dedicated
  `desktop_busy_boot` generation measurements (two ~56-second generations, 679
  and 669 presents); those measurements are author-reported, not independently
  rerun for this review.
- No tests or build gates were rerun; this was a bounded post-merge source review.
