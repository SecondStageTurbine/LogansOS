# backup-to-synology.ps1
#
# AltimitNeo backup script - produces a dated, self-verifying backup on the
# Synology NAS at Z:\AltimitNeo\<date>\.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File "Plans\backup-to-synology.ps1"
#
# Exit codes:
#   0  - backup written and verified successfully
#   1  - environment problem
#   2  - bundle creation failed
#   3  - mirror clone failed
#   4  - restore-test verification failed
#
# Stash preservation:
#   The backup preserves git stashes by creating temporary `refs/stashes/N` refs
#   in the source repo for each `stash@{N}`. These refs are picked up by both
#   `git bundle --all` and `git clone --mirror`, ensuring the stash commits are
#   reachable in the backup (not just dangling objects). The source-side refs
#   are deleted in a finally block so they do not accumulate across runs.
#
#   On restore, the stashes are accessible as `refs/stashes/N`. To re-apply:
#     git stash apply refs/stashes/0     # apply the most-recent preserved stash
#   See RESTORE.md in each backup folder for full recovery guidance.

# ---- Configuration ----
$ProjectRoot   = "E:\AI Automation\AltimitNeo"
$BackupRoot    = "Z:\AltimitNeo"
$LogPath       = "$BackupRoot\backup-log.txt"
$Timestamp     = Get-Date -Format "yyyy-MM-dd-HHmmss"
$BackupDir     = "$BackupRoot\$Timestamp"
$BundlePath    = "$BackupDir\AltimitNeo.bundle"
$MirrorPath    = "$BackupDir\AltimitNeo.git"
$IndexCopyPath = "$BackupDir\INDEX.md"
$RestoreDocPath = "$BackupDir\RESTORE.md"
$TempVerifyDir = "$env:TEMP\altimit-backup-verify-$Timestamp"

# ---- Helpers ----

function Write-Log {
    param([string]$Message, [string]$Level = "INFO")
    $ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $line = "$ts [$Level] $Message"
    Write-Host $line
    try {
        Add-Content -Path $LogPath -Value $line -Encoding UTF8 -ErrorAction Stop
    } catch {
        Write-Host "  (could not append to log file)" -ForegroundColor Yellow
    }
}

function Fail {
    param([string]$Message, [int]$ExitCode = 1)
    Write-Log -Message $Message -Level "FAIL"
    exit $ExitCode
}

function Remove-PreserveStashRefs {
    # Delete any existing refs/stashes/* in the source. Idempotent and safe to
    # call before pinning fresh ones, and again on cleanup. Errors are logged
    # but do not abort the script.
    Push-Location $ProjectRoot
    try {
        $existing = & git for-each-ref --format='%(refname)' 'refs/stashes/*' 2>$null
        if ($existing) {
            foreach ($ref in $existing) {
                if ($ref) {
                    & git update-ref -d $ref 2>&1 | Out-Null
                }
            }
        }
    } catch {
        # Best-effort cleanup; nothing to do.
    } finally {
        Pop-Location
    }
}

# ---- Pre-flight ----

Write-Log "=== AltimitNeo backup starting ==="
Write-Log "Timestamp: $Timestamp"
Write-Log "Source:    $ProjectRoot"
Write-Log "Target:    $BackupDir"
Write-Log ""

if (-not (Test-Path $ProjectRoot)) {
    Fail "Project root not accessible: $ProjectRoot" 1
}
if (-not (Test-Path "$ProjectRoot\.git")) {
    Fail "Project root is not a git repository (no .git directory found)" 1
}
if (-not (Test-Path $BackupRoot)) {
    Fail "Backup root not accessible: $BackupRoot" 1
}

