<#>
.SYNOPSIS
    Installs the ai-memory-skill hooks into the AI agent environment.
.DESCRIPTION
    Sets up periodic consolidation via Windows Task Scheduler or manual triggers.
    Works with any AI agent that can run PowerShell hooks.
#>

param(
    [string]$SkillPath = "C:/Users/mater/microscope-memory-clone/ai-memory-skill",
    [string]$ConfigPath = "C:/Users/mater/AppData/Local/microscope-memory/config.toml",
    [string]$BinaryPath = "C:/Users/mater/microscope-memory-clone/target/debug/microscope-mem.exe"
)

$ErrorActionPreference = "Stop"

Write-Host "=== AI MEMORY SKILL - INSTALLER ===" -ForegroundColor Cyan

# Verify prerequisites
Write-Host "`n[1/5] Verifying Microscope Memory binary..." -ForegroundColor Yellow
if (-not (Test-Path $BinaryPath)) {
    Write-Host "  X Binary not found at $BinaryPath" -ForegroundColor Red
    Write-Host "  Build with: cargo build --release (or cargo build for dev)" -ForegroundColor Gray
    exit 1
}
Write-Host "  OK Binary found" -ForegroundColor Green

Write-Host "`n[2/5] Verifying config..." -ForegroundColor Yellow
if (-not (Test-Path $ConfigPath)) {
    Write-Host "  X Config not found at $ConfigPath" -ForegroundColor Red
    exit 1
}
Write-Host "  OK Config found" -ForegroundColor Green

# Create wrapper scripts that inject config
Write-Host "`n[3/5] Creating wrapper scripts..." -ForegroundColor Yellow

$WrapperDir = "$SkillPath\wrappers"
New-Item -ItemType Directory -Path $WrapperDir -Force | Out-Null

# Wrapper 1: recall
$RecallContent = @"
param(`$Query, `[int]`$K=10)
`$env:MICROSCOPE_CONFIG = `"$ConfigPath`"
& `"$BinaryPath`" recall `$Query `$K
"@
Set-Content -Path "$WrapperDir\microscope-recall.ps1" -Value $RecallContent
Write-Host "  OK Created $WrapperDir\microscope-recall.ps1" -ForegroundColor Green

# Wrapper 2: store
$StoreContent = @"
param(`$Text, `[string]`$Layer='session', `[int]`$Importance=5)
`$env:MICROSCOPE_CONFIG = `"$ConfigPath`"
& `"$BinaryPath`" store `$Text -l `$Layer -i `$Importance
"@
Set-Content -Path "$WrapperDir\microscope-store.ps1" -Value $StoreContent
Write-Host "  OK Created $WrapperDir\microscope-store.ps1" -ForegroundColor Green

# Wrapper 3: reconsolidate
$ReconContent = @"
`$env:MICROSCOPE_CONFIG = `"$ConfigPath`"
& `"$BinaryPath`" reconsolidate
"@
Set-Content -Path "$WrapperDir\microscope-reconsolidate.ps1" -Value $ReconContent
Write-Host "  OK Created $WrapperDir\microscope-reconsolidate.ps1" -ForegroundColor Green

# Wrapper 4: morphogenesis
$MorphoContent = @"
`$env:MICROSCOPE_CONFIG = `"$ConfigPath`"
& `"$BinaryPath`" morphogenesis run
"@
Set-Content -Path "$WrapperDir\microscope-morphogenesis.ps1" -Value $MorphoContent
Write-Host "  OK Created $WrapperDir\microscope-morphogenesis.ps1" -ForegroundColor Green

# Wrapper 5: dream
$DreamContent = @"
`$env:MICROSCOPE_CONFIG = `"$ConfigPath`"
& `"$BinaryPath`" dream
"@
Set-Content -Path "$WrapperDir\microscope-dream.ps1" -Value $DreamContent
Write-Host "  OK Created $WrapperDir\microscope-dream.ps1" -ForegroundColor Green

# Wrapper 6: pulse
$PulseContent = @"
`$env:MICROSCOPE_CONFIG = `"$ConfigPath`"
& `"$BinaryPath`" pulse-exchange
& `"$BinaryPath`" integrate
"@
Set-Content -Path "$WrapperDir\microscope-pulse.ps1" -Value $PulseContent
Write-Host "  OK Created $WrapperDir\microscope-pulse.ps1" -ForegroundColor Green

# Wrapper 7: field
$FieldContent = @"
`$env:MICROSCOPE_CONFIG = `"$ConfigPath`"
& `"$BinaryPath`" emotional-field
"@
Set-Content -Path "$WrapperDir\microscope-field.ps1" -Value $FieldContent
Write-Host "  OK Created $WrapperDir\microscope-field.ps1" -ForegroundColor Green

# Wrapper 8: consolidate
$ConsolidateContent = @"
param([switch]`$ForceDream, `[switch]`$ForceMorphogenesis, `[switch]`$ForceFederation, `[switch]`$FullCycle)
& `"$SkillPath\hooks\periodic-consolidation.ps1`" -ConfigPath `"$ConfigPath`" -BinaryPath `"$BinaryPath`" -ForceDream:`$ForceDream -ForceMorphogenesis:`$ForceMorphogenesis -ForceFederation:`$ForceFederation -FullCycle:`$FullCycle
"@
Set-Content -Path "$WrapperDir\microscope-consolidate.ps1" -Value $ConsolidateContent
Write-Host "  OK Created $WrapperDir\microscope-consolidate.ps1" -ForegroundColor Green

# Add wrappers to PATH (current session)
$env:PATH += ";$WrapperDir"
Write-Host "`n[4/5] Wrappers added to PATH for this session" -ForegroundColor Green
Write-Host "  Add $SkillPath\wrappers to your PATH permanently for global access" -ForegroundColor Gray

# Test the setup
Write-Host "`n[5/5] Testing installation..." -ForegroundColor Yellow
$Test = & "$BinaryPath" auto-context 2>&1 | Select-String "STATE:"
if ($Test) {
    Write-Host "  OK Auto-context works" -ForegroundColor Green
    Write-Host "  $Test" -ForegroundColor Gray
} else {
    Write-Host "  X Auto-context failed" -ForegroundColor Red
    exit 1
}

Write-Host "`n=== INSTALLATION COMPLETE ===" -ForegroundColor Cyan

Write-Host "`nUSAGE:" -ForegroundColor White
Write-Host "  microscope-recall.ps1 `"your query`" [k]" -ForegroundColor Gray
Write-Host "  microscope-store.ps1 `"text to remember`" [layer] [importance]" -ForegroundColor Gray
Write-Host "  microscope-reconsolidate.ps1" -ForegroundColor Gray
Write-Host "  microscope-morphogenesis.ps1" -ForegroundColor Gray
Write-Host "  microscope-dream.ps1" -ForegroundColor Gray
Write-Host "  microscope-pulse.ps1" -ForegroundColor Gray
Write-Host "  microscope-consolidate.ps1 [-ForceDream] [-ForceMorphogenesis] [-ForceFederation] [-FullCycle]" -ForegroundColor Gray
Write-Host "`nAUTOMATIC CONSOLIDATION (replaces session-end):" -ForegroundColor White
Write-Host "  Run: powershell -File `"$SkillPath\hooks\periodic-consolidation.ps1`" -ConfigPath `"$ConfigPath`" -BinaryPath `"$BinaryPath`"" -ForegroundColor Gray
Write-Host "  Triggers: every 5 interactions -> morphogenesis, every 20 -> dream, every 3 -> federation" -ForegroundColor Gray
Write-Host "  Add to your AI agent's post-tool-use or periodic timer" -ForegroundColor Gray