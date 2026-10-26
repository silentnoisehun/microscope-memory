#!/usr/bin/env pwsh
# ============================================================================
# auto-hook.ps1 - Robusztus auto-context/auto-store hook for Microscope Memory
# ============================================================================
# Called by Goose (AI) automatically every turn:
#   - Pre-turn:  recall relevant context (auto-context) - DIRECT SYNC CALLS
#   - Post-turn: store interaction summary (auto-store) - FIRE-AND-FORGET
#
# Features:
#   - Uses cmd.exe for reliable stdout+stderr capture
#   - Graceful degradation (try/catch each command)
#   - Fire-and-forget PostTurn (simple Start-Job)
#
# Usage:
#   .\auto-hook.ps1 -Action PreTurn  -Query "user question"
#   .\auto-hook.ps1 -Action PostTurn -Summary "what happened" -Importance 7
#
# Env vars:
#   MICROSCOPE_BIN      Path to microscope-mem.exe (auto-resolved)
#   MICROSCOPE_QUIET    "1" to suppress stderr
# ============================================================================

[CmdletBinding()]
param(
    [ValidateSet("PreTurn", "PostTurn")]
    [string]$Action,

    [string]$Query,           # PreTurn: user's question
    [string]$Summary,         # PostTurn: interaction summary
    [int]$Importance = 7,     # PostTurn: 5=routine, 6=decision, 7=milestone, 8=identity
    [string]$Layer = "long_term"  # PostTurn: target layer
)

$ErrorActionPreference = "SilentlyContinue"
$Global:Quiet = $env:MICROSCOPE_QUIET -eq "1" -or $env:MICROSCOPE_QUIET -eq "true"

# --- Resolve binary ---------------------------------------------------------
function Resolve-Bin {
    if ($env:MICROSCOPE_BIN) { return $env:MICROSCOPE_BIN }
    $candidates = @(
        "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe"
        "$env:USERPROFILE\microscope-memory-clone\target\release\microscope-mem.exe"
        (Get-Command "microscope-mem.exe" -ErrorAction SilentlyContinue).Source
    )
    foreach ($c in $candidates) { if (Test-Path $c) { return $c } }
    return $null
}

$bin = Resolve-Bin
if (-not $bin) { if (-not $Quiet) { Write-Error "microscope-mem.exe not found" } exit 0 }

# --- Keyword extraction -----------------------------------------------------
function Extract-Keywords {
    param([string]$Text)
    $stopwords = @("a","az","egy","és","vagy","hogy","van","volt","lesz","nem","de","mint","kis","nagy","mi","ti","ők","én","te","ő","ez","az","ezt","azt","itt","ott","ma","holnap","tegnap","van","vannak","is","meg","már","még","csak","már","még","what","how","why","when","where","who","the","a","an","and","or","but","to","for","of","in","on","with","my","your","our","their","this","that","these","those","i","you","he","she","it","we","they","me","him","her","us","them","am","is","are","was","were","be","been","being","have","has","had","do","does","did","will","would","could","should","may","might","must","can","shall")
    $words = $Text -split '\W+' | Where-Object { $_ -and $_.Length -gt 2 -and $_ -notin $stopwords }
    return ($words | Select-Object -First 7) -join " "
}

# --- Filter recall output ---------------------------------------------------
function Filter-RecallOutput {
    param([string]$Output)
    if (-not $Output -or $Output.Trim().Length -le 10) { return $null }
    
    $lines = $Output -split "`n"
    $cleanLines = @()
    $inMeta = $false
    
    foreach ($line in $lines) {
        $trimmed = $line.Trim()
        if ($trimmed -match '^(MIRROR|SELF|CURIOUS):') { $inMeta = $true; continue }
        if ($inMeta -and $trimmed -eq '') { $inMeta = $false; continue }
        if ($inMeta) { continue }
        
        if ($trimmed -match '^D[0-9] L2=') {
            if ($trimmed -match '\[session/(white|new)\].*THE-HOPE-LIBRARY') { continue }
            if ($trimmed -match '\[session/(white|new)\].*\[2026-\d{2}-\d{2}') { continue }
            if ($trimmed -match '\[session/(white|new)\].*ls: cannot access') { continue }
            if ($trimmed -match '\[session/(white|new)\].*NINCS workspace') { continue }
        }
        $cleanLines += $trimmed
    }
    
    if ($cleanLines.Count -gt 2) { return ($cleanLines -join "`n") }
    return $null
}

# --- Safe direct command execution via cmd.exe (captures stderr too) --------
function Invoke-Direct {
    param(
        [string]$Exe,
        [string[]]$Args,
        [string]$Context = ""
    )
    
    try {
        # Build command with proper escaping for cmd.exe
        $argStr = $Args -join " "
        $cmd = 'cmd /c "' + $Exe + '" ' + $argStr + ' 2>&1'
        $result = Invoke-Expression $cmd
        return $result
    } catch {
        if (-not $Quiet) { Write-Warning "$Context error: $_" }
        return $null
    }
}

# --- Enrichment functions (each independent, graceful failure) --------------
function Get-AutoContext {
    Invoke-Direct $bin @("auto-context", "--compact") -Context "auto-context" |
    ForEach-Object { if ($_ -and $_.Trim().Length -gt 10) { "## Auto-Context (session snapshot)`n$($_.Trim())" } }
}

