<#>
.SYNOPSIS
    Microscope Memory - Post Tool Use Hook
.DESCRIPTION
    Runs after tool execution. Stores new facts, triggers reconsolidation, runs morphogenesis periodically.
#>

param(
    [string]$ToolOutput = "",
    [string]$ConfigPath = "C:/Users/mater/AppData/Local/microscope-memory/config.toml",
    [string]$BinaryPath = "C:/Users/mater/microscope-memory-clone/target/debug/microscope-mem.exe",
    [double]$StoreThreshold = 0.7,
    [int]$MorphogenesisInterval = 5
)

$env:MICROSCOPE_CONFIG = $ConfigPath

function Run-Microscope {
    param([string]$Args)
    & $BinaryPath $Args
}

# Counter for morphogenesis interval (persisted in temp file)
$CounterFile = "$env:TEMP\microscope_morpho_counter.txt"
$Counter = 0
if (Test-Path $CounterFile) {
    $Counter = [int](Get-Content $CounterFile -Raw)
}
$Counter++
Set-Content $CounterFile -Value $Counter

# 1. Analyze tool output for storable facts
$Facts = @()

# Patterns that indicate new knowledge worth storing
if ($ToolOutput -match "(?i)(fixed|javított|solved|megoldott|implemented|implementált|added|hozzáadva|created|létrehozva|discovered|r탐|found|talált|learned|tanult|understood|értett)") {
    # Extract the relevant sentence(s) - simplified: store last 500 chars
    $Fact = $ToolOutput.Substring([Math]::Max(0, $ToolOutput.Length - 500))
    $Facts += @{ Text = $Fact.Trim(); Layer = "session"; Importance = 5 }
}

if ($ToolOutput -match "(?i)(error|hiba|exception|failed|sikertelen|crash|assert|panic)") {
    $Fact = $ToolOutput.Substring([Math]::Max(0, $ToolOutput.Length - 500))
    $Facts += @{ Text = "ERROR CONTEXT: $Fact"; Layer = "reflections"; Importance = 7 }
}

if ($ToolOutput -match "(?i)(architecture|architektúra|protocol|protokoll|design|tervezés|decision|döntés|choice|választás)") {
    $Fact = $ToolOutput.Substring([Math]::Max(0, $ToolOutput.Length - 500))
    $Facts += @{ Text = "DESIGN DECISION: $Fact"; Layer = "long_term"; Importance = 8 }
}

# Store facts
foreach ($Fact in $Facts) {
    if ($Fact.Text.Length -gt 50) {
        $Cmd = "store `"$($Fact.Text)`" -l $($Fact.Layer) -i $($Fact.Importance)"
        & $BinaryPath $Cmd 2>$null | Out-Null
        Write-Host "[MICROSCOPE POST-TOOL] Stored: $($Fact.Layer)/imp=$($Fact.Importance)" -ForegroundColor Green
    }
}

# 2. Trigger reconsolidation after each recall (spatial drift + emotion blend)
& $BinaryPath "reconsolidate" 2>$null | Out-Null
Write-Host "[MICROSCOPE POST-TOOL] Reconsolidation triggered" -ForegroundColor Cyan

# 3. Periodic morphogenesis
if ($Counter % $MorphogenesisInterval -eq 0) {
    & $BinaryPath "morphogenesis run" 2>$null | Out-Null
    Write-Host "[MICROSCOPE POST-TOOL] Morphogenesis cycle run (counter: $Counter)" -ForegroundColor Magenta
}

# 4. Federation pulse exchange (every 3rd tool use)
if ($Counter % 3 -eq 0) {
    & $BinaryPath "pulse-exchange" 2>$null | Out-Null
    & $BinaryPath "integrate" 2>$null | Out-Null
    Write-Host "[MICROSCOPE POST-TOOL] Federation sync" -ForegroundColor Yellow
}