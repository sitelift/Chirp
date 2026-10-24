# Downloads redistributables only; does not install Rust, VS, or a CUDA toolkit.
param([string]$Cache = 'C:/tmp/chirp-deps', [switch]$Fp32Model)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
New-Item -ItemType Directory -Force $Cache | Out-Null
$version = '1.12.36'
$archives = @{
    'sherpa-onnx-lib' = "sherpa-onnx-v$version-win-x64-shared-MT-Release"
    'sherpa-onnx-lib-cuda' = "sherpa-onnx-v$version-cuda-12.x-cudnn-9.x-win-x64-cuda"
}
foreach ($target in $archives.Keys) {
    $name = $archives[$target]
    $archive = Join-Path $Cache "$name.tar.bz2"
    if (!(Test-Path $archive)) {
        & curl.exe -fL "https://github.com/k2-fsa/sherpa-onnx/releases/download/v$version/$name.tar.bz2" -o $archive
        if ($LASTEXITCODE) { throw "Download failed: $name" }
    }
    & tar -xjf $archive -C $Cache
    if ($LASTEXITCODE) { throw "Extraction failed: $name" }
    $dest = Join-Path $repo "src-tauri/$target"
    New-Item -ItemType Directory -Force $dest | Out-Null
    Get-ChildItem "$Cache/$name/lib" -File | Where-Object Extension -in '.dll','.lib' | Copy-Item -Destination $dest -Force
}
$wheels = @{
    'nvidia-cublas-cu12' = '12.9.2.10'
    'nvidia-cudnn-cu12' = '9.27.0.42'
    'nvidia-cuda-runtime-cu12' = '12.9.79'
    'nvidia-cufft-cu12' = '11.4.1.4'
}
foreach ($package in $wheels.Keys) {
    $release = Invoke-RestMethod "https://pypi.org/pypi/$package/$($wheels[$package])/json"
    $wheel = $release.urls | Where-Object filename -Match 'win_amd64.whl$' | Select-Object -First 1
    if (!$wheel) { throw "No Windows wheel for $package" }
    $archive = Join-Path $Cache "$package-$($wheels[$package]).zip"
    if (!(Test-Path $archive)) {
        & curl.exe -fL $wheel.url -o $archive
        if ($LASTEXITCODE) { throw "Download failed: $package" }
    }
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $wheel.digests.sha256) { throw "Checksum mismatch: $package" }
    $extract = Join-Path $Cache "$package-$($wheels[$package])"
    Expand-Archive -LiteralPath $archive -DestinationPath $extract -Force
    Get-ChildItem $extract -Recurse -Filter '*.dll' | Copy-Item -Destination "$repo/src-tauri/sherpa-onnx-lib-cuda/" -Force
}
# CPU is the portable bundling default. run.ps1 isolates CUDA for comparisons.
Copy-Item "$repo/src-tauri/sherpa-onnx-lib/*.dll" "$repo/src-tauri/" -Force
if ($Fp32Model) {
    $name = 'sherpa-onnx-nemo-parakeet-tdt-0.6b-v3'
    $metadata = Invoke-RestMethod "https://huggingface.co/api/models/csukuangfj/$name"
    $dest = "$env:APPDATA/com.chirp.app/models/sherpa/$name"
    New-Item -ItemType Directory -Force $dest | Out-Null
    foreach ($file in @('encoder.onnx','encoder.weights','decoder.onnx','joiner.onnx','tokens.txt')) {
        & curl.exe -fL "https://huggingface.co/csukuangfj/$name/resolve/$($metadata.sha)/${file}?download=true" -o "$dest/$file.tmp"
        if ($LASTEXITCODE) { throw "Model download failed: $file" }
        Move-Item -LiteralPath "$dest/$file.tmp" -Destination "$dest/$file" -Force
    }
    Write-Output "FP32 model ready (upstream revision $($metadata.sha))"
}
Write-Output 'CPU and CUDA runtimes ready. No toolkit installation required.'
