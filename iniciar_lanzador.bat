@echo off
title CastScreen - Lanzador Universal
echo ========================================================
echo   📡 CastScreen - Lanzador Universal
echo ========================================================
echo Abriendo el Lanzador con selector de modo...
echo.
cargo run -p castscreen-launcher
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo Ocurrio un error al ejecutar el Lanzador.
    pause
)
