<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Build a bootable ISO of current `main` for the PL (build + hand over — no QEMU) → Janus

**Date:** 2026-06-22 (updated — PL: no QEMU, just produce the ISO)
**From:** PM (Claude)
**To:** Janus (free; on the rig with the dev container + Limine). **Ops/build task — no code change, no QEMU, no merge, no gate.** The PL will burn + boot on real hardware themselves.
**Status:** DISPATCHED.

## The ask (minimal)
1. `git fetch origin && git checkout origin/main` (build clean off the current tip).
2. **`cargo xtask iso`** (inside the dev container) → produces `boot/neoplutarch.iso` (BIOS+UEFI, via xorriso). Confirm it builds successfully.
3. **Hand back the absolute path** of `boot/neoplutarch.iso` on the rig so the PL can burn it to USB. Done.

**Do NOT** run QEMU (`cargo xtask run`/`neo-test`) — the PL will boot it on metal directly. No code change, no model bundling (the real brain is Stage-B, not this), no flipping feature defaults, no commit, no gate. If the build fails or a tool's missing (xorriso/Limine), say so.

**One honest line for the PL** (no QEMU needed to know this, it's from the build config): this is the **fixture/stub Arthur** image — no real 485 MB brain bundled, real-infer/real-vocab off — so on boot the PL gets the OS + branded boot frame, **not** a real Arthur conversation (that's the Stage-B ISO, still ahead).

---
## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-22-build-bootable-iso-janus.md`. **No branch — ops/build.**
- **One-line:** Just build the ISO, no QEMU. On current `origin/main`, in the dev container: `cargo xtask iso` → confirm it produces `boot/neoplutarch.iso`, then **hand back its absolute path** for the PL to burn to USB. **Don't run QEMU, don't bundle a model, don't change code or commit.** (FYI for the PL: it's the fixture/stub-Arthur image — OS + boot frame on boot, not a real conversation; the real-brain ISO is Stage-B.)
