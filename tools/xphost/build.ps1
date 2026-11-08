param([string]$Sdk = $env:XPLM_SDK, [string]$Cargo = 'cargo', [switch]$DebugBuild)

$ErrorActionPreference = 'Stop'

function Invoke-Compiler([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program failed with exit code $LASTEXITCODE" }
}

try {
    $workspace = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
    if (-not (Get-Command $Cargo -ErrorAction SilentlyContinue)) {
        $rustupCargo = Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe'
        if ($Cargo -eq 'cargo' -and (Test-Path -LiteralPath $rustupCargo)) { $Cargo = $rustupCargo }
        else { throw 'Install stable Rust with the x86_64-pc-windows-msvc target, or pass -Cargo.' }
    }
    $metadataText = & $Cargo metadata --locked --format-version 1 --manifest-path (Join-Path $workspace 'Cargo.toml')
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
    $metadata = $metadataText | ConvertFrom-Json
    if (-not $Sdk) {
        $sdkPackage = $metadata.packages | Where-Object name -eq 'xplane-sdk-sys' | Select-Object -First 1
        $Sdk = Join-Path (Split-Path -Parent $sdkPackage.manifest_path) 'SDK'
    }
    $sdkPath = (Resolve-Path -LiteralPath $Sdk).Path
    $sdkHeaders = if (Test-Path -LiteralPath (Join-Path $sdkPath 'XPLMDefs.h')) { $sdkPath } else { Join-Path $sdkPath 'CHeaders/XPLM' }
    if (-not (Test-Path -LiteralPath (Join-Path $sdkHeaders 'XPLMWeather.h'))) { throw 'X-Plane SDK 4.1 fixture headers were not found.' }

    $compiler = Get-Command cl.exe -ErrorAction SilentlyContinue
    if (-not $compiler -or $env:VSCMD_ARG_TGT_ARCH -ne 'x64') {
        $vswhere = Get-Command vswhere.exe -ErrorAction SilentlyContinue
        $vswherePath = if ($vswhere) { $vswhere.Source } else {
            Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
        }
        if (-not (Test-Path -LiteralPath $vswherePath)) {
            throw 'Use an x64 Visual Studio developer shell, or install Visual Studio Build Tools with Desktop development with C++ (including the Windows SDK).'
        }
        $installation = & $vswherePath -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if (-not $installation) { throw 'vswhere did not find an MSVC x64 C++ toolchain.' }
        $vcvars = Join-Path $installation 'VC/Auxiliary/Build/vcvars64.bat'
        if (-not (Test-Path -LiteralPath $vcvars)) { throw 'The discovered MSVC installation has no vcvars64.bat.' }
        # Capture vcvars only to import its environment into this build process.
        $environment = & $env:ComSpec /d /s /c "`"$vcvars`" >nul && set"
        if ($LASTEXITCODE -ne 0) { throw 'vcvars64.bat failed.' }
        foreach ($line in $environment) {
            if ($line -match '^([^=]+)=(.*)$') {
                [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], 'Process')
            }
        }
    }
    $compiler = (Get-Command cl.exe -ErrorAction Stop).Source
    $linker = (Get-Command link.exe -ErrorAction Stop).Source
    $librarian = (Get-Command lib.exe -ErrorAction Stop).Source
    if ($env:VSCMD_ARG_TGT_ARCH -ne 'x64') { throw 'An x64 MSVC environment is required.' }
    (Get-Process -Id $PID).PriorityClass = 'BelowNormal'

    $build = Join-Path $PSScriptRoot 'build'
    $temporary = Join-Path $build 'tmp'
    $fixture = Join-Path $build 'fixture'
    $fixturePlugin = Join-Path $fixture 'plugins/test/64'
    $null = New-Item -ItemType Directory -Force -Path $temporary, $fixturePlugin
    $env:TEMP = $temporary
    $env:TMP = $temporary
    $flags = @('/nologo', '/std:c++17', '/EHsc', '/O2', '/MD', '/W4', '/D_CRT_SECURE_NO_WARNINGS', '/DNOMINMAX')
    $sdkFlags = @('/DIBM=1', '/DAPL=0', '/DLIN=0', '/DXPLM200=1', '/DXPLM210=1', '/DXPLM300=1', '/DXPLM301=1', '/DXPLM302=1', '/DXPLM400=1', '/DXPLM410=1', '/external:W0', "/external:I$sdkHeaders")

    $cargoArguments = @('build', '--locked', '--manifest-path', (Join-Path $workspace 'Cargo.toml'), '-p', 'xphost', '-p', 'xphost-sdk')
    $profile = if ($DebugBuild) { 'debug' } else { 'release' }
    if (-not $DebugBuild) { $cargoArguments += '--release' }
    Invoke-Compiler $Cargo $cargoArguments
    $artifacts = Join-Path $metadata.target_directory $profile
    Copy-Item -LiteralPath (Join-Path $artifacts 'xphost.exe') -Destination (Join-Path $build 'xphost.exe') -Force
    Copy-Item -LiteralPath (Join-Path $artifacts 'xphost_sdk.dll') -Destination (Join-Path $build 'XPLM_64.dll') -Force
    # The Cargo DLL is renamed for SDK consumers. Generate an import library
    # whose descriptor names XPLM_64.dll, so the independent C++ fixture uses
    # exactly the same import path as a real plugin.
    $exports = Get-Content -LiteralPath (Join-Path $workspace 'crates/xphost-sdk/src/exports.rs') -Raw
    $names = [regex]::Matches($exports, 'pub (?:unsafe )?extern "C" fn (XPLM\w+)\(') | ForEach-Object { $_.Groups[1].Value }
    $definition = Join-Path $build 'XPLM_64.def'
    @('LIBRARY XPLM_64.dll', 'EXPORTS') + $names | Set-Content -LiteralPath $definition -Encoding ascii
    Invoke-Compiler $librarian @('/nologo', '/MACHINE:X64', "/DEF:$definition", "/OUT:$build/XPLM_64.lib")
    & dumpbin.exe /nologo /exports (Join-Path $build 'XPLM_64.dll') | Set-Content -LiteralPath (Join-Path $build 'exports.txt')
    if ($LASTEXITCODE -ne 0) { throw 'dumpbin failed' }
    Invoke-Compiler $compiler ($flags + $sdkFlags + @('/c', (Join-Path $PSScriptRoot 'tests/fixture/plugin.cpp'), "/Fo$build/fixture.obj"))
    Invoke-Compiler $linker @('/nologo', '/DLL', '/MACHINE:X64', "/OUT:$fixturePlugin/win.xpl", "/IMPLIB:$build/fixture.lib", "/PDB:$build/fixture.pdb", '/DEBUG', "$build/fixture.obj", "$build/XPLM_64.lib")
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'tests/fixture/fixture.acf') -Destination (Join-Path $fixture 'fixture.acf') -Force
    Write-Host 'Built Rust xphost, Rust fake XPLM, and the independent C++ test fixture.'
    exit 0
} catch {
    Write-Error $_
    exit 1
}
