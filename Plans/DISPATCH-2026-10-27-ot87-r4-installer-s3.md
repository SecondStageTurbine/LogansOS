<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-87 R4, the installer for blank or incomplete targets (VM only), in four PRs → S3

**Why.** R4 is the rung that makes "Install LogansOS" real: boot the USB, pick a blank disk, and it installs and then
boots alone with the model on its own disk. Its dependencies are merged: R2 (#154 `2f066957`) and R3 (#160
`7c771c0b`). The authoritative design is `docs/THINK-2026-10-27-ot87-installer.md`:
- §2 and §2a: targets, identity and the install claim;
- §4.3–§4.8: the bundle, the GPT, choosing a disk, the minimum size, the state machine, provisioning;
- §8, the R4 row;
- §11: prior art.

Build exactly what it says. Where the code shows the THINK is wrong, stop and tell the PM before deviating.

**Scope is `fresh` targets only:** blank or INCOMPLETE disks, VM only. Destructive installs (erase or reinstall) are
R4b. The Try/Install boot entries are R5. Nothing in R4 runs on bare metal. **The Zenbook's internal disk is never a
target.**

## Split into four PRs, each gated and reviewed on its own

R4 is too large for one review. Land it as a stack, each PR rebased onto `origin/main` only after the one before it
has merged (CLAUDE.md: re-stack only onto a final base). Behind one kill-switch, `installer` (off ⇒ the build is
today's; give `.text` hashes with `NEO_BUILD_ID` pinned).

| PR | Contents (THINK §) | Proofs |
|---|---|---|
| **R4.1 — identity and the claim** (kernel) | §2a items 1–6: the registry generation and device tokens; driver-reported identity (NVMe Identify, ATA IDENTIFY, virtio `GET_ID`); `media_generation` and each driver's media-change reporting; the one-shot claim holding `Arc<GuardedDevice>`; `ClaimedWriter` with pre/post-completion checks; "identity unavailable" fails closed for destructive kinds | The §2a test table as host KATs (a model registry and a model device that bumps mid-request), including the relay #88 masked-swap KAT that **asserts the limit**. Pitfall-bank table (the new atomics, and any lock taken on a submission path). Driver trap-mining for the three identity commands and media-change events (NVMe/AHCI/virtio specs + Linux/BSD for principle). Muse security review |
| **R4.2 — the bundle** (xtask, host) | §4.3: `NEOINST1` and its manifest; the two ESP templates built on the host with vendored `fatfs` under `third_party/fatfs/` (MIT; pin a commit, keep its license, record upstream URL + SHA; replaces the `mformat` call); the slot maps from read-back with the contiguity check; the BLAKE3 pin compiled into the kernel and xtask's refusal on a mismatch | Hand-laid byte oracles for `NEOINST1` and for one slot-map entry (I-12). A non-contiguous chain fails the build (KAT). Codex review |
| **R4.3 — the writer and the state machine** (kernel + scenarios) | §4.4 GPT writer (reuse R3's `gpt_layout` and fixtures); §4.6 minimum size; §4.7 W0–W6 with the durability rules; NeoFS seed + grow; streaming the untaken model/vocab with the NeoFS pin re-check; the Limine executable-file request; zero reads from the boot medium | The write-back-cache disk model KAT with every cut and the named traces (relay #88 header-only, W6 backup flush failure, the restarted Finish). QEMU: install to a blank virtio disk at `S=512` and `S=4096`, then that disk boots alone and Arthur's model loads from it; `install-stop-after=Wn` for each step then Finish installing; 1 GiB and one-sector-short disks refused with zero writes; a no-flush device refused; a foreign GPT and a completed LogansOS disk refused (R4b's job). Pitfall-bank table. Muse security review |
| **R4.4 — the UI** (userland) | §4.5: `neo-installer` (Atrium) and the `neo-shell install` console fallback, keyboard and pointer; `fresh` actions only; refused rows shown but not selectable | A scenario driving the console fallback end to end; the Atrium path at least to the confirmation screen. Codex review |

R4.2 can be built in parallel with R4.1, since they touch disjoint code. Open each PR when it's ready; I'll gate them
in order.

## Rules that apply

- **The prior-art survey** is THINK §11. Each PR body restates the rows it relies on, with licenses and last commit
  dates. `fatfs` is the one borrow and is host-only, so state that it never reaches the kernel.
- **The write guard.** The claim is the only path past it, one-shot and bound to the device object. Every existing
  guard KAT stays green, `windows_gpt_disk_never_arms` included.
- **The close-gate:** `scripts/worker-close-gate.sh` on every PR. Push at every checkpoint. Read SHAs from git,
  never from memory.

## Setup

```
WORKTREE: cloud session S3 (your own clone)
BRANCH:   claude/ot87-r4.1-claim   (then claude/ot87-r4.2-bundle, claude/ot87-r4.3-writer, claude/ot87-r4.4-ui)
          git fetch origin && git checkout -B claude/ot87-r4.1-claim origin/main

git show origin/main:Plans/DISPATCH-2026-10-27-ot87-r4-installer-s3.md   # this file
git show origin/main:docs/THINK-2026-10-27-ot87-installer.md
```

RAG isn't available in the cloud session. Cite code by `file:line`. Send the PM each PR number with its head read from
`git ls-remote`. Before ending, `git status --porcelain` and `git log @{u}..` are both empty.
