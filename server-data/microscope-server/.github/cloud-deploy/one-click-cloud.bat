@echo off
setlocal

REM Windows one-click launcher for cloud deploy workflow.
REM It invokes the Linux one-click script via WSL.

where wsl >nul 2>nul
if errorlevel 1 (
  echo WSL nincs telepitve a gepen.
  echo Telepitsd: wsl --install
  echo Utana futtasd ujra ezt a .bat fajlt.
  exit /b 1
)

set "SCRIPT_DIR_WIN=%~dp0"

for /f "delims=" %%i in ('wsl wslpath -a "%SCRIPT_DIR_WIN%"') do set "SCRIPT_DIR_WSL=%%i"

if "%SCRIPT_DIR_WSL%"=="" (
  echo Nem sikerult atkonvertalni az utvonalat WSL formatumba.
  exit /b 1
)

echo [1/1] One-click cloud deploy indul WSL-ben...
wsl bash -lc "cd \"%SCRIPT_DIR_WSL%\" && chmod +x ./one-click-cloud.sh ./easy-start.sh ./bootstrap-ubuntu.sh ./deploy.sh ./backup.sh ./restore.sh ./install_fail2ban.sh && ./one-click-cloud.sh"

if errorlevel 1 (
  echo Hiba tortent a futtatas kozben.
  exit /b 1
)

echo Kesz.
endlocal
