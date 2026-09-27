"""Offline summary; requires each simulator transaction to be restored first."""
import csv
import json
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
BASE = Path('D:/X-Plane 12/Output/performance-tests')
NAMES = ['XPT_DXGI_B1_20260914','XPT_DXGI_TRACE_20260914']
def read(p): return json.loads(p.read_text(encoding='utf-8-sig'))
def rows(p):
    with p.open(encoding='utf-8-sig', newline='') as f:
        return list(csv.DictReader(f))

out=[]
for name in NAMES:
    run=BASE/name
    assert read(run/'restoration.json')['restored']
    report=read(run/'flight-pacing.json')
    session=read(run/'session.json')
    before=read(HERE/f'{name}-nv-before.json')
    applied=read(HERE/f'{name}-nv-applied.json')
    restored=read(HERE/f'{name}-nv-restored.json')
    assert before['setting']==restored['setting']
    assert before['profile_settings']==restored['profile_settings']
    assert applied['setting']['current']==1 and applied['setting']['location']==0
    assert session['stutter_test']=={'no_defragment':True,'texture_cache_gb':10}
    assert session['rendering_preferences']['_tex_res']==5
    result=dict(name=name,pid=session['simulator']['pid'],report_complete=report['complete'],
        report_errors=report['errors'],restored=True,nvidia_restored=True,
        source_hashes=read(run/'source-manifest.json')['harness_sha256'],
        flights=[{k:f[k] for k in ('card','valid','landing_passed','primary','qpc_frequency','flight_start_qpc','flight_end_qpc')} for f in report['flights']])
    seg=run/'segments/calm-02-diagnostic'
    if seg.is_dir():
        freq=report['flights'][1]['qpc_frequency']
        begin=read(seg/'wpr-start.json')['acknowledged_qpc']+10*freq
        end=read(seg/'stop-request.json')['qpc']
        data=rows(run/'cards/calm-02/pacing.csv')
        memory=rows(run/'gpu-memory/memory.csv')
        intervals=[(int(a['qpc']),int(b['qpc'])) for a,b in zip(data,data[1:])
            if a['paused']==b['paused']=='0' and begin<=int(a['qpc'])<int(b['qpc'])<=end]
        hitches=[]
        for a,b in intervals:
            if (b-a)/freq<=.05:continue
            m=min(memory,key=lambda r:abs(int(r['receipt_qpc'])-b))
            hitches.append(dict(start_qpc=a,end_qpc=b,ms=(b-a)*1000/freq,nearest_memory=m,
                               sample_offset_ms=(int(m['receipt_qpc'])-b)*1000/freq))
        result['trace']=dict(begin_qpc=begin,end_qpc=end,frequency=freq,exposure_seconds=sum(b-a for a,b in intervals)/freq,
                             max_ms=max(b-a for a,b in intervals)*1000/freq,hitches=hitches)
        if (seg/'presentmon.csv').exists():
            pm=rows(seg/'presentmon.csv')
            accepted=[r for r in pm if begin<=int(r['TimeInQPC'])<=end]
            result['trace']['present_modes']=dict(Counter(r['PresentMode'] for r in accepted))
            result['trace']['runtime']=dict(Counter(r['PresentRuntime'] for r in accepted))
        coverage=read(HERE/'dxgi-coverage.json')
        assert coverage['events_lost']==0
        for event in ('PerfInfo/Sample','Thread/CSwitch','StackWalk/Stack'):
            assert coverage['kernel'][event][1]<=begin<end<=coverage['kernel'][event][2]
        result['trace']['coverage_verified']=True
    out.append(result)
(HERE/'presentation-results.json').write_text(json.dumps(out,indent=2)+'\n',encoding='utf-8')
for run in out:
    print(json.dumps({k:v for k,v in run.items() if k not in ('source_hashes',)},indent=2))