Push-Location $ProjectRoot
try {
    $sourceCommitCount = & git rev-list --all --count 2>$null
    $sourceHead = & git rev-parse HEAD 2>$null
    $sourceBranch = & git rev-parse --abbrev-ref HEAD 2>$null
    if (-not $sourceCommitCount -or -not $sourceHead) {
        Fail "Could not query git state at source. Is git installed and on PATH?" 1
    }
    $headLine = "Source HEAD: " + $sourceHead + " (" + $sourceBranch + ")"
    Write-Log $headLine
    $countLine = "Source commit count: " + $sourceCommitCount + " (across all refs)"
    Write-Log $countLine

    $statusLines = (& git status --short 2>$null)
    if ($statusLines) {
        $tracked = $statusLines | Where-Object { $_ -notmatch "^\?\?" }
        $untracked = $statusLines | Where-Object { $_ -match "^\?\?" }
        $trackedCount = if ($tracked) { @($tracked).Count } else { 0 }
        $untrackedCount = if ($untracked) { @($untracked).Count } else { 0 }
        $wtLine = "Working tree: " + $trackedCount + " tracked mods, " + $untrackedCount + " untracked entries"
        Write-Log $wtLine
    } else {
        Write-Log "Working tree: clean"
    }

    # Enumerate stashes for preservation. Each stash@{N} gets pinned as
    # refs/stashes/N below so it is reachable from a ref (not just the reflog),
    # which is required for bundle --all and clone --mirror to capture it.
    $stashListRaw = & git stash list 2>$null
    $stashCount = if ($stashListRaw) { @($stashListRaw).Count } else { 0 }
    $stashShas = @()
    for ($i = 0; $i -lt $stashCount; $i++) {
        $sha = & git rev-parse "stash@{$i}" 2>$null
        if ($sha) {
            $stashShas += $sha
        } else {
            Write-Log "Could not resolve stash@{$i}; skipping" -Level "WARN"
            $stashShas += $null
        }
    }
    if ($stashCount -gt 0) {
        $stashSummary = "Stashes to preserve: " + $stashCount
        Write-Log $stashSummary
        for ($i = 0; $i -lt $stashCount; $i++) {
            if ($stashShas[$i]) {
                $stashLine = "  refs/stashes/" + $i + " -> " + $stashShas[$i].Substring(0,7) + "  (" + $stashListRaw[$i] + ")"
                Write-Log $stashLine
            }
        }
    } else {
        Write-Log "Stashes to preserve: 0"
    }
}
finally {
    Pop-Location
}

Write-Log ""

# ---- 1. Create dated backup directory ----

try {
    New-Item -ItemType Directory -Path $BackupDir -Force -ErrorAction Stop | Out-Null
    Write-Log "Created backup directory: $BackupDir"
} catch {
    Fail "Could not create backup directory" 1
}

# ---- 1.5. Pin stashes as refs/stashes/N in the source ----
#
# These are temporary side-effect refs on the source repo. We delete them in
# the finally block at the end of the script so they do not accumulate across
# runs. While they exist, both `git bundle create --all` and `git clone --mirror`
# will treat them as ordinary refs and capture both the ref entries and the
# stash commits + their parent index commits.

Remove-PreserveStashRefs   # idempotent: clear any leftovers from a prior run

if ($stashCount -gt 0) {
    Push-Location $ProjectRoot
    try {
        for ($i = 0; $i -lt $stashCount; $i++) {
            if ($stashShas[$i]) {
                $refName = "refs/stashes/$i"
                & git update-ref $refName $stashShas[$i] 2>&1 | ForEach-Object { Write-Log "  git: $_" }
                if ($LASTEXITCODE -ne 0) {
                    Pop-Location
                    Remove-PreserveStashRefs
                    Fail "Could not pin stash $i to $refName" 1
                }
            }
        }
        Write-Log "Pinned $stashCount stash(es) under refs/stashes/*"
    }
    finally {
        Pop-Location
    }
}

# ---- 2. git bundle ----
# Wrap the bundle + mirror + verify steps in a try/finally so that
# Remove-PreserveStashRefs runs even on Fail or unexpected error.

