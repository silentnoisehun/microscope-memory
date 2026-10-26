$env:MICROSCOPE_BIN = "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe"
$bin = $env:MICROSCOPE_BIN
$Action = "PreTurn"
$Query = "test hook integration"
$Importance = 5

if (-not (Test-Path $bin)) { Write-Error "microscope-mem.exe not found at $bin"; exit 1 }

function Invoke-Microscope {
    param(
        [string]$Command,
        [Parameter(ValueFromRemainingArguments=$true)]
        [string[]]$Args
    )
    $quotedArgs = @()
    foreach ($arg in $Args) {
        if ($arg -match '\s') {
            $quotedArgs += "`"$arg`""
        } else {
            $quotedArgs += $arg
        }
    }
    $cmd = "$bin $Command $($quotedArgs -join ' ')"
    Write-Host "DEBUG: $cmd"
    $result = Invoke-Expression $cmd
    Write-Host "DEBUG RESULT: $result"
    return $result
}

if ($Action -eq "PreTurn") {
    Write-Host "[RECALL] Retrieving relevant memories..." -ForegroundColor Cyan
    
    $ctx = Invoke-Microscope "auto-context" "--compact"
    Write-Host "CTX: $ctx"
    if ($ctx -and $ctx.Trim().Length -gt 20) {
        Write-Host "## Auto-Context (session snapshot)" -ForegroundColor Green
        Write-Host $ctx.Trim()
        Write-Host ""
    }
    
    $recall = Invoke-Microscope "recall" $Query "5"
    Write-Host "RECALL: $recall"
    if ($recall -and $recall.Trim().Length -gt 20) {
        Write-Host "## Relevant Memories for: $Query" -ForegroundColor Green
        Write-Host $recall.Trim()
        Write-Host ""
    }
    
    Write-Host "[READY] Context loaded." -ForegroundColor Cyan
}

exit 0