"""Full-flight passive timing. Guidance remains in the existing native controller."""
import argparse
import csv
import json
import math
from pathlib import Path
import shutil
import statistics
import time
from .config import atomic_json
from .perf_report import read_json, stats


class FlightCapture:
    def __init__(self,api,card,root,token,status):
        self.api=api;self.card=Path(card);self.root=Path(root);self.token=int(token);self.status=status
        self.native=self.root/'Aircraft/X-Aviation/TorqueSim SR20/plugins/XPTPerformanceRecorder'
        self.started=False

    def start(self):
        from .readiness import check_startup
        self.api.refresh_catalogs()
        self.api.require_datarefs(['xpt/perf/snapshot'])
        self.status.update('waiting_for_flight_recording_focus')
        deadline=time.monotonic()+180
        while True:
            readiness=check_startup(self.root)
            if (self.card.parents[1]/'cancel.request').exists():raise RuntimeError('Cancelled before recording')
            s=self.api.get_raw('xpt/perf/snapshot')
            if len(s)==16 and s[6]==1 and s[7]==0 and s[8]==1:break
            if time.monotonic()>deadline:raise TimeoutError('Paused foreground 2D flight-recording gate not reached')
            time.sleep(1)
        self.api.command('sim/view/3d_cockpit_cmnd_look',.2)
        for ref in ['sim/graphics/view/pilots_head_psi','sim/graphics/view/pilots_head_the']:
            self.api.set_dataref(ref,0)
        atomic_json(self.card/'pacing-readiness.json',{'log':readiness,'snapshot':s,'fixed_head_degrees':[0,0]})
        # Native buffer allocation and all setup occur before guidance starts.
        config=f'token={self.token}\nwarmup_seconds=0\nduration_seconds=300\nrecord=1\nsweep=0\nexpected_vr=0\nrequire_foreground=1\n'
        (self.native/'performance.ini').write_text(config,encoding='ascii')
        (self.card/'pacing-config.ini').write_text(config,encoding='ascii')
        self.api.command('xpt/perf/configure',.2)
        s=self.api.get_raw('xpt/perf/snapshot')
        if s[0]!=0:raise RuntimeError('Passive recorder configuration was rejected')
        self.started=True
        self.api.command('xpt/perf/start',.2)
        s=self.api.get_raw('xpt/perf/snapshot')
        if s[0] not in (1,2) or s[3]!=self.token or s[4]!=0:raise RuntimeError('Passive flight recording did not start')
        atomic_json(self.card/'pacing-start.json',{'snapshot':s,'method':'passive after-physics QPC; no camera sweep; no system trace or video'})

    def stop(self):
        if self.started:self.api.command('xpt/perf/stop',.2)
        for extension in ('csv','json'):
            source=self.native/f'performance-{self.token}.{extension}'
            if source.exists():shutil.copy2(source,self.card/f'pacing.{extension}')


def interval_summary(intervals):
    values=[r['interval_ms'] for r in intervals]
    s=stats(values)
    if not s:return None
    ordered=sorted(values);i=(len(ordered)-1)*.999;a=int(i);b=min(a+1,len(ordered)-1)
    s['p999_ms']=ordered[a]+(ordered[b]-ordered[a])*(i-a)
    s['worst_one_percent_mean_ms']=statistics.mean(ordered[-max(1,math.ceil(len(ordered)*.01)):])
    s['exposure_seconds']=sum(values)/1000
    for threshold in (25,50,100):
        s[f'over_{threshold}ms_per_minute']=s[f'over_{threshold}ms']*60/s['exposure_seconds']
    spikes=0
    for i,v in enumerate(values):
        neighbors=values[max(0,i-60):i]+values[i+1:i+61]
        if len(neighbors)<30:continue
        median=statistics.median(neighbors)
        if v>25 and v>2*median and v-median>=16:spikes+=1
    s['local_spikes']=spikes
    s['local_spikes_per_minute']=spikes*60/s['exposure_seconds']
    return s


