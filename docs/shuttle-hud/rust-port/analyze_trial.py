from pathlib import Path
import csv,json,sys,math
R=Path(__file__).parent
def analyze(n):
    r=[{k:float(v) for k,v in x.items()} for x in csv.DictReader((R/f'trace-{n}.csv').open())]
    d=[x for x in r if x['time']>3]
    td=next((x for x in d if x['phase']==4),None)
    if td is None:return {'trial':n,'status':'incomplete'}
    pre=[x for x in d if x['time']<td['time']]
    ff=next(x for x in pre if x['phase']==3)
    locked=next(x for x in pre if min(x['gear0'],x['gear1'],x['gear2'])>.99)
    gear=next(x for x in pre if x['gear_cmd']>.5)
    above=[x for x in pre if 3000<x['height_ft']<4000]
    pull=[x for x in pre if 500<x['height_ft']<1800]
    inner=[x for x in pre if x['height_ft']<160 and x['phase']==2]
    stable=longest=0;last=None
    for x in inner:
        stable=stable+(x['time']-last['time']) if last and -2<x['gamma']<-1 and -2<last['gamma']<-1 else 0
        longest=max(longest,stable);last=x
    rollout=[x for x in d if x['time']>=td['time']]
    result=json.loads((R/f'result-{n}.json').read_text()) if (R/f'result-{n}.json').exists() else {}
    def actual(k,fallback):return result.get('shuttle_demo/touchdown_'+k,fallback)
    m={'trial':n,'touchdown':{'time_s':td['time'],'eas':actual('eas',td['eas']),'ias':actual('ias',td['ias']),'along_m':actual('along',td['along_m']),'cross_m':actual('cross',td['cross_m']),'sink_fps':-actual('vy',td['vy'])/.3048},
       'final_flare':{'time_s':ff['time'],'height_ft':ff['height_ft'],'along_m':ff['along_m'],'eas':ff['eas'],'duration_s':td['time']-ff['time']},
       'gear':{'command_height_ft':gear['height_ft'],'locked_margin_s':td['time']-locked['time']},
       'outer':{'eas_range':[min(x['eas'] for x in above),max(x['eas'] for x in above)],'gamma_range':[min(x['gamma'] for x in above),max(x['gamma'] for x in above)]},
       'preflare':{'g_range':[min(x['nz'] for x in pull),max(x['nz'] for x in pull)]},
       'inner_stable_seconds':longest,
       'rollout':{'max_agl_m_first10':max(x['agl_m'] for x in rollout if x['time']<td['time']+10),'max_pitch':max(x['pitch'] for x in rollout),'max_cross_m':max(abs(x['cross_m']) for x in rollout),'final_along_m':r[-1]['along_m'],'final_groundspeed_kt':r[-1]['groundspeed_kt'],'final_phase':result.get('shuttle_demo/phase')}}
    curve={}
    for h in [150,100,80,60,50,40,30,20,10,5]:
        pair=next(((a,b) for a,b in zip(pre,pre[1:]) if a['height_ft']>=h>b['height_ft']),None)
        if pair:
            a,b=pair;w=(a['height_ft']-h)/(a['height_ft']-b['height_ft'])
            curve[str(h)]={k:a[k]+w*(b[k]-a[k]) for k in ['time','eas','along_m','vy','pitch']}
    m['height_speed']=curve
    m['mass_lb']=pre[-1]['mass']/.45359237
    if 'radar_height_ft' in pre[0]:
        radar_curve={}
        for h in [100,80,60,50,40,30,20,10]:
            pair=next(((a,b) for a,b in zip(pre,pre[1:]) if a['radar_height_ft']>=h>b['radar_height_ft']),None)
            if pair:
                a,b=pair;w=(a['radar_height_ft']-h)/(a['radar_height_ft']-b['radar_height_ft'])
                radar_curve[str(h)]={k:a[k]+w*(b[k]-a[k]) for k in ['time','eas','height_ft','along_m','vy','pitch']}
        m['radar_height_speed']=radar_curve
    t=m['touchdown'];m['checks']={'touchdown_speed':195<=t['eas']<=205,'touchdown_distance':1500*.3048<=t['along_m']<=3500*.3048,'touchdown_sink':0<=t['sink_fps']<=5,'touchdown_centerline':abs(t['cross_m'])<10,'gear':200<=gear['height_ft']<=400 and m['gear']['locked_margin_s']>=5,'flare_band':30<=ff['height_ft']<=81,'inner_transition':any(-2<x['gamma']<-1 for x in inner),'no_significant_bounce':m['rollout']['max_agl_m_first10']<.75,'stopped':m['rollout']['final_phase']==5}
    m['accepted']=all(m['checks'].values());(R/f'analysis-{n}.json').write_text(json.dumps(m,indent=2));return m
if __name__=='__main__':
    for n in sys.argv[1:]:print(json.dumps(analyze(int(n)),indent=2))
