param([string]$XPlaneRoot='D:\X-Plane 12',[string]$Cargo='cargo')
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
$workspace=[IO.Path]::GetFullPath((Join-Path $project '../..'))
if(-not (Get-Command $Cargo -ErrorAction SilentlyContinue)){$Cargo=Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe'}
if(-not (Test-Path -LiteralPath "$workspace/Cargo.toml")){throw 'Rust workspace is missing'}
Push-Location $workspace
try {
 & $Cargo build --release --locked -p poweroff180-controller -p poweroff180-hud -p poweroff180-attitude
 if($LASTEXITCODE -ne 0){throw 'Rust plugin build failed'}
 $metadata=& $Cargo metadata --no-deps --format-version 1
 if($LASTEXITCODE -ne 0){throw 'Cargo target lookup failed'}
 $target=($metadata | ConvertFrom-Json).target_directory
 foreach($helper in @(@{Name='XPTNativeGuidance';Dll='poweroff180_controller.dll'},@{Name='XPTVideoHUD';Dll='poweroff180_hud.dll'},@{Name='SR20G6TestController';Dll='poweroff180_attitude.dll'})){
  $output=Join-Path $project ('build/'+$helper.Name+'/win_x64')
  New-Item -ItemType Directory -Force -Path $output | Out-Null
  Copy-Item -LiteralPath (Join-Path $target ('release/'+$helper.Dll)) -Destination (Join-Path $output ($helper.Name+'.xpl')) -Force
 }
 Get-FileHash "$project/build/XPTNativeGuidance/win_x64/XPTNativeGuidance.xpl","$project/build/XPTVideoHUD/win_x64/XPTVideoHUD.xpl","$project/build/SR20G6TestController/win_x64/SR20G6TestController.xpl" |
  ConvertTo-Json | Set-Content "$project/build/native-build.json"
} finally { Pop-Location }
