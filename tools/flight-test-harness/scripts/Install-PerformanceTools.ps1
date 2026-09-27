param()
$ErrorActionPreference='Stop'
$target=Join-Path (Split-Path $PSScriptRoot -Parent) 'bin/PresentMon/PresentMon-2.5.1-x64.exe'
$url='https://github.com/GameTechDev/PresentMon/releases/download/v2.5.1/PresentMon-2.5.1-x64.exe'
$expected='9bec3083069f58f911e6a512f4806db51a27bd096103087bc1d05ef54c80a191'
New-Item -ItemType Directory -Force -Path (Split-Path $target) | Out-Null
if(-not (Test-Path -LiteralPath $target)){Invoke-WebRequest $url -OutFile $target}
if((Get-FileHash -LiteralPath $target).Hash.ToLowerInvariant() -ne $expected){throw 'PresentMon checksum mismatch'}
[pscustomobject]@{path=$target;version='2.5.1';sha256=$expected;source=$url} | ConvertTo-Json | Set-Content (Join-Path (Split-Path $target) 'provenance.json')
Write-Output "PresentMon verified at $target"
