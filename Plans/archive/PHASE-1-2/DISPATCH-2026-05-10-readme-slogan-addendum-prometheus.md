# DISPATCH ADDENDUM: README slogan addition (Sub-step 6 commit 7 or post-Sub-step-6 follow-up)

**Date:** 2026-05-10
**Routed to:** Prometheus
**Type:** Small single-file commit; addendum to `Plans/DISPATCH-2026-05-10-sub-step-6-docs-closure-prometheus.md`
**Sequencing:** Run after commit 6 of Sub-step 6 lands locally. If Sub-step 6's §9 push has NOT yet happened, this addendum is commit 7 and rides along with the same push. If §9 push HAS already happened, this addendum is a standalone follow-up commit + small push.
**Coordination constraint:** Same negative-list as Sub-step 6 — kernel/, boot/, tools/ off-limits.

---

## 1. Background

Sub-step 6 commit 6 fully rewrites `README.md` to align with bible v1.4.0. The rewrite removed the prior README's "The world runs on NEO." slogan but did not draft a substitute. Project lead picked a substitute on 2026-05-10 (evening) and requested it be added.

The substitute: **"Intelligence as a kernel primitive."**

Rationale: distillation of Bible Article 1.1's opening question ("what does an operating system look like when intelligence is a kernel primitive, not an application?"). Factual rather than marketing-voiced. Aligned with the project's distinctive technical thesis.

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

## 3. Pre-flight check

```bash
cd /e/AI\ Automation/AltimitNeo
git log --oneline -8                              # confirm Sub-step 6 commit 6 (README rewrite) is in history
git rev-parse HEAD                                # capture current HEAD
test -f README.md && echo "OK: README.md exists"
grep -q "^# NeoPlutarch" README.md && echo "OK: README has NeoPlutarch H1"
grep -c "Intelligence as a kernel primitive" README.md   # expect 0 before edit
grep -c "The world runs on NEO" README.md                 # expect 0 (Sub-step 6 commit 6 already removed it)
```

If any pre-flight check fails (especially "The world runs on NEO." still being present, which would indicate Sub-step 6 commit 6 hasn't landed yet), STOP and request PM clarification.

## 4. Edit

Apply the edit specified in §2. The change is a 2-line insertion:

```
> Intelligence as a kernel primitive.

```

(blockquote line + blank line, inserted between the H1 line+blank and the description paragraph)

Verify:

```bash
grep -A 4 "^# NeoPlutarch$" README.md
```

Expected output:

```
# NeoPlutarch

> Intelligence as a kernel primitive.

AI-native operating system in Rust. Clean-slate, monolithic kernel, capability-secured, single-node. Targets x86_64 (and later aarch64). Currently at Phase 2 (SMP + capability system + buddy/slab allocators + advanced TLB management).
```

## 5. Stage and commit

```bash
git diff README.md   # verify only the two-line insertion
git add README.md
git status --short
git commit -m "docs(readme): add 'Intelligence as a kernel primitive.' tagline

Adds the project tagline as a blockquote line under the NeoPlutarch
heading in README. Distillation of Bible Article 1.1's opening
question. Replaces the pre-rename README's 'The world runs on NEO.'
slogan which was removed in Sub-step 6 commit 6 without a substitute
being drafted at the time."
```

## 6. Push handling

**Path A — addendum-as-commit-7 (Sub-step 6 §9 push not yet executed):**

Skip a separate push. Sub-step 6's §9 push will pick this commit up along with the prior six.

**Path B — addendum-as-follow-up (Sub-step 6 §9 push already executed):**

Push this single commit:

```bash
git push origin main
gh repo view SecondStageTurbine/NeoPlutarch --json defaultBranchRef --jq '.defaultBranchRef.target.history.nodes[0].oid'
```

Per `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md` push-event policy, this addendum qualifies as part of the project-governance landing class (it completes the README to its final state for the post-rename governance commit sequence).

## 7. Out of scope

- Any other README edits beyond the two-line insertion.
- Touching files outside README.md.
- Modifying the bible (Article 25 doesn't enumerate taglines or slogans; this is editorial, not constitutional).

## 8. Cross-references

- `Plans/DISPATCH-2026-05-10-sub-step-6-docs-closure-prometheus.md` — parent dispatch; commit 6 is the README rewrite this addendum extends
- `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md` — push-event policy
- `docs/bible.md` Article 1.1 — source of the slogan's framing
- `README.md` (project root) — file being edited

---

**End of addendum.**
