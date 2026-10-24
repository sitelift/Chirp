# Renders every `spoken` line in bench/corpus/manifest.json to a WAV with Windows SAPI
# (System.Speech, fully offline). Output: 16 kHz, 16-bit, mono PCM, 300 ms of silence
# at the start and end.
#
# Deterministic: voice and rate are chosen from the entry's index in the manifest, voices
# are sorted by name, and the WAV header is written by hand, so reruns on the same machine
# (same installed voices) are byte-identical.
#
# Usage (Windows PowerShell 5.1 or pwsh on Windows):
#   powershell -ExecutionPolicy Bypass -File bench/tools/render_corpus.ps1
#   powershell -ExecutionPolicy Bypass -File bench/tools/render_corpus.ps1 -Only chat-01,email-05
#
# Entries whose id starts with "human-" are real recordings and are never rendered.

[CmdletBinding()]
param(
    [string]$Manifest = '',
    [string[]]$Only = @()
)

$ErrorActionPreference = 'Stop'
if (-not $Manifest) {
    $scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
    $Manifest = Join-Path $scriptDir '..\corpus\manifest.json'
}
Add-Type -AssemblyName System.Speech

$SampleRate = 16000
$Bits = 16
$Channels = 1
$SilenceMs = 300
# Small, deterministic rate variation (SAPI range is -10..10; 0 is the voice default).
$Rates = @(0, 1, -1, 2, 0, -1, 1)

$manifestPath = (Resolve-Path $Manifest).Path
$corpusDir = Split-Path -Parent $manifestPath
$entries = Get-Content -Raw -Encoding UTF8 $manifestPath | ConvertFrom-Json

$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
$voices = @($synth.GetInstalledVoices() |
    Where-Object { $_.Enabled -and $_.VoiceInfo.Culture.Name -like 'en-*' } |
    ForEach-Object { $_.VoiceInfo.Name } |
    Sort-Object)
if ($voices.Count -eq 0) { throw 'No enabled English SAPI voices installed.' }
Write-Host ("Voices: " + ($voices -join ', '))

$format = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(
    $SampleRate,
    [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen,
    [System.Speech.AudioFormat.AudioChannel]::Mono)

$blockAlign = $Channels * $Bits / 8
$silence = New-Object byte[] ([int]($SampleRate * $SilenceMs / 1000) * $blockAlign)

function Write-Wav([string]$path, [byte[]]$pcm) {
    $dataLen = $silence.Length * 2 + $pcm.Length
    $fs = [System.IO.File]::Create($path)
    try {
        $w = New-Object System.IO.BinaryWriter($fs)
        $ascii = [System.Text.Encoding]::ASCII
        $w.Write($ascii.GetBytes('RIFF')); $w.Write([int32](36 + $dataLen))
        $w.Write($ascii.GetBytes('WAVE'))
        $w.Write($ascii.GetBytes('fmt ')); $w.Write([int32]16)
        $w.Write([int16]1)                                  # PCM
        $w.Write([int16]$Channels)
        $w.Write([int32]$SampleRate)
        $w.Write([int32]($SampleRate * $blockAlign))        # byte rate
        $w.Write([int16]$blockAlign)
        $w.Write([int16]$Bits)
        $w.Write($ascii.GetBytes('data')); $w.Write([int32]$dataLen)
        $w.Write($silence); $w.Write($pcm); $w.Write($silence)
        $w.Flush()
    } finally { $fs.Dispose() }
}

$rendered = 0
for ($i = 0; $i -lt $entries.Count; $i++) {
    $e = $entries[$i]
    if ($e.id -like 'human-*') { continue }
    if ($Only.Count -gt 0 -and $Only -notcontains $e.id) { continue }

    $voice = $voices[$i % $voices.Count]
    $rate = $Rates[$i % $Rates.Count]
    $out = Join-Path $corpusDir $e.wav
    $dir = Split-Path -Parent $out
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Force $dir | Out-Null }

    # Fresh synthesizer per entry: SAPI carries state between Speak calls, which would make
    # the bytes depend on which entries were rendered before this one (e.g. with -Only).
    $synth.Dispose()
    $synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
    $synth.SelectVoice($voice)
    $synth.Rate = $rate
    $ms = New-Object System.IO.MemoryStream
    # Raw PCM (no header) in the requested format; we write our own header.
    $synth.SetOutputToAudioStream($ms, $format)
    $synth.Speak([string]$e.spoken)
    $synth.SetOutputToNull()
    $pcm = $ms.ToArray()
    $ms.Dispose()

    Write-Wav $out $pcm
    $secs = ($pcm.Length + 2 * $silence.Length) / ($SampleRate * $blockAlign)
    Write-Host ("{0,-10} {1,-24} rate {2,2}  {3,6:N2}s" -f $e.id, $voice, $rate, $secs)
    $rendered++
}

$synth.Dispose()
Write-Host "Rendered $rendered file(s)."
