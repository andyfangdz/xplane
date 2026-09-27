"""Read-only, bounded NVIDIA board-memory observer for an existing flight run."""
import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import queue
import subprocess
import sys
import threading
import time

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from xpt.config import atomic_json
from xpt.perf_capture import qpc


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run',type=Path,required=True)
    args=parser.parse_args()
    run=args.run.resolve()
    deadline=time.monotonic()+1500
    while not (run/'session.json').exists():
        if time.monotonic()>deadline:raise TimeoutError('Run was not created')
        time.sleep(.5)
    session=json.loads((run/'session.json').read_text(encoding='utf-8-sig'))
    run.relative_to(Path(session['xplane_root']).resolve()/'Output/performance-tests')
    if session.get('mode')!='Flight' or not session.get('trace_flight'):raise ValueError('TraceFlight run required')
    out=run/'gpu-memory'
    out.mkdir(exist_ok=False)
    executable=Path('C:/Windows/System32/nvidia-smi.exe')
    command=[str(executable),'--query-gpu=timestamp,memory.total,memory.reserved,memory.used,memory.free,utilization.gpu',
             '--format=csv,noheader,nounits','-lms','500']
    log=(out/'collector-error.log').open('wb')
    process=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=log,text=True,creationflags=subprocess.CREATE_NO_WINDOW)
    atomic_json(out/'collector.json',{'command':command,'pid':process.pid,'start_qpc':qpc(),
        'executable_sha256':hashlib.sha256(executable.read_bytes()).hexdigest(),
        'source_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'meaning':'Board-wide NVIDIA FB memory in MiB, sampled at 500 ms. Includes other applications. Receipt QPC is not exact query time. Free board memory is not X-Plane residency or its Windows memory budget.'})
    received=queue.Queue()
    def read_lines():
        for line in process.stdout:received.put((qpc(),line))
    reader=threading.Thread(target=read_lines,daemon=True)
    reader.start()
    rows=[]
    try:
        while time.monotonic()<deadline:
            if (run/'restoration.json').exists() or (run/'cancel.request').exists():break
            if process.poll() is not None:raise RuntimeError(f'NVIDIA collector exited {process.returncode}')
            try:tick,line=received.get(timeout=1)
            except queue.Empty:continue
            fields=next(csv.reader(io.StringIO(line)))
            if len(fields)!=6:raise ValueError(f'Unexpected NVIDIA memory record: {line.strip()}')
            rows.append([tick]+[v.strip() for v in fields])
        else:raise TimeoutError('GPU observer exceeded lifetime')
    except BaseException as error:
        atomic_json(out/'observer-error.json',{'qpc':qpc(),'error':str(error)})
        raise
    finally:
        if process.poll() is None:process.terminate()
        try:process.wait(timeout=5)
        except subprocess.TimeoutExpired:process.kill();process.wait(timeout=5)
        reader.join(timeout=1)
        log.close()
        with (out/'memory.csv').open('w',newline='',encoding='utf-8') as f:
            writer=csv.writer(f)
            writer.writerow(['receipt_qpc','nvidia_timestamp','total_mib','reserved_mib','used_mib','free_mib','gpu_util_percent'])
            writer.writerows(rows)
        atomic_json(out/'finished.json',{'end_qpc':qpc(),'samples':len(rows),'collector_stopped':process.poll() is not None})
    print(json.dumps({'finished':True,'samples':len(rows)}),flush=True)


if __name__=='__main__':main()