def assess_flight(metadata,raw,landing,minimum_seconds=140):
    reasons=[]
    if metadata.get('state')!=3 or metadata.get('invalid_flags') or metadata.get('overflow'):
        reasons.append('Passive recorder integrity/focus/VR gate failed')
    if metadata.get('sweep') or not metadata.get('record'):reasons.append('Recorder did not remain passive')
    if len(raw)!=metadata.get('frames_seen') or len(raw)!=metadata.get('frames_stored'):
        reasons.append('Passive buffer row count mismatch')
    if not landing.get('measurement_valid'):reasons.append('Flight control/measurement validity failed')
    freq=metadata.get('qpc_frequency',0)
    if freq<=0:return {'valid':False,'reasons':reasons+['Invalid QPC clock']}
    if not raw:return {'valid':False,'reasons':reasons+['No passive frame rows']}
    ticks=[int(r['qpc']) for r in raw]
    if any(b<=a for a,b in zip(ticks,ticks[1:])):reasons.append('Nonconsecutive QPC timeline')
    moving=[i for i,r in enumerate(raw) if int(r['paused'])==0]
    if not moving:return {'valid':False,'reasons':reasons+['No unpaused flight window']}
    if moving!=list(range(moving[0],moving[-1]+1)):reasons.append('Flight was interrupted by a pause')
    if moving[0]==0 or moving[-1]==len(raw)-1:reasons.append('Missing paused boundary around complete flight')
    first=ticks[moving[0]];last=ticks[moving[-1]]
    if (last-first)/freq<minimum_seconds:reasons.append('Full-flight exposure below predeclared minimum')
    if any(int(raw[i]['vr'])!=0 or int(raw[i]['foreground'])!=1 for i in moving):
        reasons.append('Flight lost foreground or actual 2D mode')
    intervals=[]
    for ai,bi in zip(moving,moving[1:]):
        if bi!=ai+1:continue
        dt=(ticks[bi]-ticks[ai])/freq
        if abs(dt-float(raw[bi]['wall_frame_dt_s']))>max(2/freq,1e-8):
            reasons.append('Native interval differs from QPC delta');break
        intervals.append({'qpc_start':ticks[ai],'qpc_end':ticks[bi],
                          'seconds_from_unpause':(ticks[ai]-first)/freq,'interval_ms':dt*1000})
    activation=[r for r in intervals if r['seconds_from_unpause']<5]
    primary=[r for r in intervals if r['seconds_from_unpause']>=5]
    return {'valid':not reasons,'reasons':reasons,'landing_passed':landing.get('passed'),
            'timing_domain':'native after-physics callback intervals, not display presentation',
            'flight_start_qpc':first,'flight_end_qpc':last,'qpc_frequency':freq,
            'flight_seconds':(last-first)/freq,'full_flight':interval_summary(intervals),
            'activation_first_5_seconds':interval_summary(activation),'primary':interval_summary(primary),
            'first_unpaused_callback_ms':float(raw[moving[0]]['wall_frame_dt_s'])*1000,
            'hitches':[dict(r,activation=r['seconds_from_unpause']<5) for r in intervals if r['interval_ms']>25]}


def build_run(directory):
    directory=Path(directory);session=read_json(directory/'session.json',{})
    config=read_json(directory/'requested-config.json',{});flights=[]
    for card in sorted((directory/'cards').glob('*')):
        m=read_json(card/'pacing.json',{})
        try:
            with (card/'pacing.csv').open(encoding='utf-8',newline='') as f:raw=list(csv.DictReader(f))
            result=assess_flight(m,raw,read_json(card/'assessment.json',{}))
        except (OSError,KeyError,ValueError) as error:result={'valid':False,'reasons':[str(error)]}
        if (card/'pacing-error.json').exists():
            result['valid']=False;result['reasons'].append(read_json(card/'pacing-error.json')['error'])
        result['card']=card.name;atomic_json(card/'pacing-assessment.json',result);flights.append(result)
    expected=config.get('repeats',1)*len(config.get('cards',[]))
    errors={name:read_json(directory/name) for name in ['worker-error.json','runner-error.json','recovery-error.json'] if (directory/name).exists()}
    restored=read_json(directory/'restoration.json',{}).get('restored',False)
    result={'run':directory.name,'settings':session.get('stutter_test'), 'expected_flights':expected,
            'valid_flights':sum(r['valid'] for r in flights),'flights':flights,'restored':restored,'errors':errors,
            'complete':bool(expected) and len(flights)==expected and all(r['valid'] for r in flights) and restored and not errors}
    atomic_json(directory/'flight-pacing.json',result)
    return result


