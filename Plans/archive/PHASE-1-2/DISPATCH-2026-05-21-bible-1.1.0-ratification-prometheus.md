# DISPATCH-2026-05-21: Bible v1.1.0 Ratification + ADR-016 Status Flip

| | |
|---|---|
| **From** | Cerebro (PM) |
| **To** | Prometheus |
| **Date** | 2026-05-21 |
| **Type** | Documentation cleanup — atomic ratification commit |
| **Scope** | 3 files edited, 1 commit, no code changes |
| **Authority** | Bible Article 27 (PL-ratified 2026-05-21, see RATIFICATION-2026-05-21-ADR-016-bible-1.1.0.md) |

---

## Task summary

PL has ratified Bible v1.1.0 and ADR-016 (Linux Binary Compatibility via Personality Layer). Apply the ratified amendments to the in-tree artifacts as a single atomic commit. No interpretation, no expansion — verbatim application of the ratified text plus mechanical status flips and INDEX bookkeeping.

---

## Files to edit

1. `docs/bible.md` — apply two amendments + footer bump.
2. `docs/adr/ADR-016-linux-personality-layer.md` — flip status field.
3. `Plans/INDEX.md` — record ADR-016 acceptance + add seven open commitments.

**Verify paths against the actual repo tree before editing.** The Bible path may be `docs/bible.md` or `TheAltimitNeoBible.md` at repo root — match what's already in tree. ADR-016 path should match the established `docs/adr/ADR-NNN-<slug>.md` convention.

---

## Edit 1 — `docs/bible.md` (Article 3 fourth bullet)

**Find** (in Part I, Article 3, the bulleted list of "AltimitNeo is not"):

```
A POSIX-compatible system. We may offer a POSIX compatibility shim for porting userland tools, but the native ABI is our own.
```

**Replace with:**

```
A POSIX-compatible system natively. The native ABI is our own. Foreign-OS compatibility, including POSIX/Linux, is provided through opt-in personality subsystems per Article 5.6 — not by adopting POSIX as the native shape, and not by extending the native ABI to mimic POSIX semantics piecemeal.
```

Preserve the surrounding bullet formatting (leading `-` or `*`, indentation). Only the bullet text changes.

---

## Edit 2 — `docs/bible.md` (insert new Article 5.6)

**Locate** the end of existing Article 5.5 (Part II, Article 5 — Userspace Architecture, clause 5.5 ending with `Fork is a 1970s artifact that fights every memory-management technique invented since.`).

**Insert immediately after, before Article 6:**

```
5.6 AltimitNeo natively supports its own ABI. Foreign-OS binary compatibility, when provided, is delivered through opt-in personality subsystems that translate foreign syscalls to native kernel operations at the syscall boundary. Personality layers are kernel-resident, opt-in per process (bound at exec time and inherited by descendants), and isolated from the native ABI by construction: a personality-tagged thread cannot invoke a native syscall by number, and a native thread cannot invoke a personality syscall by number.

Personality layers must not alter native ABI semantics. Native applications never observe the presence or absence of any personality. The native ABI evolves on its own schedule and does not negotiate with personality requirements.

Personality subsystems live under kernel/src/personality/<name>/. Each personality is justified by its own ADR; the existence of one personality does not authorize others. Personalities are statically compiled (Bible Article 9.2 forbids loadable kernel modules) and may be conditionally compiled out via Cargo features for embedded or minimal builds.
```

**Formatting note:** match the existing Bible's clause formatting. Existing clauses use the pattern `5.X <prose>` with paragraph breaks between sub-paragraphs. If existing clauses use bold for the "5.X" label, replicate that. If they don't, don't add it. Match what's already there.

The inline path `kernel/src/personality/<name>/` should be rendered however other inline paths in the Bible are rendered (backticks, or plain — match existing convention).

---

## Edit 3 — `docs/bible.md` (footer)

**Find** the footer block at end of document:

```
Document: AltimitNeo Bible
Version: 1.0.0
Status: Draft — pending first ratification at Phase 1.4 close
Last amended: 2026-05-02
Authority: Project Lead
```

