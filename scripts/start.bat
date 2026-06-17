@echo off
title Microscope Memory
set "ROOT=%~dp0.."
set "BIN=%ROOT%\target\release\microscope-mem.exe"

if not exist "%BIN%" (
    echo Building release binary...
    cd /d "%ROOT%"
    cargo build --release
)

echo Starting Microscope Memory services...
echo.

:: Bridge API (port 6060)
echo [1/3] Bridge API (port 6060)...
start "Microscope Bridge" "%BIN%" bridge --host 0.0.0.0 --port 6060
timeout /t 2 /nobreak >nul

:: PWA Chat Server (port 8080)
echo [2/3] PWA Chat (port 8080)...
start "Microscope PWA" "%BIN%" serve --port 8080
timeout /t 2 /nobreak >nul

:: MCP Server (port 3456)
echo [3/3] MCP Server (port 3456)...
start "Microscope MCP" "%BIN%" mcp
timeout /t 1 /nobreak >nul

echo.
echo All services started:
echo   Bridge:  http://localhost:6060
echo   PWA:     http://localhost:8080/chat.html
echo   MCP:     stdio (port 3456)
echo.
echo To stop:   scripts\stop.bat
