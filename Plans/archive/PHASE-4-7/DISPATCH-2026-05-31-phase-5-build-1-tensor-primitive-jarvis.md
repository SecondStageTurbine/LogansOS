<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 5 BUILD-1 — Tensor Primitive → Jarvis

**Date:** 2026-05-31
**From:** PM/Cerebro
**To:** Jarvis (worker tree, feature branch)
**Status:** DISPATCHED — ready for BUILD
**Authoritative plan:** `Plans/PM-DECISION-2026-05-31-phase-5-opening.md` (BUILD-1 PLAN, ISC-1…ISC-11 + anti-criteria) — **the ISC list is your acceptance gate.**
**Design substrate:** `docs/ADR-027-phase-5-ai-foundation-design-substrate.md`
**Syscall range:** Bible v1.10.0 Article 6.1 — AI = **`0x100..0x17F`** (NOT `0x180..`, which is net).

---

## 1. Objective

Implement the Article 20 tensor primitive in a new `kernel/src/ai/` subsystem:
capability-gated, refcounted, immutable-by-default, created from userspace bytes,
storage in the **distinct tensor-memory allocation class** (Article 7.5 — buddy
frames, **not** the slab/heap). Four syscalls. Host-unit-tested + QEMU-verified.

This dispatch carries the completed **OBSERVE integration map** (§4 below) so you
can go straight to THINK → `/simplify` → BUILD/VERIFY. Cerebro already did the
reconnaissance; verify it still holds, don't re-walk it from scratch.

## 2. Decisions already locked (do not re-litigate)

- **Syscall numbers:** `SYS_TENSOR_CREATE=0x100`, `SYS_TENSOR_QUERY=0x101`,
  `SYS_TENSOR_SHARE=0x102`, `SYS_TENSOR_DROP=0x103`. (An earlier recon pass
  suggested `0x1A0+` — that is **wrong**, it collides with the net range. Use
  `0x100..0x103`.)
- **Storage = contiguous buddy frames**, not `Vec`/`Box` (those are slab/heap,
  forbidden for tensor storage by Article 7.5). Allocate via
  `BuddyAllocator::allocate_blocks(order)`; the *metadata* struct (`Tensor`) may
  be a normal `Arc` heap object — only the data bytes must be the distinct class.
- **`SYS_TENSOR_DROP` is required** — `sys_close` is File-only (asserts
  `Handle::File`), it will not drop a tensor. So the dedicated drop is not
  redundant; keep it.
- **Immutability enforcement is deferred, not built.** BUILD-1 has **no mutation
  syscall**, so there is nothing to reject — do **not** invent a mutator just to
  test `EPERM`. Store the `mutable` flag and make it queryable; the EPERM-on-
  mutate guard lands in the build that first introduces a mutator (Layer 2). This
  is a scoped refinement of PLAN ISC-7/ISC-8; note it in your close report.
- **`Device::Accel(u32)` is a reserved enum variant with NO driver code**
  (Phase 6). Shipping any accelerator machinery in BUILD-1 fails ISC-11.

## 3. Suggested object shapes (THINK may refine; keep lean)

```text
Shape   { rank: u8 (≤ 8), dims: [u64; 8] }      // num_elements() = product of first `rank` dims
Dtype   enum { F32, F16, BF16, I32, I8, U8, Bool } with size_bytes() {4,2,2,4,1,1,1}
Device  enum { Cpu, Accel(u32) }                // Accel reserved, unused
Tensor  { shape, dtype, device, mutable: bool, storage: TensorStorage }   // wrapped Arc<Tensor>
TensorStorage { frame: PhysFrame<Size4KiB>, order: usize, len: usize }     // Drop frees the buddy block
```

- `byte_len = num_elements * dtype.size_bytes()`; validate `data_len == byte_len`,
  `rank ≤ 8`, dims non-zero. `order = ceil(log2(ceil(byte_len/4096)))`.
