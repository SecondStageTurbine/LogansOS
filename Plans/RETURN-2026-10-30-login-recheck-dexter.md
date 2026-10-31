<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — #186 re-check (items 2 and 4), Dexter

**Seat:** Dexter (DeepSeek family) · **Re-check of:** my `Plans/RETURN-2026-10-30-product-login-review-dexter.md`
**Reviewed at:** `#224` head `f82d2b3d162c71126c649bbebd162a348ad24262` (`pr224`)
**Branch:** `dexter/login-recheck-2026-10-30` · **Base:** `origin/main` = `bf2fc858c1d56ad5aa9bed94c643b9c775265ba9`
**RAG:** health-checked, `{"status":"healthy","ollama":true,"qdrant":true}`. All source read directly.

## Verdict

| # | Item | Verdict |
|---|---|---|
| 1 | Every path to `.system/` is EPERM without the `SystemStore` cap; cap not grantable/derivable/inheritable | **CLOSED** (two qualifiers) |
| 2 | `NEOFAIL1` persists across a reboot, written before the check, fails closed, resets only on success/unlock/recovery | **CLOSED** (one fail-safe gap) |
| 3a | Mount/unmount lets a process swap the volume the record is read from | **STILL OPEN** |
| 3b | `show()` sets `wait_until = 0`, so a lock skips a saved delay | **Real, but inert today** — one-line fix |
| 4 | Anything new since `45798eb3` | Four commits; one is a new product-image surface |

My two findings were addressed in the right places and with the right shape. The re-check found **one new
opening** (3a) that is not part of the fix and that defeats the same property by a different route, and **two
smaller gaps** (qualifier A, gap G1) that are worth one line each.

---

## 1. Item 4 — CLOSED

The fix is a kernel capability, not a path blacklist at one call site, which is why it holds across the surfaces
you listed.

### Coverage, surface by surface

| Surface | Where gated |
|---|---|
| `SYS_OPEN` (root form) | `system_store_check` first, before the path resolves — `kernel/src/arch/x86_64/syscall.rs:8662`, called at `:6339` |
| `SYS_MODEL_LOAD` | same check first in `model_load_path` — `:2897` |
| Relative opens | **Not expressible.** `normalize_absolute_path` rejects a non-`/` path (`kernel/src/fs/mod.rs:1817`), so a relative open is `EINVAL` before any check or resolution |
| `.system` as a directory, then a child | Opening the directory is itself a system-store path → `EPERM`; there is no dirfd-relative open in the ABI |
| `SYS_READDIR` | fd-only (`sys_readdir_impl`); the fd cannot be obtained |
| `SYS_FILE_REPLACE` | fd-only (`file_lookup(fd, WRITE)`); gated by the open |
| `SYS_READ`/`WRITE`/`FTRUNCATE`/`FSYNC` | fd-only; gated by the open |
| Path tricks `//`, `.`, `..` | All folded by `normalize_absolute_path`'s segment loop (`fs/mod.rs:1817`) **before** the open resolves. The predicate is `normalized.split('/').nth(1) == Some(".system")` (`fs/mod.rs:1812`); the normalizer drops the leading `/`, so `nth(1)` is the mount-relative *second* component. `/neofs/.system/…`, `//neofs//.system/…`, `/neofs/./.system/…`, `/neofs/x/../.system/…` all match; `/neofs/.systemfoo` does not (exact component equality) |
| Mount names | `validate_mount_name` rejects an empty name, `.`, `..` and any `/` (`fs/mod.rs:1150`) — no second route through the mount table |

I audited the whole ABI for path-taking entry points: only `SYS_OPEN` and `SYS_MODEL_LOAD` take a path. Every other
file syscall takes an fd. The two debug helpers (`debug_sys_open_kernel_path_for_test`) route through
`open_path_and_install`, so they carry the check too, and are `neo-test`-gated.

### The cap cannot spread

- Rights are `USE | STORE`, no `DERIVE`, no `GRANT` (`kernel/src/sched/process.rs:354`) → `derive_handle` refuses
  (`DERIVE` required), and `sys_grant` refuses both by the `GRANT` rule and by an explicit
  `matches!(source, Handle::SystemStore { .. })` at `syscall.rs:7838`.
