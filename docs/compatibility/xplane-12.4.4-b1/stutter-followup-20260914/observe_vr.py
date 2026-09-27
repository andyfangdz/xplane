"""Low-frequency, read-only proof of the simulator's actual VR mode."""
import http.client
import json
from pathlib import Path
import sys
import time

base=Path('D:/X-Plane 12/Output/performance-tests')
run=base/sys.argv[1]
out=base/'STUTTER_FOLLOWUP_20260914'/(sys.argv[1]+'-vr-mode.json')
refs=['sim/graphics/VR/enabled','sim/graphics/VR/using_3d_mouse','sim/time/paused','sim/time/total_flight_time_sec']
samples=[];catalog={};phase_key=None;deadline=time.monotonic()+1200
def get(path):
    c=http.client.HTTPConnection('127.0.0.1',8153,timeout=5)
    try:
        c.request('GET','/api/v3'+path);r=c.getresponse();data=r.read()
        if r.status!=200:raise RuntimeError(f'HTTP {r.status}')
        return json.loads(data)['data']
    finally:c.close()
while time.monotonic()<deadline and not (run/'restoration.json').exists():
    try:
        status=json.loads((run/'status.json').read_text())
        key=status.get('card')
        if not catalog or key!=phase_key:
            catalog={r['name']:r['id'] for r in get('/datarefs?limit=30000')};phase_key=key
        record={'request_epoch':time.time(),'phase':status['phase'],'card':key}
        for ref in refs:record[ref]=get(f'/datarefs/{catalog[ref]}/value')
        record['response_epoch']=time.time()
        samples.append(record);out.write_text(json.dumps(samples,indent=2))
        print(json.dumps(record),flush=True)
    except (OSError,ValueError,KeyError,RuntimeError):pass
    time.sleep(5)
print(json.dumps({'complete':True,'samples':len(samples)}),flush=True)
