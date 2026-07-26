@echo off
pwsh.exe -NoLogo -NoProfile -File "%~dp0dev-start.ps1"
exit /b %ERRORLEVEL%
