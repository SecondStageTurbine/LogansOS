# Run this in an ELEVATED PowerShell window (right-click PowerShell → "Run as Administrator").
# It pulls the drive's own self-reported failure-prediction status plus any raw SMART attributes
# Windows can extract via WMI. Read-only — does not write to any disk.

Write-Host "=== Elevated SMART probe for AltimitNeo backup target ==="
Write-Host ""

# Confirm we're actually elevated (otherwise the WMI query just returns Access Denied again).
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host "ERROR: this script must be run from an elevated (Administrator) PowerShell window."
    Write-Host "Right-click the PowerShell icon and choose 'Run as Administrator', then re-run."
    exit 1
}
Write-Host "Elevation: OK (running as Administrator)"
Write-Host ""

# 1) The drive's own pass/fail self-assessment.
Write-Host "=== 1. FailurePredictStatus (drive's own self-assessment) ==="
try {
    $statuses = Get-WmiObject -Namespace root\wmi -Class MSStorageDriver_FailurePredictStatus -ErrorAction Stop
    foreach ($s in $statuses) {
        $shortName = ($s.InstanceName -split '\\')[-1]
        Write-Host ("Drive: {0}" -f $shortName)
        Write-Host ("  PredictFailure: {0}" -f $s.PredictFailure)
        Write-Host ("  Reason:         {0}" -f $s.Reason)
        Write-Host ("  Active:         {0}" -f $s.Active)
        Write-Host ""
    }
} catch {
    Write-Host "FailurePredictStatus query failed: $_"
}

# 2) Raw SMART attribute table (vendor-specific, but useful for the big-name attributes).
Write-Host "=== 2. Raw SMART attributes ==="
Write-Host "Key attributes to look at on a 17-year-old drive:"
Write-Host "  ID 5  (Reallocated Sectors Count)     — should be 0 or very low"
Write-Host "  ID 9  (Power-On Hours)                — informational, not a failure signal"
Write-Host "  ID 187 (Reported Uncorrectable Errors) — should be 0"
Write-Host "  ID 197 (Current Pending Sectors)      — should be 0"
Write-Host "  ID 198 (Offline Uncorrectable Sectors) — should be 0"
Write-Host "  ID 199 (UDMA CRC Error Count)         — should be 0"
Write-Host ""

try {
    $smartData = Get-WmiObject -Namespace root\wmi -Class MSStorageDriver_ATAPISmartData -ErrorAction Stop

    foreach ($d in $smartData) {
        $shortName = ($d.InstanceName -split '\\')[-1]
        Write-Host ("--- Drive: {0} ---" -f $shortName)

        # The VendorSpecific buffer contains 30 12-byte SMART attribute records starting at offset 2.
        $buf = $d.VendorSpecific
        if (-not $buf -or $buf.Length -lt 362) {
            Write-Host "  (no usable VendorSpecific buffer)"
            continue
        }

        Write-Host ""
        Write-Host "  ID  Name                           Value Worst Threshold Raw"
        Write-Host "  --  ----                           ----- ----- --------- ---"

        $names = @{
            1   = "Read Error Rate"
            3   = "Spin-Up Time"
            4   = "Start/Stop Count"
            5   = "Reallocated Sectors"
            7   = "Seek Error Rate"
            9   = "Power-On Hours"
            10  = "Spin Retry Count"
            12  = "Power Cycle Count"
            187 = "Uncorrectable Errors"
            190 = "Temperature"
            192 = "Power-Off Retract Count"
            193 = "Load/Unload Cycle Count"
            194 = "Temperature (Celsius)"
            196 = "Reallocation Event Count"
            197 = "Pending Sectors"
            198 = "Offline Uncorrectable"
            199 = "UDMA CRC Error Count"
            200 = "Multi-Zone Error Rate"
        }

        for ($i = 0; $i -lt 30; $i++) {
            $base = 2 + ($i * 12)
            $id = $buf[$base]
            if ($id -eq 0) { continue }
            $value     = $buf[$base + 3]
            $worst     = $buf[$base + 4]
            $rawBytes  = $buf[($base + 5)..($base + 10)]
            # Raw is 6 bytes little-endian for most attributes.
            $raw = 0L
            for ($j = 5; $j -ge 0; $j--) {
                $raw = ($raw -shl 8) -bor $rawBytes[$j]
            }
            $name = if ($names.ContainsKey([int]$id)) { $names[[int]$id] } else { "(vendor)" }
            Write-Host ("  {0,3} {1,-30} {2,5} {3,5}     {4,-5} {5}" -f $id, $name, $value, $worst, "?", $raw)
        }
        Write-Host ""
    }
} catch {
    Write-Host "ATAPISmartData query failed: $_"
}

# 3) Failure-prediction thresholds, where available.
Write-Host "=== 3. PredictFailure thresholds ==="
try {
    Get-WmiObject -Namespace root\wmi -Class MSStorageDriver_FailurePredictThresholds -ErrorAction Stop |
        ForEach-Object {
            $shortName = ($_.InstanceName -split '\\')[-1]
            Write-Host ("Drive: {0}" -f $shortName)
            Write-Host ("  Active: {0}" -f $_.Active)
            Write-Host ""
        }
} catch {
    Write-Host "FailurePredictThresholds query failed: $_"
}

Write-Host "=== End of SMART probe ==="
Write-Host ""
Write-Host "Interpretation guide:"
Write-Host "  - PredictFailure = False (in section 1) is the headline result you want."
Write-Host "  - Reallocated Sectors (ID 5) raw value: 0 is ideal. Single digits are OK on a"
Write-Host "    17-year-old drive. Tens or hundreds = drive is degrading."
Write-Host "  - Pending Sectors (ID 197) raw value: 0 is the only acceptable value."
Write-Host "    Anything > 0 means sectors are about to be reallocated; treat as a warning."
Write-Host "  - Power-On Hours (ID 9) raw value: tells you roughly how long the drive has run."
Write-Host "    Modern HDDs are typically rated for ~25,000-50,000 hours of duty cycle."
