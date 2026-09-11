param([string]$PythonLauncher='py')
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
if(-not (Test-Path -LiteralPath "$project/.venv/Scripts/python.exe")){
 & $PythonLauncher -3.13 -m venv "$project/.venv"
 if($LASTEXITCODE -ne 0){throw 'Python 3.13 environment creation failed'}
}
& "$project/.venv/Scripts/python.exe" -m pip install -r "$project/requirements.lock"
if($LASTEXITCODE -ne 0){throw 'Dependency installation failed'}
