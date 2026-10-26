<#>
.SYNOPSIS
    Microscope Memory - User Prompt Submit Hook
.DESCRIPTION
    Runs when user submits a prompt. Analyzes intent and runs recall for relevant context.
#>

param(
    [string]$Prompt = "",
    [string]$ConfigPath = "C:/Users/mater/AppData/Local/microscope-memory/config.toml",
    [string]$BinaryPath = "C:/Users/mater/microscope-memory-clone/target/debug/microscope-mem.exe"
)

$env:MICROSCOPE_CONFIG = $ConfigPath

function Run-Microscope {
    param([string]$Args)
    & $BinaryPath $Args
}

# Analyze prompt for memory relevance
$PromptLower = $Prompt.ToLower()

# High relevance patterns
$HighRelevance = @(
    "emlékszel", "remember", "mit tudsz", "what do you know",
    "Máté", "Octopus", "HOPE", "Microscope", "Rongyász", "Silent Noise",
    "építés", "build", "hiba", "error", "architektúra", "architecture",
    "protokoll", "protocol", "memória", "memory", "recall", "store"
)

# Medium relevance patterns  
$MediumRelevance = @(
    "code", "kód", "script", "szkript", "function", "funkció",
    "class", "osztály", "module", "modul", "api", "endpoint",
    "test", "teszt", "debug", "log", "error", "exception"
)

$RelevanceScore = 0

foreach ($kw in $HighRelevance) {
    if ($PromptLower -like "*$kw*") { $RelevanceScore += 3 }
}
foreach ($kw in $MediumRelevance) {
    if ($PromptLower -like "*$kw*") { $RelevanceScore += 1 }
}

if ($RelevanceScore -ge 2) {
    # Run recall with k=5 for context
    $Results = Run-Microscope "recall `"$Prompt`" 5" 2>$null
    if ($Results -and $Results -notmatch "0 results") {
        Write-Host "[MICROSCOPE PROMPT] Context loaded (score: $RelevanceScore):" -ForegroundColor Cyan
        Write-Host $Results
    }
} elseif ($RelevanceScore -eq 1) {
    # Light recall k=3
    $Results = Run-Microscope "recall `"$Prompt`" 3" 2>$null
    if ($Results -and $Results -notmatch "0 results") {
        Write-Host "[MICROSCOPE PROMPT] Light context (score: $RelevanceScore):" -ForegroundColor Gray
        Write-Host $Results
    }
}