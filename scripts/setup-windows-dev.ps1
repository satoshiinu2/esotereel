[CmdletBinding()]
param(
    [ValidateSet('Debug', 'Release')]
    [string]$Configuration = 'Debug',
    [string]$QtBin,
    [string]$MingwBin,
    [string]$FfmpegRoot,
    [switch]$IncludeQtTranslations
)

$ErrorActionPreference = 'Stop'
$ScriptDir = $PSScriptRoot
$RepoRoot = Split-Path -Parent $ScriptDir
$RuntimeScript = Join-Path $ScriptDir 'windows-runtime.ps1'
if (-not (Test-Path -LiteralPath $RuntimeScript -PathType Leaf)) {
    throw "Runtime helper not found: $RuntimeScript"
}
. $RuntimeScript
$DeployDir = Join-Path $RepoRoot 'build\cmake'
$Executable = Join-Path $DeployDir 'esotereel_gui.exe'
$RustProfile = $Configuration.ToLowerInvariant()
$RustDir = Join-Path $RepoRoot "build\target\$RustProfile"
$RustDll = Join-Path $RustDir 'esotereel_gui_helper.dll'
$Toolchain = Resolve-EsotereelToolchain -RepoRoot $RepoRoot -QtBin $QtBin -MingwBin $MingwBin -FfmpegRoot $FfmpegRoot
$QtBin = $Toolchain.QtBin
$MingwBin = $Toolchain.MingwBin
$Windeployqt = $Toolchain.Windeployqt
$Objdump = $Toolchain.Objdump
$FfmpegBin = $Toolchain.FfmpegBin
$DockingBin = Join-Path $DeployDir 'x64\bin'
$DockingBinAlt = Join-Path $DeployDir 'bin'

foreach ($path in @($Executable, $RustDll, $Windeployqt, $Objdump)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Required file not found: $path"
    }
}

# Make the configured Qt kit and project's MinGW toolchain available to tools.
$env:PATH = "$MingwBin;$QtBin;$env:PATH"

Copy-Item -LiteralPath $RustDll -Destination $DeployDir -Force
Write-Host "Copied Rust helper ($Configuration): $RustDll"

Invoke-EsotereelWindeployqt `
    -WindeployqtPath $Windeployqt `
    -ExecutablePath $Executable `
    -IncludeQtTranslations:$IncludeQtTranslations

# Search order matters for duplicate runtime filenames: use the MinGW compiler
# selected for this project before the Qt kit's bundled runtime DLLs.
$SearchRoots = @(
    $DockingBin,
    $DockingBinAlt,
    $MingwBin,
    $QtBin,
    $FfmpegBin,
    $RustDir
) | Select-Object -Unique

Resolve-EsotereelRuntimeDependencies `
    -DeployDir $DeployDir `
    -ExecutablePath $Executable `
    -SearchRoots $SearchRoots `
    -ObjdumpPath $Objdump

Write-Host "`nDevelopment runtime prepared in: $DeployDir" -ForegroundColor Green
Write-Host "Run with: & '$Executable'"
