@echo off
setlocal EnableExtensions
cd /d "%~dp0"

echo.
echo  Haven Desktop (Tauri) — setup
echo  ==============================
echo.

where node >nul 2>&1
if errorlevel 1 (
  echo [ERROR] Node.js not found. Install Node 18+ from https://nodejs.org
  pause
  exit /b 1
)

where rustc >nul 2>&1
if errorlevel 1 (
  echo [ERROR] Rust not found. Install from https://rustup.rs then reopen this window.
  pause
  exit /b 1
)

echo Installing npm dependencies...
call npm install
if errorlevel 1 (
  echo [ERROR] npm install failed
  pause
  exit /b 1
)

echo.
echo Setup complete.
echo.
echo Next:
echo   - Double-click "Start Haven Desktop.bat" to run in dev mode
echo   - Or double-click "Build Installer.bat" to create Haven-Setup.exe
echo.
pause
endlocal
