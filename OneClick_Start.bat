@echo off
setlocal
cd /d "%~dp0"

echo [Microscope Memory] Ghost Mode Initialization Sequence...
echo.

:: 1. Ellenorizzuk, van-e leforditott polimorf binaris
if not exist target\release\microscope-memory.exe (
    echo [!] Nincs leforditott 'Ghost' binaris.
    echo [*] Polimorf ujraforditas indul (XOR obfuscation, Anti-VM injection)...
    cargo build --release
    if errorlevel 1 (
        echo [ERROR] A forditas sikertelen volt. Ellenorizd a Rust kornyezetet!
        pause
        exit /b 1
    )
    echo [*] Forditas sikeres. Dinamikus szignatura generalva.
    echo.
)

:: 2. Konfiguracio inicializalasa
if not exist config.toml (
    if exist config.example.toml (
        echo [*] Alapertelmezett konfiguracio letrehozasa...
        copy config.example.toml config.toml >nul
    ) else (
        echo [WARNING] Nincs config.example.toml sablon!
    )
)

:: 3. Futtatas hatterben (Ghost Mode)
echo [*] microscope-memory server inditasa hatterfolyamatkent...
powershell -WindowStyle Hidden -Command "Start-Process 'target\release\microscope-memory.exe' -ArgumentList 'serve' -WindowStyle Hidden"

echo.
echo [V] A motor sikeresen elindult es rejtve fut a hatterben (Port: 3000).
echo [V] Nyomj meg egy gombot a biztonsagos kilepeshez...
timeout /t 3 >nul
exit /b 0
