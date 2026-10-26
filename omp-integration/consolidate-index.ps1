<#
  consolidate-index.ps1 - merge the pending append log into the binary index.

  Run this in an ELEVATED PowerShell. The bridge daemon holds the index
  mapped, and on Windows a mapped section blocks the rebuild:

      rebuild failed: "create microscope.bin: ... os error 1224"

  So the daemon has to be stopped first, and started again afterwards.

  ASCII only on purpose: PowerShell 5.1 reads BOM-less UTF-8 as ANSI.
#>

[CmdletBinding()]
param(
  [string]$ConfigPath = "C:\Users\mater\AppData\Local\microscope-memory\config.toml",
  [string]$Bin = "C:\Users\mater\.hope\bin\microscope-mem.exe",
  [int]$Port = 6060
)

$ErrorActionPreference = 'Continue'
$env:MICROSCOPE_CONFIG = $ConfigPath

Write-Host ''
Write-Host '=== Microscope Memory - index consolidation ==='
Write-Host ''

# 1. stop the bridge daemon (it owns the mapped index)
$conn = Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue |
  Where-Object { $_.LocalPort -eq $Port } | Select-Object -First 1
if ($conn) {
  try {
    Stop-Process -Id $conn.OwningProcess -Force -ErrorAction Stop
    Write-Host ("1. daemon stopped (pid " + $conn.OwningProcess + ")")
  } catch {
    Write-Host ("1. could not stop the daemon: " + $_.Exception.Message)
    Write-Host '   run this script in an elevated PowerShell'
    exit 1
  }
  Start-Sleep -Seconds 3
} else {
  Write-Host '1. no daemon on the port'
}

# 2. rebuild: the pending append log entries are merged into the index and
#    the log is cleared afterwards
$sw = [Diagnostics.Stopwatch]::StartNew()
$out = & $Bin rebuild 2>&1 | Out-String
$rc = $LASTEXITCODE
Write-Host ("2. rebuild exit=" + $rc + " in " + [Math]::Round($sw.Elapsed.TotalSeconds, 1) + " s")
($out -split "`n" | Where-Object { $_ -match 'blocks total|Append log|panicked|refusing|WARN' }) |
  Select-Object -First 8 | ForEach-Object { Write-Host ("   " + $_.Trim()) }
if ($rc -ne 0) { Write-Host '   the rebuild failed - see the message above' }

# 3. start the daemon again
$cmd = '$env:MICROSCOPE_CONFIG=''C:/Users/mater/AppData/Local/microscope-memory/config.toml''; & ''C:/Users/mater/.hope/bin/microscope-mem.exe'' bridge --port ' + $Port
Start-Process -FilePath 'powershell.exe' -ArgumentList @('-NoProfile', '-WindowStyle', 'Hidden', '-Command', $cmd) -WindowStyle Hidden
Start-Sleep -Seconds 6
try {
  $status = (Invoke-WebRequest -Uri ("http://127.0.0.1:" + $Port + "/status") -TimeoutSec 10 -UseBasicParsing).Content
  Write-Host ("3. daemon running: " + $status)
} catch {
  Write-Host ("3. the daemon did not answer: " + $_.Exception.Message)
}
Write-Host ''
