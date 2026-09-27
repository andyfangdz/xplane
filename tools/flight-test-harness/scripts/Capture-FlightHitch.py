"""Capture X-Plane's preceding profiler frames after one completed hitch per flight.

Read-only status/dataref observer. No control or heartbeat writes. Captures are
diagnostic and must never be pooled into the capture-free flag comparison.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import time
import urllib.request

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from xpt.config import atomic_json
from xpt.perf_capture import qpc


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', type=Path, required=True)
    args = parser.parse_args()
    run = args.run.resolve()
    deadline = time.monotonic() + 1500
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def get(url):
        with opener.open(url, timeout=5) as response:
            return json.load(response)

    while not (run / 'session.json').is_file():
        if time.monotonic() >= deadline:
            raise TimeoutError('Session was not created')
        time.sleep(.5)
    session = json.loads((run / 'session.json').read_text(encoding='utf-8-sig'))
    run.relative_to(Path(session['xplane_root']).resolve() / 'Output/performance-tests')
    if session.get('mode') != 'Flight' or not session.get('trace_flight'):
        raise ValueError('A passive Flight transaction is required')
    out = run / 'microprofile'
    out.mkdir(exist_ok=False)
    atomic_json(out / 'observer.json', {
        'source_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'diagnostic_only': True, 'interval_seconds': .25, 'capture_delay_seconds': .25,
        'method': 'One retrospective 240-frame dump per flight, triggered by the native completed >50 ms hitch latch, excluding first five seconds after unpause. Wait 250 ms for profiler frame publication before requesting the dump. Exact observed/request QPC times retained. Validate actual frame coverage afterward; do not attribute any frame after the request to unperturbed flight.'})
    captured = set()
    current = None
    href = None
    status_url = None
    records = []
    errors = []
    try:
        while time.monotonic() < deadline:
            if (run / 'restoration.json').exists() or (run / 'cancel.request').exists():
                break
            try:
                if status_url is None:
                    status_url = json.loads((run / 'status.json').read_text())['status_url'] + '/status'
                status = get(status_url)
                if status.get('phase') in ('failed', 'awaiting_restoration'):
                    break
                card = status.get('card')
                if card != current:
                    current, href = card, None
                active = status.get('phase') in ('downwind', 'delay', 'turn_to_base',
                    'base', 'turn_to_final', 'final', 'rollout')
                if card in ('calm-01', 'calm-02') and card not in captured and active:
                    if href is None:
                        catalog = get('http://127.0.0.1:8153/api/v3/datarefs?limit=30000')['data']
                        href = next(r['id'] for r in catalog if r['name'] == 'xpt/perf/hitch')
                    snapshot = get(f'http://127.0.0.1:8153/api/v3/datarefs/{href}/value')['data']
                    if len(snapshot) != 8:
                        raise ValueError('Unexpected native hitch protocol')
                    if snapshot[1] > 0:
                        ticks = sum(int(snapshot[4+i]) << (16*i) for i in range(4))
                        observed_qpc = qpc()
                        # The published Microprofile frame ring can lag the native
                        # callback. This observer never supplies flight heartbeats.
                        time.sleep(.25)
                        now = qpc()
                        # Windows QPC frequency is retained in pacing.json afterward.
                        record = {'card': card, 'snapshot': snapshot, 'hitch_end_qpc': ticks,
                            'trigger_observed_qpc': observed_qpc,
                            'request_start_qpc': now, 'request_start_epoch': time.time(),
                            'status': status, 'capture': card + '.html'}
                        records.append(record)
                        captured.add(card)
                        atomic_json(out / 'capture-events.json', records)
                        try:
                            with opener.open('http://127.0.0.1:1338/240', timeout=20) as response:
                                content = response.read()
                            if b'MakeFrame' not in content or b'ThreadNames' not in content:
                                raise ValueError('Response is not a Microprofile frame capture')
                            (out / record['capture']).write_bytes(content)
                            record['bytes'] = len(content)
                        except Exception as error:
                            record['error'] = str(error)
                        record['request_end_qpc'] = qpc()
                        record['request_end_epoch'] = time.time()
                        atomic_json(out / 'capture-events.json', records)
                        print(json.dumps(record), flush=True)
            except (OSError, ValueError, KeyError, StopIteration) as error:
                if not errors or errors[-1]['error'] != str(error):
                    errors.append({'error': str(error), 'qpc': qpc()})
                    atomic_json(out / 'observer-errors.json', errors)
                href = None
            time.sleep(.25)
        else:
            raise TimeoutError('Observer exceeded its bounded lifetime')
    finally:
        atomic_json(out / 'observer-finished.json', {'qpc': qpc(), 'captures': len(records)})
    print(json.dumps({'finished': True, 'captures': len(records)}), flush=True)


if __name__ == '__main__':
    main()
