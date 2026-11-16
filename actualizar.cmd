@echo off
cd /d "%~dp0"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\install-build.ps1" -Launch
if errorlevel 1 pause
