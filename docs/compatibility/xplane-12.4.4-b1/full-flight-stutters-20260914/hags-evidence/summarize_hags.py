"""Summarize restored HAGS runs; preserve the earlier diagnostic's cleanup error."""
from pathlib import Path
import json
from datetime import datetime, timezone

HERE = Path(__file__).resolve().parent
ROOT = Path('D:/X-Plane 12/Output/performance-tests')
PRIOR = 'XPT_DXGI_B1_20260914'
RUNS = [f'XPT_HAGSOFF_DXGI_{n:02d}_20260915' for n in (2, 3, 4)]

def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))

def run_summary(name):
    path = ROOT / name
    result = read(path / 'flight-pacing.json')
    source = read(path / 'source-manifest.json')
    session = read(path / 'session.json')
    restored = read(path / 'restoration.json')
    nv = read(path / f'{name}-nv-restored.json')
    flights = []
    for f in result['flights']:
        flights.append({k: f[k] for k in ('card', 'valid', 'reasons', 'landing_passed',
            'flight_seconds', 'primary', 'activation_first_5_seconds')})
        flights[-1]['hitches_over_50ms'] = [h for h in f['hitches']
            if not h['activation'] and h['interval_ms'] > 50]
    return dict(run=name, complete=result['complete'], errors=result['errors'],
        restoration=restored, nvidia_restored_override=nv['setting'],
        settings=result['settings'], rendering_profile=session['rendering_profile'],
        flights=flights, source=source)

def aggregate(rows):
    exposure = sum(r['primary']['exposure_seconds'] for r in rows)
    return dict(flights=len(rows), exposure_seconds=exposure,
        flights_with_over_50ms=sum(r['primary']['over_50ms'] > 0 for r in rows),
        max_ms=max(r['primary']['max_ms'] for r in rows),
        **{f'over_{t}ms': sum(r['primary'][f'over_{t}ms'] for r in rows) for t in (25,50,100)},
        **{f'over_{t}ms_per_minute': sum(r['primary'][f'over_{t}ms'] for r in rows)*60/exposure for t in (25,50,100)})

def main():
    before = run_summary(PRIOR)
    after = [run_summary(name) for name in RUNS]
    for run in after:
        if not run['complete']:
            # Retain the shutdown guard failure; it does not erase completed
            # timing records, but this session is not a clean runner pass.
            assert set(run['errors']) == {'recovery-error.json'}, (run['run'], run['errors'])
            assert run['errors']['recovery-error.json']['error'] == 'An X-Plane process is active; recovery will not move files'
        assert run['restoration']['restored']
        assert run['nvidia_restored_override'] is None
        assert len(run['flights']) == 2 and all(f['valid'] for f in run['flights'])
        assert run['source'] == before['source'], f"Source manifest differs: {run['run']}"
        assert run['settings'] == before['settings']
        assert run['rendering_profile'] == before['rendering_profile']
    first = [r['flights'][0] for r in after]
    second = [r['flights'][1] for r in after]
    document = dict(utc=datetime.now(timezone.utc).isoformat(),
        method='Native after-physics QPC intervals; initial 5 seconds excluded from primary statistics; no WPR/video/GPU observer.',
        attribution_limit='HAGS changed from enabled to disabled, Windows updated from 26200.9278 to 26200.9445, and the system rebooted. A matched updated-Windows HAGS-on replication is needed for causal attribution.',
        preflight=read(HERE/'postboot-preflight.json'), before=before, after=after,
        aggregate=dict(before_all=aggregate(before['flights']), after_all=aggregate(first+second),
            after_first=aggregate(first), after_second=aggregate(second)),
        excluded_startup='XPT_HAGSOFF_DXGI_01_20260915: missing unmounted XPME terrain; no measured flights; restored.')
    lines = ['# Post-reboot HAGS-off measurements', '', document['method'], '',
        document['attribution_limit'], '',
        '| Session | Flight | Timing valid | >25 ms | >50 ms | >100 ms | p99.9 (ms) | Worst (ms) | Exposure (s) |',
        '|---|---|---|---:|---:|---:|---:|---:|---:|']
    for run in [before] + after:
        for f in run['flights']:
            p = f['primary']
            lines.append(f"| {run['run']} | {f['card']} | {f['valid']} | {p['over_25ms']} | {p['over_50ms']} | {p['over_100ms']} | {p['p999_ms']:.2f} | {p['max_ms']:.2f} | {p['exposure_seconds']:.2f} |")
    lines += ['', 'The earlier DXGI session and the final post-reboot session have valid individual timing records but retain their original shutdown recovery guard errors; restoration later succeeded through the existing recovery entry point. This is a diagnostic comparison, not a clean completed A/B campaign. All included post-reboot records pass timing and verified restoration checks; runner completeness and original errors remain in the result JSON.', '',
        'Landing acceptance is separate from frame timing. Every landing result and rejected startup is retained in the raw runs.', '',
        'Primary statistics exclude the first five unpaused seconds. The result JSON also retains activation-window statistics and every primary interval above 50 ms. Shorter delays can remain visible even when no interval exceeds 50 ms.', '']
    (HERE/'hags-table.md').write_text('\n'.join(lines), encoding='utf-8')
    (HERE/'hags-results.json').write_text(json.dumps(document, indent=2)+'\n', encoding='utf-8')
    print(json.dumps(document['aggregate'], indent=2))

if __name__ == '__main__':
    main()
