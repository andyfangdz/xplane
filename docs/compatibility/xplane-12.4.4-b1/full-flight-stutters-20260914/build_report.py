"""Rebuild the retained full-flight comparison artifacts without starting X-Plane."""
import csv
import gzip
import hashlib
import json
from pathlib import Path
import shutil
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

HERE = Path(__file__).resolve().parent
BASE = Path('D:/X-Plane 12/Output/performance-tests')
MATRIX = BASE / 'XPT_FULL_FLIGHT_AB02_20260914'
data = json.loads((MATRIX / 'comparison.json').read_text())
assert data['complete'] and data['baseline_reproduced'] and not data['comparison_issues']
plt.rcParams.update({'font.family': 'DejaVu Sans', 'font.size': 10})
fig, axes = plt.subplots(6, 2, figsize=(13, 13), sharex=True, sharey=True)
audit = []
records = []
sources = []
for attempt in data['attempts']:
    run = BASE / attempt['run']
    flag = attempt['flag']
    copy = HERE / 'evidence' / run.name
    copy.mkdir(parents=True, exist_ok=True)
    for name in ('source-manifest.json', 'restoration.json', 'protected-state-verification.json', 'flight-pacing.json'):
        shutil.copy2(run / name, copy / name)
    session = json.loads((run / 'session.json').read_text(encoding='utf-8-sig'))
    source = json.loads((run / 'source-manifest.json').read_text())['harness_sha256']
    sources.append(source)
    assert ('--gfx_no_defragment' in session['simulator']['arguments']) == flag
    assert session['stutter_test'] == {'no_defragment': flag, 'texture_cache_gb': 10}
    prefs = (run / 'after-preferences/X-Plane.prf').read_text(encoding='utf-8-sig')
    assert '_tex_cache_gb 10' in prefs and '_tex_res 5' in prefs and '_eq_vr 0' in prefs
    audit.append({'run': run.name, 'flag': flag, 'launch_arguments': session['simulator']['arguments'],
                  'preferences_verified': {'cache_gb': 10, 'texture_resolution': 5, 'vr': 0},
                  'scenery_retained': session['keep_custom_scenery'],
                  'isolated_plugins': [p['name'] for p in session['global_plugin_isolation']],
                  'restoration': json.loads((run / 'restoration.json').read_text(encoding='utf-8-sig'))})
    for index, flight in enumerate(attempt['report']['flights']):
        assert flight['valid']
        card = run / 'cards' / flight['card']
        out = copy / 'cards' / flight['card']
        out.mkdir(parents=True, exist_ok=True)
        for name in ('pacing.json', 'pacing-assessment.json', 'assessment.json', 'pacing-readiness.json'):
            shutil.copy2(card / name, out / name)
        for name in ('pacing.csv', 'trace.csv'):
            (out / (name + '.gz')).write_bytes(gzip.compress((card / name).read_bytes(), mtime=0))
        frames = list(csv.DictReader((card / 'pacing.csv').open()))
        moving = [r for r in frames if r['paused'] == '0']
        q0 = flight['flight_start_qpc']
        freq = flight['qpc_frequency']
        times = [(int(r['qpc']) - q0) / freq for r in moving[1:]]
        intervals = [float(r['wall_frame_dt_s']) * 1000 for r in moving[1:]]
        row = (attempt['pair'] - 1) * 2 + index
        ax = axes[row, int(flag)]
        ax.axvspan(0, 5, color='#e4e7eb', alpha=.8)
        ax.plot(times, intervals, color='#167481' if flag else '#b64d38', linewidth=.45)
        ax.axhline(50, color='#505968', linestyle='--', linewidth=.7)
        ax.set_yscale('log')
        ax.set_ylim(7, 1100)
        ax.set_xlim(0, 165)
        ax.set_yticks([10, 50, 200, 800], labels=['10', '50', '200', '800'])
        ax.grid(axis='y', color='#dce1e8', linewidth=.5)
        for spine in ('top', 'right'):
            ax.spines[spine].set_visible(False)
        s = flight['primary']
        ax.text(.98, .95, f">50 ms: {s['over_50ms']}  |  max: {s['max_ms']:.0f} ms",
                transform=ax.transAxes, ha='right', va='top', fontsize=9,
                bbox=dict(facecolor='white', edgecolor='none', alpha=.9))
        if not flag:
            ax.set_ylabel(f"Pair {attempt['pair']} / flight {index+1}\nInterval (ms)")
        records.append({'run': run.name, 'pair': attempt['pair'], 'flag': flag, 'card': flight['card'],
                        'landing_passed': flight['landing_passed'], **s})
assert all(s == sources[0] for s in sources)
axes[0, 0].set_title('Flag off', fontsize=15, color='#b64d38', pad=13)
axes[0, 1].set_title('Flag on', fontsize=15, color='#167481', pad=13)
for ax in axes[-1]:
    ax.set_xlabel('Seconds since unpause')
fig.suptitle('Full flights reproduce stutters with both flag settings', fontsize=18, y=.994)
fig.text(.5, .968, 'Same scenery, graphics and 10 GB cache • Gray: first 5 seconds, excluded from primary counts • Logarithmic interval axis', ha='center', fontsize=10)
fig.tight_layout(rect=(0, .005, 1, .956))
fig.savefig(HERE / 'flight-timelines.png', dpi=170)
fig.savefig(HERE / 'flight-timelines.svg')
plt.close(fig)
(HERE / 'configuration-audit.json').write_text(json.dumps({'sources_identical': True, 'runs': audit}, indent=2) + '\n')
(HERE / 'per-flight.json').write_text(json.dumps(records, indent=2) + '\n')
with (HERE / 'per-flight.csv').open('w', newline='') as f:
    writer = csv.DictWriter(f, fieldnames=list(records[0]))
    writer.writeheader()
    writer.writerows(records)
print(json.dumps({'flights': len(records), 'sources_identical': True, 'restored': all(a['restoration']['restored'] for a in audit)}))
