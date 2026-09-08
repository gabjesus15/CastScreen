@echo off
title CastScreen - Emisor (PC Gaming)
echo ========================================================
echo   📡 CastScreen Studio - Modo Emisor (PC Gaming)
echo ========================================================
echo Compilando e iniciando la ventana del Emisor...
echo.
cargo run -p castscreen-sender
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo Ocurrio un error al ejecutar el Emisor.
    pause
)
