# PM-NOTE - BUILD-4C mtools Stop Condition Resolution

Date: 2026-05-20
From: PM
Status: STOP CONDITION CONSUMED / ENVIRONMENTAL CLARIFICATION / JARVIS RE-AUTHORIZED TO PROCEED
Subject: Clearing the BUILD-4C stop condition Jarvis correctly surfaced on mtools unavailability.

Authority:

- `Plans/DISPATCH-2026-05-20-phase-3-build-4C-fat32-readonly-jarvis.md` §5 stop
  conditions (mtools unavailable in dev container at runtime)
- `Plans/OBSERVE-2026-05-16-phase-3-build-4-prereq-audit-jarvis.md` Q10 (mtools
  inventory inside the container)
- `Dockerfile` line 23 (`mtools` in the apt-get install list)
- `docker-compose.yml` service `neo` (the runtime where `cargo neo-test`
  executes)

## 1. Disposition

Jarvis's stop-condition surface is acknowledged and correct per dispatch:

> "I checked the required mtools commands in this runtime:
> mformat: not found / mcopy: not found / mmd: not found
> The dispatch explicitly says to stop and surface if mtools is unavailable."

Stop-and-surface was the correct behavior. The dispatch authorized exactly
this when mtools was unavailable.

The resolution, however, is environmental clarification rather than a
tooling change. **mtools is already bundled in the dev container.** Jarvis
checked the host shell and Windows/MSYS/WSL paths; he did not check inside
the `neoplutarch-dev` container, which is where `cargo neo-test` actually
runs.

## 2. Evidence

### 2.1 mtools is in the Dockerfile

`Dockerfile` lines 18–33 install the dev-container apt packages. Line 23:

```
        mtools \
```

The image is built from `debian:bookworm-slim`; Debian Bookworm's `mtools`
package provides `/usr/bin/mformat`, `/usr/bin/mcopy`, `/usr/bin/mmd`,
`/usr/bin/mdir`, `/usr/bin/mtype`, and the rest of the mtools suite.

### 2.2 The May 16 prereq audit confirmed this

`Plans/OBSERVE-2026-05-16-phase-3-build-4-prereq-audit-jarvis.md` Q10
inventoried mtools availability **inside the container**:

```
mformat=/usr/bin/mformat
mcopy=/usr/bin/mcopy
mdir=/usr/bin/mdir
```

The audit was authored by Jarvis on May 16. The May 20 implementation pass
did not carry forward that finding when checking runtime availability.

### 2.3 cargo neo-test runs inside the container

Per the BUILD-3 close evidence and BUILD-4A/4B close patterns:

```
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

The `neo` service mounts the workspace at `/workspace` (per
`docker-compose.yml`), runs Rust toolchain + QEMU + xtask all inside the
container. When xtask shells out to mtools during FAT32 image generation,
the shell-out happens inside the container, where mtools is available at
`/usr/bin/`.

## 3. PM Accountability

The dispatch's stop-condition language was:

> "mtools tooling turns out to be unavailable or insufficient in the dev
>  container at runtime"

The phrase "in the dev container at runtime" was intended to specify that
the runtime is the dev container — distinct from the host. The phrasing
was sufficient for context but did not explicitly direct Jarvis to verify
*inside* the container before raising the stop condition. PM owns this
small ambiguity.

For future dispatches involving tooling-availability checks, the canonical
verification command is:

```
docker compose run --rm neo bash -c 'which mformat mcopy mmd mdir'
```

PM directs that this be the canonical idiom going forward; surfacing it
here so the next dispatch doesn't repeat the same shape of ambiguity.

## 4. Verification Step (Required Before Resuming)

Jarvis should run the canonical check before proceeding, to confirm the
container is what PM and the audit say it is:

```
docker compose run --rm neo bash -c 'which mformat mcopy mmd mdir mtype'
```

Expected output: five paths, all under `/usr/bin/`. If any binary is
missing, surface that as a genuine stop condition (the Dockerfile would
need amending), but PM does not expect that to occur given the May 16
audit and the Dockerfile content.

## 5. BUILD-4C Status

The dispatch
`Plans/DISPATCH-2026-05-20-phase-3-build-4C-fat32-readonly-jarvis.md` is
**re-authorized** for implementation pending the §4 verification step.

All other aspects of the dispatch (scope, anti-scope, scenarios, gates,
required close artifact) remain unchanged.

Stop conditions in the original dispatch §5 remain in force; PM has not
relaxed any of them. The mtools-unavailability condition specifically is
clarified-not-removed: it still applies if mtools turns out to be missing
*inside the container* after the §4 verification.

## 6. PM Posture

PM monitoring. Jarvis runs the verification step, confirms mtools is
present inside the dev container, and proceeds with BUILD-4C
implementation. If the verification surfaces a real absence (which would
be inconsistent with both the Dockerfile and the May 16 audit), Jarvis
re-surfaces and PM will direct a Dockerfile amendment lane.

No other lane is affected by this clarification.
