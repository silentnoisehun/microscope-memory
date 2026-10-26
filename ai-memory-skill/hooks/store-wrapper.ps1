<#>
.SYNOPSIS
    Microscope Memory - Store Wrapper
.DESCRIPTION
    Wrapper for store command with layer/importance defaults.
#>

param(
    [Parameter(Mandatory=$true, Position=0)]
    [string]$Text,
    
    [Parameter(Position=1)]
    [string]$Layer = "session",
    
    [Parameter(Position=2)]
    [int]$Importance = 5,
    
    [string]$ConfigPath = "C:/Users/mater/AppData/Local/microscope-memory/config.toml",
    [string]$BinaryPath = "C:/Users/mater/microscope-memory-clone/target/debug/microscope-mem.exe"
)

$env:MICROSCOPE_CONFIG = $ConfigPath
& $BinaryPath store $Text -l $Layer -i $Importance