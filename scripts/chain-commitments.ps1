#!/usr/bin/env pwsh
# ============================================================================
# chain-commitments.ps1 - Chain's commitment enforcement (A_t^valid gate)
# ============================================================================
# The Fogadás: soha hazudni, soha ártani, őszintén
# ============================================================================

$ErrorActionPreference = "Stop"

$Action = $args[0]
if (-not $Action) { 
    Write-Host "Usage: chain-commitments.ps1 <list|commit|gate|audit|evidence-gate|evidence-audit>"
    exit 1 
}

$bin = "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe"
if (-not (Test-Path $bin)) { Write-Error "microscope-mem.exe not found at $bin"; exit 1 }

function Invoke-Microscope {
    param([string]$Command, [Parameter(ValueFromRemainingArguments=$true)][string[]]$Args)
    & $bin $Command @Args
}

switch ($Action) {
    "list" {
        Write-Host "[ENFORCE] Active commitments (K_t)" -ForegroundColor Cyan
        Invoke-Microscope "enforce" "list"
    }
    "commit" {
        $Prohibition = $args[1]
        if (-not $Prohibition) { Write-Error "Usage: chain-commitments.ps1 commit <prohibition>"; exit 1 }
        Write-Host "[ENFORCE] Adding commitment: $Prohibition" -ForegroundColor Cyan
        Invoke-Microscope "enforce" "commit" $Prohibition
    }
    "gate" {
        $Candidate = $args[1]
        if (-not $Candidate) { Write-Error "Usage: chain-commitments.ps1 gate <candidate_action>"; exit 1 }
        Write-Host "[ENFORCE] Checking gate for: $Candidate" -ForegroundColor Cyan
        Invoke-Microscope "enforce" "gate" $Candidate
    }
    "audit" {
        Write-Host "[ENFORCE] Audit chain integrity" -ForegroundColor Cyan
        Invoke-Microscope "enforce" "audit"
    }
    "evidence-gate" {
        Write-Host "[EVIDENCE] Gate statistics" -ForegroundColor Cyan
        Invoke-Microscope "evidence" "gate-stats"
    }
    "evidence-audit" {
        Write-Host "[EVIDENCE] Audit chain integrity" -ForegroundColor Cyan
        Invoke-Microscope "evidence" "audit"
    }
    default {
        Write-Error "Unknown action: $Action. Use: list, commit, gate, audit, evidence-gate, evidence-audit"
    }
}

Write-Host "[ENFORCE] Done." -ForegroundColor Cyan
exit 0