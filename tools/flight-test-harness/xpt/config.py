"""Single parameter schema used by Python, native code, and run manifests."""
from __future__ import annotations
import csv
import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_VERSION = 1
# The Rust workspace CSV is the sole parameter authority. Cargo and Python both
# consume its complete, ordered schema; no compiler or Python generator is needed.
REPO = ROOT.parents[1]
PARAMETERS = {}
with (REPO/'crates/poweroff180/parameters.csv').open(newline='', encoding='utf-8') as schema:
    for row in csv.DictReader(schema):
        name = row['name']
        if name in PARAMETERS:
            raise ValueError(f'Duplicate parameter in schema: {name}')
        PARAMETERS[name] = tuple(float(row[key]) for key in ('default', 'minimum', 'maximum'))

ACCEPTANCE = {
    'touchdown_min_ft': 1000, 'touchdown_max_ft': 1200,
    'touchdown_max_cross_ft': 8, 'short_final_max_cross_ft': 15,
    'short_final_start_ft': -1000, 'maximum_sink_fpm': 200,
    'minimum_touchdown_kias': 63, 'maximum_touchdown_kias': 67,
    'maximum_roundout_agl_ft': 35, 'maximum_pitch_deg': 7.5,
    'maximum_pitch_rate_deg_s': 2.8, 'maximum_post_contact_agl_ft': 2.5,
}
DEFAULT_CARDS = [{'name': n, 'wind_speed_kt': s, 'wind_offset_deg': o}
                 for n, s, o in [('calm', 0, 0), ('head5', 5, 0), ('head10', 10, 0),
                                  ('head15', 15, 0), ('tail5', 5, 180),
                                  ('cross_left10', 10, -90), ('cross_right10', 10, 90)]]

def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False)

def sha(value):
    return hashlib.sha256(canonical(value).encode()).hexdigest()

def atomic_json(path: Path, value):
    temporary = path.with_name(path.name + '.tmp')
    temporary.write_text(json.dumps(value, indent=2, allow_nan=False) + '\n', encoding='utf-8', newline='\n')
    temporary.replace(path)

def validate_number(name, value, bounds):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise ValueError(f'{name} must be a finite number')
    if not bounds[1] <= value <= bounds[2]:
        raise ValueError(f'{name} outside [{bounds[1]}, {bounds[2]}]')

def resolve(path: Path):
    raw = json.loads(path.read_text(encoding='utf-8-sig'))
    unknown = set(raw) - {'schema_version', 'name', 'repeats', 'parameters', 'acceptance', 'cards', 'supervision'}
    if unknown or raw.get('schema_version') != SCHEMA_VERSION:
        raise ValueError(f'Invalid schema or unknown fields: {sorted(unknown)}')
    params = {k: v[0] for k, v in PARAMETERS.items()}
    overrides = raw.get('parameters', {})
    if set(overrides) - set(PARAMETERS):
        raise ValueError(f'Unknown native parameters: {set(overrides) - set(PARAMETERS)}')
    if set(overrides) & {'run_token', 'wind_speed_kt', 'wind_offset_deg'}:
        raise ValueError('Run token and wind are assigned from the card, not parameter overrides')
    params.update(overrides)
    for key, value in params.items():
        validate_number(key, value, PARAMETERS[key])
    if (params['flare_float_enabled'] not in (0,1) or params['flare_float_height_ft']>=params['flare_height_ft']
            or params['flare_float_contact_height_ft']>=params['flare_float_height_ft']
            or params['flare_float_sink_fps']<params['flare_contact_sink_fps']):
        raise ValueError('Invalid late-flare float correction')
    if params['capture_blend_start_deg']<=params['capture_blend_full_deg']:
        raise ValueError('Capture blend start must exceed its full-capture angle')
    if params['deceleration_start_height_ft']<=params['deceleration_end_height_ft'] or params['deceleration_end_height_ft']<params['flare_height_ft'] or params['landing_entry_kias']>params['final_kias']:
        raise ValueError('Invalid short-final deceleration interval')
    repeats = raw.get('repeats', 2)
    if type(repeats) is not int or not 1 <= repeats <= 20:
        raise ValueError('repeats must be an integer from 1 to 20')
    acceptance = {**ACCEPTANCE, **raw.get('acceptance', {})}
    if set(acceptance) != set(ACCEPTANCE):
        raise ValueError('Unknown acceptance fields')
    for key, value in acceptance.items():
        validate_number(key, value, (0, -5000 if key == 'short_final_start_ft' else 0, 10000))
    if acceptance['touchdown_min_ft'] >= acceptance['touchdown_max_ft']:
        raise ValueError('Touchdown interval is reversed')
    if acceptance['minimum_touchdown_kias'] >= acceptance['maximum_touchdown_kias']:
        raise ValueError('Touchdown speed interval is reversed')
    cards = raw.get('cards', DEFAULT_CARDS)
    if not isinstance(cards, list) or not cards:
        raise ValueError('At least one card is required')
    seen = set()
    for card in cards:
        if set(card) != {'name', 'wind_speed_kt', 'wind_offset_deg'}:
            raise ValueError('Each card requires exactly name, wind_speed_kt, wind_offset_deg')
        name = card['name']
        if not isinstance(name, str) or not name or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789_' for c in name) or name in seen:
            raise ValueError('Card names must be unique lowercase identifiers')
        seen.add(name)
        for key in ('wind_speed_kt', 'wind_offset_deg'):
            validate_number(key, card[key], PARAMETERS[key])
    supervision = {'poll_seconds': .5, 'wall_timeout_seconds': 600, 'disconnect_probe_seconds': 0,
                   **raw.get('supervision', {})}
    if set(supervision) != {'poll_seconds', 'wall_timeout_seconds', 'disconnect_probe_seconds'}:
        raise ValueError('Unknown supervision fields')
    validate_number('poll_seconds', supervision['poll_seconds'], (0, .1, 2))
    validate_number('wall_timeout_seconds', supervision['wall_timeout_seconds'], (0, 60, 1200))
    validate_number('disconnect_probe_seconds', supervision['disconnect_probe_seconds'], (0, 0, 20))
    return {'schema_version': 1, 'name': raw.get('name', path.stem), 'repeats': repeats,
            'adapter': 'torquesim_sr20_full_fuel_400lb_v3',
            'aircraft_path': 'Aircraft/X-Aviation/TorqueSim SR20/SR20.acf',
            'setup': {'pilot_kg': 90.718474, 'copilot_kg': 90.718474, 'rear_kg': 0, 'baggage_kg': 0,
                      'fuel': 'full_balanced', 'mass_reference':'measured_after_setup',
                      'loaded_mass_min_lb':2940, 'loaded_mass_max_lb':2952,
                      'start_along_ft': 14000, 'seed_terrain_offset_ft': 10,
                      'runway_elevation_ft': 171, 'day_of_year': 234, 'hour': 12,
                      'temperature_c': 15, 'pressure_hpa': 1013.25},
            'parameters': params, 'acceptance': acceptance, 'cards': cards, 'supervision': supervision}

def native_text(parameters):
    if set(parameters) != set(PARAMETERS):
        raise ValueError('Native configuration must be complete')
    return ''.join(f'{key}={float(parameters[key]):.17g}\n' for key in PARAMETERS)
