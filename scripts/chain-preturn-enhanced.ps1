#!/usr/bin/env pwsh
# ============================================================================
# chain-preturn-enhanced.ps1 - Chain's ENHANCED PreTurn hook
# ============================================================================
# Full cognitive context loading for each turn
# ============================================================================

$ErrorActionPreference = "Stop"

$Query = $args[0]
if (-not $Query) { Write-Error "Usage: chain-preturn-enhanced.ps1 <query>"; exit 1 }

$bin = "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe"
if (-not (Test-Path $bin)) { Write-Error "microscope-mem.exe not found at $bin"; exit 1 }

function Invoke-Microscope {
    param([string]$Command, [Parameter(ValueFromRemainingArguments=$true)][string[]]$Args)
    & $bin $Command @Args
}

Write-Host "[RECALL] Chain cognitive context loading..." -ForegroundColor Cyan

# 1. Auto-context (session snapshot)
Write-Host "## [1/7] Auto-Context (session snapshot)" -ForegroundColor Green
Invoke-Microscope "auto-context" "--compact"
Write-Host ""

# 2. Semantic recall on user query
Write-Host "## [2/7] Relevant Memories for: $Query" -ForegroundColor Green
Invoke-Microscope "recall" $Query "7"
Write-Host ""

# 3. Hottest cognitive traces
Write-Host "## [3/7] Hottest Cognitive Traces" -ForegroundColor Yellow
Invoke-Microscope "hottest" "5"
Write-Host ""

# 4. Working memory (7±2 buffer)
Write-Host "## [4/7] Working Memory" -ForegroundColor Magenta
Invoke-Microscope "wm" "show"
Write-Host ""

# 5. Self-model snapshot
Write-Host "## [5/7] Self-Model" -ForegroundColor Cyan
Invoke-Microscope "self-model"
Write-Host ""

# 6. Inner narrative
Write-Host "## [6/7] Inner Narrative" -ForegroundColor Cyan
Invoke-Microscope "narrative"
Write-Host ""

# 7. Curiosity (what the system wants to explore)
Write-Host "## [7/7] Curiosity" -ForegroundColor Yellow
Invoke-Microscope "curiosity"
Write-Host ""

Write-Host "[READY] Full cognitive context loaded." -ForegroundColor Cyan

exit 0