@echo off
where pwsh >nul 2>nul
if errorlevel 1 (
    powershell.exe -NoProfile -ExecutionPolicy Bypass -STA -File "%~dp0scripts\setup.ps1"
) else (
    pwsh -NoProfile -ExecutionPolicy Bypass -STA -File "%~dp0scripts\setup.ps1"
)
if errorlevel 1 pause