**Replace with:**

```
Document: AltimitNeo Bible
Version: 1.1.0
Status: Ratified
Last amended: 2026-05-21
Authority: Project Lead
```

Preserve any markdown emphasis/formatting on the field labels.

---

## Edit 4 — `docs/adr/ADR-016-linux-personality-layer.md` (status flip)

**Find** the status row in the ADR header table:

```
| **Status** | Draft (pending PL sign-off; Bible amendment to Articles 3 and 5 required for adoption) |
```

**Replace with:**

```
| **Status** | Accepted (ratified 2026-05-21; Bible v1.1.0 in effect) |
```

No other ADR-016 content changes.

---

## Edit 5 — `Plans/INDEX.md`

Three sub-edits, in this order:

### 5a. Add ADR-016 row

Locate the ADR table (or equivalent listing for accepted ADRs in INDEX.md). Add a new row:

```
| ADR-016 | Linux Binary Compatibility via Personality Layer | Accepted | 2026-05-21 | docs/adr/ADR-016-linux-personality-layer.md |
```

Match the existing ADR row format exactly — column order, separators, spacing. If existing rows use a different status term ("closed" vs "accepted") for ratified ADRs, match that convention rather than imposing "Accepted".

### 5b. Update Bible version reference

If INDEX.md tracks the Bible version anywhere (likely a header field or a top-of-file metadata block), bump from `1.0.0` to `1.1.0`. If INDEX.md does not track Bible version, skip this sub-edit.

### 5c. Add seven commitments to "Open Project Commitments" section

In the "Open Project Commitments" section, add the following entries (match the existing entry format from the test-runner wedge commitment — that section already exists per the 2026-05-07 INDEX sweep):

1. **Phase 1.5 — Process personality flag.** `Process` struct gains `personality: Personality` field (enum, initially `Native`-only). Source: ADR-016 §Implementation.
2. **Phase 1.5 — Syscall entry personality dispatch.** Syscall entry path reads personality flag and selects dispatch table. Linux dispatch table stubbed to return `ENOSYS` until Phase 8. Source: ADR-016 §Implementation.
3. **Phase 1.5 — CI personality-stub target.** Build target ensuring `kernel/src/personality/linux/` remains compilable as it grows from empty stub. Source: ADR-016 §Implementation.
4. **Phase 3 — NeoFS hosts Linux distro tree.** NeoFS design must support mounting a Linux distro userland tree with reasonable performance for many-small-opens, dense-stat, readlink-heavy access patterns. Design constraint, no implementation work yet. Source: ADR-016 §Implementation.
5. **Phase 5+ — Syscall number non-collision audit.** Native syscall allocations must not collide with eventual Linux personality dispatch. Verified by syscall registry audit before Phase 8 lands. Source: ADR-016 §Implementation.
6. **Phase 6 — DRM-compat option in accelerator driver design.** Accelerator drivers (ANSPEC-001 §9) must be designable such that a Linux-DRM-compatible character device shim can be added in Phase 9 without redesigning the driver. Design constraint, no implementation work yet. Source: ADR-016 §Implementation.
7. **Phase 8 — PERSPEC-001 drafting.** Linux Personality Specification document to be drafted at Phase 8 planning. Reserved by ADR-016, not yet drafted. Source: ADR-016 §Implementation.

Format the entries to match existing commitment-section conventions (likely list items with phase tag, description, and source reference). If the existing test-runner-wedge commitment uses a particular subhead/bullet style, replicate it.

---

## Verification before commit

Run these checks in order. Do not commit until all pass.

1. **Git status sanity.** Exactly three files modified (Bible, ADR-016, Plans/INDEX.md). No spurious whitespace-only diffs in other files.
2. **Diff review.** `git diff` shows:
   - Bible: one bullet text replaced, one new clause inserted (Article 5.6), footer fields updated.
   - ADR-016: one row in header table updated.
   - INDEX.md: one new ADR row added, seven new commitment entries added, possibly a Bible version field bumped.
