# DISPATCH: README slogan addition (post-Sub-step-6 standalone follow-up)

**Date:** 2026-05-10
**Routed to:** Claude-3
**Type:** Small single-file commit; standalone follow-up to `Plans/DISPATCH-2026-05-10-sub-step-6-docs-closure-prometheus.md` (Prometheus, executing in parallel)
**Sequencing:** Run AFTER Prometheus completes Sub-step 6 and local `main` has been fast-forwarded to include all six Sub-step 6 commits (in particular commit 6, the README rewrite). Do NOT start until §3 pre-flight confirms Sub-step 6 commit 6 is in `main`'s history.
**Coordination constraint:** Same negative-list as Sub-step 6 — `kernel/`, `boot/`, `tools/` off-limits. Only `README.md` is touched.
**Supersedes/parallel-to:** `Plans/DISPATCH-2026-05-10-readme-slogan-addendum-prometheus.md` — that dispatch was authored as an in-session addendum for Prometheus; this is the standalone re-route for Claude-3 since Prometheus is fully tasked on Sub-step 6. Content of the edit is identical; only routing and sequencing differ.

---

## 1. Background

Sub-step 6 commit 6 fully rewrites `README.md` to align with bible v1.4.0. The rewrite removed the prior README's "The world runs on NEO." slogan but did not draft a substitute. Project lead picked a substitute on 2026-05-10 (evening) and requested it be added.

The substitute: **"Intelligence as a kernel primitive."**

Rationale: distillation of Bible Article 1.1's opening question ("what does an operating system look like when intelligence is a kernel primitive, not an application?"). Factual rather than marketing-voiced. Aligned with the project's distinctive technical thesis.

This dispatch was originally authored for Prometheus as an in-session addendum to Sub-step 6, executable as commit 7 of that dispatch. Operational reality on 2026-05-10 (next-day session): Prometheus is fully tasked on Sub-step 6 itself; the slogan addendum re-routes to Claude-3 as a parallel/follow-up agent. The edit content is unchanged.

## 2. Task

Add the slogan as a blockquote line immediately under the `# NeoPlutarch` H1 heading in `README.md`, with one blank line above and one blank line below per markdown convention.

**Before:**

```markdown
# NeoPlutarch

AI-native operating system in Rust. Clean-slate, monolithic kernel, capability-secured, single-node. Targets x86_64 (and later aarch64). Currently at Phase 2 (SMP + capability system + buddy/slab allocators + advanced TLB management).
```

**After:**

```markdown
# NeoPlutarch

> Intelligence as a kernel primitive.

AI-native operating system in Rust. Clean-slate, monolithic kernel, capability-secured, single-node. Targets x86_64 (and later aarch64). Currently at Phase 2 (SMP + capability system + buddy/slab allocators + advanced TLB management).
```

The blockquote (`>` prefix) renders as an indented italic/highlighted line on GitHub, which is the conventional placement for a project tagline.

**Note on the description paragraph:** the "After" example above shows the description paragraph Sub-step 6 commit 6 is expected to write. The exact wording of that paragraph is owned by Sub-step 6, not this dispatch. Do NOT modify the description paragraph; only insert the blockquote + blank line between the H1 (and its trailing blank line) and whatever description paragraph Sub-step 6 produced.

## 3. Pre-flight check

Run these BEFORE editing. Any failure → STOP and request PM clarification.

```bash
cd /e/AI\ Automation/AltimitNeo
git status                                        # expect clean working tree on main
git rev-parse --abbrev-ref HEAD                   # expect "main"
git log --oneline -10                             # human-verify Sub-step 6 commits are landed
git rev-parse HEAD                                # capture current HEAD for reference

# Sub-step 6 commit 6 landing checks (the README rewrite):
test -f README.md && echo "OK: README.md exists"
grep -q "^# NeoPlutarch" README.md && echo "OK: README has NeoPlutarch H1"
grep -c "Intelligence as a kernel primitive" README.md   # expect 0 before edit
grep -c "The world runs on NEO" README.md                 # expect 0 (commit 6 removed it)
grep -ci "microkernel" README.md                          # expect 0 (commit 6 corrected to monolithic)
grep -c "monolithic kernel" README.md                     # expect >=1 (commit 6 added)

# Bible v1.4.0 landing check (Sub-step 6 commit 3):
tail -5 docs/bible.md | grep "Version: 1.4.0" && echo "OK: bible v1.4.0 landed"
```

