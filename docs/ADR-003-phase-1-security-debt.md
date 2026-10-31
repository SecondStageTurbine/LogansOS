# ADR-003: Phase 1 Security Debt — KASLR, SMEP, SMAP, STAC/CLAC

| | |
|---|---|
| **Status** | Superseded |
| **Date** | 2026-05-02 |
| **Authority** | Project Lead |
| **Supersedes** | — |
| **Superseded by** | ADR-008 |
| **Related** | Bible Article 12.3 · ADR-001 (governance precedent) · `boot/limine.conf` (deferred KASLR flag) |

---

## Context

Bible Article 12.3 states:

> SMEP, SMAP, NX, KASLR, and W^X are mandatory from Phase 1. UMIP, CET shadow stacks, and LAM are enabled where the CPU supports them.

As of Phase 1.4 close (commit `aa2c949`), NeoPlutarch has shipped:

- ✅ **NX** — `arch/x86_64/enable_sse()` does not disable NX; the page-table mapper installs `PageTableFlags::NO_EXECUTE` on user data pages and on intermediate kernel mappings where appropriate. NX is honored.
- ✅ **W^X** — added during Phase 1.4 /simplify cleanup. `kernel/src/elf.rs::Elf::load_into` rejects PT_LOAD segments with both PF_W and PF_X bits set, returning `ElfError::SegmentWriteAndExecute`.
- ❌ **KASLR** — `boot/limine.conf` had `kaslr: no` until a botched 1.4.1 attempt set it to `yes`. Verification across three boots showed identical kernel virt, kernel phys, PER_CPU, HHDM offset, and module load addresses — the flag was a **no-op on that setup**. Real Limine KASLR requires kernel-as-PIE + an executable-address request/response in the limine requests block + an audit of every hardcoded virt address in the kernel (heap base `0xffff_ffff_8800_0000`, user stack top `0x0000_7fff_ffff_f000`, kernel `.text` base `0xffff_ffff_8000_0000`, etc.). Setting the flag without those pieces is a toy. Reverted.
- ❌ **SMEP** (Supervisor Mode Execution Prevention) — CR4.SMEP (bit 20) is never set. Without SMEP, the kernel can fetch and execute instructions from user-mapped pages. A kernel bug that jumps into user-mapped code becomes a CVE class instead of a trap.
- ❌ **SMAP** (Supervisor Mode Access Prevention) — CR4.SMAP (bit 21) is never set. Without SMAP, the kernel can read or write user-mapped pages without explicit consent. The current `sys_write_impl` reads the user buffer via `core::slice::from_raw_parts(buf, len)` directly. With SMAP enabled, this would #PF unless wrapped in `stac` / `clac`.
- ❌ **STAC/CLAC** discipline — required when SMAP is enabled. Every kernel path that deliberately accesses user pointers must be bracketed by `stac` (allow user access) and `clac` (deny again). Not yet implemented because SMAP is off.

This is real security debt. Each missing item is a documented Bible-mandated invariant we are not honoring.

The Bible-discipline question is whether to (a) fix all four immediately as a 1.4.1 patch, (b) accept the deviation indefinitely, or (c) defer with a binding lock-in.

This ADR chooses (c).

## Decision

**KASLR, SMEP, SMAP, and STAC/CLAC are deferred to Phase 2 entry. They are blocking gates for the Phase 2 capability system PR; Phase 2 cannot land without them.**

Concretely:

1. **Phase 2 entry is gated on a "Phase 1 security debt close-out" PR** that lands SMEP enable, SMAP enable, STAC/CLAC bracketing of every user-pointer access path in the kernel, and KASLR with all the prerequisite work (kernel-as-PIE, `ExecutableAddressRequest` in the limine 0.6 crate, hardcoded-virt-addr audit, dynamic heap base, dynamic user stack base). That PR may be split into per-feature commits but lands as one Phase 1.4.x phase.

2. **No new kernel feature work proceeds beyond Phase 1.5 until item 1 is complete.** Phase 1.5 (multiple userspace processes, basic IPC, userspace heap, signals) does not depend on these security features and can land first; Phase 2 (capabilities, SMP, allocators) does depend.

3. **A failing CI check is added** that fails the build if `boot/limine.conf` is committed with `kaslr: yes` while no executable-address request exists in `kernel/src/main.rs`. This prevents the "set the flag, ship the toy" mistake from being repeated.

4. **A failing CI check is added** that fails the build if `arch/x86_64/enable_sse()` (or its successor) does not set CR4.SMEP and CR4.SMAP, gated by a feature-detection module. This prevents silent regression after the close-out PR lands.

