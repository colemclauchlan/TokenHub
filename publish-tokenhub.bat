@echo off
REM Double-click to publish this project to GitHub as "TokenHub".
REM Requires Git + GitHub CLI (gh). If it says to run "gh auth login",
REM do that once in a terminal, then double-click this again.
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\publish-tokenhub.ps1"
echo.
pause
