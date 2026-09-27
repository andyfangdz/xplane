"""Frame pacing summaries with explicit validity and separate timing domains."""
import csv
import json
import math
import re
from collections import Counter,defaultdict
from pathlib import Path
import statistics
from .config import atomic_json

def read_json(path,default=None):
    return json.loads(path.read_text(encoding='utf-8-sig')) if path.exists() else default

def stats(values):
    values=sorted(v for v in values if math.isfinite(v) and v>0)
    if not values:return None
    def quantile(q):
        i=(len(values)-1)*q;a=int(i);b=min(a+1,len(values)-1)
        return values[a]+(values[b]-values[a])*(i-a)
    return {'frames':len(values),'mean_ms':statistics.mean(values),'p50_ms':quantile(.5),
            'p95_ms':quantile(.95),'p99_ms':quantile(.99),'max_ms':values[-1],
            **{f'over_{n}ms':sum(v>n for v in values) for n in (25,50,100)}}

def present_windows(rows,metadata,pid):
    """Use integer QPC without float rounding; never mix different swap chains."""
    lo,hi=metadata['measure_start_qpc'],metadata['end_qpc']
    chosen=[]
    for row in rows:
        if int(row['ProcessID'])!=pid:continue
        key='CPUStartQPC' if 'CPUStartQPC' in row else 'QPCTime'
        t=int(row[key])
        if lo<=t<hi:chosen.append((t,row))
    counts=Counter(row.get('SwapChainAddress','unknown') for _,row in chosen)
    if not counts:return [],dict(counts)
    main=counts.most_common(1)[0][0]
    return sorted((t,row) for t,row in chosen if row.get('SwapChainAddress','unknown')==main),dict(counts)

def angular_span(values):
    """Unwrap a camera crossing 0/360 before checking its achieved sweep."""
    values=list(values)
    if not values:return 0
    unwrapped=[values[0]]
    for a,b in zip(values,values[1:]):unwrapped.append(unwrapped[-1]+(b-a+180)%360-180)
    return max(unwrapped)-min(unwrapped)

def assess(segment,metadata,native,pm,pid,wpr_required,wpr_stop,wpr_exists):
    reasons=[]
    if metadata.get('state')!=3 or metadata.get('invalid_flags') or metadata.get('overflow'):
        reasons.append('Native recording aborted or failed a validity gate')
    if metadata.get('frames_seen',0)<30 or metadata.get('measure_start_qpc',0)<=0:
        reasons.append('Insufficient native frame callbacks')
    duration=(metadata['end_qpc']-metadata['measure_start_qpc'])/metadata['qpc_frequency']
    if duration<segment.get('duration_seconds',5)*.95:
        reasons.append('Measured interval ended early')
    if metadata['record'] and len(native)!=metadata['frames_stored']:
        reasons.append('Native CSV row count mismatch')
    if metadata['record'] and metadata['frames_seen']!=metadata['frames_stored']:
        reasons.append('Native frames were not all retained')
    if metadata['record'] and angular_span(float(r['head_psi']) for r in native)<segment.get('amplitude_degrees',1)*1.8:
        reasons.append('Camera did not move')
    window,chains=present_windows(pm,metadata,pid)
    if len(window)<30:reasons.append('Insufficient presentation samples in the measured QPC window')
    ticks=metadata['qpc_frequency']
    if window and (window[-1][0]-window[0][0])/ticks<duration*.9:
        reasons.append('Presentation trace does not cover the measured interval')
    # QPC start-to-start intervals are valid on Vulkan without pretending that
    # approximate GPU latency fields measure actual scan-out or VR compositor work.
    intervals=[(b[0]-a[0])*1000/ticks for a,b in zip(window,window[1:])]
    if wpr_required and (not wpr_stop or not wpr_exists):reasons.append('Required system trace incomplete')
    loss=re.findall(r'(?:Dropped event|Events Lost)\s*:\s*(\d+)',(wpr_stop or {}).get('status',''),re.I)
    events_lost=max(map(int,loss)) if loss else None
    if wpr_required and events_lost!=0:reasons.append('System event loss is nonzero or unverified')
    hitches=[]
    for a,b in zip(window,window[1:]):
        ms=(b[0]-a[0])*1000/ticks
        if ms>25:hitches.append({'qpc_start':a[0],'qpc_end':b[0],
            'seconds_from_start':(a[0]-metadata['measure_start_qpc'])/ticks,'interval_ms':ms,
            'seconds_from_epoch_anchor':(a[0]-metadata['epoch_qpc'])/ticks})
    return {'segment':segment,'valid':not reasons,'reasons':reasons,'swapchains':chains,
            'native_callback_intervals':stats([float(r['wall_frame_dt_s'])*1000 for r in native]),
            'presentmon_cpu_start_intervals':stats(intervals),'hitches':hitches,
            'markers':metadata.get('markers',[]),'qpc_frequency':ticks,
            'wpr_requested':wpr_required,'wpr_event_loss':events_lost}

