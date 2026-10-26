<#
  restart-bridge-daemon.ps1 - restart the Microscope Memory bridge daemon.

  The daemon serves the REST bridge API on port 6060 for the other tools on
  the machine. It runs with the machine's own MICROSCOPE_CONFIG, which points
  at this repo's instance.

  It must be stopped before a build (a running engine holds the binary and the
  build fails with "failed to remove file ... os error 5").

  ASCII only on purpose: PowerShell 5.1 reads BOM-less UTF-8 as ANSI.
#>

[CmdletBinding()]
param(
  [switch]$Stop
)

$ErrorActionPreference = 'Continue'

# A futtatási hely a .hope\bin (persisted rule: a build-dir csak forrás).
# Itt a legfrissebb embeddings-képes build van.
$Bin = 'C:\Users\mater\.hope\bin\microscope-mem.exe'
$Config = 'C:\Users\mater\AppData\Local\microscope-memory\config.toml'
$Port = 6060

function Get-Bridge {
  # The command line is not always readable, so the port is the reliable
  # handle: the bridge daemon is the engine process that owns it.
  $conn = Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue |
    Where-Object { $_.LocalPort -eq $Port } | Select-Object -First 1
  if ($conn) {
    Get-CimInstance Win32_Process -Filter ("ProcessId=" + $conn.OwningProcess) -ErrorAction SilentlyContinue
    return
  }
  Get-CimInstance Win32_Process -Filter "Name='microscope-mem.exe'" -ErrorAction SilentlyContinue |
    Where-Object { $_.CommandLine -match 'bridge' }
}

Write-Host '=== Microscope Memory bridge daemon ==='

$running = Get-Bridge
if ($running) {
  foreach ($p in $running) {
    try {
      Stop-Process -Id $p.ProcessId -Force -ErrorAction Stop
      Write-Host ("stopped " + $p.ProcessId)
    } catch {
      Write-Host ("could not stop " + $p.ProcessId + " (elevated?) - stop it from an admin shell")
      exit 1
    }
  }
  Start-Sleep -Seconds 2
}

if ($Stop) {
  Write-Host 'stopped, not starting again.'
  exit 0
}

if (-not (Test-Path $Bin)) {
  Write-Host ("binary not found: " + $Bin)
  exit 1
}

$cmd = '$env:MICROSCOPE_CONFIG=''C:/Users/mater/AppData/Local/microscope-memory/config.toml''; & ''C:/Users/mater/.hope/bin/microscope-mem.exe'' bridge --port 6060'
Start-Process -FilePath 'powershell.exe' -ArgumentList @('-NoProfile', '-WindowStyle', 'Hidden', '-Command', $cmd) -WindowStyle Hidden
Start-Sleep -Seconds 6

try {
  $status = (Invoke-WebRequest -Uri ("http://127.0.0.1:" + $Port + "/status") -TimeoutSec 10 -UseBasicParsing).Content
  Write-Host ("running: " + $status)
} catch {
  Write-Host ("the daemon did not answer: " + $_.Exception.Message)
  exit 1
}
