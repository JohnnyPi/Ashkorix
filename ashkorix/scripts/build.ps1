# Build Ashkorix on Windows with CUDA env aligned to Visual Studio.
# Usage:
#   .\scripts\build.ps1              # debug build (core + app + cli)
#   .\scripts\build.ps1 -Release     # release build
#   .\scripts\build.ps1 -- test -p ashkorix-core phase
param(
    [switch]$Release,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CargoArgs
)

$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_cuda-env.ps1"

$argsList = @("build", "-p", "ashkorix-core", "-p", "ashkorix-app", "-p", "ashkorix-cli")
if ($Release) { $argsList += "--release" }
if ($CargoArgs) { $argsList += $CargoArgs }

Invoke-AshkorixCargo -CargoArgs $argsList -UseVsDevShell