try {
    Push-Location $ProjectRoot
    try {
        Write-Log ""
        Write-Log "Creating git bundle..."
        $bundleStart = Get-Date
        & git bundle create $BundlePath --all 2>&1 | ForEach-Object { Write-Log "  git: $_" }
        if ($LASTEXITCODE -ne 0) {
            Pop-Location
            Fail "git bundle failed" 2
        }
        $bundleElapsed = (Get-Date) - $bundleStart
        $bundleSize = (Get-Item $BundlePath).Length
        $bundleMiB = [math]::Round($bundleSize / 1MB, 2)
        $bundleSeconds = [math]::Round($bundleElapsed.TotalSeconds, 1)
        $bundleMsg = "Bundle created: " + $bundleSize + " bytes / " + $bundleMiB + " MiB in " + $bundleSeconds + " sec"
        Write-Log $bundleMsg
    }
    finally {
        Pop-Location
    }

    # ---- 3. git clone --mirror ----

    Write-Log ""
    Write-Log "Creating mirror clone..."
    $mirrorStart = Get-Date
    & git clone --mirror $ProjectRoot $MirrorPath 2>&1 | ForEach-Object { Write-Log "  git: $_" }
    if ($LASTEXITCODE -ne 0) {
        Fail "git clone --mirror failed" 3
    }
    $mirrorElapsed = (Get-Date) - $mirrorStart
    $mirrorSeconds = [math]::Round($mirrorElapsed.TotalSeconds, 1)
    $mirrorMsg = "Mirror created in " + $mirrorSeconds + " sec"
    Write-Log $mirrorMsg

    # ---- 4. INDEX.md copy ----

    Write-Log ""
    $sourceIndex = "$ProjectRoot\Plans\INDEX.md"
    if (Test-Path $sourceIndex) {
        Copy-Item $sourceIndex $IndexCopyPath -Force
        Write-Log "Copied Plans/INDEX.md"
    } else {
        Write-Log "Plans/INDEX.md not found at source; skipping" -Level "WARN"
    }

    # ---- 5. RESTORE.md ----
    # Use single-quoted heredoc so PowerShell does not process backticks or interpolation,
    # then substitute placeholders manually. This avoids the parser-trap with `target/ etc.

    $restoreTemplate = @'
# AltimitNeo backup restore guide

This folder is a backup of the AltimitNeo repository, captured __TIMESTAMP__.

## Source state at backup time

- HEAD: __HEAD__
- Branch: __BRANCH__
- Total commit count (all refs): __COMMIT_COUNT__
- Stashes preserved: __STASH_COUNT__

## How to restore

### Option A - restore from the bundle (simplest)

    git clone "Z:\AltimitNeo\__TIMESTAMP__\AltimitNeo.bundle" AltimitNeo

### Option B - fetch from the mirror

    cd /path/to/your/AltimitNeo
    git remote add backup-__TIMESTAMP__ "Z:\AltimitNeo\__TIMESTAMP__\AltimitNeo.git"
    git fetch backup-__TIMESTAMP__

### Option C - direct clone of the mirror

    git clone "Z:\AltimitNeo\__TIMESTAMP__\AltimitNeo.git" AltimitNeo

## How to recover preserved stashes

The backup pins each stash@{N} from the source repo as a regular ref at
refs/stashes/N. After restoring (any option above), the stashes are reachable
by name. To re-apply:

    git stash apply refs/stashes/0     # most-recent preserved stash
    git stash apply refs/stashes/1     # next-most-recent

To rebuild the stash reflog so `git stash list` works again, apply each in
sequence with `git stash push` after applying, or use `git update-ref` to
manually re-create refs/stash. For most recovery scenarios, `git stash apply`
on the named refs is enough to retrieve the working-tree changes.

## What is NOT in this backup

- target/ directory (build artifacts, serial logs).
- Untracked files in Plans/ (the project is local-only; Plans/ is gitignored except INDEX.md).
- Uncommitted working-tree changes that were not stashed.
- The reflog (clone --mirror does not copy reflogs). Stashes were preserved
  via refs/stashes/N to compensate.

## Verification

This backup was automatically restore-tested at creation time. See ..\backup-log.txt.
'@

    $restoreText = $restoreTemplate.Replace("__TIMESTAMP__", $Timestamp).Replace("__HEAD__", $sourceHead).Replace("__BRANCH__", $sourceBranch).Replace("__COMMIT_COUNT__", "$sourceCommitCount").Replace("__STASH_COUNT__", "$stashCount")
    Set-Content -Path $RestoreDocPath -Value $restoreText -Encoding UTF8
    Write-Log "Wrote RESTORE.md"

    # ---- 6. Restore-test verification ----

    Write-Log ""
    Write-Log "=== Restore-test verification ==="
    $cloneTargetMsg = "Cloning bundle to temp location: " + $TempVerifyDir
    Write-Log $cloneTargetMsg

    if (Test-Path $TempVerifyDir) {
        Remove-Item $TempVerifyDir -Recurse -Force -ErrorAction SilentlyContinue
    }

    & git clone --quiet $BundlePath $TempVerifyDir 2>&1 | ForEach-Object { Write-Log "  git: $_" }
    if ($LASTEXITCODE -ne 0) {
        Fail "Restore-test clone failed - backup may be corrupt" 4
    }

    Push-Location $TempVerifyDir
    try {
        $verifyHead = & git rev-parse HEAD 2>$null
        & git fetch --all --quiet 2>&1 | Out-Null
        # Standard git clone of a bundle only sets up refs/heads/*; refs/stashes/*
        # exist in the bundle but aren't fetched by default. Pull them in explicitly
        # so git rev-list --all counts the stash commits.
        & git fetch --quiet $BundlePath 'refs/stashes/*:refs/stashes/*' 2>&1 | Out-Null
        $verifyCommitCount = & git rev-list --all --count 2>$null

        # Stashes preserved as refs/stashes/N are not auto-fetched by `fetch --all`
        # in a non-mirror clone. Resolve them explicitly so we can verify.
        $verifyStashRefs = & git ls-remote $BundlePath 'refs/stashes/*' 2>$null
        $verifyStashCount = if ($verifyStashRefs) { @($verifyStashRefs).Count } else { 0 }

        $verifyHeadMsg = "Restored HEAD:         " + $verifyHead
        $verifyCountMsg = "Restored commit count: " + $verifyCommitCount
        $verifyStashMsg = "Restored stash refs:   " + $verifyStashCount + " (source had " + $stashCount + ")"
        Write-Log $verifyHeadMsg
        Write-Log $verifyCountMsg
        Write-Log $verifyStashMsg

        if ($verifyHead -ne $sourceHead) {
            Pop-Location
            $msg = "HEAD mismatch: source " + $sourceHead + " vs restored " + $verifyHead
            Fail $msg 4
        }
        if ([int]$verifyCommitCount -lt [int]$sourceCommitCount) {
            Pop-Location
            $msg = "Commit count regression: source " + $sourceCommitCount + " vs restored " + $verifyCommitCount
            Fail $msg 4
        }
        if ($verifyStashCount -lt $stashCount) {
            Pop-Location
            $msg = "Stash preservation regression: source " + $stashCount + " vs restored " + $verifyStashCount
            Fail $msg 4
        }
        Write-Log "Verification: OK (HEAD matches, commit count >= source, all stashes preserved)"
    }
    finally {
        Pop-Location
    }

    Remove-Item $TempVerifyDir -Recurse -Force -ErrorAction SilentlyContinue
    Write-Log "Verify temp dir cleaned up"
}
finally {
    # Always clean up the temporary refs/stashes/* in the source, even if the
    # backup failed mid-flight. The mirror at $MirrorPath retains its copy of
    # these refs (mirror clones are independent of source ref state after creation).
    Remove-PreserveStashRefs
    if ($stashCount -gt 0) {
        Write-Log "Cleaned up source-side refs/stashes/* (mirror retains its own copy)"
    }
}

# ---- Summary ----

Write-Log ""
Write-Log "=== Backup complete ==="
$totalSize = (Get-ChildItem $BackupDir -Recurse | Measure-Object -Property Length -Sum).Sum
$totalMiB = [math]::Round($totalSize / 1MB, 1)
$totalMsg = "Total size: " + $totalSize + " bytes / " + $totalMiB + " MiB"
Write-Log $totalMsg
$folderMsg = "Backup folder: " + $BackupDir
Write-Log $folderMsg
Write-Log "Restore: see RESTORE.md in the backup folder."

exit 0
