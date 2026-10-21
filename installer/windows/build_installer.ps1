[CmdletBinding()]
param(
    [ValidateSet('Release')]
    [string]$Configuration = 'Release',
    [string]$QtBin,
    [string]$MingwBin,
    [string]$FfmpegRoot,
    [switch]$IncludeQtTranslations
)

$ErrorActionPreference = 'Stop'
$RepoRoot = $PSScriptRoot
. (Join-Path $RepoRoot 'scripts\windows-runtime.ps1')

$BuildDir = Join-Path $RepoRoot 'build\win_dist'
$CmakeExe = Join-Path $RepoRoot 'build\cmake\esotereel_gui.exe'
$RustDir = Join-Path $RepoRoot 'build\target\release'
$RustDll = Join-Path $RustDir 'esotereel_gui_helper.dll'
$Toolchain = Resolve-EsotereelToolchain -RepoRoot $RepoRoot -QtBin $QtBin -MingwBin $MingwBin -FfmpegRoot $FfmpegRoot
$QtBin = $Toolchain.QtBin
$MingwBin = $Toolchain.MingwBin
$QtWindeployqt = $Toolchain.Windeployqt
$Objdump = $Toolchain.Objdump
$FfmpegBin = $Toolchain.FfmpegBin
$NsisDir = Join-Path $RepoRoot 'installer\windows'
$NsisScript = Join-Path $NsisDir 'installer.nsi'
$DockingBin = Join-Path $RepoRoot 'build\cmake\x64\bin'
$DockingBinAlt = Join-Path $RepoRoot 'build\cmake\bin'

$NsisCommand = Get-Command 'makensis.exe' -ErrorAction SilentlyContinue
if (-not $NsisCommand) {
    throw 'makensis.exe was not found on PATH. Install NSIS or add its directory to PATH.'
}

foreach ($path in @($CmakeExe, $RustDll, $QtWindeployqt, $Objdump, $NsisScript)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Required file not found: $path"
    }
}

$PluginDir = Join-Path $RepoRoot 'plugins'
if (-not (Test-Path -LiteralPath $PluginDir -PathType Container)) {
    throw "Plugin directory not found: $PluginDir"
}

# Stop before packaging if the EXE still links to the debug ADS DLL. A release
# installer should use the Release build of Qt Advanced Docking System.
$exeImports = & $Objdump -p $CmakeExe 2>$null
if ($LASTEXITCODE -ne 0) {
    throw "Could not inspect executable: $CmakeExe"
}
if (($exeImports | Where-Object { $_ -match 'DLL Name:\s*libqtadvanceddocking-qt6d\.dll\s*$' })) {
    throw 'The EXE imports libqtadvanceddocking-qt6d.dll (Debug). Reconfigure/rebuild build\cmake in Release against the Release Qt Advanced Docking System library before packaging.'
}

# Always recreate the distribution staging directory: no stale DLLs should leak
# into the installer from earlier builds.
if (Test-Path -LiteralPath $BuildDir) {
    Remove-Item -LiteralPath $BuildDir -Recurse -Force
}
New-Item -ItemType Directory -Path $BuildDir -Force | Out-Null
$DistDir = Join-Path $RepoRoot 'dist'
New-Item -ItemType Directory -Path $DistDir -Force | Out-Null

Copy-Item -LiteralPath $CmakeExe -Destination $BuildDir
Copy-Item -LiteralPath $RustDll -Destination $BuildDir

$env:PATH = "$MingwBin;$QtBin;$env:PATH"
Invoke-EsotereelWindeployqt `
    -WindeployqtPath $QtWindeployqt `
    -ExecutablePath (Join-Path $BuildDir 'esotereel_gui.exe') `
    -IncludeQtTranslations:$IncludeQtTranslations

$SearchRoots = @(
    $DockingBin,
    $DockingBinAlt,
    $MingwBin,
    $QtBin,
    $FfmpegBin,
    $RustDir
) | Select-Object -Unique

Resolve-EsotereelRuntimeDependencies `
    -DeployDir $BuildDir `
    -ExecutablePath (Join-Path $BuildDir 'esotereel_gui.exe') `
    -SearchRoots $SearchRoots `
    -ObjdumpPath $Objdump

# NSIS paths in installer.nsi are relative to installer\windows, so compile
# with that directory as the current working directory.
Write-Host 'Building NSIS installer...'
Push-Location $NsisDir
try {
    & $NsisCommand.Source '.\installer.nsi'
    if ($LASTEXITCODE -ne 0) {
        throw "makensis.exe failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

$InstallerPath = Join-Path $DistDir 'esotereel-installer.exe'
if (-not (Test-Path -LiteralPath $InstallerPath -PathType Leaf)) {
    throw "NSIS did not create the expected installer: $InstallerPath"
}

Write-Host "`nInstaller created: $InstallerPath" -ForegroundColor Green
