<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# SDK-v0 ABI Reference

This document is the **developer-facing ABI contract** for LogansOS userland programs at Phase 8 (SDK-v0).
Every syscall number, argument order, return convention and errno below is transcribed from the source of
truth, not from memory. A transposed number would break your binary.

**Source of truth:**
- Syscall numbers and dispatch: `kernel/src/arch/x86_64/syscall.rs` (the `SYS_*` constants, the
  `syscall_dispatch` match, and one `sys_*_impl` per call)
- Errno constants: `kernel/src/errno.rs`
- libneo mirrors and wrappers: `userland/libneo/src/lib.rs` (plus `install_abi.rs`, `mem_info.rs`,
  `handle_info.rs`, `wait_abi.rs`)
- Product image feature list: `PRODUCT_IMAGE_KERNEL_FEATURES` in `tools/xtask/src/main.rs` (image A)
- ABI structs: `kernel/src/observability.rs`, `kernel/src/ai/{tensor,model}.rs`,
  `kernel/src/atrium/{logic,input_logic,buffer}.rs`, `kernel/src/sched/handle_info_logic.rs`

**Drift guard:** `tools/xtask` carries the host test
`tests::sdk_abi_doc_covers_every_kernel_syscall`. It parses every `pub const SYS_*` out of
`kernel/src/arch/x86_64/syscall.rs` and requires a matching **inventory row** here — same name, same
decimal and hex number — for each **non-test** constant; a missing row, a wrong number, or a row naming
a constant the kernel does not define is a red `cargo test --workspace`. Deleting one row fails the
test even when the name still appears in a note. Test-only probes are documented in their own table and
are excluded.

---

## Calling convention

`SYSCALL` with the number in `rax` and the arguments in `rdi, rsi, rdx, r10, r8, r9` (the Linux order).
The kernel returns a single `i64` in `rax`:

- **negative** = `-errno` (see the table below);
- **zero or positive** = success, with the per-call meaning documented in the inventory.

Every pointer is a user virtual address. The kernel validates `[ptr, ptr+len)` against the user half
before touching it: an out-of-range, wrapping or kernel-space range is `EFAULT`. Handle-index arguments
are 32-bit values resolved in the calling process's own handle table; an empty or wrong-kind slot is
`EBADF`, and a correctly-typed slot lacking the required right is `EPERM`.

**Common errno across the pointer- and handle-taking calls:** `EFAULT` (bad user pointer/length),
`EINVAL` (a length, flag, count or struct-size validation failed), `EBADF`/`EPERM` (handle
lookup/rights), `ENOSYS` (the number is not compiled into this kernel; the caller may use it as feature
detection). The per-syscall lists below are complete for each handler and include these where they apply.

**Wire records** (fixed byte layouts) are length-checked exactly: a caller that passes the wrong `len`
gets `EINVAL` and no bytes are written. They are little-endian on x86_64.

---

## ABI Stability Tiers

The SDK-v0 surface is classified into four tiers. This is an **honest promise** — not a premature
guarantee.

### STABLE

Will never change handler, number, or value. New entries are additive only.

| Surface | Promise |
|---------|---------|
| **Syscall numbers** (98 non-test, 111 defined) | Number → handler mapping is permanent. Test-only probes may be removed; production numbers are never reused or renumbered. |
| **Syscall signatures** | `fn(u64, u64, u64, u64, u64, u64) -> i64` at the machine level; each call uses a prefix of `a0..a5`. Negative return = error (errno). Zero or positive = success. |
| **Errno values** (24 constants) | Value will not change. New errno constants may be added. |

### STABILIZING

Struct name and syscall number are stable. Layout may evolve with a version bump. The kernel guards
fixed-size out-buffers with an exact buffer-size check — a mismatched binary gets `EINVAL`.

| Surface | Promise |
|---------|---------|
| **Versioned ABI families** (`ProcessSnapshotV1`, `SysStatV1`, `KernelObservabilityV1`, `ModelQueryDesc`) | Leading `version: u32` field. Adding fields bumps the version constant on both kernel and libneo. Removing fields is forbidden — use a successor struct (V2) instead. |
| **Length-checked records** (`NetStatsV1`, `ThreadInfoV1`, `TensorCreateDesc`, `TensorQueryDesc`, `MountRequestV1`, `BufferDescV1`, `DamageRectV1`, `PresentResultV1`, `InputEventV1`, `HandleInfo`, `MemInfoV1/V2`, `InstallRowV1`) | Fixed by length and field order. A layout change is a new record/struct, never a silent resize. |

**Live evolution case study:** `KernelObservabilityV1` went from v1 -> v2 on 2026-07-04 (commit
`3e3263f`), adding six `u64` syscall counters (+48 bytes). It went v2 -> v3 in the OT-9 aggregate
observability slice (merged 2026-07-08, `027d91c`), adding the fixed trace-ring shape
(`trace_capacity`, `trace_count`, and 16 `TraceEventV1` entries). The kernel's `sys_observe_impl`
rejects old buffer sizes with `EINVAL`. libneo mirrors the fields and exposes
`KERNEL_OBSERVABILITY_V1_VERSION = 3`. Syscall number (22) was unchanged across both bumps.

### EXPERIMENTAL

Surface is active under development. Expect additions, struct layout changes, and new syscalls.

| Surface | Evidence |
|---------|----------|
| **AI syscalls** (0x100–0x12F) | Tensor/model/inference calls added across Phase 5–8. `SYS_GENERATE*` changes shape with `real-lm`; without it they return `EINVAL`. |
| **Network syscalls** (0x180–0x190) | TCP/DNS/TLS stack functional; the optional `net-dial-cap` build changes several argument shapes. |
| **Atrium/GPU/input syscalls** (0x1A0–0x1B9) | Gated by `atrium-compositor-g1`/`atrium-pointer-forward`/`desktop-session` and a runtime switch; shapes may still move with the desktop. |

### BREAKS FREELY

Convenience wrappers in libneo, not ABI guarantees. Implementation detail, subject to change.

| Surface | Examples |
|---------|----------|
| **libneo internal helpers** | `encode_mount_request_v1()`, `parse_dirent_v1()`, `sys_recv_raw()` |

---

## Versioning Policy

Ratified by PL 2026-07-05 (OT-44 H1). Binding for all future ABI struct changes.

1. **Every ABI struct** crossing the syscall boundary MUST have `version: u32` as its first field
2. **Version constant** is defined on both kernel and libneo sides (e.g., `KERNEL_OBSERVABILITY_V1_VERSION`)
3. **Kernel validates buffer size** at entry: reject with `EINVAL` if `out_len != size_of::<Struct>()`
4. **Adding fields:** Bump version constant, insert fields, update both kernel and libneo simultaneously
5. **Removing fields:** NEVER — this breaks old binaries. Add a successor struct (V2) instead
6. **New syscalls:** Always additive. Never reuse a number. Use reserved gaps.

**Naming convention:** The "V1" suffix on the struct name identifies the *syscall family*, not the version
within that family. Versioning is carried by the separate `_VERSION` constant. Example:
`KernelObservabilityV1` at version 3 means it is the v3 layout of the ObservabilityV1 family.

> **Known gaps (honest accounting).** Three records predate the policy and still carry no leading
> `version: u32`: `NetStatsV1` (32 bytes: four `u64` counters), `TensorCreateDesc`/`TensorQueryDesc`,
> and `ThreadInfoV1` (40 bytes). They are length-checked, not versioned. Add the version field on the next
> code touch of each struct — not in a docs-only change — and update both sides in the same commit.

---

## Image A — what a shipped program can actually call

Image A is the feature closure of `PRODUCT_IMAGE_KERNEL_FEATURES` (in `tools/xtask/src/main.rs`). It
includes `desktop` (…→ `product`, `desktop-session`), `atrium-pointer-forward` (→
`atrium-input-routing` → `atrium-compositor-g1`), `real-lm`, `kernel-rng`, `kernel-sysstat`,
`system-store`, and the Zenbook hardware features — and it deliberately does **not** include
`browser-runtime`, `mem-quota`/`obj-quota`, `net-dial-cap`, `installer`, `wait-multiplex`,
`neofs-file-replace`, `cap-rooted-open`, or `kernel-observability`.

A call whose arm is not compiled returns `ENOSYS`; a call that is compiled but disabled by a runtime
kill-switch (the Atrium switch) also returns `ENOSYS`. The `A` column below records the image-A status
at the time of writing:

