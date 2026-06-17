# Run Ashkorix Tauri dev with CUDA env (Windows).
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_cuda-env.ps1"

Set-AshkorixCudaEnvironment | Out-Null

$appDir = Join-Path (Split-Path $PSScriptRoot -Parent) "crates\ashkorix-app"
Set-Location $appDir
npm run tauri dev
