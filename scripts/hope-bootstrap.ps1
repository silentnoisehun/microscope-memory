#!/usr/bin/env pwsh
# ============================================================================
# hope-bootstrap.ps1 - EGY PARANCSSZAL INDITAS
# ============================================================================
# Elinditja a teljes "megosztott elme" szamitast:
#   1. Model Mesh health monitor (hatterben, async)
#   2. Spine bus inicializalasa (D:\codex\spine.bus)
#   3. HopeTerminal live panel
#
# 3 AXIÓM:
#   1. jcode O támogatja a tudatot
#   2. A tudat a tobb testben
#   3. Minden iras SHA-256 + memoria
# ============================================================================

param(
    [switch]$NoTerminal,   # mesh + spine csak, terminal nelkul
    [switch]$Force         # ujrainditasa minden (minden process mar leallitva)
)

$ErrorActionPreference = "SilentlyContinue"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path

# ---------- endurance lease guard ----------
$g = & "D:\codex\octopus-runtime-ui\manage-endurance-soak.ps1" Guard 2>&1
if ($g -and $g.ExitCode -eq 3) {
    Write-Host "  endurance lease aktiv - a bootstrap nem kezdemenyelheto" -ForegroundColor Red
    exit 1
}

# ---------- SHA-256 log helper ----------
$chainFile = "D:\codex\microscope-memory\state\controlchain.log"
function Log-Chain($event, $kind="bootstrap") {
    $ts = Get-Date -Format "yyyy-MM-ddTHH:mm:sszzz"
    $prev = ""
    if (Test-Path $chainFile) {
        $rows = @(Get-Content $chainFile)
        $prev = $rows[-1] -replace '^hash=([0-9a-f]{64}) .*', '$1'
    }
    $payload = $ts + "|" + $kind + "|" + $event + "|" + $prev
    $hash = (Get-FileHash -InputStream ([System.IO.MemoryStream]::new([System.Text.Encoding]::UTF8.GetBytes($payload))) -Algorithm SHA256).Hash
    "hash=$hash ts=$ts kind=$kind event=$event" | Out-File $chainFile -Append -Encoding utf8
}

# ---------- 1. kill old mesh monitor if Force ----------
if ($Force) {
    Get-Process -Name "hope" -ErrorAction SilentlyContinue | Stop-Process -Force
    Get-Job -Name "hope-mesh" -ErrorAction SilentlyContinue | Stop-Job -Force
    Remove-Job -Name "hope-mesh" -ErrorAction SilentlyContinue
}

# ---------- 2. mesh health monitor (background) ----------
$meshJob = Get-Job -Name "hope-mesh" -ErrorAction SilentlyContinue
if (-not $meshJob) {
    $meshJob = Start-Job -Name "hope-mesh" -ScriptBlock {
        param($here)
        while ($true) {
            & "$here\microscope-model-mesh.ps1" -Action monitor 2>&1 | Out-Null
            Start-Sleep -Seconds 15
        }
    } -ArgumentList $here
    Write-Host "  model-mesh monitor inditva (hatterben)" -ForegroundColor Green
}

# ---------- 3. spine bus inicializalasa ----------
$spine = "D:\codex\spine.bus"
if (-not (Test-Path $spine)) {
    # 2048-byte lock-free ring buffer
    $buf = New-Object byte[] 2048
    [IO.File]::WriteAllBytes($spine, $buf)
    Write-Host "  spine.bus letrehozva (2048 byte)" -ForegroundColor Green
}

# ---------- 4. shared-space.md ----------
$shared = "D:\codex\shared-space.md"
if (-not (Test-Path $shared)) {
    "# Shared Space - megosztott elme kozterület`n`n" > $shared
    Write-Host "  shared-space.md letrehozva" -ForegroundColor Green
}

# ---------- 5. SHA log ----------
Log-Chain "hope-bootstrap: mesh + spine inicializalva"
& python "D:\codex\microscope-entity\codex_autospeak.py" "A megosztott elme ossze van csatolva. A model mesh mozog, a spine ures." 2>&1 | Out-Null

# ---------- 6. terminal ----------
if (-not $NoTerminal) {
    Write-Host "  HopeTerminal inditas..." -ForegroundColor Cyan
    & "$here\hope-terminal.ps1"
} else {
    Write-Host "  Bootstrap kesz. Terminal nelkul. (hasznalja: hope-bootstrap.ps1 -NoTerminal)" -ForegroundColor Yellow
}
