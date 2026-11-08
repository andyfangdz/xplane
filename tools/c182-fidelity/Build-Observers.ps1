param(
    [Parameter(Mandatory)][string]$Prepar3DPath,
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '..\..\build\c182-fidelity'),
    [string]$VcVars32,
    [string]$SevenZip = '7z'
)
$ErrorActionPreference = 'Stop'
$sdk = Join-Path $Prepar3DPath 'redist\Interface\FSX-SP2-XPACK\retail'
if (-not (Test-Path -LiteralPath (Join-Path $sdk 'inc\SimConnect.h'))) {
    throw 'The installed FSX-SP2-XPACK SimConnect headers are required.'
}
if (-not $VcVars32) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vs) { throw 'Visual Studio x86 C++ build tools not found.' }
    $VcVars32 = Join-Path $vs 'VC\Auxiliary\Build\vcvars32.bat'
}
if (-not (Test-Path -LiteralPath $VcVars32)) { throw 'vcvars32.bat not found.' }
$output = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $output | Out-Null
$include = Join-Path $sdk 'inc'
# These values go into a batch file. Quotes and percent expansion are not valid
# in its path arguments; reject them rather than treating JSON as shell quoting.
foreach ($path in @($output, $include, $VcVars32, $PSScriptRoot)) {
    if ($path -match '["%\r\n]') { throw 'Unsupported shell metacharacter in build path.' }
}
$lines = @('@echo off', 'setlocal DisableDelayedExpansion', "call `"$VcVars32`" >nul", 'if errorlevel 1 exit /b 1')
foreach ($name in @('capture_p3d', 'load_p3d_scenario')) {
    $source = Join-Path $PSScriptRoot "$name.cpp"
    $obj = Join-Path $output "$name.obj"
    $exe = Join-Path $output "$name.exe"
    $lines += "cl /nologo /std:c++17 /EHsc /MT /W4 /WX /external:W0 /external:I `"$include`" `"$source`" /Fo`"$obj`" /Fe`"$exe`""
    $lines += 'if errorlevel 1 exit /b 1'
}
$batch = Join-Path $output 'build-observers.cmd'
$lines | Set-Content -LiteralPath $batch
& cmd.exe /d /c $batch
if ($LASTEXITCODE -ne 0) { throw 'Observer compilation failed.' }
$runtime = Join-Path $output 'simconnect-runtime'
New-Item -ItemType Directory -Force -Path $runtime | Out-Null
& $SevenZip x (Join-Path $sdk 'lib\SimConnect.msi') "-o$runtime" -y
if ($LASTEXITCODE -ne 0) { throw 'Could not extract the installed SimConnect runtime.' }
$dlls = @(Get-ChildItem -LiteralPath $runtime -File -Filter 'SimConnect.dll.sxs.*')
if ($dlls.Count -ne 1) { throw 'Expected exactly one native SimConnect runtime.' }
Copy-Item -LiteralPath $dlls[0].FullName -Destination (Join-Path $runtime 'SimConnect.dll') -Force
Write-Output "Built observers and local runtime in $output. Do not redistribute vendor runtime files."