- `sys_spawn` refuses it outright (`syscall.rs:1871`).
- It cannot even be used as a channel/process handle: `ipc_lookup`, `channel_lookup` and
  `process_handle_lookup` all return `EBADF` (`syscall.rs:5871`, `:5918`, `:5963`).
- Minted only by `root_system_store_cap()` (`process.rs:1318`) into the session root at spawn; no syscall twin.
- It is in the product image: `"system-store"` in `PRODUCT_IMAGE_KERNEL_FEATURES` (`tools/xtask/src/main.rs:5988`)
  and asserted by the pairing test (`:20622`). My item-4 finding was that `cap-rooted-open` was **off**; that is
  still true (`system-store` is deliberately independent of it — it gates a *path prefix*, not the root form),
  which is the right call: it closes the record on an image that still ships the ambient root form.
- Gated by `login_record_guard`, which asserts
  `RECORDGUARD read=-1 write=-1 dotdot=-1 dir=-1 create=-1 control=ok`
  (`tools/xtask/src/login_scenarios.rs:380`) — a no-cap process is refused on read, write, `..`, the directory and
  a create, while `/neofs` still opens. That is a real discriminator, not a rubber stamp.

### Qualifier A — the *root* mount's `.system` is not protected

`is_system_store_path("/.system/x")`: normalizes to `.system/x`, `split('/').nth(1)` is `x`, so it is **false**.
Only a *child* mount's `.system` (`/neofs/.system`) matches. Harmless today — the record is on the `neofs` child,
and the root is the testfs with no `.system` (`fs/mod.rs:1659`) — but the guard silently stops applying if the
record ever moves to the root volume. One line (`nth(1)` → also accept `nth(0) == Some(".system")`) or an assert
that no root-level `.system` exists.

### Qualifier B — three surfaces are covered by construction, not by a probe

`login_record_guard` probes read / write / `..` / dir / create / control. It does **not** probe `//` or `./`, a
relative path, `SYS_MODEL_LOAD`, or `SYS_FILE_REPLACE`. Each is safe for a reason I verified by reading (above),
but the gate cannot falsify that reason. Two cheap probes would make it falsifiable: `//neofs//.system/account.cred.a`
and a `SYS_MODEL_LOAD` of the record.

## 2. Item 2 — CLOSED

| Requirement | Where | Status |
|---|---|---|
| Two copies, `NEOFAIL1` | `login-core` `attempts`, `PATH_A/PATH_B` (`userland/login-core/src/lib.rs:442-454`) | ✅ 48-byte record, magic/version/checksum/generation-zero/reserved checks (`:491`) |
| Written **before** the check | `attempts.begin()` = `set(failures+1)` (`login.rs:278`), called at `:671` (sign-in) and `:728` (recovery) before `verify_*`; lock at `lock.rs:369` before `check_password` | ✅ |
| A failed write refuses the attempt | `begin()` false → `Err(188)` in sign-in/recovery, `Ok(false)` (stays locked) in the lock | ✅ fail closed |
| Survives a reboot | `Attempts::load` at `:649`, and `wait_out(delay_secs(loaded))` **before** the first check at `:669-670` | ✅ |
| Not reset by a torn copy | Alternating target (`state.next`) + generation, so the other copy stays whole; `select` takes the newest valid copy for the account (`login-core:530`) | ✅ |
| Not reset by a corrupt copy | Both present and both undecodable → `FAIL_CLOSED = 9` → `delay_secs(9)` = 60 s (`login-core:454`, `:530`) | ✅ |
| Generation overflow | `checked_add` → `None` → `set` false → attempt refused (`login-core:560`) | ✅ |
| Resets only on success / unlock / recovery | `:677`, `lock.rs:373`, `:743` | ✅ |
| Gated | `login_backoff_reboot`: boot 2 four wrong → `LOGIN backoff test stop`; boot 3 → `LOGIN backoff wait_s=2` then sign-in (`tools/xtask/src/login_scenarios.rs:356`) | ✅ |

### Gap G1 — a failed reset is silently swallowed

All three resets are `let _ = attempts.set(0);` (`login.rs:677`, `:743`, `lock.rs:373`). If that write fails, the
session starts anyway and the old high count stays on disk, so the **next** boot charges a delay the user does not
owe (up to 60 s). It never *under*-counts, so it is not a bypass — but the requirement is "resets only on success",
and today a failed reset is indistinguishable from a successful one. Either log it (`LOGIN attempts reset=failed`)
or keep the count in memory and retry; neither is load-bearing for security.