- **Create ABI:** pass a `#[repr(C)]` descriptor by pointer:
  `{ rank: u32, dtype: u32, mutable: u32, _pad: u32, dims: [u64;8], data_ptr: u64, data_len: u64 }`
  → `sys_tensor_create(desc_ptr, desc_len)`.
- **Query ABI:** copy out `{ rank, dtype, device, refcount, dims:[u64;8], byte_len }`
  → `sys_tensor_query(handle, out_ptr, out_len)`. `refcount` = `Arc::strong_count`.
- **Share:** look up `Handle::Tensor`, clone the `Arc`, install a new
  `Handle::Tensor` with a fresh `cap_id` → new handle index (refcount++).
- **Drop:** mirror `sys_close` but match `Handle::Tensor`; remove handle + its
  `derivations` entry; the `Arc` decrements, last drop frees storage.

## 4. OBSERVE integration map (verified by Cerebro 2026-05-31)

### Module wiring — `kernel/src/main.rs`
- `mod` block lines ~28–44 (`mod net;` etc.). Add `mod ai;`.
- Init order in `_kernel_start()`: `mm::init` (~401) → device init (~412) →
  `fs::init()` (~423) → `bring_up_aps` (~428) → `sched::init_bsp()` (~433).
  **Add `ai::init();` right after `fs::init()` (~line 424)** — allocator online,
  no scheduler yet, safe to panic.

### Capability table — `kernel/src/sched/process.rs`
- `Handle` enum lines 391–427. Add:
  `Tensor { tensor: Arc<Tensor>, cap_id: CapId, rights: CapRights }`.
- Update `impl Handle` (line 429):
  - `cap_id()` (430): add `| Handle::Tensor { cap_id, .. }` to the OR-pattern.
  - `rights()` (441): add `| Handle::Tensor { rights, .. }` to the rights group.
  - `with_cap()` (~459, **exhaustive, no catch-all**): add a `Handle::Tensor` arm.
  - `file_rights()` (~452) has `_ => None` — no change.
- `process::alloc_cap_id() -> CapId` (73). `process::install_cap(&proc, handle,
  parent: Option<CapId>) -> Result<u32, InstallCapError>` (666). Copy the pattern
  in `sys_socket_create_impl` (syscall.rs 769–787).
- Rights: `CapRights::new(meta: u8, semantic: u8)` (88); `full_meta_rights()`
  (140) = USE|DERIVE|GRANT|REVOKE. Define a `tensor_rights` semantic module with a
  `USE` bit (+ `full_tensor_rights()` helper), or `CapRights::new(full_meta_rights(),
  tensor_rights::USE)`. Keep semantic rights minimal — holding the cap is
  permission for BUILD-1.
- **Run `cargo check` to find any other exhaustive `Handle` matches** (grant /
  revoke / teardown / Debug). The compiler is your exhaustiveness oracle; the
  non-exhaustive `matches!`/`if let` sites (608, 1340, 1448) won't break.

### Allocator — `kernel/src/mm/`
- `mm::with_frame_alloc(|a| ...)` (mod.rs 387). `BuddyAllocator::allocate_blocks(
  order) -> Option<PhysFrame<Size4KiB>>` (frame.rs 347, **public**).
- **No public block-free exists** — `free_block(addr, order)` (frame.rs 293) is
  private+unsafe, `free(frame)` (332) is order-0 only. **Add a small symmetric
  `pub fn deallocate_blocks(&mut self, frame: PhysFrame<Size4KiB>, order: usize)`
  that calls `self.free_block(frame.start_address().as_u64(), order)`** — this is
  the lean, justified counterpart to the existing public `allocate_blocks`.
- Kernel access to bytes: `mm::phys_to_virt(frame.start_address()) -> VirtAddr`
  (mod.rs 432) → `as_mut_ptr::<u8>()`.

