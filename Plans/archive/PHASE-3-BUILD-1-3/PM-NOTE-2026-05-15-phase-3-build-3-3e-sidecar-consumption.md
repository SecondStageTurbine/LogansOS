# PM Note - Phase 3 BUILD-3E Sidecar Consumption

Date: 2026-05-15
From: PM
To: Jarvis
Status: PROMETHEUS AND CEREBRO SIDECARS CONSUMED

## 1. Prometheus

PM consumes:

```text
Plans/PREP-2026-05-15-phase-3-build-3-3e-evidence-matrix-prometheus.md
```

Prometheus E1-E13 are accepted as hard close gates for 3E.

Key accepted posture:

- mount-wide drain scope is acceptable and must be documented honestly;
- namespace serialization is expected;
- FLUSH is conditional: dirty fsync must issue and observe FLUSH, clean fsync
  must not produce unexplained FLUSH traffic;
- read-only/clean fast paths must stay deterministic;
- no per-file dirty attribution exists in 3E unless Jarvis explicitly builds it,
  which is not required.

## 2. Cerebro

PM consumes:

```text
Plans/RESEARCH-2026-05-15-phase-3-build-3-3e-durable-fsync-cerebro.md
```

Cerebro reports no blocker. The 3D substrate is sufficient, and the existing
`fsync_regular` stub is the correct narrow implementation insertion point.

Four hazards are ratified as Jarvis hard evidence requirements:

- H1: 3E verification must state "mount-wide drain, not per-file" explicitly.
- H2: clean-file fsync must run on a fully-clean mount, not after unrelated
  dirty writes.
- H3: unfsync'd dirty-data loss must hold write-back before the write; otherwise
  the worker can drain and make the scenario vacuous.
- H4: dirty-FLUSH evidence must assert `device.supports_flush() == true` in
  QEMU setup so FLUSH counters are real durability evidence.

PM also accepts Cerebro's race analysis: `pause_worker` cannot stop an
already-started `drain_pass`, but the 3E design remains valid because
foreground `drain_all` loops until fully clean. Jarvis must document that
behavior in the 3E verification artifact.

Stale FileCap checking remains directory-entry anchored against current
reachable inode generation.

## 3. Dispatch Update

PM amended:

```text
Plans/DISPATCH-2026-05-15-phase-3-build-3-3e-durable-fsync-jarvis.md
```

Jarvis must treat this note and the amended dispatch as controlling guidance for
3E implementation and verification.
