param([string]$XPlaneRoot='D:\X-Plane 12',[string]$Cargo='cargo')
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
$workspace=[IO.Path]::GetFullPath((Join-Path $project '../..'))
if(-not (Get-Command $Cargo -ErrorAction SilentlyContinue)){$Cargo=Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe'}
$originalPath=$env:PATH
$env:PATH=(Join-Path $XPlaneRoot 'Resources/plugins')+';'+$env:PATH
Push-Location $project
try{
 & "$project/.venv/Scripts/python.exe" -m unittest discover -s tests -v
 if($LASTEXITCODE -ne 0){throw 'Python tests failed'}
 & $Cargo test --manifest-path "$workspace/Cargo.toml" --workspace --locked
 if($LASTEXITCODE -ne 0){throw 'Rust workspace tests failed'}
 & "$PSScriptRoot/Build-Native.ps1" -XPlaneRoot $XPlaneRoot -Cargo $Cargo
 $errors=@()
 foreach($file in @(Get-ChildItem "$project/scripts" -Filter '*.ps1')+@(Get-Item "$project/Run-XPlaneTest.ps1")){
  $tokens=$null;$parseErrors=$null
  $null=[Management.Automation.Language.Parser]::ParseFile($file.FullName,[ref]$tokens,[ref]$parseErrors)
  $errors+=@($parseErrors)
 }
 if($errors.Count){$errors|Format-List|Out-String|Write-Output;throw 'PowerShell parse checks failed'}
 . "$PSScriptRoot/Session-Common.ps1"
 $self=Get-Process -Id $PID
 $stale=[pscustomobject]@{Id=$self.Id;StartTime=$self.StartTime;Path=$null}
 $record=Get-ProcessRecord $stale
 if([string]::IsNullOrWhiteSpace($record.executable) -or -not (Test-ProcessIdentity $record)){throw 'Fresh process-identity capture failed'}
 $record.executable='Z:\not-the-running-process.exe'
 if(Test-ProcessIdentity $record){throw 'Mismatched executable accepted'}
 $wrongStart=[pscustomobject]@{Id=$self.Id;StartTime=$self.StartTime.AddSeconds(-1);Path=$null}
 $rejected=$false
 try{$null=Get-ProcessRecord $wrongStart}catch{$rejected=$true}
 if(-not $rejected){throw 'Changed launch identity accepted'}
 Write-Output 'Fresh process identity and mismatched-identity rejection passed'
 $recordTest=Join-Path $project ('build/record-test-'+[guid]::NewGuid().ToString('N')+'.json')
 try{
  Write-Record $recordTest @{revision=1}
  $reader=[IO.File]::Open($recordTest,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
  try{
   $rejected=$false
   try{Write-Record $recordTest @{revision=2}}catch [IO.IOException]{$rejected=$true}
   if(-not $rejected){throw 'Record replacement ignored a reader denying delete sharing'}
   if((Get-Content -LiteralPath $recordTest -Raw|ConvertFrom-Json).revision -ne 1){throw 'Failed replacement damaged original record'}
  }finally{$reader.Dispose()}
  Write-Record $recordTest @{revision=3}
  if((Get-Content -LiteralPath $recordTest -Raw|ConvertFrom-Json).revision -ne 3){throw 'Record replacement did not recover after reader closed'}
  if(@(Get-ChildItem -LiteralPath (Split-Path $recordTest) -Filter ((Split-Path $recordTest -Leaf)+'.*.tmp')).Count){throw 'Record writer left temporary files'}
  Write-Output 'Atomic record replacement preserves old data on sharing conflicts and recovers after reader closes'
 }finally{if(Test-Path -LiteralPath $recordTest){Remove-Item -LiteralPath $recordTest}}
}finally{Pop-Location;$env:PATH=$originalPath}
