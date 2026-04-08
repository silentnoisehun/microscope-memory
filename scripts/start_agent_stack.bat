@echo off
setlocal
cd /d "%~dp0.."

echo.
echo ==========================================================
echo   Microscope Memory - MCP + Agent Delegation Stack
echo ==========================================================
echo.

set "BIN_A=target\release\microscope-mem.exe"
set "BIN_B=target\release\microscope-memory.exe"
set "BIN="

if exist "%BIN_A%" set "BIN=%BIN_A%"
if not defined BIN if exist "%BIN_B%" set "BIN=%BIN_B%"

if not defined BIN (
  echo [1/2] Building release binary...
  cargo build --release
  if errorlevel 1 (
    echo [ERROR] Build failed.
    pause
    exit /b 1
  )
  if exist "%BIN_A%" set "BIN=%BIN_A%"
  if not defined BIN if exist "%BIN_B%" set "BIN=%BIN_B%"
)

if not defined BIN (
  echo [ERROR] Could not locate release binary.
  pause
  exit /b 1
)

echo [2/2] Starting services from "%BIN%"
echo.

start "Microscope MCP Server" cmd /k ""%BIN%" mcp"
timeout /t 1 /nobreak >nul

start "Microscope Bridge API" cmd /k ""%BIN%" bridge --host 127.0.0.1 --port 6060"
timeout /t 1 /nobreak >nul

start "Microscope Viewer Server" cmd /k ""%BIN%" serve --port 8080"
timeout /t 2 /nobreak >nul

start http://127.0.0.1:8080/viewer.html?file=cognitive_map.bin
start http://127.0.0.1:6060/status

echo Services started:
echo   MCP Server      - running in separate terminal
echo   Bridge API      - http://127.0.0.1:6060
echo   Viewer          - http://127.0.0.1:8080/viewer.html
echo.
echo Keep the opened service terminals running.
echo.
pause