- `yes` — the arm and handler are compiled into image A.
- `no` — not compiled into image A; the number returns `ENOSYS` (feature detection is legitimate).
- `rt` — compiled, but still behind the Atrium runtime switch (on in the product boot).

**Notable image-A consequences:** `SYS_OBSERVE`, `SYS_PROCESS_SELF`, `SYS_HANDLE_INFO`,
`SYS_WAIT_ANY` and every `browser-runtime` call return `ENOSYS` in the shipped image. `SYS_OPEN` uses
the ambient root form (with the `SystemStore` guard on `.system/` paths), because `cap-rooted-open` is
off. Network calls use the ambient (pre-`net-dial-cap`) argument shapes. An SDK program that needs any
of these must feature-detect with `ENOSYS` rather than assume it.

---

## Syscall Inventory

Legend: `#` decimal, `hex`, `Name`, `args` (`a0..a5`, in order), `returns`, `errors` (every errno the
handler can return today, including the common `EFAULT`/`EINVAL`/`EBADF`/`EPERM`; a body delegated to the
VFS/DNS/TCP layer is noted as such), `gate` (the kernel `cfg` feature; any
of the listed features compiles the arm), `A` (image A), `libneo` (wrapper in `userland/libneo`).

### Base range (0–24) — process, IPC, file, memory, status

