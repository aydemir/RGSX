@echo off
setlocal EnableDelayedExpansion
set "VERSION=1.0"
:: =============================================================================
:: RGSX Rust Launcher v%VERSION%
:: =============================================================================
:: Pure-Rust manager-bin launcher (librqbit torrent engine + native catalog).
:: No Python required: manager-bin serves the WebUI SPA and reads the catalog
:: directly from the save folder (systems_list.json / games / images).
::
:: Usage: "RGSX rust.bat" [options]
::   --windowed     Open WebUI in a windowed browser (no fullscreen kiosk)
::   --no-tvui      Start only the server (no TVUI, no browser)
::   --display=N    Reserved for future TVUI display selection
::   --help, -h     Show this help
:: =============================================================================

:: Configuration des couleurs (codes ANSI)
for /F "tokens=1,2 delims=#" %%a in ('"prompt #$H#$E# & echo on & for %%b in (1) do rem"') do (
    set "ESC=%%b"
)

:: Couleurs
set "GREEN=[92m"
set "YELLOW=[93m"
set "RED=[91m"
set "CYAN=[96m"
set "RESET=[0m"
set "BOLD=[1m"

:: =============================================================================
:: Traitement des arguments
:: =============================================================================
set "WINDOWED_MODE="
set "NO_TVUI="
set "DISPLAY_NUM="

:parse_args
if "%~1"=="" goto :args_done
if /i "%~1"=="--help" goto :show_help
if /i "%~1"=="-h" goto :show_help
if /i "%~1"=="--windowed" (
    set "WINDOWED_MODE=1"
    shift
    goto :parse_args
)
if /i "%~1"=="--no-tvui" (
    set "NO_TVUI=1"
    shift
    goto :parse_args
)
:: Check for --display=N format
echo %~1 | findstr /r "^--display=" >nul
if !ERRORLEVEL! EQU 0 (
    for /f "tokens=2 delims==" %%a in ("%~1") do set "DISPLAY_NUM=%%a"
    shift
    goto :parse_args
)
shift
goto :parse_args

:show_help
echo.
echo %ESC%%CYAN%RGSX Rust Launcher - Help%ESC%%RESET%
echo.
echo Usage: "RGSX rust.bat" [options]
echo.
echo Options:
echo   --windowed     Open WebUI in a windowed browser
echo   --no-tvui      Start only the server (no TVUI, no browser)
echo   --display=N    Reserved for future TVUI display selection
echo   --help, -h     Show this help
echo.
echo Examples:
echo   "RGSX rust.bat"            Launch Rust manager with TVUI kiosk (fullscreen)
echo   "RGSX rust.bat" --windowed Launch Rust manager, open browser windowed
echo   "RGSX rust.bat" --no-tvui  Start server only
echo.
pause
exit /b 0

:args_done

:: Obtenir le chemin du script de maniere fiable
set "SCRIPT_DIR=%~dp0"
set "SCRIPT_DIR=%SCRIPT_DIR:~0,-1%"

:: Detecter le repertoire racine (roms\windows -> RetroBat root)
for %%I in ("%SCRIPT_DIR%\..\.." ) do set "ROOT_DIR=%%~fI"

:: Configuration des logs
set "LOG_DIR=%ROOT_DIR%\roms\windows\logs"
if not exist "%LOG_DIR%" mkdir "%LOG_DIR%"
set "LOG_FILE=%LOG_DIR%\Rust_RGSX_log.txt"
set "LOG_BACKUP=%LOG_DIR%\Rust_RGSX_log.old.txt"

:: Rotation des logs avec backup
if exist "%LOG_FILE%" (
    for %%A in ("%LOG_FILE%") do (
        if %%~zA GTR 100000 (
            if exist "%LOG_BACKUP%" del /q "%LOG_BACKUP%"
            move /y "%LOG_FILE%" "%LOG_BACKUP%" >nul 2>&1
            echo [%DATE% %TIME%] Log rotated - previous log saved as .old.txt > "%LOG_FILE%"
        )
    )
)

