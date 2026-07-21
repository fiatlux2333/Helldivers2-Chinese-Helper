@echo off
setlocal EnableExtensions
cd /d "%~dp0"

set "PATH=C:\Program Files\GitHub CLI;D:\software\Git\cmd;%PATH%"

echo.
echo === HD2CN publish to GitHub ===
echo Repo: https://github.com/fiatlux2333/Helldivers2-Chinese-Helper
echo.

where gh >nul 2>&1
if errorlevel 1 (
  echo [ERROR] gh.exe not found. Install GitHub CLI first:
  echo   winget install --id GitHub.cli -e
  exit /b 1
)

where git >nul 2>&1
if errorlevel 1 (
  echo [ERROR] git.exe not found.
  exit /b 1
)

gh auth status >nul 2>&1
if errorlevel 1 (
  echo GitHub CLI is not logged in.
  echo Opening browser login...
  echo Prefer HTTPS + login via web browser, and grant "repo" scope.
  gh auth login -h github.com -p https -w
  if errorlevel 1 (
    echo [ERROR] login failed.
    exit /b 1
  )
)

echo.
echo Creating public repository if missing...
gh repo view fiatlux2333/Helldivers2-Chinese-Helper >nul 2>&1
if errorlevel 1 (
  gh repo create Helldivers2-Chinese-Helper --public --description "Helldivers 2 非官方中文输入助手（Tauri + Vue + Rust）" --source=. --remote=origin --push
  if errorlevel 1 (
    echo [ERROR] create/push failed.
    exit /b 1
  )
) else (
  echo Repository already exists. Making sure it is public...
  gh repo edit fiatlux2333/Helldivers2-Chinese-Helper --visibility public --accept-visibility-change-consequences
  git remote remove origin 2>nul
  git remote add origin https://github.com/fiatlux2333/Helldivers2-Chinese-Helper.git
  echo Pushing main...
  git push -u origin main
  if errorlevel 1 (
    echo [ERROR] push failed. If remote has unrelated history, inspect GitHub web first.
    exit /b 1
  )
)

echo.
echo OK. Open:
echo   https://github.com/fiatlux2333/Helldivers2-Chinese-Helper
echo.
pause
endlocal
