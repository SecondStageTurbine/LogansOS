<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: DI-1b move/resize cross-check — Jarvis

**Reviewed:** `origin/claude/di1b-pointer-move` at `194fe958`

**Verdict: MERGE-WITH-NITS.** The move, resize, presentation, and allocator paths survived the requested cross-check. I found one deterministic but very remote configure-serial exhaustion bug. It cannot occur before 2,147,483,648 accepted resizes in one session, so I do not consider it a practical merge blocker, but the protocol should either constrain its serial space or represent the full serial.

RAG health was `healthy` (`ollama: true`, `qdrant: true`). I used it for semantic discovery, then read the named implementation and tests directly.

## Finding

### LOW — configure serial `2^31` publishes the mailbox's reserved zero sequence

Files: `userland/libneo/src/atrium_configure.rs:39-61`, `userland/neo-init/src/desktop_session.rs:1988-2000`

`publish` encodes a `u32` serial as `serial.wrapping_mul(2)`, while `read` treats sequence zero as "nothing published" and decodes every other even sequence with `before / 2`. Consequently:

```rust
let words = [const { AtomicU32::new(0) }; MAILBOX_WORDS];
publish(&words, Published { serial: 1 << 31, width: 760, height: 460 });
assert_eq!(read(&words), None); // final seq wrapped to reserved zero
```

For subsequent serials the decoded value has lost bit 31, so it cannot equal the WM's pending serial. `Configure::publish` makes recovery impossible by advancing `next` with `saturating_add`: it eventually remains at `u32::MAX`. The WM therefore retains `pending`; that disables later move/resize starts and client pointer forwarding for the rest of the session.

Suggested fix: define a 31-bit non-zero serial domain and wrap safely only after the previous configure is acknowledged, or widen the mailbox sequence and its atomics. Add boundary KATs at `0x7fff_ffff`, `0x8000_0000`, and the chosen wrap point.

## Requested checks

1. **Position secrecy:** the client receives only its local coordinates, clamped to the size pinned at press; the configure mailbox and ack carry only serial/width/height; client damage remains surface-local. I found no position channel.
2. **Presentation after move:** the release path stops draining before later records are hit-tested; `move_window` repaints the union of old/new focus-light rectangles from the base, then composition uses current geometry. Outline and sprite are overlays restored by the same present path. Client commits, Home switching, and Destroy all consult current `scene.geo`; I found no stale copy or residue path.
3. **Blur count:** a real Glass move/resize reaches `BaseScene::place_frame` once and blurs the completed frame region once. Motion only changes the outline overlay. No-op placement returns without a blur; Reduce Transparency has no backdrop blur. The translated glow plane need not be regenerated for a size-preserving move; resize does regenerate it.
4. **Allocator:** `Heap::top()` includes `pending_extend`, so exact adjacency is judged at the allocator's true top. Non-contiguous regions open distinct arenas; deallocation selects the unique containing `[bottom, top)` range. The implementation has 64 arena slots (despite the dispatch's one reference to 16); exhaustion maps one final region and then fails cleanly without aliasing. Back-to-back capped maps extend one arena, so Argon2id's 64 MiB request remains supportable with the added metadata slack.
5. **Clamping:** the bounded-layout tests cover both 1920x1080 and 1280x800. The clamp accounts for the entire focus-light extent, not merely the client rectangle, and edge resizing anchors the opposite edge while respecting min/max client sizes.
6. **Resize protocol:** the read-only mailbox and exact pending serial/size check prevent client-selected geometry and position disclosure. A malformed/stale ack ends the session rather than mutating geometry. Release/acquire ordering prevents torn size reads, including a reader straddling a normal publication; the serial-space flaw above is the exception. The client initiates the synchronous ack while the WM is receiving requests, avoiding the unsolicited-send deadlock. Outstanding configure state survives Esc/hold/resume coherently and is dropped with Launch/Destroy session teardown. Composition uses acknowledged dimensions for registry damage, client stride/clip, snapshot, and frame placement; the glow plane keeps its output stride.
7. **Cross-family structural pass:** I traced post-move geometry users and heap growth/deallocation rather than relying only on oracle tests. No additional issue survived re-verification.

## Verification

- `cargo test -p neo-atrium` — 38 passed, 2 ignored.
- `cargo test -p atrium-logic-tests` — 106 passed.
- `cargo test -p arthur-logic-tests` — 70 passed.
- `cargo test -p libneo --features atrium-configure` — passed (doc-tests; no unit targets selected).
- `cargo check -p neo-init --features desktop-resize` — passed.
- `cargo check -p neo-arthur --features desktop-resize` — passed.

I did not rerun the optional QEMU pointer/resize scenarios; this return relies on the host discriminators plus direct control-flow and memory-layout review.
