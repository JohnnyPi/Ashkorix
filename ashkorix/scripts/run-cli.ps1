# Run ashkorix-cli with the same CUDA/data environment as the desktop app (Windows).
# Usage:
#   .\scripts\run-cli.ps1 -- doctor
#   .\scripts\run-cli.ps1 -- retrieve --query "What is Phase 1?"
#   .\scripts\run-cli.ps1 -- ask --query "..." --model "..\Data\models\your-model.gguf"
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CliArgs
)

$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_cuda-env.ps1"

$ashkorixRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$dataDir = if ($env:ASHKORIX_DATA_DIR) { $env:ASHKORIX_DATA_DIR } else { (Resolve-Path (Join-Path $ashkorixRoot "..\Data")).Path }
$env:ASHKORIX_DATA_DIR = $dataDir
Set-AshkorixCudaEnvironment | Out-Null

$exe = Join-Path $ashkorixRoot "target\debug\ashkorix.exe"
if (Test-Path $exe) {
    Set-Location $ashkorixRoot
    & $exe @CliArgs
    exit $LASTEXITCODE
}

$cargoArgs = @("run", "-p", "ashkorix-cli", "--")
if ($CliArgs) { $cargoArgs += $CliArgs }
Invoke-AshkorixCargo -CargoArgs $cargoArgs -UseVsDevShell
