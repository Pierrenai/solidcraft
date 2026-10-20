@echo off
cd /d "%~dp0"
if exist "SolidCraft.exe" (
  start "" "SolidCraft.exe"
) else (
  cargo run --release
  if errorlevel 1 pause
)
