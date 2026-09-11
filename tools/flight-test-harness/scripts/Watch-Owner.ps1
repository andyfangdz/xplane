param([Parameter(Mandatory)][string]$RunDirectory)
. "$PSScriptRoot/Session-Common.ps1"
while($true){
 if(Test-Path -LiteralPath "$RunDirectory/restoration.json"){
  if((Get-Content "$RunDirectory/restoration.json" -Raw|ConvertFrom-Json).restored){exit 0}
 }
 $m=Get-Content "$RunDirectory/session.json" -Raw|ConvertFrom-Json
 if(-not (Test-ProcessIdentity $m.owner)){
  Write-Record "$RunDirectory/owner-loss.json" @{error='Launcher process ended before restoration';utc=[datetime]::UtcNow.ToString('o')}
  & "$PSScriptRoot/Restore-Session.ps1" -RunDirectory $RunDirectory
  Push-Location $m.project_root
  try{& (Join-Path $m.project_root '.venv/Scripts/python.exe') -m xpt.cli report --run $RunDirectory}finally{Pop-Location}
  exit
 }
 Start-Sleep -Seconds 2
}
