"""Offline audit of the paired flag-on, cache10 WPR/VRAM diagnostic runs."""
import bisect
import csv
import gzip
import hashlib
import json
from pathlib import Path
import re
import shutil

HERE=Path(__file__).resolve().parent
BUILD=HERE.parents[3]/'build/stutter-etl-analysis'
BASE=Path('D:/X-Plane 12/Output/performance-tests')
RUNS=[('XPT_FLAG_ON_WPR_VRAM_20260914',5,'flag-on-coverage.json'),
      ('XPT_FLAG_ON_TEX4_WPR_20260914',4,'tex4-coverage.json')]

def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def csvrows(p):return list(csv.DictReader(p.open(encoding='utf-8-sig',newline='')))

def main():
    runs=[];sources=[];evidence=[]
    for name,texture,coverage_name in RUNS:
        run=BASE/name;segment=run/'segments/calm-02-diagnostic'
        session=read(run/'session.json');report=read(run/'flight-pacing.json')
        restored=read(run/'restoration.json');start=read(segment/'wpr-start.json');stop=read(segment/'stop-request.json')
        assert report['complete'] and report['valid_flights']==2
        assert restored['restored'] and restored['source_hashes_match']
        assert session['stutter_test']=={'no_defragment':True,'texture_cache_gb':10}
        assert '--gfx_no_defragment' in session['simulator']['arguments']
        assert "'--gfx_no_defragment'" in (run/'Log.txt').read_text(encoding='utf-8-sig',errors='replace')
        assert session['rendering_profile']=='User2D' and session['keep_custom_scenery']
        prefs=(run/'after-preferences/X-Plane.prf').read_text(encoding='utf-8-sig')
        for key,value in [('_tex_res',texture),('_tex_cache_gb',10),('_eq_vr',0)]:
            assert int(re.search(r'^'+re.escape(key)+r'\s+(\d+)\s*$',prefs,re.M)[1])==value
        sources.append(read(run/'source-manifest.json')['harness_sha256'])
        memory=csvrows(run/'gpu-memory/memory.csv')
        assert read(run/'gpu-memory/finished.json')['collector_stopped']
        assert not (run/'gpu-memory/observer-error.json').exists()
        frequency=report['flights'][1]['qpc_frequency'];begin=start['acknowledged_qpc']+10*frequency;end=stop['qpc']
        coverage=read(BUILD/coverage_name)
        assert coverage['events_lost']==0
        for event in ['PerfInfo/Sample','Thread/CSwitch','StackWalk/Stack']:
            assert coverage['kernel'][event][1]<=begin<end<=coverage['kernel'][event][2],event
        pm=csvrows(segment/'presentmon.csv')
        assert len(pm)>100 and int(pm[0]['TimeInQPC'])<=begin and int(pm[-1]['TimeInQPC'])>=end
        gpu=[]
        for flight in report['flights']:
            m=[r for r in memory if flight['flight_start_qpc']<=int(r['receipt_qpc'])<=flight['flight_end_qpc']]
            gpu.append({'card':flight['card'],'samples':len(m),'total_mib':int(m[0]['total_mib']),
                'minimum_used_mib':min(int(r['used_mib']) for r in m),'maximum_used_mib':max(int(r['used_mib']) for r in m),
                'minimum_free_mib':min(int(r['free_mib']) for r in m),'maximum_free_mib':max(int(r['free_mib']) for r in m),
                'maximum_sample_gap_ms':max((int(b['receipt_qpc'])-int(a['receipt_qpc']))*1000/frequency for a,b in zip(m,m[1:]))})
        flight=report['flights'][1];rows=csvrows(run/'cards/calm-02/pacing.csv')
        intervals=[(int(a['qpc']),int(b['qpc'])) for a,b in zip(rows,rows[1:])
                   if a['paused']==b['paused']=='0' and int(a['qpc'])>=begin and int(b['qpc'])<=end]
        hitches=[]
        for a,b in intervals:
            ms=(b-a)*1000/frequency
            if ms<=50:continue
            nearest=min(memory,key=lambda r:abs(int(r['receipt_qpc'])-b))
            hitches.append({'qpc_start':a,'qpc_end':b,'interval_ms':ms,'seconds_from_unpause':(a-flight['flight_start_qpc'])/frequency,
                'nearest_memory_sample':nearest,'sample_offset_ms':(int(nearest['receipt_qpc'])-b)*1000/frequency})
        result={'name':name,'texture_resolution':texture,'settings':report['settings'],
                'frequency':frequency,'accepted_begin_qpc':begin,'accepted_end_qpc':end,
                'accepted_exposure_seconds':sum(b-a for a,b in intervals)/frequency,
                'accepted_max_ms':max(b-a for a,b in intervals)*1000/frequency,'accepted_over50':len(hitches),
                'hitches':hitches,'gpu':gpu,'flights':[dict(card=f['card'],valid=f['valid'],landing_passed=f['landing_passed'],
                    flight_start_qpc=f['flight_start_qpc'],flight_end_qpc=f['flight_end_qpc'],primary=f['primary']) for f in report['flights']],
                'coverage':coverage,'presentmon_rows':len(pm),'etl':str(segment/'system.etl'),
                'etl_bytes':(segment/'system.etl').stat().st_size,'etl_sha256':sha(segment/'system.etl')}
        runs.append(result)
        keep=['session.json','source-manifest.json','restoration.json','protected-state-verification.json','Log.txt','flight-pacing.json',
              'gpu-memory/collector.json','gpu-memory/finished.json','gpu-memory/memory.csv']
        keep += ['segments/calm-02-diagnostic/'+f for f in ['observer.json','wpr-start.json','stop-request.json','wpr-stop.json',
                 'presentmon.csv','presentmon-decode.json','presentmon-decode.log']]
        for card in ['calm-01','calm-02']:
            keep += [f'cards/{card}/{f}' for f in ['pacing.csv','pacing.json','pacing-assessment.json','pacing-readiness.json','assessment.json']]
        for relative in keep:
            path=run/relative
            if not path.exists():continue
            dest=HERE/'wpr-vram-evidence'/name/relative;dest.parent.mkdir(parents=True,exist_ok=True)
            if path.suffix=='.csv':dest=dest.with_suffix('.csv.gz');dest.write_bytes(gzip.compress(path.read_bytes(),mtime=0))
            else:shutil.copy2(path,dest)
            evidence.append({'source':str(path),'sha256':sha(path),'retained':str(dest.relative_to(HERE)),'retained_sha256':sha(dest)})
    assert sources[0]==sources[1], 'Harness changed between matched diagnostics'
    symbols=read(BUILD/'resolved-symbols.json');waits=[]
    cases=[('flag-on-early',0,76064,[6059059215017]),('flag-on-worst',0,76064,[6059148864172,6059150315727]),
           ('flag-on-late',0,76064,[6059237127990]),('tex4-hitches',1,12036,[6072938454085,6072945471980])]
    for label,run_index,main_tid,ends in cases:
        data=read(BUILD/f'{label}.json');images=sorted({(i['address'],i['size'],i['file']) for i in data['images']});addresses=[x[0] for x in images]
        def symbol(ip):
            n=bisect.bisect_right(addresses,ip)-1
            if n<0 or ip>=images[n][0]+images[n][1]:return {'address':ip}
            base,size,file=images[n];rva=ip-base
            return {'file':file,'rva':rva,'symbol':symbols.get(file,{}).get('names',{}).get(str(rva))}
        switches=[e for e in data['events'] if e['name']=='Thread/CSwitch']
        ins=[e for e in switches if e['fields']['NewThreadID']==main_tid]
        for end_qpc in ends:
            hitch=next(h for h in runs[run_index]['hitches'] if h['qpc_end']==end_qpc)
            pairs=[]
            for out in switches:
                if out['fields']['OldThreadID']!=main_tid or not hitch['qpc_start']<=out['qpc']<end_qpc:continue
                incoming=next((i for i in ins if out['qpc']<i['qpc']<=end_qpc),None)
                if incoming:pairs.append((incoming['qpc']-out['qpc'],out,incoming))
            ticks,out,incoming=max(pairs,key=lambda x:x[0])
            ready=next(e for e in data['events'] if e['name']=='Dispatcher/ReadyThread'
                       and e['fields']['AwakenedThreadID']==main_tid and out['qpc']<=e['qpc']<=incoming['qpc'])
            reason=out['fields']['OldThreadWaitReason'];wake_tid=ready['scheduled_tid'];wake_pid=data['thread_pids'][str(wake_tid)]
            suspended=[s for s in data['stacks'] if s['tid']==main_tid and s['event_qpc']==incoming['qpc']]
            waker=[s for s in data['stacks'] if s['tid']==wake_tid and s['event_qpc']==ready['qpc']]
            blocked_stacks=[[symbol(ip) for ip in s['ips']] for s in suspended]
            wake_stacks=[[symbol(ip) for ip in s['ips']] for s in waker]
            if reason=='28':
                assert wake_tid==52112 and wake_pid==4
                assert any(x.get('symbol')=='VIDMM_RECYCLE_HEAP_MGR::GetFullMDL' for s in blocked_stacks for x in s)
                assert any(x.get('symbol')=='VIDMM_SYSMEM_SEGMENT::CommitResource' for s in wake_stacks for x in s)
            else:
                assert reason=='37' and run_index==1 and wake_pid==data['target_pid']
                assert any(x.get('file','').lower().endswith('nvoglv64.dll') for s in blocked_stacks for x in s)
            waits.append({'run':runs[run_index]['name'],'hitch':hitch,'main_tid':main_tid,'thread_name':data['threads'][str(main_tid)]['name'],
                'wait_start_qpc':out['qpc'],'wait_end_qpc':incoming['qpc'],'wait_ms':ticks/10000,
                'ready_qpc':ready['qpc'],'ready_to_run_ms':(incoming['qpc']-ready['qpc'])/10000,
                'wait_reason':'WrPushLock' if reason=='28' else 'WrAlertByThreadId','waker_tid':wake_tid,'waker_process':data['processes'][str(wake_pid)],
                'blocked_stacks':blocked_stacks,'waker_stacks':wake_stacks})
    for name in ['EtlCoverageProbe.cs','EtlHitchProbe.cs','Run-Probe.ps1','Resolve-Symbols.ps1','resolved-symbols.json',
                 'symbol-image-verification.json','flag-on-coverage.json','tex4-coverage.json']:
        target=HERE/'wpr-vram-evidence/analysis'/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(BUILD/name,target)
    result={'same_harness_sources':True,'runs':runs,'traced_waits':waits,'evidence':evidence,
            'limits':['Separate diagnostic; WPR overhead is not calibrated and counts must not enter untraced A/B rates.',
                      'NVIDIA memory is board-wide at 500 ms receipt timestamps; no per-process budget, residency or eviction history.',
                      'Graphics backing-store preparation and present-path lock contention are established for five traced pauses; the particular allocation owner/content is not identified. Another lower-texture pause includes a user-mode NVIDIA driver synchronization wait.',
                      'Full 2D moving flights; no conclusion about VR presentation.']}
    (HERE/'wpr-vram-results.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    print(json.dumps({'runs':[{k:r[k] for k in ['name','accepted_over50','accepted_max_ms','accepted_exposure_seconds','gpu']} for r in runs],
                      'waits_ms':[w['wait_ms'] for w in waits]},indent=2))

if __name__=='__main__':main()
