# PM NOTE - Phase 3 BUILD-2 2E sidecar consumption

**Date:** 2026-05-13
**Status:** CONSUMED; 2E SCOPE UNCHANGED
**Consumes:**
- `Plans/RESEARCH-2026-05-13-phase-3-build-2-2e-policy-generation-cerebro.md`
- `Plans/PREP-2026-05-13-phase-3-build-2-close-evidence-prometheus.md`

---

## 1. Decision

The Cerebro and Prometheus sidecars are consumed.

No new implementation scope is authorized. Jarvis should continue the existing
2E dispatch:

`Plans/DISPATCH-2026-05-13-phase-3-build-2-2e-typed-inode-filecap-jarvis.md`

Prometheus's close-evidence shell is accepted as prep only. It does not claim
BUILD-2 closure and should be held until Jarvis returns with the 2E commit and
verification artifact.

---

## 2. Cerebro Guidance Accepted

PM accepts the core Cerebro guidance:

1. The exec-policy helper must be type-first, rights-second.
2. The helper should not implement real mmap, disk exec, or NeoFS process
   spawning.
3. `ESTALE = -116` remains the PM-ratified stale FileCap errno.
4. FileRights must remain a `u32`.
5. Generation must be sourced from `superblock.next_generation`.
6. No retype, file-cap grant/derive/revoke, CAS, dedup, GC, write-back, or
   FLUSH negotiation is authorized in 2E.

The most important Jarvis warning remains:

```text
check_exec_policy must match on inode kind first.
Rights checks occur only inside the Regular arm.
Model + no execute right must return TypeForbidden, not InsufficientRights.
```

---

## 3. PM Correction: Stale Must Follow Current Reachability

Cerebro is correct that comparing against an in-memory `NeoInode` snapshot is
insufficient. PM tightens the guidance:

**Reading the old captured inode block is also insufficient in the current
2D substrate.**

Current 2D CoW behavior allocates a fresh inode block on write and updates the
root directory entry to point at it:

- `NeoFsState::commit_regular_write` reserves `new_inode_block`;
- writes the new inode there;
- updates `new_entries[entry_index].child_block = new_inode_block`;
- publishes the new root/superblock.

Therefore handle B can still point at the old inode block after handle A's CoW
write. If B's stale check merely rereads B's old inode block, it can see the old
generation and falsely pass.

2E stale detection must compare the FileCap's captured generation against the
**current reachable inode** for that file, not against:

- the old in-memory `NeoInode` snapshot; or
- the old captured inode block after CoW has moved the root entry.

Acceptable implementation shapes include storing enough stable file identity
on the opened handle to re-resolve through the mounted NeoFS state, or an
equivalent mount-state lookup that finds the current root directory entry before
reading the inode generation. The verification artifact must prove this with
the two-handle scenario: A mutates and refreshes; B returns `ESTALE`.

---

## 4. PM Correction: Reserved Type Evidence May Be Rejection

Cerebro recommends "mount inert" for reserved/future inode types. PM does not
require that behavior for 2E.

The current NeoFS parser rejects unsupported on-disk kinds:

- `NeoFsKind::from_disk(4..)` returns `UnsupportedType`;
- directory-entry parsing also calls `NeoFsKind::from_disk`;
- mount validation reads child inodes and rejects invalid child metadata.

Given that substrate and the 2E dispatch wording, the smallest honest 2E
evidence may be either:

1. deterministic image/candidate rejection for a reachable reserved type; or
2. inert mount/open refusal, if Jarvis can implement it narrowly without a
   format migration or broader directory semantics.

PM preference for current scope: **do not refactor the parser just to make
reserved types inert.** A clear rejection path is acceptable if the scenario
proves a reserved/future kind cannot acquire executable access by filename or
rights.

---

## 5. Prometheus Prep Accepted

Prometheus's artifact is accepted as the close-evidence shell:

`Plans/PREP-2026-05-13-phase-3-build-2-close-evidence-prometheus.md`

It correctly keeps all 2E slots pending, does not claim closure, and includes
the needed close stop conditions:

- missing 2E artifact/commit;
- stale checks not using current reachable generation;
- type/rights errors collapsed;
- fsync accidentally becoming intent-log/global flush/FLUSH path;
- root-invalid superblock accepted;
- BUILD-3 scope creep;
- broader POSIX scope creep;
- global rustfmt misrepresented as passing.

After Jarvis returns with 2E, PM should use this shell to assemble a close
report only if all required 2E and regression evidence is present.

---

## 6. What To Tell Jarvis

Ferry this delta to Jarvis:

```text
Cerebro sidecar consumed. 2E scope unchanged.

Must-follow guidance:
1. check_exec_policy is type-first: match kind first; rights only inside the
   Regular arm. Model + no execute right returns TypeForbidden.
2. Stale FileCap detection must compare against current reachable inode
   generation. In current 2D, CoW allocates a fresh inode block and updates the
   root directory entry, so rereading B's old inode block is not enough.
3. ESTALE = -116 remains ratified.
4. Reserved/future type evidence may be deterministic rejection; do not refactor
   parser behavior solely to make reserved types mount inert.

Prometheus close shell is prep-only and awaits the 2E artifact. No close report
or BUILD-3 work is authorized.
```

