# PM-NOTE: Backup posture — Synology NAS as primary backup target

**Date:** 2026-05-07 (initial); 2026-05-08 (stash preservation refinement)
**Author:** PM
**Type:** Project-state record. Documents the project's backup mechanism, the rationale for choosing it, and how to recover from it. Doc-only artifact, not a normative project commitment but an operational record. The backup *commitment* is implicit in the project's "every commit is intended to ship" discipline (Bible Article 1.3) — code that cannot be recovered cannot ship.
**Authority:** PM. The decision to add a backup mechanism does not require ADR or amendment because it does not change the project's local-only posture (Bible non-existent but per `Plans/PM-NOTE-2026-05-07-local-only-posture.md`); a backup to a private LAN device is structurally a copy on user-controlled hardware, not a remote in the git sense and not a publication.

---

## What the backup does

The script `Plans/backup-to-synology.ps1` produces a dated, self-verifying backup of the AltimitNeo repository on the Synology NAS at `<nas-ip>`, accessible from this machine as `Z:\AltimitNeo\`. Each backup invocation:

1. Creates `Z:\AltimitNeo\<YYYY-MM-DD-HHMMSS>\` as a self-contained folder.
2. Pins each `stash@{N}` from the source repo as a temporary `refs/stashes/N` ref so stashes are reachable from refs (not just the reflog) and therefore captured by `git bundle --all` and `git clone --mirror`. See "Stash preservation" below.
3. Writes `AltimitNeo.bundle` — a single-file `git bundle --all` containing every commit, branch, tag, and pinned stash ref.
4. Writes `AltimitNeo.git` — a bare-repo `git clone --mirror`, browsable as a git remote without unpacking.
5. Copies `Plans/INDEX.md` (the one tracked file in `Plans/`) for human orientation.
6. Writes `RESTORE.md` with step-by-step recovery instructions specific to that backup folder, including stash recovery commands.
7. Clones the bundle to a temp directory, fetches `refs/stashes/*` explicitly (standard bundle clone only sets up `refs/heads/*`), and verifies HEAD, commit count, and stash-ref count match the source. Fails loudly if not. Untested backups are not backups.
8. Deletes the temporary `refs/stashes/*` from the source in a finally block — even on script failure — so source-side ref state is unchanged from before. The mirror retains its independent copy.
9. Logs everything to `Z:\AltimitNeo\backup-log.txt` for audit.

Each backup is a complete snapshot. Older backups can be deleted at any time without affecting newer ones; there is no incremental dependency.

## Why this target

The Synology NAS is structurally the right backup target for this project. Independent failure-domain analysis:

| failure mode | source machine | external HDD on source | Synology NAS |
|---|---|---|---|
| Source HDD failure | data lost | recoverable | recoverable |
| Source machine PSU / motherboard | possibly recoverable from source HDD | recoverable | recoverable |
| Power surge across whole desk | data lost | data lost (same surge protector) | recoverable (different power circuit) |
| Ransomware encrypting accessible drives | data lost | likely lost | possibly lost (depends on SMB account permissions) |
| House fire / theft of single machine | data lost | varies by location | data lost (if NAS in same room) |
| RAID disk failure inside NAS | n/a | n/a | recoverable (RAID handles it) |

The Synology covers the most common failure modes. It does not cover house-wide events; an offsite (cloud) third leg would handle those. That's a separate work item.

## Why not GitHub or another remote

The project is local-only per `Plans/PM-NOTE-2026-05-07-local-only-posture.md`. The local-only posture was chosen deliberately and remains in effect. The Synology backup is a *copy on user-controlled hardware*, not a remote in the git sense — it does not change the project's distribution posture, license posture (Bible Article 26 license decision deferred), or expose source to any third party. The backup is bytes on a NAS the user owns, on a LAN the user controls.

If the project's distribution posture changes in the future, the backup mechanism does not change with it; it remains valid regardless of whether the project is also pushed to a public or private remote elsewhere.

## Stash preservation

This is a 2026-05-08 refinement of the original 2026-05-07 posture. It addresses a subtle gap: by default, neither `git bundle create --all` nor `git clone --mirror` reliably preserves stashes in a way that a future restore can recover them.

**The gap, in three parts:**

1. `git bundle create --all` enumerates refs but does not include `refs/stash` in its default ref set on most git versions. Even if it did, only the most-recent stash is reachable via `refs/stash` — older stashes are reachable only via the per-repo reflog at `.git/logs/refs/stash`.
2. `git clone --mirror` copies refs but does not copy reflogs (the project's `.git/logs/` is per-repo state, not refs). So even though the mirror contains the stash *commit objects* (because `--mirror` packs all reachable objects), older stashes have no ref pointing at them and would be reachable only as dangling objects, vulnerable to a future `git gc` pass.
3. The original verification (commit count via `git rev-list --all --count` on a bundle clone vs. on the source) flagged a regression but did not articulate the mechanism.

**The mechanism the script now uses:**

Before bundle/mirror, the script enumerates `git stash list` and creates a temporary ref at `refs/stashes/N` for each `stash@{N}`, pointing at the same commit. These are real refs, so:

- `git bundle create --all` includes them in the bundle as ref entries (not just packed objects).
- `git clone --mirror` copies them to the mirror as ref entries.
- The verification step explicitly fetches `refs/stashes/*:refs/stashes/*` after cloning the bundle (since standard bundle clone only sets up `refs/heads/*`), then counts. The check passes when both `restored_count >= source_count` AND `restored_stash_refs >= source_stash_refs`.

After the bundle/mirror/verification complete, a `finally` block deletes the temporary `refs/stashes/*` from the source. The source-side ref state is unchanged from before the script ran. The mirror at `Z:\AltimitNeo\<timestamp>\AltimitNeo.git` retains its independent copy of the refs (clone is a snapshot, not a live link).

**On restore:**

The preserved stashes are accessible by name in the restored repository:

```
git stash apply refs/stashes/0     # most-recent preserved stash
git stash apply refs/stashes/1     # next-most-recent
```

The reflog itself is not reconstructed — `git stash list` will show the apparent absence of stashes until the operator re-creates them. For most recovery scenarios, `git stash apply` on the named refs is enough to retrieve the working-tree changes; rebuilding the stash reflog is left as manual operation if the operator wants `git stash list` to work again. Each backup folder's `RESTORE.md` documents the recovery commands explicitly.

**Why this matters for AltimitNeo specifically:**

The project preserves stashes containing investigation-only instrumentation (currently `stash@{0}: wedge-investigation-2-instrumentation` and `stash@{1}: h1-experiment-instrumentation`, both kept across the chat boundary as reference state per active wedge investigation). These stashes are not "in flight" — they're durable artifacts of completed diagnostic work that future investigations may re-apply. A backup that loses them would lose project-state material. The 2026-05-08 refinement closes that gap.

## What's NOT backed up

- **`target/` directory.** Build artifacts and verification serial logs. Reproducible from source. Currently several gigabytes; not worth backing up.
- **Untracked `Plans/` artifacts.** PM-NOTE drafts, dispatch artifacts, close reports, etc. live on the source machine's E: drive and are not part of the git repository per the project's local-only posture. If they become tracked, future backups include them automatically.
- **Uncommitted working-tree changes** that are not stashed. Anything `M`, `A`, or `D` in `git status` at backup time is not in the bundle. Commit before backing up if those changes need to be preserved.
- **The reflog.** Stashes are preserved via `refs/stashes/N` per "Stash preservation" above, but the reflog itself (which `git stash list` reads) is not copied. Recovery uses the named stash refs directly.

The first two omissions are deliberate. The third is a property of any git-bundle-based backup; users should know about it. The fourth is the boundary that "Stash preservation" works around.

## How to run

Manually, in a normal (non-elevated) PowerShell window:

```powershell
cd "E:\AI Automation\AltimitNeo"
powershell -ExecutionPolicy Bypass -File "Plans\backup-to-synology.ps1"
```

The script takes ~15-20 seconds for the current project size (well under 1 GB total git data, ~110 MB/s sustained over the LAN per pre-deployment probe). It exits with code 0 on success, non-zero on failure (codes documented at the top of the script).

For automatic scheduling, the recommended pattern is Windows Task Scheduler with a "daily at end of work" trigger:

```
schtasks /Create /TN "AltimitNeo Backup" /TR "powershell -ExecutionPolicy Bypass -File E:\AI Automation\AltimitNeo\Plans\backup-to-synology.ps1" /SC DAILY /ST 23:00
```

The schedule can be tightened (per-commit via a git hook, hourly, etc.) if commit cadence justifies it. At current cadence (a few commits per day), daily is sufficient.

## How to restore

Each backup folder contains a `RESTORE.md` with restoration instructions specific to that backup. Three forms documented:

1. **Bundle restore (simplest):** `git clone Z:\AltimitNeo\<date>\AltimitNeo.bundle AltimitNeo`
2. **Mirror fetch:** add the mirror as a git remote and fetch from it.
3. **Direct copy:** copy the `.git` mirror folder somewhere and treat it as a bare repository.

In all three, preserved stashes are accessible at `refs/stashes/N` and re-apply via `git stash apply refs/stashes/N`.

Restoration has been verified at creation time of every backup; the script will not declare a backup successful if restoration fails. This is a hard property, not a guideline.

## Operational properties

- **Throughput:** ~105 MB/s sustained write to `Z:\AltimitNeo\` over gigabit Ethernet (verified via 200 MB sustained-write probe). Saturating LAN, not bottlenecked on the NAS.
- **Latency:** sub-millisecond ping to <nas-ip> (LAN-local).
- **NAS hardware:** Synology DSM 7.x (verified via DSM API version capability check). Specific model and RAID configuration are visible in DSM web UI at http://<nas-ip>:5000 under Storage Manager. RAID redundancy presumed but not independently verified by this script — the backup correctness depends only on SMB write succeeding, not on RAID; however, RAID is what makes the NAS-side data resilient to a single disk failure within the NAS.
- **Backup folder location:** `Z:\AltimitNeo\<timestamp>\`. Multiple backups accumulate; manual cleanup of old folders is the user's responsibility (no automatic retention policy in the script).
- **Log file:** `Z:\AltimitNeo\backup-log.txt`. Append-only audit trail across all backup runs.
- **Source-side ref state:** unchanged after each backup. Temporary `refs/stashes/*` are added before bundle/mirror and deleted in a finally block, so an external observer of the source repo sees no persistent refs from the backup mechanism.

## What this note explicitly does NOT do

- It does not commit the project to running backups on any specific schedule. The script exists; running it is operational discipline, not a project commitment.
- It does not replace offsite/cloud backup considerations. House-wide events (fire, theft, flood) are not covered; an encrypted-bundle-to-cloud third leg would address that and is a separate future work item.
- It does not impose a retention policy. Old backup folders can be deleted at any time. The project does not currently track which backups have been kept vs. pruned.
- It does not change the project's local-only distribution posture. The backup is internal to user-controlled hardware.
- It does not preserve the source repo's reflog. Stashes are preserved via named refs; the reflog itself is per-repo state and is not in the backup. If reflog continuity matters for a recovery scenario, the operator must reconstruct it manually.

## Cross-references

- **`Plans/backup-to-synology.ps1`** — the script implementing the backup. The 2026-05-08 stash-preservation mechanism is documented in the script's own header comment and in the "Stash preservation" section above.
- **`Plans/PM-NOTE-2026-05-07-local-only-posture.md`** — the project's local-only posture; this backup is consistent with it.
- **`Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md`** — example of a project commitment of similar weight (wedge fix before SC-3B). The backup posture is operationally important but not a deadlined commitment.
- **Bible Article 1.3** — "We build real operating systems engineering." Code that cannot be recovered is not engineering; the backup is the structural mechanism that makes "every commit is intended to ship" actually true.
- **Bible Article 23.2** — "no telemetry, training data, or usage data from the OS." This backup mechanism does not implicate Article 23.2 because the bytes never leave user-controlled hardware.
- **`Z:\AltimitNeo\<timestamp>\RESTORE.md`** — per-backup restore guide, generated by the script. Backups created at or after `2026-05-08-153939` include explicit stash-recovery commands.
- **`Z:\AltimitNeo\backup-log.txt`** — append-only audit log.

---

**Posture recorded.** First backup at `2026-05-07-231118`. As of 2026-05-08, three valid backups on disk (`2026-05-07-231118`, `2026-05-08-091323`, `2026-05-08-153939`); the latest is the first to use the stash-preservation mechanism. Future backups accumulate in dated subfolders. Restoration is documented per-backup. Recovery is verified per-backup. Drive failure on the source machine no longer means the project is gone, and stashes that the project explicitly preserves no longer have a quiet vulnerability window to a future `git gc`.
