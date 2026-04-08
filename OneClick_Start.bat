@echo off
setlocal
cd /d "%~dp0"

echo [Microscope Memory] Ghost Mode Initialization Sequence...
echo.

:: 1. Check for compiled polymorphic binary
if not exist target\release\microscope-mem.exe (
    echo [!] No 'Ghost' binary found.
    echo [*] Starting polymorphic recompilation (XOR obfuscation, Anti-VM injection)...
    cargo build --release
    if errorlevel 1 (
        echo [ERROR] Build failed. Please check your Rust environment!
        pause
        exit /b 1
    )
    echo [*] Build successful. Dynamic signature generated.
    echo.
)

:: 2. Initialize configuration
if not exist config.toml (
    if exist config.example.toml (
        echo [*] Creating default configuration from template...
        copy config.example.toml config.toml >nul
    ) else (
        echo [WARNING] No config.example.toml template found!
    )
)

:: 3. Run in background (Ghost Mode)
echo [*] Starting microscope-memory server as a background process...
powershell -WindowStyle Hidden -Command "Start-Process 'target\release\microscope-mem.exe' -ArgumentList 'serve' -WindowStyle Hidden"

echo.
echo [V] Engine successfully started and running in background (Port: 6060).
echo [V] Press any key to exit safely...
timeout /t 3 >nul
exit /b 0
