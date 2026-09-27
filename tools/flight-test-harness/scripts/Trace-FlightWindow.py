"""Separate, bounded WPR observer. Never writes simulator controls or heartbeats."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import time
import urllib.request

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from xpt.config import atomic_json
from xpt.perf_capture import WprHost, qpc


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', type=Path, required=True)
    parser.add_argument('--card', default='calm-02')
    parser.add_argument('--start-native-seconds', type=float, default=90)
    parser.add_argument('--stop-native-seconds', type=float, default=140)
    args = parser.parse_args()
    if not 0 < args.start_native_seconds < args.stop_native_seconds <= 180:
        parser.error('Invalid bounded native flight-clock window')
    if args.card not in ('calm-01', 'calm-02'):
        parser.error('Only the retained calm two-flight sequence is supported')
    run = args.run.resolve()
    deadline = time.monotonic() + 1500
    while not (run / 'session.json').is_file():
        if time.monotonic() > deadline:
            raise TimeoutError('Session was not created')
        time.sleep(.5)
    session = json.loads((run / 'session.json').read_text(encoding='utf-8-sig'))
    root = Path(session['xplane_root']).resolve()
    run.relative_to(root / 'Output/performance-tests')
    if session.get('mode') != 'Flight' or not session.get('trace_flight'):
        raise ValueError('Observer requires an existing passive Flight transaction')
    host = WprHost(root)
    host.request('ping')
    label = args.card + '-diagnostic'
    segment = run / 'segments' / label
    segment.mkdir(parents=True, exist_ok=False)
    atomic_json(segment / 'observer.json', {
        'card': args.card, 'start_native_seconds': args.start_native_seconds,
        'stop_native_seconds': args.stop_native_seconds,
        'source_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'diagnostic_only': True,
        'method': 'Separate status observer; no control writes. Exclude the first ten seconds after WPR start acknowledgement and everything after the stop request. Native flight-clock thresholds are not QPC elapsed seconds; align retained QPC timestamps afterward.'})
    active = None
    status_url = None
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    try:
        while time.monotonic() < deadline:
            if (run / 'restoration.json').exists() or (run / 'cancel.request').exists():
                raise RuntimeError('Flight ended or was cancelled before capture completed')
            if status_url is None:
                try:
                    status_url = json.loads((run / 'status.json').read_text())['status_url'] + '/status'
                except (OSError, KeyError, ValueError):
                    time.sleep(.5)
                    continue
            with opener.open(status_url, timeout=3) as response:
                status = json.load(response)
            if status.get('phase') in ('failed', 'awaiting_restoration'):
                raise RuntimeError('Flight ended before the requested trace window')
            native = status.get('native') or {}
            seconds = native.get('sim_time', -1)
            target = (status.get('card') == args.card and bool(native) and
                      status.get('phase') in ('downwind', 'delay', 'turn_to_base',
                                              'base', 'turn_to_final', 'final', 'rollout'))
            if active is None and target and seconds >= args.start_native_seconds:
                if seconds > args.start_native_seconds + 10:
                    raise RuntimeError('Observer missed its start window')
                active = {'nonce': host.host['nonce'],
                          'instance': 'XPT_' + host.host['nonce'] + '_' + session['token'][:8] + '_' + label,
                          'request_qpc': qpc(), 'status_at_request': status}
                atomic_json(segment / 'wpr-start.json', active)
                active.update(host.request('start', run_directory=str(run), segment=label))
                active['acknowledged_qpc'] = qpc()
                atomic_json(segment / 'wpr-start.json', active)
                print(json.dumps({'phase': 'capturing', 'segment': str(segment)}), flush=True)
            if active is not None and target and seconds >= args.stop_native_seconds:
                atomic_json(segment / 'stop-request.json', {'qpc': qpc(), 'status': status})
                result = host.request('stop', instance=active['instance'])
                atomic_json(segment / 'wpr-stop.json', result)
                active = None
                print(json.dumps({'phase': 'saved', 'segment': str(segment)}), flush=True)
                return
            time.sleep(.5)
        raise TimeoutError('Diagnostic observer exceeded its bounded lifetime')
    except BaseException as error:
        atomic_json(segment / 'observer-error.json', {'error': str(error), 'qpc': qpc()})
        raise
    finally:
        if active is not None:
            # Host requests are idempotent; recovery can also stop this owned instance.
            atomic_json(segment / 'observer-finally-stop.json', {'qpc': qpc()})
            atomic_json(segment / 'wpr-stop.json', host.request('stop', instance=active['instance']))


if __name__ == '__main__':
    main()
