<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #92): post-merge review of OT-87 R1, `xtask vm-image` (#152) → Jarvis

**MODEL:** GPT-6-Luna. **EFFORT:** high. Build tooling, a pinned firmware and a three-boot QEMU scenario. No kernel or
userland change. Claude-authored (S2), so it gets a Codex-family review. **Do this after relay #89.**

**Type:** review only, post-merge. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `22fd551f`, #152's squash on main
(PR head `42b911aa`). Gate: batch 3, run `40440467911`, all 22 jobs green; `vm_installed_two_boot` passed in shard 3.

**What it does.**
- **`cargo xtask vm-image [--with-model] [--fresh-data]`** writes `target/vm/boot.raw` (image A's ISO bytes) and
  `data.raw` (a 128 MiB NeoFS volume, kept across runs).
- **`xtask vm-run`** boots them under OVMF with no `-cdrom`.
- **OVMF:** QEMU v10.2.3's bundled edk2-stable202408.01, with a sha256 check in the Dockerfile and BLAKE3 pins in
  xtask.
- **`vm_installed_two_boot`** (shard 3) runs three boots: write the memory file, a control on a fresh data disk that
  must fail read-back, then a read-back.

## Setup

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot87-r1-vm-image-review-jarvis
          git fetch origin && git checkout -B codex/ot87-r1-vm-image-review-jarvis 22fd551f

git show origin/main:Plans/DISPATCH-2026-10-27-ot87-r1-vm-image-review-jarvis.md   # this file
git show origin/main:Plans/DISPATCH-2026-10-27-ot87-r1-vm-image-s2.md              # the dispatch
git diff 22fd551f^ 22fd551f
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it.

## Checklist

Answer each item PASS or FAIL with `file:line`.

1. **boot.raw is image A, unchanged.** It's byte-equal to `xtask iso`'s output. The layout KAT checks the protective
   MBR, the GPT and the ESP (its type GUID in UEFI byte order), and that there's no NeoFS superblock at any copy
   offset on the boot disk.
2. **data.raw.** A valid 128 MiB NeoFS volume with all three superblock copies and the stubs the mount requires. It
   uses the kernel's own constants, not a second copy. `--fresh-data` replaces it, and the default keeps it.
3. **The firmware is hermetic.** The sha256 check in the Dockerfile fails the image build on a mismatch. The xtask
   pins match. The scenario refuses any other firmware as INFRA, not as a test failure. Nothing is downloaded.
4. **The scenario proves persistence:**
   - boot 1 writes;
   - boot 2 on a fresh data disk fails read-back (the control);
   - boot 3 reads back;
   - every boot mounts from device 1;
   - the boot disk ends byte-identical.
   Would it still pass if persistence were broken, or if the data were read from the boot disk?
5. **Try it yourself (optional).** On your host, run `docker compose run --rm neo cargo xtask vm-image`, then
   `vm-run --headless`, and say whether it boots. This is the README's "Try LogansOS in a VM" path.
6. **Prior art.** The PR body's survey table follows CLAUDE.md's new rule: licenses and last commit dates, and
   nothing vendored.

## Close condition

- Commit `Plans/RETURN-2026-10-27-ot87-r1-vm-image-review-jarvis.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
