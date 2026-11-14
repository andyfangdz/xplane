# Exact-process identity checks. No simulator, process termination or API calls.
$ErrorActionPreference='Stop'
. "$PSScriptRoot/Session-Common.ps1"
function Expect($Condition,$Message){if(-not $Condition){throw $Message}}
$self=Get-Process -Id $PID
$launch=[pscustomobject]@{Id=$self.Id;StartTime=$self.StartTime;Path=$null}
$record=Get-ProcessRecord $launch
Expect (Test-ProcessIdentity $record) 'Fresh executable identity was rejected'
$wrong=@{pid=$record.pid;executable='Z:\not-the-running-process.exe';start_time=$record.start_time}
Expect (-not (Test-ProcessIdentity $wrong)) 'Mismatched executable was accepted'
$rejected=$false
try{Assert-TrackedProcessExited $wrong}catch{$rejected=$true}
Expect $rejected 'Live PID with a mismatched image was allowed resource cleanup'
Assert-TrackedProcessExited @{pid=[int]::MaxValue}

& {
 $script:identityReads=0
 $script:identitySleeps=0
 $script:identityStart=[datetime]'2026-01-01T12:00:00Z'
 function Get-Process {
  param($Id)
  $script:identityReads++
  [pscustomobject]@{StartTime=$script:identityStart;Path=$(if($script:identityReads -eq 1){'C:\Windows\System32\ntdll.dll'}else{'C:\fixture\sim.exe'})}
 }
 function Get-CimInstance {
  param($ClassName,$Filter)
  [pscustomobject]@{CreationDate=$script:identityStart;ExecutablePath='C:\fixture\sim.exe'}
 }
 function Start-Sleep {param($Milliseconds);$script:identitySleeps++}
 $launch=[pscustomobject]@{Id=1234;StartTime=$script:identityStart}
 $record=Get-ProcessRecord $launch
 Expect ($record.executable -eq 'C:\fixture\sim.exe' -and $script:identityReads -eq 2 -and $script:identitySleeps -eq 1) 'Transient loader DLL was recorded as the executable'
 $script:identityStart=$script:identityStart.AddSeconds(1)
 $rejected=$false
 try{$null=Get-ProcessRecord $launch}catch{$rejected=$true}
 Expect $rejected 'Changed process start time was accepted'
}
$quit=Stop-XPlaneCleanly -Record $wrong -ApiPort 1 -XPlaneRoot $PSScriptRoot -EvidenceDirectory $PSScriptRoot
Expect ($quit.identity_mismatch -and -not $quit.already_exited -and -not $quit.clean) 'Live mismatched PID was reported as exited'
Write-Output 'Exact process identity, loader transition and mismatched-quit checks passed'
