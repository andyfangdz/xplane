param(
    [string]$Cargo = "cargo",
    [string]$XPlanePath = $env:XPLANE_PATH,
    [string]$Plugin = "position-aircraft",
    [switch]$BuildOnly
)

$ErrorActionPreference = "Stop"
$workspace = Split-Path -Parent $MyInvocation.MyCommand.Path
$plugins = @{
    "position-aircraft" = @{
        Package = "position-aircraft-native"
        Artifact = "position_aircraft_native.dll"
        InstallDirectory = "PositionAircraftNative"
    }
    "xgs" = @{
        Package = "xgs-rs"
        Artifact = "xgs_rs.dll"
        InstallDirectory = "XgsRust"
        Resources = "plugins\xgs\resources"
    }
}

if (-not $plugins.ContainsKey($Plugin)) {
    $available = ($plugins.Keys | Sort-Object) -join ", "
    throw "Unknown plugin '$Plugin'. Available plugins: $available"
}
$pluginSpec = $plugins[$Plugin]

Push-Location $workspace
try {
    & $Cargo test --workspace
    if ($LASTEXITCODE -ne 0) { throw "cargo test failed" }
    & $Cargo build --release -p $pluginSpec.Package
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
} finally {
    Pop-Location
}

$artifact = Join-Path $workspace ("target\release\" + $pluginSpec.Artifact)
if ($BuildOnly) {
    Write-Host "Built $artifact"
    return
}

if (-not $XPlanePath) {
    $candidate = (Resolve-Path (Join-Path $workspace "..\..\..")).Path
    if ((Test-Path -LiteralPath (Join-Path $candidate "X-Plane.exe")) -and
        (Test-Path -LiteralPath (Join-Path $candidate "Resources\plugins"))) {
        $XPlanePath = $candidate
    } else {
        throw "Pass -XPlanePath or set XPLANE_PATH to install the plugin."
    }
}
$xplane = (Resolve-Path -LiteralPath $XPlanePath).Path
if (-not (Test-Path -LiteralPath (Join-Path $xplane "X-Plane.exe"))) {
    throw "X-Plane.exe was not found under $xplane"
}
$pluginDestination = Join-Path $xplane ("Resources\plugins\" + $pluginSpec.InstallDirectory)
$destination = Join-Path $pluginDestination "64"
New-Item -ItemType Directory -Force -Path $destination | Out-Null
Copy-Item -LiteralPath $artifact -Destination (Join-Path $destination "win.xpl") -Force
if ($pluginSpec.Resources) {
    Get-ChildItem -LiteralPath (Join-Path $workspace $pluginSpec.Resources) -File |
        Copy-Item -Destination $pluginDestination -Force
}
Write-Host "Installed $Plugin to $destination\win.xpl"
