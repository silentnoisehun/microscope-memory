<#>
.SYNOPSIS
    Microscope Memory - Periodic Consolidation Hook
.DESCRIPTION
    Runs consolidation based on triggers (not session-end which never fires for humans).
    Triggers: interaction count, time interval, memory pressure, manual.
#>

param(
    [string]$ConfigPath = "C:/Users/mater/AppData/Local/microscope-memory/config.toml",
    [string]$FederatedConfigPath = "C:/Users/mater/AppData/Local/microscope-memory-federated/config.toml",
    [string]$BinaryPath = "C:/Users/mater/microscope-memory-clone/target/debug/microscope-mem.exe",
    [int]$InteractionCount = 0,
    [switch]$ForceDream,
    [switch]$ForceMorphogenesis,
    [switch]$ForceFederation,
    [switch]$FullCycle
)

$env:MICROSCOPE_CONFIG = $ConfigPath

function Run-Microscope {
    param([string]$Args, [string]$Config = $ConfigPath)
    $env:MICROSCOPE_CONFIG = $Config
    & $BinaryPath @($Args -split ' ')
}

function Run-Federated {
    param([string]$Args)
    $env:MICROSCOPE_CONFIG = $FederatedConfigPath
    & $BinaryPath @($Args -split ' ')
}

# Persistent state file
$StateFile = "$env:TEMP\microscope_consolidation_state.json"
$State = @{
    lastDream = 0
    lastMorphogenesis = 0
    lastFederation = 0
    interactionCount = 0
    lastConsolidationTime = [DateTime]::Now
}

if (Test-Path $StateFile) {
    $State = Get-Content $StateFile -Raw | ConvertFrom-Json
}

$State.interactionCount++
$now = [DateTime]::Now

# Thresholds
$DreamThreshold = 20
$MorphogenesisThreshold = 5
$FederationThreshold = 3
$TimeThresholdHours = 2

$needsDream = $ForceDream -or ($State.interactionCount - $State.lastDream -ge $DreamThreshold) -or (($now - $State.lastConsolidationTime).TotalHours -ge $TimeThresholdHours)
$needsMorphogenesis = $ForceMorphogenesis -or ($State.interactionCount - $State.lastMorphogenesis -ge $MorphogenesisThreshold)
$needsFederation = $ForceFederation -or ($State.interactionCount - $State.lastFederation -ge $FederationThreshold)

if ($FullCycle) { $needsDream = $needsMorphogenesis = $needsFederation = $true }

Write-Host "=== MICROSCOPE MEMORY - PERIODIC CONSOLIDATION ===" -ForegroundColor Cyan
Write-Host "  Interactions: $($State.interactionCount) | Dream: $($State.interactionCount - $State.lastDream)/$DreamThreshold | Morpho: $($State.interactionCount - $State.lastMorphogenesis)/$MorphogenesisThreshold | Fed: $($State.interactionCount - $State.lastFederation)/$FederationThreshold" -ForegroundColor Gray

if ($needsDream -or $FullCycle) {
    Write-Host "`n[1/5] Dream consolidation..." -ForegroundColor Yellow
    Run-Microscope "dream"
    $State.lastDream = $State.interactionCount
    $State.lastConsolidationTime = $now
}

if ($needsMorphogenesis -or $FullCycle) {
    Write-Host "`n[2/5] Morphogenesis cycle..." -ForegroundColor Yellow
    Run-Microscope "morphogenesis run"
    $State.lastMorphogenesis = $State.interactionCount
}

if ($needsFederation -or $FullCycle) {
    Write-Host "`n[3/5] Federation sync..." -ForegroundColor Yellow
    Run-Microscope "pulse-exchange"
    Run-Microscope "integrate"
    Run-Federated "pulse-exchange"
    Run-Federated "integrate"
    $State.lastFederation = $State.interactionCount
}

Write-Host "`n[4/5] Reconsolidation..." -ForegroundColor Yellow
Run-Microscope "reconsolidate"

Write-Host "`n[5/5] Emotional field..." -ForegroundColor Yellow
Run-Microscope "emotional-field" | Write-Host
Run-Federated "emotional-field" | Write-Host

# Save state
$State | ConvertTo-Json -Depth 3 | Set-Content $StateFile

Write-Host "`nOK Periodic consolidation complete" -ForegroundColor Green