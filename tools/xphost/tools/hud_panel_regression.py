"""Exercise the two production HUD plug-ins in xphost's native panel recorder.

Build the xphost and both release plug-ins first. Only the plug-in DLLs and
public optics, runway and card definitions are needed.
"""
import argparse
import csv
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]


def fixture(path, name, binary, acf_name):
    plugin = path / 'plugins' / name
    (plugin / '64').mkdir(parents=True)
    shutil.copyfile(binary, plugin / '64/win.xpl')
    (path / acf_name).write_text('I\n1200 Version\nACF\nP acf/_Vfem_kts 104\n', encoding='ascii')
    return plugin


def run(host, plugin, acf, catalog, script, output):
    package = plugin.parents[1]
    output.mkdir(parents=True, exist_ok=True)
    (package / 'catalog.txt').write_text(catalog, encoding='ascii')
    (package / 'case.xps').write_text(script, encoding='ascii')
    result = subprocess.run([
        host, '--pkg', package, '--acf', acf, '--xpl', plugin / '64/win.xpl',
        '--profile', 'generic', '--clock', 'fast', '--datarefs', package / 'catalog.txt',
        '--script', package / 'case.xps', '--out', output,
    ], capture_output=True, text=True, timeout=45)
    (output / 'host.txt').write_text(result.stdout + result.stderr, encoding='utf-8')
    if result.returncode or 'GL context off' not in result.stdout:
        raise RuntimeError(f'{plugin.name}: {result.stdout}{result.stderr}')


def surface(output, frame, **expected):
    record = json.loads((output / (frame + '.json')).read_text(encoding='utf-8'))
    if record['errors']:
        raise AssertionError(f'{frame}: {record["errors"]}')
    matches = [s for s in record['surfaces'] if all(s.get(k) == v for k, v in expected.items())]
    if len(matches) != 1:
        raise AssertionError(f'{frame}: expected one surface matching {expected}')
    return matches[0]['commands']


def shuttle(host, binary, package, output):
    plugin = fixture(package, 'ShuttleHUD', binary, 'Orbiter_Glider.acf')
    for name in ['runways.csv', 'hud-optics.txt']:
        shutil.copyfile(ROOT / 'plugins/shuttle-hud' / name, plugin / name)
    catalog = ('sim/graphics/view/view_type\tint\ty\n'
               'sim/graphics/VR/enabled\tint\ty\n'
               'sim/cockpit2/switches/HUD_on\tint\ty\n'
               'sim/cockpit2/electrical/HUD_brightness_ratio\tfloat\ty\n')
    run(host, plugin, 'Orbiter_Glider.acf', catalog, """expect fsim_hud/aircraft_match 1 1
set sim/graphics/view/view_type 1026
set sim/cockpit2/switches/HUD_on 1
set sim/cockpit2/electrical/HUD_brightness_ratio 1
draw cockpit.json
expect fsim_hud/cockpit_draw_frames 1 1
set sim/graphics/view/view_type 1024
draw desktop.json
set sim/graphics/VR/enabled 1
draw vr.json
set sim/graphics/VR/enabled 0
set sim/cockpit2/switches/HUD_on 0
draw off.json
""", output)
    # The collimator sees the night panel. The floating overlay is suppressed
    # in both the cockpit and VR. Turning HUD power off leaves zero radiance.
    donor = {'device': '7', 'phase': 'before'}
    cockpit = surface(output, 'cockpit', **donor)
    assert cockpit[0]['op'] == 'polygon' and cockpit[0]['args']['color'] == 0xff000000
    strokes = [op['args']['points'] for op in cockpit if op['op'] == 'quadstrip']
    assert strokes and all(0 <= c <= 640 for strip in strokes for point in strip for c in point)
    assert not surface(output, 'cockpit', phase=None)
    assert any(op['op'] == 'quadstrip' for op in surface(output, 'desktop', phase=None))
    assert not surface(output, 'vr', phase=None)
    assert surface(output, 'off', **donor) == [cockpit[0]]
    assert not surface(output, 'off', phase=None)
    # The instrument ID is shared with real Garmin installations. A late
    # callback on an unmatched aircraft must leave that aircraft's MFD alone.
    (package / 'Other.acf').write_text('I\n1200 Version\nACF\n', encoding='ascii')
    other = output / 'other-aircraft'
    run(host, plugin, 'Other.acf', catalog,
        'expect fsim_hud/aircraft_match 0 0\ndraw stock.json\n', other)
    assert not surface(other, 'stock', **donor)
    assert not surface(other, 'stock', phase=None)


def poweroff(host, binary, package, output):
    plugin = fixture(package, 'XPTVideoHUD', binary, 'Synthetic.acf')
    config = package / 'plugins/XPTNativeGuidance/effective-card.ini'
    config.parent.mkdir()
    model = ROOT / 'crates/poweroff180'
    with (model / 'parameters.csv').open(encoding='ascii', newline='') as f:
        config.write_text(''.join(f"{r['name']}={r['default']}\n" for r in csv.DictReader(f)),
                          encoding='ascii')
    with (model / 'snapshot.csv').open(encoding='ascii', newline='') as f:
        fields = [r['field'] for r in csv.DictReader(f)]
    # Stand in for the separate guidance plug-in and publish its real array layout.
    run(host, plugin, 'Synthetic.acf', f'xpt/snapshot\tfloat[{len(fields)}]\ty\n',
        f"""draw idle.json
set xpt/snapshot[{fields.index('run_token')}] 1
set xpt/snapshot[{fields.index('configured')}] 1
draw instrument.json
expect xpt/video_hud/font_ready 1 1
expect xpt/video_hud/draw_frames 1 1
""", output)
    assert not surface(output, 'idle', phase=None)
    frame = surface(output, 'instrument', phase=None)
    glyphs = [op for op in frame if op['op'] == 'atlas_in']
    assert glyphs and any(op['clip'] for op in glyphs)
    assert any(op['op'] == 'polygon' for op in frame)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True, help='Directory for capture JSON and run logs')
    parser.add_argument('--host', type=Path, default=ROOT / 'tools/xphost/build/xphost.exe')
    parser.add_argument('--shuttle', type=Path, default=ROOT / 'target/release/shuttle_hud.dll')
    parser.add_argument('--poweroff', type=Path, default=ROOT / 'target/release/poweroff180_hud.dll')
    args = parser.parse_args()
    output = args.out.resolve()
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output, prefix='package-') as temporary:
        for case in [shuttle, poweroff]:
            name = case.__name__
            case(args.host.resolve(strict=True), getattr(args, name).resolve(strict=True),
                 Path(temporary) / name, output / name)
            print(f'{name}: native HUD draw checks passed')


if __name__ == '__main__':
    main()
