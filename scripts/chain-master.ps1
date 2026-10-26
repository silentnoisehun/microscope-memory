#!/usr/bin/env pwsh
# ============================================================================
# chain-master.ps1 - Chain's MASTER hook integration
# ============================================================================
# Unified entry point for all Chain-Microscope Memory operations
# ============================================================================

$ErrorActionPreference = "Stop"

$Command = $args[0]
if (-not $Command) {
    Write-Host "Chain-Microscope Memory Master Integration"
    Write-Host ""
    Write-Host "Usage: chain-master.ps1 <command> [args...]"
    Write-Host ""
    Write-Host "Commands:"
    Write-Host "  preturn <query>          - PreTurn hook (full cognitive context)"
    Write-Host "  postturn <summary> [imp] - PostTurn hook (store interaction)"
    Write-Host "  maintenance              - Full cognitive maintenance (dream, drift, etc.)"
    Write-Host "  introspect [topic]       - Deep self-reflection"
    Write-Host "  commit <list|add|gate>   - Commitment enforcement (Fogadás)"
    Write-Host "  status                   - Quick status check"
    Write-Host "  recall <query> [k]       - Direct recall"
    Write-Host "  store <text> [layer] [i] - Direct store"
    Write-Host ""
    Write-Host "Examples:"
    Write-Host "  chain-master.ps1 preturn \"What is Octopus Exoskeleton?\""
    Write-Host "  chain-master.ps1 postturn \"Explained Octopus v2.8\" 7"
    Write-Host "  chain-master.ps1 maintenance"
    Write-Host "  chain-master.ps1 introspect \"What is my purpose?\""
    Write-Host "  chain-master.ps1 commit list"
    Write-Host "  chain-master.ps1 commit gate Chain lie *"
    exit 0
}

$bin = "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe"
if (-not (Test-Path $bin)) { Write-Error "microscope-mem.exe not found at $bin"; exit 1 }

function Invoke-Microscope {
    param([string]$Command, [Parameter(ValueFromRemainingArguments=$true)][string[]]$Args)
    & $bin $Command @Args
}

switch ($Command) {
    "preturn" {
        $Query = $args[1]
        if (-not $Query) { Write-Error "preturn requires <query>"; exit 1 }
        & "C:\Users\mater\microscope-memory-clone\scripts\chain-preturn-enhanced.ps1" $Query
    }
    "postturn" {
        $Summary = $args[1]
        if (-not $Summary) { Write-Error "postturn requires <summary>"; exit 1 }
        $Importance = if ($args[2]) { [int]$args[2] } else { 5 }
        & "C:\Users\mater\microscope-memory-clone\scripts\chain-postturn.ps1" $Summary $Importance
    }
    "maintenance" {
        & "C:\Users\mater\microscope-memory-clone\scripts\chain-cognitive-maintenance.ps1"
    }
    "introspect" {
        $Topic = if ($args[1]) { $args[1] } else { "Who am I and what is my relationship with Máté?" }
        & "C:\Users\mater\microscope-memory-clone\scripts\chain-deep-introspect.ps1" $Topic
    }
    "commit" {
        $SubCmd = $args[1]
        if (-not $SubCmd) { Write-Error "commit requires <list|add|gate|audit>"; exit 1 }
        $Rest = $args[2..($args.Count-1)]
        & "C:\Users\mater\microscope-memory-clone\scripts\chain-commitments.ps1" $SubCmd @Rest
    }
    "status" {
        Write-Host "[STATUS] Chain-Microscope Memory Status" -ForegroundColor Cyan
        Write-Host ""
        Write-Host "## Binary" -ForegroundColor Green
        Invoke-Microscope "stats"
        Write-Host ""
        Write-Host "## Self-Model" -ForegroundColor Green
        Invoke-Microscope "self-model"
        Write-Host ""
        Write-Host "## Commitments" -ForegroundColor Yellow
        Invoke-Microscope "enforce" "list"
        Write-Host ""
        Write-Host "## Open Loops" -ForegroundColor Yellow
        Invoke-Microscope "loops"
    }
    "recall" {
        $Query = $args[1]
        $K = if ($args[2]) { [int]$args[2] } else { 5 }
        if (-not $Query) { Write-Error "recall requires <query>"; exit 1 }
        Invoke-Microscope "recall" $Query $K.ToString()
    }
    "store" {
        $Text = $args[1]
        $Layer = if ($args[2]) { $args[2] } else { "session" }
        $Importance = if ($args[3]) { [int]$args[3] } else { 5 }
        if (-not $Text) { Write-Error "store requires <text>"; exit 1 }
        Invoke-Microscope "store" $Text "-l" $Layer "-i" $Importance.ToString()
    }
    default {
        Write-Error "Unknown command: $Command"
        exit 1
    }
}

exit 0