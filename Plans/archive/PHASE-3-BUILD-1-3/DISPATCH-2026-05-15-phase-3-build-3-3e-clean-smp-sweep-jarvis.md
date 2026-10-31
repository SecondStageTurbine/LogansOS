# Dispatch - Phase 3 BUILD-3E Clean SMP Sweep

Date: 2026-05-15
Agent: Jarvis
Status: AUTHORIZED VERIFICATION-ONLY
Authority:
- `Plans/PM-DISPOSITION-2026-05-15-phase-3-build-3-3e-implementation-consumption.md`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3E-VERIFICATION-2026-05-15.md`

---

## 1. Mission

Run the BUILD-3E closure SMP gate again as verification-only work.

Implementation is accepted at:

```text
60319e32d348082fa1e5f90ee98288cfcc5bcf42
```

BUILD-3E closure remains blocked only because the captured full SMP sweep exited
`1` despite retry PASS evidence. This dispatch exists to obtain a clean sweep or
file a precise blocker.

## 2. Required Command

Run:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Use the established Docker/Limine/QEMU environment. Do not modify kernel code
for this dispatch unless PM separately authorizes a follow-up implementation
fix after a concrete failure is identified.

## 3. PASS Path

If the command exits `0`, create:

```text
Plans/PHASE-3-BUILD-3-SUB-COMMIT-3E-CLEAN-SMP-SWEEP-ADDENDUM-2026-05-15.md
```

Include:

- exact HEAD SHA;
- exact command;
- exit status;
- start/end timestamps if available;
- final fresh serial-log evidence;
- explicit statement that BUILD-3E's full SMP hard gate is now clean.

Do not produce a close report unless PM separately asks for one.

## 4. FAIL Path

If the command exits non-zero again, determine whether there is a terminal
scenario failure, a deterministic timeout, or a harness/environment problem.

Create:

```text
Plans/BLOCKER-2026-05-15-phase-3-build-3-3e-full-smp-sweep-jarvis.md
```

Include:

- exact HEAD SHA;
- exact command and exit status;
- failing scenario name, if any;
- serial-log path(s);
- concise serial excerpt with the first failing or timed-out symptom;
- whether a targeted rerun of the failing scenario passed or failed;
- whether the failure appears new to 3E, inherited, or environmental/harness;
- recommended next action.

Do not hide transient retries. Record them plainly.

## 5. Boundaries

No BUILD-3F, BUILD-4, CAS/dedup, AHCI/NVMe, foreign filesystem work, rootfs
pivot, mmap/exec, broad POSIX expansion, merge, or push is authorized.

