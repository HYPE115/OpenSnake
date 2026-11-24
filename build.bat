@echo off
setlocal enabledelayedexpansion

echo ========================================
echo  OpenSnake - Build script (Windows)
echo ========================================

:: Vérifie que cargo est disponible
where cargo >nul 2>&1
if errorlevel 1 (
    echo ERREUR : cargo n'est pas trouvé dans le PATH.
    echo Installe Rust depuis https://rustup.fr/ puis relance ce script.
    pause
    exit /b 1
)

echo [1/3] Nettoyage du précédent build...
cargo clean >nul 2>&1

echo [2/3] Compilation en mode Release (OpenSnake.exe optimisé)...
cargo build --release
if errorlevel 1 (
    echo ERREUR : la compilation a echoue.
    pause
    exit /b 1
)

echo [3/3] Verification de l'executable...
if exist "target\release\OpenSnake.exe" (
    echo SUCCES : target\release\OpenSnake.exe pret.
    echo.
    echo Exemple d utilisation :
    echo   target\release\OpenSnake.exe score
    echo   target\release\OpenSnake.exe best
    echo   target\release\OpenSnake.exe moves
) else (
    echo ATTENTION : l executable n'a pas ete trouve a l'emplacement attendu.
    pause
    exit /b 1
)

endlocal
pause
