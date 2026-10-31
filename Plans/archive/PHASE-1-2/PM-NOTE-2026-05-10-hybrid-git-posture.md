# PM note: hybrid git posture — GitHub remote for namespace + offsite backup, local-only operational default

**Date:** 2026-05-10
**Author:** PM
**Status:** Project-history record (informational). Codifies project-lead decision of 2026-05-10 (evening) following the trademark-driven rename to NeoPlutarch and the resulting creation of `SecondStageTurbine/NeoPlutarch` on GitHub.
**Supersedes:** `Plans/PM-NOTE-2026-05-07-local-only-posture.md` — only in the narrow sense that a remote now exists. The operational discipline of "commits land locally by default" carries forward unchanged.

---

## Background

On 2026-05-07, project lead established local-only posture after Cerebro's sub-commit 2 closure surfaced an empty `git remote -v`. The 2026-05-07 PM-NOTE (`Plans/PM-NOTE-2026-05-07-local-only-posture.md`) codified the decision and revised dispatch shape to omit push steps.

On 2026-05-10, project lead identified a trademark concern with the original project name "AltimitNeo" (derivative-name exposure to Bandai Namco's `.hack` franchise). The resolution path was a rename to "NeoPlutarch" plus reservation of public namespaces — GitHub org, package registries, domains — to prevent squatting and to claim the new name's identity space before public visibility. The Janus pre-flight at `Plans/RESEARCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md` verified NeoPlutarch namespaces clean across USPTO, EU IPO, JPO, GitHub, package registries, and domains.

The project lead then directed: "We'll have Prometheus create the git for us under github.com/SecondStageTurbine." This created `SecondStageTurbine/NeoPlutarch` as a private GitHub repo. Sub-step 6 docs closure dispatch (`Plans/DISPATCH-2026-05-10-sub-step-6-docs-closure-prometheus.md`) is currently executing under this remote.

When the project lead later asked "Why did we commit and push to git? I asked about doing this earlier and you pushed back saying to stay locally on drive, I think you mentioned the bible said not to push to git," PM acknowledged this is a real continuity error:

- The bible never spoke to remotes or distribution mechanisms. The 2026-05-07 local-only posture was a PM-NOTE, not a bible amendment.
- The 2026-05-10 rename instruction was treated as a fresh requirement without surfacing the 2026-05-07 prior posture. PM should have flagged the posture shift and asked for an explicit override before drafting the push step into the rename dispatch.
- The miss was a context-continuity failure on PM's part: posture documented in `Plans/` but not actively loaded in conversation context can be missed when current instructions implicitly override it.

Project lead's resolution: **hybrid posture.** This note codifies that.

## The hybrid posture

### What's true

- A GitHub remote exists at `https://github.com/SecondStageTurbine/NeoPlutarch.git`, configured as `origin`.
- The remote is **private**. Visibility to non-authenticated viewers is none. Visibility to `SecondStageTurbine` and any explicitly-invited collaborators is full.
- The remote's role is twofold:
  1. **Namespace reservation.** The name "NeoPlutarch" is now claimed in the GitHub namespace. No squatter can take `github.com/*/neoplutarch` or `github.com/SecondStageTurbine/NeoPlutarch`. If a future ratified decision goes to public release, the namespace is ready.
  2. **Offsite backup at deliberate intervals.** A hardware failure on the development machine no longer loses the project, provided pushes happen at meaningful project milestones. This is real backup value the local-only posture did not provide.

### What's not true

- **The remote is not the operational center of gravity.** Default dispatch posture remains local-only. Commits land on local main; pushes are explicit and infrequent.
- **Push is not per-commit by default.** Per-commit push would replicate the friction the 2026-05-07 decision sought to avoid (mandatory remote-verification step in every closure dispatch). Per-commit push also makes failure modes more confusing — a `git push` failure on every closure adds noise without commensurate value.
- **CI on the remote is not assumed.** `.github/workflows/ci.yml` may eventually become operational; until project lead explicitly decides to enable CI, the workflow file's existence does not imply CI runs.

### What changes from the 2026-05-07 posture

- **Empty `git remote -v` is no longer the expected state.** Dispatches that touch git operations should expect `origin → https://github.com/SecondStageTurbine/NeoPlutarch.git` to be configured. An empty `git remote -v` going forward is a surface-worthy anomaly.
- **The "Drop the push step from all closure dispatches" instruction from the 2026-05-07 note is rescinded** — but with replacement guidance, not blanket re-introduction (see below).
- **Stash safety-net duration extends to push-event boundaries when a dispatch includes a push.** For local-only dispatches, the 2026-05-07 stash-drop-after-commit semantics still applies.

### Push event policy

Pushes happen at **deliberate, project-meaningful intervals**, not per-commit. The following events are push events:

1. **End-of-phase closures.** Phase 2 closure, Phase 3 closure, etc. The bible's phase-completion criteria (Article 13) treat a phase as a meaningful checkpoint; backup via push at phase boundaries matches that grain.
2. **End-of-major-sub-commit closures.** Sub-commit closure that materially advances the phase state — Sub-step 6 (today's docs closure for SC-3B) qualifies; routine intermediate sub-step commits typically don't.
3. **Project-governance landings.** Bible amendments, ADR ratifications, project-rename events. These are governance commitments worth backing up immediately.
4. **Explicit project-lead direction.** "Push this" overrides routine policy.

Routine commits — daily work, mid-investigation instrumentation commits, mid-phase BUILD steps — do not trigger pushes. They accrue locally and ride along with the next push event.

### Implications for dispatch shape

Dispatches that include a push event:
- Include an explicit "Phase N: Push to origin" step with the `git push origin main` command and verification (`gh repo view ...`).
- Cite the push-event justification (which class above it falls under).
- Backup-after-push step is appropriate if the push is large (multiple commits being pushed together).

Dispatches that do not include a push event:
- Match the local-only shape from the 2026-05-07 note: stage, commit, verify via `git log --oneline -3`, no push, no remote-verification step.
- Stash safety-net drops after commit lands locally (2026-05-07 semantics).

PM judges per-dispatch which shape applies, based on the push-event policy above. When uncertain, default to no-push and let the next push-event dispatch sweep up the accumulated commits.

### What this is not

- **Not a Bible amendment.** The bible still doesn't speak to remotes or distribution mechanisms; it speaks to build/run/test discipline and architectural posture. Remote configuration is operational, not constitutional. (This was true under the 2026-05-07 posture; remains true under hybrid.)
- **Not a phase plan amendment.** Phase plans don't change.
- **Not a commitment to public release.** Repo stays private. Going public is a separate decision the project lead can make later, with separate considerations (license posture, README polish, contribution guidelines, etc.).
- **Not a CI activation.** `.github/workflows/ci.yml`'s status is unchanged. If project lead decides to activate CI, that's a separate decision recorded in a separate note.

### Backup posture

The 2026-05-07 backup-posture closure ("PM does not raise the backup question further") is partially superseded by hybrid: GitHub push events DO function as offsite backup. If the project lead has additional backup mechanisms (Synology snapshots — `Z:\AltimitNeo\YYYY-MM-DD-HHMMSS\` backups appear in verification logs — and possibly others), those continue alongside the GitHub backup. PM does not raise additional backup-mechanism questions; the project lead has the context.

## Calibration note on the PM continuity error

This note is filed in part as a calibration record. The continuity error that led to it — silently incorporating the GitHub-create instruction without flagging the 2026-05-07 posture — represents a PM workflow gap.

The gap: PM works from active conversation context. Posture documented in `Plans/` but not loaded in current context can be missed when current instructions implicitly override it.

The mitigation: **before drafting a dispatch that touches an operational concern** (git remote, backup, CI, deployment, etc.), PM does a targeted `Plans/` search for prior posture PM-NOTEs on that concern. For git-related concerns, the search would be along the lines of `grep -li 'remote\|push\|posture' Plans/PM-NOTE*.md` or equivalent. Found posture artifacts get surfaced to project lead in the drafting turn.

This discipline applies prospectively. PM commits to running the targeted search before any future dispatch that touches operational concerns previously documented in `Plans/`. The 2026-05-07 local-only posture and this 2026-05-10 hybrid posture are both already known; the targeted-search discipline catches NEW operational concerns as they emerge.

## Action items

1. **Hybrid posture in effect** — this note codifies it. No retroactive cleanup of the Sub-step 6 dispatch is needed; Sub-step 6 includes a push as commit 6's follow-up because Sub-step 6 is a project-governance landing (post-SC-3B + post-rename closure), which qualifies as a push event.
2. **Future dispatches use the per-dispatch judgment from this note's push-event policy.** PM judges push-vs-no-push per dispatch.
3. **Targeted-search discipline established** for future PM dispatches touching operational concerns.
4. **2026-05-07 local-only PM-NOTE marked superseded** by this note in its frontmatter status field. (The 2026-05-07 file itself is left unchanged as a historical record.)

## Cross-references

- `Plans/PM-NOTE-2026-05-07-local-only-posture.md` — superseded by this note.
- `Plans/RESEARCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md` — TM pre-flight that motivated the namespace reservation.
- `Plans/DISPATCH-2026-05-10-rename-three-commit-prometheus.md` — first dispatch under the hybrid posture (push event: rename + governance landing).
- `Plans/DISPATCH-2026-05-10-sub-step-6-docs-closure-prometheus.md` — second dispatch under the hybrid posture (push event: post-SC-3B governance landing).
- `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` — SC-3B closure under the hybrid posture.
- `https://github.com/SecondStageTurbine/NeoPlutarch` — the remote.

---

**End of note.**

*Project-history record. Not a normative document. Records the hybrid git posture as of 2026-05-10.*
