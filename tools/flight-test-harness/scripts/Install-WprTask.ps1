param([Parameter(Mandatory=$true)][string]$BundleManifest)
# One administrator approval installs a fixed on-demand action. Its executable
# files and task definition are not writable from the ordinary desktop token.
$ErrorActionPreference='Stop'
$identity=[Security.Principal.WindowsIdentity]::GetCurrent()
if(-not [Security.Principal.WindowsPrincipal]::new($identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Run this installer as administrator'}
$bundle=Get-Content -LiteralPath $BundleManifest -Raw | ConvertFrom-Json
if($identity.User.Value -ne $bundle.user_sid){throw 'Installer must run as the requesting signed-in user'}
if($bundle.user_sid -notmatch '^S-1-5-21-\d+-\d+-\d+-\d+$'){throw 'Unexpected user SID'}
$source=Split-Path ([IO.Path]::GetFullPath($BundleManifest)) -Parent
$destination='C:\Program Files\CodexXPlaneTracing'
$taskPath='\CodexXPlaneTracing'
$taskName='XPlaneTraceHost'
$shell='C:\Program Files\PowerShell\7\pwsh.exe'
$expectedNames=@('Start-WprHost.ps1','Session-Common.ps1','XPT-Performance.wprp')
if((@($bundle.files.name|Sort-Object)-join '|') -ne (@($expectedNames|Sort-Object)-join '|')){throw 'Unexpected bundle files'}
foreach($file in $bundle.files){
 $path=Join-Path $source $file.name
 if((Get-Item -LiteralPath $path).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Bundle contains a link'}
 if((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $file.sha256){throw 'Bundle hash mismatch'}
}
if(Test-Path -LiteralPath $destination){throw 'Installation destination exists; inspect the existing installation before updating'}
$scheduler=New-Object -ComObject 'Schedule.Service'
$scheduler.Connect()
$rootFolder=$scheduler.GetFolder('\')
$existing=$null
try{$existing=$scheduler.GetFolder($taskPath)}catch{}
if($existing){throw 'Dedicated task folder already exists; refusing to replace it'}
$sddl="O:BAG:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;FRFX;;;$($bundle.user_sid))"
$installCreated=$false
try{
 $null=New-Item -ItemType Directory -Path $destination
 $installCreated=$true
 $acl=[Security.AccessControl.DirectorySecurity]::new()
 $acl.SetAccessRuleProtection($true,$false)
 $acl.SetOwner([Security.Principal.SecurityIdentifier]::new('S-1-5-32-544'))
 $inherit=[Security.AccessControl.InheritanceFlags]'ContainerInherit, ObjectInherit'
 foreach($sid in @('S-1-5-18','S-1-5-32-544')){
  $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($sid),'FullControl',$inherit,'None','Allow'))
 }
 $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($bundle.user_sid),'ReadAndExecute',$inherit,'None','Allow'))
 Set-Acl -LiteralPath $destination -AclObject $acl
 foreach($file in $bundle.files){
  $target=Join-Path $destination $file.name
  Copy-Item -LiteralPath (Join-Path $source $file.name) -Destination $target
  $fileAcl=Get-Acl -LiteralPath $target
  $fileAcl.SetOwner([Security.Principal.SecurityIdentifier]::new('S-1-5-32-544'))
  Set-Acl -LiteralPath $target -AclObject $fileAcl
  if((Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash -ne $file.sha256){throw 'Installed hash mismatch'}
 }
 $folder=$rootFolder.CreateFolder('CodexXPlaneTracing',$sddl)
 $definition=$scheduler.NewTask(0)
 $definition.RegistrationInfo.Description='On-demand X-Plane performance trace host. Fixed protected scripts and profile; only owned X-Plane trace start, stop and status requests. No automatic trigger.'
 $definition.Principal.UserId=$bundle.user_sid
 $definition.Principal.LogonType=3 # Existing interactive token; no stored password.
 $definition.Principal.RunLevel=1 # Highest, authorized by this installer.
 $definition.Settings.Enabled=$true
 $definition.Settings.AllowDemandStart=$true
 $definition.Settings.DisallowStartIfOnBatteries=$false
 $definition.Settings.StopIfGoingOnBatteries=$false
 $definition.Settings.MultipleInstances=2 # Ignore duplicate starts.
 $definition.Settings.ExecutionTimeLimit='PT3H10M'
 $definition.Settings.Hidden=$true
 $action=$definition.Actions.Create(0)
 $action.Path=$shell
 $action.Arguments='-NoProfile -NonInteractive -WindowStyle Hidden -File "C:\Program Files\CodexXPlaneTracing\Start-WprHost.ps1" -XPlaneRoot "D:\X-Plane 12" -LifetimeMinutes 180'
 $action.WorkingDirectory=$destination
 # Do not let Task Scheduler add a broader principal ACE than read/execute.
 $task=$folder.RegisterTaskDefinition($taskName,$definition,18,$bundle.user_sid,$null,3,$sddl)
 $task.SetSecurityDescriptor($sddl,16)
 $record=@{installed=$true;utc=[datetime]::UtcNow.ToString('o');task_path=$task.Path;user_sid=$bundle.user_sid;directory=$destination;files=$bundle.files;task_sddl=$task.GetSecurityDescriptor(7);folder_sddl=$folder.GetSecurityDescriptor(7);directory_sddl=(Get-Acl -LiteralPath $destination).Sddl}
 [IO.File]::WriteAllText((Join-Path $destination 'installation.json'),($record|ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
 $record|ConvertTo-Json -Depth 8
}catch{
 if($installCreated){
  [IO.File]::WriteAllText((Join-Path $destination 'installation-error.json'),(@{error=$_.Exception.Message;utc=[datetime]::UtcNow.ToString('o')}|ConvertTo-Json),[Text.UTF8Encoding]::new($false))
 }
 throw
}
