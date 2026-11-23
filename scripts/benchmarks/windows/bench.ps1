$ErrorActionPreference = 'Continue'
$env:MICROSCOPE_CONFIG = Join-Path $env:TEMP 'mscope_perf_out\config.toml'
$exe = 'D:\codex\microscope-memory\target\release\microscope-mem.exe'
$dataDir = Join-Path $env:TEMP 'mscope_perf_out\data'
$idx = Join-Path $dataDir 'text_index.bin'

function Measure-Cmd([string]$label, [string]$cmdName, [string]$query, [int]$k) {
  $t = Measure-Command { & $exe $cmdName $query "$k" 2>&1 | Out-Null }
  Write-Output ("{0}: {1} ms" -f $label, [int]$t.TotalMilliseconds)
}

# Warmup (file cache)
& $exe find "memory" 5 2>&1 | Out-Null

# --- With index ---
Measure-Cmd 'FIND_indexed  ' 'find'   'memory indexing' 5
Measure-Cmd 'FIND_indexed  ' 'find'   'binary mmap recall' 5
Measure-Cmd 'FIND_indexed  ' 'find'   'cognitive systems' 5
Measure-Cmd 'RECALL_indexed' 'recall' 'memory indexing performance' 5
Measure-Cmd 'RECALL_indexed' 'recall' 'hebbian remap bug fix' 5

# --- Without index ---
Rename-Item $idx ($idx + '.bak')
Measure-Cmd 'FIND_scan     ' 'find'   'memory indexing' 5
Measure-Cmd 'FIND_scan     ' 'find'   'binary mmap recall' 5
Measure-Cmd 'FIND_scan     ' 'find'   'cognitive systems' 5
Measure-Cmd 'RECALL_scan   ' 'recall' 'memory indexing performance' 5
Measure-Cmd 'RECALL_scan   ' 'recall' 'hebbian remap bug fix' 5
Rename-Item ($idx + '.bak') $idx
Write-Output 'RESTORED'
