@echo off
title CastScreen - Receptor (Laptop)
echo ========================================================
echo   📡 CastScreen - Modo Receptor (Laptop / Live Preview)
echo ========================================================
echo Abriendo la ventana del Receptor a 60 FPS...
echo.
cargo run -p castscreen-receiver
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo Ocurrio un error al ejecutar el Receptor.
    pause
)
