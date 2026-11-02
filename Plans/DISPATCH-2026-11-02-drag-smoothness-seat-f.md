<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Dispatch: smooth window dragging (Seat F, after #15)

J.c. (2026-11-02, preview in compatibility mode): "the moving of the boxes is laggy, not smooth at all." Feedback
item F7 in `Plans/FEEDBACK-2026-11-02-preview.md`.

1. **Measure first.** Add frame-time instrumentation to the WM's drag path (input event to frame presented, and the
   composite time per frame), printed by a desktop scenario that drags a window across the screen. Record numbers
   under TCG (the gate) and, if you can get it, KVM/accelerated; say which part dominates.
2. **Fix what dominates.** Likely candidates: repainting the whole screen per drag frame instead of the union of the
   old and new window rectangles (damage tracking); recomposing unchanged windows instead of blitting cached
   surfaces; coalescing pointer events so a slow frame doesn't queue a backlog (draw the latest position only).
3. **Gate:** the drag scenario's frame-time budget (pick a TCG threshold from the measurement, with headroom) plus the
   existing desktop capture oracles unchanged.
Usual rules: close-gate, gate, PR, `PM:` comment; a Codex review if it touches the kernel.
