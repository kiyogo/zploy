@echo off
chcp 65001 >nul
echo ========================================================
echo        Zploy - Build Release Binary
echo ========================================================
echo.

echo [BUILD] Compiling Zploy in release mode (--release)...
cargo build --release -j 2

if %ERRORLEVEL% EQU 0 (
    echo.
    echo [SUCCESS] Binary executable created at: target\release\zploy.exe
) else (
    echo.
    echo [ERROR] Compilation failed. Please check compiler output above.
)
pause
