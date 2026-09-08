@echo off
title CastScreen - Compilador de Instalador Oficial
echo ========================================================
echo   📦 CastScreen - Compilador de Instalador Windows
echo ========================================================
echo.

cd /d "%~dp0\.."

echo [1/3] Compilando binarios de alta velocidad en modo Release...
cargo build --workspace --release
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo [ERROR] La compilacion de Rust fallo. Corrige los errores antes de continuar.
    pause
    exit /b %ERRORLEVEL%
)

echo.
echo [2/3] Verificando Inno Setup Compiler (ISCC)...

set ISCC_PATH=""
if exist "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" (
    set ISCC_PATH="C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
) else if exist "C:\Program Files\Inno Setup 6\ISCC.exe" (
    set ISCC_PATH="C:\Program Files\Inno Setup 6\ISCC.exe"
) else (
    where iscc >nul 2>nul
    if %ERRORLEVEL% EQU 0 (
        set ISCC_PATH="iscc"
    )
)

if %ISCC_PATH%=="" (
    echo.
    echo [AVISO] Inno Setup 6 no fue encontrado en las rutas estandar ni en el PATH.
    echo Puedes instalarlo desde https://jrsoftware.org/isdl.php
    echo O abrir manualmente installer\CastScreen_Setup.iss con Inno Setup Compiler.
    pause
    exit /b 1
)

echo Compilando instalador con %ISCC_PATH%...
%ISCC_PATH% "installer\CastScreen_Setup.iss"
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo [ERROR] Fallo el empaquetado del instalador.
    pause
    exit /b %ERRORLEVEL%
)

echo.
echo ========================================================
echo   ✅ Instalador generado con exito en dist\CastScreen_Setup.exe
echo ========================================================
echo.
pause
