$ErrorActionPreference='Stop'
function Write-Record($Path,$Value){
 $temporary="$Path.tmp"
 [IO.File]::WriteAllText($temporary,($Value|ConvertTo-Json -Depth 40),[Text.UTF8Encoding]::new($false))
 Move-Item -LiteralPath $temporary -Destination $Path -Force
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
 # Start-Process can return an object whose cached Path is still null.
 # Query a fresh object and retain the original start time as the identity gate.
 $recordId=$Process.Id;$recordStart=$Process.StartTime
 for($attempt=0;$attempt -lt 50;$attempt++){
  $fresh=Get-Process -Id $recordId -ErrorAction Stop
  if([Math]::Abs(($fresh.StartTime-$recordStart).TotalSeconds) -ge .01){throw 'Process identity changed during launch capture'}
  $executable=$fresh.Path
  if(-not [string]::IsNullOrWhiteSpace($executable)){
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
function Stop-TrackedProcess($Record){
 if(-not $Record){return}
 $process=Get-Process -Id $Record.pid -ErrorAction SilentlyContinue
 if(-not $process){return}
 if(-not (Test-ProcessIdentity $Record)){throw 'Tracked PID identity mismatch; refusing to stop it'}
 $null=$process.CloseMainWindow()
 if(-not $process.WaitForExit(10000)){
  if(-not (Test-ProcessIdentity $Record)){throw 'Tracked PID identity changed before forced stop'}
  Stop-Process -Id $Record.pid
  $null=$process.WaitForExit(10000)
 }
}
