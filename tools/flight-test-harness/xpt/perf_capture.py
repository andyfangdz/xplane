"""Owned external captures. All timestamps use Windows QPC ticks."""
import ctypes
import hashlib
import json
from pathlib import Path
import subprocess
import time
import uuid
from .config import ROOT, atomic_json

PRESENTMON=ROOT/'bin/PresentMon/PresentMon-2.5.1-x64.exe'
PRESENTMON_SHA='9bec3083069f58f911e6a512f4806db51a27bd096103087bc1d05ef54c80a191'

def qpc():
    value=ctypes.c_longlong()
    if not ctypes.windll.kernel32.QueryPerformanceCounter(ctypes.byref(value)):
        raise OSError('QPC failed')
    return value.value

def check_presentmon():
    if not PRESENTMON.is_file() or hashlib.sha256(PRESENTMON.read_bytes()).hexdigest()!=PRESENTMON_SHA:
        raise RuntimeError('Run scripts/Install-PerformanceTools.ps1; pinned PresentMon is missing or changed')
    return {'path':str(PRESENTMON),'sha256':PRESENTMON_SHA,'version':'2.5.1'}

def decode_etl(directory,pid):
    directory=Path(directory)
    args=[str(PRESENTMON),'--etl_file',str(directory/'system.etl'),'--process_id',str(pid),
          '--output_file',str(directory/'presentmon.csv'),'--qpc_time','--no_console_stats']
    result=subprocess.run(args,capture_output=True,timeout=180,creationflags=subprocess.CREATE_NO_WINDOW)
    (directory/'presentmon-decode.log').write_bytes(result.stdout+result.stderr)
    atomic_json(directory/'presentmon-decode.json',dict(**check_presentmon(),arguments=args,exit_code=result.returncode,source='WPR ETL'))
    if result.returncode or not (directory/'presentmon.csv').is_file():
        raise RuntimeError('PresentMon could not extract presentation events from system.etl; inspect decode log')

class WprHost:
    def __init__(self,root):
        self.queue=Path(root)/'Output/performance-tests/.xpt-wpr-host'
        self.host=json.loads((self.queue/'host.json').read_text(encoding='utf-8-sig'))
        if not self.host.get('ready') or Path(self.host['root']).resolve()!=Path(root).resolve():
            raise RuntimeError('WPR host is not ready; start scripts/Start-WprHost.ps1 as administrator')
        expected=hashlib.sha256((ROOT/'scripts/XPT-Performance.wprp').read_bytes()).hexdigest()
        if self.host.get('protocol')!=2 or self.host.get('profile_sha256','').lower()!=expected:
            raise RuntimeError('Restart the WPR host with the current fixed presentation profile')
        # A stale ready file is rejected by a bounded nonce round trip at preflight.
    def request(self,action,**values):
        key=uuid.uuid4().hex
        atomic_json(self.queue/f'request-{key}.json',dict(nonce=self.host['nonce'],action=action,**values))
        response=self.queue/f'response-{key}.json'
        deadline=time.monotonic()+(300 if action=='stop' else 60)
        while not response.exists():
            if time.monotonic()>deadline: raise TimeoutError(f'WPR {action} timed out; request {key} retained')
            time.sleep(.25)
        value=json.loads(response.read_text(encoding='utf-8-sig'))
        if not value['ok']: raise RuntimeError(f'WPR {action}: {value["error"]}')
        return value['result']

class PresentMon:
    def __init__(self,directory,pid):
        self.directory=Path(directory); self.name='XPT_PM_'+uuid.uuid4().hex
        self.provenance=check_presentmon()
        self.args=[str(PRESENTMON),'--process_id',str(pid),'--output_file',str(self.directory/'presentmon.csv'),
                   '--qpc_time','--no_console_stats','--session_name',self.name]
        self.log=(self.directory/'presentmon.log').open('wb')
        self.process=None
    def start(self):
        self.process=subprocess.Popen(self.args,stdout=self.log,stderr=subprocess.STDOUT,
                                      creationflags=subprocess.CREATE_NO_WINDOW)
        # An owned name is persisted immediately so recovery can terminate only this capture.
        atomic_json(self.directory/'presentmon-capture.json',dict(**self.provenance,arguments=self.args,
                    session_name=self.name,pid=self.process.pid,target_pid=int(self.args[2]),start_qpc=qpc(),stopped=False))
        time.sleep(1)
        self.check()
    def check(self):
        if self.process.poll() is not None: raise RuntimeError('PresentMon exited early; see presentmon.log')
    def stop(self):
        if self.process is None:return
        if self.process.poll() is None:
            result=subprocess.run([str(PRESENTMON),'--session_name',self.name,'--terminate_existing_session'],
                capture_output=True,timeout=30,creationflags=subprocess.CREATE_NO_WINDOW)
            self.log.write(result.stdout+result.stderr)
        code=self.process.wait(timeout=30);self.log.close()
        path=self.directory/'presentmon-capture.json';v=json.loads(path.read_text())
        v.update(stopped=True,end_qpc=qpc(),exit_code=code);atomic_json(path,v)
        if code!=0:raise RuntimeError(f'PresentMon exit {code}')
