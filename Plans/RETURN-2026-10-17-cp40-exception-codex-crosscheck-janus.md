<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: CP-40 exception-entry cross-family review — Janus

## Scope and verdict

**Verdict: MERGE-WITH-NITS.** The exception shims close the AC/SMAP gap without
changing the CPU-delivered exception frame or error-code layout. Both targeted
QEMU discriminators pass at `NEO_SMP=4`. I found one P2 documentation and
future-maintenance omission; it does not affect any current returning path.

- Reviewed head: `3ff45f31b1573c08769d736d0b43dc146708fe7f`.
- Stacked base: `0135fb55ba84e5c9116ef84c4d8a0e1ce7c691a8`.
- Review branch: `codex/cp40x-crosscheck-janus` (the name in the user dispatch).
- RAG health was healthy, but the exception-entry query returned no results;
  its index had not incorporated this branch. I used direct commit reads and
  Graft's refreshed exact-reference graph.
- Review only: this RETURN contains no production or test-code change.

## Finding P2 — returning-handler rules omit GS and newer xstate

**Locations:** `kernel/src/arch/x86_64/idt.rs:50-59` and
`docs/concurrency-pitfalls.md:1532`.

The rule correctly says the exception path does no `swapgs`, but its list of
constraints does not state the consequence. Gate DPL controls software
`int n`; it does not prevent a hardware exception from CPL3. Any future handler
that returns after a ring-3 #DB/#PF/etc. must perform a saved-CS-qualified,
balanced `swapgs` before using GS-relative per-CPU state and swap back before
`iretq`, or enter through an equivalent common wrapper. Today this is harmless:
`breakpoint`, the only returning handler, is reachable only from ring 0 because
its gate is DPL0, while the ring-3 page-fault kill path performs one explicit
`swapgs` and diverges.

The SIMD rule says “AVX (any VEX-encoded instruction)” but does not name
EVEX-encoded AVX-512 state: upper ZMM lanes, ZMM16-31 and opmask registers are
also outside the observed `xmm0-15` prologue save. The present target does not
enable AVX-512 and the current returning handler reaches none, so this is a
future-proofing/doc correction rather than a runtime defect. State the rule as
“no instruction that writes architectural SIMD/xstate beyond XMM0-15,” with
VEX and EVEX examples, and add the balanced-GS requirement.

## Numbered checklist

### 1. Shim versus the `x86-interrupt` ABI — no correctness finding

`set_exception_entry!` emits a naked, stack-neutral tail shim. Its
`pushfq; and [rsp], -263169; popfq` changes only live AC/DF, then `jmp`s to the
typed handler. It does not insert a return address or consume the CPU frame, so
no-error handlers see the original frame and #DF/#TS/#NP/#SS/#GP/#PF/#AC see
their original error-code slot. Passing the same handler identifier both as
the generic `Entry<F>` type witness and as `sym` prevents error-code/signature
class mismatches and wrong-target drift within the macro invocation.

The immediate is `0xfffffffffffbfbff`, exactly `~(AC|DF)`. Interrupt-gate entry
has live IF=0. Architectural delivery has already cleared live TF/RF/NT, and
the mask preserves them; `popfq` cannot reopen IF. The hardware-saved RFLAGS
remain untouched, demonstrated by the breakpoint leg restoring caller AC/DF.

#DF and NMI retain their IST indices. A nested NMI/#MC between the shim's push
and pop builds its own frame below the temporary word and cannot confuse the
outer handler's layout. Current NMI/#MC/#DF handlers never return. A future
returning nested handler would unwind its own frame before the outer pop.

### 2. Audit table — claims survive review

The live-IDT walk observed exactly 18 present exception gates, all DPL0, all
starting with the expected ten mask bytes, with NMI and #DF on their expected
IST slots. DPL0 makes a CPL3 software `int3`/`int n` fail privilege checking;
hardware-generated exceptions ignore gate DPL. INTO is invalid in 64-bit mode,
while BOUND remains a compatibility-mode source. The configured 32-bit user
code segment makes the documented #BR reachability real.

