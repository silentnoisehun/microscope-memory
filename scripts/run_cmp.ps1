Set-Location $env:TEMP\mm
$env:MICROSCOPE_CONFIG = "$env:TEMP\mm\eval_config.toml"
$env:HF_HOME = "$env:USERPROFILE\.cache\huggingface"
python scripts\compare_baselines.py --config eval_config.toml --k 1 5 10 > cmp_final.log 2>&1
"exit=$?"
Get-Content cmp_final.log -ErrorAction SilentlyContinue | Select-Object -Last 24
