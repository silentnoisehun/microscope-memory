<#>
.SYNOPSIS
    Microscope Memory - Session End Hook
.DESCRIPTION
    Runs at session end. Dream consolidation, final morphogenesis, federation sync, stats.
#>

param(
    [string]$ConfigPath = "C:/Users/mater/AppData/Local/microscope-memory/config.toml",
    [string]$FederatedConfigPath = "C:/Users/mater/AppData/Local/microscope-memory-federated/config.toml",
    [string]$BinaryPath = "C:/Users/mater/microscope-memory-clone/target/debug/microscope-mem.exe"
)

$env:MICROSCOPE_CONFIG = $ConfigPath

function Run-Microscope {
    param([string]$Args, [string]$Config = $ConfigPath)
    $env:MICROSCOPE_CONFIG = $Config
    & $BinaryPath $Args
}

function Run-Federated {
    param([string]$Args)
    $env:MICROSCOPE_CONFIG = $FederatedConfigPath
    & $BinaryPath $Args
}

Write-Host "╔══════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  MICROSCOPE MEMORY - SESSION END                          ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════════════════╝" -ForegroundColor Cyan

# 1. Final reconsolidation
Write-Host "`n[1/7] Final reconsolidation..." -ForegroundColor Yellow
Run-Microscope "reconsolidate"

# 2. Dream consolidation (offline replay, pruning, pattern solidification)
Write-Host "`n[2/7] Dream consolidation..." -ForegroundColor Yellow
$DreamResult = Run-Microscope "dream"
Write-Host $DreamResult

# 3. Final morphogenesis cycle
Write-Host "`n[3/7] Final morphogenesis..." -ForegroundColor Yellow
$MorphoResult = Run-Microscope "morphogenesis run"
Write-Host $MorphoResult

# 4. Federation sync - export our pulses, import theirs
Write-Host "`n[4/7] Federation final sync..." -ForegroundColor Yellow
Run-Microscope "pulse-exchange"
Run-Microscope "integrate"
Run-Federated "pulse-exchange"
Run-Federated "integrate"

# 5. Emotional field snapshot
Write-Host "`n[5/7] Emotional field snapshot..." -ForegroundColor Yellow
$Field = Run-Microscope "emotional-field"
Write-Host $Field
$FieldFed = Run-Federated "emotional-field"
Write-Host "[FEDERATED] $FieldFed"

# 6. Final stats
Write-Host "`n[6/7] Final index stats..." -ForegroundColor Yellow
$Stats = Run-Microscope "stats" | Select-String "Blocks:|Depth|TOTAL:|TOTAL|embedding"
Write-Host $Stats
$StatsFed = Run-Federated "stats" | Select-String "Blocks:|Depth|TOTAL:|TOTAL|embedding"
Write-Host "[FEDERATED] $StatsFed"

# 7. Auto-context for next session (pre-load)
Write-Host "`n[7/7] Preparing auto-context for next session..." -ForegroundColor Yellow
Run-Microscope "auto-context" | Out-Null

Write-Host "`n╔══════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  SESSION END COMPLETE - COGNITIVE SUBSTRATE CONSOLIDATED   ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════════════════╝" -ForegroundColor Cyan