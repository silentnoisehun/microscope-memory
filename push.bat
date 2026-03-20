@echo off
REM GitHub Push Script for Microscope Memory (Windows)

echo ========================================
echo  Microscope Memory - GitHub Push
echo ========================================
echo.

REM Check if USERNAME is provided
if "%1"=="" (
    echo Usage: push.bat YOUR_GITHUB_USERNAME
    echo Example: push.bat silentrobert
    exit /b 1
)

set USERNAME=%1
set REPO_URL=https://github.com/%USERNAME%/microscope-memory.git

echo Adding remote: %REPO_URL%
git remote add origin %REPO_URL% 2>nul || (
    echo Remote already exists, updating URL...
    git remote set-url origin %REPO_URL%
)

echo.
echo Pushing to GitHub...
echo ------------------------

echo Pushing master branch...
git push -u origin master

echo.
echo Pushing tags...
git push origin --tags

echo.
echo ========================================
echo  Push complete!
echo ========================================
echo.
echo Your repository is now live at:
echo    https://github.com/%USERNAME%/microscope-memory
echo.
echo Next steps:
echo    1. Go to: https://github.com/%USERNAME%/microscope-memory/releases
echo    2. Click 'Create a new release' from tag v0.1.0
echo    3. Add release binaries if desired
echo    4. Enable GitHub Actions in Settings
echo.
echo Suggested topics to add:
echo    rust, llm, memory-management, vector-search, hierarchical-data,
echo    mmap, performance, context-window, rag, embeddings
echo.