**Critical:** if `grep -c "The world runs on NEO"` returns nonzero, Sub-step 6 commit 6 has NOT landed; do not proceed. If `grep -ci "microkernel"` returns nonzero, Sub-step 6 commit 6 may have partially landed or have an error; STOP and request PM clarification rather than working around it.

## 4. Edit

Apply the edit specified in §2. The change is a 2-line insertion (one blockquote line + one trailing blank line) immediately after the H1's trailing blank line and immediately before the description paragraph.

After the edit, verify:

```bash
grep -B 1 -A 4 "^# NeoPlutarch$" README.md
```

Expected output (description paragraph wording per Sub-step 6 commit 6 may differ in details):

```
# NeoPlutarch

> Intelligence as a kernel primitive.

AI-native operating system in Rust. Clean-slate, monolithic kernel, capability-secured, single-node. Targets x86_64 (and later aarch64). Currently at Phase 2 (...).
```

Also verify the diff is exactly the insertion and nothing else:

```bash
git diff README.md
```

Expected diff shape: two added lines (the blockquote + a blank line), zero removed lines, zero other changes anywhere in the file.

## 5. Stage and commit

```bash
git diff --stat README.md   # expect: README.md | 2 +
git add README.md
git status --short          # expect: M  README.md
git commit -m "docs(readme): add 'Intelligence as a kernel primitive.' tagline

Adds the project tagline as a blockquote line under the NeoPlutarch
heading in README. Distillation of Bible Article 1.1's opening
question. Replaces the pre-rename README's 'The world runs on NEO.'
slogan which was removed in Sub-step 6 commit 6 without a substitute
being drafted at the time."
```

## 6. Push

Per `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md` push-event policy, this commit qualifies as part of the project-governance landing class (completes the README to its final state for the post-rename governance commit sequence). Push:

```bash
git push origin main
git rev-parse HEAD
git rev-parse origin/main   # expect: equal to HEAD
```

If Sub-step 6's §9 push has NOT yet happened at the time Claude-3 starts (unlikely but possible if PM hands this dispatch off before pushing Sub-step 6), the push above will pick up both Sub-step 6's six commits AND this slogan commit in a single push to `origin/main`. That is acceptable.

## 7. Out of scope

- Any other README edits beyond the two-line insertion.
- Touching files outside `README.md`.
- Modifying the bible (Article 25 doesn't enumerate taglines or slogans; this is editorial, not constitutional).
- Re-running Sub-step 6 verification (Prometheus owns that; if any pre-flight check from §3 fails, surface to PM rather than attempting repair).

## 8. Failure handling

If any step fails or the pre-flight surfaces a state Claude-3 doesn't expect:

1. Do NOT amend, stash, or `git reset` to recover.
2. Leave the working tree as-is.
3. File `Plans/CLAUDE-3-FAILURE-2026-05-10-slogan-addendum.md` with: what was attempted, what the actual state showed, what the expected state was, and the verbatim output of the failing command.
4. Notify PM.

## 9. Cross-references

- `Plans/DISPATCH-2026-05-10-readme-slogan-addendum-prometheus.md` — original Prometheus-routed version; preserved as historical artifact; this dispatch supersedes it operationally
- `Plans/DISPATCH-2026-05-10-sub-step-6-docs-closure-prometheus.md` — parent dispatch (in-flight under Prometheus); commit 6 is the README rewrite this addendum extends
- `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md` — push-event policy
- `Plans/HANDOFF-2026-05-10-end-of-day-sc-3b-closed-rename-complete.md` §2.14 — origin of the slogan decision
- `docs/bible.md` Article 1.1 — source of the slogan's framing
- `README.md` (project root) — file being edited

---

**End of dispatch.**
