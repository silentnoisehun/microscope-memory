<#
  enable-embeddings.ps1 - switch the engine to real embeddings.

  Steps:
    1. kill before build: a running engine process holds the binary and the
       build fails with "failed to remove file ... os error 5".
    2. build the engine with the `embeddings` feature (candle provider).
    3. point the instance config at the real provider.
    4. rebuild the index so the embedding sidecar holds real vectors.
    5. report the measured recall.

  The sentence-transformers model is expected in the HuggingFace cache
  (~/.cache/huggingface/hub/models--sentence-transformers--all-MiniLM-L6-v2).

  ASCII only on purpose: PowerShell 5.1 reads BOM-less UTF-8 as ANSI.
#>

[CmdletBinding()]
param(
  [string]$ConfigPath = "C:\Users\mater\AppData\Local\microscope-memory\config.toml",
  [string]$RepoRoot = "C:\Users\mater\microscope-memory-clone",
  [int]$EmbeddingMaxDepth = 3,
  [switch]$SkipBuild
)

$ErrorActionPreference = 'Continue'

$Bin = Join-Path $RepoRoot 'target\release\microscope-mem.exe'
$env:MICROSCOPE_CONFIG = $ConfigPath

function Write-Step($msg) { Write-Host ""; Write-Host ("== " + $msg) }

Write-Step "1. kill before build"
Get-CimInstance Win32_Process -Filter "Name='microscope-mem.exe'" -ErrorAction SilentlyContinue |
  ForEach-Object {
    try {
      Stop-Process -Id $_.ProcessId -Force -ErrorAction Stop
      Write-Host ("   killed " + $_.ProcessId)
    } catch {
      Write-Host ("   could not kill " + $_.ProcessId + " (elevated?) - the build will fail")
    }
  }
Start-Sleep -Seconds 2

if (-not $SkipBuild) {
  Write-Step "2. build with the embeddings feature"
  $env:CARGO_BUILD_JOBS = '2'
  Push-Location $RepoRoot
  $out = & cargo build --release --features embeddings 2>&1 | Out-String
  $rc = $LASTEXITCODE
  Pop-Location
  Write-Host ("   cargo build --features embeddings exit=" + $rc)
  if ($rc -ne 0) {
    ($out -split "`n" | Where-Object { $_ -match '^error' }) | Select-Object -First 20 | ForEach-Object { Write-Host $_.Trim() }
    exit 1
  }
}

Write-Step "3. point the config at the real provider"
$cfg = Get-Content $ConfigPath -Raw
$cfg = $cfg -replace '(?m)^([ \t]*)provider[ \t]*=[ \t]*"mock"', '$1provider = "candle"'
$cfg = $cfg -replace '(?m)^([ \t]*)max_depth[ \t]*=[ \t]*5([ \t]*)$', ('$1max_depth = ' + $EmbeddingMaxDepth + '$2')
$enc = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($ConfigPath, $cfg, $enc)
Select-String -Path $ConfigPath -Pattern 'provider|max_depth|dim' | ForEach-Object { Write-Host ("   " + $_.Line.Trim()) }

Write-Step "4. rebuild the index (the embedding sidecar is rebuilt with it)"
$sw = [Diagnostics.Stopwatch]::StartNew()
$out = & $Bin build --force 2>&1 | Out-String
$rc = $LASTEXITCODE
Write-Host ("   exit=" + $rc + " in " + [Math]::Round($sw.Elapsed.TotalSeconds, 1) + " s")
($out -split "`n" | Where-Object { $_ -match 'blocks total|embedding|ERROR|WARN|Provider' }) |
  Select-Object -First 20 | ForEach-Object { Write-Host ("   " + $_.Trim()) }

Write-Step "5. status"
& $Bin status 2>&1 | Select-Object -First 3 | ForEach-Object { Write-Host ("   " + $_.Trim()) }
Write-Host ""
Write-Host "next: /mscope-status in omp, and the semantic recall is now enabled automatically."
