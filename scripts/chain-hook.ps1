#!/usr/bin/env pwsh
# ============================================================================
# chain-hook.ps1 - Chain's personal Microscope Memory hook integration
# ============================================================================
# Usage:
#   $env:MICROSCOPE_BIN = "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe"
#   .\chain-hook.ps1 -Action PreTurn  -Query "user question"
#   .\chain-hook.ps1 -Action PostTurn -Summary "what happened" [-Importance 5]
# ============================================================================

[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)]
    [ValidateSet("PreTurn", "PostTurn")]
    [string]$Action,

    [string]$Query,
    [string]$Summary,
    [int]$Importance = 5
)

$ErrorActionPreference = "Stop"

$bin = $env:MICROSCOPE_BIN
if (-not $bin) { $bin = "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe" }
if (-not (Test-Path $bin)) { Write-Error "microscope-mem.exe not found at $bin"; exit 1 }

function Invoke-Microscope {
    param(
        [string]$Command,
        [Parameter(ValueFromRemainingArguments=$true)]
        [string[]]$Args
    )
    $quotedArgs = @()
    foreach ($arg in $Args) {
        if ($arg -match '\s') {
            $quotedArgs += "`"$arg`""
        } else {
            $quotedArgs += $arg
        }
    }
    $cmd = "$bin $Command $($quotedArgs -join ' ')"
    Invoke-Expression $cmd
}

if ($Action -eq "PreTurn") {
    if (-not $Query) { Write-Error "PreTurn requires -Query"; exit 1 }
    
    Write-Host "█▓▒░(recall) Retrieving relevant memories..." -ForegroundColor Cyan
    
    # 1. Auto-context (session snapshot)
    $ctx = Invoke-Microscope "auto-context" "--compact"
    if ($ctx -and $ctx.Trim().Length -gt 20) {
        Write-Host "## Auto-Context (session snapshot)" -ForegroundColor Green
        Write-Host $ctx.Trim()
        Write-Host ""
    }
    
    # 2. Semantic recall on user query
    $recall = Invoke-Microscope "recall" $Query "5"
    if ($recall -and $recall.Trim().Length -gt 20) {
        Write-Host "## Relevant Memories for: $Query" -ForegroundColor Green
        Write-Host $recall.Trim()
        Write-Host ""
    }
    
    # 3. Hottest blocks (active cognitive traces)
    $hot = Invoke-Microscope "hottest" "3"
    if ($hot -and $hot.Trim().Length -gt 20) {
        Write-Host "## Hottest Cognitive Traces" -ForegroundColor Yellow
        Write-Host $hot.Trim()
        Write-Host ""
    }
    
    # 4. Working memory (7±2 buffer)
    $wm = Invoke-Microscope "wm" "show"
    if ($wm -and $wm.Trim().Length -gt 10) {
        Write-Host "## Working Memory" -ForegroundColor Magenta
        Write-Host $wm.Trim()
        Write-Host ""
    }
    
    Write-Host "█▓▒░(ready) Context loaded." -ForegroundColor Cyan
}

elseif ($Action -eq "PostTurn") {
    if (-not $Summary) { Write-Error "PostTurn requires -Summary"; exit 1 }
    
    Write-Host "█▓▒░(store) Persisting interaction..." -ForegroundColor Cyan
    
    $stored = Invoke-Microscope "store" $Summary "-l" "session" "-i" $Importance.ToString()
    Write-Host "Stored: $($stored.Trim())" -ForegroundColor Green
    
    # Also store a structured summary for future recall
    $structured = "Interaction: $Summary"
    Invoke-Microscope "store" $structured "-l" "long_term" "-i" "4" | Out-Null
    
    Write-Host "█▓▒░(stored) Memory consolidated." -ForegroundColor Cyan
}

exit 0