<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: CP-47/CP-48 syscall-MSR cross-check (Janus/Codex)

**Verdict:** MERGE-WITH-NITS

**Reviewed tree:** `origin/main` at `1a840e54b1fb80aa626462867223aa69b6ece83a` (the CP-47/48 merge is `4fa909621512f8d57cc98bbcc7a13f75fa7ea1fa`).

The NT and compatibility-mode closures are sound. One low-severity stale comment survived the CP-46/CP-47 rebase; it does not affect code or either discriminator.

## Finding

### [LOW] CP-47/48 trap-hook comment still describes the pre-CP-46 misaligned ABI

**Location:** `kernel/src/test_harness.rs:8202-8207`

The comment says all three error-code vectors still use `x86-interrupt` bodies that call with RSP eight bytes off SysV alignment. On the reviewed tree, CP-46's `set_error_code_entry!` shims realign all three before calling this hook. The enum is still a good design and the paragraph accurately describes the historical failure that motivated it, but it should be written in the past tense and identify CP-46 as the repair. No runtime behavior is affected.

## Checks against the dispatch

### 1. NT elsewhere

- `write_syscall_msrs` includes `RFlags::NESTED_TASK` in IA32_FMASK before any syscall is enabled for use (`kernel/src/arch/x86_64/syscall.rs:504-517`). It runs on the BSP and every AP through the CPU-local trap-init path.
- A repository-wide audit found no kernel load of a user-supplied RFLAGS value. The initial user IRETQ frames use constant `USER_RFLAGS_RAW`; syscall IRETQ frames retain user R11 as intended, but IRETQ tests the current (masked) NT before restoring the user image. The only kernel `popfq` sites are the exception/IRQ/timer AC+DF masks, which snapshot and reload the already post-delivery live flags.
- Every installed IDT entry retains the crate's default interrupt-gate setting; there is no `set_disable_interrupts(false)`/trap-gate conversion. Interrupt/trap-gate delivery clears live NT architecturally, so IRQs, timer, NMI and exception entries do not inherit user NT. Cooperative switching still does not save RFLAGS, but the only user-controlled non-gate entry, SYSCALL, now clears NT before it can switch.
- Residual hardening note: the first kernel context still assumes the bootloader did not hand off NT=1. That is not user-controlled and predates this change; explicitly clearing NT during early entry would remove the assumption.

### 2. Compatibility-mode closure

- The complete live GDT construction has only: null slot 0, ring-0 64-bit code, ring-0 data, zero slot 3, ring-3 data, ring-3 64-bit code, and a two-slot ring-0 TSS (`gdt.rs:130-136`). There is no alternate 32-bit/conforming user code descriptor or call gate.
- `lldt 0` executes after each CPU loads its GDT/TSS (`gdt.rs:148-165`), and no later `lldt` exists. Ring 3 cannot install or load an LDT. Crafted far-jump, far-return or IRETQ selectors therefore have only the live GDT, where `0x1B` is null and `0x2B` is the sole user code segment.
- Keeping the zero hole preserves `0x23`/`0x2B` and STAR's user-base arithmetic. `Star::write` still validates the selector ordering, and `runtime_assert_gdt_selectors` pins the hard-coded syscall-frame selectors. The kernel does not use SYSRET, but SYSRETQ's derived user data/code selectors would still be `0x23`/`0x2B`; the null base slot is not loaded by the 64-bit form.
- Exact-symbol searches found no load or semantic use of selector `0x18`/`0x1B` outside the negative discriminator and documentation.

### 3. CSTAR sink and SYSENTER

- Fresh disassembly is exactly `cli; hlt; jmp` and has no implicit or explicit stack/memory operand. IA32_FMASK already clears IF, while `cli` makes the sink independently non-interruptible after entry.
- NMI uses its dedicated IST, so it does not consume the compat caller's RSP. SMI saves state in SMM/SMRAM and resumes the halt loop. `#MC` has no IST in this kernel and would use the current RSP; therefore the sink is stack-free in normal execution but not isolated from a coincident machine check. The #MC body halts anyway, so this does not reopen a ring-3 path or make the fail-closed sink a merge blocker.
- The sink deliberately parks one CPU and can later turn a shootdown into a kernel panic. Given that both the GDT and LDTR layers must already have failed before CSTAR is reachable, the tiny stack-free sink is a defensible last-resort containment choice. A process-killing compat entry would require safely establishing kernel GS/stack and saving state on a path the system otherwise forbids.
- IA32_SYSENTER_CS is written as zero on every CPU. Intel documents SYSENTER from both 64-bit and compatibility user code and requires a non-null selector; zero therefore closes the independent SYSENTER_EIP/ESP route. AMD's compatibility SYSCALL target is CSTAR; Intel documents compatibility-mode SYSCALL as #UD.