| # | Hex | Name | Args | Returns | Errors | Gate | A | libneo |
|---|-----|------|------|---------|--------|------|---|--------|
| 0 | 0x00 | `SYS_EXIT` | `code: i32` | does not return; records the exit code for `SYS_WAIT` | — | always | yes | `sys_exit` |
| 1 | 0x01 | `SYS_WRITE` | `fd: u32, buf: ptr, len: usize` | bytes written (= `len`) | `EINVAL` (len > `MAX_FILE_IO`), `EBADF`, `EACCES`, `EPERM`, VFS errors | always | yes | `sys_write` |
| 2 | 0x02 | `SYS_SPAWN` | `name: ptr, name_len, handles: ptr, n` | packed: low 32 = pid, high 32 = `ProcessHandle` index | `EINVAL` (name/handles/UTF-8/install), `ENOENT`, `EAGAIN`, `ENOMEM`, `EPERM`, `EBADF` | always | yes | `sys_spawn` |
| 3 | 0x03 | `SYS_CHAN_CREATE` | — | packed: low 32 = `h_a`, high 32 = `h_b` | `EINVAL`; `ENOMEM` (`obj-quota`) | always | yes | `sys_chan_create` |
| 4 | 0x04 | `SYS_SEND` | `h: u32, buf: ptr, len` | bytes sent | `EINVAL` (len 0), `EBADF`, `EPERM`, `EPIPE` (peer gone) | always | yes | `sys_send` |
| 5 | 0x05 | `SYS_RECV` | `h: u32, buf: ptr, max_len` | bytes received (truncated to `max_len`) | `EINVAL` (max_len 0), `EBADF`, `EPERM`, `EPIPE` (peer gone) | always | yes | `sys_recv`, `sys_recv_raw` |
| 6 | 0x06 | `SYS_MAP_REGION` | `size: usize` | base VA (zeroed, RW+NX) | `EINVAL` (size), `ENOMEM` | always | yes | `sys_map_region` |
| 7 | 0x07 | `SYS_KILL` | `h: u32` (ProcessHandle) | 0 | `EINVAL` (self-kill), `EBADF`, `EPERM` | always | yes | `sys_kill` |
| 8 | 0x08 | `SYS_DERIVE` | `h: u32, new_meta: u8, new_semantic: u8` | new handle index | `EINVAL`, `EBADF`, `EPERM`; `ENOMEM` (`obj-quota`) | always | yes | `sys_derive`, `sys_derive_process` |
| 9 | 0x09 | `SYS_REVOKE` | `h: u32` | 0 | `EINVAL`, `EBADF`, `EPERM` | always | yes | `sys_revoke` |
| 10 | 0x0A | `SYS_GRANT` | `channel_h: u32, src_h: u32, rights: u16` | 0 | `EINVAL`, `EBADF`, `EPERM`, `EPIPE` (peer gone) | always | yes | `sys_grant` |
| 11 | 0x0B | `SYS_RECV_GRANT` | `channel_h: u32, max_rights: u16` | new handle index | `ENOSYS` (no grant pending), `EBADF`, `EPERM` | always | yes | `sys_recv_grant` |
| 12 | 0x0C | `SYS_OPEN` | `path: ptr, path_len, flags: u32, rights: u32` | fd | `EINVAL`, `EPERM` (`system-store`/`cap-rooted-open`), `ENOENT`, `EACCES`, `EEXIST`, `EISDIR`, `ENOTDIR`, `EBUSY`, `ESTALE`, `EIO`, `EBADMSG` (`flags` 0 = open, `1<<0` = create regular) | always | yes | `sys_open` |
| 13 | 0x0D | `SYS_READ` | `fd: u32, buf: ptr, len` | bytes read | `EINVAL` (len > `MAX_FILE_IO`), `EBADF`, `EACCES`, `EPERM`, VFS errors | always | yes | `sys_read`, `sys_read_fd` |
| 14 | 0x0E | `SYS_CLOSE` | `fd: u32` (file, channel or, under `pipe`, a pipe end, also in slot 0 or 1; more kinds under `close-object-handles` and `net-dial-public-only`) | 0 | `EBADF` (unallocated, wrong kind, or stdio fd) | always | yes | `sys_close`, `sys_channel_close` |
| 15 | 0x0F | `SYS_FSYNC` | `fd: u32` | 0 (needs no file right) | `EBADF`, VFS errors | always | yes | `sys_fsync` |
| 16 | 0x10 | `SYS_READDIR` | `fd: u32, out: ptr, out_len, cursor: ptr<u64>` | bytes of one `NeoDirentV1` record; 0 at EOF | `EINVAL`, `EBADF`, `EACCES`, `ENOTDIR`, `EPERM`, VFS errors | always | yes | `sys_readdir` |
| 17 | 0x11 | `SYS_MOUNT` | `dir_fd: u32, req: ptr, req_len, flags: u32` | 0 | `EINVAL`, `EBADF`, `EPERM`, `ENOENT`, `EACCES`, `EEXIST`, `ENOTDIR`, `ENOSYS`, `EIO`, `EBADMSG` (`MountRequestV1`) | always | yes | `sys_mount` |
| 18 | 0x12 | `SYS_UMOUNT` | `dir_fd: u32, name: ptr, name_len, flags: u32` | 0 | `EINVAL`, `EBADF`, `EPERM`, `ENOENT`, `EBUSY`, `ENOTDIR`, `EACCES`, VFS errors | always | yes | `sys_umount` |
| 19 | 0x13 | `SYS_WAIT` | `h: u32, flags: u64` (`1<<0` = `WNOHANG`) | `(1<<32) \| exit_code`; `EAGAIN` for a still-running child with `WNOHANG` | `EINVAL` (self/flags), `EBADF`, `EPERM`, `ESRCH`, `EAGAIN` | always | yes | `sys_wait` |
| 20 | 0x14 | `SYS_INSPECT` | `h: u32, out: ptr, out_len` (68 = `ProcessSnapshotV1`) | 0, fills the snapshot | `EINVAL` (size), `EBADF`, `EPERM` | always | yes | `sys_inspect` |
| 21 | 0x15 | `SYS_SYSSTAT` | `out: ptr, out_len` (48 = `SysStatV1`) | 0, fills the stats | `EINVAL` (size), `EFAULT`, `ENOSYS` | `kernel-sysstat` | yes | `sys_sysstat` |
| 22 | 0x16 | `SYS_OBSERVE` | `out: ptr, out_len` (`KernelObservabilityV1`) | 0, fills the snapshot | `EINVAL` (size), `EFAULT`, `ENOSYS` | `kernel-observability` | **no** | `sys_observe` |
| 23 | 0x17 | `SYS_FTRUNCATE` | `fd: u32, new_len: u64` | 0 | `EINVAL`, `EBADF`, `EACCES`, `EPERM`, `ESTALE`, VFS errors | always | yes | `sys_ftruncate` |
| 24 | 0x18 | `SYS_FILE_REPLACE` | `fd: u32, bytes: ptr, len` | 0 | `EINVAL`, `EBADF`, `EACCES`, `EPERM`, `EBUSY`, `EEXIST`, `EIO`, `EISDIR`, `ENOENT`, `ENOMEM`, `ENOSYS`, `ENOTDIR`, `ESTALE` | `neofs-file-replace` | **no** | `sys_file_replace` |
| 25 | 0x19 | `SYS_POWER` | `action: u64` (1 = power off, 2 = restart), `flags: u64` (0) | 0: the clean shutdown started (the caller is spared until the power goes) | `EINVAL` (action, flags), `EPERM` (no `PowerControl` cap, or the cap lacks that action's right), `EBUSY` (a shutdown is already running), `ENOSYS` (a kernel without `power-cap`) | `power-cap` | **no** | `sys_power` |

Notes:
- `SYS_SPAWN` inherits only handles carrying `GRANT` (and a spawned own-directory cap); a `File` handle
  without it, or a `SystemStore`/`Installer` authority, is `EPERM`. `EAGAIN` = scheduler full or object
  quota; `ENOMEM` = a `SYS_SPAWN_BUDGETED` budget too small for the image.
- `SYS_WRITE` fd 1 is permanently stdout (serial console, plus the framebuffer console when built), unless a stdout pipe was chosen at spawn (`SYS_SPAWN_STDIO`).
  fd 0 is read-only stdin and is `EBADF` to `SYS_WRITE`/`SYS_CLOSE`.
- `SYS_READ` fd 0 is stdin when `serial-stdin` or `usb-hid` is compiled (image A has `usb-hid`).
- `SYS_OPEN` root form: `rights` is the `FileRights` bit set — `READ=1<<0`, `WRITE=1<<1`,
  `LOOKUP=1<<2`, `READDIR=1<<3`, `EXECUTE=1<<4`, `MOUNT=1<<27`. With `system-store` (image A) a
  normalized path under a mount's `.system/` opens only for a holder of the `SystemStore` authority
  (the session root); every other process gets `EPERM` before anything resolves. With
  `cap-rooted-open` (off in image A) the root form resolves only through a `RootDirCap` the caller
  holds (that build returns `EPERM` without one).
- `SYS_MOUNT`/`SYS_UMOUNT` take a directory handle; the `MOUNT` file right is enforced by the VFS body
  (`EACCES`). `MountRequestV1` is a 32-byte header (`version=1`, `fs_type`, source/target offsets and
  lengths, two zero reserved words) plus the two strings.

### Browser-runtime block (0x40–0x54) — VM, clock, threads, per-process accounts

| # | Hex | Name | Args | Returns | Errors | Gate | A | libneo |
|---|-----|------|------|---------|--------|------|---|--------|
| 64 | 0x40 | `SYS_VM_MAP` | `size: usize, prot: u64, guard: u64` | base VA | `ENOMEM`, `EINVAL` (`W\|X`, unknown prot bits) | `browser-runtime` | **no** | `sys_vm_map` |
| 65 | 0x41 | `SYS_VM_UNMAP` | `va: u64, size: usize` | 0 | `EINVAL` (size/not mapped), `EFAULT` (non-canonical `va`) | `browser-runtime` | **no** | `sys_vm_unmap` |
| 66 | 0x42 | `SYS_VM_PROTECT` | `va: u64, size: usize, prot: u64` | 0 | `EINVAL` (`W\|X`, size/not mapped), `EFAULT` (non-canonical `va`) | `browser-runtime` | **no** | `sys_vm_protect` |
| 67 | 0x43 | `SYS_CLOCK_GETTIME` | `clock_id: u64` (1 monotonic, 2 realtime) | nanoseconds | `EINVAL` (unknown clock id) | `browser-runtime` \| `atrium-pointer-forward` \| `desktop-session` | yes | `sys_clock_gettime` |
| 68 | 0x44 | `SYS_FUTEX_WAIT` | `uaddr: u64, expected: u32, deadline_ns: u64` | 0; `ETIMEDOUT`; `EAGAIN` (value changed) | `EINVAL` (misaligned), `EFAULT` (untranslatable `uaddr`); `EAGAIN`/`ETIMEDOUT` per Returns | `browser-runtime` | **no** | `sys_futex_wait` |
| 69 | 0x45 | `SYS_FUTEX_WAKE` | `uaddr: u64, count: u32` | number woken | `EINVAL` (misaligned), `EFAULT` (untranslatable `uaddr`) | `browser-runtime` | **no** | `sys_futex_wake` |
| 70 | 0x46 | `SYS_THREAD_CREATE` | `entry: u64, arg: u64, stack_size: usize, guard: u64` | tid | `EPERM` (no `BrowserRuntime` THREAD_SPAWN cap), `EINVAL` (stack size), `EFAULT` (entry) | `browser-runtime` | **no** | `sys_thread_create` |
| 71 | 0x47 | `SYS_THREAD_EXIT` | `code: i32` | does not return | — | `browser-runtime` | **no** | `sys_thread_exit` |
| 72 | 0x48 | `SYS_THREAD_JOIN` | `tid: u64, exit_out: ptr<i32>` | 0; the exit code is written through `exit_out` (a null pointer discards it) | `EFAULT` (bad `exit_out`), `EINVAL` (self/detached/already joined), `ESRCH` (no such thread) | `browser-runtime` | **no** | `sys_thread_join` |
| 73 | 0x49 | `SYS_THREAD_DETACH` | `tid: u64` | 0 | `EINVAL` (detached/join-claimed), `ESRCH` (no such thread) | `browser-runtime` | **no** | `sys_thread_detach` |
| 74 | 0x4A | `SYS_SCHED_YIELD` | — | 0 | — | `browser-runtime` \| `atrium-pointer-forward` \| `desktop-session` | yes | `sys_sched_yield` |
| 75 | 0x4B | `SYS_THREAD_SELF` | — | tid | — | `browser-runtime` | **no** | `sys_thread_self` |
| 76 | 0x4C | `SYS_THREAD_SET_NAME` | `ptr: ptr, len` (≤ `THREAD_NAME_CAP`) | 0 | `EINVAL` (too long), `EFAULT`, `ESRCH` | `browser-runtime` | **no** | `sys_thread_set_name` |
| 77 | 0x4D | `SYS_THREAD_GET_INFO` | `tid: u64, out: ptr, out_len` (40 = `ThreadInfoV1`) | 0, fills the record | `EINVAL` (size/ptr), `EFAULT`, `ESRCH` (no such tid) | `browser-runtime` | **no** | `sys_thread_get_info` |
| 79 | 0x4F | `SYS_THREAD_SET_TLS` | `base: u64` (FS.base) | 0 | `EPERM` (no thread cap), `EFAULT` (`base` >= user-half limit) | `browser-runtime` | **no** | `sys_thread_set_tls` |
| 80 | 0x50 | `SYS_PROCESS_SELF` | — | pid | — | `browser-runtime` \| `cap-rooted-open` | **no** | `sys_process_self` |
| 81 | 0x51 | `SYS_HANDLE_INFO` | `idx: u64, out: ptr, out_len` (16 = `HandleInfo`) | 0, fills the record | `EINVAL` (size), `EFAULT`, `EBADF` (empty/out-of-range/wrong kind) | `browser-runtime` \| `cap-rooted-open` \| `installer` | **no** | `sys_handle_info` |
| 82 | 0x52 | `SYS_SPAWN_BUDGETED` | `name, name_len, handles, n, budget_bytes: u64` | packed pid \| `ProcessHandle` | `EINVAL`, `ENOMEM`, plus `SYS_SPAWN`'s (`ENOENT`, `EAGAIN`, `EPERM`, `EBADF`) | `mem-quota` | **no** | `sys_spawn_budgeted` |
| 83 | 0x53 | `SYS_MEM_INFO` | `out: ptr, len` (32 = v1, 48 = v2) | 0, fills the account record | `EINVAL` (size), `EFAULT` | `mem-quota` | **no** | `sys_mem_info`, `sys_mem_info_v2` |
| 84 | 0x54 | `SYS_SPAWN_BUDGETED2` | `name, name_len, handles, n, frame_bytes, obj_bytes` | packed pid \| `ProcessHandle` | `EINVAL` (`obj_bytes` 0), `EAGAIN` (object quota), `ENOMEM`, plus `SYS_SPAWN`'s | `obj-quota` | **no** | `sys_spawn_budgeted2` |
| 85 | 0x55 | `SYS_BOOT_TIMES` | `out: ptr, len` (80 = v1) | 0, fills the boot-time record (flags: bit 0 firmware times present, bit 1 estimated TSC rate) | `EINVAL` (len), `EFAULT` (`out`) | `boot-time` | **no** (only `iso --boot-time`) | `sys_boot_times` |

Notes:
- `SYS_VM_MAP` enforces W^X: `PROT_WRITE|PROT_EXEC` is `EINVAL`. `guard != 0` leaves an unmapped page
  below the region. `PROT_READ=1`, `PROT_WRITE=2`, `PROT_EXEC=4`.
- `SYS_FUTEX_WAIT`'s deadline is an **absolute monotonic** instant in the `SYS_CLOCK_GETTIME`
  timebase; `u64::MAX` = infinite, `0` = already expired. The expected value is re-checked under the
  queue guard (no lost wakeup).
- `SYS_THREAD_SET_TLS` sets the calling thread's `FS.base` (the `arch_prctl(ARCH_SET_FS)` analogue).
- `SYS_HANDLE_INFO` returns a 16-byte record: kind, flags (including `FLAG_ROOT_DIR` for a root
  directory cap), meta rights, and semantic rights. Reading your own table grants nothing.
- The per-process accounting calls (`SYS_SPAWN_BUDGETED`, `SYS_MEM_INFO`, `SYS_SPAWN_BUDGETED2`) exist
  only in quota builds; a budget only ever narrows.

### AI range (0x100–0x123) — tensors, models, inference

| # | Hex | Name | Args | Returns | Errors | Gate | A | libneo |
|---|-----|------|------|---------|--------|------|---|--------|
| 256 | 0x100 | `SYS_TENSOR_CREATE` | `desc: ptr, desc_len` (`TensorCreateDesc`) | tensor handle | `EINVAL` (dtype/shape/length/mutable), `EFAULT`, `ENOMEM` | always | yes | `sys_tensor_create` |
| 257 | 0x101 | `SYS_TENSOR_QUERY` | `h: u32, out: ptr, out_len` (`TensorQueryDesc`) | 0, fills the record | `EINVAL` (size), `EFAULT`, `EBADF`, `EPERM` | always | yes | `sys_tensor_query` |
| 258 | 0x102 | `SYS_TENSOR_SHARE` | `h: u32` | new tensor handle (one physical copy) | `EBADF`, `EPERM`, `EINVAL` | always | yes | `sys_tensor_share` |
| 259 | 0x103 | `SYS_TENSOR_DROP` | `h: u32` | 0 | `EBADF` (unallocated or not a tensor) | always | yes | `sys_tensor_drop` |
| 260 | 0x104 | `SYS_TENSOR_WRITE` | `h: u32, offset: u64, data: ptr, len` | 0 | `EINVAL` (range), `EFAULT`, `EBADF`, `EPERM` (no WRITE right or immutable), `ENOMEM` | always | yes | *(none — no libneo constant)* |
| 272 | 0x110 | `SYS_MODEL_LOAD` | `path: ptr, path_len, flags: u32` (must be 0) | model handle | `EINVAL`, `EACCES`, `EPERM`, `ENOENT`, `ENOMEM`, `EIO`, `EBADMSG` (digest), `ESTALE`, VFS errors | always | yes | `sys_model_load` |
| 273 | 0x111 | `SYS_MODEL_QUERY` | `h: u32, out: ptr, out_len` (`ModelQueryDesc`) | 0, fills the record | `EINVAL` (size), `EFAULT`, `EBADF`, `EPERM`, `ESTALE` (NeoFS generation) | always | yes | `sys_model_query` |
| 274 | 0x112 | `SYS_MODEL_DROP` | `h: u32` | 0 | `EBADF` (unallocated or not a model), `EINVAL` | always | yes | `sys_model_drop` |
| 288 | 0x120 | `SYS_INFER_RUN` | `model: u32, inputs: ptr, n, outputs: ptr, cap, flags: u32` (0) | number of output handles written | `EINVAL`, `EFAULT`, `E2BIG` (`cap` too small), `EBADF`, `EPERM`, `ENOMEM`, `ENOSYS` (unsupported op), `ECANCELED`, `ESTALE` | always | yes | `sys_infer_run` |
| 289 | 0x121 | `SYS_GENERATE` | `model: u32, prompt: ptr, len, out: ptr, cap, packed: u64` | tokens written | `EINVAL` (bad request/token, or no `real-lm`), `EFAULT`, `EBADF`, `EPERM`, `ENOMEM`, `ESTALE` | `real-lm` (real body; otherwise `EINVAL`) | yes | `sys_generate` |
| 290 | 0x122 | `SYS_GENERATE_CONSTRAINED` | `model, prompt, len, out, cap, desc: ptr` (`GenerateConstrainedDesc`) | tokens written | `EINVAL` (bad request/candidate set, or no `real-lm`), `EFAULT`, `EBADF`, `EPERM`, `ENOMEM`, `ECANCELED`, `ESTALE` | `real-lm` | yes | `sys_generate_constrained` |
| 291 | 0x123 | `SYS_GENERATE_MASKED` | `model, prompt, len, out, desc: ptr` (`GenerateMaskDesc`) | 1 (one token written to `*out`) | `EINVAL` (bad mask, or no `real-lm`), `EFAULT`, `EBADF`, `EPERM`, `ENOMEM`, `ECANCELED`, `ESTALE` | `real-lm` | yes | `sys_generate_masked_step` |

Notes:
- `TensorCreateDesc` = `{ rank: u32, dtype: u32, mutable: u32, _pad: u32, dims: [u64; MAX_RANK],
  data_ptr: u64, data_len: u64 }`; `TensorQueryDesc` = `{ rank, dtype, device, mutable: u32,
  refcount: u64, dims: [u64; MAX_RANK], byte_len: u64 }`.
- `SYS_TENSOR_WRITE` is the only libneo gap in the syscall surface: the number exists in the kernel but
  neither the constant nor a wrapper is in `userland/libneo/src/lib.rs`. Raw `raw3`/`raw4` use works;
  treat it as kernel-only until a wrapper lands.
- `SYS_GENERATE`'s `packed` = low 32 bits `max_new_tokens`, high 32 bits `eos_token_id` (`u32::MAX` =
  no EOS stop).
