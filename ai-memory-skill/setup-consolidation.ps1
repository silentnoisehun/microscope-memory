#Requires -RunAsAdministrator

$BinaryPath = "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe"
$ConfigPath = "C:\Users\mater\AppData\Local\microscope-memory\config.toml"
$FederatedConfigPath = "C:\Users\mater\AppData\Local\microscope-memory-federated\config.toml"

# Delete existing task
Unregister-ScheduledTask -TaskName "MicroscopeConsolidationDaemon" -Confirm:$false -ErrorAction SilentlyContinue

# Create trigger - repeat every 5 minutes
$Trigger = New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes(1) -RepetitionInterval (New-TimeSpan -Minutes 5) -RepetitionDuration ([TimeSpan]::MaxValue)

# Create action
$Argument = "-WindowStyle Hidden -Command `"$env:MICROSCOPE_CONFIG = '" + $ConfigPath + "'; & '" + "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe" + "' pulse-exchange; & '" + "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe" + "' integrate; $env:MICROSCOPE_CONFIG = '" + $FederatedConfigPath + "'; & '" + "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe" + "' pulse-exchange; & '" + "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe" + "' integrate; & '" + "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe" + "' reconsolidate`""

$Action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument "-WindowStyle Hidden -Command `"$env:MICROSCOPE_CONFIG = '" + $ConfigPath + "'; & '" + "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe" + "' pulse-exchange; & '" + "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe" + "' integrate; $env:MICROSCOPE_CONFIG = '" + $FederatedConfigPath + "'; & '" + "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe" + "' pulse-exchange; & '" + "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe" + "' integrate; & '" + "C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe" + "' reconsolidate`""

# Settings
$Settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1)

# Register task
Register-ScheduledTask -TaskName "MicroscopeConsolidationDaemon" -Action $Action -Trigger (New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes(1) -RepetitionInterval (New-TimeSpan -Minutes 5) -RepetitionDuration ([TimeSpan]::MaxValue)) -Settings (New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1)) -RunLevel Highest -Force

Write-Host "✅ Scheduled task created: MicroscopeConsolidationDaemon" -ForegroundColor Green
Write-Host "  Runs every 5 minutes" -ForegroundColor Gray
Write-Host "  Runs pulse-exchange, integrate, reconsolidate" -ForegroundColor Gray