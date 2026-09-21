@echo off
title Haven Desktop
cd /d "%~dp0"

if not exist "node_modules" (
    color 0C
    echo.
    echo  Haven Desktop has not been set up yet.
    echo  Please run "Setup.bat" first.
    echo.
    pause
    exit /b 1
)

echo Starting Haven Desktop...
echo  Server: host path from prefs / auto-detect Haven-Braid
echo  Tip: set HAVEN_DEVTOOLS=1 for DevTools; pass --dev for electron dev flag
echo.

if /I "%~1"=="--dev" (
  node "./node_modules/electron/cli.js" . --dev
) else (
  node "./node_modules/electron/cli.js" .
)
