$ErrorActionPreference='Stop'
$current=Get-Content "$PSScriptRoot/current-trial.json" -Raw | ConvertFrom-Json
$run=Join-Path 'D:/X-Plane 12/Output/performance-tests' $current.trial.name
$record=@{utc=[datetime]::UtcNow.ToString('o');sequence=$current.trial.sequence;texture=$current.trial.texture_resolution;wrapper_phase=$current.phase}
if(Test-Path "$run/status.json"){
 $s=Get-Content "$run/status.json" -Raw | ConvertFrom-Json
 if($current.phase -eq 'running'){
  try{$s=Invoke-RestMethod ($s.status_url+'/status') -TimeoutSec 3}catch{}
 }
 $record.phase=$s.phase;$record.card=$s.card;$record.completed=$s.completed
 $record.native_seconds=$s.native.sim_time;$record.error=$s.error
}
if(Test-Path "$run/flight-pacing.json"){
 $f=Get-Content "$run/flight-pacing.json" -Raw | ConvertFrom-Json
 $record.runner_complete=$f.complete;$record.restored=$f.restored;$record.errors=$f.errors
 $record.flights=@($f.flights|Select-Object card,valid,landing_passed,@{n='over_50ms';e={$_.primary.over_50ms}},@{n='max_ms';e={$_.primary.max_ms}})
}
$record | ConvertTo-Json -Depth 10
