#!/usr/bin/env pwsh
# ============================================================================
# chain-preturn.ps1 - Chain's PreTurn hook for Microscope Memory (C: clone)
# ============================================================================
# Usage:
#   .\chain-preturn.ps1 "user question"
# ============================================================================

$ErrorActionPreference = "Stop"

$Query = $args[0]
if (-not $Query) { Write-Error "Usage: chain-preturn.ps1 <query>"; exit 1 }

# Force correct binary for THIS clone
$bin = "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe"
if (-not (Test-Path $bin)) { Write-Error "microscope-mem.exe not found at $bin"; exit 1 }

function Invoke-Microscope {
    param(
        [string]$Command,
        [Parameter(ValueFromRemainingArguments=$true)]
        [string[]]$Args
    )
    & $bin $Command @Args
}

Write-Host "[RECALL] Retrieving relevant memories for: $Query" -ForegroundColor Cyan

# 1. Auto-context (session snapshot)
Write-Host "## Auto-Context (session snapshot)" -ForegroundColor Green
& $bin "auto-context" "--compact"
Write-Host ""

# 2. Semantic recall on user query
Write-Host "## Relevant Memories for: $Query" -ForegroundColor Green
& $bin "recall" $Query "5"
Write-Host ""

# 3. Hottest blocks (active cognitive traces)
Write-Host "## Hottest Cognitive Traces" -ForegroundColor Yellow
& $bin "hottest" "3"
Write-Host ""

# 4. Working memory (7±2 buffer)
Write-Host "## Working Memory" -ForegroundColor Magenta
& $bin "wm" "show"
Write-Host ""

Write-Host "[READY] Context loaded." -ForegroundColor Cyan

exit 0