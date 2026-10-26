#Requires -RunAsAdministrator

$ServiceName = "MicroscopeMemoryDaemon"
$DisplayName = "Microscope Memory Daemon"
$BinaryPath = "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe"
$ConfigPath = "C:\Users\mater\AppData\Local\microscope-memory\config.toml"

# Stop and delete existing service
if (Get-Service "MicroscopeMemoryDaemon" -ErrorAction SilentlyContinue) {
    Stop-Service "MicroscopeMemoryDaemon" -Force
    sc.exe delete MicroscopeMemoryDaemon
}

# Create service with sc.exe
$BinPath = '"' + $BinaryPath + '" --serve-http --port 6060 --config "' + $ConfigPath + '"'
sc.exe create MicroscopeMemoryDaemon binPath= $BinPath DisplayName= "Microscope Memory Daemon" start= auto obj= LocalSystem

# Recovery options
sc.exe failure MicroscopeMemoryDaemon reset= 86400 actions= restart/5000/restart/10000/restart/20000

# Start service
Start-Service MicroscopeMemoryDaemon

Write-Host "Service installed and started" -ForegroundColor Green