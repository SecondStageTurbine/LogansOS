<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Codex re-check of DL1-X1 (Janus)

**Commit reviewed:** `d9296bfb2a752fafd9aa139d920b67a86bc4fef4` (#65)

**Base:** `02cff42adf0c2bb63961860ebb6a5bb8fae4cc76`

**Verdict:** **CLOSED — no findings.** The fix closes the High finding in
`RETURN-2026-10-19-desktop-dl1-codex-crosscheck-janus.md`: the stale 1120x400
configure is replaced by a new 728x400 configure before Arthur can read the
mailbox, the replacement is placeable with the Lens docked, and the former
underflow/error-92 path is no longer reachable.

## Numbered review verdicts

### 1. Closure — PASS

- `Layout::docked_max_client` accounts for the frame padding, Lens gap, Lens
  frame, right margin and two `GLOW` margins. Its height removes two `GLOW`
  margins and the frame's title/padding, while `docked_group` uses the maximum
  of the Arthur and Lens frame heights. Because the Lens can bloom only when
  the current group fits, a refit cannot fall below the live client minimum.
- The width bound is the same bound used by docked `Layout::resized`: the
  frame's right edge stops before `GLOW + MIN_MARGIN + LENS_GAP +
  lens_frame_w()`. The height bound agrees with the resize paths: the frame is
  inside the workfield with `GLOW`, and a frame shorter than the fixed Lens is
  additionally top-bounded by `lens_frame_h()`.
- At 1280x800, the original x=98 / 1120x400 repro refits to 728x400. The
  permanent KAT proves the impossible group no longer underflows and checks
  maximum-width/height combinations on 1280x800, 1366x768, 1440x900,
  1920x1080 and 2560x1440.
- `apply_configure` clamps a docked ack from its stored WM-only origin. The
  special unchanged case is exact equality of origin and size, so skipping
  `place_frame` cannot admit a new geometry; registry bounds and repaint still
  run.

### 2. Refit timing — PASS

- Arthur's `atrium_client::launch` sends `OP_LAUNCH`, sends the proof and then
  blocks in `recv_reply`. It has no configure poll between the stale publish
  and the WM's successful `lens::bloom`/refit/reply sequence. Thus it cannot
  read or ack the old serial; the next poll sees only the seqlock's complete
  replacement with its new serial.
- A failed bloom does not refit, but it also does not set `lens_docked`; the
  pending undocked configure remains valid. In the fault-test retry loop,
  Arthur immediately makes another synchronous launch call and still does not
  poll the mailbox between attempts. The eventual successful bloom performs
  the refit.
- Pointer service runs while the broker polls for proof. Once `lens::bloom`
  begins, the WM does not run that idle callback. A second resize released
  after bloom is therefore handled only after the launch reply, when docked
  `resized` bounds are already active. While a configure is pending,
  `request_resize` also refuses to publish another one.

### 3. Saturation — PASS

For every input on which the former expressions were defined,
`(right - GLOW).saturating_sub(frame_w)` and
`(bottom - GLOW).saturating_sub(frame_h)` return exactly the former subtraction.
Only the previously underflowing cases change, and `max(lo)` puts those at the
low edge as intended.

### 4. Kill-switch — PASS

- `desktop-lens-pointer` does not compile the refit or configure path.
- `desktop-resize` does not compile the Lens refit path; `lens_docked` remains
  false and frame-only bounds are unchanged.
- `desktop-full` compiles the composition, and the refit runs only after a
  successful Lens bloom with an outstanding oversize configure.
- Clippy with `-D warnings` passed for `neo-init` under
  `desktop-lens-pointer`, `desktop-resize`, and `desktop-full`. It also passed
  for `neo-arthur` with `desktop-lens,desktop-resize`.

### 5. Coverage — PASS for closure; QEMU gap retained as a note

The permanent host KAT contains the exact reported geometry and broadens the
check across five outputs and both axes. A QEMU discriminator would require a
dedicated 1280x800 scenario (or extending `desktop-full`) whose HID release is
scheduled after the `awaiting-proof` witness but before proof delivery, then
asserting the `resize refit` serial/log and successful ack. That is useful but
not a cheap deterministic addition to this narrowly scoped fix; the dispatch
and THINK accurately retain it as unadded coverage, not as evidence that the
closure is open.

## Verification

- RAG health: healthy (`ollama=true`, `qdrant=true`); semantic discovery was
  used before exact-symbol inspection.
- `cargo test -p neo-atrium`: PASS — 49 passed, 2 ignored.
- `cargo clippy -p neo-init --features desktop-lens-pointer -- -D warnings`:
  PASS.
- `cargo clippy -p neo-init --features desktop-resize -- -D warnings`: PASS.
- `cargo clippy -p neo-init --features desktop-full -- -D warnings`: PASS.
- `cargo clippy -p neo-arthur --features desktop-lens,desktop-resize -- -D warnings`:
  PASS.
- Optional QEMU scenario: not run; no new proof-window scenario exists in this
  fix.
