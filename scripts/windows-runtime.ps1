# Shared Windows runtime-dependency collector for Esotereel.
# Intended to be dot-sourced by setup-windows-dev.ps1 and build_installer.ps1.

function Resolve-EsotereelToolchain {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [string]$QtBin,
        [string]$MingwBin,
        [string]$FfmpegRoot
    )

    $cachePath = Join-Path $RepoRoot 'build\cmake\CMakeCache.txt'
    $cache = @{}
    if (Test-Path -LiteralPath $cachePath -PathType Leaf) {
        foreach ($line in Get-Content -LiteralPath $cachePath) {
            if ($line -match '^([^:#]+):[^=]*=(.*)$') {
                $cache[$Matches[1]] = $Matches[2]
            }
        }
    }

    # Prefer explicit parameters, then infer the toolchain from CMakeCache.txt,
    # then fall back to commands already available on PATH.
    if (-not $QtBin) {
        $qtConfig = $cache['Qt6_DIR']
        if ($qtConfig) {
            $prefix = $qtConfig
            # Qt6_DIR normally ends in <prefix>/lib/cmake/Qt6.
            for ($i = 0; $i -lt 3; $i++) { $prefix = Split-Path -Parent $prefix }
            $candidate = Join-Path $prefix 'bin'
            if (Test-Path -LiteralPath (Join-Path $candidate 'windeployqt.exe') -PathType Leaf) {
                $QtBin = $candidate
            }
        }
    }
    if (-not $QtBin) {
        $cmd = Get-Command 'windeployqt.exe' -ErrorAction SilentlyContinue
        if ($cmd) { $QtBin = Split-Path -Parent $cmd.Source }
    }
    if (-not $QtBin -or -not (Test-Path -LiteralPath (Join-Path $QtBin 'windeployqt.exe') -PathType Leaf)) {
        throw 'Could not locate windeployqt.exe. Put the matching Qt kit bin directory on PATH or pass -QtBin.'
    }

    if (-not $MingwBin) {
        $compiler = $cache['CMAKE_CXX_COMPILER']
        if ($compiler -and (Test-Path -LiteralPath $compiler -PathType Leaf)) {
            $candidate = Split-Path -Parent $compiler
            if (Test-Path -LiteralPath (Join-Path $candidate 'objdump.exe') -PathType Leaf) {
                $MingwBin = $candidate
            }
        }
    }
    if (-not $MingwBin) {
        $cmd = Get-Command 'objdump.exe' -ErrorAction SilentlyContinue
        if ($cmd) { $MingwBin = Split-Path -Parent $cmd.Source }
    }
    if (-not $MingwBin -or -not (Test-Path -LiteralPath (Join-Path $MingwBin 'objdump.exe') -PathType Leaf)) {
        throw 'Could not locate objdump.exe. Put the matching MinGW bin directory on PATH or pass -MingwBin.'
    }

    $ffmpegBin = Find-EsotereelFfmpegBin -RepoRoot $RepoRoot -FfmpegRoot $FfmpegRoot

    return [pscustomobject]@{
        QtBin       = (Resolve-Path -LiteralPath $QtBin).Path
        MingwBin    = (Resolve-Path -LiteralPath $MingwBin).Path
        Windeployqt = Join-Path $QtBin 'windeployqt.exe'
        Objdump     = Join-Path $MingwBin 'objdump.exe'
        FfmpegBin   = $ffmpegBin
    }
}

