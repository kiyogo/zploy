@echo off
chcp 65001 >nul
echo ========================================================
echo        Zploy - Static Web Hosting Server
echo ========================================================
echo.

if not exist "config\config.toml" (
    echo [INFO] config\config.toml not found, copying from config.example.toml...
    copy "config\config.example.toml" "config\config.toml" >nul
)

if not exist "data" mkdir "data"
if not exist "storage\sites" mkdir "storage\sites"
if not exist "storage\staging" mkdir "storage\staging"
if not exist "storage\temp" mkdir "storage\temp"

echo [START] Starting Zploy on Windows...
if exist "target\release\zploy.exe" (
    target\release\zploy.exe
) else (
    cargo run --release
)
pause
