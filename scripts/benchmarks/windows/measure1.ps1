$ErrorActionPreference = 'Continue'
$env:MICROSCOPE_CONFIG = Join-Path $env:TEMP 'mscope_perf_out\config.toml'
$exe = 'D:\codex\microscope-memory\target\release\microscope-mem.exe'
$t = Measure-Command { $null = & $exe build 2>&1 | Out-String }
Write-Output ("BUILD_MS=" + [int]$t.TotalMilliseconds)
Get-ChildItem (Join-Path $env:TEMP 'mscope_perf_out\data') | ForEach-Object { Write-Output ("DATA: " + $_.Name + " " + $_.Length) }
