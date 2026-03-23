@echo off
title Microscope Memory — Full Stack
echo.
echo  ============================================
echo   Microscope Memory — Full Stack Launcher
echo   13-layer consciousness architecture
echo  ============================================
echo.

set "ROOT=%~dp0.."

:: Edge TTS server (port 8880)
echo  [1/3] Edge TTS server (port 8880)...
start /B python "%ROOT%\tools\edge_tts_server.py"
timeout /t 2 /nobreak >nul
echo  [OK] Edge TTS running

:: Microscope Memory HTTP server (port 6060)
echo.
echo  [2/3] Microscope Memory serve (port 6060)...
start /B "" "%ROOT%\target\release\microscope-memory.exe" serve
timeout /t 2 /nobreak >nul
echo  [OK] Microscope Memory API running

:: Cognitive map + viewer (port 8888)
echo.
echo  [3/3] Cognitive map export + viewer...
"%ROOT%\target\release\microscope-memory.exe" cognitive-map
start /B python -m http.server 8888 --directory "%ROOT%"
timeout /t 2 /nobreak >nul
start http://localhost:8888/viewer.html
echo  [OK] Viewer opened

echo.
echo  ============================================
echo   All services running:
echo   Memory API:   http://localhost:6060
echo   TTS:          http://localhost:8880
echo   Viewer:       http://localhost:8888/viewer.html
echo  ============================================
echo.
echo  Press CTRL+C or close this window to stop.
pause >nul
