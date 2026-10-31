<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: CP-42 SMAP asm cross-family review (Jarvis / Codex)

**Date:** 2026-10-17

**Reviewed tip:** `620171d8e5a41e48f1f781affe78db1a3758128c`

**Comparison points:** `57261d66f9c777e02a167370349a8a5fc6383b0a` (requested before tree) and
`95a2c6ce6b08b5ea52de73644532b52b4fc2b0df` (`origin/main` at review time)

**Verdict:** **MERGE.** No correctness, safety, or documentation finding survived re-verification.

## Findings

No findings.

One non-blocking pre-existing note: `with_user_access` is not nest-safe because an inner invocation would
execute `clac` before the outer closure finishes (`kernel/src/arch/x86_64/mod.rs:237`). The complete current
caller audit found no nested invocation: every closure directly performs the intended raw access. This does
not weaken CP-42, but future wrappers should not compose this helper recursively without a per-CPU nesting
guard.

## Requested checks

### 1. The barrier is real

Confirmed at the Rust contract, LLVM IR, assembly, and linked-binary levels.

- Under the Rust inline-assembly contract, omitting `nomem`, `readonly`, and `pure` means the block may read
  or write memory. That assumption prevents an ordinary or volatile access from being hoisted/sunk across
  the asm. `options(nostack)` does not weaken the memory effect.
- A pinned-toolchain microfixture lowered each fixed `stac`/`clac` to LLVM `asm sideeffect` with
  `~{memory}`. The `nomem` control lacked `~{memory}`; LLVM moved both an ordinary raw-pointer load and a
  `read_volatile` beyond `clac`. With the fix, the accesses stayed between the two asm blocks in LLVM IR
  and final assembly.
- The clobber applies to arbitrary memory, not just compiler-proven references. Forming a pointer with an
  integer-to-pointer cast does not exempt its load/store from the memory effect.
- GVN, LICM, MemCpyOpt, InstCombine, SimplifyCFG, MachineSink, and machine scheduling may transform accesses
  only while preserving the inline asm's side effects and memory dependence. The post-LTO disassembly check
  below verifies that no later IR or machine pass escaped the barrier in the reviewed build.

