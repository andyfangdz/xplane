param([ValidateSet('Coverage','Hitch')][string]$Probe='Coverage',[Parameter(Mandatory=$true)][string]$Etl,[Parameter(Mandatory=$true)][string]$Output,[int]$TargetPid,[long]$Begin,[long]$End)
$ErrorActionPreference='Stop'
$libs=Join-Path $PSScriptRoot 'traceevent.3.2.6/lib/netstandard2.0'
Add-Type -Path "$libs/Microsoft.Diagnostics.FastSerialization.dll"
Add-Type -Path "$libs/Microsoft.Diagnostics.Tracing.TraceEvent.dll"
$refs=@(Get-ChildItem "$PSHOME/ref/*.dll" | ForEach-Object FullName)+@("$libs/Microsoft.Diagnostics.Tracing.TraceEvent.dll","$libs/Microsoft.Diagnostics.FastSerialization.dll")
$name="Etl${Probe}Probe"
Add-Type -TypeDefinition (Get-Content -LiteralPath (Join-Path $PSScriptRoot "$name.cs") -Raw) -ReferencedAssemblies $refs
if($Probe -eq 'Coverage'){$result=[EtlCoverageProbe]::Run($Etl)}
else{$result=[EtlHitchProbe]::Run($Etl,$TargetPid,$Begin,$End)}
[IO.File]::WriteAllText($Output,$result,[Text.UTF8Encoding]::new($false))
Write-Output "Saved $Probe analysis to $Output"
