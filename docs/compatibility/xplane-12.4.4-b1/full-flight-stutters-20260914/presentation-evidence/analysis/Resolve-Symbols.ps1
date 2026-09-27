$ErrorActionPreference='Stop'
$p='V:/src/xplane/build/stutter-etl-analysis/traceevent.3.2.6/lib/netstandard2.0'
Add-Type -Path "$p/Microsoft.Diagnostics.FastSerialization.dll"
Add-Type -Path "$p/Microsoft.Diagnostics.Tracing.TraceEvent.dll"
Add-Type -Path "$p/Dia2Lib.dll"
$log=[IO.StreamWriter]::new((Join-Path $PSScriptRoot 'symbols.log'))
$reader=[Microsoft.Diagnostics.Symbols.SymbolReader]::new($log,"srv*V:\src\xplane\build\stutter-etl-analysis\symbols*https://msdl.microsoft.com/download/symbols",$null)
$reader.ServerTimeout=[timespan]::FromSeconds(30)
$queries=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'symbol-queries.json') -Raw | ConvertFrom-Json -AsHashtable
$result=@{}
try {
 foreach($file in $queries.Keys){
  $pdb=$reader.FindSymbolFilePathForModule($file,$false)
  if(-not $pdb){$result[$file]=@{error='No matching PDB'};continue}
  $module=$reader.OpenSymbolFile($pdb)
  $names=@{}
  foreach($rva in $queries[$file]){$names[[string]$rva]=$module.FindNameForRva([uint32]$rva)}
  $result[$file]=@{binary_sha256=(Get-FileHash -LiteralPath $file).Hash;pdb=$pdb;pdb_sha256=(Get-FileHash -LiteralPath $pdb).Hash;names=$names}
  $module.Dispose()
 }
 [IO.File]::WriteAllText((Join-Path $PSScriptRoot 'resolved-symbols.json'),($result|ConvertTo-Json -Depth 7),[Text.UTF8Encoding]::new($false))
}finally{$reader.Dispose();$log.Dispose()}
Write-Output 'Resolved matching Microsoft symbols.'
