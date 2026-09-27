"""Summarize every available campaign trace without changing simulator state."""
import csv
import json
from collections import defaultdict
from pathlib import Path
import analyze

HERE=Path(__file__).resolve().parent
ROOT=Path('D:/X-Plane 12/Output/performance-tests')
ORDER=['A1','B1','C1','D1','D2','C2','B2']
LABELS={
    'A':'Flag absent / cache 5 GB', 'B':'Flag present / cache 5 GB',
    'C':'Flag present / cache 0 GB', 'D':'Flag absent / cache 0 GB'}

def build():
    runs=[]
    grouped=defaultdict(list)
    frames=defaultdict(list)
    for name in ORDER:
        path=ROOT/f'XPT_STUTTER_{name}_20260913'
        if not (path/'session.json').exists(): continue
        result=analyze.run(path)
        result['label']=name
        manifest=path/'source-manifest.json'
        if manifest.exists():
            hashes=json.loads(manifest.read_text())['harness_sha256']
            result['binary_hashes']={k:v for k,v in hashes.items() if k.endswith('.xpl')}
        log=path/'Log.txt'
        if log.exists():
            result['cache_log']=[line for line in log.read_text(errors='replace').splitlines() if 'TEX_cache' in line]
        for card in result['cards']:
            grouped[name[0]].append({**card,'run':name})
            with (path/'cards'/card['card']/'trace.csv').open(newline='') as f:
                frames[name[0]].extend(float(row['wall_frame_dt_s']) for row in csv.DictReader(f) if float(row['wall_frame_dt_s'])>0)
        runs.append(result)
    groups={}
    for key,cards in grouped.items():
        intervals=sorted(frames[key]); seconds=sum(intervals)
        groups[key]={
            'description':LABELS[key], 'flights':len(cards),
            'sessions':len({c['run'] for c in cards}),
            'observed_seconds':seconds, 'frames':len(intervals),
            'average_callback_hz':len(intervals)/seconds,
            'p99_ms':analyze.quantile(intervals,.99)*1000,
            'p999_ms':analyze.quantile(intervals,.999)*1000,
            'max_ms':max(intervals)*1000,
            'over_50_ms':sum(c['over_50_ms'] for c in cards),
            'over_100_ms':sum(c['over_100_ms'] for c in cards),
            'over_50_ms_per_minute':sum(c['over_50_ms'] for c in cards)*60/seconds,
            'passed':sum(c['landing_passed'] is True for c in cards),
            'valid':sum(c['measurement_valid'] is True for c in cards),
            'individual_flights':[{k:v for k,v in c.items() if k!='largest_frames'} for c in cards]}
    data={'order':ORDER,'runs':runs,'conditions':groups}
    (HERE/'results.json').write_text(json.dumps(data,indent=2)+'\n')
    fields=['run','card','observed_wall_seconds','frames','average_fps','p95_ms','p99_ms','p999_ms','max_ms','over_50_ms','over_100_ms','over_50_ms_per_minute','landing_passed','measurement_valid','native_step_gaps','dt_matches_wall_timestamp']
    with (HERE/'flights.csv').open('w',newline='') as f:
        writer=csv.DictWriter(f,fieldnames=fields,extrasaction='ignore');writer.writeheader()
        for key in 'ABCD':writer.writerows(grouped[key])
    print(json.dumps({k:{a:b for a,b in v.items() if a!='individual_flights'} for k,v in groups.items()},indent=2))
    return data

if __name__=='__main__':build()
