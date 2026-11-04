@echo off
title FO3 Private Server (Rust)
cd /d "%~dp0"

echo ========================================================
echo   FIFA Online 3 Private Server (Rust)
echo.
echo   - HTTPS API ^& CDN:  https://127.0.0.1:443
echo   - HTTP API ^& CDN:   http://127.0.0.1:8080
echo   - Admin Dashboard:  http://127.0.0.1:8088/
echo   - Messenger:        127.0.0.1:28002
echo   - Blaze Redirector: 127.0.0.1:28000
echo   - Blaze Noti:       127.0.0.1:28001
echo ========================================================
echo.

rem Auto-ensure hosts entry for APSEODESK172
findstr /I "APSEODESK172" "%windir%\System32\drivers\etc\hosts" >nul 2>&1
if errorlevel 1 (
    echo Ensuring 127.0.0.1 APSEODESK172 in hosts file...
    powershell -NoProfile -Command "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile -Command \"Add-Content -Path C:\Windows\System32\drivers\etc\hosts -Value \"\"`n127.0.0.1 APSEODESK172\"\"\"' -Wait"
)

echo Starting all servers... (Press Ctrl+C to stop)
echo.

rem Stop any stale server processes first
powershell -NoProfile -Command "Get-CimInstance Win32_Process | Where-Object { $_.ProcessName -match 'start_all|http_config|messenger|server' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }"

if exist "target\release\start_all.exe" (
    "target\release\start_all.exe"
) else (
    cargo run --release --bin start_all
)

pause
