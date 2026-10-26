<#>
.SYNOPSIS
    Microscope Memory - Nightly Memory Maintenance
.DESCRIPTION
    Scheduled task entry point. Runs while Máté is not using the machine:
      1. dream          - offline consolidation + pruning
      2. reconsolidate  - emotion blend + spatial drift on recent recalls
      3. hippo          - episodic binding
    Everything logs to nightly-maintenance.log next to this script.
    Never shows a window (set in the scheduled task).
#>

$ErrorActionPreference = "SilentlyContinue"
$env:MICROSCOPE_CONFIG = "C:/Users/mater/AppData/Local/microscope-memory/config.toml"
$Bin = "C:/Users/mater/microscope-memory-clone/target/release/microscope-mem.exe"
$Log = "C:/Users/mater/microscope-memory-clone/ai-memory-skill/hooks/nightly-maintenance.log"

if (-not (Test-Path $Bin)) { exit 1 }

function Log($msg) {
    "$([DateTime]::Now.ToString('yyyy-MM-dd HH:mm:ss'))  $msg" | Out-File -FilePath $Log -Append -Encoding utf8
}

Log "=== nightly maintenance start ==="

# 1. Dream consolidation (the big one: replay + prune + strengthen)
$out = & $Bin dream 2>&1 | Out-String
Log "dream: $($out.Trim() -replace "`r?`n", ' | ')"

# 2. Reconsolidation of recent recalls
$out = & $Bin reconsolidate 2>&1 | Out-String
Log "reconsolidate: $($out.Trim() -replace "`r?`n", ' | ')"

# 3. Hippocampal episodic binding
$out = & $Bin hippo 2>&1 | Out-String
Log "hippo: $($out.Trim() -replace "`r?`n", ' | ')"

Log "=== nightly maintenance done ==="
exit 0
