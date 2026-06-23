@echo off
title Microscope Memory — Electron App
cd /d "%~dp0"

echo.
echo === Microscope Memory — Electron App ===
echo.

:: Check if native addon exists
if not exist "native\target\release\microscope_native.dll" (
    echo [BUILD] Building native addon...
    cd native
    cargo build --release
    cd ..
    if errorlevel 1 (
        echo [ERROR] Native addon build failed
        pause
        exit /b 1
    )
    echo [BUILD] Native addon built successfully
)

:: Check if node_modules exists
if not exist "electron\node_modules" (
    echo [NPM] Installing Electron dependencies...
    cd electron
    call npm install
    cd ..
    if errorlevel 1 (
        echo [ERROR] npm install failed
        pause
        exit /b 1
    )
    echo [NPM] Dependencies installed
)

:: Start the Electron app (detached — cmd window closes on its own)
echo [LAUNCH] Starting Microscope Memory...
echo.
cd electron
start "" npx electron .
cd ..
