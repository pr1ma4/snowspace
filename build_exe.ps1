param([string]$PythonPath)

$ErrorActionPreference = 'Stop'

$project = $PSScriptRoot
$build = Join-Path $project '.build'
$python = $PythonPath
if (-not $python) {
    $localAppData = $env:LOCALAPPDATA
    if (-not $localAppData) { $localAppData = Join-Path $env:USERPROFILE 'AppData\Local' }
    $candidates = @(
        (Join-Path $localAppData 'Python\bin\python.exe'),
        (Join-Path $env:USERPROFILE 'AppData\Local\Python\bin\python.exe')
    )
    $candidates += @(Get-ChildItem (Join-Path $localAppData 'Programs\Python') -Filter python.exe -File -Recurse -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName)
    $python = $candidates | Where-Object { $_ -and (Test-Path -LiteralPath $_) } | Select-Object -First 1
}
$ffmpegZip = Join-Path $build 'ffmpeg.zip'
$ffmpegRoot = Join-Path $build 'ffmpeg'
$venv = Join-Path $build 'venv'
$dist = Join-Path $project 'dist'
$downloadUrl = 'https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-full_build.zip'

if (-not (Test-Path -LiteralPath $python)) {
    throw "Python executable not found: $python"
}
New-Item -ItemType Directory -Force -Path $build | Out-Null
if (-not (Test-Path -LiteralPath $venv)) {
    & $python -m venv $venv
}
$venvPython = Join-Path $venv 'Scripts\python.exe'
& $venvPython -m PyInstaller --version *> $null
if ($LASTEXITCODE -ne 0) {
    & $venvPython -m pip install pyinstaller
    if ($LASTEXITCODE -ne 0) { throw 'Could not install PyInstaller.' }
}

if (-not (Test-Path -LiteralPath $ffmpegZip)) {
    Write-Host 'Downloading FFmpeg 9.0.2 (includes ffmpeg and ffprobe)...'
    Invoke-WebRequest -Uri $downloadUrl -OutFile $ffmpegZip
}
if (-not (Test-Path -LiteralPath (Join-Path $ffmpegRoot 'bin\ffmpeg.exe'))) {
    Expand-Archive -LiteralPath $ffmpegZip -DestinationPath $ffmpegRoot -Force
}
$ffmpegExe = Get-ChildItem -LiteralPath $ffmpegRoot -Filter ffmpeg.exe -Recurse | Select-Object -First 1
$ffprobeExe = Get-ChildItem -LiteralPath $ffmpegRoot -Filter ffprobe.exe -Recurse | Select-Object -First 1
if (-not $ffmpegExe -or -not $ffprobeExe) {
    throw 'Could not find ffmpeg.exe and ffprobe.exe in the downloaded archive.'
}

New-Item -ItemType Directory -Force -Path $dist | Out-Null
& $venvPython -m PyInstaller --noconfirm --clean --onefile --windowed `
    --distpath $dist --workpath (Join-Path $build 'pyinstaller-work') --specpath $build `
    --name VideoCompressor `
    --add-binary "$($ffmpegExe.FullName);." `
    --add-binary "$($ffprobeExe.FullName);." `
    (Join-Path $project 'compress_video_gui.py')
if ($LASTEXITCODE -ne 0) {
    throw "PyInstaller failed with exit code $LASTEXITCODE"
}

$exe = Join-Path $project 'dist\VideoCompressor.exe'
if (-not (Test-Path -LiteralPath $exe)) {
    throw 'Build finished but VideoCompressor.exe was not created.'
}
$ffmpegLicense = Get-ChildItem -LiteralPath $ffmpegRoot -Filter LICENSE -Recurse | Select-Object -First 1
if ($ffmpegLicense) {
    Copy-Item -LiteralPath $ffmpegLicense.FullName -Destination (Join-Path $dist 'FFmpeg-LICENSE.txt') -Force
}
@'
Video Compressor bundles FFmpeg 9.0.2 (full_build) from GyanD.
FFmpeg project: https://ffmpeg.org/
Build and corresponding source information: https://github.com/GyanD/codexffmpeg/releases/tag/9.0.2
The included FFmpeg license text is in FFmpeg-LICENSE.txt.
FFmpeg is a separate command-line program invoked by this application.
'@ | Set-Content -LiteralPath (Join-Path $dist 'THIRD_PARTY_NOTICES.txt') -Encoding UTF8
Write-Host "Build complete: $exe"
Write-Host 'Copy VideoCompressor.exe and the accompanying license/notice files to another compatible Windows PC.'
