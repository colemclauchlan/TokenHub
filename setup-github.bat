@echo off
REM Double-click this to init git, create the private GitHub repo, push, and file issues.
REM It always runs from its own folder, so "path does not exist" can't happen.
cd /d "%~dp0"
echo Running GitHub setup from: %~dp0
echo.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\init-and-push.ps1"
echo.
echo ---- Done. If it asked you to run "gh auth login", do that once, then double-click this again. ----
pause
