from pathlib import Path
from collections import Counter, defaultdict
import bisect,json
H=Path(__file__).resolve().parent
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
summary=read(H/'trace-summary.json');analysis=read(H/'hitch-analysis.json')
symbols={k.lower():v for k,v in read(H/'resolved-symbols.json').items()}
queries={k:set(v) for k,v in read(H/'symbol-queries.json').items()};out=[]
public={'ntoskrnl.exe','dxgmms2.sys','dxgkrnl.sys','dxgi.dll','dxcore.dll','win32u.dll','ntdll.dll','kernelbase.dll','kernel32.dll','gdi32full.dll','win32kbase.sys','win32kfull.sys','d3d11.dll'}
for result in analysis[:2]:
    d=read(H/(result['case']+'.json'));w=result['main_waits'][0]
    a,b,tid=w['start_qpc'],w['end_qpc'],w['waking_tid']
    images=sorted({(i['address'],i['size'],i['file']) for i in d['images']});starts=[i[0] for i in images]
    def sym(ip):
        n=bisect.bisect_right(starts,ip)-1
        if n<0 or ip>=images[n][0]+images[n][1]:return hex(ip)
        base,size,file=images[n];rva=ip-base
        if Path(file).name.lower() in public:queries.setdefault(file,set()).add(rva)
        name=symbols.get(file.lower(),{}).get('names',{}).get(str(rva))
        return f'{Path(file).name}!{name}' if name else f'{Path(file).name}+{rva:#x}'
    samples=[e for e in d['events'] if e['name']=='PerfInfo/Sample' and e['scheduled_tid']==tid and a<=e['qpc']<=b]
    ticks={e['qpc'] for e in samples}
    stacks=Counter(tuple(sym(ip) for ip in s['ips']) for s in d['stacks'] if s['tid']==tid and s['event_qpc'] in ticks)
    pending=None;waits=[]
    for e in d['events']:
        if e['name']!='Thread/CSwitch':continue
        if e['fields']['NewThreadID']==tid and pending:
            overlap=max(0,min(b,e['qpc'])-max(a,pending['qpc']))
            if overlap:
                ready=[r for r in d['events'] if r['name']=='Dispatcher/ReadyThread' and r['fields']['AwakenedThreadID']==tid and pending['qpc']<=r['qpc']<=e['qpc']]
                wake=ready[-1] if ready else None
                actor=wake['scheduled_tid'] if wake else None
                waits.append(dict(begin=pending['qpc'],end=e['qpc'],overlap_ms=overlap/10000,full_wait_ms=(e['qpc']-pending['qpc'])/10000,
                    reason=pending['fields']['OldThreadWaitReason'],
                    waking_tid=actor,waking_pid=d['thread_pids'].get(str(actor)),ready_qpc=wake['qpc'] if wake else None,
                    ready_to_run_ms=(e['qpc']-wake['qpc'])/10000 if wake else None,
                    ready_stack=[[sym(ip) for ip in s['ips']] for s in d['stacks'] if wake and s['tid']==actor and s['event_qpc']==wake['qpc']],
                    stack=[[sym(ip) for ip in s['ips']] for s in d['stacks'] if s['tid']==tid and s['event_qpc']==e['qpc']]))
            pending=None
        if e['fields']['OldThreadID']==tid:pending=e
    r=dict(case=result['case'],thread=d['threads'][str(tid)],main_block_ms=w['full_wait_ms'],
        worker_samples=len(samples),worker_ips=Counter(sym(int(e['fields']['InstructionPointer'])) for e in samples).most_common(15),
        worker_sample_stacks=[dict(count=n,stack=list(s)) for s,n in stacks.most_common(12)],
        worker_off_cpu_ms=sum(x['overlap_ms'] for x in waits),worker_waits=sorted(waits,key=lambda x:x['overlap_ms'],reverse=True)[:6])
    out.append(r)
(H/'worker-analysis.json').write_text(json.dumps(out,indent=2)+'\n',encoding='utf-8')
(H/'symbol-queries.json').write_text(json.dumps({k:sorted(v) for k,v in queries.items()},indent=2)+'\n',encoding='utf-8')
for r in out:
    print(json.dumps({k:v for k,v in r.items() if k!='worker_waits'},indent=2))
