<#>
.SYNOPSIS
    Microscope Memory - Session Start Hook
.DESCRIPTION
    Claude Code SessionStart hook. Loads cognitive context from the
    Microscope Memory binary index at the start of every session:
    identity layer, recent session summaries, and standing orders.
    Emits as "additionalContext" so the model receives it before the
    first reply. Fails silent (exit 0) on any error - never blocks.
#>

$ErrorActionPreference = "SilentlyContinue"

$ConfigPath = "C:/Users/mater/AppData/Local/microscope-memory/config.toml"
$BinaryPath = "C:/Users/mater/microscope-memory-clone/target/release/microscope-mem.exe"

if (-not (Test-Path $BinaryPath)) { exit 0 }
if (-not (Test-Path $ConfigPath)) { exit 0 }

$env:MICROSCOPE_CONFIG = $ConfigPath

$parts = @()

# 1. Standing orders + identity (high precision: find, not recall)
$recall = & $BinaryPath find "STANDING ORDER" 2>$null
if ($LASTEXITCODE -eq 0 -and $recall) { $parts += $recall }

# 2. Most recent session summary
$recent = & $BinaryPath find "SESSION CONTEXT" 2>$null
if ($LASTEXITCODE -eq 0 -and $recent) { $parts += $recent }

if ($parts.Count -eq 0) { exit 0 }

$context = "[MICROSCOPE MEMORY - auto-loaded session context]`n" + ($parts -join "`n")

# Claude Code hook JSON protocol: stdout as additionalContext
$json = @{
    hookSpecificOutput = @{
        hookEventName     = "SessionStart"
        additionalContext = $context
    }
} | ConvertTo-Json -Depth 4 -Compress

Write-Output $json
exit 0
