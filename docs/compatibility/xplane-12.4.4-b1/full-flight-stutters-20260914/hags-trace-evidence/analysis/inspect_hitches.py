"""Offline thread scheduling and symbol queries; never reuse pre-update symbols."""
from pathlib import Path
from collections import Counter, defaultdict
import bisect, json

H=Path(__file__).resolve().parent
def read(p): return json.loads(p.read_text(encoding='utf-8-sig'))
summary=read(H/'trace-summary.json'); freq=summary['frequency']
symbols=read(H/'resolved-symbols.json') if (H/'resolved-symbols.json').exists() else {}
symbols={k.lower():v for k,v in symbols.items()}; queries=defaultdict(set); results=[]
public={'ntoskrnl.exe','dxgmms2.sys','dxgkrnl.sys','dxgi.dll','win32u.dll','ntdll.dll',
    'kernelbase.dll','kernel32.dll','gdi32full.dll','win32kbase.sys','win32kfull.sys','d3d11.dll'}

for case in summary['cases']:
    d=read(H/(case['label']+'.json')); assert d['events_lost']==0
    a,b=case['start_qpc'],case['end_qpc']
    main=[int(k) for k,v in d['threads'].items() if v['name']=='X-Plane-11']
    assert len(main)==1, main
    tid=main[0]
    images=sorted({(i['address'],i['size'],i['file']) for i in d['images']})
    starts=[i[0] for i in images]
    def sym(ip):
        n=bisect.bisect_right(starts,ip)-1
        if n<0 or ip>=images[n][0]+images[n][1]: return hex(ip)
        base,size,file=images[n]; rva=ip-base
        name=symbols.get(file.lower(),{}).get('names',{}).get(str(rva))
        if Path(file).name.lower() in public: queries[file].add(rva)
        return f'{Path(file).name}!{name}' if name else f'{Path(file).name}+{rva:#x}'
    stack_index=defaultdict(list)
    for s in d['stacks']: stack_index[(s['tid'],s['event_qpc'])].append(s['ips'])
    def stack(thread,tick): return [[sym(ip) for ip in ips] for ips in stack_index[(thread,tick)]]
    pending={}; waits=[]
    ready=[e for e in d['events'] if e['name']=='Dispatcher/ReadyThread']
    for e in d['events']:
        if e['name']!='Thread/CSwitch':continue
        incoming=e['fields']['NewThreadID']; old=e['fields']['OldThreadID']
        if incoming in pending and str(incoming) in d['threads']:
            start=pending[incoming]
            overlap=max(0,min(b,e['qpc'])-max(a,start['qpc']))
            if overlap and (incoming==tid or overlap>=freq*.02):
                wakes=[r for r in ready if r['fields']['AwakenedThreadID']==incoming
                    and start['qpc']<=r['qpc']<=e['qpc']]
                wake=wakes[-1] if wakes else None
                actor=wake['scheduled_tid'] if wake else None
                waits.append(dict(tid=incoming,name=d['threads'][str(incoming)]['name'],
                    start_qpc=start['qpc'],end_qpc=e['qpc'],
                    full_wait_ms=(e['qpc']-start['qpc'])*1000/freq,
                    overlap_ms=overlap*1000/freq,fully_inside=a<=start['qpc']<e['qpc']<=b,
                    reason=start['fields']['OldThreadWaitReason'],
                    state=start['fields'].get('OldThreadState'),
                    ready_qpc=wake['qpc'] if wake else None,
                    ready_to_run_ms=(e['qpc']-wake['qpc'])*1000/freq if wake else None,
                    waking_tid=actor,waking_pid=d['thread_pids'].get(str(actor)),
                    switch_out_stack=stack(incoming,start['qpc']),
                    switch_in_stack=stack(incoming,e['qpc']),
                    ready_stack=stack(actor,wake['qpc']) if wake else []))
        pending[old]=e
    main_waits=sorted([w for w in waits if w['tid']==tid],key=lambda w:w['overlap_ms'],reverse=True)
    samples=[e for e in d['events'] if e['name']=='PerfInfo/Sample' and a<=e['qpc']<=b]
    main_samples=[e for e in samples if e['scheduled_tid']==tid]
    main_ips=Counter(sym(int(e['fields']['InstructionPointer'])) for e in main_samples)
    sample_stacks=[]
    for sample in main_samples:
        sample_stacks.extend(stack(tid,sample['qpc']))
    counted=Counter(tuple(s) for s in sample_stacks)
    other_stacks=defaultdict(Counter)
    memory_stacks=defaultdict(Counter)
    for s in d['stacks']:
        if not a<=s['event_qpc']<=b or s['tid']==tid:continue
        names=tuple(sym(ip) for ip in s['ips'])
        if s['pid']==4:other_stacks[s['tid']][names]+=1
        if any(any(key in x for key in ('PageInAllocation','PageInOneAllocation','GetFullMDL',
             'LockAllocationBackingStore','LockHeapAllocation','MmProbeAndLockPages')) for x in names):
            memory_stacks[s['tid']][names]+=1
    result=dict(case=case['label'],hitch_ms=case['ms'],main_tid=tid,
        main_off_cpu_overlap_ms=sum(w['overlap_ms'] for w in main_waits),
        main_waits=main_waits[:20],other_long_waits=sorted([w for w in waits if w['tid']!=tid],
            key=lambda w:w['overlap_ms'],reverse=True)[:15],
        main_cpu_samples=len(main_samples),all_cpu_samples=len(samples),main_sample_ips=main_ips.most_common(20),
        main_sample_stacks=[dict(count=n,stack=list(s)) for s,n in counted.most_common(10)],
        system_stacks={t:[dict(count=n,stack=list(s)) for s,n in c.most_common(6)] for t,c in other_stacks.items()},
        memory_stacks={t:[dict(count=n,stack=list(s)) for s,n in c.most_common(6)] for t,c in memory_stacks.items()},
        faults=[e for e in d['events'] if e['name']=='Memory/HardFault' and a<=e['qpc']<=b],
        file_stats=d['file_stats'])
    results.append(result)
(H/'symbol-queries.json').write_text(json.dumps({k:sorted(v) for k,v in queries.items()},indent=2)+'\n',encoding='utf-8')
(H/'hitch-analysis.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
for r in results:
    print(json.dumps({k:r[k] for k in ('case','hitch_ms','main_tid','main_off_cpu_overlap_ms','main_cpu_samples','main_sample_ips')},indent=2))
    for w in r['main_waits'][:2]:
        print(json.dumps({k:v for k,v in w.items() if k not in ('switch_out_stack',)},indent=2))