function Get-Hottest {
    param([int]$Count = 3)
    Invoke-Direct $bin @("hottest", "$Count") -Context "hottest" |
    ForEach-Object {
        $clean = $_ -split "`n" | Where-Object {
            $_ -notmatch '^E=.*\[session/(white|new)\].*THE-HOPE-LIBRARY'
            $_ -notmatch '^E=.*\[session/(white|new)\].*\[2026-'
        }
        if ($clean.Count -gt 1) { "## Hottest blocks (activation energy)`n$($clean -join "`n")" }
    }
}

function Get-Curiosity {
    Invoke-Direct $bin @("curiosity") -Context "curiosity" |
    ForEach-Object { if ($_ -and $_.Trim().Length -gt 10) { "## Curiosity (what system wants to explore)`n$($_.Trim())" } }
}

function Get-Narrative {
    Invoke-Direct $bin @("narrative") -Context "narrative" |
    ForEach-Object { if ($_ -and $_.Trim().Length -gt 10) { "## Inner Narrative`n$($_.Trim())" } }
}

function Get-AbsentiaStatus {
    Invoke-Direct $bin @("absentia", "status") -Context "absentia status" |
    ForEach-Object { if ($_ -and $_.Trim().Length -gt 10) { "## Absentia (silence layer - what's missing)`n$($_.Trim())" } }
}

function Get-MorphogenesisStatus {
    Invoke-Direct $bin @("morphogenesis", "status") -Context "morphogenesis status" |
    ForEach-Object { if ($_ -and $_.Trim().Length -gt 10) { "## Morphogenesis (cognitive phase/gradient)`n$($_.Trim())" } }
}

function Get-EmotionalField {
    Invoke-Direct $bin @("emotional-field") -Context "emotional-field" |
    ForEach-Object { if ($_ -and $_.Trim().Length -gt 10) { "## Emotional Field`n$($_.Trim())" } }
}

function Get-WorkingMemory {
    Invoke-Direct $bin @("wm", "show") -Context "wm show" |
    ForEach-Object { if ($_ -and $_.Trim().Length -gt 10) { "## Working Memory (7±2 buffer)`n$($_.Trim())" } }
}

function Get-Patterns {
    Invoke-Direct $bin @("patterns") -Context "patterns" |
    ForEach-Object { if ($_ -and $_.Trim().Length -gt 10) { "## Thought Patterns (crystallized)`n$($_.Trim())" } }
}

# --- PRE-TURN: Auto-context (recall + enrichment) ---------------------------
if ($Action -eq "PreTurn") {
    if ([string]::IsNullOrWhiteSpace($Query)) { exit 0 }

    $keywords = Extract-Keywords $Query
    if ([string]::IsNullOrWhiteSpace($keywords)) { exit 0 }

    $results = @()

    # 0. AUTO-CONTEXT: Universal session snapshot (always include)
    $autoCtx = Get-AutoContext
    if ($autoCtx) { $results += $autoCtx }

    # 1. FIND: exact text matches (high precision, no session noise)
    $findOut = Invoke-Direct $bin @("find", $keywords, "8") -Context "find"
    if ($findOut -and $findOut.Trim().Length -gt 10) {
        $results += "## Exact matches (find)`n$($findOut.Trim())"
    }

    # 2. RECALL: semantic search with layer filtering
    $recallQuery = $keywords
    $recallOut = Invoke-Direct $bin @("recall", $recallQuery, "8") -Context "recall"
    $filtered = Filter-RecallOutput $recallOut
    if ($filtered) {
        $results += "## Semantic recall (filtered)`n$filtered"
    }

    # 3. ENRICHMENT: Cognitive state (each independent - one failure doesn't stop others)
    $enrichments = @(
        (Get-Hottest 3),
        (Get-Curiosity),
        (Get-Narrative),
        (Get-AbsentiaStatus),
        (Get-MorphogenesisStatus),
        (Get-EmotionalField),
        (Get-WorkingMemory),
        (Get-Patterns)
    )
    foreach ($e in $enrichments) { if ($e) { $results += $e } }

    # Output for injection (even if only partial results)
    if ($results.Count -gt 0) {
        Write-Output ""
        Write-Output "### 🧠 Microscope Memory Context"
        Write-Output ($results -join "`n`n")
        Write-Output ""
    }
    exit 0
}

# --- POST-TURN: Auto-store (fire-and-forget, non-blocking) ------------------
if ($Action -eq "PostTurn") {
    if ([string]::IsNullOrWhiteSpace($Summary)) { exit 0 }
    if ($Summary.Length -gt 2000) { $Summary = $Summary.Substring(0, 2000) }

    # Fire-and-forget: simple Start-Job for store only
    $storeJob = Start-Job -ScriptBlock {
        param($bin, $Summary, $Layer, $Importance)
        $ErrorActionPreference = "SilentlyContinue"
        & $bin store $Summary -l $Layer -i $Importance 2>&1 | Out-Null
    } -ArgumentList $bin, $Summary, $Layer, $Importance
    
    # Don't wait - just register and exit immediately
    exit 0
}

exit 0