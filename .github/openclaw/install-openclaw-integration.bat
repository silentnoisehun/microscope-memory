@echo off
setlocal

where wsl >nul 2>nul
if errorlevel 1 (
  echo WSL nincs telepitve. Telepitsd: wsl --install
  exit /b 1
)

set "SCRIPT_DIR_WIN=%~dp0"
for /f "delims=" %%i in ('wsl wslpath -a "%SCRIPT_DIR_WIN%"') do set "SCRIPT_DIR_WSL=%%i"

if "%SCRIPT_DIR_WSL%"=="" (
  echo Nem sikerult atkonvertalni az utvonalat WSL formatumba.
  exit /b 1
)

wsl bash -lc "cd \"%SCRIPT_DIR_WSL%\" && chmod +x ./install-openclaw-integration.sh && ./install-openclaw-integration.sh"

if errorlevel 1 (
  echo Hiba tortent az OpenClaw integracio telepitese kozben.
  exit /b 1
)

echo OpenClaw integracio telepitve.
endlocal