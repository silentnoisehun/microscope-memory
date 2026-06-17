@echo off
cd /d "%~dp0"
title Microscope Memory

if not exist target\release\microscope-mem.exe (
    echo Building release binary...
    cargo build --release
    if errorlevel 1 (
        echo Build failed. Check your Rust environment.
        pause
        exit /b 1
    )
)

echo Starting Microscope Memory services...
start "Microscope Bridge" target\release\microscope-mem.exe bridge --host 0.0.0.0 --port 6060
timeout /t 2 /nobreak >nul
start "Microscope PWA" target\release\microscope-mem.exe serve --port 8080

echo.
echo Services started:
echo   Bridge: http://localhost:6060
echo   PWA:    http://localhost:8080/chat.html
echo.
timeout /t 5 /nobreak >nul
