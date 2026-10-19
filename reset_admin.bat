@echo off
chcp 65001 >nul
echo ========================================================
echo        Zploy - Reset Administrator Password
echo ========================================================
echo.
if exist "target\release\reset_admin.exe" (
    target\release\reset_admin.exe %*
) else (
    cargo run --bin reset_admin -- %*
)
pause
