Set-Location $env:TEMP\mm
$deadline = (Get-Date).AddMinutes(28)
while ((Get-Date) -lt $deadline) {
    if (-not (Get-Process microscope-mem -ErrorAction SilentlyContinue)) { "BUILD DONE"; break }
    Start-Sleep -Seconds 20
}
Select-String -Path ev6.log -Pattern 'stored vectors|Blocks:|OK embed|WARN|ERROR' |
    Select-Object -Last 4 | ForEach-Object { $_.Line }
if (-not (Test-Path eval_output\embeddings.bin)) { "embeddings.bin MISSING"; exit 1 }
"--- semantic recall check (the fix under test) ---"
$env:MICROSCOPE_CONFIG = "$env:TEMP\mm\eval_config.toml"
$env:HF_HOME = "$env:USERPROFILE\.cache\huggingface"
foreach ($q in @("nut allergy", "where does the user live", "coffee")) {
    "### $q"
    & .\target\release\microscope-mem.exe recall "$q" 3 2>&1 | Select-Object -First 5
}
