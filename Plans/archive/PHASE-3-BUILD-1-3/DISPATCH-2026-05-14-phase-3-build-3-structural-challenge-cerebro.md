# Dispatch - Phase 3 BUILD-3 Structural Challenge

**Date:** 2026-05-14
**Agent:** Cerebro
**Status:** DISPATCHED
**Authority:** `Plans/PM-DECISION-2026-05-14-phase-3-build-3-opening.md`

---

## 1. Mission

Run a structural challenge before BUILD-3 implementation starts.

Create:

```text
Plans/RESEARCH-2026-05-14-phase-3-build-3-structural-challenge-cerebro.md
```

No code, no normative docs, and no BUILD-3 implementation are authorized.

## 2. Sharpened Gate

Start with the Level-0 question:

> Should Phase 3 BUILD-3 be implemented now, narrowed to a prerequisite-only build, or explicitly deferred to Phase 5 model-storage substrate?

Do not assume CAS/dedup belongs in the next code dispatch just because THINK v2 names it as BUILD-3 candidate scope.

## 3. Questions

1. **Admit or defer CAS.** What evidence should PM require before content-addressed lookup/dedup enters Phase 3 rather than Phase 5? Give a crisp admit/defer rule.
2. **Separate FLUSH/cache from CAS.** Can virtio FLUSH negotiation, write-back buffer cache, and `sys_fsync` semantics land without CAS/dedup? If yes, recommend the boundary.
3. **Small-kernel kthread pattern.** What is the safest minimal lifecycle model for kernel-owned background FS workers in a single-thread-per-process kernel?
4. **Write-back durability.** What are the known pitfalls when moving from write-through CoW to write-back cache plus FLUSH? Include ordering, cache eviction, dirty metadata, and fsync scope.
5. **Mark-sweep GC.** For a CoW filesystem with direct blocks and redundant superblocks, what is the smallest correct mark-sweep design? What must be atomic, and what can be rebuild-only?
6. **CAS index shape.** If CAS is admitted, is the THINK v2 "Venti-style bloom filter plus on-disk hash table" still the right first shape? What smaller alternative, if any, is safer?
7. **Tests and stop conditions.** Name the top runtime gates that would falsify a proposed BUILD-3 plan.

## 4. Required Framing

Carry BUILD-2 facts:

- NeoFS v1 is physical-block-addressed CoW.
- Inodes already carry content hashes for integrity and future CAS hooks.
- `sys_fsync` is currently a documented no-op over write-through I/O.
- The allocator is monotone bump; orphaned blocks accumulate until BUILD-3 or later GC.
- No file-backed mmap/exec exists.

## 5. Expected Output

Return a research artifact with:

1. Verdict: admit, narrow, or defer
2. Recommended BUILD-3 boundary
3. kthread lifecycle recommendation
4. FLUSH/cache/fsync ordering recommendation
5. GC recommendation
6. CAS/dedup measurement gate
7. Top risks
8. Dispatch language PM can give Jarvis

## 6. Anti-Scope

No broad prior-art survey unless it changes the gate. Keep the output pointed at PM decision quality.
