param(
    [string]$Cargo = "cargo",
    [string]$XPlanePath = $env:XPLANE_PATH,
    [string]$SdkPath = $env:XPLM_SDK_PATH,
    [switch]$BuildOnly
)

$ErrorActionPreference = "Stop"
$project = Split-Path -Parent $MyInvocation.MyCommand.Path

if (-not $SdkPath) {
    $workspaceSdk = Join-Path (Split-Path -Parent $project) "sr20-g6-custom-fm\XPSDK430\SDK"
    if (Test-Path -LiteralPath (Join-Path $workspaceSdk "Libraries\Win\XPLM_64.lib")) {
        $SdkPath = $workspaceSdk
    } else {
        throw "Set XPLM_SDK_PATH or pass -SdkPath with the X-Plane SDK directory."
    }
}
$env:XPLM_SDK_PATH = (Resolve-Path -LiteralPath $SdkPath).Path

Push-Location $project
try {
    & $Cargo test
    if ($LASTEXITCODE -ne 0) { throw "cargo test failed" }
    & $Cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
} finally {
    Pop-Location
}

if ($BuildOnly) {
    Write-Host "Built target\release\position_aircraft_native.dll"
    return
}

if (-not $XPlanePath) {
    $candidate = (Resolve-Path (Join-Path $project "..\..\..")).Path
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
$destination = Join-Path $xplane "Resources\plugins\PositionAircraftNative\64"
New-Item -ItemType Directory -Force -Path $destination | Out-Null
Copy-Item -LiteralPath (Join-Path $project "target\release\position_aircraft_native.dll") -Destination (Join-Path $destination "win.xpl") -Force
Write-Host "Installed PositionAircraftNative to $destination\win.xpl"
