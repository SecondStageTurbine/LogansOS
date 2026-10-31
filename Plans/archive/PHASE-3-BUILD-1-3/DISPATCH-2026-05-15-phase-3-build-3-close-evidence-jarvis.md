# Dispatch - Phase 3 BUILD-3 Close Evidence

Date: 2026-05-15
Agent: Jarvis
Status: AUTHORIZED VERIFICATION-ONLY
Authority:
- `Plans/PM-DISPOSITION-2026-05-15-phase-3-build-3-post-3e-close-plan-consumption.md`
- `Plans/OBSERVE-2026-05-15-phase-3-build-3-post-3e-close-plan-prometheus.md`

---

## 1. Mission

Run final close-evidence verification for Phase 3 BUILD-3. This is not a close
report, not post-close docs, and not a merge.

No code changes are authorized unless PM separately opens a blocker-fix dispatch.

## 2. Required Commands

Run on current BUILD-3 HEAD:

```text
git diff --check
cargo check -p xtask
cargo check -p neofs-mkfs
cargo k
cargo k --no-default-features
docker compose run --rm neo cargo neo-test phase3_aarch64_virtio_barrier_lane
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
git diff --cached --check
```

If the final full SMP sweep hits the known wrapper-timeout pattern but the
container continues, use the established `docker wait` evidence path and record
it exactly. Prefer a clean direct exit `0` when possible.

## 3. PASS Path

If all gates pass, create:

```text
Plans/PHASE-3-BUILD-3-CLOSE-EVIDENCE-2026-05-15.md
```

Include:

- exact HEAD SHA;
- exact commands and exit statuses;
- start/end timestamps for the full SMP sweep if available;
- aarch64 runtime barrier PASS frame;
- final full SMP sweep result;
- build/check gate results;
- scope statement that no BUILD-4, BUILD-3F, CAS/dedup, AHCI/NVMe, foreign FS,
  rootfs pivot, mmap/exec, broad POSIX, merge, or push occurred;
- the eight caveats C1-C8 from the PM disposition.

Suggested commit message:

```text
docs: record Phase 3 BUILD-3 close evidence
```

Do not create the BUILD-3 close report unless PM separately authorizes it.

## 4. FAIL Path

If any gate fails, stop and file:

```text
Plans/BLOCKER-2026-05-15-phase-3-build-3-close-evidence-jarvis.md
```

Include:

- exact HEAD SHA;
- failing command and exit status;
- failing scenario or build step;
- serial-log path and excerpt for runtime failures;
- targeted rerun result if relevant;
- whether the failure is new, inherited, environment/harness, or code.

## 5. Boundaries

No implementation work, BUILD-4, BUILD-3F, CAS/dedup, AHCI/NVMe, foreign
filesystem support, rootfs pivot, mmap/exec, broad POSIX expansion, merge, or
push is authorized.

