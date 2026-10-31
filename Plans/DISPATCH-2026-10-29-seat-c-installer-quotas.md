<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH — Seat C: installer Back (#148), then the quota fixes

**Date:** 2026-10-29 · **From:** PM · **Seat:** C (a new cloud coding session; it replaces the old S3)

Read: `Plans/HANDOFF-2026-10-29-pm.md` §4 (the S3 rows) and each RETURN named below
(`git fetch origin && git show origin/main:<path>`).

## Environment

This container has QEMU and Limine. Run scenarios as CI does:
`LIMINE_DIR=/opt/limine NEO_SMP=4 cargo neo-test <scenario>`. UEFI scenarios (install_*, login_*, vm_*) need the
pinned OVMF:
```
mkdir -p /usr/local/share/qemu && cd /tmp && for f in edk2-x86_64-code.fd edk2-i386-vars.fd; do
  curl -fsSLo $f.bz2 https://gitlab.com/qemu-project/qemu/-/raw/v10.2.3/pc-bios/$f.bz2 && bunzip2 -f $f.bz2; done
sha256sum edk2-*.fd   # 33090cc0…6dbb2a (code) and 5d2ac383…be5d1e (vars): the Dockerfile pins
cp edk2-x86_64-code.fd edk2-i386-vars.fd /usr/local/share/qemu/
```
The disk allowance is small: delete `target/neo-test/*.raw|*.img|*.iso` between scenario runs.

## Work, in order (one PR each, off `origin/main`)

1. **#148 F1 (P1): clicking the visible Back label can start an install.**
   (`Plans/RETURN-2026-10-29-ot87-r4.4-ui-review-chad.md`, items 2–4.) `installer_view.rs` draws both buttons in one
   left-aligned string, but `hit` splits the whole card into equal halves, so the drawn "[ Back ]" lands in the Go
   half. Make each button's hit rectangle the rendered label's own rectangle (measured with the shipped font), with
   a dead zone between them. Add the discriminator #148 item 4 asks for: a pointer click at the centre of the drawn
   Back label must not start an install (boot disk unchanged), and it fails with the old `hit`.
   **F2 in the same PR:** `install_abi.rs` must reject unknown class/action/refusal values and unknown flag bits.
   Add a host KAT using Chad's 128-byte record (byte0=1, byte2=0xff).
2. **#143: the mem-quota leaf frame leaks on a failed map** (`Plans/RETURN-2026-10-29-ot61-c2.2-review-jarvis.md`).
   Add a guard plus a physical-frame discriminator (frame counts equal before and after a refused map).
3. **#165: obj-quota coverage is partial, and the emptied handle table's capacity is uncharged**
   (`Plans/RETURN-2026-10-29-ot61-c2.2b-review-chad.md`). If #221 (your predecessor's pin-charge PR) is still open,
   build on `origin/main` and keep the two PRs apart.
4. **#202 F1 / #170: TCP listener retirement vs in-flight admission**
   (`Plans/RETURN-2026-10-29-tcp-listen-bounds-review-chad.md`). Coordinate with #218's listener retirement (F3 of
   Chad #173): one fix, owned here, with #218 rebasing onto it.

## Close condition (each PR)

- `scripts/worker-close-gate.sh` passes; its evidence block is in the PR body. Kernel-touching PRs say so, and the PM
  gates them in a 4-shard batch.
- A PR comment starting `RETURN (seat C):` with the head SHA and the scenario results.
- **Don't merge;** the PM gates and merges. Commit and push at every checkpoint. No model identifiers in commits.
