"""Read-only live and final wall-frame timing analysis for the paired campaign."""
import argparse
from collections import Counter
import csv
import json
import math
from pathlib import Path

def quantile(a, p):
    k = (len(a)-1)*p
    lo, hi = math.floor(k), math.ceil(k)
    return a[lo] + (a[hi]-a[lo])*(k-lo)

def analyze(card):
    with (card/'trace.csv').open(newline='') as f:
        rows = list(csv.DictReader(f))
    # The first unpaused observation deliberately has no prior wall timestamp.
    good = [r for r in rows if float(r.get('wall_frame_dt_s', 0))>0]
    if not good:
        return {'card':card.name,'frames':0}
    dt = [float(r['wall_frame_dt_s']) for r in good]
    ordered = sorted(dt)
    seconds = sum(dt)
    assessment = json.loads((card/'assessment.json').read_text()) if (card/'assessment.json').exists() else None
    result = dict(card=card.name, frames=len(good), observed_wall_seconds=seconds,
        average_fps=len(dt)/seconds, median_ms=quantile(ordered,.5)*1000,
        p95_ms=quantile(ordered,.95)*1000, p99_ms=quantile(ordered,.99)*1000,
        p999_ms=quantile(ordered,.999)*1000, max_ms=max(dt)*1000,
        sim_steps_above_50ms=sum(float(r['attitude_frame_dt_s'])>.05 for r in good),
        simulator_step_max_ms=max(float(r['attitude_frame_dt_s']) for r in good)*1000,
        landing_passed=assessment['passed'] if assessment else None,
        measurement_valid=assessment['measurement_valid'] if assessment else None,
        landing_reasons=assessment['reasons'] if assessment else [],
        dt_matches_wall_timestamp=True)
    for ms in [25, 1000/30, 50, 100, 250, 500]:
        key = str(round(ms,3))
        hit = [v>ms/1000 for v in dt]
        result['over_'+key+'_ms'] = sum(hit)
        result['events_over_'+key+'_ms'] = sum(v and (i==0 or not hit[i-1]) for i,v in enumerate(hit))
    result['over_50_ms_per_minute'] = result['over_50_ms']*60/seconds
    result['excess_seconds_above_50_ms'] = sum(max(v-.05,0) for v in dt)
    for previous, current in zip(rows,rows[1:]):
        if float(current['wall_frame_dt_s'])>0:
            delta = float(current['wall_elapsed_s'])-float(previous['wall_elapsed_s'])
            if abs(delta-float(current['wall_frame_dt_s']))>3e-8:
                result['dt_matches_wall_timestamp']=False
    # X-Plane's global counter advances by two per callback on this build.
    # Preserve its actual increments; use our native step counter for continuity.
    result['sdk_counter_increments'] = dict(Counter(int(b['flight_loop_counter'])-int(a['flight_loop_counter']) for a,b in zip(rows,rows[1:])))
    result['native_step_gaps'] = sum(float(b['native_steps'])!=float(a['native_steps'])+1 for a,b in zip(rows,rows[1:]))
    result['zero_dt_samples'] = len(rows)-len(good)
    result['largest_frames'] = [{k:r.get(k) for k in ['sim_time','phase_id','wall_elapsed_s','wall_frame_dt_s','attitude_frame_dt_s','render_frame_period_s','gpu_frame_time_s_approx','flight_loop_counter']} for r in sorted(good,key=lambda r:float(r['wall_frame_dt_s']),reverse=True)[:10]]
    return result

def run(path):
    session=json.loads((path/'session.json').read_text(encoding='utf-8-sig'))
    restoration=json.loads((path/'restoration.json').read_text(encoding='utf-8-sig')) if (path/'restoration.json').exists() else {}
    output={'run':path.name,'settings':session.get('stutter_test'),
        'kind':'planned_load_only_stop' if (path/'load-benchmark.json').exists() else 'flights',
        'arguments':(session.get('simulator') or {}).get('arguments'),
        'restored':restoration.get('restored',False),
        'cards':[analyze(p.parent) for p in sorted((path/'cards').glob('*/trace.csv'))]}
    return output

if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('run')
    parser.add_argument('--output')
    args=parser.parse_args()
    data=run(Path(args.run))
    if args.output: Path(args.output).write_text(json.dumps(data,indent=2)+'\n')
    compact={**data,'cards':[{k:v for k,v in c.items() if k!='largest_frames'} for c in data['cards']]}
    print(json.dumps(compact,indent=2))
