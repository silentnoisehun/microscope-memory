# Cleanup: remove the omp bridge test entries from the canonical D:\codex
# memory layers. The entries were written while the bridge still pointed at
# the canonical instance. The layer files are the source of truth for a
# rebuild, so removing them here and rebuilding restores the instance.
#
# ASCII only on purpose (PowerShell 5.1 + BOM-less UTF-8).

$ErrorActionPreference = 'Stop'

$LayerDir = 'D:\codex\microscope-memory\layers'
$BackupDir = 'D:\codex\microscope-memory\layers\backup-20261023-cleanup'
$Stamp = '20261023-1405'

$Markers = @(
  'omp-mem-test',
  'PROBE session context entry ZZPROBE-CTX',
  'PROBE store schema check ZZPSTORE',
  'OMP INTEGRATION 2026-10-23',
  'MICROSCOPE ENGINE ARCHITEKTURA',
  'MIGRATION TEST',
  'ZZZPROBE',
  'ENCODING TEST'
)

if (-not (Test-Path $BackupDir)) {
  New-Item -ItemType Directory -Path $BackupDir -Force | Out-Null
}

$enc = New-Object System.Text.UTF8Encoding($false)

foreach ($name in @('session.txt', 'long_term.txt')) {
  $path = Join-Path $LayerDir $name
  if (-not (Test-Path $path)) { Write-Host ("skip (missing): " + $name); continue }

  $backup = Join-Path $BackupDir ($name + '.' + $Stamp + '.bak')
  Copy-Item -LiteralPath $path -Destination $backup -Force

  $raw = [System.IO.File]::ReadAllText($path)
  $sep = if ($raw.Contains("`r`n`r`n")) { "`r`n`r`n" } else { "`n`n" }
  $blocks = $raw -split "`r`n`r`n|`n`n"

  $keep = New-Object System.Collections.ArrayList
  $removed = 0
  foreach ($b in $blocks) {
    $hit = $false
    foreach ($m in $Markers) {
      if ($b.Contains($m)) { $hit = $true; break }
    }
    if ($hit) { $removed++ } else { [void]$keep.Add($b) }
  }

  [System.IO.File]::WriteAllText($path, ($keep -join $sep), $enc)
  Write-Host ("{0,-16} blocks {1} -> {2}  (removed {3})  backup: {4}" -f $name, $blocks.Count, $keep.Count, $removed, (Split-Path $backup -Leaf))
}

Write-Host ''
Write-Host 'cleanup done - now rebuild the canonical index so the binary index matches.'
