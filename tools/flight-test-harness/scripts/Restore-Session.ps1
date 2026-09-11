param([Parameter(Mandatory)][string]$RunDirectory)
. "$PSScriptRoot/Session-Common.ps1"
$RunDirectory=[IO.Path]::GetFullPath($RunDirectory)
$recordPath=Join-Path $RunDirectory 'session.json'
if(-not (Test-Path -LiteralPath $recordPath)){throw 'Session manifest missing'}
$m=Get-Content -LiteralPath $recordPath -Raw|ConvertFrom-Json
$root=[IO.Path]::GetFullPath($m.xplane_root)
$null=Assert-InRoot $RunDirectory (Join-Path $root 'Output/performance-tests')
$mutex=[Threading.Mutex]::new($false,"Local\XPT-Restore-$($m.token)")
$locked=$false
try{
 try{$locked=$mutex.WaitOne(30000)}catch [Threading.AbandonedMutexException]{$locked=$true}
 if(-not $locked){throw 'Another recovery is still active'}
 if(Test-Path -LiteralPath "$RunDirectory/restoration.json"){
  $r=Get-Content "$RunDirectory/restoration.json" -Raw|ConvertFrom-Json
  if($r.restored){return}
 }
 # A standalone recovery can also stop the orphaned worker. Never stop its caller.
 if($m.worker -and $m.worker.pid -ne $PID){Stop-TrackedProcess $m.worker}
 Stop-TrackedProcess $m.simulator
 if(Get-Process X-Plane -ErrorAction SilentlyContinue){throw 'An X-Plane process is active; recovery will not move files'}
 if(Test-Path -LiteralPath "$root/Log.txt"){Copy-Item -LiteralPath "$root/Log.txt" -Destination "$RunDirectory/Log.txt" -Force}
 foreach($helper in $m.helpers){
  if(Test-Path -LiteralPath $helper.live){Move-Exact $helper.live $helper.retired $root}
 }
 foreach($plugin in $m.global_plugin_isolation){
  if(Test-Path -LiteralPath $plugin.parked){Move-Exact $plugin.parked $plugin.live $root}
  Assert-Files $plugin.live $plugin.files
  if(Test-Path -LiteralPath $plugin.parked){throw "Plugin quarantine remains: $($plugin.parked)"}
 }
 $live=Join-Path $root 'Custom Scenery';$park=Join-Path $RunDirectory 'original-scenery';$placeholder=Join-Path $RunDirectory 'test-scenery-placeholder'
 if(Test-Path -LiteralPath $park){
  if(Test-Path -LiteralPath $live){
   if(-not (Test-Path -LiteralPath "$live/xpt-marker.txt") -or (Get-Content "$live/xpt-marker.txt" -Raw) -ne $m.token){throw 'Scenery placeholder marker mismatch'}
   foreach($entry in Get-ChildItem -LiteralPath $live -Force){
    if($entry.Name -eq 'xpt-marker.txt'){continue}
    if($entry.Name -ne 'scenery_packs.ini'){throw "Unexpected placeholder content: $($entry.Name)"}
    if((Get-Content -LiteralPath $entry.FullName -Raw).Replace("`r`n","`n") -ne "I`n1000 Version`nSCENERY`n`nSCENERY_PACK *GLOBAL_AIRPORTS*`n"){throw 'Unexpected generated scenery index'}
   }
   Move-Exact $live $placeholder $root
  }
  try{Move-Exact $park $live $root}catch{
   if(-not (Test-Path -LiteralPath $live) -and (Test-Path -LiteralPath $placeholder)){Move-Exact $placeholder $live $root}
   throw
  }
 }
 if($m.backups_ready){
  foreach($state in $m.states){
   if(Test-Path -LiteralPath $state.backup){
    if(Test-Path -LiteralPath $state.live){Move-Exact $state.live $state.after $root}
    Move-Exact $state.backup $state.live $root
   }
   Assert-Files $state.live $state.files
  }
 }
 if($m.scenery_inventory){Assert-Inventory $m.scenery_inventory (Get-TreeInventory $live)}
 if($m.global_plugin_names){
  $names=@(Get-ChildItem -LiteralPath "$root/Resources/plugins" -Force|Select-Object -ExpandProperty Name|Sort-Object)
  if(($names -join "`n") -ne (@($m.global_plugin_names)-join "`n")){throw 'Global plugin names changed'}
 }
 foreach($source in $m.source_hashes){if((Get-FileHash -LiteralPath $source.path).Hash -ne $source.sha256){throw "Original source changed: $($source.path)"}}
 if(Test-Path -LiteralPath "$RunDirectory/protected-install-state.json"){
  & "$PSScriptRoot/Protect-XPlaneInstallState.ps1" -Mode Verify -XPlaneRoot $root -SnapshotPath "$RunDirectory/protected-install-state.json" | Set-Content "$RunDirectory/protected-state-verification.json"
  if($LASTEXITCODE -and $LASTEXITCODE -ne 0){throw 'Protected state verification failed'}
 }
 Write-Record "$RunDirectory/restoration.json" @{restored=$true;utc=[datetime]::UtcNow.ToString('o');source_hashes_match=$true;helpers_removed=$true;state_files_verified=@($m.states|ForEach-Object {$_.files.Count});scenery_count=@($m.scenery_inventory.names).Count;scenery_links=@($m.scenery_inventory.links).Count;exact_name_sets_verified=$true;isolated_plugins_restored=@($m.global_plugin_isolation|ForEach-Object {$_.name})}
}catch{
 Write-Record "$RunDirectory/recovery-error.json" @{utc=[datetime]::UtcNow.ToString('o');error=$_.Exception.Message}
 throw
}finally{if($locked){$mutex.ReleaseMutex()};$mutex.Dispose()}
