import bisect
from collections import Counter,defaultdict
import json
from pathlib import Path

H=Path(__file__).resolve().parent
symbols=json.loads((H.parent/'stutter-etl-analysis/resolved-symbols.json').read_text())
for f,v in json.loads((H/'resolved-symbols.json').read_text()).items():
    if f not in symbols:symbols[f]=v
    else:symbols[f]['names'].update(v.get('names',{}))
symbols={f.lower():v for f,v in symbols.items()}
queries=json.loads((H/'symbol-queries.json').read_text())
for label,a,b in [('dxgi-early',6105653814431,6105655148597),('dxgi-worst',6105766646143,6105768538783)]:
    d=json.loads((H/f'{label}.json').read_text())
    images=sorted({(i['address'],i['size'],i['file']) for i in d['images']});starts=[x[0] for x in images]
    def sym(ip):
        n=bisect.bisect_right(starts,ip)-1
        if n<0 or ip>=images[n][0]+images[n][1]:return hex(ip)
        base,size,f=images[n];rva=ip-base
        name=symbols.get(f.lower(),{}).get('names',{}).get(str(rva))
        if not name and Path(f).name.lower() in ('ntoskrnl.exe','dxgmms2.sys','dxgkrnl.sys','dxgi.dll','win32u.dll','ntdll.dll','kernelbase.dll','gdi32full.dll','win32kbase.sys','win32kfull.sys'):
            queries.setdefault(f,[]).append(rva)
        return f'{Path(f).name}!{name}' if name else f'{Path(f).name}+{rva:#x}'
    tid=60208
    samples=[e for e in d['events'] if e['name']=='PerfInfo/Sample' and a<=e['qpc']<=b]
    main=[e for e in samples if e['scheduled_tid']==tid]
    print(label,'samples',len(main),'all',len(samples))
    print(Counter(sym(int(e['fields']['InstructionPointer'])) for e in main).most_common(15))
    for sample in main[::max(1,len(main)//3)]:
        stack=[s for s in d['stacks'] if s['tid']==tid and s['event_qpc']==sample['qpc']]
        print('sample',sample['qpc'],[[sym(ip) for ip in s['ips']] for s in stack])
    names=defaultdict(set)
    for s in d['stacks']:
        if not a<=s['event_qpc']<=b:continue
        syms=[sym(ip) for ip in s['ips']]
        if any('GetFullMDL' in x or 'LockAllocationBackingStore' in x for x in syms):
            names[s['tid']].add(' -> '.join(syms[-15:]))
    print('memory-lock threads', {k:list(v)[:3] for k,v in names.items()})
    pending={}; waits=[]
    for e in d['events']:
        if e['name']!='Thread/CSwitch':continue
        incoming=e['fields']['NewThreadID']; old=e['fields']['OldThreadID']
        if incoming in pending and str(incoming) in d['threads']:
            start=pending[incoming]
            if start['qpc']<b and a<e['qpc'] and e['qpc']-start['qpc']>300000:
                stack=[s for s in d['stacks'] if s['tid']==incoming and s['event_qpc']==e['qpc']]
                waits.append(dict(tid=incoming,name=d['threads'][str(incoming)]['name'],ms=(e['qpc']-start['qpc'])/10000,
                                  start=start['qpc'],end=e['qpc'],reason=start['fields']['OldThreadWaitReason'],
                                  stack=[[sym(ip) for ip in s['ips']] for s in stack]))
        pending[old]=e
    print('long target waits',json.dumps(sorted(waits,key=lambda r:r['ms'],reverse=True)[:10]))
(H/'symbol-queries.json').write_text(json.dumps({k:sorted(set(v)) for k,v in queries.items()},indent=2))
