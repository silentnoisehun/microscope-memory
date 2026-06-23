#!/usr/bin/env pwsh
# ============================================================================
# microscope-recall-hook.ps1 - Universal LLM hook for Microscope Memory
# ============================================================================
#
# Drop this into any LLM client's hook system. Tested with:
#   - Claude Code (.claude/settings.json -> UserPromptSubmit / Stop)
#
# The hook stores the prompt before each query (crash-safe), recalls relevant
# memories, and injects them into the context. On stop, persists the full
# assistant transcript to long-term memory.
#
# --- Parameters -------------------------------------------------------------
#
#   -Action  Hook type: UserPromptSubmit | Stop
#            If omitted, falls back to $env:CLAUDE_HOOK_TYPE
#
# --- Environment variables (all optional) -----------------------------------
#
#   MICROSCOPE_BIN         Full path to microscope-mem executable
#   MICROSCOPE_HOME        Project root (binary: $HOME\target\release\microscope-mem.exe)
#   MICROSCOPE_CFG         Path to config.toml (default: ./config.toml)
#   MICROSCOPE_RECALL_K    Number of recall hits (default: 5)
#   MICROSCOPE_QUIET       "1" or "true" to suppress stderr (default: false)
#
# --- Resolution order for the binary ----------------------------------------
#   1. $MICROSCOPE_BIN
#   2. $MICROSCOPE_HOME\target\release\microscope-mem.exe
#   3. Get-Command microscope-mem.exe
#   4. <script dir>\..\target\release\microscope-mem.exe
#   5. <script dir>\..\..\target\release\microscope-mem.exe
#
# --- Exit codes -------------------------------------------------------------
#   0  always - hooks must never break the host.
# ============================================================================

[CmdletBinding()]
param([string]$Action = "")

$ErrorActionPreference = "SilentlyContinue"
$Global:MicroscopeQuiet = $env:MICROSCOPE_QUIET -eq "1" -or $env:MICROSCOPE_QUIET -eq "true"

# --- Resolve binary path ----------------------------------------------------
function Resolve-MicroscopeBin {
    if ($env:MICROSCOPE_BIN) { return $env:MICROSCOPE_BIN }
    $home = $env:MICROSCOPE_HOME
    if ($home) {
        $p = "$home\target\release\microscope-mem.exe"
        if (Test-Path $p) { return $p }
    }
    $fromPath = (Get-Command "microscope-mem.exe" -ErrorAction SilentlyContinue).Source
    if ($fromPath) { return $fromPath }
    $scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
    $candidates = @(
        "$scriptDir\..\target\release\microscope-mem.exe"
        "$scriptDir\..\..\target\release\microscope-mem.exe"
    )
    foreach ($c in $candidates) { if (Test-Path $c) { return $c } }
    return $null
}

$memBin = Resolve-MicroscopeBin
if (-not $memBin) { exit 0 }

# --- Dispatch ---------------------------------------------------------------
$hookType = if ($Action) { $Action } else { $env:CLAUDE_HOOK_TYPE }

# ─── UserPromptSubmit: Store + Recall + Inject ───
if ($hookType -eq "UserPromptSubmit") {
    $raw = [Console]::In.ReadToEnd()
    if ([string]::IsNullOrWhiteSpace($raw)) { exit 0 }
    try { $jsonData = $raw | ConvertFrom-Json } catch { exit 0 }
    $prompt = $jsonData.prompt
    if ([string]::IsNullOrWhiteSpace($prompt) -or $prompt.Length -lt 3) { exit 0 }

    # 1. STORE: prompt mentése session memóriába (crash-safe)
    $storeText = if ($prompt.Length -gt 500) { $prompt.Substring(0, 500) } else { $prompt }
    & $memBin store $storeText -l session -i 5 2>&1 | Out-Null

    # 2. RECALL: releváns memóriák lekérése
    $recallK = if ($env:MICROSCOPE_RECALL_K) { [int]$env:MICROSCOPE_RECALL_K } else { 5 }
    $recallQuery = if ($prompt.Length -gt 200) { $prompt.Substring(0, 200) } else { $prompt }
    $result = & $memBin recall $recallQuery $recallK 2>&1 | Out-String

    # 3. INJECT: context beinjektálása a promptba
    if ($result -and $result.Trim().Length -gt 10) {
        Write-Output ""
        Write-Output "## Microscope Memory Context"
        Write-Output $result.Trim()
        Write-Output ""
    }
    exit 0
}

# ─── Stop: Store teljes session összefoglaló long_term-be ───
if ($hookType -eq "Stop") {
    $raw = [Console]::In.ReadToEnd()
    if ([string]::IsNullOrWhiteSpace($raw)) { exit 0 }
    try { $jsonData = $raw | ConvertFrom-Json } catch { exit 0 }
    $transcript = $jsonData.transcript
    if (-not $transcript -or $transcript.Count -eq 0) { exit 0 }
    $summary = ""
    foreach ($msg in $transcript) {
        if ($msg.role -eq "assistant" -and $msg.content) {
            $text = if ($msg.content -is [array]) {
                ($msg.content | Where-Object { $_.type -eq "text" } | ForEach-Object { $_.text }) -join "`n"
            } else { $msg.content }
            if ($text -and $text.Length -gt 10) { $summary += $text + "`n" }
        }
    }
    if ($summary.Length -gt 20) {
        if ($summary.Length -gt 2000) { $summary = $summary.Substring(0, 2000) }
        & $memBin store $summary -l long_term -i 6 2>&1 | Out-Null
    }
    exit 0
}

exit 0
