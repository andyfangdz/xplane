param(
 [ValidateSet('Flag','Cache')][string]$Compare='Flag',
 [string]$XPlaneRoot='D:\X-Plane 12',
 [string]$Name=('XPT_PERF_MATRIX_'+[datetime]::UtcNow.ToString('yyyyMMdd_HHmmss')),
 [ValidateRange(1,10)][int]$Repeats=3,
 [string]$Config="$PSScriptRoot/../configs/performance-stutters.json",
 [switch]$SkipBuild
)
# All simulator/installation changes remain inside the existing transaction.
. "$PSScriptRoot/Session-Common.ps1"
$project=Split-Path $PSScriptRoot -Parent
if($Name -notmatch '^[a-zA-Z0-9_-]+$'){throw 'Invalid matrix name'}
$directory=Assert-InRoot (Join-Path $XPlaneRoot "Output/performance-tests/$Name") (Join-Path $XPlaneRoot 'Output/performance-tests')
if(Test-Path -LiteralPath $directory){throw 'Matrix already exists'}
New-Item -ItemType Directory -Path $directory|Out-Null
$configValue=Get-Content -LiteralPath $Config -Raw|ConvertFrom-Json
$configValue.repeats=1
Write-Record "$directory/config.json" $configValue
$shell=(Get-Process -Id $PID).Path
$attempts=@()
for($repeat=1;$repeat -le $Repeats;$repeat++){
 $order=if($repeat%2){@(0,1)}else{@(1,0)}
 foreach($variant in $order){
  $runName="${Name}_${repeat}_$variant"
  $flag=if($Compare -eq 'Flag'){$variant -eq 1}else{$true}
  $cache=if($Compare -eq 'Cache' -and $variant -eq 0){0}else{10}
  $args=@('-NoProfile','-File',"$project/Run-XPlaneTest.ps1",'-Mode','Performance',
          '-Config',"$directory/config.json",'-XPlaneRoot',$XPlaneRoot,'-Name',$runName,
          '-RenderingProfile','User2D','-KeepCustomScenery','-VisibleSimulator','-TextureCacheGB',"$cache")
  if($flag){$args+='-NoDefragment'}
  if($SkipBuild -or $attempts.Count -gt 0){$args+='-SkipBuild'}
  $attempt=@{repeat=$repeat;variant=$variant;no_defragment=$flag;cache_gb=$cache;run=$runName;started_utc=[datetime]::UtcNow.ToString('o');exit_code=$null}
  $attempts+=$attempt;Write-Record "$directory/matrix.json" @{compare=$Compare;expected_runs=2*$Repeats;attempts=$attempts}
  & $shell @args
  $attempt.exit_code=$LASTEXITCODE;$attempt.ended_utc=[datetime]::UtcNow.ToString('o')
  Write-Record "$directory/matrix.json" @{compare=$Compare;expected_runs=2*$Repeats;attempts=$attempts}
  # Ordinary failed attempts remain in the matrix. Unverified recovery blocks
  # the next process so a failed transaction cannot contaminate later runs.
  $restored=Join-Path $XPlaneRoot "Output/performance-tests/$runName/restoration.json"
  if(-not (Test-Path -LiteralPath $restored) -or -not (Get-Content $restored -Raw|ConvertFrom-Json).restored){throw 'Recovery is not verified; matrix stopped'}
 }
}
Push-Location $project
try{& "$project/.venv/Scripts/python.exe" -m xpt.perf_matrix --run $directory}finally{Pop-Location}
exit $LASTEXITCODE
