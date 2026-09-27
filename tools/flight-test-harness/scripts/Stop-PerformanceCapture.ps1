param([Parameter(Mandatory)][string]$RunDirectory)
. "$PSScriptRoot/Session-Common.ps1"
$m=Get-Content -LiteralPath "$RunDirectory/session.json" -Raw|ConvertFrom-Json
$null=Assert-InRoot $RunDirectory (Join-Path $m.xplane_root 'Output/performance-tests')
$queue=Join-Path $m.xplane_root 'Output/performance-tests/.xpt-wpr-host'
foreach($start in Get-ChildItem -LiteralPath "$RunDirectory/segments" -Filter 'wpr-start.json' -Recurse -ErrorAction SilentlyContinue){
 $stop=Join-Path $start.DirectoryName 'wpr-stop.json'
 if(Test-Path -LiteralPath $stop){continue}
 $record=Get-Content -LiteralPath $start.FullName -Raw|ConvertFrom-Json
 if($record.instance -notmatch '^XPT_[a-f0-9]{32}_[a-z0-9_-]{1,80}$'){throw 'Invalid owned WPR instance'}
 $key=[guid]::NewGuid().ToString('N')
 Write-Record (Join-Path $queue "request-$key.json") @{nonce=$record.nonce;action='stop';instance=$record.instance}
 $response=Join-Path $queue "response-$key.json";$deadline=[datetime]::UtcNow.AddSeconds(300)
 while(-not (Test-Path -LiteralPath $response) -and [datetime]::UtcNow -lt $deadline){Start-Sleep -Milliseconds 250}
 if(Test-Path -LiteralPath $response){
  $answer=Get-Content -LiteralPath $response -Raw|ConvertFrom-Json
  if($answer.ok){Write-Record $stop $answer.result}
  else{Write-Record (Join-Path $start.DirectoryName 'wpr-recovery-error.json') @{error=$answer.error}}
 }else{Write-Record (Join-Path $start.DirectoryName 'wpr-recovery-error.json') @{error='Host did not acknowledge recovery; retained owned capture may expire at its 600-second deadline'}}
}
$pmPath=Join-Path $RunDirectory 'presentmon-capture.json'
if(Test-Path -LiteralPath $pmPath){
 $pm=Get-Content -LiteralPath $pmPath -Raw|ConvertFrom-Json
 if(-not $pm.stopped){
  $expected=Join-Path $m.project_root 'bin/PresentMon/PresentMon-2.5.1-x64.exe'
  if($pm.path -ne $expected -or $pm.session_name -notmatch '^XPT_PM_[a-f0-9]{32}$'){throw 'Invalid PresentMon ownership record'}
  if((Get-FileHash -LiteralPath $expected).Hash -ne '9bec3083069f58f911e6a512f4806db51a27bd096103087bc1d05ef54c80a191'){throw 'PresentMon executable changed'}
  $output=& $expected --session_name $pm.session_name --terminate_existing_session 2>&1|Out-String
  Write-Record "$RunDirectory/presentmon-recovery.json" @{exit_code=$LASTEXITCODE;output=$output}
 }
}