## Phase 1.5c BUILD-2 Amendment

Phase 1.5c splits the original "all four at once" close-out plan:
SMEP/SMAP/STAC land in BUILD-2, while KASLR remains BUILD-3. This supersedes
the bundling preference in this ADR but not the no-toy KASLR rule. The reason
is concrete: BUILD-1 landed a real QEMU integration harness with deliberate
red-test verification, so SMEP/SMAP can now be enabled behind production policy
tests instead of relying on prose review. KASLR still requires kernel-as-PIE,
a Limine executable-address request, and hardcoded-address verification;
flipping `kaslr: yes` alone remains rejected.

## Phase 1.5c BUILD-3 Amendment

BUILD-3 closes the KASLR portion of this ADR. The kernel is now linked as a
PIE (`ET_DYN` with `R_X86_64_RELATIVE` relocations), `main.rs` adds Limine's
`ExecutableAddressRequest`, the heap base derives from the relocated kernel
image end, the paging sanity check uses the relocated `_kernel_start` address
and the actual HHDM offset, and production `boot/limine.conf` has `kaslr: yes`.

The CI gate is `kaslr_smoke`: it boots with KASLR enabled, requires an
executable-address response, and fails if the executable virtual base remains
the old fixed `0xffff_ffff_8000_0000` link base. On the current BIOS/QEMU
Limine setup, the executable base randomizes while HHDM remains at
`0xffff_8000_0000_0000`; BUILD-3 treats HHDM fixedness as a documented
bootloader property, not as proof that the executable slide failed.

## Consequences

### Positive

- **The deviation is named and time-boxed.** Future contributors reading the Bible see Article 12.3, see this ADR, see the Phase 2 entry gate, and know exactly when the debt closes.
- **No toy code shipped.** BUILD-3 enables KASLR only after the kernel is PIE-linked, requests Limine's executable address response, removes fixed kernel-image-dependent virtual addresses from heap and paging checks, and adds a CI smoke that fails on the old fixed executable base.
- **Phase 1.5 can proceed.** Multi-process userspace doesn't depend on SMEP/SMAP/KASLR. We can keep building the substrate while the security work is scoped.
- **The CI gates** make recurrence impossible without explicit override.

### Negative

- **The kernel is genuinely less secure than the Bible mandates** until the close-out PR lands. A kernel bug in Phase 1.5 — e.g., during multi-process work — that jumps into user code or follows a malicious user pointer is exploitable in a way it would not be with SMEP/SMAP. We accept this risk for the duration of Phase 1.5 only.
- **Mitigation:** Phase 1.5 PRs are reviewed with extra attention to user-pointer handling. Every `*const u8 / *mut u8` from a syscall arg gets a comment naming the validation. The current `sys_write_impl::USER_HALF_LIMIT` check is the pattern.
- **The "Bible-discipline" claim is weakened.** Article 12.3 says "mandatory from Phase 1." We are at Phase 1.4 and have not honored it. A future contributor may reasonably ask "if Article 12.3 can be deferred, what else can?" The answer must be: nothing else can be deferred without an ADR like this one. The cost of the deferral is the cost of writing this ADR. If that cost is not paid, the deferral is not legitimate.

### Neutral

- **The W^X portion of Article 12.3 is honored.** The ELF loader's W^X check is real, in production, and tested. So this ADR is a partial deferral, not a wholesale one.
- **NX is honored.** Same.

## Alternatives Considered

### Alternative 1: Land all four items as Phase 1.4.1 immediately

Rejected as currently scoped, with caveats. SMEP and SMAP-without-STAC are each ~10 LOC and could be done now. STAC/CLAC bracketing is ~5 LOC per user-pointer access path; we currently have one (`sys_write_impl`). KASLR is the heavy item — easily 200 LOC across kernel, linker.ld, and limine config, plus an audit of every hardcoded virt address.

The argument against doing the cheap subset (SMEP + SMAP + STAC) now: doing it without KASLR creates an incoherent security posture ("we're partially secure"), and the close-out PR is cleaner if all four land together with their CI gates as one reviewable unit.

The argument for doing the cheap subset now: SMEP alone is a real CVE-class fix and waiting for the perfect bundle is the perfect being the enemy of the good.

This ADR rejects the cheap-subset path because the discipline of the close-out PR is itself the point. If we do SMEP+SMAP now and KASLR later, we've split the debt rather than closing it. The Phase 2 gate exists to force the full close-out.

