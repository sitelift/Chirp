# Windows x64 build prerequisites. No Python, CUDA toolkit, or model downloads.
[CmdletBinding()]
param(
    [string]$Cache = 'C:/tmp/chirp-deps',
    [switch]$Bundle,
    # Exact Microsoft.VC143.CRT directory, for a fixed VS toolchain/CI image.
    [string]$VcRedistDir = ''
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$vs = $null
if (Test-Path -LiteralPath $vswhere) {
    $env:PATH = "$(Split-Path $vswhere -Parent);$env:PATH"
    $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
}
if (!(Get-Command lib.exe -ErrorAction SilentlyContinue)) {
    if (!$vs) { throw 'Install VS 2022 Build Tools with the Desktop development with C++ workload.' }
    $dev = Join-Path $vs 'Common7/Tools/VsDevCmd.bat'
    # Import the developer environment without logging its contents.
    $lines = & cmd.exe /d /c "`"$dev`" -arch=x64 >nul && set"
    if ($LASTEXITCODE) { throw 'Could not initialize the x64 Visual Studio developer environment.' }
    foreach ($line in $lines) {
        if ($line -match '^([^=]+)=(.*)$') { [Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process') }
    }
}
New-Item -ItemType Directory -Force $Cache | Out-Null
$Cache = (Resolve-Path -LiteralPath $Cache).Path
$name = 'sherpa-onnx-v1.12.36-win-x64-shared-MT-Release'
$archive = Join-Path $Cache "$name.tar.bz2"
$sha256 = '0c027b0729f28e229082ff53d4a1cc8f6636e736ef4cefe05069d75a613cfc58'
if (!(Test-Path -LiteralPath $archive)) {
    & curl.exe -fL --retry 3 "https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.12.36/$name.tar.bz2" -o "$archive.part"
    if ($LASTEXITCODE) { throw 'CPU runtime download failed.' }
    if ((Get-FileHash -LiteralPath "$archive.part" -Algorithm SHA256).Hash.ToLowerInvariant() -ne $sha256) { throw 'CPU runtime checksum mismatch.' }
    Move-Item -LiteralPath "$archive.part" -Destination $archive -Force
}
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $sha256) { throw "CPU runtime checksum mismatch: $archive" }
& tar.exe -xjf $archive -C $Cache
if ($LASTEXITCODE) { throw 'CPU runtime extraction failed.' }
$native = Join-Path $repo 'src-tauri/sherpa-onnx-lib'
New-Item -ItemType Directory -Force $native | Out-Null
Get-ChildItem -LiteralPath "$Cache/$name/lib" -File | Where-Object Extension -in '.dll','.lib' | Copy-Item -Destination $native -Force
Get-ChildItem -LiteralPath "$Cache/$name/lib" -Filter '*.dll' | Copy-Item -Destination "$repo/src-tauri" -Force
$imports = Join-Path $Cache 'chirp-import-libs'
New-Item -ItemType Directory -Force $imports | Out-Null
& lib.exe "/DEF:$native/onnxruntime.def" "/OUT:$imports/onnxruntime.lib" /MACHINE:X64
if ($LASTEXITCODE) { throw 'ONNX import library generation failed.' }
Copy-Item -LiteralPath "$imports/onnxruntime.lib" -Destination "$native/onnxruntime.lib" -Force
Write-Output "CPU runtime ready: sherpa-onnx 1.12.36 (SHA-256 verified)."

if ($Bundle) {
    if (!$VcRedistDir -and $env:VCToolsRedistDir) {
        $VcRedistDir = Join-Path $env:VCToolsRedistDir 'x64/Microsoft.VC143.CRT'
    }
    if (!$VcRedistDir -and $vs) {
        $versions = Get-ChildItem -LiteralPath "$vs/VC/Redist/MSVC" -Directory |
            Where-Object { $_.Name -match '^\d+\.\d+\.\d+$' } |
            Sort-Object { [version]$_.Name } -Descending
        foreach ($version in $versions) {
            $candidate = Join-Path $version.FullName 'x64/Microsoft.VC143.CRT'
            if (Test-Path -LiteralPath $candidate) { $VcRedistDir = $candidate; break }
        }
    }
    if (!$VcRedistDir -or !(Test-Path -LiteralPath $VcRedistDir)) {
        throw 'VC143 x64 redistributables missing. Install VS C++ tools (14.44 or newer), or pass -VcRedistDir <Microsoft.VC143.CRT directory>.'
    }
    $VcRedistDir = (Resolve-Path -LiteralPath $VcRedistDir).Path
    # The bundle manifest is the source of truth; do not rely on developer DLLs.
    $base = Get-Content "$repo/src-tauri/tauri.conf.json" -Raw | ConvertFrom-Json
    $bundleConfig = Get-Content "$repo/src-tauri/tauri.bundle.conf.json" -Raw | ConvertFrom-Json
    $files = @($bundleConfig.bundle.resources | Where-Object { $_ -notin $base.bundle.resources })
    foreach ($file in $files) {
        if (!(Test-Path -LiteralPath (Join-Path $VcRedistDir $file))) { throw "Required redistributable $file missing in $VcRedistDir. Use VS C++ tools 14.44 or newer." }
    }
    foreach ($file in $files) { Copy-Item -LiteralPath (Join-Path $VcRedistDir $file) -Destination "$repo/src-tauri/$file" -Force }
    Write-Output "Provisioned $($files.Count) bundle CRT DLLs from $VcRedistDir"
}
