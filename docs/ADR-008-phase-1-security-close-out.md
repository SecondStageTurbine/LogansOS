# ADR-008: Phase 1 Security Debt Close-Out

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-05-04 |
| **Authority** | Project Lead |
| **Supersedes** | ADR-003 implementation gate |
| **Superseded by** | — |
| **Related** | Bible Article 12.3 · ADR-003 · Phase 1.5c BUILD-1/2/3 · commits `8847fd4`, `6591ab9`, `3aa07d1` |

---

## Context

ADR-003 named the Phase 1 security debt: SMEP, SMAP, STAC/CLAC discipline,
and real KASLR were mandatory by Bible Article 12.3 but not yet implemented.
Phase 1.5c was created to close that debt before Phase 2 capability and SMP
work starts.

BUILD-1 shipped the QEMU integration-test infrastructure and proved the runner
can catch deliberate failure. BUILD-2 enabled SMEP/SMAP in production and
added policy tests. BUILD-3 enabled executable-image KASLR with a PIE kernel
and added a default-suite KASLR smoke test.

This ADR records the close-out decision and the exact scope of the KASLR claim.

## Decision

### Q1 — SMEP/SMAP/STAC Closure

The SMEP/SMAP portion of ADR-003 is closed.

- `arch::enable_sse()` enables CR4.SMEP and CR4.SMAP when CPUID advertises
  support.
- `with_user_access` is the canonical bracket for deliberate direct user-VA
  access. It sets AC with STAC only around bounded copies and clears AC with
  CLAC before returning.
- `sys_write_impl` and `sys_spawn_impl` copy user buffers into kernel-owned
  memory under the bracket before parsing, serial output, or handle lookup.
- IPC rendezvous copies remain HHDM-mediated through `Pmap::translate` and
  `phys_to_virt`; they do not require AC because they do not dereference
  user-virtual addresses directly.
- The default `cargo neo-test` suite includes `smep_policy` and `smap_policy`,
  so future regressions that disable production CR4 policy fail CI.

### Q2 — KASLR Closure Scope

The KASLR portion of ADR-003 is closed for executable-image randomization.

- The kernel builds as PIE (`ET_DYN`) with `R_X86_64_RELATIVE` relocations.
- Limine's `ExecutableAddressRequest` is present and production boot logs the
  load-time executable physical and virtual bases to serial.
- The heap base derives from the relocated `__kernel_image_end` linker symbol,
  not from the old fixed `0xffff_ffff_8800_0000` virtual address.
- The paging sanity check probes relocated `_kernel_start` and uses the actual
  Limine HHDM offset.
- Production `boot/limine.conf` sets `kaslr: yes`.
- The default `cargo neo-test` suite includes `kaslr_smoke`, which fails if
  Limine reports the old fixed `0xffff_ffff_8000_0000` executable base.

**Scope limitation:** On the current BIOS/QEMU Limine setup, HHDM remains fixed
at `0xffff_8000_0000_0000`. BUILD-3 therefore provides executable-image KASLR
(`.text`, `.rodata`, `.data`, `.bss` slide), not full address-space
randomization. A leak of a kernel-image address still reveals the executable
slide; a leak of an HHDM address does not reveal the executable slide but does
reveal the fixed direct-map base.

This is still meaningful KASLR for code-reuse resistance, because ROP/JOP
gadget locations live in the executable image. It is not the strongest possible
KASLR. Phase 2 hardware bring-up should revisit HHDM behavior on real UEFI
hardware and newer Limine configurations.

### Q3 — CI Gate

The security close-out is mechanically gated by the default integration suite:

- `ipc_heap_kill_smoke` proves existing userspace IPC/heap/kill behavior still
  composes with SMEP, SMAP, STAC/CLAC, PIE, and KASLR.
- `smep_catch` and `smap_catch` prove the hardware mechanisms trap the expected
  synthetic violations.
- `smep_policy`, `smap_policy`, and `kaslr_smoke` prove production policy stays
  enabled.
- `expect_pass_but_fail` remains the meta-test proving the runner is not
  silently green.

## Consequences

### Positive

- Bible Article 12.3's Phase 1 SMEP, SMAP, KASLR, NX, and W^X requirements are
  now honored to the explicitly documented scope.
- Phase 2 capability work starts with hardware enforcement for accidental
  user-code execution and user-pointer dereference bugs.
- The KASLR implementation is falsifiable at the binary level (`readelf` shows
  PIE + relocations) and at runtime (`kaslr_smoke` checks Limine's reported
  executable base).

### Negative

- HHDM fixedness remains a known limitation on the current boot path.
- Serial debug output still includes kernel virtual addresses. Serial is not
  user-accessible in Phase 1.5c, so this is acceptable for development, but a
  future user-readable diagnostics surface must redact kernel-image addresses.
- UMIP, CET shadow stacks, and LAM are named by Bible Article 12.3 but were not
  part of ADR-003's Phase 1 close-out bundle. Phase 2 CPU-feature policy should
  audit them explicitly.

## Verification

- `docker compose run --rm neo cargo k` — PASS.
- `docker compose run --rm neo cargo neo-test kaslr_smoke --no-meta` — PASS.
- `readelf` check — PASS; kernel reports `DYN (Position-Independent Executable
  file)` and `R_X86_64_RELATIVE` relocations.
- `docker compose run --rm neo cargo neo-test` — PASS; covers IPC, SMEP/SMAP
  catch tests, SMEP/SMAP policy tests, KASLR smoke, and deliberate-failure
  meta-test.
- `docker compose run --rm neo bash -lc 'cargo iso && timeout 60 cargo neo-tty; code=$?; if [ $code -eq 124 ]; then exit 0; else exit $code; fi'` — PASS under committed `kaslr: yes`.

## Notes

ADR-003 remains useful as the historical deferral record. This ADR is the
close-out artifact: future work should cite ADR-008 when relying on the Phase 1
security baseline.

---

**Decision recorded.** Per Bible Article 27, this ADR is amendable by
superseding ADR.
