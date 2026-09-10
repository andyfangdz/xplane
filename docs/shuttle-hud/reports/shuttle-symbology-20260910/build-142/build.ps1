param([string]$OutputPath = '')
$ErrorActionPreference = 'Stop'
$taskRoot = 'D:\X-Plane 12'
$compiler = Join-Path $taskRoot 'Output\build-tools\sr20-g6-custom-fm\zig-0.16.0\zig.exe'
$sdk = Join-Path $taskRoot 'Output\build-tools\sr20-g6-custom-fm\XPSDK430\SDK'
$outDir = if ($OutputPath) { $OutputPath } else { Join-Path $taskRoot 'Output\shuttle-hud-20260909' }
New-Item -ItemType Directory -Path $outDir -Force | Out-Null
& $compiler c++ -std=c++17 -O2 -Wall -Wextra -Werror (Join-Path $PSScriptRoot 'test_math.cpp') -o (Join-Path $outDir 'test_math.exe')
if ($LASTEXITCODE) { throw 'HUD math build failed' }
& (Join-Path $outDir 'test_math.exe')
if ($LASTEXITCODE) { throw 'HUD math test failed' }
& $compiler c++ -std=c++17 -O2 -Wall -Wextra -Werror (Join-Path $PSScriptRoot 'test_guidance.cpp') -o (Join-Path $outDir 'test_guidance.exe')
if ($LASTEXITCODE) { throw 'Landing guidance test build failed' }
& (Join-Path $outDir 'test_guidance.exe')
if ($LASTEXITCODE) { throw 'Landing guidance test failed' }
& $compiler c++ -std=c++17 -O2 -Wall -Wextra -Werror (Join-Path $PSScriptRoot 'test_presentation.cpp') -o (Join-Path $outDir 'test_presentation.exe')
if ($LASTEXITCODE) { throw 'HUD presentation test build failed' }
& (Join-Path $outDir 'test_presentation.exe')
if ($LASTEXITCODE) { throw 'HUD presentation test failed' }
& $compiler c++ -target x86_64-windows-gnu -std=c++17 -O2 -Wall -Wextra -Werror -Wno-nullability-completeness -shared -DIBM=1 -DAPL=0 -DLIN=0 -DXPLM410=1 -DXPLM400=1 -DXPLM303=1 -DXPLM301=1 -DXPLM300=1 -DXPLM210=1 -DXPLM200=1 -I (Join-Path $sdk 'CHeaders\XPLM') (Join-Path $PSScriptRoot 'shuttle_hud.cpp') (Join-Path $sdk 'Libraries\Win\XPLM_64.lib') -lopengl32 -o (Join-Path $outDir 'ShuttleHUD.dll')
if ($LASTEXITCODE) { throw 'HUD plugin build failed' }
Copy-Item -LiteralPath (Join-Path $outDir 'ShuttleHUD.dll') -Destination (Join-Path $outDir 'win.xpl') -Force
Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $outDir 'win.xpl'), (Join-Path $PSScriptRoot 'shuttle_hud.cpp'), (Join-Path $PSScriptRoot 'hud_math.hpp'), (Join-Path $PSScriptRoot 'hud_presentation.hpp'), (Join-Path $PSScriptRoot 'landing_guidance.hpp'), (Join-Path $PSScriptRoot 'ValidationController.lua'), (Join-Path $PSScriptRoot 'hud-optics.txt'), (Join-Path $PSScriptRoot 'install_native.py') | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $outDir 'build-hashes.json')
