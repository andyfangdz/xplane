import bisect
import json
from pathlib import Path

HERE=Path(__file__).resolve().parent
OLD=HERE.parent/'stutter-etl-analysis'
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
symbols=read(OLD/'resolved-symbols.json')
if (HERE/'resolved-symbols.json').exists():
    for f,v in read(HERE/'resolved-symbols.json').items():
        if f not in symbols:symbols[f]=v
        else:symbols[f]['names'].update(v.get('names',{}))
symbols={f.lower():v for f,v in symbols.items()}
queries={}
results=[]
cases=[('dxgi-early',6105653814431,6105655148597),('dxgi-worst',6105766646143,6105768538783)]
for label,begin,end in cases:
    d=read(HERE/f'{label}.json')
    tid=next(int(t) for t,v in d['threads'].items() if v['name']=='X-Plane-11')
    images=sorted({(i['address'],i['size'],i['file']) for i in d['images']})
    starts=[i[0] for i in images]
    def symbol(ip):
        n=bisect.bisect_right(starts,ip)-1
        if n<0 or ip>=images[n][0]+images[n][1]:return dict(address=ip)
        base,size,file=images[n];rva=ip-base
        name=symbols.get(file.lower(),{}).get('names',{}).get(str(rva))
        if not name and Path(file).name.lower() in ('ntoskrnl.exe','dxgmms2.sys','dxgkrnl.sys','dxgi.dll','win32u.dll','ntdll.dll','kernelbase.dll','gdi32full.dll','win32kbase.sys','win32kfull.sys'):
            queries.setdefault(file,set()).add(rva)
        return dict(file=file,rva=rva,symbol=name)
    switches=[e for e in d['events'] if e['name']=='Thread/CSwitch']
    ins=[e for e in switches if e['fields']['NewThreadID']==tid]
    pairs=[]
    for ev in switches:
        if ev['fields']['OldThreadID']!=tid or not begin<=ev['qpc']<end:continue
        incoming=next((i for i in ins if ev['qpc']<i['qpc']<=end),None)
        if incoming:pairs.append((incoming['qpc']-ev['qpc'],ev,incoming))
    ticks,out,inc=max(pairs,key=lambda p:p[0])
    ready=next(e for e in d['events'] if e['name']=='Dispatcher/ReadyThread' and e['fields']['AwakenedThreadID']==tid and out['qpc']<=e['qpc']<=inc['qpc'])
    wt=ready['scheduled_tid']
    blocked=[[symbol(ip) for ip in s['ips']] for s in d['stacks'] if s['tid']==tid and s['event_qpc']==inc['qpc']]
    waker=[[symbol(ip) for ip in s['ips']] for s in d['stacks'] if s['tid']==wt and s['event_qpc']==ready['qpc']]
    row=dict(case=label,hitch_ms=(end-begin)/10000,main_tid=tid,wait_ms=ticks/10000,reason=out['fields']['OldThreadWaitReason'],
             wait_begin_qpc=out['qpc'],wait_end_qpc=inc['qpc'],ready_qpc=ready['qpc'],ready_to_run_ms=(inc['qpc']-ready['qpc'])/10000,
             waker_tid=wt,waker_pid=d['thread_pids'].get(str(wt)),blocked_stacks=blocked,waker_stacks=waker)
    results.append(row)
(HERE/'dxgi-waits.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
(HERE/'symbol-queries.json').write_text(json.dumps({k:sorted(v) for k,v in queries.items()},indent=2),encoding='utf-8')
for r in results:
    print(json.dumps({k:v for k,v in r.items() if not k.endswith('stacks')},indent=2))
    for label in ('blocked_stacks','waker_stacks'):
        print(label)
        for stack in r[label]:
            print('\n'.join(str(x.get('symbol') or str(x.get('file'))+'+'+hex(x.get('rva',0))) for x in stack))
