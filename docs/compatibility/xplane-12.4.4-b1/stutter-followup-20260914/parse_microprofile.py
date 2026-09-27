"""Parse literal capture data without executing the capture's JavaScript."""
import ast
from collections import defaultdict
import json
from pathlib import Path
import re
import sys

def parse(path):
    s=Path(path).read_text(encoding='utf-8')
    def literal(t):
        return ast.literal_eval(t)
    timers={}
    for index,args in re.findall(r'TimerInfo\[(\d+)\] = MakeTimer\((.*?)\);',s,re.S):
        a=literal('['+args+']')
        timers[int(index)]={'name':a[1],'group':a[2],'average_ms':a[5],'max_ms':a[6],'exclusive_average_ms':a[8],'calls':a[11]}
    names=literal(re.search(r'var ThreadNames = (\[.*?\]);',s).group(1))
    arrays={}
    for kind,index,body in re.findall(r'var (tt|ts|ti)(\d+) = (\[.*?\]);',s,re.S):
        if kind=='ts':
            def extra(m):
                tt=arrays['tt',int(m[3])][int(m[4])]
                return repr([float(m[2] if tt[i]==4 else m[1])*x for i,x in enumerate(literal(m[5]))])
            body=re.sub(r'MakeTimesExtra\(([^,]+),([^,]+),tt(\d+)\[(\d+)\],(\[.*?\])\)',extra,body)
            body=re.sub(r'MakeTimes\(([^,]+),(\[.*?\])\)',lambda m:repr([float(m[1])*x for x in literal(m[2])]),body)
        arrays[kind,int(index)]=literal(body)
    frames=[]
    for index,start,end in re.findall(r'Frames\[(\d+)\] = MakeFrame\([^,]+, ([^,]+), ([^,]+),',s):
        frames.append({'index':int(index),'start_ms':float(start),'end_ms':float(end),'duration_ms':float(end)-float(start)})
    scopes=[]
    for thread,name in enumerate(names):
        stack=[]
        for frame in frames:
            n=frame['index']
            for tt,ts,ti in zip(arrays['tt',n][thread],arrays['ts',n][thread],arrays['ti',n][thread]):
                if tt==1:
                    stack.append((ti,ts))
                elif tt==0:
                    if stack and stack[-1][0]==ti:
                        token,start=stack.pop()
                        if ts>=start:
                            scopes.append({'thread':name,'timer':timers[token]['name'],'start_ms':start,'end_ms':ts,'duration_ms':ts-start})
                    else:
                        stack=[] # leading/truncated scope; never invent its start.
    main=[x for x in scopes if x['thread']=='Main Thread']
    for f in frames:
        overlaps=[]
        for x in main:
            overlap=max(0,min(f['end_ms'],x['end_ms'])-max(f['start_ms'],x['start_ms']))
            if overlap>.25:
                overlaps.append({'timer':x['timer'],'overlap_ms':overlap,'scope_ms':x['duration_ms']})
        f['main_scopes']=sorted(overlaps,key=lambda x:x['overlap_ms'],reverse=True)[:12]
    return {'path':str(path),'frames':frames,'scope_count':len(scopes),
            'longest_gpu_scopes':sorted([x for x in scopes if x['thread']=='GPU'],key=lambda x:x['duration_ms'],reverse=True)[:30],
            'longest_main_scopes':sorted(main,key=lambda x:x['duration_ms'],reverse=True)[:30],
            'aggregate_timers':sorted(timers.values(),key=lambda x:x['max_ms'],reverse=True)[:30]}

if __name__=='__main__':
    p=Path(sys.argv[1]); d=parse(p)
    p.with_suffix('.analysis.json').write_text(json.dumps(d,indent=2)+'\n')
    print(json.dumps({'frames':len(d['frames']),'scopes':d['scope_count'],
        'longest_main_scopes':d['longest_main_scopes'][:12],
        'aggregate_timers':d['aggregate_timers'][:8]},indent=2))