### 4. Discriminator vacuity

- Fresh `syscall_nt_masked` disassembly confirms each leg executes `push 0x4202; popfq; pushfq/pop rax; mov` setup and then `syscall`; no flag-changing instruction occurs between the captured pre-flags and entry. It checks a real ENOSYS return and two yields, exact user flag restoration, a Rust-entry NT counter, and harness NT.
- The dispatch counter is before syscall-number dispatch/yield. A missing SFMASK bit faults at the syscall's IRETQ; a wrong-place exit-only clear is caught by the counter before the yields. The #GP/#DF hook provides deterministic fail-fast coverage if return faults first.
- The GDT walk uses `limit+1`, scans all complete eight-byte slots, catches any present DPL3 system descriptor, catches any 32-bit DPL3 or conforming code segment, and skips the upper half of each present 16-byte system descriptor. Its current eight-slot table is fully covered.
- Fresh compat-probe disassembly decodes the critical bytes as `ljmp m16:32` with a six-byte pointer containing selector `0x1B`, followed at the target by `syscall`; this is not an assembler-width accident. The expected #GP error is the selector with RPL cleared (`0x18`) and the hook additionally requires the exact jump RIP, ring-3 CS, armed marker, failed LAR checks, clean GDT bitmap, CSTAR=sink and unusable SYSENTER_CS.
- `PROBE_DATA` is release-published before the thread is queued and acquire-read in the trap hook. `syscall_msr_trap` handles all relevant GS states: ring-3 #GP, post-exit-swapgs ring-0 #GP and CSTAR/RIP-0 faults have a low live GS and swap; a CP-44 page-fault body that already swapped has the kernel high-half GS and does not swap twice. Its global re-entry guard terminates without taking the serial lock again.

### 5. Vendor behavior and the CP-46 side finding

- Current Intel SDM material states compatibility-mode SYSCALL is #UD, while IA32_CSTAR remains an architectural MSR when SYSCALL/SYSRET is supported. AMD documentation defines CSTAR as the compatibility-mode SYSCALL target. Thus removing compatibility mode is the portable primary fix, while programming CSTAR is useful AMD/QEMU defense in depth and harmless on Intel implementations advertising the feature.
- Intel documents SYSENTER in IA-32e compatibility and 64-bit modes, confirming that zeroing SYSENTER_CS closes a distinct path rather than merely testing dead state.
- I agree with the original CP-46 measurement: the pinned-toolchain disassembly and the timed-futex path substantiate the extra-error-code alignment failure. Keeping its repair separate from the MSR/GDT change was appropriate. On this reviewed merged tree CP-46 is already repaired; only the stale hook comment above still says otherwise.

## Verification performed

- RAG health: healthy (`ollama=true`, `qdrant=true`); used for conceptual discovery, then verified by direct full-file reads and exact-symbol searches.
- `NEO_SMP=4 NEO_TEST_ATTEMPTS=1 cargo xtask neo-test syscall_nt_masked`: scenario build PASS; QEMU packaging unavailable because xtask requires `/home/neo/limine` on this host.
- `NEO_SMP=4 NEO_TEST_ATTEMPTS=1 cargo xtask neo-test compat_mode_unreachable`: scenario build PASS; same Limine-only execution limitation.
- Fresh `llvm-nm`/`llvm-objdump`: verified the NT probe instruction ordering, `compat_syscall_sink`, LAR operations, `ljmp m16:32` pointer width/selector, and compat `syscall` target.
- Vendor cross-check: Intel SDM (SYSCALL compatibility #UD; SYSENTER valid in IA-32e 64/compat modes) and AMD BKDG/APM material (CSTAR compatibility target; FMASK semantics).
