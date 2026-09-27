"""Summarize the focused 5/10/10/5 GB loading comparison, with no landing claims."""
import json
from pathlib import Path
from statistics import mean,median

HERE=Path(__file__).resolve().parent
ROOT=Path('D:/X-Plane 12/Output/performance-tests')
NAMES=['XPT_STUTTER_B2_20260913','XPT_CACHE_LOAD_L10a_20260913',
       'XPT_CACHE_LOAD_L10b_20260913','XPT_CACHE_LOAD_L5b_20260913']
rows=[]
for name in NAMES:
    path=ROOT/name
    if not (path/'load-benchmark.json').exists():continue
    result=json.loads((path/'load-benchmark.json').read_text())
    restoration=path/'restoration.json'
    result['run']=name
    result['restored']=json.loads(restoration.read_text(encoding='utf-8-sig')).get('restored') if restoration.exists() else False
    result['flight_traces']=len(list((path/'cards').glob('*/trace.csv')))
    error=path/'worker-error.json'
    result['stop_reason']=json.loads(error.read_text()).get('error') if error.exists() else None
    log=path/'Log.txt'
    result['cache_log']=[line for line in log.read_text(errors='replace').splitlines() if 'TEX_cache ready' in line] if log.exists() else []
    session=json.loads((path/'session.json').read_text(encoding='utf-8-sig'))
    result['settings']=session['stutter_test']
    result['arguments']=session['simulator']['arguments']
    rows.append(result)
groups=[]
for cache in [5,10]:
    values=[r['load_to_first_resume_seconds'] for r in rows if r['cache_gb']==cache]
    if values:groups.append({'cache_gb':cache,'n':len(values),'mean_seconds':mean(values),
        'median_seconds':median(values),'min_seconds':min(values),'max_seconds':max(values)})
data={'order_gb':[5,10,10,5],'flag':True,'rows':rows,'groups':groups}
(HERE/'cache-capacity-results.json').write_text(json.dumps(data,indent=2)+'\n')
print(json.dumps(groups,indent=2))