- `ModelQueryDesc` is the one AI record with a leading `version: u32` (`MODEL_QUERY_VERSION = 1`).
- On a kernel without `real-lm`, all three generation calls are compiled as `EINVAL` stubs, not
  `ENOSYS`.

### Network range (0x180–0x190) — sockets, TCP, DNS, TLS

With `net-dial-cap` off (image A) the pre-capability argument shapes and ambient roots are compiled.
With `net-dial-cap` on, the legacy roots are closed (`ENOSYS`), `SYS_NET_DIAL` takes a fourth handle
argument, `SYS_NET_LISTEN_TCP` takes a root `NetDialCap` in `a3`, and `SYS_NET_DIAL_CAP_DERIVE` becomes
live. See the capability section below.

| # | Hex | Name | Args | Returns | Errors | Gate | A | libneo |
|---|-----|------|------|---------|--------|------|---|--------|
| 384 | 0x180 | `SYS_SOCKET_CREATE` | `family: u32, socktype: u32` | socket handle | `EINVAL` (family/socktype) | `not(net-dial-cap)` | yes | `sys_socket_create` |
| 385 | 0x181 | `SYS_SOCKET_BIND` | `h: u32, addr: ptr, addr_len` (≤32) | 0 | `EINVAL`, `EFAULT`, `EBADF`, `EPERM`, `EBUSY`, `EAGAIN` | `not(net-dial-cap)` | yes | `sys_socket_bind_in`, `sys_socket_bind_in6` |
| 386 | 0x182 | `SYS_SOCKET_SEND` | `h: u32, buf: ptr, len, flags: u32` (0) | bytes sent | `EINVAL`, `EFAULT`, `EBADF`, `EPERM`, `EAGAIN`, `EBUSY`, `ENOSYS` (legacy datagram under `net-dial-cap`) | always | yes | `sys_socket_send`, `sys_tcp_send` |
| 387 | 0x183 | `SYS_SOCKET_RECV` | `h: u32, buf: ptr, len, flags: u32` (0) | bytes received | `EINVAL`, `EFAULT`, `EBADF`, `EPERM`, `EAGAIN` | always | yes | `sys_socket_recv`, `sys_tcp_recv` |
| 388 | 0x184 | `SYS_SOCKET_CLOSE` | `h: u32` | 0 | `EBADF` | always | yes | `sys_socket_close`, `sys_tcp_close` |
| 389 | 0x185 | `SYS_NET_STATS` | `h: u32, out: ptr, out_len` (32 = `NetStatsV1`) | 0, fills the counters | `EINVAL` (size), `EFAULT`, `EBADF`, `EPERM` | always | yes | `sys_net_stats` |
| 390 | 0x186 | `SYS_SOCKET_SENDTO` | `h, buf, len, flags, addr: ptr, addr_len` | bytes sent | `EINVAL`, `EFAULT`, `EBADF`, `EPERM`, `EAGAIN`, `EBUSY` | `not(net-dial-cap)` | yes | `sys_socket_sendto` |
| 391 | 0x187 | `SYS_SOCKET_RECVFROM` | `h, buf, len, flags, addr_out: ptr, addr_len_io: ptr<usize>` | bytes received | `EINVAL`, `EFAULT`, `EBADF`, `EPERM`, `EAGAIN` | `not(net-dial-cap)` | yes | `sys_socket_recvfrom` |
| 392 | 0x188 | `SYS_NET_CONNECT_TCP` | `remote: ptr, remote_len, local_port: u16` | TCP socket handle | `EINVAL`, `EFAULT`, `EAGAIN`, `EBUSY` | `not(net-dial-cap)` | yes | *(none)* |
| 393 | 0x189 | `SYS_NET_LISTEN_TCP` | `local: ptr, local_len, backlog[, cap_h: u32]` | listener handle | `EINVAL`, `EFAULT`, `EACCES` (cap mismatch; `net-dial-cap` only), `EBUSY` | both arities | yes (3-arg) | `sys_net_listen_tcp`, `..._with_cap` |
| 394 | 0x18A | `SYS_NET_ACCEPT_TCP` | `listener_h: u32` | TCP socket handle | `EINVAL`, `EBADF`, `EPERM`, `EAGAIN` | always | yes | `sys_net_accept_tcp` |
| 395 | 0x18B | `SYS_NET_SHUTDOWN_TCP` | `h: u32, dir: u32` (1 read, 2 write) | 0 | `EINVAL`, `EBADF`, `EPERM` | always | yes | *(none)* |
| 396 | 0x18C | `SYS_NET_RESOLVE` | `name: ptr, name_len, family_hint: u32, out: ptr, out_len` | candidate count written | `EINVAL`, `EFAULT`, `ENOENT`, `EAGAIN` | `not(net-dial-cap)` | yes | `sys_net_resolve` |
| 397 | 0x18D | `SYS_NET_DIAL` | `name: ptr, name_len, port: u16[, cap_h: u32]` | TCP socket handle | `EINVAL`, `EFAULT`, `EACCES` (cap mismatch), `ENOENT`, `EAGAIN` | both arities | yes (3-arg) | `sys_net_dial` |
| 398 | 0x18E | `SYS_NET_TCP_ENABLE_TLS` | `h: u32, key: ptr, key_len` (exact key length) | 0 | `EINVAL` (key length), `EFAULT`, `EBADF`, `EPERM`, `ENOMEM` | always | yes | `sys_net_tcp_enable_tls` |
| 399 | 0x18F | `SYS_NET_DIAL_CAP_DERIVE` | `parent_h: u32, host: ptr, host_len, port: u16` | derived `NetDialCap` handle | `EINVAL`, `EFAULT`, `EBADF`, `EPERM`, `EACCES` (widening), `ENOMEM`, `ENOSYS` (feature off) | `net-dial-cap` (else `ENOSYS`) | yes (ENOSYS) | `sys_net_dial_cap_derive` |
| 400 | 0x190 | `SYS_GETRANDOM` | `buf: ptr, len` | bytes filled (= `len`; 0-length is a no-op) | `EINVAL` (`len` > `MAX_GETRANDOM_LEN`), `EFAULT`, `ENOMEM`, `ENOSYS` (feature off) | `kernel-rng` | yes | `sys_getrandom` |

