@echo off
setlocal EnableExtensions
cd /d "%~dp0"

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

if not exist "node_modules\" (
  echo Installing dependencies first...
  call npm install
  if errorlevel 1 (
    echo [ERROR] npm install failed
    pause
    exit /b 1
  )
)

echo Starting Haven Desktop...
call npm run tauri:dev
pause
endlocal
