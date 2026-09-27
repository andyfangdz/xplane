param([string]$XPlaneRoot='D:\X-Plane 12')
$ErrorActionPreference='Stop'
if([IO.Path]::GetFullPath($XPlaneRoot).TrimEnd('\','/') -ne 'D:\X-Plane 12'){throw 'Installed task is scoped to D:\X-Plane 12'}
$installed='C:\Program Files\CodexXPlaneTracing'
foreach($name in @('Start-WprHost.ps1','Session-Common.ps1','XPT-Performance.wprp')){
 if((Get-FileHash -LiteralPath (Join-Path $installed $name)).Hash -ne (Get-FileHash -LiteralPath (Join-Path $PSScriptRoot $name)).Hash){throw 'Installed helper differs from reviewed workspace source; an administrator must explicitly update the installation'}
 $acl=Get-Acl -LiteralPath (Join-Path $installed $name)
 $owner=$acl.GetOwner([Security.Principal.SecurityIdentifier]).Value
 if($owner -notin @('S-1-5-18','S-1-5-32-544')){throw 'Installed script owner is not protected'}
 foreach($rule in $acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])){
  if($rule.AccessControlType -eq 'Allow' -and $rule.IdentityReference.Value -notin @('S-1-5-18','S-1-5-32-544') -and ([int]$rule.FileSystemRights -band 0xD0116)){throw 'Installed helper is writable by an ordinary account'}
 }
}
$task=Get-ScheduledTask -TaskPath '\CodexXPlaneTracing\' -TaskName XPlaneTraceHost
$expected='-NoProfile -NonInteractive -WindowStyle Hidden -File "C:\Program Files\CodexXPlaneTracing\Start-WprHost.ps1" -XPlaneRoot "D:\X-Plane 12" -LifetimeMinutes 180'
if($task.Actions.Count -ne 1 -or $task.Actions[0].Execute -ne 'C:\Program Files\PowerShell\7\pwsh.exe' -or $task.Actions[0].Arguments -ne $expected -or $task.Actions[0].WorkingDirectory -ne $installed){throw 'Unexpected installed task action'}
if($task.Triggers.Count -ne 0 -or [int]$task.Principal.RunLevel -ne 1 -or [int]$task.Principal.LogonType -ne 3){throw 'Unexpected task principal or trigger policy'}
$sid=([Security.Principal.NTAccount]::new($task.Principal.UserId)).Translate([Security.Principal.SecurityIdentifier]).Value
if($sid -ne [Security.Principal.WindowsIdentity]::GetCurrent().User.Value){throw 'Task belongs to a different signed-in user'}
Start-ScheduledTask -InputObject $task
Write-Output 'Requested the installed on-demand tracing host without elevation. Allow its bounded CPU privilege probe to finish before pinging it.'