Notes:
- Socket addresses are a small serialized form (≤32 bytes, family-tagged), not raw `sockaddr`
  structs; the libneo `SockAddr*` helpers encode them.
- `SYS_NET_STATS`' `NetStatsV1` is four `u64` counters (`packets_in`, `packets_out`, `bytes_in`,
  `bytes_out`) — no version field yet.
- `SYS_NET_SHUTDOWN_TCP` and `SYS_NET_CONNECT_TCP` have libneo number constants but no wrapper
  functions.

### Atrium: compositor, GPU, input (0x1A0–0x1B9)

All are compiled under `atrium-compositor-g1` (or the narrower feature named) and additionally guarded
by the Atrium runtime switch: with the switch off they return `ENOSYS`.

| # | Hex | Name | Args | Returns | Errors | Gate | A | libneo |
|---|-----|------|------|---------|--------|------|---|--------|
| 416 | 0x1A0 | `SYS_BUFFER_CREATE` | `desc: ptr, desc_len` (24 = `BufferDescV1`) | buffer handle | `EINVAL` (descriptor), `ENOMEM` | `atrium-compositor-g1` | rt | `sys_buffer_create` |
| 417 | 0x1A1 | `SYS_BUFFER_MAP` | `h: u32, role: u32` (0 client RW, 1 compositor RO) | base VA | `EINVAL` (role), `ENOMEM`, `EBADF`, `EPERM` (role right) | `atrium-compositor-g1` | rt | `sys_buffer_map` |
| 418 | 0x1A2 | `SYS_BUFFER_QUERY` | `h: u32, out: ptr, out_len` (24) | 0, fills the descriptor | `EINVAL` (size), `EFAULT`, `EBADF`, `EPERM` (QUERY right) | `atrium-compositor-g1` | rt | `sys_buffer_query` |
| 419 | 0x1A3 | `SYS_BUFFER_COMMIT` | `h: u32` | 0 | `EBADF`, `EPERM` (COMMIT right) | `atrium-compositor-g1` | rt | `sys_buffer_commit` |
| 420 | 0x1A4 | `SYS_FB_PRESENT` | `h: u32, dst_x, dst_y` | present epoch (≥1) | `EBADF`, `EPERM` (PRESENT right / `GpuSubmitCap`), `EINVAL` | `atrium-compositor-g1` | rt | `sys_fb_present` |
| 421 | 0x1A5 | `SYS_PRESENT_FENCE_WAIT` | `h: u32, epoch: u64, out: ptr, out_len` (32), `timeout_ticks: u64` | 0, fills `PresentResultV1` | `EINVAL`, `EFAULT`, `EBADF`, `EPERM`, `EAGAIN` (non-blocking pending), `ETIMEDOUT` | `atrium-compositor-g1` | rt | `sys_present_fence_wait` |
| 422 | 0x1A6 | `SYS_INPUT_FOCUS_CREATE` | — | client focus handle (RECEIVE, no ACTIVATE) | `EINVAL` (install) | `atrium-compositor-g1` | rt | `sys_input_focus_create` |
| 423 | 0x1A7 | `SYS_INPUT_FOCUS_ACTIVATE` | `h: u32` | 0, or `EBUSY` | `EPERM` (not the input-system holder), `EBUSY`, `EBADF` | `atrium-compositor-g1` | rt | `sys_input_focus_activate` |
| 424 | 0x1A8 | `SYS_INPUT_FOCUS_DEACTIVATE` | `h: u32` | 0, or `EAGAIN` (not the active route) | `EAGAIN` (not the active route), `EBADF`, `EPERM` | `atrium-compositor-g1` | rt | `sys_input_focus_deactivate` |
| 425 | 0x1A9 | `SYS_INPUT_FOCUS_POLL` | `h: u32` | 0 (this leaf is active), `EPERM` (not active) | `EPERM` (not active / not a client leaf), `EBADF` | `atrium-compositor-g1` | rt | `sys_input_focus_poll` |
| 426 | 0x1AA | `SYS_INPUT_FOCUS_RECV` | `h: u32, out: ptr, out_len` (8 = `InputEventV1`) | 0, fills the event | `EINVAL`, `EFAULT`, `EAGAIN`, `EBADF`, `EPERM` | `atrium-compositor-g1` | rt | `sys_input_focus_recv` |
| 427 | 0x1AB | `SYS_INPUT_FOCUS_MINT_ACTIVATOR` | `h: u32` | ACTIVATE-only sibling handle | `EBADF` (parent gone), `EPERM`, `EINVAL` | `atrium-compositor-g1` | rt | `sys_input_focus_mint_activator` |
| 429 | 0x1AD | `SYS_BUFFER_COMMIT_DAMAGE` | `h: u32, rect: ptr, rect_len` (8 = `DamageRectV1`) | 0 | `EINVAL`, `EFAULT`, `EBADF`, `EPERM` (COMMIT right) | `atrium-compositor-g1` | rt | `sys_buffer_commit_damage` |
| 430 | 0x1AE | `SYS_GPU_COMPUTE_SUBMIT_BLUR` | `src_h, dst_h, queue_h: u32` | new compute-fence handle | `EBADF`, `EPERM` (BUFFER/QUEUE caps), `EINVAL` | `atrium-compositor-g1` | rt | `sys_gpu_compute_submit_blur` |
| 431 | 0x1AF | `SYS_GPU_COMPUTE_FENCE_WAIT` | `fence_h: u32` | completed sequence number (0 = CPU floor) | `EBADF`, `EPERM`, `EINVAL` | `atrium-compositor-g1` | rt | `sys_gpu_compute_fence_wait` |
| 432 | 0x1B0 | `SYS_FB_OUTPUT_QUERY` | — | packed `(width << 32) \| height` | `EPERM` (no `GpuSubmitCap`), `EINVAL` | `desktop-session` | rt | `sys_fb_output_query` |
| 433 | 0x1B1 | `SYS_INPUT_POINTER_CLAIM` | `flags: u64` (bit 0 = reclaim) | RECEIVE-only route handle | `EEXIST` (live route), `EPERM`, `EINVAL` | `atrium-pointer-forward` | yes | `sys_input_pointer_claim` |
| 434 | 0x1B2 | `SYS_INPUT_POINTER_FORWARD` | `route: u32, tag: u64, target: u32, x: i64, y: i64` | 0 | `ESRCH`, `ESTALE`, `EALREADY`, `EPERM`, `EBADF`, `EINVAL` | `atrium-pointer-forward` | yes | `sys_input_pointer_forward` |
| 435 | 0x1B3 | `SYS_INPUT_SAK_TAKE` | `op: u64, h: u32` | chord bits / lock-op result | `EPERM` (not input-system holder), `EINVAL`, `EBUSY`, `ESRCH` | `atrium-pointer-forward` | yes | `sys_input_sak_take`, `sys_input_sak_op` |
| 436 | 0x1B4 | `SYS_TRY_RECV` | `h: u32, buf: ptr, max_len` | bytes received, or `EAGAIN` | `EAGAIN` (no sender), `EINVAL`, `EFAULT`, `EBADF`, `EPERM` | `atrium-pointer-forward` | yes | `sys_try_recv` |
| 437 | 0x1B5 | `SYS_WAIT_ANY` | `items: ptr, n, deadline_ns: u64` | ready mask (bit `i`), or `ETIMEDOUT` | `EPIPE`, `EAGAIN`, `EINVAL`, `EFAULT`, `EBADF`, `EPERM`, `ETIMEDOUT` | `wait-multiplex` | **no** | `sys_wait_any` |
| 438 | 0x1B6 | `SYS_BUFFER_UNMAP` | `va: u64` (a `SYS_BUFFER_MAP` base) | 0 | `EINVAL` (no mapping at `va`) | `atrium-compositor-g1` | rt | `sys_buffer_unmap` |
| 439 | 0x1B7 | `SYS_TRY_SEND` | `h: u32, buf: ptr, len` | bytes sent, or `EAGAIN` | `EAGAIN` (no receiver parked), `EINVAL`, `EFAULT`, `EBADF`, `EPERM` | `ipc-try-send` | yes | `sys_try_send` |
| 440 | 0x1B8 | `SYS_TRY_GRANT` | `channel: u32, src: u32, rights: u16` | 0, or `EAGAIN` | `EAGAIN` (no `SYS_RECV_GRANT` parked), `EBADF`, `EPERM`, `EINVAL` | `ipc-try-send` | yes | `sys_try_grant` |
| 441 | 0x1B9 | `SYS_TRY_RECV_GRANT` | `channel: u32, max: u16` (rights) | new handle, or `EAGAIN` | `EAGAIN` (no grantor parked), `EBADF`, `EPERM`, `EINVAL` | `ipc-try-send` | yes | `sys_try_recv_grant` |

