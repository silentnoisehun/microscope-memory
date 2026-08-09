$ErrorActionPreference = 'Continue'
$env:MICROSCOPE_CONFIG = Join-Path $env:TEMP 'mscope_perf_out\config.toml'
$exe = 'D:\codex\microscope-memory\target\release\microscope-mem.exe'

function Measure-Cmd([string]$label, [string]$cmdName, [string]$query, [int]$k) {
  $t = Measure-Command { & $exe $cmdName $query "$k" 2>&1 | Out-Null }
  Write-Output ("{0}: {1} ms" -f $label, [int]$t.TotalMilliseconds)
}

# Warmup
& $exe recall "memory" 5 2>&1 | Out-Null
& $exe find "memory" 5 2>&1 | Out-Null

# Recall with zero hits -> skips the entire write block
Measure-Cmd 'RECALL_zero_hits   ' 'recall' 'qqqqzzzznonexistent' 5
Measure-Cmd 'RECALL_zero_hits   ' 'recall' 'qqqqzzzznonexistent' 5
# Recall with hits -> full write block
Measure-Cmd 'RECALL_with_hits   ' 'recall' 'memory indexing' 5
# Find zero hits
Measure-Cmd 'FIND_zero_hits     ' 'find'   'qqqqzzzznonexistent' 5
Measure-Cmd 'FIND_normal        ' 'find'   'memory indexing' 5
