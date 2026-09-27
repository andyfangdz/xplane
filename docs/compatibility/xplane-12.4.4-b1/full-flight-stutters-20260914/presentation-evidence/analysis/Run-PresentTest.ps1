param(
 [Parameter(Mandatory)][ValidatePattern('^XPT_[A-Z0-9_]+$')][string]$Name,
 [ValidateSet('Original','DXGI')][string]$Method='DXGI'
)
$ErrorActionPreference='Stop'
$harness='V:\src\xplane\tools\flight-test-harness'
$python=Join-Path $harness '.venv/Scripts/python.exe'
$nv=Join-Path $PSScriptRoot 'nv_present.py'
$before=Join-Path $PSScriptRoot "$Name-nv-before.json"
$run=Join-Path 'D:\X-Plane 12\Output\performance-tests' $Name
if(Get-Process X-Plane -ErrorAction SilentlyContinue){throw 'X-Plane is already running'}
if(Test-Path -LiteralPath $run){throw 'Run already exists'}
if(Test-Path -LiteralPath $before){throw 'Recovery record already exists'}
$code=1
try {
 if($Method -eq 'DXGI'){
  & $python $nv apply --value 1 --before $before --output (Join-Path $PSScriptRoot "$Name-nv-applied.json")
 }else{
  & $python $nv read --output $before
 }
 if($LASTEXITCODE -ne 0){throw 'NVIDIA setting preparation failed'}
 & "$harness/Run-XPlaneTest.ps1" -Mode Flight -TraceFlight -Config "$harness/configs/stutter-full-flight.json" -Name $Name -RenderingProfile User2D -KeepCustomScenery -VisibleSimulator -TextureCacheGB 10 -TextureResolution 5 -NoDefragment -IsolateGlobalPlugin TelemFFB-XPP -SkipBuild
 $code=$LASTEXITCODE
}finally{
 if($Method -eq 'DXGI' -and (Test-Path -LiteralPath $before)){
  & $python $nv restore --value 1 --before $before --output (Join-Path $PSScriptRoot "$Name-nv-restored.json")
  if($LASTEXITCODE -ne 0){throw "NVIDIA restore failed; recovery snapshot: $before"}
 }
 if(Test-Path -LiteralPath $run){
  Get-ChildItem -LiteralPath $PSScriptRoot -Filter "$Name-nv-*.json" | Copy-Item -Destination $run
 }
}
exit $code
