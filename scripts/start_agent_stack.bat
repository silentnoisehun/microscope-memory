@echo off
cd /d "%~dp0.."
set "BIN=target\release\microscope-mem.exe"

if not exist "%BIN%" (
    echo [1/2] Building release binary...
    cargo build --release
)

echo [2/2] Starting MCP + Bridge stack...
echo.
start "Microscope Bridge" "%BIN%" bridge --host 0.0.0.0 --port 6060
timeout /t 2 /nobreak >nul
start "Microscope MCP" "%BIN%" mcp
timeout /t 1 /nobreak >nul

echo MCP + Bridge stack running.
echo Bridge: http://localhost:6060
echo MCP:    stdio (configure Claude Code / Cline to connect)