def build(directory):
    directory=Path(directory);session=read_json(directory/'session.json',{})
    c=read_json(directory/'resolved-config.json',read_json(directory/'requested-config.json',{}))
    pm=[]
    if (directory/'presentmon.csv').exists():
        with (directory/'presentmon.csv').open(newline='',encoding='utf-8-sig') as f:pm=list(csv.DictReader(f))
    results=[]
    for path in sorted((directory/'segments').glob('*')) if (directory/'segments').exists() else []:
        segment=read_json(path/'segment.json',{'label':path.name})
        try:
            metadata=read_json(path/'native.json')
            if not metadata:raise ValueError('Native metadata missing; inspect retained helper output and errors')
            with (path/'native.csv').open(newline='') as f:native=list(csv.DictReader(f))
            segment_pm=pm
            if (path/'presentmon.csv').is_file():
                with (path/'presentmon.csv').open(newline='',encoding='utf-8-sig') as f:segment_pm=list(csv.DictReader(f))
            result=assess(segment,metadata,native,segment_pm,session['simulator']['pid'],c.get('wpr',True),
                          read_json(path/'wpr-stop.json'),(path/'system.etl').is_file())
        except Exception as e:result={'segment':segment,'valid':False,'reasons':[str(e)]}
        if (path/'error.json').exists():result['valid']=False;result['reasons'].append(read_json(path/'error.json')['error'])
        atomic_json(path/'assessment.json',result);results.append(result)
    grouped=defaultdict(list);paired=defaultdict(dict)
    for r in results:
        if r['valid'] and r.get('presentmon_cpu_start_intervals'):
            s=r['segment'];v=r['presentmon_cpu_start_intervals'];grouped[s['name']].append(v)
            paired[s['repeat']][s['name']]=v
    summary={name:{'valid_segments':len(v),'median_mean_ms':statistics.median(i['mean_ms'] for i in v),
                   'median_p99_ms':statistics.median(i['p99_ms'] for i in v),
                   'over_50ms_total':sum(i['over_50ms'] for i in v)} for name,v in grouped.items()}
    names=[v['name'] for v in c.get('variants',[])]
    deltas=[]
    if len(names)==2:
        for repeat,pair in paired.items():
            if all(n in pair for n in names):deltas.append({'repeat':repeat,
                'mean_ms_second_minus_first':pair[names[1]]['mean_ms']-pair[names[0]]['mean_ms'],
                'p99_ms_second_minus_first':pair[names[1]]['p99_ms']-pair[names[0]]['p99_ms']})
    restoration=read_json(directory/'restoration.json',{})
    expected=c.get('repeats',3)*len(c.get('variants',[{},{}]))
    errors={p.name:read_json(p) for p in [directory/'worker-error.json',directory/'runner-error.json',directory/'presentmon-error.json'] if p.exists()}
    result={'segments':len(results),'expected_segments':expected,'valid_segments':sum(r['valid'] for r in results),
            'complete':len(results)==expected and all(r['valid'] for r in results) and restoration.get('restored',False) and not errors,
            'restored':restoration.get('restored',False),'variants':summary,'paired_deltas':deltas,'errors':errors}
    atomic_json(directory/'performance-report.json',result)
    lines=['# X-Plane performance recording','',
        f"Valid segments: {result['valid_segments']}/{expected}. Restoration: {result['restored']}. Complete: {result['complete']}.",'',
        'Paused SR20 camera sweep. Native callback intervals and PresentMon CPU-start intervals are separate measurements; neither proves monitor scan-out or VR compositor timing.', '',
        '| Segment | Valid | PresentMon p99 ms | Maximum ms | >50 ms |','|---|---|---:|---:|---:|']
    for r in results:
        s=r.get('presentmon_cpu_start_intervals') or {};label=r['segment']['label']
        lines.append(f"| [{label}](segments/{label}/assessment.json) | {r['valid']} | {s.get('p99_ms','—')} | {s.get('max_ms','—')} | {s.get('over_50ms','—')} |")
        if r['reasons']:lines.append('\n'+label+': '+'; '.join(r['reasons'])+'\n')
    lines+=['','## Paired comparison','',json.dumps(deltas,indent=2),'',
        'Positive deltas mean the second configured variant was slower. Small samples are descriptive, not proof of a causal improvement.', '',
        'Native JSON records absolute QPC windows and markers. Each assessment lists hitch QPC ranges for locating CPU, GPU, file I/O and disk activity in system.etl. WPR status is retained in wpr-stop.json; event loss must be checked before causal trace interpretation.', '',
        'Load timing is reported separately in load-timing.json. Cache capacity changes require fresh launches; these warm render segments do not establish cold-load performance. All errors remain in this directory.']
    (directory/'PERFORMANCE.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
    return result
