"""Audit every planned texture trial and summarize independent process repeats."""
from pathlib import Path
from datetime import datetime, timezone
import csv
import hashlib
import json
import statistics

HERE = Path(__file__).resolve().parent
ROOT = Path('D:/X-Plane 12/Output/performance-tests')


def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def aggregate(rows):
    exposure = sum(r['primary']['exposure_seconds'] for r in rows)
    if not rows:
        return {'flights': 0, 'exposure_seconds': 0}
    return dict(flights=len(rows), exposure_seconds=exposure,
        max_ms=max(r['primary']['max_ms'] for r in rows),
        flights_with_over_50ms=sum(r['primary']['over_50ms'] > 0 for r in rows),
        **{f'over_{t}ms': sum(r['primary'][f'over_{t}ms'] for r in rows) for t in (25, 50, 100)},
        **{f'over_{t}ms_per_minute': sum(r['primary'][f'over_{t}ms'] for r in rows) * 60 / exposure for t in (25, 50, 100)})


def main():
    plan = read(HERE / 'plan.json')
    trials = []
    issues = []
    sources = []
    for trial in plan['runs']:
        path = ROOT / trial['name']
        report = read(path / 'flight-pacing.json')
        session = read(path / 'session.json')
        source = read(path / 'source-manifest.json')
        restoration = read(path / 'restoration.json')
        after = read(HERE / f"{trial['name']}-after.json")
        assert restoration['restored'] and after['nvidia_matches']
        assert report['settings'] == {'texture_cache_gb': 10, 'no_defragment': True}
        assert session['rendering_profile'] == 'User2D'
        assert session['keep_custom_scenery'] and session['visible_simulator']
        assert session['trace_flight'] and not session['record_video']
        assert session['rendering_preferences'] == {
            '_eq_vr': 0, '_tex_res': trial['texture_resolution'],
            'renopt_tex_res_04': trial['texture_resolution'] - 1}
        assert '--gfx_no_defragment' in session['simulator']['arguments']
        assert [p['name'] for p in session['global_plugin_isolation']] == ['TelemFFB-XPP']
        assert len(report['flights']) == 2 and all(f['valid'] for f in report['flights'])
        assert after['hags_override'] == 1
        if not report['complete']:
            assert set(report['errors']) == {'recovery-error.json'}, report['errors']
            assert report['errors']['recovery-error.json']['error'] == 'An X-Plane process is active; recovery will not move files'
            issues.append({'run': trial['name'], 'errors': report['errors']})
        assert not list(path.rglob('*.etl')), 'Unexpected system trace'
        sources.append(source)
        flights = []
        for ordinal, f in enumerate(report['flights'], 1):
            rows = list(csv.DictReader((path / 'cards' / f['card'] / 'pacing.csv').open(newline='')))
            meta = read(path / 'cards' / f['card'] / 'pacing.json')
            moving = [r for r in rows if int(r['paused']) == 0]
            first = int(moving[0]['qpc'])
            hz = meta['qpc_frequency']
            primary = [(int(b['qpc']) - int(a['qpc'])) * 1000 / hz
                       for a, b in zip(moving, moving[1:])
                       if (int(a['qpc']) - first) / hz >= 5]
            assert abs(sum(primary) / 1000 - f['primary']['exposure_seconds']) < 1e-7
            for t in (25, 50, 100):
                assert sum(v > t for v in primary) == f['primary'][f'over_{t}ms']
            assert abs(max(primary) - f['primary']['max_ms']) < 1e-7
            startup = read(path / 'cards' / f['card'] / 'startup-resume.json')
            rendering = read(path / 'cards' / f['card'] / 'rendering-readback.json')
            assert rendering['vr_enabled'] == 0
            flights.append({**f, 'ordinal': ordinal, 'startup_resume': startup})
        trials.append({**trial, 'runner_complete': report['complete'], 'errors': report['errors'],
            'restoration': restoration, 'flights': flights, 'aggregate': aggregate(flights),
            'simulator': session['simulator']})
    assert all(s == sources[0] for s in sources), 'Source/configuration manifests differ'
    groups = {}
    for texture in (5, 4):
        runs = [r for r in trials if r['texture_resolution'] == texture]
        flights = [f for r in runs for f in r['flights']]
        groups[str(texture)] = dict(all=aggregate(flights),
            first=aggregate([f for f in flights if f['ordinal'] == 1]),
            second=aggregate([f for f in flights if f['ordinal'] == 2]),
            fresh_processes=len(runs),
            processes_with_over_50ms=sum(r['aggregate']['over_50ms'] > 0 for r in runs))
    pairs = []
    for pair in (1, 2, 3):
        sides = {r['texture_resolution']: r for r in trials if r['pair'] == pair}
        rates = {t: r['aggregate']['over_50ms_per_minute'] for t, r in sides.items()}
        pairs.append({'pair': pair, 'max_rate': rates[5], 'lower_rate': rates[4],
            'lower_minus_max': rates[4] - rates[5],
            'observed_reduction_percent': 100 * (1 - rates[4] / rates[5]) if rates[5] else None})
    max_rate = groups['5']['all']['over_50ms_per_minute']
    low_rate = groups['4']['all']['over_50ms_per_minute']
    excluded = []
    for attempt in plan.get('excluded_attempts', []):
        path = ROOT / attempt['name']
        restoration = read(path / 'restoration.json')
        report = read(path / 'flight-pacing.json')
        assert restoration['restored'] and report['valid_flights'] == 0
        assert not report['complete']
        excluded.append({**attempt, 'restoration': restoration, 'errors': report['errors'],
                         'valid_flights': report['valid_flights']})
    result = dict(utc=datetime.now(timezone.utc).isoformat(), plan=plan, trials=trials,
        groups=groups, pairs=pairs, comparison_issues=issues,
        excluded_attempts=excluded,
        sources_identical=True, baseline_reproduced=groups['5']['processes_with_over_50ms'] >= 2,
        observed_reduction_percent=100 * (1 - low_rate / max_rate) if max_rate else None,
        all_pairs_lower=all(p['lower_minus_max'] < 0 for p in pairs),
        all_accepted_flights_valid=True,
        clean_runner_campaign=not issues and not excluded,
        limitations=['Three independent launches per setting; frames are not independent replicates',
                     'Warm evolving caches; paired order alternates AB/BA/AB',
                     'Native callbacks measure simulator pacing, not display or VR presentation',
                     'All flights are short 2D maneuvers; no long-flight or VR claim',
                     'No system tracing, Microprofile requests, video or GPU observer'])
    (HERE / 'results.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    lines = ['| Pair | Textures | Flight | >50 ms | >100 ms | Worst (ms) | p99.9 (ms) | Exposure (s) |',
             '|---|---|---|---:|---:|---:|---:|---:|']
    for r in trials:
        for f in r['flights']:
            p = f['primary']
            lines.append(f"| {r['pair']} | {r['texture_resolution']} | {f['ordinal']} | {p['over_50ms']} | {p['over_100ms']} | {p['max_ms']:.2f} | {p['p999_ms']:.2f} | {p['exposure_seconds']:.2f} |")
    (HERE / 'flight-table.md').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    print(json.dumps({k: result[k] for k in ('groups', 'pairs', 'baseline_reproduced', 'observed_reduction_percent', 'all_pairs_lower', 'comparison_issues')}, indent=2))


if __name__ == '__main__':
    main()