Notes:
- `BufferDescV1` = `{ width, height, stride, format: u32, bytes: u64 }` (24 bytes);
  `DamageRectV1` = `{ x, y, width, height: u16 }` (8); `PresentResultV1` = 32 bytes
  (`present_id`, `outcome`, `flags`, `submit_tick`, `complete_tick`); `InputEventV1` = 8 bytes.
- The buffer returned by `SYS_BUFFER_CREATE` is the client root (full rights). The compositor role is
  minted from it with `SYS_DERIVE` (MAP_READ|QUERY|PRESENT, no MAP_WRITE). `SYS_FB_PRESENT` also
  requires the process-held `GpuSubmitCap` when scanout enforcement is armed.
- `SYS_INPUT_FOCUS_ACTIVATE`/`DEACTIVATE`/`POLL` and `SYS_INPUT_POINTER_*`/`SYS_INPUT_SAK_TAKE` are
  input-system authority operations: the caller must hold the WM's input-system authority (`EPERM`
  otherwise). `SYS_INPUT_FOCUS_MINT_ACTIVATOR` gives any creator an ACTIVATE sibling, which is why the
  route transfer itself is authority-gated.
- `SYS_WAIT_ANY` items are 8 bytes each (`u32 handle, u32 kind`); `deadline_ns` is absolute monotonic,
  and `n == 0` with `u64::MAX` (wait forever on nothing) is `EINVAL`.

### Pipes (0x1BA–0x1BF)

A byte pipe with per-handle endpoint counts (`docs/THINK-2026-11-02-l1a1-pipe-wiring.md`). Compiled under
`pipe` (which needs `wait-multiplex`); without it the numbers return `ENOSYS`.

| # | Hex | Name | Args | Returns | Errors | Gate | A | libneo |
|---|-----|------|------|---------|--------|------|---|--------|
| 442 | 0x1BA | `SYS_PIPE_CREATE` | `capacity: u64` (512..=65536) | `read_end \| write_end << 32` | `EINVAL` (capacity), `ENOMEM` | `pipe` | **no** | `sys_pipe_create` |
| 443 | 0x1BB | `SYS_PIPE_READ` | `h: u32, buf: ptr, len, flags` (bit 0 `PIPE_NONBLOCK`) | bytes read, 0 at EOF | `EAGAIN` (nonblocking, empty, or this side's waiter list full), `EFAULT`, `EBADF` (also: a sibling closed this end's last handle meanwhile), `EPERM` (not a read end), `ESRCH` (process retiring) | `pipe` | **no** | `sys_pipe_read` |
| 444 | 0x1BC | `SYS_PIPE_WRITE` | `h: u32, buf: ptr, len, flags` (bit 0 `PIPE_NONBLOCK`) | bytes written (all of them when blocking; ≤ 512 bytes atomic) | `EPIPE` (no reader), `EAGAIN` (nonblocking, no room, or this side's waiter list full), `EFAULT`, `EBADF` (also: a sibling closed this end's last handle meanwhile), `EPERM` (not a write end), `ESRCH` | `pipe` | **no** | `sys_pipe_write` |
| 445 | 0x1BD | `SYS_SPAWN_STDIO` | `name: ptr, len, handles: ptr, n, stdio: u64` (byte 0 stdin, byte 1 stdout, byte 2 term page: each an index into `handles` or `0xFF`; bits 24–63 zero) | as `SYS_SPAWN`: `pid \| process_handle << 32` | in this order: `SYS_SPAWN`'s pre-checks (`EINVAL`, `EFAULT`); the stdio word (`EINVAL`: a high bit, an index ≥ `n`, two roles on one index, a term index without `atrium-compositor-g1`); the name and each handle as `SYS_SPAWN` (`EINVAL`, `EBADF`, `EPERM`); each role's kind and rights (`EINVAL`); the spawn's own (`EAGAIN`, `ENOENT`, `ENOMEM`) | `pipe` | **no** | `sys_spawn_stdio`, `sys_spawn_stdio_word` |
| 446 | 0x1BE | `SYS_STDIO_INFO` | `out: ptr, len` (8) | 0; writes stdin, stdout and term-page slots (LE `u16`, `0xFFFF` none or gone), then flags (bit 0: selected at spawn) | `EINVAL` (`len != 8`, checked first), `EFAULT` | `pipe` | yes | `sys_stdio_info`, `sys_stdio_info_raw` |
| 447 | 0x1BF | `SYS_STDIO_TERM_MAP` | `role: u32` (as `SYS_BUFFER_MAP`) | the mapping's address | `EBADF` (no term page selected, or its handle has gone or been replaced), `SYS_BUFFER_MAP`'s errors, `ENOSYS` (no `atrium-compositor-g1`) | `pipe` | **no** | `sys_stdio_term_map` |
Notes:
- `SYS_PIPE_CREATE`'s return packs the read end's handle in bits 0–31 and the write end's in bits 32–63:
  handles `(0x11223344, 0x55667788)` return `0x5566778811223344`, little-endian bytes
  `44 33 22 11 88 77 66 55` (the byte oracle: `tools/pipe-logic-tests` `abi_byte_oracle`, which checks the
  kernel's packer and libneo's `pipe_abi.rs` against those bytes). `PIPE_NONBLOCK` is bit 0 of `flags`; other bits are ignored.