3. **Bible renders cleanly.** Open the rendered markdown (preview in VS Code, or `pandoc docs/bible.md -o /tmp/bible.html` and inspect). Verify Article 5.6 sits between 5.5 and Article 6 with no formatting breakage. Verify Article 3 fourth bullet reads naturally with the rest of the list.
4. **No unintended edits.** Other Bible articles, other ADRs, other INDEX entries should be untouched. Pay particular attention to Article 5.5's existing text — Article 5.6 inserts *after* it; do not modify 5.5.
5. **Formatting consistency.** Article 5.6's prose matches surrounding article formatting. The ADR-016 status table cell matches the markdown table formatting. The INDEX additions match existing INDEX rows.

---

## Commit message

Use this commit message verbatim:

```
docs: ratify Bible v1.1.0 + accept ADR-016 (Linux personality layer)

PL ratified 2026-05-21. Two Bible amendments applied:
  - Article 3 fourth bullet: edited to reference personality subsystems
    per new Article 5.6 instead of the prior "POSIX compatibility shim"
    placeholder language.
  - Article 5.6 added: formalizes personality subsystems as the
    foreign-OS-compat mechanism (kernel-resident syscall translation,
    opt-in per process at exec time, isolated from native ABI by
    construction).

Bible version: 1.0.0 → 1.1.0 (additive amendment per Article 27).
Status: Draft → Ratified.

ADR-016 status: Draft → Accepted. ADR text is unchanged; only the
header status row flips.

Plans/INDEX.md updated:
  - ADR-016 row added as Accepted.
  - Seven pre-Phase-8 commitments added to Open Project Commitments:
    process personality flag (1.5), syscall entry personality dispatch
    (1.5), CI personality-stub target (1.5), NeoFS Linux-tree hosting
    (3), syscall number non-collision audit (5+), DRM-compat option in
    accelerator drivers (6), PERSPEC-001 drafting (8).

No code changes. Documentation-only ratification commit.

Refs: ADR-016, RATIFICATION-2026-05-21-ADR-016-bible-1.1.0.md
```

---

## Anti-scope (do not do)

- **Do not edit ADR-016's body.** Only the header status row. The decision text, alternatives, consequences, and notes are all already-ratified content and must not be touched.
- **Do not rewrite or reformat Bible articles other than the two amendments.** No "while I'm here" cleanups, no whitespace normalization, no reflowing prose. Atomic ratification commit only.
- **Do not draft PERSPEC-001.** Reserved by ADR-016 for Phase 8 planning. The INDEX commitment entry records the intent; no actual drafting work happens now.
- **Do not start any of the seven pre-Phase-8 commitments.** Recording them in INDEX is the entire deliverable. Implementation happens in their respective phases.
- **Do not add an entry to ADR-016 saying "this ADR was ratified."** The status row flip is sufficient. Adding extra ratification narrative inside the ADR is noise.
- **Do not bump any other version numbers.** Only the Bible version field changes. ANSPEC-001, other ADRs, the kernel crate version — none of these change.

---

## On uncertainty

If any of the following are true, stop and ferry back to Cerebro before committing:

- The Bible path is not `docs/bible.md` and is not `TheAltimitNeoBible.md` at root — the actual path is unexpected.
- The existing Bible formatting differs materially from what this dispatch assumes (e.g., articles are numbered differently, footer block is structured differently).
- `Plans/INDEX.md` does not have an "Open Project Commitments" section despite the 2026-05-07 INDEX sweep entry suggesting it does.
- Article 5.5 in the in-tree Bible does not end with `Fork is a 1970s artifact that fights every memory-management technique invented since.` — the anchor for Article 5.6 insertion would then be wrong.

Stopping and asking is correct behavior. Do not improvise around mismatches.

---

## Hand-back

On successful commit, ferry back:

- The commit SHA.
- A brief confirmation that all verification checks passed.
- Any minor formatting decisions made where the dispatch was underspecified (e.g., "matched existing bold convention on '5.X' labels" or "INDEX uses 'closed' rather than 'Accepted' for ratified ADRs").

That's the entire deliverable. Atomic commit, three files, no code.

---

**Dispatch issued by Cerebro (PM). Authority: PL ratification 2026-05-21.**