If a real exploit emerges in Phase 1.5 testing that SMEP would have caught, this ADR is superseded by an emergency ADR that lands SMEP earlier.

### Alternative 2: Amend Bible Article 12.3 to "mandatory from Phase 2"

Rejected. Article 12.3 is correct as written. The work is real, the security justification is real, and amending the Bible to match the implementation's deficiencies is the wrong direction for governance — see ADR-001 Alternative 3 for the same argument applied to a different deviation.

### Alternative 3: Accept the deviation indefinitely with no close-out plan

Rejected. This is the "drift" mode that ADR-001 Alternative 3 explicitly named as the reason ADRs exist. Either the debt is closed by a date or it is permanent; permanent deviation from Article 12.3 makes Article 12.3 not law.

### Alternative 4: Implement KASLR via a custom solution rather than Limine's

Rejected. Limine's KASLR mechanism is well-documented and standard. Rolling our own would mean reimplementing a randomization scheme, an entropy source, and the kernel-side relocation logic Limine handles for free. The cost is wildly higher than the integration cost. ADR-able if Limine ever proves limiting.

## Implementation

The close-out PR lands at Phase 2 entry. Its scope:

- [x] **SMEP enable.** Probe via `cpuid` leaf 7 EBX bit 7. If supported, set CR4.SMEP (bit 20) during early x86_64 CPU bring-up. BUILD-2 verifies this with `smep_policy`.
- [x] **SMAP enable.** Probe via `cpuid` leaf 7 EBX bit 20. If supported, set CR4.SMAP (bit 21). BUILD-2 verifies this with `smap_policy`.
- [x] **STAC/CLAC bracketing.** Wrap every kernel access of a user pointer:
  - `sys_write_impl` user-buffer read → copy bounded chunks under `with_user_access`, then write serial with AC clear.
  - `sys_spawn_impl` process-name and inherited-handle array reads → copy to kernel-owned buffers under `with_user_access` before UTF-8 parsing or handle lookup.
  - `sys_send_impl` / `sys_recv_impl` rendezvous copies remain HHDM-mediated via `Pmap::translate` + `phys_to_virt`, so SMAP does not apply to that path.
  - Any future `sys_read_impl`, `sys_tensor_write_impl`, etc. → same pattern.
  - A helper `with_user_access<T, F: FnOnce() -> T>(f: F) -> T` in `arch::x86_64` does `stac; let r = f(); clac; r` when SMAP is active.
- [x] **KASLR enable.**
  - Kernel target uses `relocation-model=pic` and `-pie`; `readelf` reports `ET_DYN` and `R_X86_64_RELATIVE` relocations.
  - `ExecutableAddressRequest` is present in `.requests` and serial logs emit the Limine-reported physical and virtual executable bases.
  - Hardcoded kernel-image-dependent virtual addresses were removed: heap placement derives from `__kernel_image_end`, and paging sanity checks `_kernel_start` at its relocated address. User stack and user heap constants remain lower-half user-space policy and are unaffected.
  - `boot/limine.conf` is `kaslr: yes`.
  - Verification shows executable virtual base randomization; HHDM remains fixed on current Limine/QEMU and is documented as outside the executable-slide claim.
- [x] **KASLR CI gate** added per Decision item 3. Default `cargo neo-test`
  includes `kaslr_smoke`, which fails if Limine reports the old fixed
  executable virtual base.
- [x] **SMEP/SMAP CI gate** added per Decision item 4. Default `cargo neo-test`
  includes `smep_policy` and `smap_policy`, and GitHub Actions runs that suite
  on PR/push.
- [ ] **Bible Article 12.3** is verified satisfied; this ADR is superseded by the close-out ADR.

## Notes

This ADR is shorter than ADR-001 because it follows ADR-001's format precedent and because the decision (defer with a binding gate) is conceptually simple. The complexity is in the Implementation section, which lists the actual work.

The CI gates are the load-bearing discipline. Without them, "Phase 2 entry is gated" is just a comment in a markdown file. With them, the build literally fails until the work lands.

If the close-out PR is not landed by Phase 2 entry — i.e., someone tries to land Phase 2 work first — the correct response is to reject that PR until ADR-003's items are complete. This is the same discipline ADR-001 invokes for governance: the bible is law; ADRs are law via the bible's amendment mechanism.

---

**Decision recorded.** Per Bible Article 27, this ADR is amendable by superseding ADR. The expected superseder is the Phase 2 entry close-out ADR.
