param(
    [string]$Cargo = 'cargo',
    [string]$XPlanePath = $env:XPLANE_PATH
)
$ErrorActionPreference = 'Stop'
# Share the repository test/build workflow; installation is aircraft-local.
& (Join-Path $PSScriptRoot '../../build.ps1') -Plugin shuttle-hud -BuildOnly -Cargo $Cargo -XPlanePath $XPlanePath
if ($LASTEXITCODE -ne 0) { throw 'Shuttle Rust build failed' }
