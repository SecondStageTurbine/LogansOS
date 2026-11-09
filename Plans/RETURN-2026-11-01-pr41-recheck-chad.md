<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 259: re-check of PR #41 design revision 2

Reviewed THINK head: `43c866210a93664432563f9da4fcb84af16f13fc` (`dexter/l2d-images-think`).

- **PASS — Per-image/per-session enforcement is now stated accurately.** The 16 MiB decoded and 32 MiB encoded caps apply on every build; the 64 MiB viewer account only applies with `mem-quota`, and the design explicitly claims no whole-session or product-desktop hard ceiling while that feature is off (`docs/THINK-2026-11-01-l2d-images-in-the-desktop.md:479-493,502-517,542-544`).
- **PASS — The quota discriminator now isolates account refusal.** The 2000×2000 image decodes to 16,000,000 bytes, below the 16 MiB cap; Q2 verifies the size check first and then the fallible reservation fails with `ENOMEM`, with `TooLarge` and `reserve-ok` forbidden. Q1 validates the control and Q3 separately describes the contained infallible-allocation abort (`docs/THINK-2026-11-01-l2d-images-in-the-desktop.md:675-708`).
- **PASS — Spawn citations distinguish the file-cap recipe from the Lens layout example.** The THINK cites `permission.rs` for deriving the `USE`-only cap and `libneo` for `sys_spawn_budgeted`; it labels the Lens only as the fd-slot layout example and explicitly says that code opens a directory and uses plain `sys_spawn` (`docs/THINK-2026-11-01-l2d-images-in-the-desktop.md:312-329,617-622`).
- **No new issue found** in this re-check.
