$ErrorActionPreference = "Stop"

$BuildDir = "build\win_dist"
$CmakeBin = "build\cmake\esotereel_gui.exe"
$RustDll = "build\target\release\esotereel_gui_helper.dll"
$NsisScript = "installer\windows\installer.nsi"

# クリーンアップとフォルダ作成
if (Test-Path $BuildDir) { Remove-Item -Recurse -Force $BuildDir }
New-Item -ItemType Directory -Force $BuildDir | Out-Null
New-Item -ItemType Directory -Force dist | Out-Null

# 実行ファイルと独自DLLのコピー
Copy-Item $CmakeBin $BuildDir\
Copy-Item $RustDll $BuildDir\

# Qt依存関係の収集
Write-Host "Running windeployqt..."
windeployqt.exe "$BuildDir\esotereel_gui.exe"

# FFmpeg依存関係の収集
Write-Host "Copying FFmpeg DLLs..."
$FfmpegDir = (Get-ChildItem C:\ffmpeg | Select-Object -First 1).FullName + "\bin"
Copy-Item "$FfmpegDir\*.dll" $BuildDir\

# NSISインストーラーのビルド
Write-Host "Building NSIS installer..."
makensis.exe $NsisScript