function Find-EsotereelFfmpegBin {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [string]$FfmpegRoot
    )

    $searchRoots = [System.Collections.Generic.List[string]]::new()
    $pathRoots = [System.Collections.Generic.List[string]]::new()

    if ($FfmpegRoot) { $searchRoots.Add($FfmpegRoot) }
    foreach ($envName in @('FFMPEG_ROOT', 'FFMPEG_DIR', 'FFMPEG_BIN')) {
        $value = [Environment]::GetEnvironmentVariable($envName)
        if ($value) { $searchRoots.Add($value) }
    }
    foreach ($relative in @('third_party\ffmpeg', 'deps\ffmpeg', 'external\ffmpeg', 'build\ffmpeg')) {
        $candidate = Join-Path $RepoRoot $relative
        if (Test-Path -LiteralPath $candidate -PathType Container) { $searchRoots.Add($candidate) }
    }
    foreach ($pathEntry in ($env:PATH -split [System.IO.Path]::PathSeparator)) {
        if ($pathEntry -and (Test-Path -LiteralPath $pathEntry -PathType Container)) {
            $pathRoots.Add($pathEntry)
        }
    }

    # Explicit roots may contain a nested FFmpeg package; PATH entries are only
    # checked directly to avoid recursively crawling every directory on PATH.
    foreach ($root in ($searchRoots | Select-Object -Unique)) {
        if (-not (Test-Path -LiteralPath $root -PathType Container)) { continue }
        foreach ($candidatePath in @(
            (Join-Path $root 'avformat-63.dll'),
            (Join-Path $root 'bin\avformat-63.dll')
        )) {
            if (Test-Path -LiteralPath $candidatePath -PathType Leaf) {
                $bin = Split-Path -Parent $candidatePath
                Write-Host "Using FFmpeg DLL directory: $bin"
                return (Resolve-Path -LiteralPath $bin).Path
            }
        }
        $candidate = Get-ChildItem -LiteralPath $root -Recurse -File -Filter 'avformat-63.dll' -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($candidate) {
            Write-Host "Using FFmpeg DLL directory: $($candidate.DirectoryName)"
            return $candidate.DirectoryName
        }
    }

    foreach ($root in ($pathRoots | Select-Object -Unique)) {
        $candidate = Join-Path $root 'avformat-63.dll'
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            Write-Host "Using FFmpeg DLL directory from PATH: $root"
            return (Resolve-Path -LiteralPath $root).Path
        }
    }

    return $null
}

function Invoke-EsotereelWindeployqt {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$WindeployqtPath,
        [Parameter(Mandatory = $true)][string]$ExecutablePath,
        [switch]$IncludeQtTranslations
    )

    if (-not (Test-Path -LiteralPath $WindeployqtPath -PathType Leaf)) {
        throw "windeployqt.exe not found: $WindeployqtPath"
    }
    if (-not (Test-Path -LiteralPath $ExecutablePath -PathType Leaf)) {
        throw "Application executable not found: $ExecutablePath"
    }

    # Resolve Qt libraries/plugins, but let the collector below use the exact
    # MinGW runtime from the compiler configured for this project.
    $arguments = @('--force', '--no-compiler-runtime')
    if (-not $IncludeQtTranslations) {
        $arguments += '--no-translations'
    }
    $arguments += $ExecutablePath

    Write-Host "Running windeployqt for: $ExecutablePath"
    & $WindeployqtPath @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "windeployqt failed with exit code $LASTEXITCODE"
    }
}