:: =============================================================================
:: Ecran d'accueil
:: =============================================================================
cls
echo.
echo %ESC%%CYAN%  ____   ____ ______  __ %ESC%%RESET%
echo %ESC%%CYAN% ^|  _ \ / ___^/ ___\ \/ / %ESC%%RESET%
echo %ESC%%CYAN% ^| ^|_) ^| ^|  _\___ \\  /  %ESC%%RESET%
echo %ESC%%CYAN% ^|  _ ^<^| ^|_^| ^|___) /  \  %ESC%%RESET%
echo %ESC%%CYAN% ^|_^| \_\\____^|____/_/\_\ %ESC%%RESET%
echo.
echo %ESC%%BOLD%  RGSX Rust Launcher v%VERSION%%ESC%%RESET%
echo   --------------------------------
if defined DISPLAY_NUM (
    echo   %ESC%%CYAN%Display: !DISPLAY_NUM! - reserved%ESC%%RESET%
)
if defined WINDOWED_MODE (
    echo   %ESC%%CYAN%Mode: Windowed browser%ESC%%RESET%
)
if defined NO_TVUI (
    echo   %ESC%%CYAN%Mode: Server only%ESC%%RESET%
)
echo.

:: Debut du log
echo [%DATE% %TIME%] ========================================== >> "%LOG_FILE%"
echo [%DATE% %TIME%] RGSX Rust Launcher v%VERSION% started >> "%LOG_FILE%"
echo [%DATE% %TIME%] Display: !DISPLAY_NUM!, Windowed: !WINDOWED_MODE!, NoTvui: !NO_TVUI! >> "%LOG_FILE%"
echo [%DATE% %TIME%] ========================================== >> "%LOG_FILE%"

:: =============================================================================
:: Configuration des chemins
:: =============================================================================
set "RUST_MANAGER_BIN=%ROOT_DIR%\roms\ports\RGSX\manager-bin.exe"
set "RGSX_WEBUI_DIR=%ROOT_DIR%\roms\ports\RGSX\webui"
:: Fallback (local dev / CI build yokken): repo kokundeki webui/dist'i kullan.
if not exist "%RGSX_WEBUI_DIR%\index.html" (
    if exist "%ROOT_DIR%\webui\dist\index.html" (
        set "RGSX_WEBUI_DIR=%ROOT_DIR%\webui\dist"
    )
)
set "RGSX_DATA_DIR=%ROOT_DIR%\saves\ports\rgsx"
set "RGSX_LANGUAGES_FOLDER=%ROOT_DIR%\roms\ports\RGSX\languages"
set "RGSX_DOWNLOADS_FOLDER=%ROOT_DIR%\saves\ports\rgsx\downloads"
set "RGSX_LOGS_FOLDER=%ROOT_DIR%\saves\ports\rgsx\logs"
set "RGSX_MANAGER_PORT=5000"

echo [%DATE% %TIME%] System info: >> "%LOG_FILE%"
echo [%DATE% %TIME%]   ROOT_DIR: %ROOT_DIR% >> "%LOG_FILE%"
echo [%DATE% %TIME%]   RUST_MANAGER_BIN: %RUST_MANAGER_BIN% >> "%LOG_FILE%"
echo [%DATE% %TIME%]   RGSX_WEBUI_DIR: %RGSX_WEBUI_DIR% >> "%LOG_FILE%"
echo [%DATE% %TIME%]   RGSX_DATA_DIR: %RGSX_DATA_DIR% >> "%LOG_FILE%"
echo [%DATE% %TIME%]   RGSX_MANAGER_PORT: %RGSX_MANAGER_PORT% >> "%LOG_FILE%"

:: =============================================================================
:: Verification manager-bin
:: =============================================================================
echo %ESC%%YELLOW%[1/2]%ESC%%RESET% Checking Rust manager...
echo [%DATE% %TIME%] Step 1/2: Checking manager-bin >> "%LOG_FILE%"

