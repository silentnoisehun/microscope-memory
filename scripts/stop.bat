@echo off
echo  Microscope Memory — stopping all services...
taskkill /F /IM microscope-memory.exe 2>nul && echo  [OK] Memory API stopped || echo  [--] Memory API was not running
for /f "tokens=5" %%P in ('netstat -ano ^| findstr ":8880.*LISTEN"') do taskkill /F /PID %%P 2>nul
echo  [OK] Edge TTS stopped
for /f "tokens=5" %%P in ('netstat -ano ^| findstr ":8888.*LISTEN"') do taskkill /F /PID %%P 2>nul
echo  [OK] Viewer server stopped
echo  Done.
timeout /t 2 /nobreak >nul
