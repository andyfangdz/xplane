param(
    [string]$Zig = 'zig',
    [string]$SdkPath = $env:XPLANE_SDK_PATH,
    [string]$OutputPath = ''
)
$ErrorActionPreference = 'Stop'
if (-not $SdkPath) { throw 'Pass -SdkPath pointing to the XPSDK430 SDK directory, or set XPLANE_SDK_PATH.' }
$sdk = (Resolve-Path -LiteralPath $SdkPath).Path
if (-not (Test-Path -LiteralPath (Join-Path $sdk 'CHeaders/XPLM/XPLMPlugin.h'))) { throw 'Invalid X-Plane SDK directory.' }
$compiler = (Get-Command $Zig -ErrorAction Stop).Source
$outDir = if ($OutputPath) { [IO.Path]::GetFullPath($OutputPath) } else { Join-Path $PSScriptRoot '../../target/shuttle-hud' }
New-Item -ItemType Directory -Path $outDir -Force | Out-Null
foreach ($suite in @('math', 'guidance', 'presentation')) {
    $test = Join-Path $outDir "test_$suite.exe"
    & $compiler c++ -std=c++17 -O2 -Wall -Wextra -Werror (Join-Path $PSScriptRoot "test_$suite.cpp") -o $test
    if ($LASTEXITCODE) { throw "HUD $suite build failed" }
    & $test
    if ($LASTEXITCODE) { throw "HUD $suite test failed" }
}
& $compiler c++ -target x86_64-windows-gnu -std=c++17 -O2 -Wall -Wextra -Werror -Wno-nullability-completeness -shared -DIBM=1 -DAPL=0 -DLIN=0 -DXPLM410=1 -DXPLM400=1 -DXPLM303=1 -DXPLM301=1 -DXPLM300=1 -DXPLM210=1 -DXPLM200=1 -I (Join-Path $sdk 'CHeaders/XPLM') (Join-Path $PSScriptRoot 'shuttle_hud.cpp') (Join-Path $sdk 'Libraries/Win/XPLM_64.lib') -lopengl32 -o (Join-Path $outDir 'ShuttleHUD.dll')
if ($LASTEXITCODE) { throw 'HUD plugin build failed' }
Copy-Item -LiteralPath (Join-Path $outDir 'ShuttleHUD.dll') -Destination (Join-Path $outDir 'win.xpl') -Force
$inputs = @('shuttle_hud.cpp','hud_math.hpp','hud_presentation.hpp','landing_guidance.hpp','ValidationController.lua','hud-optics.txt','install_native.py','build.ps1') | ForEach-Object { Join-Path $PSScriptRoot $_ }
Get-FileHash -Algorithm SHA256 -LiteralPath (@((Join-Path $outDir 'win.xpl')) + $inputs) | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $outDir 'build-hashes.json')
Write-Host "Built $outDir/win.xpl; all three test suites passed."
