# Create Desktop Shortcut for Microscope Memory
$ProjectDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$DesktopPath = [Environment]::GetFolderPath("Desktop")
$ShortcutPath = Join-Path $DesktopPath "Microscope Memory.lnk"

$WScriptShell = New-Object -ComObject WScript.Shell
$Shortcut = $WScriptShell.CreateShortcut($ShortcutPath)

$Shortcut.TargetPath = Join-Path $ProjectDir "start-microscope.bat"
$Shortcut.WorkingDirectory = $ProjectDir
$Shortcut.Description = "Microscope Memory - Cognitive Memory Engine"

# Use electron.exe as icon (Windows shortcuts need .exe or .ico, not SVG)
$ElectronIcon = Join-Path $ProjectDir "electron\node_modules\electron\dist\electron.exe"
if (Test-Path $ElectronIcon) {
    $Shortcut.IconLocation = $ElectronIcon
}

$Shortcut.Save()

Write-Host "Desktop shortcut created: $ShortcutPath" -ForegroundColor Green
