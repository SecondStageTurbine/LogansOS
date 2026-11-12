<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Seat F, OT-100 R3 — the persistence area and its mount (2026-11-12)

**Seat:** Seat F (cloud). **Starts when #104 (R2) is on `main`.**
**Branch:** `claude/ot100-r3-area-mount`, cut from `origin/main` after #104 lands.
**Design (approved):** `docs/THINK-2026-11-10-ot100-usb-persistence.md`:
- the §10 R3 row;
- §2.2 (layout and the image oracle), §2.3 (consent and "Try"), §4 (the RW rule and the read-only flip) and §5 (the
  census and never-internal);
- §5.1, the CP table, and §9, I-14.

J.c.'s rule stands: the internal drive is **never** written.

## Build exactly R3's row

Behind feature `usb-persistence`, which implies `usb-storage-write` and `neofs-partition`:
- `xtask iso --persist-mib` and the GPT edit (§2.2);
- the two Limine entries and `persist_logic::parse_cmdline` (§2.3);
- boot-device identification and `arming_report`;
- the RW rule and the read-only flip (§4.1, §4.2);
- the census (§5);
- the vm-image refusal;
- the Zenbook runbook update: the stick is now a data medium.

**Nothing from R4** (the boot log) goes in this PR.

## Added from R2's reviews (must be in R3)

1. **A guard-side armed check for the non-write paths** (Muse's #104 observations 1 and 2,
   `Plans/RETURN-2026-11-12-pr104-security-muse.md`).
   - **What's wrong today:** `GuardedDevice::flush`, `GuardedDevice::write_protected` and `GuardedDevice::submit(BlockOp::Flush)`
     forward to the inner device without the guard's own armed check. MSC is safe only because of its `sync_ok` latch.
   - **Fix:** give each one the same `writes_allowed()` gate the guard puts on `write_probe`. An unarmed device answers
     `Unsupported` (or `None`) without touching the inner device.
   - **Prove it:**
     - a host KAT over a mock inner device that counts calls: zero calls when unarmed, one call when armed;
     - with `usb-persistence` off, `GuardedDevice` must behave exactly as it did before R3.
2. **No network secrets on the volume (THINK §6).** Wi-Fi PSKs and other network secrets are never written to the
   persistence volume until encryption lands. The setup and network pages keep them in RAM for the boot. A scenario or
   KAT must show the volume has no such record after a setup walk.

## Gate

- **The full 4-shard tier**, with the scenarios from R3's row:
  - `usb_persist_two_boot`: boot 1 writes and bumps the counter, boot 2 reads it;
  - `usb_persist_never_internal`;
  - `usb_persist_unplug_midwrite`, under `--smp 4`;
  - the try entry: the stick's hash is unchanged and the mount is read-only;
  - an absent area;
  - a corrupt area;
  - a second LogansOS stick attached: nothing is mounted RW;
  - two sticks from one image: neither is identified.
- **Oracles:**
  - §2.2's image oracle: its own GPT parser checks both GPT copies and the hand-laid superblock fields;
  - the guard KAT over the built image and over its "repaired" variant.
- **Kill-switch:**
  - without `--persist-mib`, the image is **byte-identical** to the pre-R3 builder's on the same commit and inputs. Pin
    xorriso's dates first if the build isn't already reproducible;
  - with `usb-persistence` off, the kernel mounts as it does today.
- **Close-gate:** `scripts/worker-close-gate.sh` and the personal-details scan.
- **The PR body must carry:**
  - the trap bank (§8.1);
  - the I-14 statement;
  - the CP table, adding the new read-only-flip seam to `docs/concurrency-pitfalls.md` as §5.1 says;
  - the prior-art note (§7 still applies; say so).

## Review

This is a kernel driver and it touches the write guard. When the PR is green, the PM queues:
- a **cross-family review** (Codex, GPT-6.1-Sol, high);
- a **Muse security read** of the guard change, the RW rule and never-internal.

## Safety (the standing rules)

- No path may write any device the guard hasn't armed.
- The Zenbook's internal NVMe is never written.
- Guard-control features stay unbuildable with `bare-metal`, `product` or `fb-progress-console`.

## Close

Commit and push at every checkpoint. Post PM: on the PR with the 4-shard run URL. Then R4 is dispatched.

**Disk reclaim:** once the PM confirms the merge, `cargo clean` the worktree and remove it.
