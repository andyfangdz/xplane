"""Decode the snapshot schema shared with the native controller and HUD."""
import csv
from .config import REPO

with (REPO/'crates/poweroff180/snapshot.csv').open(newline='', encoding='utf-8') as schema:
    FIELDS = [row['field'] for row in csv.DictReader(schema)]

PHASES = ['idle', 'ready', 'downwind', 'delay', 'turn_to_base', 'base', 'turn_to_final', 'final', 'rollout', 'complete', 'aborted']
REASONS = ['none', 'entry_gate', 'low_before_alignment', 'envelope', 'sim_timeout', 'supervisor_lost',
           'invalid_configuration', 'native_frame_gap', 'wind_mismatch', 'mass_mismatch', 'override_conflict',
           'cancelled', 'missing_dataref', 'trace_error']

def decode(values):
    if len(values) != len(FIELDS):
        raise RuntimeError(f'Native protocol mismatch: {len(values)} fields, expected {len(FIELDS)}')
    result = dict(zip(FIELDS, values))
    result['phase'] = PHASES[int(result['phase_id'])]
    result['reason'] = REASONS[int(result['reason_id'])]
    return result
