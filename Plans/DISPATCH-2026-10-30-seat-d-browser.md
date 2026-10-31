<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH — Seat D: the web browser (OT-61), continuously, until it ships

**J.c. (2026-10-30):** *"we should have one coder just working nonstop on the webbrowser until it is done and ready."*
Seat D is that coder. It has one lane and no other queue. OT-61 is release-gating: "an OS that can't browse the
internet isn't shippable".

## Read first (`git fetch origin && git show origin/main:<path>`)
- `docs/ADR-078-ot61-browser-c-ladder.md`: the plan of record and the rung table (§3), with the reserved invariant
  rungs A1 (W^X ⊗ JIT) and A2 (per-content memory quota).
- `docs/RESEARCH-2026-10-29-ot61-c2.3b-html5ever-stylo-closure-SPIKE.md` (#200): the C2.3b closure, 28 linked plus 15
  build-only crates. J.c.'s decisions: **vendor the sources**, and **the first build runs on the CI runners**. Two
  items carried in: the `parking_lot_core` SYS_FUTEX parker, and an AVX/XSAVE check for memchr.
- `docs/RESEARCH-2026-07-22-ot61-servo-host-closure-spike-FINDINGS.md`, `…-swgl-browser-floor-spike-FINDINGS.md`,
  and `docs/RESEARCH-2026-07-20-ot61-browser-architecture-FINDINGS.md`.
- The ledger tail, for C2.1–C2.3a (#190, #199, #205, #221, #223, #229).

## Where it stands
C0, C0.5, C1–C1.3 and C2.1–C2.3a are built and on main: real `std` programs, threads/TLS/futex, sealed `std::fs`,
mem and obj quotas, and `servo_host_smoke` without third-party crates.

## The ladder from here (each rung its own PR, its own falsifiable gate)
1. **C2.3b:** the vendored html5ever + selectors closure under `third_party/` (licenses and upstream SHAs recorded);
   the CI build job; the `parking_lot_core` parker on SYS_FUTEX; the memchr AVX/XSAVE check. **Gate:** a smoke test
   that parses real HTML and runs selector matching on-target in QEMU.
2. **Stylo / layout closure:** the same method, extended to style and layout.
3. **C3, SpiderMonkey:** the static C++ runtime, interpreter only (no JIT; A1 stays reserved). **Gate:** ADR-078
   §3's C3 row.
4. **The SWGL software-render floor:** into a confined `SharedPixelBuffer`, with no scanout authority (§2.2). Then
   the modern-site correctness battery begins.
5. **Networking:** the browser gets **no dial capability of its own** (C2). Fetch goes through a broker that holds
   the net cap. That composes with lane L1's `net-dial-cap` in image A. HTTPS uses the existing TLS path
   (`phase4_https_fetch`).
6. **The browser app:** a window under the OT-88 app model. **Dependency:** A1a/A1b (#220, lane L4). When you get
   there, coordinate with the PM; you may help land A1a if it is the blocker.
7. **Into image A and onto the Zenbook:** a real site renders and scrolls on silicon. That's the acceptance
   milestone.

**"Done and ready":** rung 7's acceptance on the Zenbook, with A2 (the per-content-process memory quota) enforced, and
the browser a removable, permissioned app.

## Standing rules
- **Don't wait on merges.** When a rung's PR is up, start the next rung stacked on it, and re-stack once the base
  merges (CLAUDE.md re-stack rule).
- **Every third-party crate:** license, upstream URL and SHA recorded; MPL/MIT/Apache/BSD only; GPL never.
- **Each rung:** a kill-switch (the ladder is opt-in; the default build stays bit-identical), the close-gate, and a
  RETURN (seat D) PR comment. The PM gates, runs the AAG and merges.
- Commit and push at every checkpoint. No model identifiers. Keep disk use in check (`cargo clean` between big
  vendored builds).
