param(
    [Parameter(Mandatory)][string]$Ack,
    [string]$XPlaneRoot = 'D:\X-Plane 12',
    [string]$BinaryDirectory,
    [string]$Name = ('native-panels-' + [datetime]::UtcNow.ToString('yyyyMMdd-HHmmss')),
    [int]$ApiPort = 8186,
    [ValidateSet('cockpit','fullscreen','poweroff180','position','xgs')]
    [string[]]$Cases = @('cockpit','fullscreen','poweroff180','position','xgs')
)
$ErrorActionPreference = 'Stop'
if ([string]::IsNullOrWhiteSpace($Ack) -or $Name -notmatch '^[A-Za-z0-9_-]+$') { throw 'Supply the session acknowledgment and a simple, unique run name.' }
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../../../..')).Path
$harness = Join-Path $repo 'tools/flight-test-harness'
. "$harness/scripts/Session-Common.ps1"
$root = (Resolve-Path -LiteralPath $XPlaneRoot).Path
$python = (Get-Command python -ErrorAction Stop).Source
if (-not $BinaryDirectory) { $BinaryDirectory = Join-Path $repo 'target/release' }
$binary = (Resolve-Path -LiteralPath (Join-Path $BinaryDirectory 'shuttle_hud.dll')).Path
$testRoot = Join-Path $root 'Output/performance-tests'
$out = Assert-InRoot (Join-Path $testRoot $Name) $testRoot
$relative = "Aircraft/Codex/$Name"
$aircraft = Assert-InRoot (Join-Path $root $relative) (Join-Path $root 'Aircraft/Codex')
$globals = Join-Path $root 'Resources/plugins'
$replace = [ordered]@{PositionAircraftNative='position_aircraft_native.dll'; XgsRust='xgs_rs.dll'}
$argsXP = @("--web_server_port=$ApiPort",'--no_sound','--no_joysticks','--no_save_prefs','--no_crash_reporter',
    '--safe_mode=SCN','--pref:_eq_vr=0','--pref:_show_qfl_on_start=0',('--script="'+$out+'/startup.test"'),
    ('--new_flight_json="'+$out+'/flight.json"'))
