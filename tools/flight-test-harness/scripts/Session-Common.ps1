$ErrorActionPreference='Stop'
function Write-Record($Path,$Value){
 $temporary="$Path.$([guid]::NewGuid().ToString('N')).tmp"
 try{
  [IO.File]::WriteAllText($temporary,($Value|ConvertTo-Json -Depth 40),[Text.UTF8Encoding]::new($false))
  # Readers can briefly deny delete sharing on Windows. Preserve the old
  # complete record and retry atomic replacement instead of Move-Item -Force.
  for($attempt=0;;$attempt++){
   try{
    if([IO.File]::Exists($Path)){[IO.File]::Replace($temporary,$Path,[NullString]::Value)}
    else{[IO.File]::Move($temporary,$Path)}
    break
   }catch [IO.IOException]{
    if($attempt -ge 99){throw}
    Start-Sleep -Milliseconds 25
   }
  }
 }finally{
  if([IO.File]::Exists($temporary)){[IO.File]::Delete($temporary)}
 }
}
function Assert-InRoot($Path,$Root){
 $full=[IO.Path]::GetFullPath($Path);$prefix=[IO.Path]::GetFullPath($Root).TrimEnd('\','/')+'\'
 if(-not $full.StartsWith($prefix,[StringComparison]::OrdinalIgnoreCase)){throw "Path outside intended root: $full"}
 return $full
}
function Move-Exact($Source,$Destination,$Root){
 $sourcePath=Assert-InRoot $Source $Root;$destinationPath=Assert-InRoot $Destination $Root
 if(Test-Path -LiteralPath $destinationPath){throw "Move destination collision: $destinationPath"}
 [IO.Directory]::Move($sourcePath,$destinationPath)
}
function Get-TreeInventory($Directory){
 $queue=[Collections.Generic.Queue[string]]::new();$queue.Enqueue($Directory);$links=@()
 while($queue.Count){
  foreach($entry in Get-ChildItem -LiteralPath $queue.Dequeue() -Force){
   if($entry.Attributes -band [IO.FileAttributes]::ReparsePoint){$links+=@{relative=$entry.FullName.Substring($Directory.Length);link_type=$entry.LinkType;target=@($entry.Target)}}
   elseif($entry.PSIsContainer){$queue.Enqueue($entry.FullName)}
  }
 }
 return @{names=@(Get-ChildItem -LiteralPath $Directory -Force|Select-Object -ExpandProperty Name|Sort-Object);links=$links}
}
function Assert-Inventory($Expected,$Actual){
 if((@($Expected.names)-join "`n") -ne (@($Actual.names)-join "`n")){throw 'Inventory name set differs'}
 $a=@($Expected.links|ForEach-Object {"$($_.relative)|$($_.link_type)|$($_.target -join '|')"}|Sort-Object)
 $b=@($Actual.links|ForEach-Object {"$($_.relative)|$($_.link_type)|$($_.target -join '|')"}|Sort-Object)
 if(($a -join "`n") -ne ($b -join "`n")){throw 'Inventory link topology differs'}
}
function Get-FileInventory($Directory){
 $inventory=Get-TreeInventory $Directory
 if($inventory.links.Count){throw "State directory contains links: $Directory"}
 return @(Get-ChildItem -LiteralPath $Directory -Recurse -File|ForEach-Object {@{relative=$_.FullName.Substring($Directory.Length);sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}}|Sort-Object relative)
}
function Assert-Files($Directory,$Expected){
 $actual=Get-FileInventory $Directory
 $a=@($Expected|ForEach-Object {"$($_.relative)|$($_.sha256)"}|Sort-Object)
 $b=@($actual|ForEach-Object {"$($_.relative)|$($_.sha256)"}|Sort-Object)
 if(($a -join "`n") -ne ($b -join "`n")){throw "State hash/name mismatch: $Directory"}
}
function Get-ProcessRecord($Process){
 # Process.Path can transiently name the loader DLL during startup. Require
 # agreement with the OS executable image, retaining the original start time.
 $recordId=$Process.Id;$recordStart=$Process.StartTime
 for($attempt=0;$attempt -lt 50;$attempt++){
  $fresh=Get-Process -Id $recordId -ErrorAction Stop
  if([Math]::Abs(($fresh.StartTime-$recordStart).TotalSeconds) -ge .01){throw 'Process identity changed during launch capture'}
  $image=Get-CimInstance Win32_Process -Filter "ProcessId=$recordId" -ErrorAction Stop
  if(-not $image -or [Math]::Abs(($image.CreationDate-$recordStart).TotalSeconds) -ge .01){throw 'Process identity changed during image capture'}
  $executable=$fresh.Path
  if(-not [string]::IsNullOrWhiteSpace($executable) -and $executable -eq $image.ExecutablePath){
   return @{pid=$recordId;executable=$executable;start_time=$recordStart.ToString('o')}
  }
  Start-Sleep -Milliseconds 100
 }
 throw 'Process executable path unavailable after bounded launch capture'
}
function Test-ProcessIdentity($Record){
 if(-not $Record){return $false}
 $process=Get-Process -Id $Record.pid -ErrorAction SilentlyContinue
 if(-not $process){return $false}
 return $process.Path -eq $Record.executable -and [Math]::Abs(($process.StartTime-[datetime]$Record.start_time).TotalSeconds) -lt .01
}
function Assert-TrackedProcessExited($Record){
 # An unverified live PID is never authority to remove its runtime resources.
 if($Record -and (Get-Process -Id $Record.pid -ErrorAction SilentlyContinue)){
  throw 'Recorded process still exists; retaining its resources until exit is verified'
 }
}
function Stop-TrackedProcess($Record){
 if(-not $Record){return}
 $process=Get-Process -Id $Record.pid -ErrorAction SilentlyContinue
 if(-not $process){return}
 if(-not (Test-ProcessIdentity $Record)){throw 'Tracked PID identity mismatch; refusing to stop it'}
 $null=$process.CloseMainWindow()
 if(-not $process.WaitForExit(10000)){
  # A normal exit between WaitForExit and identity lookup is success. Keep the
  # original process handle: never turn a PID reuse into authority to kill it.
  $process.Refresh()
  if($process.HasExited){return}
  if(-not (Test-ProcessIdentity $Record)){
   $process.Refresh()
   if($process.HasExited){return}
   throw 'Tracked PID identity changed before forced stop'
  }
  try{$process.Kill()}catch{
   $process.Refresh()
   if(-not $process.HasExited){throw}
  }
  if(-not $process.WaitForExit(10000)){throw 'Tracked process did not exit after forced stop'}
 }
}

# ---- Unattended-session preflight, clean quit and repeat-failure limits ----
# These helpers observe and report. None of them dismisses a simulator prompt,
# stops a process it did not start, or edits preferences.

function Get-XPlaneVersion($XPlaneRoot){
 $exe=Join-Path $XPlaneRoot 'X-Plane.exe'
 if(-not (Test-Path -LiteralPath $exe)){return $null}
 $info=(Get-Item -LiteralPath $exe).VersionInfo
 if(-not [string]::IsNullOrWhiteSpace($info.ProductVersion)){return $info.ProductVersion.Trim()}
 return $null
}
function Get-PreferenceValue($Path,$Key){
 if(-not (Test-Path -LiteralPath $Path)){return $null}
 foreach($line in [IO.File]::ReadAllLines($Path)){
  $parts=$line.Trim() -split '\s+',2
  if($parts.Count -eq 2 -and $parts[0] -eq $Key){return $parts[1]}
 }
 return $null
}
function Test-SimReady{
 # Returns @{ready; problems; checks}. Each problem says what to do; nothing is fixed here.
 param(
  [Parameter(Mandatory)][string]$XPlaneRoot,
  [Parameter(Mandatory)][int]$ApiPort,
  [string[]]$Arguments=@(),
  [string]$BaselineVersion,
  [switch]$AllowVersionChange,
  [long]$MinimumFreeBytes=20GB,
  [string[]]$ExtraPaths=@()
 )
 $problems=[Collections.Generic.List[string]]::new();$checks=[ordered]@{}
 $version=Get-XPlaneVersion $XPlaneRoot
 $checks.version=@{current=$version;baseline=$BaselineVersion}
 if(-not $version){$problems.Add("X-Plane.exe version unreadable under $XPlaneRoot")}
 elseif($BaselineVersion -and $version -ne $BaselineVersion -and -not $AllowVersionChange){
  $problems.Add("X-Plane changed from $BaselineVersion to $version since the last baseline. A new version can raise an update or release-notes modal that blocks a hidden launch, and results no longer compare with the baseline. Start X-Plane once yourself, deal with any prompt, then rerun with -AllowVersionChange.")
 }
 $simulators=@(Get-Process -Name 'X-Plane' -ErrorAction SilentlyContinue)
 $checks.running_simulators=@($simulators|ForEach-Object {@{pid=$_.Id;path=$_.Path}})
 if($simulators.Count){$problems.Add("X-Plane is already running (PID $(@($simulators.Id) -join ', ')). Quit it first; the suite needs its own session.")}
 $roar=@(Get-Process -Name 'TDSROARService' -ErrorAction SilentlyContinue)
 $checks.tds_roar=@($roar|ForEach-Object {@{pid=$_.Id;path=$_.Path;start_time=$_.StartTime.ToString('o')}})
 if($roar.Count -and -not $simulators.Count){
  $problems.Add("Orphaned TDSROARService (PID $(@($roar.Id) -join ', ')) is left from a stopped simulator; the next GTN will not boot. Stop that exact process yourself, then rerun.")
 }
 $listeners=@([Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners()|Where-Object {$_.Port -eq $ApiPort})
 $checks.api_port=@{port=$ApiPort;listeners=@($listeners|ForEach-Object {$_.ToString()})}
 if($listeners.Count){$problems.Add("Web API port $ApiPort is already in use ($(@($listeners|ForEach-Object {$_.ToString()}) -join ', ')). Choose another -ApiPort or stop the owner.")}
 $checks.disk=@()
 foreach($path in @($XPlaneRoot)+$ExtraPaths){
  $root=[IO.Path]::GetPathRoot([IO.Path]::GetFullPath($path))
  if(@($checks.disk|Where-Object {$_.drive -eq $root}).Count){continue}
  $free=[IO.DriveInfo]::new($root).AvailableFreeSpace
  $checks.disk+=@{drive=$root;free_bytes=$free;minimum_bytes=$MinimumFreeBytes}
  if($free -lt $MinimumFreeBytes){$problems.Add(('Drive {0} has {1:N1} GB free; at least {2:N1} GB is required. Free space first.' -f $root,($free/1GB),($MinimumFreeBytes/1GB)))}
 }
 $script=@($Arguments|Where-Object {$_ -like '--script=*'})
 $checks.arguments=@{script=($script.Count -gt 0);no_prefs=($Arguments -contains '--no_prefs')}
 if(-not $script.Count){$problems.Add('Launch arguments lack --script=<file>. Scripted test mode is what keeps an update prompt from blocking a hidden simulator.')}
 if($Arguments -contains '--no_prefs'){$problems.Add('Launch arguments include --no_prefs. It resets "Confirm before quitting", so sim/operation/quit opens a popup in the hidden window and the quit times out. Use --no_save_prefs instead.')}
 $confirm=Get-PreferenceValue (Join-Path $XPlaneRoot 'Output/preferences/Miscellaneous.prf') 'confirm_quit'
 $checks.confirm_quit=$confirm
 if($confirm -ne '0'){$problems.Add('X-Plane "Confirm before quitting" is on (confirm_quit is not 0 in Output/preferences/Miscellaneous.prf), so an unattended quit would wait on a hidden popup. Turn it off in X-Plane settings yourself, then rerun.')}
 return @{ready=($problems.Count -eq 0);problems=@($problems);checks=$checks}
}
function Assert-SimReady{
 param([Parameter(Mandatory)][string]$XPlaneRoot,[Parameter(Mandatory)][int]$ApiPort,[string[]]$Arguments=@(),[string]$BaselineVersion,[switch]$AllowVersionChange,[long]$MinimumFreeBytes=20GB,[string[]]$ExtraPaths=@())
 $result=Test-SimReady @PSBoundParameters
 if(-not $result.ready){throw ("Simulator preflight failed; nothing was changed:`n - "+($result.problems -join "`n - "))}
 return $result
}
function Read-SharedText($Path){
 # X-Plane keeps Log.txt open for writing; read it without taking a lock.
 if(-not (Test-Path -LiteralPath $Path)){return $null}
 $stream=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete)
 try{$reader=[IO.StreamReader]::new($stream,[Text.UTF8Encoding]::new($false));return $reader.ReadToEnd()}finally{$stream.Dispose()}
}
function Get-LogShutdownStage([string]$Text){
 # The furthest shutdown marker X-Plane wrote. Its own lines are buffered, so a
 # forced stop can lose the last few; plugin lines are usually flushed.
 if(-not $Text){return 'no-log'}
 foreach($stage in @(@('shut-down','----- X-Plane has shut down -----'),@('threads-exited','Clean exit from threads.'),@('plugins-unloaded','Plugin unload complete.'),@('plugins-unloading','Unloading plugin '),@('exit-requested','Exiting the sim'))){
  if($Text.Contains($stage[1])){return $stage[0]}
 }
 return 'running'
}
function Invoke-SimApi{
 param([Parameter(Mandatory)][int]$ApiPort,[Parameter(Mandatory)][string]$Path,[string]$Method='GET',$Body)
 $uri="http://127.0.0.1:$ApiPort/api/v3$Path"
 if($null -eq $Body){return Invoke-RestMethod -Uri $uri -Method $Method -TimeoutSec 5}
 return Invoke-RestMethod -Uri $uri -Method $Method -ContentType 'application/json' -Body ($Body|ConvertTo-Json -Compress -Depth 5) -TimeoutSec 5
}
function Assert-SimName([string]$Name){
 if($Name -notmatch '^[A-Za-z0-9_./\[\]-]+$'){throw "Unsupported simulator name: $Name"}
 return $Name
}
function Get-SimDatarefId([int]$ApiPort,[string]$Name){
 $found=@((Invoke-SimApi -ApiPort $ApiPort -Path ('/datarefs?filter%5Bname%5D='+(Assert-SimName $Name))).data)
 if(-not $found.Count){throw "Dataref not found: $Name"}
 return $found[0].id
}
function Invoke-SimCommand([int]$ApiPort,[string]$Name){
 $found=@((Invoke-SimApi -ApiPort $ApiPort -Path ('/commands?filter%5Bname%5D='+(Assert-SimName $Name))).data)
 if(-not $found.Count){throw "Command not found: $Name"}
 Invoke-SimApi -ApiPort $ApiPort -Path "/command/$($found[0].id)/activate" -Method POST -Body @{duration=0.2}|Out-Null
}
function Stop-XPlaneCleanly{
 # Unpause, release only the caller's named overrides, request sim/operation/quit,
 # wait, and copy Log.txt before any forced stop. Returns the quit record.
 param(
  [Parameter(Mandatory)]$Record,
  [Parameter(Mandatory)][int]$ApiPort,
  [Parameter(Mandatory)][string]$XPlaneRoot,
  [Parameter(Mandatory)][string]$EvidenceDirectory,
  [string[]]$ReleaseOverrides=@(),
  [int]$TimeoutSeconds=60
 )
 $log=Join-Path $XPlaneRoot 'Log.txt'
 $q=[ordered]@{requested_utc=[datetime]::UtcNow.ToString('o');already_exited=$false;identity_mismatch=$false;unpaused=$false;overrides=[ordered]@{};quit_sent=$false;api_errors=@();exited=$false;wait_seconds=$null;forced_stop=$false;stage_at_timeout=$null;log_before_stop=$null;stage=$null;log=$null;clean=$false}
 $process=if(Test-ProcessIdentity $Record){Get-Process -Id $Record.pid -ErrorAction SilentlyContinue}
 if(-not $process){
  $q.identity_mismatch=[bool](Get-Process -Id $Record.pid -ErrorAction SilentlyContinue)
  $q.already_exited=-not $q.identity_mismatch
 }else{
  try{
   if((Invoke-SimApi -ApiPort $ApiPort -Path "/datarefs/$(Get-SimDatarefId $ApiPort 'sim/time/paused')/value").data -ne 0){
    Invoke-SimCommand $ApiPort 'sim/operation/pause_off';$q.unpaused=$true
   }
  }catch{$q.api_errors+="unpause: $($_.Exception.Message)"}
  foreach($name in $ReleaseOverrides){
   try{
    $id=Get-SimDatarefId $ApiPort $name
    $value=(Invoke-SimApi -ApiPort $ApiPort -Path "/datarefs/$id/value").data
    if($value -ne 0){Invoke-SimApi -ApiPort $ApiPort -Path "/datarefs/$id/value" -Method PATCH -Body @{data=0}|Out-Null;$q.overrides[$name]="released from $value"}
    else{$q.overrides[$name]='not held'}
   }catch{$q.api_errors+="override ${name}: $($_.Exception.Message)"}
  }
  try{Invoke-SimCommand $ApiPort 'sim/operation/quit';$q.quit_sent=$true}catch{$q.api_errors+="quit: $($_.Exception.Message)"}
  $watch=[Diagnostics.Stopwatch]::StartNew()
  $q.exited=$process.WaitForExit($TimeoutSeconds*1000)
  $q.wait_seconds=[Math]::Round($watch.Elapsed.TotalSeconds,1)
  if(-not $q.exited){
   $text=Read-SharedText $log
   $q.stage_at_timeout=Get-LogShutdownStage $text
   if($null -ne $text){
    $q.log_before_stop=Join-Path $EvidenceDirectory 'Log-before-forced-stop.txt'
    [IO.File]::WriteAllText($q.log_before_stop,$text,[Text.UTF8Encoding]::new($false))
   }
   $q.forced_stop=$true
   Stop-TrackedProcess $Record
  }
 }
 $text=Read-SharedText $log
 $q.stage=Get-LogShutdownStage $text
 if($null -ne $text){
  $q.log=Join-Path $EvidenceDirectory 'Log.txt'
  [IO.File]::WriteAllText($q.log,$text,[Text.UTF8Encoding]::new($false))
 }
 $q.clean=(-not $q.already_exited) -and $q.exited -and -not $q.forced_stop -and $q.stage -eq 'shut-down'
 return $q
}
function Get-StrikeLedger($Path){
 if(-not (Test-Path -LiteralPath $Path)){return @{schema=1;cases=@{}}}
 $ledger=Get-Content -LiteralPath $Path -Raw|ConvertFrom-Json -AsHashtable
 if(-not $ledger.cases){$ledger.cases=@{}}
 return $ledger
}
function Update-StrikeLedger{
 # A fail or error adds a strike to <case>@<build>; a pass or warn clears it.
 param([Parameter(Mandatory)][string]$Path,[Parameter(Mandatory)][string]$Key,[Parameter(Mandatory)][ValidateSet('pass','warn','fail','error')][string]$Verdict,[string]$Evidence)
 $ledger=Get-StrikeLedger $Path
 if($Verdict -in @('pass','warn')){$ledger.cases.Remove($Key)}
 else{
  $entry=$ledger.cases[$Key]
  if(-not $entry){$entry=@{count=0;attempts=@()}}
  $entry.count=[int]$entry.count+1
  $entry.attempts=@($entry.attempts)+@(@{utc=[datetime]::UtcNow.ToString('o');verdict=$Verdict;evidence=$Evidence})
  $ledger.cases[$Key]=$entry
 }
 Write-Record $Path $ledger
 if($ledger.cases.ContainsKey($Key)){return [int]$ledger.cases[$Key].count}
 return 0
}
function Get-BlockedStrikes{
 # Keys that reached the limit: stop and report instead of trying again.
 param([Parameter(Mandatory)][string]$Path,[string[]]$Keys=@(),[int]$Limit=3)
 $ledger=Get-StrikeLedger $Path
 return @($Keys|Where-Object {$ledger.cases.ContainsKey($_) -and [int]$ledger.cases[$_].count -ge $Limit}|ForEach-Object {@{key=$_;count=[int]$ledger.cases[$_].count;attempts=$ledger.cases[$_].attempts}})
}
function Clear-StrikeLedger{
 param([Parameter(Mandatory)][string]$Path,[string[]]$Keys=@())
 $ledger=Get-StrikeLedger $Path
 foreach($key in $Keys){$ledger.cases.Remove($key)}
 Write-Record $Path $ledger
}
