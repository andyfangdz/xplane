"""Inspect startup through a fresh catalog; optional single guarded resume."""
from pathlib import Path
from datetime import datetime, timezone
import argparse
import json
import sys
import time

HERE = Path(__file__).resolve().parent
sys.path.insert(0, 'V:/src/xplane/tools/flight-test-harness/vendor')
from flight_test.api import XPlaneApi

p = argparse.ArgumentParser()
p.add_argument('--resume', action='store_true')
args = p.parse_args()
trial = json.loads((HERE / 'current-trial.json').read_text(encoding='utf-8-sig'))['trial']
run = Path('D:/X-Plane 12/Output/performance-tests') / trial['name']
status = json.loads((run / 'status.json').read_text(encoding='utf-8-sig'))
assert status['phase'] == 'preparing' and status['completed'] == 0
card = run / 'cards' / status['card']
assert not (card / 'pacing-start.json').exists()
assert not (card / 'air-start-seed.json').exists()
record = {'utc': datetime.now(timezone.utc).isoformat(), 'run': run.name,
          'stage': status, 'resume_requested': args.resume, 'command_sent': False}
keys = ['sim/time/paused', 'sim/time/total_running_time_sec',
        'sim/flightmodel/position/elevation',
        'sim/operation/override/override_planepath', 'sr20g6/test_controller/armed']
try:
    with XPlaneApi(port=8153, timeout=3) as api:
        api.refresh_catalogs()
        record['before'] = {k: api.get_raw(k) for k in keys}
        record['pause_off_command'] = api.commands['sim/operation/pause_off']
        if args.resume:
            assert record['before']['sim/time/paused'] == 1
            assert record['before']['sim/operation/override/override_planepath'][0] == 1
            assert record['before']['sr20g6/test_controller/armed'] == 0
            assert record['before']['sim/time/total_running_time_sec'] == 0
            # The harness has frozen the setup path and is trying to resume
            # initialization. Use one fresh-catalog command; never open a menu.
            record['command_sent'] = True
            api.command('sim/operation/pause_off', .2)
            time.sleep(.5)
            record['after'] = {k: api.get_raw(k) for k in keys}
except Exception as error:
    record['error'] = repr(error)
finally:
    output = HERE / f"{run.name}-startup-{'resume' if args.resume else 'probe'}.json"
    if output.exists():
        output = output.with_stem(output.stem + '-' + str(time.time_ns()))
    output.write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(record, indent=2))
