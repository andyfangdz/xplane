"""Strict configuration for the paused SR20 rendering benchmark."""
import json
import math
import re
from pathlib import Path

DEFAULT = dict(schema_version=1, scenario='parked_camera_sweep', airport='KCDW', runway='22',
               warmup_seconds=20, duration_seconds=40, amplitude_degrees=60, period_seconds=20,
               repeats=3, variants=[{'name':'recorder_off','record':False},
                                    {'name':'buffered','record':True}], wpr=True)

def resolve(path):
    supplied=json.loads(Path(path).read_text(encoding='utf-8-sig'))
    unknown=set(supplied)-set(DEFAULT)
    if unknown: raise ValueError(f'Unknown performance settings: {sorted(unknown)}')
    c={**DEFAULT, **supplied}
    if type(c['schema_version']) is not int or c['schema_version']!=1 or c['scenario']!='parked_camera_sweep':
        raise ValueError('Only schema 1 parked_camera_sweep is supported')
    for k,lo,hi in [('warmup_seconds',0,120),('duration_seconds',5,300),
                    ('amplitude_degrees',1,90),('period_seconds',5,120),('repeats',1,10)]:
        v=c[k]
        if type(v) not in (int,float) or not math.isfinite(v) or not lo<=v<=hi:
            raise ValueError(f'Invalid {k}')
    if type(c['repeats']) is not int or type(c['wpr']) is not bool:
        raise ValueError('repeats must be integer and wpr boolean')
    if c['duration_seconds']<c['period_seconds']:
        raise ValueError('A segment must contain at least one complete camera sweep')
    for key in ['airport','runway']:
        if not isinstance(c[key],str) or not re.fullmatch('[A-Z0-9]{1,8}',c[key]):
            raise ValueError(f'Invalid {key}')
    if (c['airport'],c['runway'])!=('KCDW','22'):
        raise ValueError('The current verified scene adapter supports KCDW runway 22 only')
    names=set()
    if not isinstance(c['variants'],list) or not 1<=len(c['variants'])<=2:
        raise ValueError('Provide one variant or an A/B pair')
    for v in c['variants']:
        if set(v)!= {'name','record'} or not isinstance(v['name'],str) or not re.fullmatch('[a-z][a-z0-9_]{0,30}',v['name']) or type(v['record']) is not bool or v['name'] in names:
            raise ValueError('Variants require unique simple names and a boolean record setting')
        names.add(v['name'])
    return c

def segments(c):
    """Alternate AB/BA; retain pair identity, including failed attempts."""
    for repeat in range(1,c['repeats']+1):
        order=c['variants'] if repeat%2 else list(reversed(c['variants']))
        for v in order:
            yield {**v,'repeat':repeat,'label':f'{repeat:02d}_{v["name"]}'}

def flight_request(c):
    return {'data':{
        'aircraft':{'path':'Aircraft/X-Aviation/TorqueSim SR20/SR20.acf'},
        'runway_start':{'airport_id':c['airport'],'runway':c['runway']},
        'engine_status':{'all_engines':{'running':False}},
        'ai_aircraft':[], 'local_time':{'day_of_year':150,'time_in_24_hours':14},
        'weather':{'definition':'vfr_few_clouds','vertical_speed_in_thermal_in_feet_per_minute':0,
                   'wave_height_in_meters':0,'wave_direction_in_degrees':0,'terrain_state':'dry',
                   'variation_across_region_percentage':0,'evolution_over_time_enum':'static'}}}
