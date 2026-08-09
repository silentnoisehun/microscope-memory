$ErrorActionPreference = 'Stop'
$dir = Join-Path $env:TEMP 'mscope_perf'
$dirFwd = $dir.Replace('\', '/')
if (Test-Path $dir) { Remove-Item $dir -Recurse -Force }
New-Item -ItemType Directory -Path (Join-Path $dir 'layers') | Out-Null
New-Item -ItemType Directory -Path (Join-Path $dir 'data') | Out-Null
New-Item -ItemType Directory -Path (Join-Path $dir 'tmp') | Out-Null

$sb = New-Object System.Text.StringBuilder
for ($i = 0; $i -lt 200000; $i++) {
  [void]$sb.Append('(imp=5) unique_term_').Append($i).Append(' memory block about cognitive systems and binary indexing with common filler words for scale testing').AppendLine()
}
$layer = Join-Path $dir 'layers\long_term.txt'
[System.IO.File]::WriteAllText($layer, $sb.ToString())

$cfg = @"
[index]
auto_rebuild = false

[paths]
layers_dir = "$dirFwd/layers/"
output_dir = "$dirFwd/data/"
temp_dir = "$dirFwd/tmp/"

[memory_layers]
layers = ["long_term"]

[embedding]
provider = "none"

[search]
default_k = 10
"@
$cfgFile = Join-Path $dir 'config.toml'
[System.IO.File]::WriteAllText($cfgFile, $cfg)

Write-Output "DIR=$dir"
Write-Output ("LAYER_BYTES=" + (Get-Item $layer).Length)
