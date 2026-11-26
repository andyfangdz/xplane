"""Exercise the two production HUD plug-ins in xphost's native panel recorder.

Build the xphost and both release plug-ins first. Only the plug-in DLLs and
public aircraft definitions and the installed Roboto-Regular.ttf are needed.
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
draw cockpit-replay.json
set fsim_hud/brightness 0.25
draw cockpit-dim.json
set fsim_hud/brightness 1
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
    repeated = surface(output, 'cockpit-replay', **donor)
    dimmed = surface(output, 'cockpit-dim', **donor)
    assert cockpit == repeated
    bright_ink = [op['args']['color'] for op in cockpit if op['op'] == 'quadstrip']
    dim_ink = [op['args']['color'] for op in dimmed if op['op'] == 'quadstrip']
    assert len(dim_ink) == len(bright_ink) and all((b >> 8 & 255) > (d >> 8 & 255) for b, d in zip(bright_ink, dim_ink))
    first, again, dim = [json.loads((output / (frame + '.json')).read_text())['retained']
                         for frame in ['cockpit', 'cockpit-replay', 'cockpit-dim']]
    assert first['recorded'] >= 2 and again['recorded'] == first['recorded']
    assert again['replayed'] > first['replayed'] and dim['destroyed'] > again['destroyed']
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
    heading = 'sim/cockpit2/gauges/indicators/heading_AHARS_deg_mag_pilot'
    catalog = f'xpt/snapshot\tfloat[{len(fields)}]\ty\n{heading}\tfloat\ty\n'
    run(host, plugin, 'Synthetic.acf', catalog,
        f"""draw idle.json
set xpt/snapshot[{fields.index('run_token')}] 1
set xpt/snapshot[{fields.index('configured')}] 1
draw instrument.json
expect xpt/video_hud/font_ready 1 1
expect xpt/video_hud/draw_frames 1 1
set {heading} 90
draw quarter-turn.json
""", output)
    assert not surface(output, 'idle', phase=None)
    frame = surface(output, 'instrument', phase=None)
    assert any(op['op'] == 'polygon' for op in frame)
    assert not any(op['op'].startswith('atlas_') for op in frame)
    texts = [op for op in frame if op['op'] == 'text']
    assert any(op['clip'] for op in texts)
    # Reference coordinates are global boxels on the host's 1920x1080 desktop.
    # Widths are measured by GDI on the original Arial strike.
    white, gold = 0xfffcfaf7, 0xff30deff
    for label, x, top, size, justify, ink, width in [
            ('POWER-OFF 180  |  KCDW RWY 22', 286, 15, 24, 0, white, 727),
            ('CONTROL INPUT', 1660, 374, 22, 0, white, 357),
            ('0%', 128, 192, 48, 1, white, 63),
            ('ROUNDOUT 35 FT', 850, 72, 22, 0, gold, 379),
            ('3.5 NM', 498, 1008, 16, 2, 0xffccc4b8, 141),
            ('IDLE', 284, 1048, 20, 0, white, 97)]:
        extent = width * size / 44
        left = x - extent * justify * .5
        y = 1080 - top - 40 * size / 44
        def within(op, dx, dy):
            px, py = op['args']['anchor']
            return left + dx - .1 <= px < left + dx + extent and abs(py - dy - y) < .1
        foreground = [op for op in texts if op['args']['color'] == ink and within(op, 0, 0)]
        shadows = [op for op in texts if op['args']['color'] == 0xcc000000 and within(op, 1.2, -1.2)]
        assert ''.join(op['args']['text'] for op in foreground) == label.replace(' ', '')
        assert len(foreground) == len(shadows)
        assert abs(foreground[0]['args']['anchor'][0] - left) < .1
        last = foreground[-1]
        right = last['args']['anchor'][0] + last['args']['advance'] * last['transform'][0]
        assert abs(right - (left + extent)) < .1
        for op, shadow in zip(foreground, shadows):
            fx, fy = op['args']['anchor']
            sx, sy = shadow['args']['anchor']
            assert abs(sx - fx - 1.2) < .01 and abs(sy - fy + 1.2) < .01
    turned = surface(output, 'quarter-turn', phase=None)
    def north(ops):
        return next(op['args']['anchor'] for op in ops if op['op'] == 'text'
                    and op['args']['text'] == 'N' and op['args']['color'] == 0xffd6d9d1)
    before, after = north(frame), north(turned)
    # The left baseline of the N sits half a glyph left of its centered anchor.
    # At a quarter turn that same half-width points down from the compass center.
    assert 949 < before[0] < 953 and before[1] > 290
    assert after[0] < 870 and 189 < after[1] < 194
    recorded, rotated = [json.loads((output / (name + '.json')).read_text())['retained']
                         for name in ['instrument', 'quarter-turn']]
    assert recorded['recorded'] == rotated['recorded'] == 7
    assert rotated['replayed'] >= recorded['replayed'] + 7


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True, help='Directory for capture JSON and run logs')
    parser.add_argument('--host', type=Path, default=ROOT / 'tools/xphost/build/xphost.exe')
    parser.add_argument('--shuttle', type=Path, default=ROOT / 'target/release/shuttle_hud.dll')
    parser.add_argument('--poweroff', type=Path, default=ROOT / 'target/release/poweroff180_hud.dll')
    parser.add_argument('--font', type=Path, required=True,
                        help="X-Plane's Resources/fonts/Roboto-Regular.ttf")
    args = parser.parse_args()
    output = args.out.resolve()
    output.mkdir(parents=True, exist_ok=True)
    installed_font = output / 'poweroff/xplane/Resources/fonts/Roboto-Regular.ttf'
    installed_font.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(args.font.resolve(strict=True), installed_font)
    with tempfile.TemporaryDirectory(dir=output, prefix='package-') as temporary:
        for case in [shuttle, poweroff]:
            name = case.__name__
            case(args.host.resolve(strict=True), getattr(args, name).resolve(strict=True),
                 Path(temporary) / name, output / name)
            print(f'{name}: native HUD draw checks passed')


if __name__ == '__main__':
    main()
