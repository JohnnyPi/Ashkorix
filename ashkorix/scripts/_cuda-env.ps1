# Shared CUDA environment setup for Ashkorix Windows builds.
# Dot-source from other scripts: . "$PSScriptRoot\_cuda-env.ps1"

function Resolve-AshkorixCudaRoot {
    if ($env:ASHKORIX_CUDA_PATH -and (Test-Path (Join-Path $env:ASHKORIX_CUDA_PATH "bin\nvcc.exe"))) {
        return $env:ASHKORIX_CUDA_PATH
    }

    if ($env:CUDA_PATH -and (Test-Path (Join-Path $env:CUDA_PATH "bin\nvcc.exe"))) {
        $systemCuda = $env:CUDA_PATH
    } else {
        $systemCuda = $null
    }

    # Prefer the toolkit version that matches the newest VS CUDA MSBuild integration.
    $candidates = @(
        "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.3",
        "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.5"
    )
    foreach ($path in $candidates) {
        if (Test-Path (Join-Path $path "bin\nvcc.exe")) {
            return $path
        }
    }

    if ($systemCuda) {
        return $systemCuda
    }

    throw @"
No CUDA toolkit found.
Install CUDA 13.x or 12.x, or set ASHKORIX_CUDA_PATH to your toolkit root
(e.g. C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.3).
"@
}

function Set-AshkorixCudaEnvironment {
    $cuda = Resolve-AshkorixCudaRoot
    $env:CUDA_PATH = $cuda
    $env:CUDA_HOME = $cuda
    $ver = Split-Path $cuda -Leaf
    $label = "CUDA_PATH_" + ($ver -replace '\.', '_')
    Set-Item -Path "env:$label" -Value $cuda
    $env:PATH = "$cuda\bin\x64;$cuda\bin;$env:PATH"

    $nvcc = Join-Path $cuda "bin\nvcc.exe"
    $env:CMAKE_CUDA_COMPILER = $nvcc
    $env:CMAKE_CUDA_FLAGS = "-allow-unsupported-compiler"

    $llvm = "C:\Program Files\LLVM\bin"
    if (Test-Path $llvm) {
        $env:LIBCLANG_PATH = $llvm
    }

    return $cuda
}

function Resolve-VsVars64 {
    $candidates = @(
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat",
        "E:\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat"
    )
    foreach ($path in $candidates) {
        if (Test-Path $path) {
            return $path
        }
    }
    return $null
}

function Invoke-AshkorixCargo {
    param(
        [string[]]$CargoArgs,
        [switch]$UseVsDevShell
    )

    $ashkorixRoot = Join-Path $PSScriptRoot ".."
    $cuda = Set-AshkorixCudaEnvironment
    Write-Host "Using CUDA at: $cuda"

    if ($UseVsDevShell) {
        $vsVars = Resolve-VsVars64
        if (-not $vsVars) {
            throw "Could not find vcvars64.bat for VS 2022. Install Desktop development with C++."
        }
        $nvcc = Join-Path $cuda "bin\nvcc.exe"
        $llvm = $env:LIBCLANG_PATH
        $cargoLine = "cargo " + ($CargoArgs -join " ")
        $cmd = @"
set "CUDA_PATH=$cuda"
set "CUDA_HOME=$cuda"
set "CMAKE_CUDA_COMPILER=$nvcc"
set "CMAKE_CUDA_FLAGS=-allow-unsupported-compiler"
set "CMAKE_GENERATOR=Visual Studio 17 2022"
set "LIBCLANG_PATH=$llvm"
cd /d "$ashkorixRoot"
$cargoLine
"@
        cmd /c "call `"$vsVars`" && $cmd"
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        return
    }

    Set-Location $ashkorixRoot
    & cargo @CargoArgs
}
