param(
 [string]$XPlaneRoot='D:\X-Plane 12',
 [string]$Name=('XPT_FLIGHT_STUTTER_'+[datetime]::UtcNow.ToString('yyyyMMdd_HHmmss')),
 [ValidateRange(1,5)][int]$Pairs=3,
 [switch]$SkipBuild
)
. "$PSScriptRoot/Session-Common.ps1"
$project=Split-Path $PSScriptRoot -Parent
if($Name -notmatch '^[a-zA-Z0-9_-]+$'){throw 'Invalid matrix name'}
$directory=Assert-InRoot (Join-Path $XPlaneRoot "Output/performance-tests/$Name") (Join-Path $XPlaneRoot 'Output/performance-tests')
if(Test-Path -LiteralPath $directory){throw 'Matrix already exists'}
New-Item -ItemType Directory -Path $directory|Out-Null
Copy-Item -LiteralPath "$project/configs/stutter-full-flight.json" -Destination "$directory/config.json"
$shell=(Get-Process -Id $PID).Path;$attempts=@();$abort=$false
for($pair=1;$pair -le $Pairs;$pair++){
 $order=if($pair%2){@(0,1)}else{@(1,0)}
 foreach($flag in $order){
  $runName="${Name}_${pair}_$flag"
  $attempt=@{run=$runName;pair=$pair;flag=($flag -eq 1);exit_code=$null;started_utc=[datetime]::UtcNow.ToString('o')}
  $attempts+=$attempt
  Write-Record "$directory/matrix.json" @{expected_runs=2*$Pairs;attempts=$attempts;ordering='AB/BA alternating pairs';cache_gb=10}
  $args=@('-NoProfile','-File',"$project/Run-XPlaneTest.ps1",'-Mode','Flight','-TraceFlight',
   '-Config',"$directory/config.json",'-XPlaneRoot',$XPlaneRoot,'-Name',$runName,
   '-RenderingProfile','User2D','-KeepCustomScenery','-VisibleSimulator','-TextureCacheGB','10','-IsolateGlobalPlugin','TelemFFB-XPP')
  if($flag){$args+='-NoDefragment'}
  if($SkipBuild -or $attempts.Count -gt 1){$args+='-SkipBuild'}
  & $shell @args
  $attempt.exit_code=$LASTEXITCODE;$attempt.ended_utc=[datetime]::UtcNow.ToString('o')
  Write-Record "$directory/matrix.json" @{expected_runs=2*$Pairs;attempts=$attempts;ordering='AB/BA alternating pairs';cache_gb=10}
  $restoration="$XPlaneRoot/Output/performance-tests/$runName/restoration.json"
  if(-not (Test-Path -LiteralPath $restoration) -or -not (Get-Content -LiteralPath $restoration -Raw|ConvertFrom-Json).restored){throw 'Unverified recovery blocks the next launch'}
  if($attempt.exit_code -ne 0){$abort=$true;break}
 }
 if($abort){break}
}
Push-Location $project
try{& "$project/.venv/Scripts/python.exe" -m xpt.flight_pacing --matrix --run $directory;$code=$LASTEXITCODE}finally{Pop-Location}
exit $code
