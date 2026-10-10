@echo off
rem Repair / reinstall Microsoft Edge WebView2 Runtime.
rem vsnote is a WebView2 app: if the runtime is broken, it exits silently on start.
rem Run this script as Administrator for a machine-wide repair.

setlocal
set SETUP=%TEMP%\MicrosoftEdgeWebview2Setup.exe

echo [1/2] Downloading WebView2 bootstrapper ...
curl -sL -o "%SETUP%" "https://go.microsoft.com/fwlink/p/?LinkId=2124703"
if errorlevel 1 (
    echo Download failed. Get it manually from:
    echo   https://developer.microsoft.com/microsoft-edge/webview2/
    exit /b 1
)

echo [2/2] Installing / repairing WebView2 Runtime ...
"%SETUP%" /install

echo.
echo Done. Reboot if prompted, then run:
echo   python scripts\check-webview2.py
echo to confirm the runtime is healthy again.
endlocal
