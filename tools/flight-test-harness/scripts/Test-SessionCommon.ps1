# Offline checks for the preflight, shutdown-stage and strike helpers in
# Session-Common.ps1. Needs no simulator, network listener or installation.
$ErrorActionPreference='Stop'
. "$PSScriptRoot/Session-Common.ps1"
$failures=[Collections.Generic.List[string]]::new()
function Expect($Condition,$Message){if(-not $Condition){$failures.Add($Message)}}

$stages=[ordered]@{
 'no-log'=''
 'running'="0:00:01 I/SIM: flight loaded`nC182 conversion: bracket yoke_ratio 2 -> 0"
 'exit-requested'="0:05:26.436 I/SIM: Exiting the sim without confirmation"
 'plugins-unloading'="I/SIM: Exiting the sim without confirmation`nUnloading plugin PluginAdmin (xpsdk.examples.pluginadmin)"
 'plugins-unloaded'="Exiting the sim`nUnloading plugin X`nPlugin unload complete."
 'threads-exited'="Exiting the sim`nPlugin unload complete.`nClean exit from threads."
 'shut-down'="Exiting the sim`nClean exit from threads.`n----- X-Plane has shut down -----"
}
foreach($expected in $stages.Keys){
 $actual=Get-LogShutdownStage $stages[$expected]
 Expect ($actual -eq $expected) "Shutdown stage: expected $expected, got $actual"
}

$root=Join-Path ([IO.Path]::GetTempPath()) ('xpt-selftest-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path (Join-Path $root 'Output/preferences')|Out-Null
try{
 $prefs=Join-Path $root 'Output/preferences/Miscellaneous.prf'
 [IO.File]::WriteAllText($prefs,"I`n1100 Version`nconfirm_quit 1`n")
 $good=@('--no_sound','--no_save_prefs','--script=C:/run/startup.test')
 $result=Test-SimReady -XPlaneRoot $root -ApiPort 1 -Arguments ($good+'--no_prefs') -MinimumFreeBytes 1
 $text=$result.problems -join "`n"
 Expect (-not $result.ready) 'Preflight accepted a broken fixture'
 Expect ($text -match 'version unreadable') 'Missing X-Plane.exe was not reported'
 Expect ($text -match '--no_prefs') '--no_prefs was not refused'
 Expect ($text -match 'Confirm before quitting') 'confirm_quit 1 was not refused'
 Expect ($text -notmatch 'lack --script') '--script was reported missing although present'
 $result=Test-SimReady -XPlaneRoot $root -ApiPort 1 -Arguments @('--no_sound') -MinimumFreeBytes 1
 Expect (($result.problems -join "`n") -match 'lack --script') 'Missing --script was not refused'
 [IO.File]::WriteAllText($prefs,"I`nconfirm_quit 0`n")
 $result=Test-SimReady -XPlaneRoot $root -ApiPort 1 -Arguments $good -MinimumFreeBytes ([long]::MaxValue)
 $text=$result.problems -join "`n"
 Expect ($text -notmatch 'Confirm before quitting') 'confirm_quit 0 was refused'
 Expect ($text -match 'GB free') 'Low disk space was not reported'
 Expect ($result.checks.confirm_quit -eq '0') 'confirm_quit readback missing'
 $refused=$false
 try{$null=Assert-SimReady -XPlaneRoot $root -ApiPort 1 -Arguments $good -MinimumFreeBytes ([long]::MaxValue)}catch{$refused=$_.Exception.Message -match 'nothing was changed'}
 Expect $refused 'Assert-SimReady did not stop with a report'
 Expect ((Get-Content -LiteralPath $prefs -Raw) -eq "I`nconfirm_quit 0`n") 'Preflight modified preferences'

 $ledger=Join-Path $root 'strikes.json'
 Expect ((Update-StrikeLedger -Path $ledger -Key 'smoke@abc' -Verdict fail -Evidence 'run1') -eq 1) 'First strike not counted'
 Expect ((Update-StrikeLedger -Path $ledger -Key 'smoke@abc' -Verdict error -Evidence 'run2') -eq 2) 'Error not counted as a strike'
 Expect (@(Get-BlockedStrikes -Path $ledger -Keys @('smoke@abc')).Count -eq 0) 'Blocked before three strikes'
 Expect ((Update-StrikeLedger -Path $ledger -Key 'smoke@abc' -Verdict fail -Evidence 'run3') -eq 3) 'Third strike not counted'
 $blocked=@(Get-BlockedStrikes -Path $ledger -Keys @('smoke@abc','load@abc'))
 Expect ($blocked.Count -eq 1 -and $blocked[0].key -eq 'smoke@abc' -and @($blocked[0].attempts).Count -eq 3) 'Three strikes did not block with evidence'
 Expect (@(Get-BlockedStrikes -Path $ledger -Keys @('smoke@def')).Count -eq 0) 'Another build inherited strikes'
 Expect ((Update-StrikeLedger -Path $ledger -Key 'load@abc' -Verdict fail) -eq 1) 'Second key not counted'
 Expect ((Update-StrikeLedger -Path $ledger -Key 'load@abc' -Verdict warn) -eq 0) 'Warn did not clear strikes'
 Clear-StrikeLedger -Path $ledger -Keys @('smoke@abc')
 Expect (@(Get-BlockedStrikes -Path $ledger -Keys @('smoke@abc')).Count -eq 0) 'Reset did not clear strikes'

 $dead=@{pid=[int]::MaxValue;executable='Z:\not-running.exe';start_time=[datetime]::UtcNow.ToString('o')}
 $quit=Stop-XPlaneCleanly -Record $dead -ApiPort 1 -XPlaneRoot $root -EvidenceDirectory $root
 Expect ($quit.already_exited -and -not $quit.quit_sent -and -not $quit.clean -and $quit.stage -eq 'no-log') 'Quit of an exited process was not recorded as such'
}finally{
 Remove-Item -LiteralPath $root -Recurse -Force
}
if($failures.Count){$failures|ForEach-Object {Write-Output "FAIL $_"};throw "$($failures.Count) Session-Common self-test(s) failed"}
Write-Output 'Session-Common preflight, shutdown-stage, strike and exited-quit checks passed'