### Two wording notes

- "Fails closed to the higher count" is implemented as a **constant** `9`, not the higher of the two copies
  (`login-core:530`). That is stronger in the dangerous direction and correct; only the THINK wording ("the longest
  delay") matches, so align the requirement text rather than the code.
- An **absent + corrupt** pair resets to 0 (`damaged` requires both present). Making a copy absent needs `unlink`,
  which the ABI does not have, so it is not reachable — but it is the one shape where "both damaged" and "one gone,
  one damaged" differ, and it is worth a comment at `login-core:530` so a future `unlink` does not re-open it.

## 3a. Unmount/remount — STILL OPEN

**Reachable, and it does not need `cap-rooted-open` to be off — it needs only an ordinary `MOUNT` handle.**

`do_mount` / `do_umount` gate on a `Handle::File` on a directory whose `rights ∩ handle` contains `MOUNT`
(`kernel/src/arch/x86_64/syscall.rs:6590`, `:6614` → `fs/mod.rs:536`, `:557`). There is **no capability check**.
`MOUNT` is an ordinary `FileRights` bit (`fs/mod.rs:59`), included in `KNOWN_BUILD4B`, and `MOUNT_RESERVED` is
merely an alias for it (`:60`) — nothing reserves it. The root testfs directory deliberately offers it
(`fs/mod.rs:1659`), and `rights_mask` for the root mount is `KNOWN_BUILD4B`.

So, in the product image where `cap-rooted-open` is off and any process may open root paths:

1. `SYS_OPEN("/", OPEN_ROOT_TEMPORARY, MOUNT)` → a mount-capable root handle.
2. `SYS_UMOUNT(root_fd, "neofs")` → succeeds whenever `live_handles == 0` (`remove_child`, `fs/mod.rs:768`,
   returns `Busy` otherwise).
3. `SYS_MOUNT(root_fd, {target:"neofs", source:"deviceN", fs_type:2}, 0)` → a writable NeoFS of the caller's
   choosing at `/neofs`.

**What is reachable today.** Step 1 and 2: the **unmount** itself. The mount table is global, so any process can
remove the user's home volume. Consequences now: every `/neofs` open fails for the rest of the session (the WM's
lock finds no record and *stays locked* — fail closed, so not a bypass, but an unrecoverable lockout until reboot),
and the user's session is broken. That is a DoS and a namespace-integrity break — an unprivileged process mutating
the global VFS namespace — and it is worth closing on its own.

**What is *not* reachable today.** Planted foreign record. `vfs_mount_at_parent` mounts `deviceN` through
`drivers::block::get_device`, and the block layer registers only AHCI, NVMe and virtio-blk
(`kernel/src/drivers/block/*`). `kernel/src/drivers/usb/` has **no mass-storage driver** (I listed the directory:
boot_eth0, cdc_ecm, hid_*, mt7601u, xhci — no storage class), and there is no syscall that writes or formats a
device. So the USB-stick-with-its-own-`.system/` variant cannot become a block device in this tree. The record-swap
consequence is **latent**: it becomes live the day a removable block driver lands, and 3a's fix should land before
that.

One thing that *is* right and worth keeping: the `system-store` check is per-path, so mounting the **real** volume
at a different name does not leak the record — `/stolen/.system/account.cred.a` is `EPERM` too. The gap is the
volume swap, not the check.

**What should close it.** A kernel-minted, non-delegable mount authority — the exact shape `system-store` uses —
consulted by `mount_child`/`umount_child` (a `MountStore` right, `USE|MOUNTSTORE`, minted into the session root
only). Equivalent and cheaper: stop exposing `MOUNT` to a plain root open, i.e. mask `MOUNT_RESERVED` out of
`requested` in `open_root_temporary` unless the caller holds the authority — the `_RESERVED` name already implies
that intent. Interim shim if the capability waits: have the WM hold a session-long handle on `/neofs`, which keeps
`live_handles > 0` and makes the unmount `EBUSY`; that closes the window but not a mount before the WM starts.
This is a PL decision (it changes what a client may do), but it should not default to "open".

