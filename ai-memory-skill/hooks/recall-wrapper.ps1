<#>
.SYNOPSIS
    Microscope Memory - Recall Wrapper
.DESCRIPTION
    Wrapper for recall command with configurable defaults.
#>

param(
    [Parameter(Mandatory=$true, Position=0)]
    [string]$Query,
    
    [Parameter(Position=1)]
    [int]$K = 10,
    
    [string]$ConfigPath = "C:/Users/mater/AppData/Local/microscope-memory/config.toml",
    [string]$BinaryPath = "C:/Users/mater/microscope-memory-clone/target/debug/microscope-mem.exe"
)

$env:MICROSCOPE_CONFIG = $ConfigPath
& $BinaryPath recall $Query $K