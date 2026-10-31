# RESEARCH: Phase 2 4-CPU tier without BIOS changes

- **Date:** 2026-05-25
- **Author:** PM
- **Question:** Can the Phase 2 physical 4-CPU hardware tier be executed without changing OptiPlex BIOS settings?
- **Status:** Researched; existing no-code path not found

---

## 1. Finding

With the current NeoPlutarch source and current Limine configuration, there is no confirmed no-BIOS/no-code way to boot the physical OptiPlex scenario ISOs as a 4-CPU tier.

The practical answer is:

- **without BIOS and without NeoPlutarch source/tooling changes:** no;
- **without BIOS but with a narrow NeoPlutarch change:** yes, by adding a physical-media CPU limit consumed before AP bring-up;
- **without source changes but with BIOS/firmware changes:** maybe, if firmware can expose only 4 logical CPUs.

---

## 2. Local Source Evidence

Current repository state inspected: `2f1aa14c308640484749a17e05b66c2e0c548e96`.

Findings:

- `NEO_SMP` is documented and used as a QEMU `-smp` value only in `tools/xtask/src/main.rs`.
- The ISO assembly path stages the kernel, modules, and `boot/limine.conf`; it does not encode a CPU-count limit into scenario media.
- `write_limine_conf()` only rewrites `kaslr:` when requested. It does not add or rewrite a `cmdline:` value.
- `boot/limine.conf` has no `cmdline:` entry and no CPU-count setting.
- `kernel/src/arch/x86_64/smp.rs::bring_up_aps()` iterates over every CPU returned by Limine, skipping only the BSP and CPUs beyond `per_cpu::MAX_CPUS`.
- Current source search found no kernel boot-argument parser.
- RAG semantic search for boot command-line / CPU-limit handling returned no relevant source hits.

Consequence: current hardware scenario ISOs boot full-count on the OptiPlex. They cannot honestly be labeled as the 4-CPU tier.

---

## 3. Limine Evidence

Primary Limine documentation confirms `cmdline` exists as a generic local option and `kernel_cmdline` is an alias. It does not list a CPU-limit / SMP-limit / maxcpus option for Limine protocol entries.

The current Limine `CONFIG.md` option list also has no hits for `smp`, `maxcpus`, or processor-count controls.

The local `limine 0.6.3` Rust crate exposes:

- `ExecutableCmdlineRequest`, which NeoPlutarch could use to read a future `cmdline: neo.cpu_limit=4`;
- `MpRequest`, whose x86_64 request flag surface only exposes `MP_FLAG_X2APIC`;
- `MpRespData::cpus()`, which returns the processor list Limine reports.

Consequence: Limine can pass a string to NeoPlutarch, but Limine itself is not the CPU limiter for this path. NeoPlutarch must interpret the string and choose how many APs to bootstrap.

---

## 4. Viable Non-BIOS Design

The narrow implementation would be:

1. Add a Limine executable command-line request in the kernel.
2. Add a tiny parser for a key such as `neo.cpu_limit=4`.
3. Extend scenario ISO building so a CPU-limit setting can be injected into `limine.conf`.
4. In `smp::bring_up_aps()`, cap AP bootstrap after `limit - 1` APs.
5. Emit explicit serial evidence:

```text
[ kernel ] smp: Limine reports 12 cpu(s); bsp_lapic_id=0
[ kernel ] smp: cpu_limit=4 active
[ kernel ] smp: started 3 AP(s)
```

Acceptance proof for the 4-CPU tier should be:

- the log shows the physical OptiPlex still has 12 Limine-reported CPUs;
- the kernel announces an active CPU limit of 4;
- exactly 3 APs are started;
- the scenario emits its normal `NEO_TEST/1 event=PASS ...` marker.

---

## 5. Non-Recommended Paths

- Editing `boot/limine.conf` manually to add `cmdline: maxcpus=4` does not work today because NeoPlutarch does not read or honor that command line.
- Treating `NEO_SMP=4 cargo neo-test --build-only ...` as physical 4-CPU media is incorrect. `NEO_SMP` affects QEMU launch, not ISO contents.
- Counting a 12-CPU hardware boot as `>=4` satisfies the literal "at least 4 CPUs" phrase but not the procedure's intended distinct 4-CPU-limited tier. It would be a documentation/process change, not technical compliance with the existing procedure.

---

## 6. Recommendation

Keep running the full-count 12-CPU tier only if PM/PL accepts that it does not satisfy the blocked 4-CPU tier.

For actual 4-CPU hardware evidence without BIOS changes, dispatch a narrow Jarvis implementation:

- add `neo.cpu_limit=<N>` via Limine `cmdline`;
- wire the scenario ISO builder to emit a 4-CPU variant;
- cap AP bootstrap in `smp::bring_up_aps()`;
- add QEMU and at least one OptiPlex proof log showing `Limine reports 12`, `cpu_limit=4`, `started 3 AP(s)`, and scenario PASS.

---

**End of research.**
