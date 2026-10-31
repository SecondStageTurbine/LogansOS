# NeoPlutarch Integration Tests

Phase 1.5c BUILD-1 establishes the QEMU integration-test harness required by
Bible Article 16.2. BUILD-2 extends that harness with production policy checks
for SMEP and SMAP. BUILD-3 adds the KASLR production policy smoke. Tests are
launched through:

```powershell
docker compose run --rm neo cargo neo-test
```

The runner builds one test-mode kernel per scenario, boots it in QEMU, captures
COM1 serial output to `target/neo-test/<scenario>.serial.log`, and requires
both a structured terminal frame and the expected QEMU `isa-debug-exit` status.
The IPC scenario also requires `neo-init` to exit with code 0 so semantic
userland failures cannot pass on a clean frame leak audit alone.

## Serial Frame ABI

The serial frame format is stable from BUILD-1 onward:

```text
NEO_TEST/1 event=<KIND> name=<scenario> [k=v ...]
```

Rules:

- `NEO_TEST/1` is the versioned frame prefix and must not change without a
  deliberate compatibility decision.
- `event=BEGIN` marks scenario dispatch.
- `event=PASS` and `event=FAIL` are terminal events; exactly one terminal event
  is required per scenario boot.
- `name=<scenario>` must match the xtask scenario name.
- Extra fields are space-separated `k=v` tokens with no embedded whitespace.

## BUILD-1 Scenarios

- `ipc_heap_kill_smoke` runs the existing Phase 1.5b userland smoke: IPC,
  userspace heap, ProcessHandle kill paths, and frame leak audit.
- `smep_catch` enables SMEP in a test-mode kernel and proves a ring-0 fetch from
  a user page becomes an expected #PF.
- `smap_catch` enables SMAP in a test-mode kernel and proves a ring-0 read from
  a user page with AC clear becomes an expected #PF.
- `expect_pass_but_fail` is ignored by the default passing suite. The default
  `cargo neo-test` command runs it only as a meta-test and requires the runner
  to report it as `FAIL`, proving the gate catches red tests instead of merely
  observing a green command.

## BUILD-2 Scenarios

- `smep_policy` proves normal kernel init enables CR4.SMEP when CPUID advertises
  SMEP.
- `smap_policy` proves normal kernel init enables CR4.SMAP when CPUID advertises
  SMAP.

The BUILD-1 catch tests remain mechanism tests. The BUILD-2 policy tests are
the CI gate: a future regression that removes production CR4 enablement fails
the default `cargo neo-test` suite even if the synthetic catch tests still work.

## BUILD-3 Scenario

- `kaslr_smoke` boots a test-mode ISO with Limine `kaslr: yes`, requires
  Limine's executable-address response, and fails if the executable virtual
  base is still the old fixed `0xffff_ffff_8000_0000` link base. The PASS frame
  includes `kaslr=on virt_base=<addr> phys_base=<addr>` for audit logs.
