"""Keep a newly spawned worker inert until the launcher can recover it."""
import json
import time


def wait_for_launch(directory, timeout=10):
    deadline = time.monotonic() + timeout
    while True:
        if (directory / 'restoration.json').exists() or (directory / 'cancel.request').exists():
            raise RuntimeError('Launch cancelled before worker identity publication')
        session = json.loads((directory / 'session.json').read_text(encoding='utf-8-sig'))
        if session.get('worker') and session.get('simulator'):
            return session
        if time.monotonic() >= deadline:
            raise TimeoutError('Launcher did not publish worker identity; simulator API was not opened')
        time.sleep(.05)
