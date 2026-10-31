<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: copy-bounds table, relay #153 / #201

Reviewer: Jarvis. Worktree: `E:\AI Automation\workers\jarvis`.
Branch: `codex/review-bundle-2026-10-29c-jarvis`. **FAIL: one table row
overstates the tensor-relative pre-copy bound.** Read squash `1faa0bec`;
all syscall line numbers in this RETURN refer to that squash.

1. **FAIL for tensor write; PASS for the other 24 sites.** The complete
   inventory below uses `kernel/src/arch/x86_64/syscall.rs` copy-site lines.
   Each bound is checked before the indicated copy, with the exception noted.

   | Copy site | Pre-copy bound actually enforced |
   |---|---|
   | :1090 OPEN watchdog reread | successful bounded OPEN, same scalar path length <=256 |
   | :1612 constrained descriptors | caller's request validation caps count at 16; checked count*8 |
   | :1813 SPAWN name | nonzero, <=64 |
   | :2052 CREATE descriptor | exactly size_of TensorCreateDesc |
   | :2212 CREATE data | shape byte length equality, <=MAX_TENSOR_BYTES (64 MiB) |
   | :2400 WRITE data | <=64 MiB; tensor-relative end check **only if mutable** |
   | :2779 MODEL_LOAD path | nonzero, <=256 |
   | :4570 GENERATE_MASKED mask | request validator: <=32 KiB and ceil(mask_bits/8) equality |
   | :5165 NET_RESOLVE name | nonzero, <=253 |
   | :5194 NET_DIAL name, no cap lane | nonzero, <=253 |
   | :5234 NET_DIAL name, cap lane | nonzero, <=253 |
   | :5315 NET_DIAL_CAP_DERIVE host | nonzero, <=253 |
   | :5340 TCP_ENABLE_TLS key | exactly KEY_MATERIAL_LEN |
   | :5423 socket address helper | nonzero, <=32 |
   | :5438 SOCKET_BIND address | nonzero, <=32 |
   | :5461 SOCKET_SEND TCP data | nonzero, <=2048 |
   | :5489 SOCKET_SEND UDP data | same enclosing <=2048 check |
   | :5521 SOCKET_SENDTO data | nonzero, <=2048 |
   | :5525 SOCKET_SENDTO address | nonzero, <=32 |
   | :6207 OPEN path | nonzero, <=256 |
   | :6523 MOUNT request | nonzero, <=1024 |
   | :6537 UMOUNT name | nonzero, <=256 |
   | :6565 FILE_WRITE data | <=4096, zero handled before copy |
   | :7341 THREAD_SET_NAME | <=32 |
   | :7698 BUFFER_CREATE descriptor | exactly 24 |

   Concrete mismatch: create a 64-byte immutable tensor with a WRITE-capable
   handle, then request a 32 MiB write at offset zero from a mapped user range.
   `sys_tensor_write_impl` (:2392) skips the end check because is_mutable=false,
   then :2400 attempts the whole 32 MiB copy, despite the table claiming the
   copy is bounded by the named tensor. On the default 4 MiB heap it returns
   ENOMEM before `tensor_write_into` can return EPERM. The fixed absolute cap
   and fallible reserve prevent the original panic, but the tensor-relative
   table claim and the PR's immutable-EPERM precedence claim are false. Refuse
   immutable writes before allocating, and cover this case permanently.
2. **PASS — copy-allocation Err has no partial requested effect.**
   `copy_user_bytes` (:1498) reserves before setting length or accessing source;
   every syscall copy uses `?` or returns Err before write/create/send/bind/
   mount/name mutation. SENDTO's first temporary drops if its address copy
   fails. The tensor Arc is pinned across validation/copy and drops normally
   on Err, so a concurrent handle-index reuse cannot redirect the write.
   The watchdog reread deliberately uses `is_ok_and`: failure skips optional
   telemetry after an already-successful OPEN and still returns that fd;
   it does not return ENOMEM after partially completing OPEN.
3. **PASS by old-code trace and inspected author falsification.**
   The squash adds `user_copy_bounds` in `userland/neo-init/src/main.rs`;
   its first oversized WRITE requests 1 GiB on a 64-byte tensor. The parent
   version of `copy_user_bytes` uses infallible Vec::with_capacity and WRITE
   copies before tensor lookup/range checks. #201 records the old-code
   `memory allocation of 1073741824 bytes failed` panic at check 2, while the
   fixed scenario checks EINVAL/EBADF/ENOMEM, a valid-write control and later
   syscall service. No new kernel/QEMU execution was performed here.

Read all 25 copy expressions and their enclosing validation paths, including
the constrained/mask validators in `kernel/src/ai/generate_logic.rs`.
Independently queried
[batch 16](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40843436986):
success at `fb039fbdd6b925e4aaf346025e6789f34e0cd908`. RAG healthy.
This RETURN checks the dispatched table and Err paths; it does not establish
the absence of every other infallible kernel allocation.
