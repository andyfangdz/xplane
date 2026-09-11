param(
 [string]$Config="$PSScriptRoot/configs/smoke.json",
 [string]$XPlaneRoot='D:\X-Plane 12',
 [string]$Name=('XPT_'+[datetime]::UtcNow.ToString('yyyyMMdd_HHmmss')),
 [int]$ApiPort=8153,
 [switch]$SkipBuild,
 [switch]$RecordVideo,
 [string[]]$IsolateGlobalPlugin=@(),
 [ValidateSet('','after_prepare','after_launch','owner_exit_after_prepare')][string]$InjectFailure=''
)
. "$PSScriptRoot/scripts/Session-Common.ps1"
if($Name -notmatch '^[a-zA-Z0-9_-]+$'){throw 'Run name must be a simple identifier'}
$root=[IO.Path]::GetFullPath($XPlaneRoot).TrimEnd('\','/')
$run=Join-Path $root "Output/performance-tests/$Name"
$python=Join-Path $PSScriptRoot '.venv/Scripts/python.exe'
$Config=[IO.Path]::GetFullPath($Config)
if(Test-Path -LiteralPath $run){throw 'Run directory already exists; use a fresh name'}
if(Get-Process X-Plane -ErrorAction SilentlyContinue){throw 'Close X-Plane before launching an isolated harness session'}
if(-not (Test-Path -LiteralPath $python)){& "$PSScriptRoot/scripts/Bootstrap.ps1"}
$null=Assert-InRoot $run (Join-Path $root 'Output/performance-tests')
$campaignLock=[IO.File]::Open((Join-Path $root 'Output/performance-tests/.xpt-session.lock'),[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)
$owner=Get-ProcessRecord (Get-Process -Id $PID)
$m=@{schema_version=1;token=[guid]::NewGuid().ToString('N');xplane_root=$root;project_root=$PSScriptRoot;owner=$owner;worker=$null;simulator=$null;backups_ready=$false;helpers=@();states=@();source_hashes=@();global_plugin_names=@();scenery_inventory=$null}
$m.record_video=$RecordVideo.IsPresent
$m.global_plugin_isolation=@()
$exitCode=1
try{
 Push-Location $PSScriptRoot
 try{
  & $python -m xpt.cli validate --config $Config
  if($LASTEXITCODE -ne 0){throw 'Configuration validation failed'}
  if(-not $SkipBuild){& "$PSScriptRoot/scripts/Build-Native.ps1" -XPlaneRoot $root}
 }finally{Pop-Location}
 $listener=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,$ApiPort)
 try{$listener.Start()}finally{$listener.Stop()}
 $helperNames=@('SR20G6TestController','XPTNativeGuidance')
 if($RecordVideo){$helperNames+='XPTVideoHUD'}
 foreach($helperName in $helperNames){
  $live=Join-Path $root "Aircraft/X-Aviation/TorqueSim SR20/plugins/$helperName"
  if(Test-Path -LiteralPath $live){throw "Temporary helper destination exists: $live"}
  $m.helpers+=@{name=$helperName;live=$live;retired=(Join-Path $run "retired-$helperName")}
 }
 New-Item -ItemType Directory -Path $run | Out-Null
 Write-Record "$run/session.json" $m
 Copy-Item -LiteralPath $Config -Destination "$run/requested-config.json"
 # Recovery is available before the first installation mutation.
 $shell=(Get-Process -Id $PID).Path
 $watch=Start-Process -FilePath $shell -ArgumentList @('-NoProfile','-File',('"'+$PSScriptRoot+'/scripts/Watch-Owner.ps1"'),'-RunDirectory',('"'+$run+'"')) -WindowStyle Hidden -PassThru -RedirectStandardOutput "$run/watchdog.log" -RedirectStandardError "$run/watchdog-error.log"
 Write-Record "$run/watchdog-process.json" (Get-ProcessRecord $watch)
 & "$PSScriptRoot/scripts/Protect-XPlaneInstallState.ps1" -Mode Capture -XPlaneRoot $root -SnapshotPath "$run/protected-install-state.json" | Set-Content "$run/protected-capture.json"
 if($LASTEXITCODE -and $LASTEXITCODE -ne 0){throw 'Protected state preflight failed'}
 $m.global_plugin_names=@(Get-ChildItem -LiteralPath "$root/Resources/plugins" -Force|Select-Object -ExpandProperty Name|Sort-Object)
 # Explicit, temporary isolation of an external control writer. Persist the
 # recovery intent before any atomic move; the owner watchdog uses this record.
 foreach($pluginName in @($IsolateGlobalPlugin|Select-Object -Unique)){
  if($pluginName -notmatch '^[a-zA-Z0-9][a-zA-Z0-9 _.-]*$' -or $pluginName -in @('PluginAdmin','XPLM_64.dll','XPWidgets_64.dll','Commands.txt','DataRefs.txt')){throw "Invalid isolation target: $pluginName"}
  $pluginPath=Assert-InRoot (Join-Path "$root/Resources/plugins" $pluginName) "$root/Resources/plugins"
  if(-not (Test-Path -LiteralPath $pluginPath -PathType Container)){throw "Plugin directory missing: $pluginPath"}
  if((Get-Item -LiteralPath $pluginPath).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Plugin isolation target must be a regular directory'}
  $pluginFiles=Get-FileInventory $pluginPath
  $m.global_plugin_isolation+=@{name=$pluginName;live=$pluginPath;parked=(Join-Path $run "isolated-$pluginName");files=$pluginFiles}
 }
 Write-Record "$run/session.json" $m
 foreach($plugin in $m.global_plugin_isolation){Move-Exact $plugin.live $plugin.parked $root;Assert-Files $plugin.parked $plugin.files}
 foreach($relative in @('Output/preferences','Output/SR20')){
  $live=Join-Path $root $relative;$key=Split-Path $relative -Leaf
  $files=Get-FileInventory $live
  $backup=Join-Path $run "backup-$key"
  Copy-Item -LiteralPath $live -Destination $backup -Recurse
  Assert-Files $backup $files
  $m.states+=@{live=$live;backup=$backup;after=(Join-Path $run "after-$key");files=$files}
 }
 $m.backups_ready=$true
 $m.scenery_inventory=Get-TreeInventory (Join-Path $root 'Custom Scenery')
 $sourceFiles=@(Get-Item -LiteralPath "$root/Aircraft/X-Aviation/TorqueSim SR20/SR20.acf")+@(Get-ChildItem -LiteralPath "$root/Aircraft/X-Aviation/TorqueSim SR20/plugins" -Recurse -File|Where-Object {$_.Extension -in @('.xpl','.dll')})
 $m.source_hashes=@($sourceFiles|ForEach-Object {@{path=$_.FullName;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}})
 Write-Record "$run/session.json" $m
 Move-Exact (Join-Path $root 'Custom Scenery') (Join-Path $run 'original-scenery') $root
 New-Item -ItemType Directory -Path "$root/Custom Scenery" | Out-Null
 [IO.File]::WriteAllText("$root/Custom Scenery/xpt-marker.txt",$m.token)
 Copy-Item -LiteralPath "$PSScriptRoot/build/SR20G6TestController" -Destination $m.helpers[0].live -Recurse
 Copy-Item -LiteralPath "$PSScriptRoot/build/XPTNativeGuidance" -Destination $m.helpers[1].live -Recurse
 # Automated flight and video campaigns use the same 2D rendering profile.
 # Original preferences were snapshotted above and are verified by recovery.
 $prefs="$root/Output/preferences/X-Plane.prf"
 $text=Get-Content -LiteralPath $prefs -Raw
 $renderPrefs=@{'_eq_vr'=0;'_tex_res'=3;'renopt_tex_res_04'=2;
  'renopt_draw_3d_04'=1;'renopt_draw_distance04'=2;
  'renopt_vegetation_3d'=0;'renopt_vegetation_quality_04'=1;
  'renopt_shadow_quality_04'=1;'renopt_SSAO_04'=1}
 foreach($key in $renderPrefs.Keys){
  $pattern='(?m)^'+[regex]::Escape($key)+' .*$'
  if($text -notmatch $pattern){throw "Rendering preference key is missing: $key"}
  $text=$text -replace $pattern, ($key+' '+$renderPrefs[$key])
 }
 if($RecordVideo){
  Copy-Item -LiteralPath "$PSScriptRoot/build/XPTVideoHUD" -Destination $m.helpers[2].live -Recurse
  $text=$text -replace '(?m)^_movie_dx .*$', '_movie_dx 1920'
  $text=$text -replace '(?m)^_movie_framerate .*$', '_movie_framerate 30.000000'
 }
 [IO.File]::WriteAllText($prefs,$text,[Text.UTF8Encoding]::new($false))
 $m.rendering_profile='2d';$m.rendering_preferences=$renderPrefs;Write-Record "$run/session.json" $m
 if($InjectFailure -eq 'after_prepare'){throw 'Injected failure after preparation'}
 if($InjectFailure -eq 'owner_exit_after_prepare'){[Environment]::Exit(23)}
 $simArgs=@("--web_server_port=$ApiPort",'--no_joysticks','--no_save_prefs')
 if(-not $RecordVideo){$simArgs+='--no_sound'}
 $sim=Start-Process -FilePath "$root/X-Plane.exe" -WorkingDirectory $root -ArgumentList $simArgs -WindowStyle Hidden -PassThru
 $m.simulator=Get-ProcessRecord $sim;$m.simulator.arguments=$simArgs;Write-Record "$run/session.json" $m
 if($InjectFailure -eq 'after_launch'){throw 'Injected failure after simulator launch'}
 $worker=Start-Process -FilePath $python -WorkingDirectory $PSScriptRoot -ArgumentList @('-m','xpt.cli','worker','--run',('"'+$run+'"'),'--port',"$ApiPort") -WindowStyle Hidden -PassThru -RedirectStandardOutput "$run/worker.log" -RedirectStandardError "$run/worker-error.log"
 $m.worker=Get-ProcessRecord $worker;Write-Record "$run/session.json" $m
 Write-Output "Run directory: $run"
 Write-Output "Status: & '$python' -m xpt.cli status --run '$run'"
 while(-not $worker.HasExited){Start-Sleep -Seconds 1;$worker.Refresh()}
 $exitCode=$worker.ExitCode
 if($exitCode -ne 0){throw "Worker failed with exit code $exitCode; see worker-error.log"}
}catch{
 if(Test-Path -LiteralPath $run){Write-Record "$run/runner-error.json" @{error=$_.Exception.Message;utc=[datetime]::UtcNow.ToString('o')}}
 Write-Error $_ -ErrorAction Continue
 $exitCode=1
}finally{
 try{
  if(Test-Path -LiteralPath "$run/session.json"){
   & "$PSScriptRoot/scripts/Restore-Session.ps1" -RunDirectory $run
   Push-Location $PSScriptRoot
   try{& $python -m xpt.cli report --run $run;if($LASTEXITCODE -ne 0){$exitCode=1}}finally{Pop-Location}
  }
 }catch{Write-Error $_ -ErrorAction Continue;$exitCode=1}
 $campaignLock.Dispose()
}
exit $exitCode
