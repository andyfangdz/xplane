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

$xplane = $null
if ($XPlanePath) {
    $xplane = (Resolve-Path -LiteralPath $XPlanePath).Path
} else {
    $candidate = (Resolve-Path (Join-Path $workspace "..\..\..")).Path
    if ((Test-Path -LiteralPath (Join-Path $candidate "X-Plane.exe")) -and
        (Test-Path -LiteralPath (Join-Path $candidate "Resources\plugins"))) {
        $xplane = $candidate
    }
}
if (-not $xplane -or -not (Test-Path -LiteralPath (Join-Path $xplane "X-Plane.exe"))) {
    throw "An X-Plane installation is required to run the native plugin tests. Pass -XPlanePath or set XPLANE_PATH."
}

$sdkRuntimeDirectory = Join-Path $xplane "Resources\plugins"
$originalProcessPath = $env:Path
$env:Path = "$sdkRuntimeDirectory;$originalProcessPath"
Push-Location $workspace
try {
    & $Cargo test --workspace
    if ($LASTEXITCODE -ne 0) { throw "cargo test failed" }
    & $Cargo build --release -p $pluginSpec.Package
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
} finally {
    Pop-Location
    $env:Path = $originalProcessPath
}

$artifact = Join-Path $workspace ("target\release\" + $pluginSpec.Artifact)
if ($BuildOnly) {
    Write-Host "Built $artifact"
    return
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
