#Requires -RunAsAdministrator

$BinaryPath = "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe"
$ConfigPath = "C:\Users\mater\AppData\Local\microscope-memory\config.toml"

# Delete existing task
Unregister-ScheduledTask -TaskName "MicroscopeMemoryDaemon" -Confirm:$false -ErrorAction SilentlyContinue

# Create trigger at logon
$Trigger = New-ScheduledTaskTrigger -AtLogOn

# Create action with proper escaping
$Argument = "-WindowStyle Hidden -Command `"$env:MICROSCOPE_CONFIG = '" + $ConfigPath + "'; & '" + "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe" + "' bridge --port 6060`""

$Action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument $Argument

# Settings
$Settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1)

# Register task
Register-ScheduledTask -TaskName "MicroscopeMemoryDaemon" -Action $Action -Trigger (New-ScheduledTaskTrigger -AtLogOn) -Settings $Settings -RunLevel Highest -Force

Write-Host "✅ Scheduled task created: MicroscopeMemoryDaemon" -ForegroundColor Green
Write-Host "  Runs at logon with 30s delay" -ForegroundColor Gray
Write-Host "  Restarts up to 3 times if crashed" -ForegroundColor Gray