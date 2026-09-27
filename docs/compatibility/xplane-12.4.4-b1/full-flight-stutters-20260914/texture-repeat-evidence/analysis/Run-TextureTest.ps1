param([Parameter(Mandatory)][ValidateRange(1,6)][int]$Sequence,[switch]$AfterRecovery)
$ErrorActionPreference='Stop'
$harness='V:/src/xplane/tools/flight-test-harness'
$python=Join-Path $harness '.venv/Scripts/python.exe'
$nv='V:/src/xplane/build/stutter-fix-20260914/nv_present.py'
. "$harness/scripts/Session-Common.ps1"
$plan=Get-Content "$PSScriptRoot/plan.json" -Raw | ConvertFrom-Json
$baseline=Get-Content "$PSScriptRoot/preflight.json" -Raw | ConvertFrom-Json -DateKind String
$trial=@($plan.runs | Where-Object sequence -eq $Sequence)[0]
if(-not $trial){throw 'Unplanned trial'}
$run=Join-Path 'D:/X-Plane 12/Output/performance-tests' $trial.name
if((Test-Path -LiteralPath $run) -and -not $AfterRecovery){throw 'Run already exists; retain it and use an explicitly documented replacement'}

function Get-TrialAudit([string]$Stage){
 if(Get-Process X-Plane -ErrorAction SilentlyContinue){throw 'An X-Plane process is active'}
 $persistent=@(foreach($file in $baseline.persistent_files){
  $hash=(Get-FileHash -LiteralPath $file.path -Algorithm SHA256).Hash.ToLower()
  if($hash -ne $file.sha256){throw "Persistent file differs: $($file.path)"}
  @{path=$file.path;sha256=$hash;matches=$true}
 })
 $hags=(Get-ItemProperty 'HKLM:/SYSTEM/CurrentControlSet/Control/GraphicsDrivers' -Name HwSchMode).HwSchMode
 $os=Get-ItemProperty 'HKLM:/SOFTWARE/Microsoft/Windows NT/CurrentVersion'
 $boot=(Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToUniversalTime().ToString('o')
 if($hags -ne $baseline.hags_override -or $os.CurrentBuild -ne $baseline.build -or $os.UBR -ne $baseline.ubr -or $boot -ne $baseline.boot_utc){throw 'HAGS, Windows build or boot changed'}
 if((Get-FileHash 'D:/X-Plane 12/X-Plane.exe' -Algorithm SHA256).Hash.ToLower() -ne $baseline.simulator_sha256){throw 'Simulator binary changed'}
 $task=Get-ScheduledTask -TaskPath '\CodexXPlaneTracing\' -TaskName 'XPlaneTraceHost'
 if([string]$task.State -ne 'Ready'){throw 'Tracing host is not idle'}
 foreach($terrain in $baseline.terrain_files){if(-not (Test-Path -LiteralPath $terrain -PathType Leaf)){throw "Scenery mount missing: $terrain"}}
 $nvPath=Join-Path $PSScriptRoot "$($trial.name)-nv-$Stage.json"
 & $python $nv read --output $nvPath | Out-Null
 if($LASTEXITCODE -ne 0){throw 'NVIDIA profile read failed'}
 $nvNow=Get-Content $nvPath -Raw | ConvertFrom-Json
 $nvBase=Get-Content "$PSScriptRoot/nv-baseline.json" -Raw | ConvertFrom-Json
 if(($nvNow.profile_settings | ConvertTo-Json -Depth 20 -Compress) -cne ($nvBase.profile_settings | ConvertTo-Json -Depth 20 -Compress) -or ($nvNow.setting | ConvertTo-Json -Depth 10 -Compress) -cne ($nvBase.setting | ConvertTo-Json -Depth 10 -Compress)){throw 'NVIDIA profile changed'}
 $audit=@{utc=[datetime]::UtcNow.ToString('o');stage=$Stage;trial=$trial;persistent_files=$persistent;hags_override=$hags;build=$os.CurrentBuild;ubr=$os.UBR;boot_utc=$boot;nvidia_matches=$true;tracing_task_state=[string]$task.State}
 Write-Record (Join-Path $PSScriptRoot "$($trial.name)-$Stage.json") $audit
}

if($AfterRecovery){
 $restore=Get-Content "$run/restoration.json" -Raw | ConvertFrom-Json
 if(-not $restore.restored){throw 'Verified restoration is missing'}
 Get-TrialAudit 'after'
 Copy-Item -LiteralPath "$PSScriptRoot/$($trial.name)-before.json","$PSScriptRoot/$($trial.name)-after.json","$PSScriptRoot/$($trial.name)-nv-before.json","$PSScriptRoot/$($trial.name)-nv-after.json" -Destination $run
 Write-Record "$PSScriptRoot/current-trial.json" @{trial=$trial;phase='restored_after_recovery';exit_code=1;utc=[datetime]::UtcNow.ToString('o')}
 exit 0
}
Get-TrialAudit 'before'
Write-Record "$PSScriptRoot/current-trial.json" @{trial=$trial;phase='running';utc=[datetime]::UtcNow.ToString('o')}
$code=1
try {
 & "$harness/Run-XPlaneTest.ps1" -Mode Flight -TraceFlight -Config "$PSScriptRoot/config.json" -Name $trial.name -RenderingProfile User2D -KeepCustomScenery -VisibleSimulator -TextureCacheGB 10 -TextureResolution $trial.texture_resolution -NoDefragment -IsolateGlobalPlugin TelemFFB-XPP -SkipBuild
 $code=$LASTEXITCODE
} finally {
 # Installation recovery belongs to the existing transaction. Never move files here.
 Write-Record "$PSScriptRoot/current-trial.json" @{trial=$trial;phase='runner_returned';exit_code=$code;utc=[datetime]::UtcNow.ToString('o')}
}
$restore=Get-Content "$run/restoration.json" -Raw | ConvertFrom-Json
if(-not $restore.restored){throw 'Verified restoration is missing'}
Get-TrialAudit 'after'
Copy-Item -LiteralPath "$PSScriptRoot/$($trial.name)-before.json","$PSScriptRoot/$($trial.name)-after.json","$PSScriptRoot/$($trial.name)-nv-before.json","$PSScriptRoot/$($trial.name)-nv-after.json" -Destination $run
Write-Record "$PSScriptRoot/current-trial.json" @{trial=$trial;phase='restored';exit_code=$code;utc=[datetime]::UtcNow.ToString('o')}
exit $code
