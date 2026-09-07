@echo off
setlocal EnableExtensions
cd /d "%~dp0"

echo.
echo  Haven Desktop (Tauri) — one-click Windows installer build
echo  ========================================================
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

where cargo >nul 2>&1
if errorlevel 1 (
  echo [ERROR] cargo not found. Finish the Rust install from https://rustup.rs
  pause
  exit /b 1
)

echo [1/3] npm install...
call npm install
if errorlevel 1 (
  echo [ERROR] npm install failed
  pause
  exit /b 1
)

echo [2/3] Building NSIS installer (this can take several minutes)...
call npm run tauri build -- --bundles nsis
if errorlevel 1 (
  echo.
  echo [ERROR] Build failed.
  echo Make sure Visual Studio Build Tools are installed with
  echo "Desktop development with C++", then retry.
  pause
  exit /b 1
)

echo.
echo [3/3] Done.
echo Installer should be in:
echo   src-tauri\target\release\bundle\nsis\
echo.
dir /b "src-tauri\target\release\bundle\nsis\*.exe" 2>nul
echo.
echo Double-click the .exe to install Haven Desktop.
pause
endlocal
