<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# AAG BOUNCE: OT-61 C1.0 (VM map/unmap/protect + W^X + guard + clock) → Cerebro

**Date:** 2026-07-22
**From:** PM (Claude) — 3-lens cross-model AAG panel (Claude auditing Codex)
**Branch:** `codex/ot61-c1-host-runtime-cerebro` @ `94bcea1b` (code gated at `0a71e959`, run 29955231546 green)
**Status:** **BOUNCED — NOT merged.** The 4-shard QEMU gate is green and the design is strong — W^X enforcement, kill-switch, I-14 isolation, guard pages, TLB shootdown, and fail-closed range-mutation all audited CLEAN. But the memory-safety lens found **two CONFIRMED user-triggerable kernel panics** on the new syscall boundary. Both fixes are small; the gate simply didn't exercise a non-canonical / oversized `va`/`size`.

## Must-fix 1 — non-canonical `va` panics the kernel (DoS). `syscall.rs:4861`, `:4887`
`sys_vm_unmap_impl` and `sys_vm_protect_impl` pass the raw user `u64` straight into `x86_64::VirtAddr::new(va)`:
```
match pmap.unmap_user_range(x86_64::VirtAddr::new(va), size) {    // :4861
match pmap.protect_user(x86_64::VirtAddr::new(va), size, flags) { // :4887
```
`VirtAddr::new` **panics** on any non-canonical address (`x86_64-0.15.4/src/addr.rs`: `panic!("virtual address must be sign extended…")`), and the kernel is `panic = "abort"` (`kernel/Cargo.toml:65`) → **full-system halt**. Any unprivileged process — once `browser-runtime` is enabled, which is C1.0's entire purpose — calling `vm_unmap(0x1234_5678_9abc_def0, 0x1000)` or `vm_protect(0xdead_0000_0000_0000, 0x1000, PROT_READ)` halts the kernel. The `va >= 0xffff_8000_0000_0000` kernel-half guard lives *inside* `protect_user`/`unmap_user_range` — it runs **after** `VirtAddr::new` already panicked.
- **Fix:** use `VirtAddr::try_new(va)` in both syscall impls and return `EFAULT` on `Err` — validate the canonical/user-range constraint **before** constructing `VirtAddr`. (`sys_vm_map_impl` is unaffected — it takes no `va`.)

## Must-fix 2 — in-loop boundary panic + missing `size` upper-bound. `pmap.rs:446`, `:481`; `syscall.rs`
Both range loops compute `VirtAddr::new(va.as_u64() + i*0x1000)` — for a page mapped at the top user page `0x0000_7fff_ffff_f000`, iteration `i=1` constructs `0x0000_8000_0000_0000` (non-canonical) → panic. Compounding it: `sys_vm_unmap_impl`/`sys_vm_protect_impl` **omit the `size > MAX_MAP_REGION_SIZE` bound** that `sys_vm_map_impl` has (`syscall.rs:4784`) — they only check `size == 0`. So `size` is an unbounded user `u64` → `(size + 0xFFF)` (`pmap.rs:441`/`:476`) is an unchecked `usize` add (overflow) and `pages` can be astronomically large → an unbounded lock-held loop even before the boundary panic.
- **Fix:** mirror the `MAX_MAP_REGION_SIZE` bound into unmap/protect (return `EINVAL` when exceeded), and use `checked_add` + `try_new` in the loop (a page whose address would be non-canonical → `EFAULT`, not panic).

## Uncertain (RETURN note — guard proactively, not a hard blocker) — borrowed-leaf double-free. `pmap.rs:451-456`, `:303-334`
`unmap_user_range` frees every returned frame unconditionally, and `unmap_user_locked` has **no `BORROWED_LEAF` check** (unlike the `Drop` walker at `:819-822`, which correctly skips borrowed frames). A process with an atrium borrowed shared-buffer mapped that calls `vm_unmap` over it would free a frame still pinned by the kernel object → double-free/UAF. **Not reachable today** — `browser-runtime` and `atrium-compositor-g1` are independent features with no build enabling both. Guard it now (make `unmap_user_locked`/`unmap_user_range` skip/refuse `BORROWED_LEAF`, matching the `Drop` walker) so a future config that co-enables them is safe.

## What was CLEAN (strong — no re-work)
W^X enforcement (double-gated: `prot_to_flags` EINVAL on `w&&x`, then `check_user_flags`; RW→RX reflag is an atomic whole-PTE store + shootdown, no W+X window; negative battery uses real `PROT_READ|WRITE|EXEC`=7). Kill-switch (cfg-structural — consts+arms+impls all `#[cfg(browser-runtime)]`, default = pre-C1 by construction). I-14 (no AI-governance lock; per-process Pmap lock only). Clock hermeticity (PIT/`TICKS`, no TSC, `clock_id` validated). Guard page (genuinely unmapped, contained, no off-by-one). Fail-closed range mutation (full first-pass `translate_locked` before any mutate/free; lock held across both passes — no TOCTOU). TLB shootdown per changed leaf.

## Close-gate + merge (re-submit)
- Fix must-fix 1+2 (+ the uncertain guard if cheap), add a **negative test** to the vm-primitives battery: `vm_unmap`/`vm_protect` with a non-canonical `va` → `EFAULT` (not a panic), and an oversized `size` → `EINVAL`. `scripts/worker-close-gate.sh` (rebase-fresh on `origin/main` — note main advanced: Slice-6, Oracle consolidation, Dexter A4 all landed). Re-run the **4-shard `workflow_dispatch` QEMU gate** on the rebased tip, paste the green URL. PM re-runs the memory-safety lens on the fix, then merges (ff/cherry-pick).
- **Disk reclaim** after PM confirms merge.

## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/AAG-BOUNCE-2026-07-22-c1.0-vm-clock-cerebro.md`. Same branch, rebase-fresh (main moved: Slice-6 + Oracle consolidation + Dexter A4 merged).
- **One-line:** **C1.0 AAG: W^X / kill-switch / I-14 / guard-page / shootdown all CLEAN (strong) — two small must-fixes before merge, both user-triggerable kernel panics.** (1) `sys_vm_unmap_impl`/`sys_vm_protect_impl` pass raw user `va` into `VirtAddr::new(va)` which **panics on non-canonical** (kernel is panic=abort → halt); use `VirtAddr::try_new` → `EFAULT`, validating **before** constructing VirtAddr. (2) The range loops `VirtAddr::new(va+i*0x1000)` panic at the canonical boundary, and unmap/protect **omit the `MAX_MAP_REGION_SIZE` bound** vm_map has → unbounded `usize` overflow + lock-held loop; mirror the size bound + `checked_add`+`try_new` in the loop. Add negative tests (non-canonical va → EFAULT; oversized size → EINVAL). Uncertain (guard proactively): `unmap_user_range` frees frames without the `BORROWED_LEAF` check the Drop walker has — latent cross-feature UAF if browser-runtime + atrium-compositor-g1 ever co-enable; make unmap skip BORROWED_LEAF. Rebase-fresh, re-run the 4-shard QEMU gate, paste the URL; PM re-audits the memory-safety lens then merges.
- **RAG** (PowerShell): health-check first; e.g. `sys_vm_unmap_impl VirtAddr new try_new`, `pmap unmap_user_locked BORROWED_LEAF Drop walker`, `MAX_MAP_REGION_SIZE sys_vm_map_impl bound`.
