param($Text, [string]$Layer='session', [int]$Importance=5)
$env:MICROSCOPE_CONFIG = "C:/Users/mater/AppData/Local/microscope-memory/config.toml"
& "C:/Users/mater/microscope-memory-clone/target/debug/microscope-mem.exe" store $Text -l $Layer -i $Importance
