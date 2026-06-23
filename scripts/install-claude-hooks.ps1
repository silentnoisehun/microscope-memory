# Generate .claude/settings.json with the current project path baked in.
# Usage:
#   pwsh scripts/install-claude-hooks.ps1
#   bash scripts/install-claude-hooks.sh
#
# This is the right way to ship a "universal" hook integration: the settings.json
# in the repo contains a literal placeholder; running this script (or install.sh /
# install.ps1 with -Hooks flag) replaces it with your real project path.
#
# Why a generated settings.json (and not a hardcoded one in the repo):
#   - The repo's settings.json can't have hardcoded user paths
#   - But hooks in Claude Code need an absolute path to the script
#   - So we generate it locally on first install, and gitignore it
#
# Override defaults with env vars:
#   MICROSCOPE_CLAUDE_DIR  - target directory (default: ./.claude)
#   MICROSCOPE_HOOK_SCRIPT - hook script path (default: ./scripts/microscope-recall-hook.ps1)
param(
    [string]$ClaudeDir = $env:MICROSCOPE_CLAUDE_DIR,
    [string]$HookScript = $env:MICROSCOPE_HOOK_SCRIPT,
    [switch]$Uninstall
)

$ErrorActionPreference = "Stop"
$ScriptDir = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Definition }
$RepoRoot  = (Resolve-Path (Join-Path $ScriptDir "..")).Path

if (-not $ClaudeDir) { $ClaudeDir = Join-Path $RepoRoot ".claude" }
if (-not $HookScript) {
    $HookScript = Join-Path $ScriptDir "microscope-recall-hook.ps1"
}

$settingsPath = Join-Path $ClaudeDir "settings.json"
$hookAbs = (Resolve-Path $HookScript -ErrorAction SilentlyContinue)
if (-not $hookAbs) { $hookAbs = $HookScript }
$hookAbs = $hookAbs.Path

if ($Uninstall) {
    if (Test-Path $settingsPath) {
        Remove-Item $settingsPath -Force
        Write-Host "removed $settingsPath"
    }
    exit 0
}

if (-not (Test-Path $ClaudeDir)) {
    New-Item -ItemType Directory -Path $ClaudeDir -Force | Out-Null
}

$json = @{
    hooks = @{
        UserPromptSubmit = @{
            command = "powershell.exe"
            args = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $hookAbs)
        }
        PostToolUse = @{
            command = "powershell.exe"
            args = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $hookAbs)
        }
        Stop = @{
            command = "powershell.exe"
            args = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $hookAbs)
        }
    }
} | ConvertTo-Json -Depth 10

$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($settingsPath, $json + "`n", $utf8NoBom)
Write-Host "wrote $settingsPath"
Write-Host "  hook script: $hookAbs"
Write-Host ""
Write-Host "Restart Claude Code for the hooks to take effect."