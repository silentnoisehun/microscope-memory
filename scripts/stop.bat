@echo off
echo Stopping Microscope Memory services...
taskkill /F /IM microscope-mem.exe 2>nul && echo [OK] Stopped || echo [--] Not running
for /f "tokens=5" %%P in ('netstat -ano ^| findstr ":6060.*LISTEN"') do taskkill /F /PID %%P 2>nul
for /f "tokens=5" %%P in ('netstat -ano ^| findstr ":8080.*LISTEN"') do taskkill /F /PID %%P 2>nul
for /f "tokens=5" %%P in ('netstat -ano ^| findstr ":3456.*LISTEN"') do taskkill /F /PID %%P 2>nul
echo Done.
