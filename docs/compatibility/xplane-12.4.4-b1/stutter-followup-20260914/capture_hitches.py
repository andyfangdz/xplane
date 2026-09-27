"""Observe flushed native traces; capture preceding profiler frames after a hitch."""
import csv
import io
import json
from pathlib import Path
import sys
import time
import urllib.request

root=Path('D:/X-Plane 12')
run=root/'Output/performance-tests'/sys.argv[1]
out=root/'Output/performance-tests/STUTTER_FOLLOWUP_20260914'/sys.argv[1]
out.mkdir(exist_ok=True)
seen={}; last_capture=0.; captures=0; records=[]
deadline=time.monotonic()+1200
while time.monotonic()<deadline and not (run/'restoration.json').exists():
    for trace in (root/'Aircraft/X-Aviation/TorqueSim SR20/plugins/XPTNativeGuidance').glob('trace-*.csv'):
        if '--first-flight-only' in sys.argv and trace.name!='trace-1.csv':continue
        try:
            text=trace.read_text()
            rows=list(csv.DictReader(io.StringIO(text[:text.rfind('\n')+1])))
            previous=seen.get(trace.name,0)
            seen[trace.name]=len(rows)
            new=rows[previous:]
            hits=[r for r in new if float(r['wall_frame_dt_s'])>.050]
            if hits and captures<8 and time.monotonic()-last_capture>20:
                start=time.time(); before=time.perf_counter()
                last_capture=time.monotonic(); captures+=1
                record={'request_start_epoch':start,'native_trace':trace.name,'observed_rows':len(rows),
                        'last_native_wall_s':float(rows[-1]['wall_elapsed_s']),
                        'trigger_intervals':[{'wall_s':float(r['wall_elapsed_s']),'dt_s':float(r['wall_frame_dt_s'])} for r in hits]}
                name=f'capture-{captures:02d}.html'
                try:
                    with urllib.request.urlopen('http://127.0.0.1:1338/240',timeout=15) as response:
                        (out/name).write_bytes(response.read())
                    record['capture']=name
                except Exception as e:
                    record['error']=str(e)
                record['request_end_epoch']=time.time();record['request_duration_s']=time.perf_counter()-before
                records.append(record)
                (out/'capture-events.json').write_text(json.dumps(records,indent=2))
                print(json.dumps(record),flush=True)
        except (OSError,ValueError,KeyError,TypeError):
            pass # retry only complete flushed rows; retained traces remain authoritative.
    time.sleep(.5)
print(json.dumps({'stopped':True,'captures':captures,'restored':(run/'restoration.json').exists()}),flush=True)
