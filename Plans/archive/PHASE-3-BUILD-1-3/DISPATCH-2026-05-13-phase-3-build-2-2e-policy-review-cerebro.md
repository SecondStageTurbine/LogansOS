# DISPATCH - Cerebro 2E policy and generation review

**Date:** 2026-05-13
**Owner:** Cerebro
**Status:** DISPATCHED
**Mode:** research/review only
**Authority:** `Plans/PM-DECISION-2026-05-13-phase-3-build-2-2d-consumption-and-2e-authorization.md`

---

## 1. Mission

While Jarvis implements BUILD-2 sub-commit 2E, perform an independent review of
the 2E typed-inode and FileCap-generation design constraints.

Output a concise artifact:

`Plans/RESEARCH-2026-05-13-phase-3-build-2-2e-policy-generation-cerebro.md`

Do not edit kernel, userland, tool, docs/spec, or normative phase files.

---

## 2. Questions To Answer

### Q1 - Typed policy helper

What is the smallest correct executable-policy helper for BUILD-2, given that
real file-backed `mmap`, disk-backed ELF exec, and NeoFS process spawning are
out of scope?

Answer should name:

- where the helper should live conceptually;
- whether it should be VFS-only, syscall-facing, or test-harness-only;
- how to prove `Model` is type-forbidden without implementing actual exec.

### Q2 - Type-first / rights-second matrix

Review the required four-corner matrix:

- `Model` + execute right -> `TypeForbidden`;
- `Model` + no execute right -> `TypeForbidden`;
- `Regular` + no execute right -> `InsufficientRights`;
- `Regular` + execute right -> success.

Confirm whether this preserves the ADR-021/Cerebro distinction between
type-denial and rights-denial. Call out any trap where existing `OpenFile`
logic checks rights before type and would make the test lie.

### Q3 - FileCap generation capture

Review the smallest safe way to capture inode generation in file capabilities.

The desired behavior:

- handle A and handle B open the same Regular inode;
- A mutates through the CoW write path;
- A may refresh to the new generation after its successful mutation;
- B must fail later operations as stale.

The answer must specifically address why comparing against an old in-memory
`NeoInode` snapshot is insufficient. The check must compare against current
reachable NeoFS state.

### Q4 - Stale errno

PM ratifies `ESTALE = -116` by default. Confirm whether that is the right
distinct syscall-visible error for stale FileCaps in this codebase, or name a
better local convention if one exists.

Do not recommend collapsing stale to `EBADF`, `EACCES`, `EISDIR`, or `EINVAL`
unless there is a strong reason.

### Q5 - Reserved/future inode type behavior

Given the current NeoFS mount validator, what is the smallest honest BUILD-2
evidence for reserved/future inode type refusal?

Options to evaluate:

- reject the whole mount candidate when a reachable reserved type appears;
- mount it inert and refuse policy/open;
- use a deterministic test-image variant that proves the rejection path.

Name which option best fits the current code and BUILD-2 closure claim.

### Q6 - Forward-compat invariants

Confirm the 2E implementation preserves:

- `FileRights` as `u32`;
- on-disk type discriminant as source of truth, not filename suffix;
- inode generation sourced from `superblock.next_generation`;
- `reserved_retype_audit[16]` untouched;
- no retype implementation;
- no file-cap grant/derive/revoke semantics;
- no BUILD-3 CAS, dedup, GC, write-back, or FLUSH negotiation.

---

## 3. Deliverable Format

Use this structure:

1. Verdict: "2E plan confirmed" or "2E blocker found".
2. Six question answers, one section per question.
3. Top risks for Jarvis to check before commit.
4. If blocked, exact blocker language PM should use.
5. Time-budget actual.

Keep it review-only. Do not claim 2E or BUILD-2 closure.