Primary contract references: the [Rust Reference inline-assembly options](https://doc.rust-lang.org/stable/reference/inline-assembly.html)
and LLVM's [inline assembler expression semantics](https://llvm.org/docs/LangRef.html#inline-assembler-expressions).

### 2. The options are sound

`nostack` is correct: `stac` and `clac` neither push/pop nor adjust the stack pointer. `preserves_flags` is
not required for correctness and leaving it out is conservative. On x86 the Rust Reference's
`preserves_flags` promise covers the arithmetic status flags CF/PF/AF/ZF/SF/OF, the x87 status word, and
MXCSR exception flags; RFLAGS.AC is not in that enumerated set. The instructions only change AC, so adding
`preserves_flags` could be valid under that compiler contract, but it provides no safety benefit here.

No other option should be added. In particular, `nomem`, `readonly`, or `pure` would reintroduce the bug.
The reviewed definitions are at `kernel/src/arch/x86_64/mod.rs:290-305`.

### 3. Completeness

- Exact searches of the kernel found no other `stac` or `clac` implementation. The IRQ and timer entry
  stubs clear AC with `pushfq; and [rsp]; popfq` (`idt.rs:243-244`, `timer.rs:127-128,258-259`) and do not
  carry Rust inline-asm memory options.
- `write_cr4_raw` still uses `nomem` (`mod.rs:204-207`). Its production call is the early per-CPU
  `enable_sse` path before normal Rust work; BSP and AP entry sites are `main.rs:499` and `smp.rs:217`.
  Its only other call sites are the SMEP/SMAP test scenarios. Those reassert bits already enabled during
  boot and do not bracket a live user access. The CP-42 warning against reusing it to toggle SMAP around
  live accesses is therefore accurate.
- I audited the raw `read_unaligned`, `write_unaligned`, `copy_nonoverlapping`, `read_volatile`, and
  `write_volatile` sites in the syscall and futex paths. Production user-address accesses route through
  `with_user_access`; `futex::wait` does so at `kernel/src/sched/futex.rs:219`. IPC cross-address-space copy
  translates the user VAs and touches their supervisor HHDM aliases, so it does not require an AC window.
  The SMAP negative test intentionally bypasses the helper.
- No current `with_user_access` closure invokes another `with_user_access` wrapper.

### 4. Before/after evidence

Both requested release/LTO trees built successfully with:

```text
product,browser-runtime,kernel-rng,kernel-sysstat,kernel-observability,
net-dial-cap,virtio-accel,framebuffer-console
```

I disassembled both kernels with the pinned toolchain's `llvm-objdump -d -M intel --demangle
--symbolize-operands`. There were 35 `stac ... clac` windows before and after. Normalizing addresses and
grouping by demangled function plus occurrence produced exactly seven changed windows and 28 identical
windows:

| Changed window | Before `57261d66` | Reviewed tip |
|---|---|---|
| `sys_futex_wait_impl` #1 | empty | futex-word load inside |
| `sys_net_stats_impl` #1 | empty | four output stores inside |
| `sys_readdir_impl` #1 | empty | cursor load inside |
| `sys_readdir_impl` #3 | empty | cursor output store inside |
| `sys_inspect_impl` #1 | initial output stores after `clac` | complete output stores inside |
| `sys_model_query_impl` #1 | initial/trailing stores after `clac` | complete output stores inside |
| `sys_tensor_query_impl` #1 | header/trailing stores after `clac` | complete output stores inside |

Every after-tree window contained its intended access. Control-flow inspection confirmed the access executes
before `clac` on the branch that executes `stac`; apparent duplicate copies belong only to the expected
SMAP-off branch reached by the preceding conditional jump.

I also built the qwen35 kernel configuration at its actual dev opt-level 2
(`neo-test,neo-test-arthur-real-brain-boot-qwen35`). Both trees contained 18 windows, with zero normalized
instruction-body differences. This independently confirms the documentation's qwen35 statement.

### 5. CP-40 and panic interactions

Confirmed both boundaries described by the dispatch.

- IRQ and timer assembly entry clears AC before Rust. An `x86-interrupt` exception taken inside an open
  window can still enter with AC=1. That is pre-existing CP-40 coverage debt, not introduced or widened by
  removing `nomem`; the fixed barrier correctly preserves the intended window until either the access
  completes or an exception transfers control. Fatal kernel-fault handlers do not resume through the
  interrupted closure.
- A panic before the trailing `clac` likewise leaves AC set, but both workspace profiles specify
  `panic = "abort"` (`Cargo.toml:63-69`). The kernel panic handlers terminate the test or halt rather than
  unwind and resume (`kernel/src/panic.rs:27-62`). It is therefore harmless under the current panic model.

### 6. CP-42 text

Every load-bearing factual claim checked out.

- **Latent:** normal CI and `neo-test` kernel builds use dev opt-level 0. The only scenario-specific override
  sets opt-level 2 for qwen35 (`tools/xtask/src/main.rs:3729-3731`), whose 18 windows are unchanged. The
  affected wide-feature release build is not a current gate.
- **SMAP scenarios:** `smap_catch` directly reads a user page and expects a fault
  (`test_harness.rs:8488-8504`); `smap_policy` checks policy/CR4 state (`test_harness.rs:8522`). Neither tests
  `with_user_access`, so passing them cannot discriminate this patch.
- **Historical CP-40 statement:** commit `c8b25dc2eac040ad42f0cb6dff44a4a01c2eb2a9` removed `nomem` from its
  `clear_rflags_ac` inline asm for the same ordering reason. The current entry stubs implement the AC clear
  directly in assembly.
- **Linux comparison:** at Linux master `58161ead29e50d70a317ea26b472e0c7508e69da`, raw
  [`arch/x86/include/asm/smap.h`](https://raw.githubusercontent.com/torvalds/linux/58161ead29e50d70a317ea26b472e0c7508e69da/arch/x86/include/asm/smap.h)
  routes `stac()`/`clac()` through `alternative()`, and raw
  [`arch/x86/include/asm/alternative.h`](https://raw.githubusercontent.com/torvalds/linux/58161ead29e50d70a317ea26b472e0c7508e69da/arch/x86/include/asm/alternative.h)
  gives that inline asm a `"memory"` clobber.

### 7. Additional adversarial checks

I checked conditional control flow around all seven changed windows rather than treating textual instruction
order as sufficient; none retained a user access outside the window on the SMAP-on path. I also checked for
partial output stores, volatile special-casing, pointer-provenance exceptions, current helper nesting, and
another AC-changing asm with an optimizer-relaxing option. No additional issue survived.

## Commands and hygiene

- Pinned compiler: `rustc 1.97.0-nightly (e8e4541ff 2026-04-15)`, LLVM 22.1.2.
- Four kernel builds passed: before/after release wide-feature builds and before/after qwen35 opt-level-2
  builds. Warnings were non-fatal and unrelated to CP-42.
- The microfixture's fixed and `nomem` controls were checked in emitted LLVM IR and assembly.
- The SMAP runtime scenarios were not used as evidence because, as confirmed above, they cannot discriminate
  this change.
- RAG health reported healthy (`ollama=true`, `qdrant=true`), but path-filtered queries returned no project
  results and unfiltered results were polluted by generated `graft/` documents. I therefore used direct
  source reads and exact searches for the load-bearing audit.
- All four build target directories were reclaimed with `cargo clean`; the temporary before-tree and
  microfixture artifacts were removed. The pre-existing untracked `graft/` directory was not touched.