if not exist "%RUST_MANAGER_BIN%" (
    echo.
    echo %ESC%%RED%  ERROR: manager-bin.exe not found!%ESC%%RESET%
    echo.
    echo   Expected location:
    echo   %ESC%%CYAN%%RUST_MANAGER_BIN%%ESC%%RESET%
    echo.
    echo [%DATE% %TIME%] ERROR: manager-bin.exe not found at %RUST_MANAGER_BIN% >> "%LOG_FILE%"
    goto :error
)
if not exist "%RGSX_WEBUI_DIR%\index.html" (
    echo       %ESC%%YELLOW%^> Warning: WebUI SPA not deployed, placeholder UI will be served%ESC%%RESET%
    echo [%DATE% %TIME%] WARN: WebUI index.html missing at %RGSX_WEBUI_DIR% >> "%LOG_FILE%"
)
if not exist "%RGSX_DATA_DIR%\systems_list.json" (
    echo       %ESC%%YELLOW%^> Warning: native catalog data missing, catalog may be empty%ESC%%RESET%
    echo [%DATE% %TIME%] WARN: systems_list.json missing at %RGSX_DATA_DIR% >> "%LOG_FILE%"
)

echo       %ESC%%GREEN%^> Rust manager files OK%ESC%%RESET%
echo [%DATE% %TIME%] manager-bin verified >> "%LOG_FILE%"

:: =============================================================================
:: Arret de l'instance existante (evite conflit de port / fichier verrouille)
:: =============================================================================
taskkill /F /IM manager-bin.exe >nul 2>&1
if !ERRORLEVEL! EQU 0 (
    echo       %ESC%%YELLOW%^> Existing manager-bin.exe stopped%ESC%%RESET%
    echo [%DATE% %TIME%] Previous manager-bin.exe terminated >> "%LOG_FILE%"
)
timeout /t 1 /nobreak >nul

:: =============================================================================
:: Environnement
:: =============================================================================
set "RGSX_ROOT=%ROOT_DIR%"
set "RGSX_RUST_WEBUI=1"
set "RGSX_MANAGER_BIN_PORT=%RGSX_MANAGER_PORT%"
set "RGSX_NATIVE_CATALOG=1"
set "RGSX_TORRENT_ENGINE=librqbit"
set "RGSX_NO_AUTOSTART=1"
set "RGSX_WEBUI_DIR=%RGSX_WEBUI_DIR%"
set "RGSX_DATA_DIR=%RGSX_DATA_DIR%"
set "RGSX_LANGUAGES_FOLDER=%RGSX_LANGUAGES_FOLDER%"
set "RGSX_DOWNLOADS_FOLDER=%RGSX_DOWNLOADS_FOLDER%"
set "RGSX_LOGS_FOLDER=%RGSX_LOGS_FOLDER%"
if defined DISPLAY_NUM set "RGSX_DISPLAY=!DISPLAY_NUM!"
if not exist "%RGSX_DOWNLOADS_FOLDER%" mkdir "%RGSX_DOWNLOADS_FOLDER%" >nul 2>&1
if not exist "%RGSX_LOGS_FOLDER%" mkdir "%RGSX_LOGS_FOLDER%" >nul 2>&1

:: TVUI kiosk: varsayilan olarak acik; --windowed / --no-tvui ile kapatilir
set "TVUI_MODE="
if not defined WINDOWED_MODE if not defined NO_TVUI (
    set "TVUI_MODE=1"
    set "RGSX_TVUI=1"
)

echo [%DATE% %TIME%] Environment: >> "%LOG_FILE%"
echo [%DATE% %TIME%]   RGSX_ROOT=%RGSX_ROOT% >> "%LOG_FILE%"
echo [%DATE% %TIME%]   RGSX_RUST_WEBUI=1, RGSX_MANAGER_BIN_PORT=%RGSX_MANAGER_PORT% >> "%LOG_FILE%"
echo [%DATE% %TIME%]   RGSX_NATIVE_CATALOG=1, RGSX_TORRENT_ENGINE=librqbit >> "%LOG_FILE%"
echo [%DATE% %TIME%]   RGSX_NO_AUTOSTART=1 >> "%LOG_FILE%"
if defined RGSX_TVUI echo [%DATE% %TIME%]   RGSX_TVUI=1 >> "%LOG_FILE%"

:: =============================================================================
:: Lancement
:: =============================================================================
echo %ESC%%YELLOW%[2/2]%ESC%%RESET% Launching RGSX (Rust manager)...
echo [%DATE% %TIME%] Step 2/2: Launching manager-bin >> "%LOG_FILE%"
echo [%DATE% %TIME%] Executing: "%RUST_MANAGER_BIN%" >> "%LOG_FILE%"

