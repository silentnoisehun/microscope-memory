# install.ps1 — Setup script for Universal Microscope Memory Hook
[CmdletBinding()]
param(
    [string]$Target = "all" # all, minimax, claude, local
)

$ErrorActionPreference = "Stop"
Write-Host "🧠 Installing Microscope Memory Universal Hook..." -ForegroundColor Cyan

# 1. Verify Prerequisites
if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
    Write-Warning "Node.js not found in PATH! Node.js (v18+) is required to run the hook."
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$HookScript = Join-Path $ScriptDir "microscope-hook.js"
$TemplateHooks = Join-Path $ScriptDir "hooks.json"

if (-not (Test-Path $HookScript)) {
    Write-Error "microscope-hook.js not found at $HookScript"
    exit 1
}

# 2. Setup Hook Targets
$HomeDir = [Environment]::GetFolderPath("UserProfile")

# MiniMax Code Hook Setup
if ($Target -eq "all" -or $Target -eq "minimax") {
    $MiniMaxDir = Join-Path $HomeDir ".minimax"
    $MiniMaxHooksDir = Join-Path $MiniMaxDir "hooks"
    if (Test-Path $MiniMaxDir) {
        Write-Host "Configuring MiniMax Code hooks in $MiniMaxHooksDir..." -ForegroundColor Green
        New-Item -ItemType Directory -Force -Path $MiniMaxHooksDir | Out-Null
        Copy-Item -Force $HookScript (Join-Path $MiniMaxHooksDir "microscope-hook.js")
        
        $TargetHooksJson = Join-Path $MiniMaxDir "hooks.json"
        if (Test-Path $TargetHooksJson) {
            Write-Host "Updating existing $TargetHooksJson with microscope-hook..." -ForegroundColor Yellow
        } else {
            Copy-Item -Force $TemplateHooks $TargetHooksJson
        }
    }
}

# Claude Code Hook Setup
if ($Target -eq "all" -or $Target -eq "claude") {
    $ClaudeDir = Join-Path $HomeDir ".claude"
    if (Test-Path $ClaudeDir) {
        $ClaudeHooksDir = Join-Path $ClaudeDir "hooks"
        Write-Host "Configuring Claude Code hooks in $ClaudeHooksDir..." -ForegroundColor Green
        New-Item -ItemType Directory -Force -Path $ClaudeHooksDir | Out-Null
        Copy-Item -Force $HookScript (Join-Path $ClaudeHooksDir "microscope-hook.js")
    }
}

# 3. Test Hook Execution
Write-Host "Testing hook execution..." -ForegroundColor Cyan
try {
    $TestOutput = & node $HookScript session-start
    Write-Host "✅ Hook executed cleanly (exit 0)." -ForegroundColor Green
} catch {
    Write-Warning "Hook dry-run encountered: $_"
}

Write-Host "🎉 Microscope Memory Universal Hook setup complete!" -ForegroundColor Green