- A handle carries every meta right (`USE`, `GRANT`, `DERIVE`, `REVOKE`) and only its end's semantic right
  (`READ` = bit 0 on a read end, `WRITE` = bit 1 on a write end); the wrong end is `EPERM`.
- A write of at most 512 bytes (`PIPE_ATOMIC`) that fits the capacity is atomic. A `SYS_WAIT_ANY` item of
  kind 4 (`WAIT_KIND_PIPE`) on a write end is ready when a write of `min(512, capacity)` bytes would fit,
  or no reader is left; on a read end, when bytes are buffered or no writer is left.
- A timed read is `SYS_WAIT_ANY` with a deadline, then a `PIPE_NONBLOCK` read.
- The capacity is at least 512 (`PIPE_ATOMIC`), so a write of at most 512 bytes is always atomic; a smaller
  capacity is `EINVAL`.
- A read checks each chunk's destination before taking its bytes. If a sibling thread unmaps the buffer between
  that check and the copy, the read returns the bytes that reached the buffer (`EFAULT` only if none did), and
  the rest of that chunk (at most 512 bytes) is lost, as with stdin (`docs/concurrency-pitfalls.md` CP-99).
- **Stdio chosen at spawn** (`docs/THINK-2026-11-08-l1a1-stdio-selection.md`). `SYS_SPAWN_STDIO` names the child's
  stdin (a pipe read end with `READ`), stdout (a pipe write end with `WRITE`) and term page (a 4096-byte buffer
  with `MAP_READ` and `MAP_WRITE`); each handle needs `USE`, and `GRANT` to be inherited at all. The word
  `0x00FF_FFFF` (all none) gives the same child as `SYS_SPAWN`. Nothing is inherited implicitly: a child spawned
  with `SYS_SPAWN` has no selection, whatever its parent has.
- With a stdout selected, `SYS_WRITE` fd 1 is a blocking pipe write on it (atomic up to 512 bytes, `EPIPE` when
  the reader is gone; `len == 0` is 0); with a stdin selected, `SYS_READ` fd 0 is a blocking pipe read on it,
  ahead of the keyboard. Once the selected handle is closed, revoked or replaced, fd 0/1 are `EBADF` for good:
  never the console. A non-blocking or timed read uses `SYS_PIPE_READ` or `SYS_WAIT_ANY` on the slot
  `SYS_STDIO_INFO` reports.
- `SYS_STDIO_INFO`'s byte oracle (`tools/pipe-logic-tests` `stdio_info_oracle`, against the kernel's encoder
  and libneo's decoder): stdin in 0, stdout in 1, term page in 2, selected is `00 00 01 00 02 00 01 00`; the
  same without a term page `00 00 01 00 ff ff 01 00`; selected with stdout since closed
  `00 00 ff ff 02 00 01 00`; no selection `ff ff ff ff ff ff 00 00`.

### Installer (0x1C0–0x1C1)

| # | Hex | Name | Args | Returns | Errors | Gate | A | libneo |
|---|-----|------|------|---------|--------|------|---|--------|
| 448 | 0x1C0 | `SYS_INSTALL_LIST` | `out: ptr, len` (128-byte `InstallRowV1` records) | number of disks written | `EPERM` (no `Installer` cap), `EINVAL`, `EFAULT` | `installer` | **no** | `install::sys_install_list` |
| 449 | 0x1C1 | `SYS_INSTALL_RUN` | `slot: u64, generation: u64, start: u64` | install status (synchronous) | `EPERM` (no `Installer` cap), `EINVAL` | `installer` | **no** | `install::sys_install_run` |

`SYS_INSTALL_LIST` writes as many `ROW_BYTES` (128) records as fit in `len` and returns the disk count;
`len == 0` is a count-only probe. The `Installer` capability is minted once by the session root and is
inherited only by the shell.

---

### Clock (0x1C8–0x1C9)

| # | Hex | Name | Args | Returns | Errors | Gate | A | libneo |
|---|-----|------|------|---------|--------|------|---|--------|
| 456 | 0x1C8 | `SYS_CLOCK_CONFIRM_REALTIME` | `offset_s: i64` (the RTC's offset from UTC, seconds east) | 0 | `EPERM` (not input-system holder), `EINVAL` (past ±24 h), `EALREADY` (confirmed once), `EAGAIN` (no RTC epoch), `ENOSYS` (no `rtc-offset`) | `rtc-offset` | **no** | `sys_clock_confirm_realtime` |
| 457 | 0x1C9 | `SYS_CLOCK_REALTIME_STATUS` | — | 1 confirmed UTC, 0 unconfirmed | `ENOSYS` (no `rtc-offset`) | `rtc-offset` | yes | `sys_clock_realtime_status` |

`rtc-offset` (`docs/THINK-2026-11-02-time-zones.md` §2.1): `CLOCK_REALTIME` is UTC. The RTC epoch starts
unconfirmed; the session (the WM, which has the zone) confirms it once with the RTC's offset from UTC (0 when the RTC
holds UTC), which moves realtime by that offset. Read the status before the clock: a `1` means the realtime read after
it is UTC.

## Test-only syscalls (not SDK)

These are compiled only for their scenario and are **not** part of the SDK surface. They are excluded
from the drift guard. The 13 numbers:

| Number | Name | Why it exists |
|--------|------|---------------|
| 0x4E | `SYS_FUTEX_QUEUE_LEN` | C1.1 test-only queue-length probe (borrows the reserved slot) |
| 0x1AC | `SYS_INPUT_FOCUS_TEST_INJECT` | Atrium input-routing scenarios' synthetic key event |
| 0xF00 | `SYS_TEST_FLOAT_PROBE` | CP-41 MXCSR-on-entry probe |
| 0xF01 | `SYS_TEST_DIRTY_SCRATCH` | CP-43 return-register reload probe |
| 0xF02 | `SYS_TEST_IRQ_WINDOW` | CP-50 syscall-exit interrupt window sweep |
| 0xF03 | `SYS_TEST_IDLE_TICKS` | Desktop idle witness |
| 0xF04 | `SYS_TEST_TLS` | C1.2 TLS probe/park/unpark |
| 0xF05 | `SYS_TEST_THREAD_CREATE_ON` | Cross-CPU thread creation for `threads_tls` |
| 0xF06 | `SYS_TEST_FUTEX_LATE_PARK` | C1.3b late-parker futex refusal |
| 0xF07 | `SYS_TEST_KERNEL_SPIN` | C1.3b ring-0 reschedule-IPI probe |
| 0xF08 | `SYS_TEST_CAP_GRAPH_RACE` | K1a-0 capability-graph race arming |
| 0xF09 | `SYS_TEST_TAIL_HOLD` | C1.3b syscall-tail retire window |
| 0xF0A | `SYS_TEST_BLOCK_CUT` | Login-cut block device probe |

A test-only number never ships in image A; a scenario kernel that defines it is not an SDK target.

---

## Capability notes by surface

- **Root-form file access.** In image A `SYS_OPEN` is ambient (any process may name any path) except
  for `/…/.system/…`, which only a `SystemStore` holder (the session root) may open or load as a model.
  The `cap-rooted-open` build replaces the ambient root with a `RootDirCap` the caller must hold; a
  program that wants to work under both must treat `EPERM` from `SYS_OPEN` as "ask the holder".