function Resolve-EsotereelRuntimeDependencies {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$DeployDir,
        [Parameter(Mandatory = $true)][string]$ExecutablePath,
        [Parameter(Mandatory = $true)][string[]]$SearchRoots,
        [Parameter(Mandatory = $true)][string]$ObjdumpPath
    )

    if (-not (Test-Path -LiteralPath $DeployDir -PathType Container)) {
        throw "Deployment directory not found: $DeployDir"
    }
    if (-not (Test-Path -LiteralPath $ExecutablePath -PathType Leaf)) {
        throw "Executable not found: $ExecutablePath"
    }
    if (-not (Test-Path -LiteralPath $ObjdumpPath -PathType Leaf)) {
        throw "objdump.exe not found: $ObjdumpPath"
    }

    # Build a case-insensitive filename index. Search-root order sets priority
    # when duplicate DLL filenames exist in multiple toolchain directories.
    $dllIndex = @{}
    foreach ($root in $SearchRoots) {
        if (-not (Test-Path -LiteralPath $root -PathType Container)) {
            Write-Warning "Optional DLL search directory not found: $root"
            continue
        }

        Get-ChildItem -LiteralPath $root -Recurse -File -Filter '*.dll' `
            -ErrorAction SilentlyContinue | ForEach-Object {
                if (-not $dllIndex.ContainsKey($_.Name)) {
                    $dllIndex[$_.Name] = $_.FullName
                }
            }
    }

    $queue = [System.Collections.Generic.Queue[string]]::new()
    $processed = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::OrdinalIgnoreCase
    )
    $unresolved = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::OrdinalIgnoreCase
    )

    $queue.Enqueue((Get-Item -LiteralPath $ExecutablePath).FullName)

    # Also inspect deployed DLLs and Qt plugins. Skip CMake's generated tools
    # and the source-only x64/bin tree; dependencies from the latter are copied
    # to the deployment root only when an actual import requires them.
    Get-ChildItem -LiteralPath $DeployDir -Recurse -File -Filter '*.dll' `
        -ErrorAction SilentlyContinue |
        Where-Object {
            $_.FullName -notmatch '[\\/](CMakeFiles|x64|target)[\\/]'
        } | ForEach-Object {
            $queue.Enqueue($_.FullName)
        }

    while ($queue.Count -gt 0) {
        $current = $queue.Dequeue()
        if (-not (Test-Path -LiteralPath $current -PathType Leaf)) { continue }

        $currentFullPath = (Get-Item -LiteralPath $current).FullName
        if (-not $processed.Add($currentFullPath)) { continue }

        $output = & $ObjdumpPath -p $currentFullPath 2>$null
        if ($LASTEXITCODE -ne 0) {
            Write-Warning "Could not inspect binary imports: $currentFullPath"
            continue
        }

        $imports = @(
            $output | ForEach-Object {
                if ($_ -match 'DLL Name:\s*(.+?)\s*$') {
                    $Matches[1].Trim()
                }
            } | Select-Object -Unique
        )

        foreach ($name in $imports) {
            # These are Windows API-set contracts, not redistributable DLLs.
            if ($name -match '^(api-ms-win-|ext-ms-win-)') { continue }

            $destination = Join-Path $DeployDir $name

            # Preserve Qt6 DLLs already deployed by windeployqt: Qt may patch
            # Qt6Core.dll for relocatable deployment. --force above makes
            # windeployqt refresh these files before this scan.
            if (($name -match '^Qt6.*\.dll$') -and
                (Test-Path -LiteralPath $destination -PathType Leaf)) {
                $queue.Enqueue($destination)
                continue
            }

            # Prefer the exact DLL from the configured source/toolchain for
            # non-Qt dependencies; this refreshes stale FFmpeg/MinGW/ADS copies.
            if ($dllIndex.ContainsKey($name)) {
                $source = $dllIndex[$name]
                $sourceFullPath = (Get-Item -LiteralPath $source).FullName

                if (-not $sourceFullPath.Equals(
                    [System.IO.Path]::GetFullPath($destination),
                    [System.StringComparison]::OrdinalIgnoreCase
                )) {
                    Copy-Item -LiteralPath $sourceFullPath -Destination $destination -Force
                    Write-Host "DLL: $name"
                }

                $queue.Enqueue($destination)
                continue
            }

            # If windeployqt or another deployment step has already placed the
            # DLL in the deployment root, inspect its imports.
            if (Test-Path -LiteralPath $destination -PathType Leaf) {
                $queue.Enqueue($destination)
                continue
            }

            # System DLLs are provided by Windows, not copied into the app.
            $systemDll = Join-Path (Join-Path $env:WINDIR 'System32') $name
            if (Test-Path -LiteralPath $systemDll -PathType Leaf) { continue }

            [void]$unresolved.Add("$name (imported by $currentFullPath)")
        }
    }

    if ($unresolved.Count -gt 0) {
        Write-Host ''
        Write-Host 'Unresolved DLL imports:' -ForegroundColor Red
        foreach ($item in ($unresolved | Sort-Object)) {
            Write-Host "  - $item" -ForegroundColor Red
        }
        throw "Could not resolve $($unresolved.Count) DLL import(s). Do not package this directory until they are resolved."
    }

    Write-Host 'Static DLL import scan completed without unresolved imports.' -ForegroundColor Green
    Write-Warning 'objdump cannot detect DLLs loaded only by LoadLibrary()/runtime plugin discovery. Review dynamically loaded libraries separately.'
}
