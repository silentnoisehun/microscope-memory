# auto-save.ps1 — AfterMessage hook: saves last session entry to microscope memory
# Reads layers/session.txt, takes the last entry, stores it via CLI

$ErrorActionPreference = "Stop"

$projectDir = "E:\microscope-local"
$sessionFile = Join-Path $projectDir "layers\session.txt"
$bin = Join-Path $projectDir "target\release\microscope-mem.exe"

if (-not (Test-Path $sessionFile)) { exit 0 }
if (-not (Test-Path $bin)) { exit 0 }

# Ensure we're in the project directory so config.toml is found
Push-Location $projectDir

$content = Get-Content $sessionFile -Raw
$entries = $content -split "`n`n" | Where-Object { $_.Trim() -ne "" }
if ($entries.Count -eq 0) { Pop-Location; exit 0 }

$lastEntry = $entries[-1]
$lastEntry = $lastEntry.Substring(0, [Math]::Min($lastEntry.Length, 500))
$lastEntry = $lastEntry -replace '"', "'" -replace "`r`n", " " -replace "`n", " "

& $bin store --layer session --importance 6 $lastEntry 2>$null

Pop-Location
exit 0