- **Network egress.** Image A compiles the ambient network roots. The `net-dial-cap` build closes
  `SYS_SOCKET_CREATE`, `SYS_SOCKET_BIND`, `SYS_SOCKET_SENDTO`, `SYS_SOCKET_RECVFROM`,
  `SYS_NET_CONNECT_TCP` and `SYS_NET_RESOLVE` as ambient roots (`ENOSYS`), makes `SYS_NET_DIAL` consume
  an opaque `NetDialCap` in `a3` (denying a missing or out-of-scope `(host, port)` with `EACCES`), makes
  `SYS_NET_LISTEN_TCP` consume a root `NetDialCap` in `a3`, and makes `SYS_NET_ACCEPT_TCP` require the
  listener's capability. `SYS_SOCKET_SEND` stays usable for TCP sockets created by a scoped dial, and
  rejects the legacy datagram fallback with `ENOSYS`. Receive/close/shutdown/stats/TLS are handle
  operations that cannot root a new outbound connection. The design purpose is that resolve-to-IP, raw
  TCP, UDP and listener/accept paths cannot bypass the hostname-and-port scope.
- **IPC capability transfer.** `SYS_DERIVE` attenuates a handle; `SYS_GRANT`/`SYS_RECV_GRANT` move a
  derived handle over a channel; `SYS_REVOKE` removes it and every descendant derived from it. Spawn
  inheritance is not a general grant path: only handles carrying `GRANT` (and a spawned own-directory
  cap) cross a spawn.
- **Atrium authority.** Buffer rights are minted per handle and attenuated by `SYS_DERIVE`; scanout
  (`SYS_FB_PRESENT`) additionally requires the process-held `GpuSubmitCap`; input routing requires the
  WM's non-delegable input-system authority; `SYS_FB_OUTPUT_QUERY` is the output-geometry read granted
  only to that scanout holder.
- **Memory and object budgets.** A budgeted spawn's child has its own account nested under the
  caller's; a charge over any limit up the chain is a local `ENOMEM` (frames) or `EAGAIN` (objects). A
  budgeted process may not allocate tensor storage. These calls are absent from image A.

---

## ABI Structs and Records

All structs are `#[repr(C)]`, duplicated on kernel and libneo sides, and length-checked at the syscall
boundary.

| Struct | Version constant | Current | Syscall | Size |
|--------|-----------------|---------|---------|------|
| `ProcessSnapshotV1` | `PROCESS_SNAPSHOT_V1_VERSION` | 1 | `SYS_INSPECT` | 68 |
| `SysStatV1` | `SYSSTAT_V1_VERSION` | 1 | `SYS_SYSSTAT` | 48 |
| `KernelObservabilityV1` | `KERNEL_OBSERVABILITY_V1_VERSION` | **3** | `SYS_OBSERVE` | variable by CPU/trace arrays |
| `ModelQueryDesc` | `MODEL_QUERY_VERSION` | 1 | `SYS_MODEL_QUERY` | fixed |
| `MountRequestV1` | header `version` (must be 1) | 1 | `SYS_MOUNT` (input) | 32 + strings |
| `ThreadInfoV1` | *(none — length only)* | — | `SYS_THREAD_GET_INFO` | 40 |
| `NetStatsV1` | *(none — length only)* | — | `SYS_NET_STATS` | 32 |
| `TensorCreateDesc` | *(none — length only)* | — | `SYS_TENSOR_CREATE` | fixed |
| `TensorQueryDesc` | *(none — length only)* | — | `SYS_TENSOR_QUERY` | fixed |
| `BufferDescV1` | *(none — length only)* | — | `SYS_BUFFER_CREATE`/`_QUERY` | 24 |
| `DamageRectV1` | *(none — length only)* | — | `SYS_BUFFER_COMMIT_DAMAGE` | 8 |
| `PresentResultV1` | *(none — length only)* | — | `SYS_PRESENT_FENCE_WAIT` | 32 |
| `InputEventV1` | *(none — length only)* | — | `SYS_INPUT_FOCUS_RECV` | 8 |
| `HandleInfo` | *(none — length only)* | — | `SYS_HANDLE_INFO` | 16 |
| `MemInfoV1` / `MemInfoV2` | *(none — length only)* | — | `SYS_MEM_INFO` | 32 / 48 |
| `InstallRowV1` | *(none — length only)* | — | `SYS_INSTALL_LIST` | 128 |

Nested structs (no independent version, inherit the parent's versioning):
- `SchedulerStatsV1` — nested inside `KernelObservabilityV1`
- `TraceEventV1` — nested inside `KernelObservabilityV1`

---

## Reserved syscall ranges

| Range | Purpose | Status |
|-------|---------|--------|
| 0–23 | Base: process, IPC, file, memory, status | **Populated** |
| 24 | Base extension: whole-file replace | Populated under `neofs-file-replace` |
| 25 | Base extension: power-off and restart | Populated under `power-cap` |
| 26–0x3F | Reserved for base extensions | Empty |
| 0x40–0x54 | Browser host runtime: VM, clock, futex/threads, per-process accounts | Populated under `browser-runtime` / quotas |
| 0x55 | Boot-time record (L7.1) | Populated under `boot-time` |
| 0x56–0xFF | Reserved | Empty |
| 0x100–0x17F | AI: tensors, models, inference (Bible Article 6.1) | **Formally reserved**; 0x100–0x123 populated |
| 0x180–0x190 | Network: sockets, TCP, DNS, TLS, CSPRNG | Populated |
| 0x1A0–0x1B9 | Atrium: compositor, GPU compute, input routing | Populated under the Atrium features |
| 0x1BA–0x1BF | Pipes; all populated under `pipe` (0x1BD–0x1BF: stdio chosen at spawn) | none |
| 0x1C0–0x1C1 | Installer | Populated under `installer` |
| 0xF00–0xF0A | Test-only probes | **Never ships** |
| 0x4E, 0x1AC | Test-only probes in production ranges | **Never ships** |

---

## Errno Table

**24 constants. Values match Linux/POSIX for recognizability.** Kernel and libneo define identical
constants with identical values (libneo does not yet mirror `ECANCELED`). Error flow is raw `i64`
returns — no conversion layer. Negative = error, zero or positive = success.

| Constant | Value | Meaning |
|----------|-------|---------|
| `EPERM` | -1 | Capability/authority operation denied |
| `ENOENT` | -2 | No such file, process, or module |
| `ESRCH` | -3 | No such process / pointer-forward target |
| `EIO` | -5 | Storage device failed the operation |
| `E2BIG` | -7 | Caller-provided output capacity too small |
| `EBADF` | -9 | Bad handle index, wrong endpoint side, or stale fd-like ref |
| `EAGAIN` | -11 | Operation would block (or a quota/route is temporarily unavailable) |
| `ENOMEM` | -12 | Kernel could not satisfy a memory request (or a frame budget) |
| `EACCES` | -13 | Requested file rights exceed the opened object's rights / network scope mismatch |
| `EFAULT` | -14 | Bad pointer (translate failed, out of user half, or overflow into kernel space) |
| `EBUSY` | -16 | Resource is busy |
| `EEXIST` | -17 | Target already exists |
| `ENOTDIR` | -20 | Non-directory inode used where a directory is required |
| `EISDIR` | -21 | Directory/non-regular inode used where a regular file is required |
| `EINVAL` | -22 | Bounds, argument, flag or struct-size validation failed |
| `EPIPE` | -32 | Peer closed (process exit, IPC peer gone) |
| `ENOSYS` | -38 | Unrecognized syscall number, uncompiled feature, or unsupported operation |
| `ENOTEMPTY` | -39 | Directory is not empty |
| `EBADMSG` | -74 | Integrity check failed (pinned model digest) |
| `ENOTCONN` | -107 | Socket has no connected or default peer |
| `ETIMEDOUT` | -110 | A bounded wait reached its deadline |
| `EALREADY` | -114 | A one-shot operation was already done |
| `ESTALE` | -116 | Stale file capability generation / NeoFS record |
| `ECANCELED` | -125 | An in-flight inference was cancelled (kernel-only constant today) |

---

## Binary format requirements

- **Format:** ELF64 x86_64, statically linked
- **Entry point:** `_start` (extern "C", returns `!`)
- **Base load address:** `0x400000` (4 MiB into user half)
- **Segments:** Three PT_LOAD — `.text` (R+X), `.rodata` (R), `.data`/`.bss` (R+W)
- **W^X enforced:** Kernel refuses segments with both PF_W and PF_X
- **No dynamic linking, no relocations, no PT_INTERP, no PT_TLS**

See `userland/template-app/` for a minimal working example of the build configuration.
