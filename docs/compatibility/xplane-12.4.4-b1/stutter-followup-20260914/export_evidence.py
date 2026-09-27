"""Copy derived evidence, never complete private simulator logs, into the report."""
import csv
import json
from pathlib import Path
import shutil

BASE=Path('D:/X-Plane 12/Output/performance-tests')
MASTER=BASE/'STUTTER_FOLLOWUP_20260914'
OUT=Path(__file__).resolve().parent

def read(path):return json.loads(path.read_text(encoding='utf-8-sig'))

if __name__=='__main__':
    captures=[]
    for name in ['XPT_USER_FLAG10_B_20260914','XPT_USER_FLAG10_VISIBLE_20260914']:
        for event in read(MASTER/name/'capture-events.json'):
            analysis=read((MASTER/name/event['capture']).with_suffix('.analysis.json'))
            captures.append({'run':name,**event,
                'frames':len(analysis['frames']),'scope_count':analysis['scope_count'],
                'longest_main_scopes':analysis['longest_main_scopes'],
                'aggregate_timers_latest_60_frames':analysis['aggregate_timers'],
                'gpu_scopes_available':bool(analysis['longest_gpu_scopes'])})
    (OUT/'profiler-evidence.json').write_text(json.dumps(captures,indent=2)+'\n')
    for name in ['audit.json','settings-change.json','visible-ui-activity.json',
                 'capture-free-ui-activity.json','offline-tests-texture.log']:
        shutil.copy2(MASTER/name,OUT/name)
    flights=[]
    for name in ['XPT_USER_FLAG10_B_20260914','XPT_USER_FLAG10_VISIBLE_20260914',
                 'XPT_USER_FLAG10_TEX4_20260914','XPT_USER_FLAG10_CONTROL_20260914']:
        data=read(MASTER/(name+'.json'))
        shutil.copy2(MASTER/(name+'.json'),OUT/(name+'.json'))
        for card in data['cards']:
            flights.append({'run':name,**{k:v for k,v in card.items()
                           if not isinstance(v,(dict,list))}})
    with (OUT/'flights.csv').open('w',newline='') as f:
        w=csv.DictWriter(f,fieldnames=list(flights[0]));w.writeheader();w.writerows(flights)
