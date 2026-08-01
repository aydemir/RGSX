@echo off
setlocal EnableDelayedExpansion

:: =============================================================================
:: RGSX Retrobat Launcher v1.3
:: =============================================================================
:: Usage: "RGSX Retrobat.bat" [options]
::   --display=N    Launch on display N (0=primary, 1=secondary, etc.)
::   --windowed     Launch in windowed mode instead of fullscreen
::   --webui        Start only the web server (no TV UI)
::   --create-shortcut  Create a desktop shortcut for Web UI
::   --help         Show this help
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
set "DISPLAY_NUM="
set "WINDOWED_MODE="
set "CONFIG_FILE="

:parse_args
if "%~1"=="" goto :args_done
if /i "%~1"=="--help" goto :show_help
if /i "%~1"=="-h" goto :show_help
if /i "%~1"=="--windowed" (
    set "WINDOWED_MODE=1"
    shift
    goto :parse_args
)
if /i "%~1"=="--webui" (
    set "WEBUI_ONLY=1"
    shift
    goto :parse_args
)
if /i "%~1"=="--create-shortcut" (
    set "CREATE_SHORTCUT=1"
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
echo %ESC%%CYAN%RGSX Retrobat Launcher - Help%ESC%%RESET%
echo.
echo Usage: "RGSX Retrobat.bat" [options]
echo.
echo Options:
echo   --display=N         Launch on display N (0=primary, 1=secondary, etc.)
echo   --windowed          Launch in windowed mode instead of fullscreen
echo   --webui             Start only the web server (no TV UI)
echo   --create-shortcut   Create a desktop shortcut for Web UI
echo   --help, -h          Show this help
echo.
echo Examples:
echo   "RGSX Retrobat.bat"                   Launch on primary display
echo   "RGSX Retrobat.bat" --display=1       Launch on secondary display (TV)
echo   "RGSX Retrobat.bat" --windowed        Launch in windowed mode
echo   "RGSX Retrobat.bat" --webui           Start web server only
echo   "RGSX Retrobat.bat" --create-shortcut Create desktop shortcut for Web UI
echo.
echo You can also create shortcuts with different display settings.
echo.
pause
exit /b 0

:args_done

:: =============================================================================
:: --create-shortcut: Create desktop shortcut for Web UI
:: =============================================================================
if defined CREATE_SHORTCUT (
    echo.
    echo %ESC%%CYAN%Creating desktop shortcut for RGSX Web UI...%ESC%%RESET%
    
    set "SHORTCUT_NAME=RGSX Web UI"
    set "BAT_SOURCE=%~dp0RGSX Retrobat.bat"
    
    :: Create VBS script to create shortcut
    (
        echo Set WshShell = CreateObject^("WScript.Shell"^)
        echo Set shortcut = WshShell.CreateShortcut^(WshShell.SpecialFolders^("Desktop"^) ^& "\%SHORTCUT_NAME%.lnk"^)
        echo shortcut.TargetPath = "%COMSPEC%"
        echo shortcut.Arguments = "/c ""%BAT_SOURCE%"" --webui"
        echo shortcut.WorkingDirectory = "%~dp0"
        echo shortcut.IconLocation = "%~dp0..\..\roms\ports\RGSX\assets\images\favicon_rgsx.ico"
        echo shortcut.Description = "RGSX Web UI - Web browser interface"
        echo shortcut.Save
    ) > "%TEMP%\rgsx_shortcut.vbs"
    
    cscript //nologo "%TEMP%\rgsx_shortcut.vbs"
    del /q "%TEMP%\rgsx_shortcut.vbs" 2>nul
    
    if exist "%USERPROFILE%\Desktop\%SHORTCUT_NAME%.lnk" (
        echo %ESC%%GREEN%^> Desktop shortcut created successfully!%ESC%%RESET%
        echo %ESC%%CYAN%  Location: %USERPROFILE%\Desktop\%SHORTCUT_NAME%.lnk%ESC%%RESET%
    ) else (
        echo %ESC%%RED%^> Failed to create shortcut%ESC%%RESET%
    )
    echo.
    pause
    exit /b 0
)

:: URL de telechargement Python
set "PYTHON_ZIP_URL=https://github.com/RetroGameSets/RGSX/raw/main/windows/python.zip"

:: Obtenir le chemin du script de maniere fiable
set "SCRIPT_DIR=%~dp0"
set "SCRIPT_DIR=%SCRIPT_DIR:~0,-1%"

:: Detecter le repertoire racine
for %%I in ("%SCRIPT_DIR%\..\.." ) do set "ROOT_DIR=%%~fI"

:: Configuration des logs
set "LOG_DIR=%ROOT_DIR%\roms\windows\logs"
if not exist "%LOG_DIR%" mkdir "%LOG_DIR%"
set "LOG_FILE=%LOG_DIR%\Retrobat_RGSX_log.txt"
set "LOG_BACKUP=%LOG_DIR%\Retrobat_RGSX_log.old.txt"

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
echo %ESC%%BOLD%  RetroBat Launcher v1.3%ESC%%RESET%
echo   --------------------------------
if "!DISPLAY_NUM!" NEQ "0" (
    echo   %ESC%%CYAN%Display: !DISPLAY_NUM!%ESC%%RESET%
)
if "!WINDOWED_MODE!"=="1" (
    echo   %ESC%%CYAN%Mode: Windowed%ESC%%RESET%
)
echo.

:: Debut du log
echo [%DATE% %TIME%] ========================================== >> "%LOG_FILE%"
echo [%DATE% %TIME%] RGSX Launcher v1.3 started >> "%LOG_FILE%"
echo [%DATE% %TIME%] Display: !DISPLAY_NUM!, Windowed: !WINDOWED_MODE! >> "%LOG_FILE%"
echo [%DATE% %TIME%] ========================================== >> "%LOG_FILE%"

:: Configuration des chemins
set "PYTHON_DIR=%ROOT_DIR%\system\tools\Python"
set "PYTHON_EXE=%PYTHON_DIR%\python.exe"
set "MAIN_SCRIPT=%ROOT_DIR%\roms\ports\RGSX\__main__.py"
set "ZIP_FILE=%ROOT_DIR%\roms\windows\python.zip"

:: Exporter RGSX_ROOT pour le script Python
set "RGSX_ROOT=%ROOT_DIR%"

:: Logger les chemins
echo [%DATE% %TIME%] System info: >> "%LOG_FILE%"
echo [%DATE% %TIME%]   ROOT_DIR: %ROOT_DIR% >> "%LOG_FILE%"
echo [%DATE% %TIME%]   PYTHON_EXE: %PYTHON_EXE% >> "%LOG_FILE%"
echo [%DATE% %TIME%]   MAIN_SCRIPT: %MAIN_SCRIPT% >> "%LOG_FILE%"
echo [%DATE% %TIME%]   RGSX_ROOT: %RGSX_ROOT% >> "%LOG_FILE%"

:: Determiner la source Python a utiliser
set "PYTHON_EXE="
set "PYTHON_ARGS="
set "PYTHON_SOURCE=none"

:: Essayer d'abord python.exe dans le PATH, mais seulement si il fonctionne vraiment
for /f "delims=" %%I in ('where python.exe 2^>nul') do (
    set "PYTHON_CANDIDATE=%%~fI"
    if exist "!PYTHON_CANDIDATE!" (
        if /i not "!PYTHON_CANDIDATE!"=="%localappdata%\Microsoft\WindowsApps\python.exe" (
            "!PYTHON_CANDIDATE!" --version >nul 2>&1
            if !ERRORLEVEL! EQU 0 (
                set "PYTHON_EXE=!PYTHON_CANDIDATE!"
                set "PYTHON_ARGS="
                set "PYTHON_SOURCE=user"
            )
        )
    )
)

if not defined PYTHON_EXE (
    where py.exe >nul 2>&1
    if !ERRORLEVEL! EQU 0 (
        py.exe -3 --version >nul 2>&1
        if !ERRORLEVEL! EQU 0 (
            set "PYTHON_EXE=py.exe"
            set "PYTHON_ARGS=-3"
            set "PYTHON_SOURCE=user"
        )
    )
)

if not defined PYTHON_EXE (
    if exist "%PYTHON_DIR%\python.exe" (
        "%PYTHON_DIR%\python.exe" --version >nul 2>&1
        if !ERRORLEVEL! EQU 0 (
            set "PYTHON_EXE=%PYTHON_DIR%\python.exe"
            set "PYTHON_ARGS="
            set "PYTHON_SOURCE=local-bundle"
        )
    )
)

echo [%DATE% %TIME%] Python source: !PYTHON_SOURCE! >> "%LOG_FILE%"
echo [%DATE% %TIME%] Python executable: !PYTHON_EXE! >> "%LOG_FILE%"

:: =============================================================================
:: Verification Python
:: =============================================================================
echo %ESC%%YELLOW%[1/3]%ESC%%RESET% Checking Python environment...
echo [%DATE% %TIME%] Step 1/3: Checking Python >> "%LOG_FILE%"

if /i "!PYTHON_SOURCE!"=="local-bundle" if not exist "!PYTHON_EXE!" (
    echo       %ESC%%YELLOW%^> Python not found, installing...%ESC%%RESET%
    echo [%DATE% %TIME%] Python not found, starting installation >> "%LOG_FILE%"
    
    :: Creer le dossier Python
    if not exist "%PYTHON_DIR%" (
        mkdir "%PYTHON_DIR%" 2>nul
        echo [%DATE% %TIME%] Created folder: %PYTHON_DIR% >> "%LOG_FILE%"
    )
    
    :: Verifier si le ZIP existe, sinon le telecharger
    if not exist "%ZIP_FILE%" (
        echo       %ESC%%YELLOW%^> python.zip not found, downloading from GitHub...%ESC%%RESET%
        echo [%DATE% %TIME%] python.zip not found, attempting download >> "%LOG_FILE%"
        echo [%DATE% %TIME%] Download URL: %PYTHON_ZIP_URL% >> "%LOG_FILE%"
        
        :: Verifier si curl est disponible
        where curl.exe >nul 2>&1
        if !ERRORLEVEL! EQU 0 (
            echo       %ESC%%CYAN%^> Using curl to download...%ESC%%RESET%
            echo [%DATE% %TIME%] Using curl.exe for download >> "%LOG_FILE%"
            curl.exe -L -# -o "%ZIP_FILE%" "%PYTHON_ZIP_URL%"
            set DOWNLOAD_RESULT=!ERRORLEVEL!
        ) else (
            :: Fallback sur PowerShell
            echo       %ESC%%CYAN%^> Using PowerShell to download...%ESC%%RESET%
            echo [%DATE% %TIME%] curl not found, using PowerShell >> "%LOG_FILE%"
            powershell -NoProfile -ExecutionPolicy Bypass -Command "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; $ProgressPreference = 'SilentlyContinue'; Invoke-WebRequest -Uri '%PYTHON_ZIP_URL%' -OutFile '%ZIP_FILE%'"
            set DOWNLOAD_RESULT=!ERRORLEVEL!
        )
        
        :: Verifier le resultat du telechargement
        if !DOWNLOAD_RESULT! NEQ 0 (
            echo.
            echo %ESC%%RED%  ERROR: Download failed!%ESC%%RESET%
            echo.
            echo   Please download python.zip manually from:
            echo   %ESC%%CYAN%%PYTHON_ZIP_URL%%ESC%%RESET%
            echo.
            echo   And place it in:
            echo   %ESC%%CYAN%%ROOT_DIR%\roms\windows\%ESC%%RESET%
            echo.
            echo [%DATE% %TIME%] ERROR: Download failed with code !DOWNLOAD_RESULT! >> "%LOG_FILE%"
            goto :error
        )
        
        :: Verifier que le fichier a bien ete telecharge et n'est pas vide
        if not exist "%ZIP_FILE%" (
            echo.
            echo %ESC%%RED%  ERROR: Download failed - file not created!%ESC%%RESET%
            echo [%DATE% %TIME%] ERROR: ZIP file not created after download >> "%LOG_FILE%"
            goto :error
        )
        
        :: Verifier la taille du fichier (doit etre > 1MB pour etre valide)
        for %%A in ("%ZIP_FILE%") do set ZIP_SIZE=%%~zA
        if !ZIP_SIZE! LSS 1000000 (
            echo.
            echo %ESC%%RED%  ERROR: Downloaded file appears invalid ^(too small^)!%ESC%%RESET%
            echo [%DATE% %TIME%] ERROR: Downloaded file too small: !ZIP_SIZE! bytes >> "%LOG_FILE%"
            del /q "%ZIP_FILE%" 2>nul
            goto :error
        )
        
        echo       %ESC%%GREEN%^> Download complete ^(!ZIP_SIZE! bytes^)%ESC%%RESET%
        echo [%DATE% %TIME%] Download successful: !ZIP_SIZE! bytes >> "%LOG_FILE%"
    )
    
    :: Verifier que tar existe (Windows 10 1803+)
    where tar >nul 2>&1
    if !ERRORLEVEL! NEQ 0 (
        echo.
        echo %ESC%%RED%  ERROR: tar command not available!%ESC%%RESET%
        echo.
        echo   Please update Windows 10 or extract manually to:
        echo   %ESC%%CYAN%%PYTHON_DIR%%ESC%%RESET%
        echo.
        echo [%DATE% %TIME%] ERROR: tar command not found >> "%LOG_FILE%"
        goto :error
    )
    
    :: Extraction avec progression simulee
    echo       %ESC%%YELLOW%^> Extracting Python...%ESC%%RESET%
    echo [%DATE% %TIME%] Extracting python.zip >> "%LOG_FILE%"
    
    <nul set /p "=       ["
    powershell -NoProfile -ExecutionPolicy Bypass -Command "try { Expand-Archive -LiteralPath '%ZIP_FILE%' -DestinationPath '%PYTHON_DIR%' -Force; exit 0 } catch { Write-Host $_.Exception.Message; exit 1 }" >>"%LOG_FILE%" 2>&1
    set TAR_RESULT=!ERRORLEVEL!
    echo %ESC%%GREEN%##########%ESC%%RESET%] Done
    
    if !TAR_RESULT! NEQ 0 (
        echo.
        echo %ESC%%RED%  ERROR: Extraction failed!%ESC%%RESET%
        echo [%DATE% %TIME%] ERROR: tar extraction failed with code !TAR_RESULT! >> "%LOG_FILE%"
        goto :error
    )
    
    echo [%DATE% %TIME%] Extraction completed >> "%LOG_FILE%"
    
    :: Supprimer ZIP
    del /q "%ZIP_FILE%" 2>nul
    echo       %ESC%%GREEN%^> python.zip cleaned up%ESC%%RESET%
    echo [%DATE% %TIME%] python.zip deleted >> "%LOG_FILE%"
    
    :: Verifier installation
    if not exist "%PYTHON_EXE%" (
        echo.
        echo %ESC%%RED%  ERROR: Python not found after extraction!%ESC%%RESET%
        echo [%DATE% %TIME%] ERROR: python.exe not found after extraction >> "%LOG_FILE%"
        goto :error
    )
)

:: Afficher et logger la version Python
for /f "tokens=*" %%v in ('"!PYTHON_EXE!" !PYTHON_ARGS! --version 2^>^&1') do set "PYTHON_VERSION=%%v"
echo       %ESC%%GREEN%^> !PYTHON_VERSION! found via !PYTHON_SOURCE!%ESC%%RESET%
echo [%DATE% %TIME%] !PYTHON_VERSION! detected using !PYTHON_SOURCE! >> "%LOG_FILE%"

:: Verifier et installer les dependances Python si necessaires
set "MISSING_PACKAGES="
for /f "delims=" %%m in ('"!PYTHON_EXE!" !PYTHON_ARGS! -c "import importlib.util,sys;mods=['requests','pygame']; missing=[m for m in mods if importlib.util.find_spec(m) is None]; print(' '.join(missing))" 2^>nul') do set "MISSING_PACKAGES=%%m"
if defined MISSING_PACKAGES (
    echo       %ESC%%YELLOW%^> Installing Python packages: !MISSING_PACKAGES!%ESC%%RESET%
    echo [%DATE% %TIME%] Installing missing packages: !MISSING_PACKAGES! >> "%LOG_FILE%"
    "!PYTHON_EXE!" !PYTHON_ARGS! -m pip install requests pygame >> "%LOG_FILE%" 2>&1
    set "PIP_RESULT=!ERRORLEVEL!"
    if !PIP_RESULT! NEQ 0 (
        echo.
        echo %ESC%%RED%  ERROR: Failed to install Python dependencies!%ESC%%RESET%
        echo [%DATE% %TIME%] ERROR: pip install failed with code !PIP_RESULT! >> "%LOG_FILE%"
        goto :error
    )
    echo       %ESC%%GREEN%^> Python dependencies installed%ESC%%RESET%
    echo [%DATE% %TIME%] Python dependencies installed successfully >> "%LOG_FILE%"
) else (
    echo       %ESC%%GREEN%^> Python dependencies OK%ESC%%RESET%
    echo [%DATE% %TIME%] Python dependencies already available >> "%LOG_FILE%"
)

:: =============================================================================
:: Configuration automatique du pare-feu Windows (une seule fois, transparente)
:: =============================================================================
:: Ajoute des regles entrantes pour aria2c.exe (BitTorrent) et python.exe (UPnP)
:: afin que les telechargements torrent et la decouverte UPnP fonctionnent sans
:: que l'utilisateur ait a configurer quoi que ce soit manuellement. Peut demander
:: une elevation UAC une seule fois (marqueur ecrit dans tous les cas pour ne
:: jamais redemander a chaque lancement).
set "ARIA2C_EXE=%ROOT_DIR%\roms\ports\RGSX\assets\progs\aria2c.exe"
set "FIREWALL_SCRIPT=%ROOT_DIR%\roms\ports\RGSX\assets\scripts\rgsx_firewall_setup.ps1"
set "FIREWALL_MARKER_DIR=%ROOT_DIR%\saves\ports\rgsx"
set "FIREWALL_MARKER=%FIREWALL_MARKER_DIR%\.firewall_rules_configured"

if not exist "%FIREWALL_MARKER%" (
    if exist "%FIREWALL_SCRIPT%" (
        echo       %ESC%%YELLOW%^> Configuring Windows Firewall for downloads ^(one-time^)...%ESC%%RESET%
        echo [%DATE% %TIME%] One-time firewall setup: launching %FIREWALL_SCRIPT% >> "%LOG_FILE%"
        powershell -NoProfile -ExecutionPolicy Bypass -File "%FIREWALL_SCRIPT%" -Aria2cPath "%ARIA2C_EXE%" -PythonPath "!PYTHON_EXE!" >> "%LOG_FILE%" 2>&1
        if not exist "%FIREWALL_MARKER_DIR%" mkdir "%FIREWALL_MARKER_DIR%" 2>nul
        echo [%DATE% %TIME%] Firewall setup attempted >> "%FIREWALL_MARKER%" 2>nul
        echo [%DATE% %TIME%] Firewall setup attempt complete, marker written >> "%LOG_FILE%"
    ) else (
        echo [%DATE% %TIME%] Firewall setup script not found, skipping >> "%LOG_FILE%"
    )
) else (
    echo [%DATE% %TIME%] Firewall already configured previously, skipping >> "%LOG_FILE%"
)

:: =============================================================================
:: Verification script principal
:: =============================================================================
echo %ESC%%YELLOW%[2/3]%ESC%%RESET% Checking RGSX application...
echo [%DATE% %TIME%] Step 2/3: Checking RGSX files >> "%LOG_FILE%"

if not exist "%MAIN_SCRIPT%" (
    echo.
    echo %ESC%%RED%  ERROR: __main__.py not found!%ESC%%RESET%
    echo.
    echo   Expected location:
    echo   %ESC%%CYAN%%MAIN_SCRIPT%%ESC%%RESET%
    echo.
    echo [%DATE% %TIME%] ERROR: __main__.py not found at %MAIN_SCRIPT% >> "%LOG_FILE%"
    goto :error
)

echo       %ESC%%GREEN%^> RGSX files OK%ESC%%RESET%
echo [%DATE% %TIME%] RGSX files verified >> "%LOG_FILE%"

:: =============================================================================
:: Lancement
:: =============================================================================
echo %ESC%%YELLOW%[3/3]%ESC%%RESET% Launching RGSX...
echo [%DATE% %TIME%] Step 3/3: Launching application >> "%LOG_FILE%"

:: Changer le repertoire de travail
cd /d "%ROOT_DIR%\roms\ports\RGSX"
echo [%DATE% %TIME%] Working directory: %CD% >> "%LOG_FILE%"

:: Configuration SDL/Pygame
set PYGAME_HIDE_SUPPORT_PROMPT=1
set SDL_VIDEODRIVER=windows
set SDL_AUDIODRIVER=directsound
set PYTHONWARNINGS=ignore::UserWarning:pygame.pkgdata
set PYTHONIOENCODING=utf-8

:: =============================================================================
:: Configuration multi-ecran
:: =============================================================================
:: SDL_VIDEO_FULLSCREEN_HEAD: Selectionne l'ecran pour le mode plein ecran
:: 0 = ecran principal, 1 = ecran secondaire, etc.
:: Ces variables ne sont definies que si --display=N ou --windowed est passe
:: Sinon, le script Python utilisera les parametres de rgsx_settings.json

echo [%DATE% %TIME%] Display configuration: >> "%LOG_FILE%"
if defined DISPLAY_NUM (
    set SDL_VIDEO_FULLSCREEN_HEAD=!DISPLAY_NUM!
    set RGSX_DISPLAY=!DISPLAY_NUM!
    echo [%DATE% %TIME%]   SDL_VIDEO_FULLSCREEN_HEAD=!DISPLAY_NUM! ^(from --display arg^) >> "%LOG_FILE%"
    echo [%DATE% %TIME%]   RGSX_DISPLAY=!DISPLAY_NUM! ^(from --display arg^) >> "%LOG_FILE%"
) else (
    echo [%DATE% %TIME%]   Display: using rgsx_settings.json config >> "%LOG_FILE%"
)
if defined WINDOWED_MODE (
    set RGSX_WINDOWED=!WINDOWED_MODE!
    echo [%DATE% %TIME%]   RGSX_WINDOWED=!WINDOWED_MODE! ^(from --windowed arg^) >> "%LOG_FILE%"
) else (
    echo [%DATE% %TIME%]   Windowed: using rgsx_settings.json config >> "%LOG_FILE%"
)

:: Log environnement
echo [%DATE% %TIME%] Environment variables set: >> "%LOG_FILE%"
echo [%DATE% %TIME%]   RGSX_ROOT=%RGSX_ROOT% >> "%LOG_FILE%"
echo [%DATE% %TIME%]   SDL_VIDEODRIVER=%SDL_VIDEODRIVER% >> "%LOG_FILE%"
echo [%DATE% %TIME%]   SDL_AUDIODRIVER=%SDL_AUDIODRIVER% >> "%LOG_FILE%"
echo [%DATE% %TIME%]   PYTHONIOENCODING=%PYTHONIOENCODING% >> "%LOG_FILE%"

:: =============================================================================
:: --webui: Start only web server (no TV UI)
:: =============================================================================
if defined WEBUI_ONLY (
    echo.
    echo %ESC%%CYAN%========================================%ESC%%RESET%
    echo %ESC%%CYAN%  RGSX Web UI Server%ESC%%RESET%
    echo %ESC%%CYAN%========================================%ESC%%RESET%
    echo.
    echo   %ESC%%YELLOW%Starting web server only...%ESC%%RESET%
    echo   %ESC%%CYAN%URL: http://localhost:5000%ESC%%RESET%
    echo   %ESC%%BOLD%Press Ctrl+C to stop the server%ESC%%RESET%
    echo.
    echo [%DATE% %TIME%] Web UI mode - starting web server only >> "%LOG_FILE%"
    
    cd /d "%ROOT_DIR%\roms\ports\RGSX"
    "!PYTHON_EXE!" !PYTHON_ARGS! "rgsx_web.py" --host 0.0.0.0 --port 5000
    set EXITCODE=!ERRORLEVEL!
    
    echo [%DATE% %TIME%] Web server exited with code !EXITCODE! >> "%LOG_FILE%"
    exit /b !EXITCODE!
)

echo.
if defined DISPLAY_NUM (
    echo   %ESC%%CYAN%Launching on display !DISPLAY_NUM!...%ESC%%RESET%
)
if defined WINDOWED_MODE (
    echo   %ESC%%CYAN%Windowed mode enabled%ESC%%RESET%
)
echo   %ESC%%CYAN%Starting RGSX application...%ESC%%RESET%
echo   %ESC%%BOLD%Press Ctrl+C to force quit if needed%ESC%%RESET%
echo.
echo [%DATE% %TIME%] Executing: "!PYTHON_EXE!" !PYTHON_ARGS! "%MAIN_SCRIPT%" >> "%LOG_FILE%"
echo [%DATE% %TIME%] --- Application output start --- >> "%LOG_FILE%"

"!PYTHON_EXE!" !PYTHON_ARGS! "%MAIN_SCRIPT%" >> "%LOG_FILE%" 2>&1
set EXITCODE=!ERRORLEVEL!

echo [%DATE% %TIME%] --- Application output end --- >> "%LOG_FILE%"
echo [%DATE% %TIME%] Exit code: !EXITCODE! >> "%LOG_FILE%"

if "!EXITCODE!"=="0" (
    echo.
    echo   %ESC%%GREEN%RGSX closed successfully.%ESC%%RESET%
    echo.
    echo [%DATE% %TIME%] Application closed successfully >> "%LOG_FILE%") else if "!EXITCODE!"=="1" (
    echo.
    echo   %ESC%%GREEN%RGSX closed normally.%ESC%%RESET%
    echo.
    >> "%LOG_FILE%" echo [%DATE% %TIME%] Application closed normally >> "%LOG_FILE%"
    goto :end) else (
    echo.
    echo   %ESC%%RED%RGSX exited with error code !EXITCODE!%ESC%%RESET%
    echo.
    echo [%DATE% %TIME%] ERROR: Application exited with code !EXITCODE! >> "%LOG_FILE%"
    goto :error
)

:end
echo [%DATE% %TIME%] ========================================== >> "%LOG_FILE%"
echo [%DATE% %TIME%] Session ended normally >> "%LOG_FILE%"
echo [%DATE% %TIME%] ========================================== >> "%LOG_FILE%"
ping -n 1 -w 5000 127.255.255.255 >nul
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