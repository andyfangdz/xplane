param([string]$XPlaneRoot='D:\X-Plane 12',[int]$LifetimeMinutes=120)
# The elevated part only controls its own named WPR sessions. X-Plane and the
# ordinary test worker stay unelevated. Requests cannot supply commands/profiles.
$ErrorActionPreference='Stop'
. "$PSScriptRoot/Session-Common.ps1"
$root=[IO.Path]::GetFullPath($XPlaneRoot).TrimEnd('\','/')
$base=Join-Path $root 'Output/performance-tests'
if(-not (Test-Path -LiteralPath "$root/X-Plane.exe")){throw 'X-Plane root is invalid'}
if($LifetimeMinutes -lt 1 -or $LifetimeMinutes -gt 180){throw 'Invalid host lifetime'}
$queue=Join-Path $base '.xpt-wpr-host'
New-Item -ItemType Directory -Force -Path $queue | Out-Null
$lock=[IO.File]::Open((Join-Path $queue 'host.lock'),[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)
$nonce=[guid]::NewGuid().ToString('N')
$active=$null
$completed=@{}
$profile=Join-Path $PSScriptRoot 'XPT-Performance.wprp'
$profileHash=(Get-FileHash -LiteralPath $profile -Algorithm SHA256).Hash
$probeStarted=$false
function Invoke-OwnWpr([string[]]$WprArgs){
 $result=& "$env:WINDIR/System32/wpr.exe" @WprArgs 2>&1 | Out-String
 if($LASTEXITCODE -ne 0){throw $result.Trim()}
 return $result
}
function Stop-OwnCapture {
 if($null -eq $script:active){return $null}
 $state=$script:active
 $status=Invoke-OwnWpr @('-status','-instancename',$state.instance)
 $details=Invoke-OwnWpr @('-status','collectors','-details','-instancename',$state.instance)
 $output=Invoke-OwnWpr @('-stop',$state.output,'XPT performance segment','-skipPdbGen','-instancename',$state.instance)
 $script:active=$null
 $result=@{status=$status;collector_details=$details;output=$state.output;stop_output=$output;stopped_utc=[datetime]::UtcNow.ToString('o')}
 Write-Record (Join-Path (Split-Path $state.output -Parent) 'wpr-stop.json') $result
 $script:completed[$state.instance]=$result
 return $result
}
try {
 # Prove the required privilege before advertising readiness.
 $probe="XPT_privilege_$nonce"
 $null=Invoke-OwnWpr @('-start','CPU','-instancename',$probe)
 $probeStarted=$true
 $null=Invoke-OwnWpr @('-stop',(Join-Path $queue "privilege-$nonce.etl"),'-skipPdbGen','-instancename',$probe)
 $probeStarted=$false
 Write-Record (Join-Path $queue 'host.json') @{ready=$true;nonce=$nonce;root=$root;profile_sha256=$profileHash;protocol=2;process=(Get-ProcessRecord (Get-Process -Id $PID));started_utc=[datetime]::UtcNow.ToString('o')}
 $deadline=[datetime]::UtcNow.AddMinutes($LifetimeMinutes)
 while([datetime]::UtcNow -lt $deadline){
  if(Test-Path -LiteralPath (Join-Path $queue "stop-$nonce.request")){break}
  if($active -and [datetime]::UtcNow -gt $active.deadline){$null=Stop-OwnCapture}
  foreach($request in Get-ChildItem -LiteralPath $queue -Filter 'request-*.json'){
   $response=Join-Path $queue ($request.BaseName.Replace('request-','response-')+'.json')
   if(Test-Path -LiteralPath $response){continue}
   try{
    $job=Get-Content -LiteralPath $request.FullName -Raw | ConvertFrom-Json
    if($job.nonce -ne $nonce){throw 'Stale host request'}
    if($job.action -eq 'ping'){
     $result=@{ready=$true;nonce=$nonce;protocol=2;profile_sha256=$profileHash;active_instance=$(if($active){$active.instance}else{$null})}
    }elseif($job.action -eq 'stop'){
     if($completed.ContainsKey($job.instance)){$result=$completed[$job.instance]}
     else{
      if(-not $active -or $job.instance -ne $active.instance){throw 'Capture is not owned by this request'}
      $result=Stop-OwnCapture
     }
    }elseif($job.action -eq 'start'){
     if($active){throw 'A capture is already active'}
     if($job.segment -notmatch '^[a-z0-9_-]{1,80}$'){throw 'Invalid segment identifier'}
     $run=Assert-InRoot ([IO.Path]::GetFullPath($job.run_directory)) $base
     $session=Get-Content -LiteralPath (Join-Path $run 'session.json') -Raw | ConvertFrom-Json
     if([IO.Path]::GetFullPath($session.xplane_root).TrimEnd('\','/') -ne $root){throw 'Session root mismatch'}
     if([IO.Path]::GetFullPath($session.simulator.executable) -ne (Join-Path $root 'X-Plane.exe')){throw 'Only the configured X-Plane executable can be traced'}
     if(-not (Test-ProcessIdentity $session.simulator)){throw 'Simulator identity mismatch'}
     $segment=Assert-InRoot (Join-Path $run ('segments/'+$job.segment)) $run
     $check=Get-Item -LiteralPath $segment
     while($check.FullName.Length -ge $base.Length){
      if($check.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Trace output must not traverse links'}
      $check=$check.Parent
     }
     $out=Join-Path $segment 'system.etl'
     if(Test-Path -LiteralPath $out){throw 'Trace output already exists'}
     if($session.token -notmatch '^[a-f0-9]{32}$'){throw 'Session token is invalid'}
     $instance='XPT_'+$nonce+'_'+$session.token.Substring(0,8)+'_'+$job.segment
     if((Get-FileHash -LiteralPath $profile -Algorithm SHA256).Hash -ne $profileHash){throw 'Fixed WPR profile changed'}
     $null=Invoke-OwnWpr @('-start',($profile+'!XPT'),'-instancename',$instance)
     $script:active=@{instance=$instance;output=$out;deadline=[datetime]::UtcNow.AddSeconds(600)}
     $result=@{instance=$instance;profile_sha256=$profileHash;started_utc=[datetime]::UtcNow.ToString('o');output=$out}
    }else{throw 'Unsupported trace request'}
    Write-Record $response @{ok=$true;result=$result}
   }catch{Write-Record $response @{ok=$false;error=$_.Exception.Message}}
  }
  Start-Sleep -Milliseconds 250
 }
}finally{
 if($probeStarted){$null=Invoke-OwnWpr @('-cancel','-instancename',$probe)}
 try{$null=Stop-OwnCapture}finally{
  Write-Record (Join-Path $queue 'host.json') @{ready=$false;nonce=$nonce;root=$root;stopped_utc=[datetime]::UtcNow.ToString('o')}
  $lock.Dispose()
 }
}
