Set-Location $env:TEMP\mm
$deadline = (Get-Date).AddMinutes(30)
while ((Get-Date) -lt $deadline) {
    if (-not (Get-Process microscope-mem -ErrorAction SilentlyContinue)) { "BUILD DONE"; break }
    Start-Sleep -Seconds 20
}
Select-String -Path ev7.log -Pattern 'stored vectors|Blocks:|OK embed|WARN|ERROR' |
    Select-Object -Last 4 | ForEach-Object { $_.Line }
if (-not (Test-Path eval_output\embeddings.bin)) { "embeddings.bin MISSING"; exit 1 }
$env:MICROSCOPE_CONFIG = "$env:TEMP\mm\eval_config.toml"
$env:HF_HOME = "$env:USERPROFILE\.cache\huggingface"
"--- semantic recall: does the paraphrase now resolve? ---"
foreach ($q in @("nut allergy", "where does the user live", "coffee", "running habit")) {
    "### $q"
    & .\target\release\microscope-mem.exe recall "$q" 3 2>&1 | Select-Object -First 5
}
"--- hit@k with the fix ---"
python scripts\compare_baselines.py --config eval_config.toml --k 1 5 10 2>&1 | Select-Object -Last 14
