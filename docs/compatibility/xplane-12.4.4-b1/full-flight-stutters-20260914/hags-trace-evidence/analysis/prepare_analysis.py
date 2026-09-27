from pathlib import Path
from collections import Counter
import csv, json

H = Path(__file__).resolve().parent
RUN = Path('D:/X-Plane 12/Output/performance-tests/XPT_HAGSOFF_WPR_20260915')
def read(p): return json.loads(p.read_text(encoding='utf-8-sig'))
def rows(p):
    with p.open(encoding='utf-8-sig', newline='') as f: return list(csv.DictReader(f))

assert read(RUN/'restoration.json')['restored']
report = read(RUN/'flight-pacing.json')
assert len(report['flights']) == 2 and all(f['valid'] for f in report['flights'])
session = read(RUN/'session.json')
seg = RUN/'segments/calm-02-diagnostic'
frequency = report['flights'][1]['qpc_frequency']
begin = read(seg/'wpr-start.json')['acknowledged_qpc'] + 10*frequency
end = read(seg/'stop-request.json')['qpc']
data = rows(RUN/'cards/calm-02/pacing.csv')
memory = rows(RUN/'gpu-memory/memory.csv')
intervals = [(int(a['qpc']), int(b['qpc'])) for a,b in zip(data,data[1:])
    if a['paused']==b['paused']=='0' and begin<=int(a['qpc'])<int(b['qpc'])<=end]
hitches = []
for a,b in intervals:
    if (b-a)/frequency <= .05: continue
    m = min(memory, key=lambda r: abs(int(r['receipt_qpc'])-b))
    hitches.append(dict(start_qpc=a,end_qpc=b,ms=(b-a)*1000/frequency,
        seconds_from_unpause=(a-report['flights'][1]['flight_start_qpc'])/frequency,
        nearest_memory=m,sample_offset_ms=(int(m['receipt_qpc'])-b)*1000/frequency))
cases=[]
for h in sorted(hitches, key=lambda r:r['ms'], reverse=True):
    if any(abs(h['start_qpc']-c['start_qpc']) < frequency/2 for c in cases): continue
    cases.append(dict(h, label=f'hitch-{len(cases)+1}'))
    if len(cases)==3: break
out=dict(run=str(RUN),pid=session['simulator']['pid'],frequency=frequency,
    begin_qpc=begin,end_qpc=end,exposure_seconds=sum(b-a for a,b in intervals)/frequency,
    max_ms=max(b-a for a,b in intervals)*1000/frequency,hitches=hitches,cases=cases,
    min_free_mib=min(float(r['free_mib']) for r in memory),
    report_complete=report['complete'],report_errors=report['errors'],
    flights=[{k:f[k] for k in ('card','valid','landing_passed','primary')} for f in report['flights']])
if (H/'coverage.json').exists():
    coverage=read(H/'coverage.json'); assert coverage['events_lost']==0
    for event in ('PerfInfo/Sample','Thread/CSwitch','Dispatcher/ReadyThread','StackWalk/Stack'):
        assert coverage['kernel'][event][1]<=begin<end<=coverage['kernel'][event][2], event
    out['kernel_coverage_verified']=True
if (seg/'presentmon.csv').exists():
    pm=rows(seg/'presentmon.csv')
    accepted=[r for r in pm if begin<=int(r['TimeInQPC'])<=end]
    out['present_modes']=dict(Counter(r['PresentMode'] for r in accepted))
    out['present_runtimes']=dict(Counter(r['PresentRuntime'] for r in accepted))
    out['present_rows']=len(accepted)
    for case in cases:
        case['nearby_presentations']=[r for r in pm
            if case['start_qpc']-frequency/2<=int(r['TimeInQPC'])<=case['end_qpc']+frequency/2]
(H/'trace-summary.json').write_text(json.dumps(out,indent=2)+'\n', encoding='utf-8')
print(json.dumps({k:v for k,v in out.items() if k not in ('hitches','cases','flights')},indent=2))
print(json.dumps([{k:v for k,v in c.items() if k not in ('nearby_presentations',)} for c in cases],indent=2))
