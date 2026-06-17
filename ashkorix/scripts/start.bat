@echo off
setlocal EnableExtensions

title Ashkorix

rem --- Rust (cargo) — not always on PATH when launched from Explorer ---
if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
)

rem --- Node.js (npm) ---
if exist "C:\Program Files\nodejs\npm.cmd" (
    set "PATH=C:\Program Files\nodejs;%PATH%"
)

rem --- LLVM (llama-cpp bindgen) ---
if exist "C:\Program Files\LLVM\bin\libclang.dll" (
    set "LIBCLANG_PATH=C:\Program Files\LLVM\bin"
)

rem --- CUDA (prefer newest installed) ---
set "CUDA_ROOT="
if exist "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.3\bin\nvcc.exe" (
    set "CUDA_ROOT=C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.3"
    set "CUDA_PATH_V13_3=%CUDA_ROOT%"
) else if exist "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.5\bin\nvcc.exe" (
    set "CUDA_ROOT=C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.5"
    set "CUDA_PATH_V12_5=%CUDA_ROOT%"
)

if defined CUDA_ROOT (
    set "CUDA_PATH=%CUDA_ROOT%"
    set "CUDA_HOME=%CUDA_ROOT%"
    set "PATH=%CUDA_ROOT%\bin\x64;%CUDA_ROOT%\bin;%PATH%"
    echo Using CUDA: %CUDA_ROOT%
) else (
    echo Warning: CUDA toolkit not found. GPU inference may be unavailable.
)

if defined LIBCLANG_PATH (
    echo Using LLVM: %LIBCLANG_PATH%
)

cd /d "%~dp0..\crates\ashkorix-app"
if errorlevel 1 (
    echo Failed to find ashkorix-app directory.
    pause
    exit /b 1
)

where cargo >nul 2>&1
if errorlevel 1 (
    echo Error: cargo not found. Install Rust from https://rustup.rs
    echo Expected: %USERPROFILE%\.cargo\bin\cargo.exe
    pause
    exit /b 1
)

where npm >nul 2>&1
if errorlevel 1 (
    echo Error: npm not found. Install Node.js from https://nodejs.org
    pause
    exit /b 1
)

echo Starting Ashkorix...
echo.
call npm run tauri dev
set "EXIT_CODE=%ERRORLEVEL%"

if not "%EXIT_CODE%"=="0" (
    echo.
    echo Ashkorix exited with code %EXIT_CODE%.
    pause
)

exit /b %EXIT_CODE%