### Userspace copy — `kernel/src/arch/x86_64/syscall.rs`
- `validate_user_buf(addr: u64, len: usize) -> Result<(), i64>` (~386), returns
  `EFAULT`/`EINVAL`. `copy_user_bytes(src: *const u8, len) -> Vec<u8>` (~438).
  `copy_kernel_bytes_to_user(dst: *mut u8, src: &[u8])` (for query). All wrap
  `arch::x86_64::with_user_access(|| ...)` for SMAP.

### errno — `kernel/src/errno.rs`
- `EINVAL=-22`, `EBADF=-9`, `ENOMEM=-12`, `EPERM=-1`, `EFAULT=-14`, `ESTALE=-116`.
  Bare `i64` constants; return convention: **negative = error, ≥0 = handle/count.**
- Use: bad rank/shape/dtype/len → `EINVAL`; bad/wrong-variant handle → `EBADF`;
  block alloc fails → `ENOMEM`; bad pointer → `EFAULT`.

### Dispatch — `kernel/src/arch/x86_64/syscall.rs`
- Consts: add the four `SYS_TENSOR_*` after the net block (~line 88), **and add a
  doc comment marking `0x100..0x17F` as the AI-reserved range** (Bible 6.1).
- `syscall_dispatch(num, a0..a5)` (306), `match num` (321). Add four arms:
  - `SYS_TENSOR_CREATE => sys_tensor_create_impl(a0 as *const u8, a1 as usize)`
  - `SYS_TENSOR_QUERY  => sys_tensor_query_impl(a0 as u32, a1 as *mut u8, a2 as usize)`
  - `SYS_TENSOR_SHARE  => sys_tensor_share_impl(a0 as u32)`
  - `SYS_TENSOR_DROP   => sys_tensor_drop_impl(a0 as u32)`

### Test harness — `kernel/src/test_harness.rs`, `kernel/src/scenario/`, userland `neo-init`
- Add Cargo feature `neo-test-tensor-create-query-drop`; gate a
  `SCENARIO = "tensor_create_query_drop"` const; add a runner that drives the
  userland scenario, prints `NEO_TEST/1 event=PASS name=tensor_create_query_drop`
  (or `event=FAIL reason=...`) to serial, and calls `debug_exit` (port `0xF4`,
  `EXIT_SUCCESS=0x10`). Userland exercises: create → query (shape/dtype/mutable
  match) → share (refcount up) → drop both (storage freed at refcount 0) →
  query-after-drop returns `EBADF`.

### SPDX / module conventions
- First line of every new `.rs`: `// SPDX-License-Identifier: MPL-2.0`, then a
  `//!` module doc. New docs: `<!-- SPDX-License-Identifier: CC-BY-4.0 -->`.

## 5. Acceptance gate (ISC-1…ISC-11 from the PLAN)

All ISC items in `PM-DECISION-2026-05-31-phase-5-opening.md` §4 must pass, with
the §2 immutability refinement above. Specifically verifiable:
- `cargo check -p neoplutarch-kernel --target x86_64-unknown-none` clean.
- `cargo test` host unit tests (ISC-9): shape/rank validation, `Dtype::size_bytes`,
  byte-len-vs-shape consistency, refcount arithmetic.
- rustfmt + clippy (warnings-as-errors), SAFETY comments, no post-init
  `unwrap`/`expect`, SPDX headers (ISC-10).
- `/simplify` gate before BUILD; no accelerator/`.neom`/op-kernel code (ISC-11).
- **QEMU boot scenario (ISC-8): verified in the worker tree** — this is the
  build's hardware-truth gate. (Cerebro's container has no QEMU; that is why this
  is dispatched to the worker tree, which does.)

## 6. Workflow

Worker branch off current `main` (e.g. `codex/phase-5-build-1-tensor-jarvis` or
`prometheus/...`). Single squashed commit on close. Write a BUILD-1 close report
(`Plans/CLOSE-REPORT-...`) noting ISC pass/fail, the §2 immutability deferral, and
QEMU verification evidence. Cerebro merges the branch to `main` and updates the
Article 13 ledger on phase progress.