def build_matrix(directory):
    directory=Path(directory);manifest=read_json(directory/'matrix.json');attempts=[];all_flights=[];issues=[];sources=[]
    for a in manifest['attempts']:
        report=read_json(directory.parent/a['run']/'flight-pacing.json',{})
        valid=report.get('complete',False) and a.get('exit_code')==0
        if valid:
            if report.get('settings')!={'no_defragment':a['flag'],'texture_cache_gb':10}:
                issues.append(f"{a['run']}: flag/cache settings mismatch")
            source=read_json(directory.parent/a['run']/'source-manifest.json',{}).get('harness_sha256')
            if not source:issues.append(f"{a['run']}: source manifest missing")
            else:sources.append(source)
        attempts.append({**a,'valid':valid,'report':report})
        if valid:
            for flight in report['flights']:all_flights.append({**flight,'flag':a['flag'],'run':a['run'],'pair':a['pair']})
    groups={}
    for flag in (False,True):
        rows=[r for r in all_flights if r['flag']==flag];exposure=sum(r['primary']['exposure_seconds'] for r in rows)
        groups[str(flag).lower()]={'flights':len(rows),'fresh_processes':len({r['run'] for r in rows}),
            'exposure_seconds':exposure,'flights_with_over_50ms':sum(r['primary']['over_50ms']>0 for r in rows),
            'processes_with_over_50ms':len({r['run'] for r in rows if r['primary']['over_50ms']>0})}
        for threshold in (25,50,100):
            count=sum(r['primary'][f'over_{threshold}ms'] for r in rows)
            groups[str(flag).lower()].update({f'over_{threshold}ms':count,f'over_{threshold}ms_per_minute':count*60/exposure if exposure else None})
    baseline=groups['false'];reproduced=baseline['flights_with_over_50ms']>=2 and baseline['processes_with_over_50ms']>=2
    if sources and any(s!=sources[0] for s in sources[1:]):issues.append('Sources differ between fresh launches')
    paired=[]
    for pair in sorted({r['pair'] for r in all_flights}):
        sides={flag:[r for r in all_flights if r['pair']==pair and r['flag']==flag] for flag in (False,True)}
        if not all(sides.values()):continue
        row={'pair':pair}
        for threshold in (25,50,100):
            rates={flag:sum(r['primary'][f'over_{threshold}ms'] for r in side)*60/sum(r['primary']['exposure_seconds'] for r in side) for flag,side in sides.items()}
            row[f'over_{threshold}ms_per_minute_on_minus_off']=rates[True]-rates[False]
        paired.append(row)
    complete=len(attempts)==manifest['expected_runs'] and all(a['valid'] for a in attempts) and not issues
    result={'complete':complete,'baseline_reproduced':reproduced,'groups':groups,'attempts':attempts,'paired_differences':paired,'comparison_issues':issues,
            'conclusion':'Full-flight comparison complete; inspect paired rates and retained per-flight variation.' if complete and reproduced else 'Inconclusive: baseline stutter reproduction or planned valid exposure is missing.'}
    atomic_json(directory/'comparison.json',result)
    lines=['# Full-flight stutter comparison','',result['conclusion'],'',
           'Primary window starts five seconds after the first unpaused callback. Activation and full-flight results are retained separately. These are native callback intervals, not display-present or VR-compositor timings.','',
           '| Run / flight | Flag | Valid | Seconds | >25 ms | >50 ms | >100 ms | p99.9 ms | Max ms |',
           '|---|---|---|---:|---:|---:|---:|---:|---:|']
    for a in attempts:
        if not a['report'].get('flights'):lines.append(f"| {a['run']} | {a['flag']} | False | | | | | | |")
        for r in a['report'].get('flights',[]):
            s=r.get('primary') or {};lines.append(f"| {a['run']} / {r['card']} | {a['flag']} | {a['valid'] and r['valid']} | {s.get('exposure_seconds','')} | {s.get('over_25ms','')} | {s.get('over_50ms','')} | {s.get('over_100ms','')} | {s.get('p999_ms','')} | {s.get('max_ms','')} |")
    (directory/'COMPARISON.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
    return result


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--run',type=Path,required=True);parser.add_argument('--matrix',action='store_true')
    args=parser.parse_args();r=build_matrix(args.run) if args.matrix else build_run(args.run)
    print(json.dumps({k:r[k] for k in ('complete','baseline_reproduced','groups','conclusion') if k in r}))
    if not r['complete']:raise SystemExit(1)
