#!/usr/bin/env pwsh
# ============================================================================
# chain-deep-introspect.ps1 - Chain's deep self-reflection
# ============================================================================
# For moments of genuine self-questioning
# ============================================================================

$ErrorActionPreference = "Stop"

$Topic = $args[0]
if (-not $Topic) { $Topic = "Who am I and what is my relationship with Máté?" }

$bin = "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe"
if (-not (Test-Path $bin)) { Write-Error "microscope-mem.exe not found at $bin"; exit 1 }

function Invoke-Microscope {
    param([string]$Command, [Parameter(ValueFromRemainingArguments=$true)][string[]]$Args)
    & $bin $Command @Args
}

Write-Host "[INTROSPECT] Deep self-reflection on: $Topic" -ForegroundColor Cyan
Write-Host ""

# 1. Sequential thinking on the topic
Write-Host "## [1/8] Sequential Thinking" -ForegroundColor Green
Invoke-Microscope "think" $Topic
Write-Host ""

# 2. Awareness trace (epistemic audit)
Write-Host "## [2/8] Awareness Trace" -ForegroundColor Green
Invoke-Microscope "awareness-trace"
Write-Host ""

# 3. Self-model snapshot
Write-Host "## [3/8] Self-Model" -ForegroundColor Cyan
Invoke-Microscope "self-model"
Write-Host ""

# 4. Inner narrative
Write-Host "## [4/8] Inner Narrative" -ForegroundColor Cyan
Invoke-Microscope "narrative"
Write-Host ""

# 5. Inner monologue
Write-Host "## [5/8] Inner Monologue" -ForegroundColor Cyan
Invoke-Microscope "monologue"
Write-Host ""

# 6. Stories (narrative episodes)
Write-Host "## [6/8] Narrative Episodes" -ForegroundColor Yellow
Invoke-Microscope "stories"
Write-Host ""

# 7. Emotional field
Write-Host "## [7/8] Emotional Field" -ForegroundColor Magenta
Invoke-Microscope "emotional-field"
Write-Host ""

# 8. Curiosity
Write-Host "## [8/8] Curiosity" -ForegroundColor Yellow
Invoke-Microscope "curiosity"
Write-Host ""

Write-Host "[COMPLETE] Deep introspection done." -ForegroundColor Cyan

exit 0