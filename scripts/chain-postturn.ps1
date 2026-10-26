#!/usr/bin/env pwsh
# ============================================================================
# chain-postturn.ps1 - Chain's PostTurn hook for Microscope Memory (C: clone)
# ============================================================================
# Usage:
#   .\chain-postturn.ps1 "what happened" [importance]
# ============================================================================

$ErrorActionPreference = "Stop"

$Summary = $args[0]
if (-not $Summary) { Write-Error "Usage: chain-postturn.ps1 <summary> [importance]"; exit 1 }
$Importance = if ($args[1]) { [int]$args[1] } else { 5 }

# Force correct binary for THIS clone
$bin = "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe"
if (-not (Test-Path $bin)) { Write-Error "microscope-mem.exe not found at $bin"; exit 1 }

Write-Host "[STORE] Persisting interaction (importance: $Importance)..." -ForegroundColor Cyan

# Store in session layer
$stored = & $bin "store" $Summary "-l" "session" "-i" $Importance.ToString()
Write-Host "Stored: $($stored.Trim())" -ForegroundColor Green

# Also store structured summary in long_term for future recall
$structured = "Interaction: $Summary"
& $bin "store" $structured "-l" "long_term" "-i" "4" | Out-Null

Write-Host "[STORED] Memory consolidated." -ForegroundColor Cyan

exit 0