powershell -NoProfile -ExecutionPolicy Bypass -Command "Start-Process -FilePath '%RUST_MANAGER_BIN%' -WindowStyle Hidden -RedirectStandardOutput '%LOG_DIR%\Rust_manager_out.log' -RedirectStandardError '%LOG_DIR%\Rust_manager_err.log'" >nul 2>&1
set "LAUNCH_RESULT=!ERRORLEVEL!"

if !LAUNCH_RESULT! NEQ 0 (
    echo.
    echo %ESC%%RED%  ERROR: Failed to launch manager-bin.exe!%ESC%%RESET%
    echo [%DATE% %TIME%] ERROR: Start-Process failed with code !LAUNCH_RESULT! >> "%LOG_FILE%"
    goto :error
)

:: =============================================================================
:: Attente du serveur (health)
:: =============================================================================
echo       %ESC%%YELLOW%^> Waiting for server on port !RGSX_MANAGER_PORT!...%ESC%%RESET%
set "SERVER_UP="
for /l %%i in (1,1,40) do (
    powershell -NoProfile -Command "try { Invoke-WebRequest -UseBasicParsing -Uri 'http://127.0.0.1:!RGSX_MANAGER_PORT!/api/health' -TimeoutSec 2 | Out-Null; exit 0 } catch { exit 1 }" >nul 2>&1
    if !ERRORLEVEL! EQU 0 (
        set "SERVER_UP=1"
        goto :server_up
    )
    timeout /t 1 /nobreak >nul
)

if not defined SERVER_UP (
    echo.
    echo %ESC%%RED%  ERROR: Rust manager did not start on port !RGSX_MANAGER_PORT!!%ESC%%RESET%
    echo.
    echo   Check the logs:
    echo   %ESC%%CYAN%%LOG_FILE%%ESC%%RESET%
    echo   %ESC%%CYAN%%LOG_DIR%\Rust_manager_err.log%ESC%%RESET%
    echo.
    echo [%DATE% %TIME%] ERROR: server health check failed >> "%LOG_FILE%"
    goto :error
)

:server_up
echo       %ESC%%GREEN%^> Server up: http://127.0.0.1:!RGSX_MANAGER_PORT!/%ESC%%RESET%
echo [%DATE% %TIME%] Server up on port !RGSX_MANAGER_PORT! >> "%LOG_FILE%"

if defined TVUI_MODE (
    echo       %ESC%%CYAN%^> TVUI kiosk requested - manager tries chromium/chrome; browser opened as fallback%ESC%%RESET%
    echo [%DATE% %TIME%] TVUI kiosk requested; opening default browser at http://127.0.0.1:!RGSX_MANAGER_PORT!/?mode=tv >> "%LOG_FILE%"
    start "" "http://127.0.0.1:!RGSX_MANAGER_PORT!/?mode=tv"
) else if not defined NO_TVUI (
    echo       %ESC%%CYAN%^> Opening WebUI in browser...%ESC%%RESET%
    echo [%DATE% %TIME%] Opening browser at http://127.0.0.1:!RGSX_MANAGER_PORT!/ >> "%LOG_FILE%"
    start "" "http://127.0.0.1:!RGSX_MANAGER_PORT!/"
)

echo.
echo   %ESC%%BOLD%RGSX Rust manager is running in the background.%ESC%%RESET%
echo   %ESC%%CYAN%Press any key to close this window (manager keeps running).%ESC%%RESET%
echo   %ESC%%CYAN%Tray icon: Open UI / Settings / Downloads / Logs / Quit%ESC%%RESET%
echo.
pause >nul
goto :end

:end
echo [%DATE% %TIME%] ========================================== >> "%LOG_FILE%"
echo [%DATE% %TIME%] Session ended normally >> "%LOG_FILE%"
echo [%DATE% %TIME%] ========================================== >> "%LOG_FILE%"
exit /b 0

:error
echo.
echo   %ESC%%RED%An error occurred. Check the log file:%ESC%%RESET%
echo   %ESC%%CYAN%%LOG_FILE%%ESC%%RESET%
echo.
echo [%DATE% %TIME%] ========================================== >> "%LOG_FILE%"
echo [%DATE% %TIME%] Session ended with errors >> "%LOG_FILE%"
echo [%DATE% %TIME%] ========================================== >> "%LOG_FILE%"
echo.
echo   Press any key to close...
pause >nul
exit /b 1
