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
