# auto-inject.ps1 — Universal auto-context injector for any LLM wrapper.
#
# Usage (PowerShell):
#   .\auto-inject.ps1                          # prints to stdout
#   .\auto-inject.ps1 -Compact                 # compact version
#   .\auto-inject.ps1 -OutputPath C:\ctx.txt   # write to file
#
# Drop this into your LLM wrapper's startup:
#   - Claude Code:        see C:\Users\mater\.microscope\microscope-recall-hook.ps1
#   - Hermes / generic:   call from your session bootstrap script
#
# Env vars (optional):
#   MICROSCOPE_BIN   — full path to microscope-mem.exe
#                       (default: $PSScriptRoot\..\target\release\microscope-mem.exe)

param(
  [switch]$Compact,
  [string]$OutputPath
)

$ErrorActionPreference = "Stop"

# Resolve binary
if (-not $env:MICROSCOPE_BIN) {
  $RootDir = Split-Path -Parent $PSScriptRoot
  $candidate = Join-Path $RootDir "target\release\microscope-mem.exe"
  if (Test-Path $candidate) {
    $env:MICROSCOPE_BIN = $candidate
  } else {
    $found = Get-Command microscope-mem -ErrorAction SilentlyContinue
    if ($found) {
      $env:MICROSCOPE_BIN = $found.Source
    } else {
      Write-Error "[auto-inject] ERROR: microscope-mem binary not found"
      exit 1
    }
  }
}

$args = @("auto-context")
if ($Compact) { $args += "--compact" }
if ($OutputPath) {
  $args += "--output", $OutputPath
  & $env:MICROSCOPE_BIN @args
  Write-Host "Auto-context written to $OutputPath"
} else {
  & $env:MICROSCOPE_BIN @args
}