`enable_sse` clears CR0.EM and now CR0.AM; the codebase never sets CR0.TS.
Calls occur in BSP `_kernel_start` and AP `ap_entry`, so #NM/#AC are not normal
user-triggered paths. Vectors 9, 15 and 20-31 remain absent. Of the ring-3
reachable handlers, all halt except browser-runtime #PF, whose kill path
explicitly swaps to kernel GS and ends in a scheduler exit. Before this fix,
that switch could carry live AC because the context shims do not save RFLAGS;
the inference is correct even though the leak itself is not a runtime oracle.

### 3. AC leaks outside exception entry — bounded existing gap

I exhaustively inspected the 23 direct `with_user_access` call sites. Their
closures perform bounded copies, scalar reads/writes or one volatile futex
value check; the futex parks only after that closure returns and `clac` runs.
No current closure blocks, yields or switches. IRQ/exception entry also clears
live AC while preserving the interrupted frame, so an interrupt-driven switch
cannot hand the window to another thread. A future blocking/yielding closure
would still leak AC because `context_switch` does not normalize RFLAGS; CP-40
documents this accurately and does not claim enforcement.

### 4. Returning-handler rules — implementation safe; see P2

The current breakpoint path uses the compiler's GPR/XMM saves and reaches no
AVX, x87 or floating-point SSE. It is ring-0-only, so kernel GS is already
active. MXCSR and x87 control/status remain owned by the interrupted context;
the stated no-floating-point rule is therefore necessary. P2 records the two
missing future constraints: CPL3 GS handling and EVEX/AVX-512 state.

### 5. Discriminator vacuity — no finding

`exception_entry_flags` sets AC/DF and XMM patterns in one asm block, samples
caller flags immediately after `iretq`, and checks the handler's live-AC
counter delta. Its SIDT leg reads the active table rather than the Rust
initializer, checks every present vector 0-31, and separately checks DPL and
IST. The exact byte oracle includes both AC and DF mask bits.

The ring-3 guard-fault leg requires `pf_frame_ac > 0` on a successful scenario,
so live-AC zero cannot pass merely because the probe never armed or #PF never
ran. The handler increments saved-frame AC before sampling live AC, and the
harness reads after the process teardown protocol completes. On x86, the
locked relaxed RMWs are globally visible; the successful frame counter plus
completed IPC/process path makes a stale-zero live counter an implausible and
architecturally excluded pass. Stronger ordering would not improve this x86
discriminator.

### 6. Additional adversarial checks

- Installing via `set_handler_addr` resets options, but the code reapplies
  #DF/NMI IST and the live test proves the resulting descriptor values.
- The shim's first stack write adds only eight transient bytes; it has no call,
  alignment or red-zone dependency. An already unusable ordinary stack would
  fault before any Rust prologue too, while #DF uses its independent IST.
- Clearing CR0.AM deliberately prevents user alignment-check delivery but does
  not weaken SMAP: RFLAGS.AC still controls supervisor SMAP access. Both BSP
  and AP initialization apply the policy.
- The AAG's pre-existing SYSCALL NT/SFMASK and CSTAR notes are outside this
  branch and do not arise from the exception shim.

## Validation

- `NEO_SMP=4 cargo xtask neo-test exception_entry_flags` in the repository
  dev container: **PASS** — 64 rounds, 64 breakpoint runs, AC cleared, caller
  flags restored, XMM0-15 preserved, 18 gates all masked, IST/DPL correct,
  SMAP enabled.
- `NEO_SMP=4 cargo xtask neo-test threads_guard_fault` in the same container:
  **PASS** — ring-3 guard fault contained and `user_pf_ac=cleared`.
- The host build also compiled `exception_entry_flags`; its host QEMU launch
  could not assemble an ISO because `/home/neo/limine` is absent, so the
  Limine-equipped container supplied the runtime result.
- `git diff --check` against the stacked PR #15 base: clean.
