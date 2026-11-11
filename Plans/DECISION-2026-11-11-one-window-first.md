<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DECISION: Files and the editor first open one window at a time (J.c., 2026-11-11)

**Asked:** should the first version of Files and the text editor open one window at a time (full-screen, like
Arthur's today, which is A1a's model), or wait for side-by-side movable windows (A1b, A1c)?
**J.c.:** "One at a time first."

**What changes** (`Plans/AUDIT-2026-11-11-mvp-app-chain.md` §2):
- The critical path to "Files opens a file in the editor and saves it" is now **A2 → F1 → E2**, with **F0** in parallel
  (dispatched to Seat E). That's three rungs in series instead of five.
- **A2** (launching from the Spine and Home) is next on the WM side. It goes after **#98** lands, because both edit the
  launch path in `desktop_session.rs`.
- **A1b and A1c** (side-by-side windows, snapping) stay in the plan as the upgrade right after E2, and are no longer
  on the critical path.
