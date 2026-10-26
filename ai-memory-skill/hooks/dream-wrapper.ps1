<#>
.SYNOPSIS
    Microscope Memory - Dream Wrapper
#>
param(
    [string]$ConfigPath = "C:/Users/mater/AppData/Local/microscope-memory/config.toml",
    [string]$BinaryPath = "C:/Users/mater/microscope-memory-clone/target/debug/microscope-mem.exe"
)
$env:MICROSCOPE_CONFIG = $ConfigPath
& $BinaryPath dream