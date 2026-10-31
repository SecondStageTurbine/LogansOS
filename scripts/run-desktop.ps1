# SPDX-License-Identifier: MPL-2.0
# One-click LogansOS desktop for Windows (Docker Desktop with the WSL 2 engine).
#
#   powershell -ExecutionPolicy Bypass -File scripts\run-desktop.ps1
#
# Builds the dev image the first time, boots the live desktop session inside
# QEMU, and opens your VNC viewer on localhost:5901 once the screen is up.
# See docs/HOWTO-run-desktop.md for the plain-language walkthrough.

param(
    # Start from a clean Arthur memory disk (target/desktop/neofs.raw).
    [switch]$FreshDisk,
    # Opaque window chrome instead of frosted glass (easier to read).
    [switch]$ReduceTransparency
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    Write-Host "Docker is not installed or not on PATH. Install Docker Desktop first (see docs/HOWTO-run-desktop.md)." -ForegroundColor Red
    exit 1
}
docker info *> $null
if ($LASTEXITCODE -ne 0) {
    Write-Host "Docker Desktop is installed but not running. Start it, wait for 'Engine running', then run this again." -ForegroundColor Red
    exit 1
}

$env:NEO_DESKTOP_MATERIAL = if ($ReduceTransparency) { "reduce-transparency" } else { "glass" }

if ($FreshDisk -and (Test-Path "target\desktop\neofs.raw")) {
    Remove-Item "target\desktop\neofs.raw"
}

# Open the VNC viewer once QEMU itself answers on 5901 (background job).
# Docker's port proxy accepts TCP as soon as the container starts, long before
# the build and QEMU are up, so wait for the VNC server's "RFB" greeting.
$viewer = @(
    "$env:ProgramFiles\TigerVNC\vncviewer.exe",
    "$env:ProgramFiles\RealVNC\VNC Viewer\vncviewer.exe",
    "${env:ProgramFiles(x86)}\TigerVNC\vncviewer.exe"
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1

$opener = Start-Job -ArgumentList $viewer -ScriptBlock {
    param($viewer)
    for ($i = 0; $i -lt 900; $i++) {
        try {
            $c = New-Object Net.Sockets.TcpClient("127.0.0.1", 5901)
            $s = $c.GetStream()
            $s.ReadTimeout = 2000
            $buf = New-Object byte[] 12
            $n = $s.Read($buf, 0, 12)
            $c.Close()
            if ($n -ge 3 -and [Text.Encoding]::ASCII.GetString($buf, 0, 3) -eq "RFB") {
                if ($viewer) { Start-Process $viewer "localhost:5901" }
                return
            }
        } catch { }
        Start-Sleep -Seconds 2
    }
}

Write-Host ""
Write-Host "Starting the LogansOS desktop. The first run builds everything and can take a while." -ForegroundColor Cyan
if ($viewer) {
    Write-Host "Your VNC viewer will open automatically when the desktop is ready."
} else {
    Write-Host "When you see 'Connect a VNC viewer to localhost:5901', open a VNC viewer and connect to localhost:5901."
}
Write-Host "Press Ctrl+C in this window to shut the desktop down."
Write-Host ""

try {
    docker compose up --build neo-desktop
} finally {
    Stop-Job $opener -ErrorAction SilentlyContinue
    Remove-Job $opener -Force -ErrorAction SilentlyContinue
    docker compose stop neo-desktop *> $null
}