$readiness = Assert-SimReady -XPlaneRoot $root -ApiPort $ApiPort -Arguments $argsXP
foreach ($filename in @($replace.Values) + @('poweroff180_hud.dll')) {
    if (-not (Test-Path -LiteralPath (Join-Path $BinaryDirectory $filename) -PathType Leaf)) { throw "Missing candidate $filename" }
}
$null = New-Item -ItemType Directory -Force -Path $testRoot
$lock = [IO.File]::Open((Join-Path $testRoot '.xpt-session.lock'), 'OpenOrCreate', 'ReadWrite', 'None')
try {
    if ((Test-Path -LiteralPath $out) -or (Test-Path -LiteralPath $aircraft)) { throw 'The run or temporary aircraft path already exists.' }
    foreach ($nameKey in $replace.Keys) {
        if (Test-Path -LiteralPath (Join-Path $globals "$Name-$nameKey")) { throw "Temporary global name already in use: $nameKey" }
    }
    $null = New-Item -ItemType Directory -Path $out
    $manifest = [ordered]@{token=[guid]::NewGuid().ToString('N'); ack=$Ack; xplane_root=$root; project_root=$harness; mode='NativePanels';
        owner=(Get-ProcessRecord (Get-Process -Id $PID)); worker=$null; simulator=$null; backups_ready=$false; states=@();
        helpers=@(); source_hashes=@(); global_plugin_isolation=@(); global_plugin_names=@(Get-ChildItem -LiteralPath $globals -Force | Select-Object -ExpandProperty Name | Sort-Object); scenery_inventory=$null}
    Write-Record "$out/session.json" $manifest
    Write-Record "$out/preflight.json" $readiness
    $watch = Start-Process -FilePath (Get-Process -Id $PID).Path -ArgumentList @('-NoProfile','-File',('"' + $harness + '/scripts/Watch-Owner.ps1"'),'-RunDirectory',('"' + $out + '"')) -WindowStyle Hidden -PassThru
    Write-Record "$out/watchdog.json" (Get-ProcessRecord $watch)
    try {
        & "$harness/scripts/Protect-XPlaneInstallState.ps1" -Mode Capture -XPlaneRoot $root -SnapshotPath "$out/protected-install-state.json" | Out-File "$out/protected-capture.log"
        $prefs = Join-Path $root 'Output/preferences'
        $files = Get-FileInventory $prefs
        Copy-Item -LiteralPath $prefs -Destination "$out/backup-preferences" -Recurse
        Assert-Files "$out/backup-preferences" $files
        $manifest.states += @{live=$prefs; backup="$out/backup-preferences"; after="$out/after-preferences"; files=$files}
        $manifest.backups_ready = $true
        Write-Record "$out/session.json" $manifest

        # The aircraft generator requires an exact source revision and writes a new directory under Output.
        $stageResult = & $python "$repo/plugins/shuttle-hud/install_native.py" --xplane $root --binary $binary --stage-only
        if ($LASTEXITCODE) { throw 'Shuttle generation failed' }
        $stage = Assert-InRoot (($stageResult -join [Environment]::NewLine | ConvertFrom-Json).created) (Join-Path $root 'Output/ShuttleHUD-install')
        if ((Get-TreeInventory $stage).links.Count) { throw 'Staged aircraft unexpectedly contains links' }
        $manifest.helpers += @{name=$Name;live=$aircraft;retired="$out/retired-aircraft"}
        $stagedHud = Join-Path $stage 'plugins/XPTVideoHUD/64'
        $null = New-Item -ItemType Directory -Path $stagedHud
        Copy-Item -LiteralPath (Join-Path $BinaryDirectory 'poweroff180_hud.dll') -Destination "$stagedHud/win.xpl"
        & $python "$PSScriptRoot/native_panel.py" --prepare --root $root --aircraft $stage --binary $binary --relative-acf "$relative/Orbiter_Glider.acf" --out $out --port $ApiPort
        if ($LASTEXITCODE) { throw 'Visual fixture preparation failed' }
        $hashes = [ordered]@{ShuttleHUD=(Get-FileHash -LiteralPath "$stage/plugins/ShuttleHUD/64/win.xpl").Hash;
            XPTVideoHUD=(Get-FileHash -LiteralPath "$stagedHud/win.xpl").Hash}
        foreach ($nameKey in $replace.Keys) {
            $live = Assert-InRoot (Join-Path $globals $nameKey) $globals
            $parked = Join-Path $out "original-$nameKey"
            $temporary = Assert-InRoot (Join-Path $globals "$Name-$nameKey") $globals
            $inventory = Get-FileInventory $live
            $manifest.global_plugin_isolation += @{name=$nameKey; live=$live; parked=$parked; files=$inventory}
            $manifest.helpers += @{name="$Name-$nameKey";live=$temporary;retired="$out/retired-$nameKey"}
            $prepared = Join-Path $out "staged-$nameKey"
            Copy-Item -LiteralPath $live -Destination $prepared -Recurse
            Copy-Item -LiteralPath (Join-Path $BinaryDirectory $replace[$nameKey]) -Destination "$prepared/64/win.xpl" -Force
            $hashes[$nameKey] = (Get-FileHash -LiteralPath "$prepared/64/win.xpl").Hash
            Write-Record "$out/session.json" $manifest
            Move-Exact $live $parked $root
            Move-Exact $prepared $temporary $root
        }
        Write-Record "$out/candidate-hashes.json" $hashes
        Write-Record "$out/session.json" $manifest
        Move-Exact $stage $aircraft $root
        [IO.File]::WriteAllText("$out/startup.test","A`n1000`nCLI_SCRIPT`n`nwait 1`n")
        $sim = Start-Process -FilePath "$root/X-Plane.exe" -WorkingDirectory $root -ArgumentList $argsXP -PassThru
        $manifest.simulator = Get-ProcessRecord $sim
        $manifest.simulator.arguments = $argsXP
        Write-Record "$out/session.json" $manifest
        Write-Output "Capturing $($Cases -join ', ') at $out."
        if ($Cases -contains 'xgs') { Write-Output "When xgs-ready appears, select Plugins > Landing Speed Rust > Preview Overlay and create $out/xgs-selected." }
        & $python "$PSScriptRoot/native_panel.py" --root $root --aircraft $aircraft --binary $binary --out $out --port $ApiPort --cases @Cases
        if ($LASTEXITCODE) { throw 'Visual capture failed; see the run evidence.' }
    } finally {
        if ($manifest.simulator) {
            $quit = Stop-XPlaneCleanly -Record $manifest.simulator -ApiPort $ApiPort -XPlaneRoot $root -EvidenceDirectory $out
            Write-Record "$out/quit.json" $quit
        }
        & "$harness/scripts/Restore-Session.ps1" -RunDirectory $out
    }
    if (-not $quit.clean) { throw 'X-Plane did not shut down cleanly. See quit.json.' }
} finally {
    $lock.Dispose()
}
