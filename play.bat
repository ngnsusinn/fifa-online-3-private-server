@echo off
rem One click: start the FO3ID Rust private server (own window; close it to stop the server)
rem and then the game.
cd /d "%~dp0"
rem Stop any server instances still running
powershell -NoProfile -Command "Get-CimInstance Win32_Process | Where-Object { $_.ProcessName -match 'start_all|http_config|messenger|server' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }"
if exist "target\release\start_all.exe" (
    start "FO3ID Rust server" target\release\start_all.exe
) else (
    start "FO3ID Rust server" cargo run --release --bin start_all
)
timeout /t 4 /nobreak >nul
rem RunAsInvoker: start the game without the UAC prompt (it runs fine unelevated)
set __COMPAT_LAYER=RunAsInvoker
if exist "%~dp0launcher\fifalauncher.exe" (
    start "" "%~dp0launcher\fifalauncher.exe"
)