## 3b. `show()` sets `wait_until = 0` — real, but inert today

`lock.rs:301` clears `wait_until` right after loading the persisted count into `s.failures` (`:299-300`), so a
count ≥ 3 owes no delay at the next lock paint. The first attempt after that paint is checked immediately.

**Why it does not bite in the normal flow.** `show()` runs only from `lock_input`, once per lock, and a lock can
only follow a successful sign-in, unlock or recovery — each of which resets the count to 0. So at the moment
`show()` runs, the owed delay is normally 0.

**Where it does bite.** Two shapes:
- A failed `attempts.set(0)` (gap G1) leaves a high count after a successful sign-in; the lock then gives one free
  guess per lock cycle. Bounded (a new cycle needs an unlock), but needless.
- It makes the code's intent unclear: the field is loaded and then contradicted two lines later.

**The reboot case you are worried about is not this code.** After a reboot the session starts at `sign_in`, which
applies `wait_out(delay_secs(loaded))` *before* the first check (`login.rs:669-670`); `login_backoff_reboot` proves
it (`wait_s=2` on boot 3). So the reboot bypass is closed on the login path; only the lock paint drops the delay.

**Fix (one line, `lock.rs:301`):**

```rust
let owed = lc::delay_secs(s.failures);
s.wait_until = if owed == 0 { 0 } else { now_ns().saturating_add(u64::from(owed) * 1_000_000_000) };
```

Then both paints enforce the backoff and G1 stops mattering.

## 4. New since `45798eb3`

The PR's own five commits (it also carries main's newer work by rebase):

| Commit | What |
|---|---|
| `d231e5f4` | The original "login and the lock in the product image" — patch-identical to `45798eb3` (same `git patch-id`), rebased |
| `f73cc6dd` | **`system-store`**: kernel feature, `SystemStore` handle/rights/cap, `KIND_SYSTEM_STORE`, the three IPC lookups, spawn/grant refusals, `is_system_store_path` + `system_store_check`, product feature, `login_record_guard` scenario, `record-guard-probe` |
| `029dfc18` | **`NEOFAIL1`**: `login-core::attempts` (encode/decode/select/next, 88 lines of host tests), `Attempts` in `login.rs`, the lock counting with the same record, `login_backoff_reboot` scenario, `login-attempts-per-boot-control` kill-switch, `login-backoff-test` stop hook |
| `7c1894cd` | THINK §3.2 ("the record is the WM's alone") and §4.4 ("it survives a reboot") rewritten to match; `login.rs` doc text |
| `f82d2b3d` | `"sak-lock-route"` (#216) added to `PRODUCT_IMAGE_KERNEL_FEATURES` (`tools/xtask/src/main.rs:5984`) + the pairing assert (`:20620`) |

Two observations on the last two:

- **`sak-lock-route` is a new product-image surface and it is not part of my #186 review.** It brings #216's
  kernel-owned lock route (CP-97) into the shipped image; `lock.rs` now calls `sys_input_sak_op(OP_ENGAGE, ..)` and
  `lock_input` fails the lock with `Err(194)` if the kernel refuses (`lock.rs:269-272`). Failing closed is right,
  but it is a behaviour change in the shipped image that should be named in the merge report, not just a feature
  string. It came from main, so it is not this PR's code — it is this PR that first enables it in the product.
- **The branch is stale-based.** Merge-base with `origin/main` is `5e4f7a2c`; `origin/main` is `bf2fc858`, 20+
  commits ahead, and the tree diff vs main shows ~4.6k deletions. It needs a rebase and a fresh gate before merge.
  All of the above is of the PR's own commits, which should apply unchanged.

## Disposition

- **Item 4: CLOSED.** Qualifiers A (root `.system` not matched) and B (three unprobed surfaces) are one-liners /
  two probes, not blockers.
- **Item 2: CLOSED.** Gap G1 (swallowed reset) is fail-safe; fix opportunistically with 3b.
- **3a: STILL OPEN and should gate** — an unprivileged process can unmount the user's volume today; the
  credential-swap consequence is latent behind the missing USB storage driver. It needs a mount authority, and it
  is a PL decision because it changes what clients may do.
- **3b: fix with the one-liner**; it is inert today only because the reset paths work.
- I did not run any gate; this is a code review against `f82d2b3d`.
