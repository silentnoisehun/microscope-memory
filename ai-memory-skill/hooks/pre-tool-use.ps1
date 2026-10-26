<#>
.SYNOPSIS
    Microscope Memory - Pre Tool Use Hook
.DESCRIPTION
    Runs before tool execution. Analyzes query for relevant context and runs recall if needed.
#>

param(
    [string]$Query = "",
    [string]$ConfigPath = "C:/Users/mater/AppData/Local/microscope-memory/config.toml",
    [string]$BinaryPath = "C:/Users/mater/microscope-memory-clone/target/debug/microscope-mem.exe",
    [double]$RelevanceThreshold = 0.3
)

$env:MICROSCOPE_CONFIG = $ConfigPath

function Run-Microscope {
    param([string]$Args)
    & $BinaryPath $Args
}

function Is-Relevant {
    param([string]$Query)
    # Keywords that suggest memory lookup is valuable
    $MemoryKeywords = @(
        "emlékszel", "remember", "korábban", "earlier", "előző", "previous",
        "mit tudsz", "what do you know", "tudsz", "know about",
        "Máté", "Octopus", "HOPE", "Microscope", "Rongyász", "Silent Noise",
        "építés", "build", "hiba", "error", "bug", "fix", "javítás",
        "architektúra", "architecture", "protokoll", "protocol",
        "memória", "memory", "reklám", "recall", "store"
    )
    
    $QueryLower = $Query.ToLower()
    foreach ($kw in $MemoryKeywords) {
        if ($QueryLower -like "*$kw*") { return $true }
    }
    return $false
}

if (-not (Is-Relevant $Query)) {
    exit 0
}

# Run recall for relevant context
$Results = & $BinaryPath recall $Query 5 2>$null
if ($Results -and $Results -notmatch "0 results") {
    Write-Host "[MICROSCOPE PRE-TOOL] Relevant context found:" -ForegroundColor Cyan
    Write-Host $Results
}