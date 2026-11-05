param([string]$Sdk = $env:XPLM_SDK)

$ErrorActionPreference = 'Stop'

function Invoke-Compiler([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program failed with exit code $LASTEXITCODE" }
}

try {
    if (-not $Sdk) { throw 'Pass -Sdk <X-Plane SDK root or CHeaders/XPLM directory>, or set XPLM_SDK. Headers are not bundled.' }
    $sdkPath = (Resolve-Path -LiteralPath $Sdk).Path
    $sdkHeaders = if (Test-Path -LiteralPath (Join-Path $sdkPath 'XPLMDefs.h')) {
        $sdkPath
    } else { Join-Path $sdkPath 'CHeaders/XPLM' }
    if (-not (Test-Path -LiteralPath (Join-Path $sdkHeaders 'XPLMWeather.h'))) {
        throw 'X-Plane SDK 4.1 headers were not found. Supply the SDK root or CHeaders/XPLM directory.'
    }

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

    Invoke-Compiler $compiler ($flags + $sdkFlags + @('/DXPLM=1', '/c', (Join-Path $PSScriptRoot 'src/fake_xplm.cpp'), "/Fo$build/fake_xplm.obj"))
    Invoke-Compiler $linker @('/nologo', '/DLL', '/MACHINE:X64', "/OUT:$build/XPLM_64.dll", "/IMPLIB:$build/XPLM_64.lib", "/PDB:$build/XPLM_64.pdb", '/DEBUG', "$build/fake_xplm.obj")
    foreach ($unit in @('xphost', 'serve')) {
        Invoke-Compiler $compiler ($flags + @('/c', (Join-Path $PSScriptRoot "src/$unit.cpp"), "/Fo$build/$unit.obj"))
    }
    Invoke-Compiler $linker @('/nologo', '/MACHINE:X64', "/OUT:$build/xphost.exe", "/PDB:$build/xphost.pdb", '/DEBUG', "$build/xphost.obj", "$build/serve.obj", 'opengl32.lib', 'user32.lib', 'gdi32.lib', 'ws2_32.lib')
    Invoke-Compiler $compiler ($flags + $sdkFlags + @('/c', (Join-Path $PSScriptRoot 'tests/fixture/plugin.cpp'), "/Fo$build/fixture.obj"))
    Invoke-Compiler $linker @('/nologo', '/DLL', '/MACHINE:X64', "/OUT:$fixturePlugin/win.xpl", "/IMPLIB:$build/fixture.lib", "/PDB:$build/fixture.pdb", '/DEBUG', "$build/fixture.obj", "$build/XPLM_64.lib")
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'tests/fixture/fixture.acf') -Destination (Join-Path $fixture 'fixture.acf') -Force
    Write-Host 'Built xphost, fake XPLM, and the standalone test fixture.'
    exit 0
} catch {
    Write-Error $_
    exit 1
}
