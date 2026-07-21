@echo off
setlocal EnableExtensions
cd /d "%~dp0"

REM One-click HD2CN tauri dev launcher (ASCII-only for cmd reliability).
set "RUSTUP_HOME=D:\software\.rustup"
set "CARGO_HOME=D:\software\.cargo"
set "NODE_DIR=C:\Users\Miku\.workbuddy\binaries\node\versions\22.22.2"
set "PATH=%NODE_DIR%;%CARGO_HOME%\bin;D:\software\npm-global;%PATH%"

where node >nul 2>nul
if errorlevel 1 (
  echo [ERROR] node.exe not found. Expected: %NODE_DIR%\node.exe
  echo Fix: install Node 22+ or update NODE_DIR in this script.
  pause
  exit /b 1
)

where cargo >nul 2>nul
if errorlevel 1 (
  echo [ERROR] cargo.exe not found. Expected: %CARGO_HOME%\bin\cargo.exe
  echo Fix: install Rust toolchain to D:\software\.cargo
  pause
  exit /b 1
)

echo node:
node -v
echo cargo:
cargo -V
echo.

REM Prefer pnpm.cmd to avoid PowerShell ExecutionPolicy blocking pnpm.ps1
where pnpm.cmd >nul 2>nul
if errorlevel 1 (
  echo [INFO] pnpm.cmd missing on PATH, falling back to node + pnpm.mjs
  if exist "D:\software\npm-global\node_modules\pnpm\bin\pnpm.mjs" (
    node "D:\software\npm-global\node_modules\pnpm\bin\pnpm.mjs" tauri dev
  ) else if exist "%NODE_DIR%\node_modules\pnpm\bin\pnpm.mjs" (
    node "%NODE_DIR%\node_modules\pnpm\bin\pnpm.mjs" tauri dev
  ) else (
    echo [ERROR] pnpm not found. Run: npm install -g pnpm@11.14.0
    pause
    exit /b 1
  )
) else (
  pnpm.cmd tauri dev
)

set "EC=%ERRORLEVEL%"
if not "%EC%"=="0" (
  echo.
  echo [ERROR] tauri dev exited with code %EC%
  pause
)
exit /b %EC%
