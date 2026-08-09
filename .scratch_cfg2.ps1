$ErrorActionPreference = 'Stop'
$out = Join-Path $env:TEMP 'mscope_perf_out'
if (Test-Path $out) { Remove-Item $out -Recurse -Force }
New-Item -ItemType Directory -Path (Join-Path $out 'data') | Out-Null
New-Item -ItemType Directory -Path (Join-Path $out 'tmp') | Out-Null
$outFwd = $out.Replace('\', '/')
$cfg = @"
project_id = "global"

[paths]
layers_dir = "D:/codex/microscope-memory/layers"
output_dir = "$outFwd/data/"
temp_dir = "$outFwd/tmp/"

[index]
max_depth = 8
header_size = 32
auto_rebuild = false
auto_rebuild_entries = 50
layer_retention_entries = 2000
max_blocks = 1500000
protect_min_importance = 8
promote_energy_threshold = 0.35

[search]
default_k = 10
zoom_weight = 3.0
keyword_boost = 0.4
semantic_weight = 0.3
emotional_bias_weight = 0.1

[memory_layers]
layers = ["long_term","short_term","session","associative","emotional","relational","reflections","crypto_chain","echo_cache","rust_state"]

[performance]
use_mmap = true
cache_size = 64
build_workers = 4
use_gpu = false
compression = false
cache_ttl_secs = 300

[embedding]
provider = "none"
model = "sentence-transformers/all-MiniLM-L6-v2"
dim = 384
max_depth = 4

[server]
port = 6060
api_key = ""
cors_origin = "*"

[logging]
level = "info"
file = "microscope.log"
"@
$cfgFile = Join-Path $env:TEMP 'mscope_perf_out\config.toml'
[System.IO.File]::WriteAllText($cfgFile, $cfg)
Write-Output "OUT=$out"
Write-Output "CFG=$cfgFile"
