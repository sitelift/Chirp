param(
    [ValidateSet('auto','cpu','cuda')][string]$Provider = 'auto',
    [int]$Threads = 0,
    [int]$Iterations = 3,
    [string]$ModelDir = '',
    [string]$Label = '',
    [int]$Limit = 0,
    [double]$MinSeconds = 0,
    [double]$MaxSeconds = 100000,
    [switch]$Online,
    [switch]$Unpaced,
    [switch]$Llm,
    [switch]$Speculate,
    [switch]$CompareSpeculation,
    [switch]$WarmOnly,
    [double]$TrailingSilenceMs = -1,
    [switch]$MicLifecycle,
    [string]$Vad = '',
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
Set-Location $repo
$env:PATH = "$env:USERPROFILE/.cargo/bin;C:/Program Files/CMake/bin;C:/Program Files/LLVM/bin;${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer;$env:PATH"
$env:CARGO_TARGET_DIR = 'C:/tmp/chirp-target'
$env:LIBCLANG_PATH = 'C:/Program Files/LLVM/bin'
if (!$SkipBuild) {
    $vswhere = "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe"
    $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (!$vs) { throw 'Visual C++ workload is not ready yet.' }
    $vcvars = Join-Path $vs 'VC/Auxiliary/Build/vcvars64.bat'
    # Import the developer environment without writing its contents to logs.
    $lines = & cmd.exe /d /c "`"$vcvars`" >nul && set"
    if ($LASTEXITCODE) { throw 'Could not initialize the Visual C++ environment.' }
    foreach ($line in $lines) { if ($line -match '^([^=]+)=(.*)$') { [Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process') } }
    & cargo build --release --manifest-path src-tauri/Cargo.toml --bin bench
    if ($LASTEXITCODE) { throw 'Benchmark build failed' }
}
# The Windows loader prefers the executable directory over PATH. Isolate runtimes.
$runtimeProvider = if ($Provider -eq 'auto') { if (Test-Path 'src-tauri/sherpa-onnx-lib-cuda/onnxruntime_providers_cuda.dll') { 'cuda' } else { 'cpu' } } else { $Provider }
$runtime = Join-Path $env:CARGO_TARGET_DIR "bench-$runtimeProvider"
New-Item -ItemType Directory -Force $runtime | Out-Null
$native = if ($runtimeProvider -eq 'cuda') { 'src-tauri/sherpa-onnx-lib-cuda' } else { 'src-tauri/sherpa-onnx-lib' }
foreach ($dll in Get-ChildItem $native -Filter '*.dll') {
    $dest = Join-Path $runtime $dll.Name
    if (!(Test-Path $dest) -or (Get-Item $dest).LastWriteTimeUtc -ne $dll.LastWriteTimeUtc) { Copy-Item -LiteralPath $dll.FullName -Destination $dest -Force }
}
Copy-Item -LiteralPath "$env:CARGO_TARGET_DIR/release/bench.exe" -Destination "$runtime/bench.exe" -Force
$benchArgs = @('--provider',$Provider,'--iterations',"$Iterations",'--min-seconds',"$MinSeconds",'--max-seconds',"$MaxSeconds")
if ($Threads -gt 0) { $benchArgs += @('--threads',"$Threads") }
if ($ModelDir) { $benchArgs += @('--model-dir',$ModelDir) }
if ($Label) { $benchArgs += @('--label',$Label) }
if ($Limit -gt 0) { $benchArgs += @('--limit',"$Limit") }
if ($Online) { $benchArgs += '--online' }
if ($Unpaced) { $benchArgs += '--unpaced' }
if ($Llm) { $benchArgs += '--llm' }
if ($Speculate) { $benchArgs += '--speculate' }
if ($CompareSpeculation) { $benchArgs += '--compare-speculation' }
if ($WarmOnly) { $benchArgs += '--warm-only' }
if ($TrailingSilenceMs -ge 0) { $benchArgs += @('--trailing-silence-ms',"$TrailingSilenceMs") }
if ($MicLifecycle) { $benchArgs += '--mic-lifecycle' }
if ($Vad) { $benchArgs += @('--vad',$Vad) }
& "$runtime/bench.exe" @benchArgs
if ($LASTEXITCODE) {
    $failure = @{status='failed'; exit_code=$LASTEXITCODE; provider=$Provider; model_dir=$ModelDir; threads=$Threads; arguments=$benchArgs}
    New-Item -ItemType Directory -Force bench/results | Out-Null
    $failure | ConvertTo-Json -Depth 5 | Set-Content "bench/results/$([DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffZ'))-failure.json"
    throw "Benchmark failed ($LASTEXITCODE)"
}
